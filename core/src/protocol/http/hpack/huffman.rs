/*
 * huffman.rs
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

//! Huffman codec for HPACK (RFC 7541 Appendix B).
//!
//! Uses a trie for decoding and the static code table for encoding.
//! Ported from the Gumdrop reference implementation.

use std::io;
use std::sync::OnceLock;

/// (code_bits, num_bits) for symbols 0..=256. Index 256 is the EOS symbol.
/// RFC 7541 Appendix B.
const HUFFMAN_TABLE: [(u32, u8); 257] = [
    (0x1ff8, 13),     // 0
    (0x7fffd8, 23),   // 1
    (0xfffffe2, 28),  // 2
    (0xfffffe3, 28),  // 3
    (0xfffffe4, 28),  // 4
    (0xfffffe5, 28),  // 5
    (0xfffffe6, 28),  // 6
    (0xfffffe7, 28),  // 7
    (0xfffffe8, 28),  // 8
    (0xffffea, 24),   // 9
    (0x3ffffffc, 30), // 10
    (0xfffffe9, 28),  // 11
    (0xfffffea, 28),  // 12
    (0x3ffffffd, 30), // 13
    (0xfffffeb, 28),  // 14
    (0xfffffec, 28),  // 15
    (0xfffffed, 28),  // 16
    (0xfffffee, 28),  // 17
    (0xfffffef, 28),  // 18
    (0xffffff0, 28),  // 19
    (0xffffff1, 28),  // 20
    (0xffffff2, 28),  // 21
    (0x3ffffffe, 30), // 22
    (0xffffff3, 28),  // 23
    (0xffffff4, 28),  // 24
    (0xffffff5, 28),  // 25
    (0xffffff6, 28),  // 26
    (0xffffff7, 28),  // 27
    (0xffffff8, 28),  // 28
    (0xffffff9, 28),  // 29
    (0xffffffa, 28),  // 30
    (0xffffffb, 28),  // 31
    (0x14, 6),        // 32 ' '
    (0x3f8, 10),      // 33 '!'
    (0x3f9, 10),      // 34 '"'
    (0xffa, 12),      // 35 '#'
    (0x1ff9, 13),     // 36 '$'
    (0x15, 6),        // 37 '%'
    (0xf8, 8),        // 38 '&'
    (0x7fa, 11),      // 39 '''
    (0x3fa, 10),      // 40 '('
    (0x3fb, 10),      // 41 ')'
    (0xf9, 8),        // 42 '*'
    (0x7fb, 11),      // 43 '+'
    (0xfa, 8),        // 44 ','
    (0x16, 6),        // 45 '-'
    (0x17, 6),        // 46 '.'
    (0x18, 6),        // 47 '/'
    (0x0, 5),         // 48 '0'
    (0x1, 5),         // 49 '1'
    (0x2, 5),         // 50 '2'
    (0x19, 6),        // 51 '3'
    (0x1a, 6),        // 52 '4'
    (0x1b, 6),        // 53 '5'
    (0x1c, 6),        // 54 '6'
    (0x1d, 6),        // 55 '7'
    (0x1e, 6),        // 56 '8'
    (0x1f, 6),        // 57 '9'
    (0x5c, 7),        // 58 ':'
    (0xfb, 8),        // 59 ';'
    (0x7ffc, 15),     // 60 '<'
    (0x20, 6),        // 61 '='
    (0xffb, 12),      // 62 '>'
    (0x3fc, 10),      // 63 '?'
    (0x1ffa, 13),     // 64 '@'
    (0x21, 6),        // 65 'A'
    (0x5d, 7),        // 66 'B'
    (0x5e, 7),        // 67 'C'
    (0x5f, 7),        // 68 'D'
    (0x60, 7),        // 69 'E'
    (0x61, 7),        // 70 'F'
    (0x62, 7),        // 71 'G'
    (0x63, 7),        // 72 'H'
    (0x64, 7),        // 73 'I'
    (0x65, 7),        // 74 'J'
    (0x66, 7),        // 75 'K'
    (0x67, 7),        // 76 'L'
    (0x68, 7),        // 77 'M'
    (0x69, 7),        // 78 'N'
    (0x6a, 7),        // 79 'O'
    (0x6b, 7),        // 80 'P'
    (0x6c, 7),        // 81 'Q'
    (0x6d, 7),        // 82 'R'
    (0x6e, 7),        // 83 'S'
    (0x6f, 7),        // 84 'T'
    (0x70, 7),        // 85 'U'
    (0x71, 7),        // 86 'V'
    (0x72, 7),        // 87 'W'
    (0xfc, 8),        // 88 'X'
    (0x73, 7),        // 89 'Y'
    (0xfd, 8),        // 90 'Z'
    (0x1ffb, 13),     // 91 '['
    (0x7fff0, 19),    // 92 '\'
    (0x1ffc, 13),     // 93 ']'
    (0x3ffc, 14),     // 94 '^'
    (0x22, 6),        // 95 '_'
    (0x7ffd, 15),     // 96 '`'
    (0x3, 5),         // 97 'a'
    (0x23, 6),        // 98 'b'
    (0x4, 5),         // 99 'c'
    (0x24, 6),        // 100 'd'
    (0x5, 5),         // 101 'e'
    (0x25, 6),        // 102 'f'
    (0x26, 6),        // 103 'g'
    (0x27, 6),        // 104 'h'
    (0x6, 5),         // 105 'i'
    (0x74, 7),        // 106 'j'
    (0x75, 7),        // 107 'k'
    (0x28, 6),        // 108 'l'
    (0x29, 6),        // 109 'm'
    (0x2a, 6),        // 110 'n'
    (0x7, 5),         // 111 'o'
    (0x2b, 6),        // 112 'p'
    (0x76, 7),        // 113 'q'
    (0x2c, 6),        // 114 'r'
    (0x8, 5),         // 115 's'
    (0x9, 5),         // 116 't'
    (0x2d, 6),        // 117 'u'
    (0x77, 7),        // 118 'v'
    (0x78, 7),        // 119 'w'
    (0x79, 7),        // 120 'x'
    (0x7a, 7),        // 121 'y'
    (0x7b, 7),        // 122 'z'
    (0x7ffe, 15),     // 123 '{'
    (0x7fc, 11),      // 124 '|'
    (0x3ffd, 14),     // 125 '}'
    (0x1ffd, 13),     // 126 '~'
    (0xffffffc, 28),  // 127
    (0xfffe6, 20),    // 128
    (0x3fffd2, 22),   // 129
    (0xfffe7, 20),    // 130
    (0xfffe8, 20),    // 131
    (0x3fffd3, 22),   // 132
    (0x3fffd4, 22),   // 133
    (0x3fffd5, 22),   // 134
    (0x7fffd9, 23),   // 135
    (0x3fffd6, 22),   // 136
    (0x7fffda, 23),   // 137
    (0x7fffdb, 23),   // 138
    (0x7fffdc, 23),   // 139
    (0x7fffdd, 23),   // 140
    (0x7fffde, 23),   // 141
    (0xffffeb, 24),   // 142
    (0x7fffdf, 23),   // 143
    (0xffffec, 24),   // 144
    (0xffffed, 24),   // 145
    (0x3fffd7, 22),   // 146
    (0x7fffe0, 23),   // 147
    (0xffffee, 24),   // 148
    (0x7fffe1, 23),   // 149
    (0x7fffe2, 23),   // 150
    (0x7fffe3, 23),   // 151
    (0x7fffe4, 23),   // 152
    (0x1fffdc, 21),   // 153
    (0x3fffd8, 22),   // 154
    (0x7fffe5, 23),   // 155
    (0x3fffd9, 22),   // 156
    (0x7fffe6, 23),   // 157
    (0x7fffe7, 23),   // 158
    (0xffffef, 24),   // 159
    (0x3fffda, 22),   // 160
    (0x1fffdd, 21),   // 161
    (0xfffe9, 20),    // 162
    (0x3fffdb, 22),   // 163
    (0x3fffdc, 22),   // 164
    (0x7fffe8, 23),   // 165
    (0x7fffe9, 23),   // 166
    (0x1fffde, 21),   // 167
    (0x7fffea, 23),   // 168
    (0x3fffdd, 22),   // 169
    (0x3fffde, 22),   // 170
    (0xfffff0, 24),   // 171
    (0x1fffdf, 21),   // 172
    (0x3fffdf, 22),   // 173
    (0x7fffeb, 23),   // 174
    (0x7fffec, 23),   // 175
    (0x1fffe0, 21),   // 176
    (0x1fffe1, 21),   // 177
    (0x3fffe0, 22),   // 178
    (0x1fffe2, 21),   // 179
    (0x7fffed, 23),   // 180
    (0x3fffe1, 22),   // 181
    (0x7fffee, 23),   // 182
    (0x7fffef, 23),   // 183
    (0xfffea, 20),    // 184
    (0x3fffe2, 22),   // 185
    (0x3fffe3, 22),   // 186
    (0x3fffe4, 22),   // 187
    (0x7ffff0, 23),   // 188
    (0x3fffe5, 22),   // 189
    (0x3fffe6, 22),   // 190
    (0x7ffff1, 23),   // 191
    (0x3ffffe0, 26),  // 192
    (0x3ffffe1, 26),  // 193
    (0xfffeb, 20),    // 194
    (0x7fff1, 19),    // 195
    (0x3fffe7, 22),   // 196
    (0x7ffff2, 23),   // 197
    (0x3fffe8, 22),   // 198
    (0x1ffffec, 25),  // 199
    (0x3ffffe2, 26),  // 200
    (0x3ffffe3, 26),  // 201
    (0x3ffffe4, 26),  // 202
    (0x7ffffde, 27),  // 203
    (0x7ffffdf, 27),  // 204
    (0x3ffffe5, 26),  // 205
    (0xfffff1, 24),   // 206
    (0x1ffffed, 25),  // 207
    (0x7fff2, 19),    // 208
    (0x1fffe3, 21),   // 209
    (0x3ffffe6, 26),  // 210
    (0x7ffffe0, 27),  // 211
    (0x7ffffe1, 27),  // 212
    (0x3ffffe7, 26),  // 213
    (0x7ffffe2, 27),  // 214
    (0xfffff2, 24),   // 215
    (0x1fffe4, 21),   // 216
    (0x1fffe5, 21),   // 217
    (0x3ffffe8, 26),  // 218
    (0x3ffffe9, 26),  // 219
    (0xffffffd, 28),  // 220
    (0x7ffffe3, 27),  // 221
    (0x7ffffe4, 27),  // 222
    (0x7ffffe5, 27),  // 223
    (0xfffec, 20),    // 224
    (0xfffff3, 24),   // 225
    (0xfffed, 20),    // 226
    (0x1fffe6, 21),   // 227
    (0x3fffe9, 22),   // 228
    (0x1fffe7, 21),   // 229
    (0x1fffe8, 21),   // 230
    (0x7ffff3, 23),   // 231
    (0x3fffea, 22),   // 232
    (0x3fffeb, 22),   // 233
    (0x1ffffee, 25),  // 234
    (0x1ffffef, 25),  // 235
    (0xfffff4, 24),   // 236
    (0xfffff5, 24),   // 237
    (0x3ffffea, 26),  // 238
    (0x7ffff4, 23),   // 239
    (0x3ffffeb, 26),  // 240
    (0x7ffffe6, 27),  // 241
    (0x3ffffec, 26),  // 242
    (0x3ffffed, 26),  // 243
    (0x7ffffe7, 27),  // 244
    (0x7ffffe8, 27),  // 245
    (0x7ffffe9, 27),  // 246
    (0x7ffffea, 27),  // 247
    (0x7ffffeb, 27),  // 248
    (0xffffffe, 28),  // 249
    (0x7ffffec, 27),  // 250
    (0x7ffffed, 27),  // 251
    (0x7ffffee, 27),  // 252
    (0x7ffffef, 27),  // 253
    (0x7fffff0, 27),  // 254
    (0x3ffffee, 26),  // 255
    (0x3fffffff, 30), // 256 EOS
];

const EOS_SYMBOL: u16 = 256;

struct HuffmanNode {
    symbol: i16,
    children: [Option<Box<HuffmanNode>>; 2],
}

impl HuffmanNode {
    fn new() -> Self {
        Self {
            symbol: -1,
            children: [None, None],
        }
    }
}

fn build_trie() -> Box<HuffmanNode> {
    let mut root = Box::new(HuffmanNode::new());
    for (symbol, &(code, num_bits)) in HUFFMAN_TABLE.iter().enumerate() {
        let mut node = &mut *root;
        for i in 0..num_bits {
            let bit = ((code >> (num_bits - 1 - i)) & 1) as usize;
            if node.children[bit].is_none() {
                node.children[bit] = Some(Box::new(HuffmanNode::new()));
            }
            node = node.children[bit].as_deref_mut().unwrap();
        }
        node.symbol = symbol as i16;
    }
    root
}

static HUFFMAN_ROOT: OnceLock<Box<HuffmanNode>> = OnceLock::new();

fn root() -> &'static HuffmanNode {
    HUFFMAN_ROOT.get_or_init(build_trie)
}

/// Decode HPACK Huffman-encoded bytes into plaintext.
pub fn decode(encoded: &[u8]) -> io::Result<Vec<u8>> {
    let root_node = root();
    let mut out = Vec::new();
    let mut node = root_node;
    let mut last_decoded_bit = 0usize;

    for (byte_idx, &byte) in encoded.iter().enumerate() {
        for bit_idx in (0..8).rev() {
            let bit = ((byte >> bit_idx) & 1) as usize;
            node = match &node.children[bit] {
                Some(child) => child,
                None => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "HPACK Huffman: invalid bit sequence",
                    ));
                }
            };

            if node.symbol >= 0 {
                let sym = node.symbol as u16;
                if sym == EOS_SYMBOL {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "HPACK Huffman: EOS symbol in string literal",
                    ));
                }
                out.push(sym as u8);
                node = root_node;
                last_decoded_bit = byte_idx * 8 + (7 - bit_idx) + 1;
            }
        }
    }

    let total_bits = encoded.len() * 8;
    let padding_bits = total_bits - last_decoded_bit;
    if padding_bits > 7 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "HPACK Huffman: padding longer than 7 bits",
        ));
    }
    if padding_bits > 0 {
        let last_byte = encoded[encoded.len() - 1];
        let mask = (1u8 << padding_bits) - 1;
        if (last_byte & mask) != mask {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "HPACK Huffman: padding not all 1-bits",
            ));
        }
    }

    Ok(out)
}

/// Encode plaintext bytes using HPACK Huffman coding.
pub fn encode(plaintext: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut current_byte: u32 = 0;
    let mut bits_in_byte: u8 = 0;

    for &b in plaintext {
        let (code, num_bits) = HUFFMAN_TABLE[b as usize];
        for i in (0..num_bits).rev() {
            let bit = (code >> i) & 1;
            current_byte = (current_byte << 1) | bit;
            bits_in_byte += 1;
            if bits_in_byte == 8 {
                out.push(current_byte as u8);
                current_byte = 0;
                bits_in_byte = 0;
            }
        }
    }

    if bits_in_byte > 0 {
        current_byte = (current_byte << (8 - bits_in_byte)) | ((1u32 << (8 - bits_in_byte)) - 1);
        out.push(current_byte as u8);
    }

    out
}

/// Compute the Huffman-encoded length in bytes for the given plaintext.
pub fn encoded_length(plaintext: &[u8]) -> usize {
    let total_bits: usize = plaintext
        .iter()
        .map(|&b| HUFFMAN_TABLE[b as usize].1 as usize)
        .sum();
    (total_bits + 7) / 8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_abc() {
        let plain = b"abc";
        let encoded = encode(plain);
        assert_eq!(&encoded, &[0x1c, 0x64]);
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded, plain);
    }

    #[test]
    fn roundtrip_hello_world() {
        let plain = b"Hello, world!";
        let encoded = encode(plain);
        let expected: &[u8] = &[0xc6, 0x5a, 0x28, 0x3f, 0xd2, 0x9e, 0x0f, 0x65, 0x12, 0x7f, 0x1f];
        assert_eq!(&encoded, expected);
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded, plain);
    }

    #[test]
    fn decode_empty() {
        let decoded = decode(&[]).unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn encode_empty() {
        let encoded = encode(&[]);
        assert!(encoded.is_empty());
    }

    #[test]
    fn encoded_length_matches() {
        let plain = b"Hello, world!";
        let encoded = encode(plain);
        assert_eq!(encoded_length(plain), encoded.len());
    }

    #[test]
    fn roundtrip_url() {
        let plain = b"https://oauth2.googleapis.com/token";
        let encoded = encode(plain);
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded, plain);
    }

    #[test]
    fn roundtrip_all_printable_ascii() {
        let plain: Vec<u8> = (32u8..=126).collect();
        let encoded = encode(&plain);
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded, plain);
    }
}
