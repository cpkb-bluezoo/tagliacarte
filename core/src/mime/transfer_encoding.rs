/*
 * transfer_encoding.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte, a cross-platform email client.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Tagliacarte is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Tagliacarte.  If not, see <http://www.gnu.org/licenses/>.
 */

//! Transfer decoding for bodies fetched outside a full MIME parse: whole
//! IMAP sections, and the mail-view server's streamed literals. The codecs
//! are rmimeparser's push decoders; the streaming wrappers keep whatever
//! the decoder could not consume yet and re-present it with the next chunk.

use rmimeparser::{
    decode_base64, decode_quoted_printable, decode_uuencode, estimate_base64_decoded_size,
    estimate_qp_decoded_size, estimate_uuencode_decoded_size,
};

/// Decode a body part fetched via IMAP `BODY.PEEK[section]` using the
/// encoding from `BODYSTRUCTURE`. Unknown encodings pass the bytes through.
pub fn decode_content_transfer_encoding(encoding: &str, raw: &[u8]) -> Vec<u8> {
    let mut d = StreamingCteDecoder::for_encoding(encoding);
    let mut out = Vec::with_capacity(raw.len());
    d.feed(raw, &mut out);
    d.finish(&mut out);
    out
}

/// Which push decoder a streaming wrapper drives.
#[derive(Clone, Copy)]
enum Codec {
    Base64,
    QuotedPrintable,
    Uuencode,
}

impl Codec {
    fn run(self, src: &mut &[u8], out: &mut Vec<u8>, end_of_stream: bool) {
        let len = src.len();
        match self {
            Codec::Base64 => {
                decode_base64(src, out, estimate_base64_decoded_size(len), end_of_stream, false);
            }
            Codec::QuotedPrintable => {
                decode_quoted_printable(src, out, estimate_qp_decoded_size(len), end_of_stream);
            }
            Codec::Uuencode => {
                decode_uuencode(src, out, estimate_uuencode_decoded_size(len), end_of_stream);
            }
        }
    }
}

/// Incremental decoder over one codec, keeping the undecodable tail of
/// each chunk for the next.
pub struct StreamingDecoder {
    codec: Codec,
    pending: Vec<u8>,
}

impl StreamingDecoder {
    fn new(codec: Codec) -> Self {
        Self { codec, pending: Vec::new() }
    }

    pub fn feed(&mut self, input: &[u8], out: &mut Vec<u8>) {
        self.pending.extend_from_slice(input);
        let mut src = self.pending.as_slice();
        self.codec.run(&mut src, out, false);
        let left = src.len();
        let consumed = self.pending.len() - left;
        self.pending.drain(..consumed);
    }

    pub fn finish(&mut self, out: &mut Vec<u8>) {
        if self.pending.is_empty() {
            return;
        }
        let pending = std::mem::take(&mut self.pending);
        let mut src = pending.as_slice();
        self.codec.run(&mut src, out, true);
    }
}

/// Incremental base64 decoder for literal streaming (IMAP / HTTP).
pub struct StreamingBase64Decoder(StreamingDecoder);

impl Default for StreamingBase64Decoder {
    fn default() -> Self {
        Self(StreamingDecoder::new(Codec::Base64))
    }
}

impl StreamingBase64Decoder {
    pub fn feed(&mut self, input: &[u8], out: &mut Vec<u8>) {
        self.0.feed(input, out)
    }
    pub fn finish(&mut self, out: &mut Vec<u8>) {
        self.0.finish(out)
    }
}

/// Incremental quoted-printable decoder (handles soft line breaks).
pub struct StreamingQuotedPrintableDecoder(StreamingDecoder);

impl Default for StreamingQuotedPrintableDecoder {
    fn default() -> Self {
        Self(StreamingDecoder::new(Codec::QuotedPrintable))
    }
}

impl StreamingQuotedPrintableDecoder {
    pub fn feed(&mut self, input: &[u8], out: &mut Vec<u8>) {
        self.0.feed(input, out)
    }
    pub fn finish(&mut self, out: &mut Vec<u8>) {
        self.0.finish(out)
    }
}

/// Incremental uuencode decoder (`begin`/`end` framing skipped).
pub struct StreamingUuencodeDecoder(StreamingDecoder);

impl Default for StreamingUuencodeDecoder {
    fn default() -> Self {
        Self(StreamingDecoder::new(Codec::Uuencode))
    }
}

impl StreamingUuencodeDecoder {
    pub fn feed(&mut self, input: &[u8], out: &mut Vec<u8>) {
        self.0.feed(input, out)
    }
    pub fn finish(&mut self, out: &mut Vec<u8>) {
        self.0.finish(out)
    }
}

