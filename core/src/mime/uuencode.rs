/*
 * uuencode.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * Classic uuencode (historical Unix encoding), still used in plain-text Usenet posts.
 */

const UU_TABLE: &[u8; 64] = b"`!\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_";

/// One uuencode file section: `begin` line, encoded lines, `end`.
/// [filename_for_begin] should be a single token (no newlines); invalid path chars are replaced.
pub fn uuencode_file_section(filename_for_begin: &str, data: &[u8]) -> String {
    let name = sanitize_uu_filename(filename_for_begin);
    let mut out = String::new();
    out.push_str("begin 644 ");
    out.push_str(&name);
    out.push_str("\r\n");
    out.push_str(&uuencode_data_lines(data));
    out.push_str("end\r\n");
    out
}

fn sanitize_uu_filename(name: &str) -> String {
    let base = name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(name)
        .trim();
    if base.is_empty() {
        return "attachment".to_owned();
    }
    base.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn uuencode_data_lines(data: &[u8]) -> String {
    let mut out = String::new();
    for chunk in data.chunks(45) {
        let n = chunk.len();
        let len_ch = char::from_u32((n + 32) as u32).unwrap_or('M');
        out.push(len_ch);
        let mut i = 0;
        while i < n {
            let b0 = chunk[i];
            let b1 = chunk.get(i + 1).copied().unwrap_or(0);
            let b2 = chunk.get(i + 2).copied().unwrap_or(0);
            let (c0, c1, c2, c3) = encode_triplet_bytes(b0, b1, b2);
            out.push(UU_TABLE[c0] as char);
            out.push(UU_TABLE[c1] as char);
            out.push(UU_TABLE[c2] as char);
            out.push(UU_TABLE[c3] as char);
            i += 3;
        }
        out.push_str("\r\n");
    }
    out
}

fn encode_triplet_bytes(b0: u8, b1: u8, b2: u8) -> (usize, usize, usize, usize) {
    let c0 = (b0 >> 2) as usize;
    let c1 = (((b0 & 0x03) << 4) | ((b1 & 0xf0) >> 4)) as usize;
    let c2 = (((b1 & 0x0f) << 2) | ((b2 & 0xc0) >> 6)) as usize;
    let c3 = (b2 & 0x3f) as usize;
    (c0, c1, c2, c3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuencode_empty() {
        let s = uuencode_file_section("a.bin", &[]);
        assert!(s.starts_with("begin 644 a.bin\r\n"));
        assert!(s.ends_with("end\r\n"));
    }

    #[test]
    fn uuencode_three_bytes() {
        let s = uuencode_file_section("x", &[0, 1, 2]);
        assert!(s.contains("\r\n"));
        // Line length char for 3 payload bytes is 3 + 32 = '#' (0x23).
        assert!(s.contains("#"));
    }
}
