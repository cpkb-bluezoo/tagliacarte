/*
 * writer.rs
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

//! HTTP/2 frame writer: serializes frames into a buffer.

use bytes::{BufMut, Bytes, BytesMut};
use std::io;

use super::frame::*;

/// Writes HTTP/2 frames into a BytesMut. Caller is responsible for sending the buffer to the stream.
pub struct H2Writer {
    buf: BytesMut,
}

impl H2Writer {
    pub fn new() -> Self {
        Self {
            buf: BytesMut::with_capacity(16384 + FRAME_HEADER_LENGTH),
        }
    }

    fn write_frame_header(&mut self, length: usize, frame_type: u8, flags: u8, stream_id: u32) {
        self.buf.put_u8((length >> 16) as u8);
        self.buf.put_u8((length >> 8) as u8);
        self.buf.put_u8(length as u8);
        self.buf.put_u8(frame_type);
        self.buf.put_u8(flags);
        self.buf.put_u32(stream_id);
    }

    /// Append a DATA frame. Returns the number of payload bytes written.
    pub fn write_data(&mut self, stream_id: u32, data: &[u8], end_stream: bool) -> io::Result<usize> {
        if stream_id == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "DATA frame stream_id must be non-zero",
            ));
        }
        let flags = if end_stream {
            FLAG_END_STREAM
        } else {
            0
        };
        let len = data.len();
        self.write_frame_header(len, TYPE_DATA, flags, stream_id);
        self.buf.extend_from_slice(data);
        Ok(len)
    }

    /// Append a HEADERS frame (no priority, no padding). Header block must be HPACK-encoded.
    pub fn write_headers(
        &mut self,
        stream_id: u32,
        header_block: &[u8],
        end_stream: bool,
        end_headers: bool,
    ) -> io::Result<()> {
        if stream_id == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "HEADERS frame stream_id must be non-zero",
            ));
        }
        let mut flags = 0u8;
        if end_stream {
            flags |= FLAG_END_STREAM;
        }
        if end_headers {
            flags |= FLAG_END_HEADERS;
        }
        self.write_frame_header(header_block.len(), TYPE_HEADERS, flags, stream_id);
        self.buf.extend_from_slice(header_block);
        Ok(())
    }

    /// Append a HEADERS frame with priority (dependency, weight, exclusive).
    pub fn write_headers_with_priority(
        &mut self,
        stream_id: u32,
        header_block: &[u8],
        end_stream: bool,
        end_headers: bool,
        stream_dependency: u32,
        weight: u8,
        exclusive: bool,
    ) -> io::Result<()> {
        if stream_id == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "HEADERS frame stream_id must be non-zero",
            ));
        }
        let mut flags = FLAG_PRIORITY;
        if end_stream {
            flags |= FLAG_END_STREAM;
        }
        if end_headers {
            flags |= FLAG_END_HEADERS;
        }
        let payload_len = 5 + header_block.len();
        self.write_frame_header(payload_len, TYPE_HEADERS, flags, stream_id);
        let dep = if exclusive {
            stream_dependency | 0x8000_0000
        } else {
            stream_dependency
        };
        self.buf.put_u32(dep);
        self.buf.put_u8(weight.saturating_sub(1));
        self.buf.extend_from_slice(header_block);
        Ok(())
    }

    pub fn write_rst_stream(&mut self, stream_id: u32, error_code: u32) -> io::Result<()> {
        if stream_id == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "RST_STREAM stream_id must be non-zero",
            ));
        }
        self.write_frame_header(4, TYPE_RST_STREAM, 0, stream_id);
        self.buf.put_u32(error_code);
        Ok(())
    }

    /// Empty SETTINGS frame (for connection preface). Optional: SETTINGS with params.
    pub fn write_settings(&mut self, settings: &[(u16, u32)]) -> io::Result<()> {
        let payload_len = settings.len() * 6;
        self.write_frame_header(payload_len, TYPE_SETTINGS, 0, 0);
        for (id, value) in settings {
            self.buf.put_u16(*id);
            self.buf.put_u32(*value);
        }
        Ok(())
    }

    pub fn write_settings_ack(&mut self) -> io::Result<()> {
        self.write_frame_header(0, TYPE_SETTINGS, FLAG_ACK, 0);
        Ok(())
    }

    pub fn write_ping(&mut self, opaque_data: u64, ack: bool) -> io::Result<()> {
        let flags = if ack {
            FLAG_ACK
        } else {
            0
        };
        self.write_frame_header(8, TYPE_PING, flags, 0);
        self.buf.put_u64(opaque_data);
        Ok(())
    }

    pub fn write_window_update(&mut self, stream_id: u32, increment: u32) -> io::Result<()> {
        self.write_frame_header(4, TYPE_WINDOW_UPDATE, 0, stream_id);
        self.buf.put_u32(increment & 0x7FFF_FFFF);
        Ok(())
    }

    pub fn write_goaway(&mut self, last_stream_id: u32, error_code: u32, debug_data: &[u8]) -> io::Result<()> {
        self.write_frame_header(8 + debug_data.len(), TYPE_GOAWAY, 0, 0);
        self.buf.put_u32(last_stream_id & 0x7fff_ffff); // reserved bit
        self.buf.put_u32(error_code);
        self.buf.extend_from_slice(debug_data);
        Ok(())
    }

    /// Take the accumulated buffer. Writer remains usable (buffer is replaced with new empty).
    pub fn take_buffer(&mut self) -> Bytes {
        self.buf.split().freeze()
    }

    /// Current length of buffered data.
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }
}

impl Default for H2Writer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_empty_settings() {
        let mut w = H2Writer::new();
        w.write_settings(&[]).unwrap();
        let buf = w.take_buffer();
        assert_eq!(buf.len(), FRAME_HEADER_LENGTH);
        assert_eq!(buf[3], TYPE_SETTINGS);
        assert_eq!(buf[4], 0); // no ACK
        // stream id 0
        assert_eq!(&buf[5..9], &[0, 0, 0, 0]);
        // length 0
        assert_eq!(&buf[0..3], &[0, 0, 0]);
    }

    #[test]
    fn write_settings_with_params() {
        let mut w = H2Writer::new();
        w.write_settings(&[
            (SETTINGS_MAX_FRAME_SIZE, 32768),
            (SETTINGS_HEADER_TABLE_SIZE, 8192),
        ])
        .unwrap();
        let buf = w.take_buffer();
        // 9-byte header + 2 * 6 bytes payload
        assert_eq!(buf.len(), FRAME_HEADER_LENGTH + 12);
        let payload_len = (buf[0] as usize) << 16 | (buf[1] as usize) << 8 | buf[2] as usize;
        assert_eq!(payload_len, 12);
    }

    #[test]
    fn write_settings_ack() {
        let mut w = H2Writer::new();
        w.write_settings_ack().unwrap();
        let buf = w.take_buffer();
        assert_eq!(buf.len(), FRAME_HEADER_LENGTH);
        assert_eq!(buf[3], TYPE_SETTINGS);
        assert_eq!(buf[4], FLAG_ACK);
        assert_eq!(&buf[0..3], &[0, 0, 0]); // length 0
    }

    #[test]
    fn write_headers_end_stream() {
        let mut w = H2Writer::new();
        let block = b"fake-hpack";
        w.write_headers(1, block, true, true).unwrap();
        let buf = w.take_buffer();
        let payload_len = (buf[0] as usize) << 16 | (buf[1] as usize) << 8 | buf[2] as usize;
        assert_eq!(payload_len, block.len());
        assert_eq!(buf[3], TYPE_HEADERS);
        assert_eq!(buf[4], FLAG_END_STREAM | FLAG_END_HEADERS);
        let stream_id =
            ((buf[5] & 0x7f) as u32) << 24 | (buf[6] as u32) << 16 | (buf[7] as u32) << 8 | buf[8] as u32;
        assert_eq!(stream_id, 1);
        assert_eq!(&buf[FRAME_HEADER_LENGTH..], block);
    }

    #[test]
    fn write_headers_with_body() {
        let mut w = H2Writer::new();
        w.write_headers(3, b"hdr", false, true).unwrap();
        let buf = w.take_buffer();
        assert_eq!(buf[4], FLAG_END_HEADERS); // no END_STREAM
    }

    #[test]
    fn write_data() {
        let mut w = H2Writer::new();
        let payload = b"Hello, world!";
        let written = w.write_data(1, payload, false).unwrap();
        assert_eq!(written, payload.len());
        let buf = w.take_buffer();
        let payload_len = (buf[0] as usize) << 16 | (buf[1] as usize) << 8 | buf[2] as usize;
        assert_eq!(payload_len, payload.len());
        assert_eq!(buf[3], TYPE_DATA);
        assert_eq!(buf[4], 0); // no END_STREAM
        assert_eq!(&buf[FRAME_HEADER_LENGTH..], payload);
    }

    #[test]
    fn write_data_end_stream() {
        let mut w = H2Writer::new();
        w.write_data(1, b"done", true).unwrap();
        let buf = w.take_buffer();
        assert_eq!(buf[4], FLAG_END_STREAM);
    }

    #[test]
    fn write_data_zero_stream_id_errors() {
        let mut w = H2Writer::new();
        assert!(w.write_data(0, b"x", false).is_err());
    }

    #[test]
    fn write_ping() {
        let mut w = H2Writer::new();
        w.write_ping(0xDEADBEEF_CAFEBABE, false).unwrap();
        let buf = w.take_buffer();
        assert_eq!(buf.len(), FRAME_HEADER_LENGTH + 8);
        assert_eq!(buf[3], TYPE_PING);
        assert_eq!(buf[4], 0);
    }

    #[test]
    fn write_ping_ack() {
        let mut w = H2Writer::new();
        w.write_ping(42, true).unwrap();
        let buf = w.take_buffer();
        assert_eq!(buf[4], FLAG_ACK);
    }

    #[test]
    fn write_goaway() {
        let mut w = H2Writer::new();
        w.write_goaway(5, 0, b"bye").unwrap();
        let buf = w.take_buffer();
        assert_eq!(buf[3], TYPE_GOAWAY);
        let payload_len = (buf[0] as usize) << 16 | (buf[1] as usize) << 8 | buf[2] as usize;
        assert_eq!(payload_len, 8 + 3);
    }

    #[test]
    fn write_rst_stream() {
        let mut w = H2Writer::new();
        w.write_rst_stream(1, 0x8).unwrap();
        let buf = w.take_buffer();
        assert_eq!(buf[3], TYPE_RST_STREAM);
        let payload_len = (buf[0] as usize) << 16 | (buf[1] as usize) << 8 | buf[2] as usize;
        assert_eq!(payload_len, 4);
    }

    #[test]
    fn take_buffer_resets() {
        let mut w = H2Writer::new();
        w.write_settings_ack().unwrap();
        assert!(!w.is_empty());
        let _ = w.take_buffer();
        assert!(w.is_empty());
        assert_eq!(w.len(), 0);
        // Writer is still usable after take
        w.write_settings_ack().unwrap();
        assert!(!w.is_empty());
    }
}
