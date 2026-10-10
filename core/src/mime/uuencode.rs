/*
 * uuencode.rs
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

//! The uuencode section an NNTP post carries an attachment in.

use rmimeparser::encode_uuencode;

/// One uuencode file section: `begin` line, encoded lines, `end`.
/// `filename_for_begin` is reduced to a safe single token.
pub fn uuencode_file_section(filename_for_begin: &str, data: &[u8]) -> String {
    let name = sanitize_uu_filename(filename_for_begin);
    let mut out = Vec::with_capacity(data.len() / 3 * 4 + 64);
    encode_uuencode(&mut out, &name, 0o644, data).expect("uuencode into memory");
    String::from_utf8(out).expect("uuencode output is ASCII")
}

fn sanitize_uu_filename(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name).trim();
    if base.is_empty() {
        return "attachment".to_owned();
    }
    base.chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+') { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_round_trips_and_sanitizes() {
        let s = uuencode_file_section("dir/my file.txt", b"Cat");
        assert_eq!(s, "begin 644 my_file.txt\r\n#0V%T\r\n`\r\nend\r\n");
        assert!(uuencode_file_section("", b"").starts_with("begin 644 attachment\r\n"));
    }
}
