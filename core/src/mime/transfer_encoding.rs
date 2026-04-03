/*
 * transfer_encoding.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

//! Decode Content-Transfer-Encoding for raw MIME / IMAP BODY[] part bytes.

use base64::Engine;

/// Decode a body part fetched via IMAP `BODY.PEEK[section]` using the encoding from `BODYSTRUCTURE`.
pub fn decode_content_transfer_encoding(encoding: &str, raw: &[u8]) -> Vec<u8> {
    let enc = encoding.trim().to_ascii_uppercase();
    match enc.as_str() {
        "BASE64" | "B" => decode_base64_relaxed(raw),
        "QUOTED-PRINTABLE" | "Q" => decode_quoted_printable_all(raw),
        "7BIT" | "8BIT" | "BINARY" | "8-BIT" | "" => raw.to_vec(),
        _ => raw.to_vec(),
    }
}

fn decode_base64_relaxed(raw: &[u8]) -> Vec<u8> {
    let cleaned: Vec<u8> = raw
        .iter()
        .copied()
        .filter(|b| !matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
        .collect();
    base64::engine::general_purpose::STANDARD
        .decode(cleaned)
        .unwrap_or_else(|_| raw.to_vec())
}

fn decode_quoted_printable_all(src: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(src.len());
    let mut i = 0usize;
    while i < src.len() {
        let b = src[i];
        if b != b'=' {
            out.push(b);
            i += 1;
            continue;
        }
        let remaining = src.len() - i;
        if remaining >= 3 {
            let h1 = src[i + 1];
            let h2 = src[i + 2];
            if let (Some(v1), Some(v2)) = (hex_val(h1), hex_val(h2)) {
                out.push((v1 << 4) | v2);
                i += 3;
                continue;
            }
            if h1 == b'\r' && h2 == b'\n' {
                i += 3;
                continue;
            }
            if h1 == b'\n' {
                i += 2;
                continue;
            }
        } else if remaining == 2 && src[i + 1] == b'\n' {
            i += 2;
            continue;
        }
        out.push(b);
        i += 1;
    }
    out
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'A'..=b'F' => Some(b - b'A' + 10),
        b'a'..=b'f' => Some(b - b'a' + 10),
        _ => None,
    }
}

/// Incremental base64 decoder for literal streaming (IMAP / HTTP).
#[derive(Default)]
pub struct StreamingBase64Decoder {
    buf: Vec<u8>,
}

impl StreamingBase64Decoder {
    pub fn feed(&mut self, input: &[u8], out: &mut Vec<u8>) {
        for &b in input {
            if matches!(b, b' ' | b'\t' | b'\r' | b'\n') {
                continue;
            }
            self.buf.push(b);
            while self.buf.len() >= 4 {
                let chunk: Vec<u8> = self.buf.drain(..4).collect();
                let dec = decode_content_transfer_encoding("BASE64", &chunk);
                out.extend_from_slice(&dec);
            }
        }
    }

    pub fn finish(&mut self, out: &mut Vec<u8>) {
        if !self.buf.is_empty() {
            let dec = decode_content_transfer_encoding("BASE64", &self.buf);
            out.extend_from_slice(&dec);
            self.buf.clear();
        }
    }
}

/// Incremental quoted-printable decoder (handles soft line breaks `=<CR><LF>`).
#[derive(Default)]
pub struct StreamingQuotedPrintableDecoder {
    line: Vec<u8>,
}

impl StreamingQuotedPrintableDecoder {
    pub fn feed(&mut self, input: &[u8], out: &mut Vec<u8>) {
        for &b in input {
            if b == b'\n' {
                if self.line.last() == Some(&b'=') {
                    self.line.pop();
                    continue;
                }
                let dec = decode_quoted_printable_all(&self.line);
                out.extend_from_slice(&dec);
                self.line.clear();
            } else if b != b'\r' {
                self.line.push(b);
            }
        }
    }

    pub fn finish(&mut self, out: &mut Vec<u8>) {
        if !self.line.is_empty() {
            let dec = decode_quoted_printable_all(&self.line);
            out.extend_from_slice(&dec);
            self.line.clear();
        }
    }
}

/// Decoder matching [`decode_content_transfer_encoding`] but for streaming input chunks.
pub enum StreamingCteDecoder {
    Base64(StreamingBase64Decoder),
    QuotedPrintable(StreamingQuotedPrintableDecoder),
    Passthrough,
}

impl StreamingCteDecoder {
    pub fn for_encoding(encoding: &str) -> Self {
        let enc = encoding.trim().to_ascii_uppercase();
        match enc.as_str() {
            "BASE64" | "B" => Self::Base64(StreamingBase64Decoder::default()),
            "QUOTED-PRINTABLE" | "Q" => {
                Self::QuotedPrintable(StreamingQuotedPrintableDecoder::default())
            }
            _ => Self::Passthrough,
        }
    }

    pub fn feed(&mut self, chunk: &[u8], out: &mut Vec<u8>) {
        match self {
            Self::Base64(d) => d.feed(chunk, out),
            Self::QuotedPrintable(d) => d.feed(chunk, out),
            Self::Passthrough => out.extend_from_slice(chunk),
        }
    }

    pub fn finish(&mut self, out: &mut Vec<u8>) {
        match self {
            Self::Base64(d) => d.finish(out),
            Self::QuotedPrintable(d) => d.finish(out),
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
                        out.push_str(
                            std::str::from_utf8(&self.pending[..n]).expect("valid_up_to invariant"),
                        );
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
    fn qp_simple() {
        let s = b"hello=20world";
        assert_eq!(decode_quoted_printable_all(s), b"hello world");
    }

    #[test]
    fn b64_round() {
        let raw = b"Zm9vYmFy"; // "foobar"
        let out = decode_content_transfer_encoding("BASE64", raw);
        assert_eq!(out, b"foobar");
    }

    #[test]
    fn streaming_b64_matches_batch() {
        let full = b"Zm9vYmFy";
        let mut dec = StreamingCteDecoder::for_encoding("BASE64");
        let mut out = Vec::new();
        dec.feed(&full[..3], &mut out);
        dec.feed(&full[3..], &mut out);
        dec.finish(&mut out);
        assert_eq!(out, decode_content_transfer_encoding("BASE64", full));
    }

    #[test]
    fn streaming_qp_soft_break() {
        let mut dec = StreamingQuotedPrintableDecoder::default();
        let mut out = Vec::new();
        dec.feed(b"hello=\nworld\n", &mut out);
        dec.finish(&mut out);
        assert_eq!(out, b"helloworld");
    }
}
