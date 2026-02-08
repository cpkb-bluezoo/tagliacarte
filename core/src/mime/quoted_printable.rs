/*
 * quoted_printable.rs
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

//! Quoted-Printable decoder for Content-Transfer-Encoding (RFC 2045).

const HEX_DECODE: [i8; 256] = {
    let mut t = [-1i8; 256];
    let mut i = 0u8;
    while i < 10 {
        t[(b'0' + i) as usize] = i as i8;
        i = i.wrapping_add(1);
    }
    let mut i = 0u8;
    while i < 6 {
        t[(b'A' + i) as usize] = (10 + i) as i8;
        t[(b'a' + i) as usize] = (10 + i) as i8;
        i = i.wrapping_add(1);
    }
    t
};

/// Decode quoted-printable from `src` into `dst`. Handles =XX and soft line breaks (=CRLF, =LF).
/// Incomplete = at end left unconsumed unless end_of_stream.
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
    let dst_limit = (*dst_pos + max_decode).min(dst.len());

    while *src_pos < src.len() && *dst_pos < dst_limit {
        let b = src[*src_pos];
        if b != b'=' {
            dst[*dst_pos] = b;
            *dst_pos += 1;
            *src_pos += 1;
            continue;
        }
        let remaining = src.len() - *src_pos;
        if remaining >= 3 {
            let hex1 = src[*src_pos + 1];
            let hex2 = src[*src_pos + 2];
            let v1 = HEX_DECODE[hex1 as usize];
            let v2 = HEX_DECODE[hex2 as usize];
            if v1 >= 0 && v2 >= 0 {
                dst[*dst_pos] = ((v1 << 4) | v2) as u8;
                *dst_pos += 1;
                *src_pos += 3;
                continue;
            }
            if hex1 == b'\r' && hex2 == b'\n' {
                *src_pos += 3;
                continue;
            }
            if hex1 == b'\n' {
                *src_pos += 2;
                continue;
            }
            dst[*dst_pos] = b;
            *dst_pos += 1;
            *src_pos += 1;
        } else if remaining == 2 {
            let next = src[*src_pos + 1];
            if next == b'\n' {
                *src_pos += 2;
                continue;
            }
            if next == b'\r' && !end_of_stream {
                break;
            }
            if end_of_stream {
                dst[*dst_pos] = b;
                *dst_pos += 1;
                *src_pos += 1;
            } else {
                break;
            }
        } else {
            if end_of_stream {
                dst[*dst_pos] = b;
                *dst_pos += 1;
                *src_pos += 1;
            } else {
                break;
            }
        }
    }
    *src_pos - start_src
}
