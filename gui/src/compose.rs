//! Composing, replying, forwarding and sending.
//!
//! Everything slow (reading the original, sending, saving a draft) returns at
//! once and reports with an event. Attachments are never in the arguments:
//! they are ids in the upload store, and SMTP reads their files as the message
//! is written. Gmail's REST send, NNTP posting and the IMAP draft copy still
//! take a whole message from their libraries, so for those the attachments are
//! read into memory at the last moment.

use std::sync::Arc;

use tagliacarte_app::compose_send::{OutgoingFile, send_smtp_streaming};
use tagliacarte_app::frb_api::frb_contacts::frb_contacts_recipient_completion;
use tagliacarte_app::frb_api::{
    FrbComposeAttachment, FrbComposeMessage, FrbNntpComposeMessage, frb_generate_smtp_compose_message_id,
    frb_save_imap_draft, frb_send_gmail_message, frb_send_nntp_message,
};
use tagliacarte_app::mail_view::{self, MessageRef, SourceMessage};
use tagliacarte_core::json::{JsonWriter, writer_into_string};

use crate::args::Args;
use crate::commands::Ctx;
use crate::events::{int, mailboxes, opt_str, str};

fn event(build: impl FnOnce(&mut JsonWriter)) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    build(&mut w);
    w.write_end_object();
    writer_into_string(w)
}

/// What the page needs of the original: its envelope and its text.
fn source_event(request_id: &str, r: &Result<SourceMessage, String>) -> String {
    event(|w| {
        str(w, "type", "composeSource");
        str(w, "requestId", request_id);
        match r {
            Err(e) => str(w, "error", e),
            Ok(m) => {
                let h = &m.headers;
                mailboxes(w, "from", &h.from);
                mailboxes(w, "to", &h.to);
                mailboxes(w, "cc", &h.cc);
                opt_str(w, "subject", &h.subject);
                if let Some(ms) = h.date_ms {
                    int(w, "dateMs", ms);
                }
                opt_str(w, "messageId", &h.message_id);
                opt_str(w, "references", &h.references);
                opt_str(w, "bodyPlain", &m.body_plain);
                opt_str(w, "bodyHtml", &m.body_html);
            }
        }
    })
}

fn non_empty(v: Option<&str>) -> Option<String> {
    v.map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned)
}

/// `none` and empty mean no signing or encryption.
fn crypto_mode(v: Option<&str>) -> Option<String> {
    non_empty(v).filter(|m| m != "none")
}

/// The message fields from the page's arguments; attachments are supplied
/// separately because how they are carried depends on where the message goes.
fn compose_from_args(a: &Args, attachments: Vec<FrbComposeAttachment>) -> Result<FrbComposeMessage, String> {
    let list = |key: &str| -> Result<Vec<String>, String> {
        Ok(a.strings(key).unwrap_or_default().into_iter().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()).collect())
    };
    Ok(FrbComposeMessage {
        from: a.str("from")?.trim().to_owned(),
        to: list("to")?,
        cc: list("cc")?,
        bcc: list("bcc")?,
        subject: a.opt_str("subject").unwrap_or("").trim().to_owned(),
        body_plain: a.opt_str("bodyPlain").unwrap_or("").to_owned(),
        body_html: non_empty(a.opt_str("bodyHtml")),
        attachments,
        dsn_notify: non_empty(a.opt_str("dsn")),
        store_account_id: None,
        in_reply_to: non_empty(a.opt_str("inReplyTo")),
        references: non_empty(a.opt_str("references")),
        message_id: non_empty(a.opt_str("messageId")),
        crypto_mode: crypto_mode(a.opt_str("crypto")),
    })
}

/// The uploaded attachments named in `ids`, as files to stream.
fn files(ctx: &Ctx, ids: &[String]) -> Result<Vec<OutgoingFile>, String> {
    ids.iter().map(|id| ctx.uploads.outgoing(id)).collect()
}

/// The same attachments read into memory, for the paths that cannot stream.
fn in_memory(ctx: &Ctx, ids: &[String]) -> Result<Vec<FrbComposeAttachment>, String> {
    ids.iter().map(|id| ctx.uploads.for_message(id)).collect()
}

/// `Some(result)` if `name` is one of this module's commands.
pub fn run(ctx: &Ctx, name: &str, body: &str) -> Option<Result<Option<String>, String>> {
    const MINE: [&str; 5] =
        ["compose.source", "compose.messageId", "compose.send", "compose.saveDraft", "contacts.complete"];
    if !MINE.contains(&name) {
        return None;
    }
    Some(dispatch(ctx, name, body))
}

