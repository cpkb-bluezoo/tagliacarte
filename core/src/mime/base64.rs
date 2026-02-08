/*
 * base64.rs
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

//! Base64 decoder for Content-Transfer-Encoding (RFC 2045).

use std::sync::OnceLock;

fn decode_table() -> &'static [i8; 256] {
    static TABLE: OnceLock<[i8; 256]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t = [-1i8; 256];
        t[32] = -2;  // space
        t[9] = -2;   // tab
        t[13] = -2;  // \r
        t[10] = -2;  // \n
        for i in 0..26u8 {
            t[(b'A' + i) as usize] = i as i8;
            t[(b'a' + i) as usize] = (26 + i) as i8;
        }
        for i in 0..10u8 {
            t[(b'0' + i) as usize] = (52 + i) as i8;
        }
        t[b'+' as usize] = 62;
        t[b'/' as usize] = 63;
        t
    })
}

#[allow(dead_code)]
const INVALID: i8 = -1;
const WHITESPACE: i8 = -2;

/// Decode base64 from `src` into `dst`. Consumes only complete 4-char quanta; leaves remainder.
/// If `end_of_stream` then flush remaining bits into dst.
/// Returns number of bytes consumed from src.
pub fn decode(
    src: &[u8],
    src_pos: &mut usize,
    dst: &mut [u8],
    dst_pos: &mut usize,
    max_decode: usize,
    end_of_stream: bool,
) -> usize {
    let start_src = *src_pos;
    let mut quantum: u32 = 0;
    let mut quantum_bits: u32 = 0;
    let mut last_valid_src = *src_pos;
    let mut saw_padding = false;
    let dst_limit = (*dst_pos + max_decode).min(dst.len());

    while *src_pos < src.len() {
        let b = src[*src_pos];
        *src_pos += 1;
        let val = decode_table()[b as usize];

        if val >= 0 {
            quantum = (quantum << 6) | (val as u32);
            quantum_bits += 6;
            if quantum_bits >= 24 {
                if *dst_pos + 3 <= dst_limit {
                    dst[*dst_pos] = (quantum >> 16) as u8;
                    dst[*dst_pos + 1] = (quantum >> 8) as u8;
                    dst[*dst_pos + 2] = quantum as u8;
                    *dst_pos += 3;
                    last_valid_src = *src_pos;
                    quantum = 0;
                    quantum_bits = 0;
                } else {
                    *src_pos = last_valid_src;
                    break;
                }
            }
        } else if val == WHITESPACE {
            continue;
        } else if b == b'=' {
            saw_padding = true;
            break;
        }
    }

    if (saw_padding || end_of_stream) && quantum_bits >= 8 && *dst_pos < dst_limit {
        dst[*dst_pos] = (quantum >> (quantum_bits - 8)) as u8;
        *dst_pos += 1;
        if quantum_bits >= 16 && *dst_pos < dst_limit {
            dst[*dst_pos] = (quantum >> (quantum_bits - 16)) as u8;
            *dst_pos += 1;
        }
        last_valid_src = *src_pos;
    }

    *src_pos = last_valid_src;
    last_valid_src - start_src
}
