/*
 * dot_stuffer.rs
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

//! Dot stuffing for SMTP DATA (RFC 5321: lines starting with . get an extra .).

#[derive(Clone, Copy)]
enum State {
    Normal,
    SawCr,
    SawCrLf,
}

/// Performs dot stuffing: after CRLF, a leading . is doubled (gumdrop DotStuffer).
pub struct DotStuffer {
    state: State,
}

impl Default for DotStuffer {
    fn default() -> Self {
        Self { state: State::Normal }
    }
}

impl DotStuffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a chunk; call `out` for each slice to send.
    pub fn process_chunk<F>(&mut self, chunk: &[u8], mut out: F)
    where
        F: FnMut(&[u8]),
    {
        let mut start = 0;
        let mut i = 0;
        while i < chunk.len() {
            let b = chunk[i];
            match self.state {
                State::Normal => {
                    if b == b'\r' {
                        if start < i {
                            out(&chunk[start..i]);
                        }
                        self.state = State::SawCr;
                        start = i + 1;
                        i += 1;
                    } else {
                        i += 1;
                    }
                }
                State::SawCr => {
                    if b == b'\n' {
                        self.state = State::SawCrLf;
                        start = i + 1;
                        i += 1;
                    } else {
                        out(b"\r");
                        out(&chunk[i..i + 1]);
                        start = i + 1;
                        self.state = if b == b'\r' {
                            State::SawCr
                        } else {
                            State::Normal
                        };
                        i += 1;
                    }
                }
                State::SawCrLf => {
                    if b == b'.' {
                        out(&chunk[start..i]);
                        out(b".");
                        start = i + 1;
                        self.state = State::Normal;
                        i += 1;
                    } else {
                        out(b"\r\n");
                        out(&chunk[i..i + 1]);
                        start = i + 1;
                        self.state = if b == b'\r' {
                            State::SawCr
                        } else {
                            State::Normal
                        };
                        i += 1;
                    }
                }
            }
        }
        if start < chunk.len() {
            out(&chunk[start..]);
        }
    }

    /// Emit pending bytes and CRLF.CRLF terminator; reset state.
    pub fn end_message<F>(&mut self, mut out: F)
    where
        F: FnMut(&[u8]),
    {
        match self.state {
            State::SawCr => out(b"\r"),
            State::SawCrLf => out(b"\r\n"),
            State::Normal => {}
        }
        out(b"\r\n.\r\n");
        self.state = State::Normal;
    }

    pub fn reset(&mut self) {
        self.state = State::Normal;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(stuffer: &mut DotStuffer, input: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        stuffer.process_chunk(input, |s| out.extend_from_slice(s));
        out
    }

    #[test]
    fn dot_after_crlf_is_doubled() {
        let mut s = DotStuffer::new();
        let out = collect(&mut s, b".\r\n");
        assert_eq!(out, b"..\r\n");
    }

    #[test]
    fn end_message_emits_terminator() {
        let mut s = DotStuffer::new();
        let mut out = Vec::new();
        s.end_message(|x| out.extend_from_slice(x));
        assert_eq!(out, b"\r\n.\r\n");
    }

    #[test]
    fn line_with_dot_stuffed() {
        let mut s = DotStuffer::new();
        let mut out = Vec::new();
        s.process_chunk(b"Hi\r\n.\r\nBye", |x| out.extend_from_slice(x));
        s.end_message(|x| out.extend_from_slice(x));
        assert_eq!(out, b"Hi\r\n..\r\nBye\r\n.\r\n");
    }
}