fn dispatch(ctx: &Ctx, name: &str, body: &str) -> Result<Option<String>, String> {
    let a = Args::parse(body)?;
    match name {
        "compose.source" => {
            let request_id = a.str("requestId")?;
            let msg = MessageRef {
                account_id: a.str("accountId")?,
                folder: a.str("folder")?,
                message_id: a.str("messageId")?,
            };
            let hub = Arc::clone(&ctx.hub);
            mail_view::load_source(msg, Box::new(move |r| hub.push(source_event(&request_id, &r))));
            Ok(None)
        }
        "compose.messageId" => {
            let id = frb_generate_smtp_compose_message_id(a.str("from")?)?;
            Ok(Some(event(|w| str(w, "messageId", &id))))
        }
        "compose.send" => {
            let kind = a.str("kind")?;
            let ids = a.strings("attachments").unwrap_or_default();
            let request_id = a.str("requestId")?;
            let work: Box<dyn FnOnce() -> Result<(), String> + Send> = match kind.as_str() {
                "smtp" => {
                    let transport = a.str("transportId")?;
                    let compose = compose_from_args(&a, Vec::new())?;
                    let files = files(ctx, &ids)?;
                    Box::new(move || send_smtp_streaming(&transport, &compose, &files))
                }
                "gmail" => {
                    let mut compose = compose_from_args(&a, in_memory(ctx, &ids)?)?;
                    compose.store_account_id = None;
                    let account = a.str("accountId")?;
                    Box::new(move || frb_send_gmail_message(account, compose))
                }
                "nntp" => {
                    let account = a.str("accountId")?;
                    let nntp = FrbNntpComposeMessage {
                        from: a.str("from")?.trim().to_owned(),
                        newsgroups: a.strings("newsgroups")?.into_iter().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()).collect(),
                        subject: a.opt_str("subject").unwrap_or("").trim().to_owned(),
                        body_plain: a.opt_str("bodyPlain").unwrap_or("").to_owned(),
                        attachments: in_memory(ctx, &ids)?,
                        in_reply_to: non_empty(a.opt_str("inReplyTo")),
                        references: non_empty(a.opt_str("references")),
                    };
                    Box::new(move || frb_send_nntp_message(account, nntp))
                }
                other => return Err(format!("unknown send kind '{other}'")),
            };
            let hub = Arc::clone(&ctx.hub);
            std::thread::spawn(move || {
                let r = work();
                hub.push(crate::events::done_json("composeSent", &request_id, r.err().as_deref()));
            });
            Ok(None)
        }
        "compose.saveDraft" => {
            let request_id = a.str("requestId")?;
            let account = a.str("accountId")?;
            let ids = a.strings("attachments").unwrap_or_default();
            let compose = compose_from_args(&a, in_memory(ctx, &ids)?)?;
            let replace = a.opt_u64("replaceUid").map(|u| u as i64);
            let hub = Arc::clone(&ctx.hub);
            std::thread::spawn(move || {
                let r = frb_save_imap_draft(account, compose, replace);
                hub.push(event(|w| {
                    str(w, "type", "composeDraftSaved");
                    str(w, "requestId", &request_id);
                    match &r {
                        Ok(uid) => {
                            if let Some(u) = uid {
                                int(w, "uid", *u);
                            }
                        }
                        Err(e) => str(w, "error", e),
                    }
                }));
            });
            Ok(None)
        }
        "contacts.complete" => {
            let c = frb_contacts_recipient_completion(a.str("prefix")?)?;
            Ok(Some(event(|w| {
                if let Some(c) = c {
                    str(w, "fullAddress", &c.full_address);
                    str(w, "ghostSuffix", &c.ghost_suffix);
                }
            })))
        }
        _ => Err(format!("unknown command '{name}'")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;
    use tagliacarte_app::mail_view::{Headers, Mailbox};

    #[test]
    fn the_message_comes_from_the_arguments_with_empties_dropped() {
        let a = Args::parse(
            r#"{"from":" Alice <a@x.org> ","to":["b@x.org"," ","c@x.org"],"cc":[],"subject":" Hi ","bodyPlain":"text","bodyHtml":" ","dsn":"","inReplyTo":" <m@x> ","crypto":"none","messageId":"<n@x>"}"#,
        )
        .unwrap();
        let m = compose_from_args(&a, Vec::new()).unwrap();
        assert_eq!(m.from, "Alice <a@x.org>");
        assert_eq!(m.to, ["b@x.org", "c@x.org"]);
        assert!(m.cc.is_empty() && m.bcc.is_empty());
        assert_eq!(m.subject, "Hi");
        assert_eq!(m.body_html, None);
        assert_eq!(m.dsn_notify, None);
        assert_eq!(m.in_reply_to.as_deref(), Some("<m@x>"));
        assert_eq!(m.crypto_mode, None);
        assert_eq!(m.message_id.as_deref(), Some("<n@x>"));
        assert!(compose_from_args(&Args::parse(r#"{"to":["b@x"]}"#).unwrap(), Vec::new()).is_err());

        let signed = Args::parse(r#"{"from":"a@x","crypto":"sign_encrypt"}"#).unwrap();
        assert_eq!(compose_from_args(&signed, Vec::new()).unwrap().crypto_mode.as_deref(), Some("sign_encrypt"));
    }

    #[test]
    fn the_source_event_carries_the_envelope_and_text_or_the_error() {
        let ok = SourceMessage {
            headers: Headers {
                from: vec![Mailbox { name: Some("Ann".into()), email: "ann@x.org".into() }],
                subject: Some("S \"q\"".into()),
                date_ms: Some(9),
                message_id: Some("<i@x>".into()),
                references: Some("<r@x>".into()),
                ..Headers::default()
            },
            body_plain: Some("plain\nbody".into()),
            body_html: None,
        };
        let v = Value::parse(&source_event("r1", &Ok(ok))).unwrap();
        assert_eq!(v.opt_str("type"), Some("composeSource"));
        assert_eq!(v.opt_str("subject"), Some("S \"q\""));
        assert_eq!(v.opt_str("bodyPlain"), Some("plain\nbody"));
        assert_eq!(v.opt_str("references"), Some("<r@x>"));
        assert_eq!(v.opt_str("bodyHtml"), None);
        let bad = Value::parse(&source_event("r2", &Err("nope".into()))).unwrap();
        assert_eq!(bad.opt_str("error"), Some("nope"));
        assert_eq!(bad.opt_str("subject"), None);
    }
}
