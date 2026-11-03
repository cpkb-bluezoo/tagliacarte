/*
 * mail_html.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * Small pieces the message views share: escaping text for HTML, pointing `cid:`
 * references at a message's own parts as the text streams past, and reading a
 * whole message through a store's callbacks.
 */

use std::sync::{Arc, Mutex as StdMutex};

use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};

use tagliacarte_core::message_id::MessageId;
use tagliacarte_core::store::Folder;

use crate::mail_store::DynStore;

pub(crate) fn escape_html_text(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            _ => o.push(c),
        }
    }
    o
}

/// Incremental `cid:` → local URL rewrite. Defers output only when a `cid:` URL may span chunks.
pub(crate) struct StreamingCidRewriter {
    local_view_prefix: String,
    pending: String,
}

impl StreamingCidRewriter {
    pub(crate) fn new(local_view_prefix: String) -> Self {
        Self {
            local_view_prefix,
            pending: String::new(),
        }
    }

    pub(crate) fn feed(&mut self, text: &str) -> String {
        self.pending.push_str(text);
        let mut out = String::new();

        loop {
            let buf = self.pending.as_str();
            let Some(cid_start) = buf.find("cid:") else {
                let keep = if buf.ends_with("cid") {
                    3
                } else if buf.ends_with("ci") {
                    2
                } else if buf.ends_with('c') {
                    1
                } else {
                    0
                };
                if buf.len() > keep {
                    let emit_len = buf.len() - keep;
                    out.push_str(&buf[..emit_len]);
                    self.pending.drain(..emit_len);
                }
                break;
            };

            out.push_str(&buf[..cid_start]);
            self.pending.drain(..cid_start);

            let buf = self.pending.as_str();
            debug_assert!(buf.starts_with("cid:"));
            let after = &buf[4..];
            let end = after
                .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '>' | '&'))
                .unwrap_or(after.len());

            if end < after.len() {
                let cid_raw = &after[..end];
                let cid_enc = utf8_percent_encode(cid_raw, NON_ALPHANUMERIC).to_string();
                out.push_str(&format!("{}/cid/{}", self.local_view_prefix, cid_enc));
                self.pending.drain(..4 + end);
                continue;
            }

            // No terminator yet — hold from `cid:` onward for the next chunk (or finish).
            break;
        }

        out
    }

    /// Remaining tail (incomplete `cid:` URLs are emitted verbatim).
    pub(crate) fn finish(self) -> String {
        self.pending
    }
}

/// The whole raw message through the store's callbacks.
pub(crate) fn load_message_raw(store: DynStore, folder: &str, message_id: &str, done: Box<dyn FnOnce(Result<Vec<u8>, String>) + Send>) {
    let id = MessageId::new(message_id.to_owned());
    store.open_folder(
        folder,
        Box::new(|_| {}),
        Box::new(move |r| {
            let folder: Arc<Box<dyn Folder>> = match r {
                Ok(f) => Arc::new(f),
                Err(e) => return done(Err(e.to_string())),
            };
            let raw: Arc<StdMutex<Vec<u8>>> = Arc::new(StdMutex::new(Vec::new()));
            let (raw_chunks, keep) = (Arc::clone(&raw), Arc::clone(&folder));
            folder.get_message(
                &id,
                Box::new(|_| {}),
                Box::new(move |chunk| raw_chunks.lock().expect("raw lock").extend_from_slice(chunk)),
                Box::new(move |r| {
                    // The folder lives until its fetch has reported.
                    drop(keep);
                    done(r.map(|()| std::mem::take(&mut *raw.lock().expect("raw lock"))).map_err(|e| e.to_string()))
                }),
            );
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_text_is_escaped() {
        assert_eq!(escape_html_text("a < b & \"c\" > d"), "a &lt; b &amp; &quot;c&quot; &gt; d");
        assert_eq!(escape_html_text("plain"), "plain");
    }

    /// The same text through the rewriter whole and a character at a time.
    fn rewritten(text: &str, one_at_a_time: bool) -> String {
        let mut r = StreamingCidRewriter::new("/mail/a/b/c".to_owned());
        let mut out = String::new();
        if one_at_a_time {
            let mut buf = [0u8; 4];
            for c in text.chars() {
                out.push_str(&r.feed(c.encode_utf8(&mut buf)));
            }
        } else {
            out.push_str(&r.feed(text));
        }
        out.push_str(&r.finish());
        out
    }

    #[test]
    fn a_cid_reference_is_pointed_at_the_message_however_the_text_arrives() {
        // An `&` ends the reference, as the start of an entity; what follows is left as it was.
        let html = r#"<img src="cid:logo@example"><p>x</p><img src='cid:b&c'>"#;
        let want = r#"<img src="/mail/a/b/c/cid/logo%40example"><p>x</p><img src='/mail/a/b/c/cid/b&c'>"#;
        assert_eq!(rewritten(html, false), want);
        assert_eq!(rewritten(html, true), want, "a character at a time");
    }

    #[test]
    fn text_without_a_cid_and_an_unfinished_one_come_out_as_they_went_in() {
        for s in ["no references here, a c ci cid", "decide", "ends in cid:unfinished", "a ci"] {
            assert_eq!(rewritten(s, false), s, "{s}");
            assert_eq!(rewritten(s, true), s, "{s} one at a time");
        }
    }
}
