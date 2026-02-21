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

//! Dot stuffing for SMTP DATA (RFC 5321 §4.5.2: lines starting with `.` get an extra `.`).

#[derive(Clone, Copy, PartialEq)]
enum State {
    /// At the beginning of a line (initial state, or immediately after CRLF).
    LineStart,
    /// Mid-line.
    Normal,
    /// Saw CR, waiting for LF.
    SawCr,
}

/// Performs dot stuffing: any line that starts with `.` gets an extra `.` prepended.
/// The beginning of the message is treated as the start of a line.
pub struct DotStuffer {
    state: State,
    emitted: bool,
}

impl Default for DotStuffer {
    fn default() -> Self {
        Self {
            state: State::LineStart,
            emitted: false,
        }
    }
}

impl DotStuffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a chunk; call `out` for each slice to send.
    ///
    /// Bytes flow through transparently. The only modification is inserting an
    /// extra `.` before any `.` that appears at the start of a line.
    pub fn process_chunk<F>(&mut self, chunk: &[u8], mut out: F)
    where
        F: FnMut(&[u8]),
    {
        let mut start = 0;
        for i in 0..chunk.len() {
            let b = chunk[i];
            match self.state {
                State::LineStart => {
                    if b == b'.' {
                        out(&chunk[start..i]);
                        out(b".");
                        start = i;
                    }
                    self.state = if b == b'\r' { State::SawCr } else { State::Normal };
                }
                State::Normal => {
                    if b == b'\r' {
                        self.state = State::SawCr;
                    }
                }
                State::SawCr => {
                    self.state = if b == b'\n' {
                        State::LineStart
                    } else if b == b'\r' {
                        State::SawCr
                    } else {
                        State::Normal
                    };
                }
            }
        }
        if start < chunk.len() {
            self.emitted = true;
            out(&chunk[start..]);
        }
    }

    /// Emit CRLF.CRLF terminator; reset state.
    ///
    /// Ensures the message ends with CRLF before the `.CRLF` terminator.
    pub fn end_message<F>(&mut self, mut out: F)
    where
        F: FnMut(&[u8]),
    {
        match self.state {
            State::LineStart if self.emitted => {
                out(b".\r\n");
            }
            State::SawCr => {
                out(b"\n.\r\n");
            }
            _ => {
                out(b"\r\n.\r\n");
            }
        }
        self.state = State::LineStart;
        self.emitted = false;
    }

    pub fn reset(&mut self) {
        self.state = State::LineStart;
        self.emitted = false;
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

    #[test]
    fn no_stuffing_needed() {
        let mut s = DotStuffer::new();
        let out = collect(&mut s, b"Hello\r\nWorld\r\n");
        assert_eq!(out, b"Hello\r\nWorld\r\n");
    }

    #[test]
    fn dot_only_at_line_start() {
        let mut s = DotStuffer::new();
        let out = collect(&mut s, b"a.b\r\nc.d\r\n");
        assert_eq!(out, b"a.b\r\nc.d\r\n");
    }

    #[test]
    fn multiple_dots_at_line_start() {
        let mut s = DotStuffer::new();
        let out = collect(&mut s, b"...\r\n");
        assert_eq!(out, b"....\r\n");
    }

    #[test]
    fn end_message_after_crlf() {
        let mut s = DotStuffer::new();
        let mut out = Vec::new();
        s.process_chunk(b"Hi\r\n", |x| out.extend_from_slice(x));
        s.end_message(|x| out.extend_from_slice(x));
        assert_eq!(out, b"Hi\r\n.\r\n");
    }

    #[test]
    fn end_message_after_cr() {
        let mut s = DotStuffer::new();
        let mut out = Vec::new();
        s.process_chunk(b"Hi\r", |x| out.extend_from_slice(x));
        s.end_message(|x| out.extend_from_slice(x));
        assert_eq!(out, b"Hi\r\n.\r\n");
    }

    #[test]
    fn chunked_input() {
        let mut s = DotStuffer::new();
        let mut out = Vec::new();
        s.process_chunk(b"Hi\r", |x| out.extend_from_slice(x));
        s.process_chunk(b"\n.bye\r\n", |x| out.extend_from_slice(x));
        assert_eq!(out, b"Hi\r\n..bye\r\n");
    }
}
