/*
 * decoder.rs
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

//! HPACK decoder (RFC 7541). Decodes header blocks into (name, value) pairs.
//! Supports indexed (static table), literal with/without indexing, and Huffman-encoded strings.

use bytes::Buf;
use std::collections::VecDeque;
use std::io;

use super::huffman;
use super::static_table::{STATIC_TABLE, STATIC_TABLE_SIZE};

/// Decoded header (name, value).
#[derive(Debug, Clone)]
pub struct Header {
    pub name: String,
    pub value: String,
}

/// Callback for each decoded header.
pub trait HeaderHandler {
    fn header(&mut self, name: &str, value: &str);
}

/// HPACK decoder. Uses static table and optional dynamic table.
pub struct Decoder {
    header_table_size: usize,
    dynamic_table: VecDeque<Header>,
    max_size: usize,
}

impl Decoder {
    pub fn new(header_table_size: usize) -> Self {
        Self {
            header_table_size,
            dynamic_table: VecDeque::new(),
            max_size: header_table_size,
        }
    }

    pub fn set_header_table_size(&mut self, size: usize) {
        self.header_table_size = size;
    }

    /// Decode a header block. Calls handler for each header.
    pub fn decode<B: Buf, H: HeaderHandler>(&mut self, buf: &mut B, handler: &mut H) -> io::Result<()> {
        while buf.has_remaining() {
            let b = buf.get_u8();
            if (b & 0x80) != 0 {
                // Indexed header field (7-bit index)
                let index = decode_integer(buf, b, 7)?;
                if index == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "HPACK indexed header index 0",
                    ));
                }
                let (name, value) = self.get_indexed(index)?;
                handler.header(&name, &value);
            } else if (b & 0x40) != 0 {
                // Literal with incremental indexing (6-bit index)
                let (name, value) = self.get_literal(buf, b, 6)?;
                self.add_to_dynamic(name.clone(), value.clone());
                handler.header(&name, &value);
            } else if (b & 0x20) != 0 {
                // Dynamic table size update (5-bit)
                let max_size = decode_integer(buf, b, 5)? as usize;
                if max_size > self.header_table_size {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "HPACK dynamic table size exceeds SETTINGS",
                    ));
                }
                self.evict_to(max_size);
                self.max_size = max_size;
            } else {
                // Literal without indexing (4-bit) or never indexed
                let (name, value) = self.get_literal(buf, b, 4)?;
                handler.header(&name, &value);
            }
        }
        Ok(())
    }

    fn get_indexed(&self, index: u64) -> io::Result<(String, String)> {
        if index < STATIC_TABLE_SIZE as u64 {
            let (name, value) = STATIC_TABLE[index as usize];
            Ok((
                name.to_string(),
                value.unwrap_or("").to_string(),
            ))
        } else {
            let dyn_index = index - STATIC_TABLE_SIZE as u64;
            let idx = dyn_index as usize;
            if idx < self.dynamic_table.len() {
                let h = &self.dynamic_table[idx];
                Ok((h.name.clone(), h.value.clone()))
            } else {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "HPACK index out of range",
                ))
            }
        }
    }

    fn get_literal<B: Buf>(&self, buf: &mut B, opcode: u8, nbits: u8) -> io::Result<(String, String)> {
        let index = decode_integer(buf, opcode, nbits)?;
        let name = if index == 0 {
            decode_string(buf)?
        } else {
            let (n, _) = self.get_indexed(index)?;
            n
        };
        let value = decode_string(buf)?;
        Ok((name, value))
    }

    fn add_to_dynamic(&mut self, name: String, value: String) {
        let entry_size = name.len() + value.len() + 32;
        while self.dynamic_size() + entry_size > self.max_size && !self.dynamic_table.is_empty() {
            self.dynamic_table.pop_back();
        }
        if self.dynamic_size() + entry_size <= self.max_size {
            self.dynamic_table.push_front(Header { name, value });
        }
    }

    fn dynamic_size(&self) -> usize {
        self.dynamic_table
            .iter()
            .map(|h| h.name.len() + h.value.len() + 32)
            .sum()
    }

    fn evict_to(&mut self, max: usize) {
        while self.dynamic_size() > max && !self.dynamic_table.is_empty() {
            self.dynamic_table.pop_back();
        }
    }
}

fn decode_integer<B: Buf>(buf: &mut B, opcode: u8, nbits: u8) -> io::Result<u64> {
    let nmask = (1u64 << nbits) - 1;
    let mut value = (opcode & (nmask as u8)) as u64;
    if value < nmask {
        return Ok(value);
    }
    let mut shift = 0u32;
    loop {
        if !buf.has_remaining() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "HPACK integer overflow",
            ));
        }
        let b = buf.get_u8();
        value += ((b & 0x7f) as u64) << shift;
        if (b & 0x80) == 0 {
            break;
        }
        shift += 7;
        if shift > 63 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "HPACK integer too large",
            ));
        }
    }
    Ok(value)
}

fn decode_string<B: Buf>(buf: &mut B) -> io::Result<String> {
    if !buf.has_remaining() {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "HPACK string length",
        ));
    }
    let b = buf.get_u8();
    let huffman = (b & 0x80) != 0;
    let len = decode_integer(buf, b, 7)? as usize;
    if buf.remaining() < len {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "HPACK string truncated",
        ));
    }
    let mut bytes = vec![0u8; len];
    buf.copy_to_slice(&mut bytes);
    if huffman {
        let decoded = huffman::decode(&bytes)?;
        return String::from_utf8(decoded).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "HPACK Huffman string not UTF-8")
        });
    }
    String::from_utf8(bytes).map_err(|_| {
        io::Error::new(io::ErrorKind::InvalidData, "HPACK string not UTF-8")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct CollectHeaders(Vec<(String, String)>);
    impl HeaderHandler for CollectHeaders {
        fn header(&mut self, name: &str, value: &str) {
            self.0.push((name.to_string(), value.to_string()));
        }
    }

    #[test]
    fn decode_indexed_static() {
        // 0x82 = indexed, index 2 = :method GET
        let data: &[u8] = &[0x82];
        let mut decoder = Decoder::new(4096);
        let mut collector = CollectHeaders(Vec::new());
        decoder.decode(&mut &data[..], &mut collector).unwrap();
        assert_eq!(collector.0.len(), 1);
        assert_eq!(collector.0[0], (":method".into(), "GET".into()));
    }

    #[test]
    fn decode_multiple_indexed() {
        // 0x82 = :method GET, 0x87 = :scheme https, 0x84 = :path /
        let data: &[u8] = &[0x82, 0x87, 0x84];
        let mut decoder = Decoder::new(4096);
        let mut collector = CollectHeaders(Vec::new());
        decoder.decode(&mut &data[..], &mut collector).unwrap();
        assert_eq!(collector.0.len(), 3);
        assert_eq!(collector.0[0].1, "GET");
        assert_eq!(collector.0[1].1, "https");
        assert_eq!(collector.0[2].1, "/");
    }

    #[test]
    fn decode_literal_without_indexing_plain() {
        // 0x00 = literal without indexing, new name
        // name: 3 bytes "foo", value: 3 bytes "bar"
        let data: &[u8] = &[
            0x00, // literal, new name
            0x03, b'f', b'o', b'o', // name (not Huffman, len 3)
            0x03, b'b', b'a', b'r', // value (not Huffman, len 3)
        ];
        let mut decoder = Decoder::new(4096);
        let mut collector = CollectHeaders(Vec::new());
        decoder.decode(&mut &data[..], &mut collector).unwrap();
        assert_eq!(collector.0.len(), 1);
        assert_eq!(collector.0[0], ("foo".into(), "bar".into()));
    }

    #[test]
    fn decode_literal_with_indexing() {
        // 0x40 = literal with incremental indexing, new name
        let data: &[u8] = &[
            0x40, // literal with indexing, new name
            0x04, b't', b'e', b's', b't', // name
            0x05, b'v', b'a', b'l', b'u', b'e', // value
        ];
        let mut decoder = Decoder::new(4096);
        let mut collector = CollectHeaders(Vec::new());
        decoder.decode(&mut &data[..], &mut collector).unwrap();
        assert_eq!(collector.0[0], ("test".into(), "value".into()));
    }

    #[test]
    fn decode_huffman_encoded_value() {
        // Encode ":status 200" using indexed for :status (index 8)
        // 0x88 = indexed, index 8 = :status 200
        let data: &[u8] = &[0x88];
        let mut decoder = Decoder::new(4096);
        let mut collector = CollectHeaders(Vec::new());
        decoder.decode(&mut &data[..], &mut collector).unwrap();
        assert_eq!(collector.0[0], (":status".into(), "200".into()));
    }

    #[test]
    fn decode_huffman_string_literal() {
        // Build a header with Huffman-encoded value manually:
        // literal without indexing, name index 0 (new name)
        // name: "x" (plain), value: "abc" (Huffman-encoded)
        // Huffman "abc" = [0x1c, 0x64] (2 bytes)
        let data: &[u8] = &[
            0x00,       // literal, new name
            0x01, b'x', // name: plain, len 1
            0x82,       // value: Huffman flag (0x80) + len 2
            0x1c, 0x64, // Huffman "abc"
        ];
        let mut decoder = Decoder::new(4096);
        let mut collector = CollectHeaders(Vec::new());
        decoder.decode(&mut &data[..], &mut collector).unwrap();
        assert_eq!(collector.0[0], ("x".into(), "abc".into()));
    }

    #[test]
    fn decode_dynamic_table_size_update() {
        // 0x3f 0x01 = dynamic table size update to 32 (0x20 prefix, value 32)
        // Then indexed :method GET
        let data: &[u8] = &[0x20, 0x82];
        let mut decoder = Decoder::new(4096);
        let mut collector = CollectHeaders(Vec::new());
        decoder.decode(&mut &data[..], &mut collector).unwrap();
        assert_eq!(collector.0.len(), 1);
        assert_eq!(collector.0[0].1, "GET");
    }

    #[test]
    fn decode_index_zero_errors() {
        // 0x80 = indexed, index 0 → error
        let data: &[u8] = &[0x80];
        let mut decoder = Decoder::new(4096);
        let mut collector = CollectHeaders(Vec::new());
        assert!(decoder.decode(&mut &data[..], &mut collector).is_err());
    }

    #[test]
    fn encoder_decoder_roundtrip() {
        use crate::protocol::http::hpack::encode_request_headers;

        let input = &[
            (":method", "GET"),
            (":scheme", "https"),
            (":authority", "www.example.com"),
            (":path", "/resource"),
            ("accept", "text/html"),
            ("user-agent", "tagliacarte/0.1"),
        ];
        let mut buf = bytes::BytesMut::new();
        encode_request_headers(input, &mut buf).unwrap();

        let mut decoder = Decoder::new(4096);
        let mut collector = CollectHeaders(Vec::new());
        let mut cursor = &buf[..];
        decoder.decode(&mut cursor, &mut collector).unwrap();

        assert_eq!(collector.0.len(), input.len());
        for (i, &(name, value)) in input.iter().enumerate() {
            assert_eq!(collector.0[i].0, name);
            assert_eq!(collector.0[i].1, value);
        }
    }
}
