/*
 * thread_headers.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 *
 * Parse Subject, Message-ID, References, In-Reply-To for email threading.
 */

use crate::mime::content_id::ContentID;
use crate::mime::handler::{MimeHandler, MimeParseError};
use crate::mime::rfc5322::handler::MessageHandler;
use crate::mime::rfc5322::MessageParser;

/// Subject, Message-ID, and References/In-Reply-To for thread grouping.
#[derive(Debug, Default)]
pub struct ThreadHeaders {
    pub subject: Option<String>,
    pub message_id: Option<String>,
    /// References and In-Reply-To msg-ids in order (References first, then In-Reply-To).
    pub references: Vec<String>,
}

fn content_id_to_string(c: &ContentID) -> String {
    format!("<{}@{}>", c.get_local_part(), c.get_domain())
}

struct ThreadHeadersCollector {
    out: ThreadHeaders,
}

impl Default for ThreadHeadersCollector {
    fn default() -> Self {
        Self {
            out: ThreadHeaders::default(),
        }
    }
}

impl MessageHandler for ThreadHeadersCollector {
    fn header(&mut self, name: &str, value: &str) -> Result<(), MimeParseError> {
        if name.eq_ignore_ascii_case("subject") {
            self.out.subject = Some(value.trim().to_string());
        }
        Ok(())
    }

    fn message_id_header(&mut self, name: &str, ids: &[ContentID]) -> Result<(), MimeParseError> {
        if ids.is_empty() {
            return Ok(());
        }
        let s = content_id_to_string(&ids[0]);
        if name.eq_ignore_ascii_case("message-id") {
            self.out.message_id = Some(s);
        } else if name.eq_ignore_ascii_case("references") || name.eq_ignore_ascii_case("in-reply-to") {
            for id in ids {
                self.out.references.push(content_id_to_string(id));
            }
        }
        Ok(())
    }
}

impl MimeHandler for ThreadHeadersCollector {}

/// Parse Subject, Message-ID, References, In-Reply-To from raw message bytes (headers only).
pub fn parse_thread_headers(raw: &[u8]) -> Result<ThreadHeaders, MimeParseError> {
    let collector = ThreadHeadersCollector::default();
    let mut parser = MessageParser::new(collector);
    parser.receive(raw)?;
    parser.close()?;
    Ok(parser.into_inner().out)
}