/// Decoder matching [`decode_content_transfer_encoding`] for streaming input chunks.
pub enum StreamingCteDecoder {
    Base64(StreamingBase64Decoder),
    QuotedPrintable(StreamingQuotedPrintableDecoder),
    Uuencode(StreamingUuencodeDecoder),
    Passthrough,
}

impl StreamingCteDecoder {
    pub fn for_encoding(encoding: &str) -> Self {
        let enc = encoding.trim().to_ascii_uppercase();
        match enc.as_str() {
            "BASE64" | "B" => Self::Base64(StreamingBase64Decoder::default()),
            "QUOTED-PRINTABLE" | "Q" => Self::QuotedPrintable(StreamingQuotedPrintableDecoder::default()),
            "X-UUENCODE" | "X-UUE" | "UUENCODE" | "UUE" => Self::Uuencode(StreamingUuencodeDecoder::default()),
            _ => Self::Passthrough,
        }
    }

    pub fn feed(&mut self, chunk: &[u8], out: &mut Vec<u8>) {
        match self {
            Self::Base64(d) => d.feed(chunk, out),
            Self::QuotedPrintable(d) => d.feed(chunk, out),
            Self::Uuencode(d) => d.feed(chunk, out),
            Self::Passthrough => out.extend_from_slice(chunk),
        }
    }

    pub fn finish(&mut self, out: &mut Vec<u8>) {
        match self {
            Self::Base64(d) => d.finish(out),
            Self::QuotedPrintable(d) => d.finish(out),
            Self::Uuencode(d) => d.finish(out),
            Self::Passthrough => {}
        }
    }
}

/// Accumulates bytes and releases valid UTF-8 prefixes (handles split codepoints).
#[derive(Default)]
pub struct Utf8StreamAssembler {
    pending: Vec<u8>,
}

impl Utf8StreamAssembler {
    pub fn push_decode(&mut self, bytes: &[u8]) -> String {
        self.pending.extend_from_slice(bytes);
        let mut out = String::new();
        loop {
            if self.pending.is_empty() {
                break;
            }
            match std::str::from_utf8(&self.pending) {
                Ok(s) => {
                    out.push_str(s);
                    self.pending.clear();
                    break;
                }
                Err(e) => {
                    let n = e.valid_up_to();
                    if n > 0 {
                        out.push_str(std::str::from_utf8(&self.pending[..n]).expect("valid_up_to invariant"));
                        self.pending.drain(..n);
                        continue;
                    }
                    let elen = e.error_len().unwrap_or(0);
                    if elen > 0 {
                        out.push('\u{FFFD}');
                        self.pending.drain(..elen);
                        continue;
                    }
                    break;
                }
            }
        }
        out
    }

    pub fn flush_utf8_lossy(&mut self) -> String {
        if self.pending.is_empty() {
            return String::new();
        }
        let s = String::from_utf8_lossy(&self.pending).into_owned();
        self.pending.clear();
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_decoding() {
        assert_eq!(decode_content_transfer_encoding("BASE64", b"Zm9vYmFy"), b"foobar");
        assert_eq!(decode_content_transfer_encoding("quoted-printable", b"hello=20world=\r\n!"), b"hello world!");
        assert_eq!(decode_content_transfer_encoding("x-uuencode", b"begin 644 f\r\n#0V%T\r\n`\r\nend\r\n"), b"Cat");
        assert_eq!(decode_content_transfer_encoding("8bit", b"as is"), b"as is");
    }

    #[test]
    fn streaming_matches_whole_at_every_split() {
        let mut uu = Vec::new();
        rmimeparser::encode_uuencode(&mut uu, "f", 0o644, b"Hello, uu").unwrap();
        let cases: [(&str, &[u8], &[u8]); 3] = [
            ("base64", b"SGVsbG8sIHN0cmVhbWluZyB3b3JsZCE=\r\n", b"Hello, streaming world!"),
            ("quoted-printable", b"caf=C3=A9 =\r\nau lait=3D\r\n", b"caf\xc3\xa9 au lait=\r\n"),
            ("x-uuencode", &uu, b"Hello, uu"),
        ];
        for (enc, input, expected) in cases {
            for step in 1..=input.len() {
                let mut d = StreamingCteDecoder::for_encoding(enc);
                let mut out = Vec::new();
                for chunk in input.chunks(step) {
                    d.feed(chunk, &mut out);
                }
                d.finish(&mut out);
                assert_eq!(out, expected, "{enc} step {step}");
            }
        }
    }

    #[test]
    fn utf8_assembler_handles_split_codepoints() {
        let mut a = Utf8StreamAssembler::default();
        let s = "h\u{e9}llo".as_bytes();
        let mut out = String::new();
        for b in s {
            out.push_str(&a.push_decode(std::slice::from_ref(b)));
        }
        out.push_str(&a.flush_utf8_lossy());
        assert_eq!(out, "h\u{e9}llo");
    }
}
