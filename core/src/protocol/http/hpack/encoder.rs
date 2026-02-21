/*
 * encoder.rs
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

//! HPACK encoder (RFC 7541). Encodes HTTP/2 request headers using static table
//! indexing and Huffman encoding.

use bytes::BufMut;
use std::io;

use super::huffman;
use super::static_table::STATIC_TABLE;

/// Encode a list of (name, value) request headers into an HPACK header block.
///
/// Uses indexed representation for exact matches in the static table,
/// literal with name index for name-only matches, and literal without
/// indexing (with Huffman) for everything else.
pub fn encode_request_headers(headers: &[(&str, &str)], out: &mut impl BufMut) -> io::Result<()> {
    for &(name, value) in headers {
        if let Some(idx) = find_static_exact(name, value) {
            encode_indexed(idx, out);
        } else if let Some(idx) = find_static_name(name) {
            encode_literal_with_name_index(idx, value.as_bytes(), out)?;
        } else {
            encode_literal_new_name(name.as_bytes(), value.as_bytes(), out)?;
        }
    }
    Ok(())
}

fn find_static_exact(name: &str, value: &str) -> Option<usize> {
    STATIC_TABLE
        .iter()
        .position(|&(n, v)| n == name && v == Some(value))
}

fn find_static_name(name: &str) -> Option<usize> {
    STATIC_TABLE
        .iter()
        .position(|&(n, _)| n == name)
}

/// Indexed header field (RFC 7541 6.1): 1-bit prefix + 7-bit index.
fn encode_indexed(index: usize, out: &mut impl BufMut) {
    encode_integer(index as u64, 7, 0x80, out);
}

/// Literal without indexing, name from static table index (RFC 7541 6.2.2).
/// Prefix: 0000, 4-bit index for name.
fn encode_literal_with_name_index(
    name_index: usize,
    value: &[u8],
    out: &mut impl BufMut,
) -> io::Result<()> {
    encode_integer(name_index as u64, 4, 0x00, out);
    encode_string_huffman(value, out);
    Ok(())
}

/// Literal without indexing, new name (RFC 7541 6.2.2).
/// Prefix: 0000 0000, then name string, then value string.
fn encode_literal_new_name(
    name: &[u8],
    value: &[u8],
    out: &mut impl BufMut,
) -> io::Result<()> {
    out.put_u8(0x00);
    encode_string_huffman(name, out);
    encode_string_huffman(value, out);
    Ok(())
}

/// Encode a string with Huffman if it saves space, plain otherwise.
fn encode_string_huffman(s: &[u8], out: &mut impl BufMut) {
    let huff_len = huffman::encoded_length(s);
    if huff_len < s.len() {
        let encoded = huffman::encode(s);
        encode_integer(encoded.len() as u64, 7, 0x80, out);
        out.put_slice(&encoded);
    } else {
        encode_integer(s.len() as u64, 7, 0x00, out);
        out.put_slice(s);
    }
}

fn encode_integer(mut value: u64, nbits: u8, prefix: u8, out: &mut impl BufMut) {
    let max_prefix = (1u64 << nbits) - 1;
    if value < max_prefix {
        out.put_u8(prefix | value as u8);
        return;
    }
    out.put_u8(prefix | max_prefix as u8);
    value -= max_prefix;
    while value >= 128 {
        out.put_u8(0x80 | (value % 128) as u8);
        value /= 128;
    }
    out.put_u8(value as u8);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::http::hpack::{Decoder, HeaderHandler};

    struct CollectHeaders(Vec<(String, String)>);
    impl HeaderHandler for CollectHeaders {
        fn header(&mut self, name: &str, value: &str) {
            self.0.push((name.to_string(), value.to_string()));
        }
    }

    fn roundtrip_headers(input: &[(&str, &str)]) -> Vec<(String, String)> {
        let mut buf = bytes::BytesMut::new();
        encode_request_headers(input, &mut buf).unwrap();
        let mut decoder = Decoder::new(4096);
        let mut collector = CollectHeaders(Vec::new());
        let mut cursor = &buf[..];
        decoder.decode(&mut cursor, &mut collector).unwrap();
        collector.0
    }

    #[test]
    fn roundtrip_get_request() {
        let headers = &[
            (":method", "GET"),
            (":scheme", "https"),
            (":authority", "example.com"),
            (":path", "/"),
        ];
        let decoded = roundtrip_headers(headers);
        assert_eq!(decoded.len(), 4);
        assert_eq!(decoded[0], (":method".into(), "GET".into()));
        assert_eq!(decoded[1], (":scheme".into(), "https".into()));
        assert_eq!(decoded[2], (":authority".into(), "example.com".into()));
        assert_eq!(decoded[3], (":path".into(), "/".into()));
    }

    #[test]
    fn roundtrip_post_with_headers() {
        let headers = &[
            (":method", "POST"),
            (":scheme", "https"),
            (":authority", "oauth2.googleapis.com"),
            (":path", "/token"),
            ("content-type", "application/x-www-form-urlencoded"),
            ("content-length", "42"),
        ];
        let decoded = roundtrip_headers(headers);
        assert_eq!(decoded.len(), 6);
        assert_eq!(decoded[0].1, "POST");
        assert_eq!(decoded[4], ("content-type".into(), "application/x-www-form-urlencoded".into()));
    }

    #[test]
    fn static_table_exact_match_uses_indexed() {
        // :method GET is static table index 2 → should emit single indexed byte
        let mut buf = bytes::BytesMut::new();
        encode_request_headers(&[(":method", "GET")], &mut buf).unwrap();
        // Indexed representation: high bit set, index 2 → 0x82
        assert_eq!(buf[0], 0x82);
    }

    #[test]
    fn static_table_exact_match_post() {
        let mut buf = bytes::BytesMut::new();
        encode_request_headers(&[(":method", "POST")], &mut buf).unwrap();
        // :method POST is index 3 → 0x83
        assert_eq!(buf[0], 0x83);
    }

    #[test]
    fn static_table_exact_match_scheme_https() {
        let mut buf = bytes::BytesMut::new();
        encode_request_headers(&[(":scheme", "https")], &mut buf).unwrap();
        // :scheme https is index 7 → 0x87
        assert_eq!(buf[0], 0x87);
    }

    #[test]
    fn static_table_exact_match_path_root() {
        let mut buf = bytes::BytesMut::new();
        encode_request_headers(&[(":path", "/")], &mut buf).unwrap();
        // :path / is index 4 → 0x84
        assert_eq!(buf[0], 0x84);
    }

    #[test]
    fn encode_integer_small() {
        let mut buf = bytes::BytesMut::new();
        encode_integer(10, 7, 0x00, &mut buf);
        assert_eq!(buf[0], 10);
    }

    #[test]
    fn encode_integer_at_max_prefix() {
        let mut buf = bytes::BytesMut::new();
        encode_integer(127, 7, 0x00, &mut buf);
        // 127 == max prefix for 7-bit, so must be multi-byte
        assert_eq!(buf[0], 0x7f);
        assert_eq!(buf[1], 0); // 127 - 127 = 0
    }

    #[test]
    fn encode_integer_large() {
        let mut buf = bytes::BytesMut::new();
        encode_integer(300, 7, 0x00, &mut buf);
        assert_eq!(buf[0], 0x7f);
        // 300 - 127 = 173 → 173 < 128, so single continuation byte
        assert_eq!(buf[1], 173u8);
    }
}
