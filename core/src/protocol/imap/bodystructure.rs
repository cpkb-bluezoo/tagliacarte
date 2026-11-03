/*
 * bodystructure.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

//! Plan which `BODY.PEEK[section]` parts of a message to fetch, from the parsed `BODYSTRUCTURE`.

use hopf_imap::{ImapBodyPart, ImapBodyStructure};

// --- Semantic tree ----------------------------------------------------------

#[derive(Debug, Clone)]
pub(crate) struct PartInfo {
    pub section: String,
    pub major: String,
    pub minor: String,
    pub params: Vec<(String, String)>,
    /// IMAP body-fld-id (Content-ID), angle brackets stripped for matching `cid:`.
    pub content_id: Option<String>,
    pub encoding: String,
    pub size: u64,
    pub disposition: Option<String>,
    pub filename: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) enum BodyTree {
    Part(PartInfo),
    Multipart {
        subtype: String,
        children: Vec<BodyTree>,
    },
}

fn charset_from_params(params: &[(String, String)]) -> Option<String> {
    params
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("charset"))
        .map(|(_, v)| v.clone())
}

fn part_info(part: &ImapBodyPart, section: String) -> PartInfo {
    let content_id = part
        .content_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.strip_prefix('<').and_then(|t| t.strip_suffix('>')).unwrap_or(s).to_string());
    PartInfo {
        section,
        major: part.media_type.to_ascii_uppercase(),
        minor: part.media_subtype.to_ascii_uppercase(),
        params: part.params.clone(),
        content_id,
        encoding: part.encoding.to_ascii_uppercase(),
        size: part.size,
        disposition: part.disposition.as_ref().map(|d| d.kind.to_ascii_uppercase()),
        filename: part.disposition.as_ref().and_then(|d| d.filename()).map(str::to_string),
    }
}

/// Our view of hopf's parsed structure. `prefix` is the section number of
/// `bs` itself; empty at the top level, where a lone part is section `1`
/// and a multipart's children are `1`, `2`, ...
fn build_tree(bs: &ImapBodyStructure, prefix: &str) -> BodyTree {
    match bs {
        ImapBodyStructure::Part(part) => {
            let section = if prefix.is_empty() { "1".to_string() } else { prefix.to_string() };
            BodyTree::Part(part_info(part, section))
        }
        ImapBodyStructure::Multipart(mp) => {
            let children = mp
                .parts
                .iter()
                .enumerate()
                .map(|(i, child)| {
                    let section = if prefix.is_empty() { format!("{}", i + 1) } else { format!("{prefix}.{}", i + 1) };
                    build_tree(child, &section)
                })
                .collect();
            BodyTree::Multipart { subtype: mp.media_subtype.to_ascii_lowercase(), children }
        }
    }
}

// --- Planning ---------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum DisplayFetch {
    None,
    TextPart {
        section: String,
        encoding: String,
        is_html: bool,
        charset_hint: Option<String>,
    },
    NestedMessage {
        section: String,
        encoding: String,
    },
}

#[derive(Debug, Clone)]
pub struct AttachmentPlan {
    pub section: String,
    pub encoding: String,
    pub content_type: String,
    pub filename: Option<String>,
    pub size: u64,
    pub content_id: Option<String>,
}

/// Inline part with Content-ID for `cid:` resolution in HTML.
#[derive(Debug, Clone)]
pub struct CidPartInfo {
    pub cid: String,
    pub section: String,
    pub encoding: String,
    pub content_type: String,
}

#[derive(Debug, Clone)]
pub struct BodyFetchPlan {
    pub display: DisplayFetch,
    pub attachments: Vec<AttachmentPlan>,
    pub inline_cids: Vec<CidPartInfo>,
    /// The message is `multipart/signed` at the top, whatever the signature kind.
    /// Checking it needs the whole message, so a caller fetches that only for these.
    pub signed: bool,
}

#[derive(Debug, Clone)]
struct PartRef {
    section: String,
    encoding: String,
    charset_hint: Option<String>,
}

fn collect_text_candidates(node: &BodyTree) -> (Vec<PartRef>, Vec<PartRef>) {
    let mut html = Vec::new();
    let mut plain = Vec::new();
    match node {
        BodyTree::Part(p) => {
            if p.major == "TEXT" && p.minor == "HTML" {
                html.push(PartRef {
                    section: p.section.clone(),
                    encoding: p.encoding.clone(),
                    charset_hint: charset_from_params(&p.params),
                });
            } else if p.major == "TEXT" && p.minor == "PLAIN" {
                plain.push(PartRef {
                    section: p.section.clone(),
                    encoding: p.encoding.clone(),
                    charset_hint: charset_from_params(&p.params),
                });
            } else if p.major == "MESSAGE" && p.minor == "RFC822" {
                // handled separately when walking attachments
            }
        }
        BodyTree::Multipart { children, .. } => {
            for ch in children {
                let (h, pl) = collect_text_candidates(ch);
                html.extend(h);
                plain.extend(pl);
            }
        }
    }
    (html, plain)
}

fn pick_display_for_alternative(node: &BodyTree) -> DisplayFetch {
    let (htmls, plains) = collect_text_candidates(node);
    if let Some(r) = htmls.last() {
        return DisplayFetch::TextPart {
            section: r.section.clone(),
            encoding: r.encoding.clone(),
            is_html: true,
            charset_hint: r.charset_hint.clone(),
        };
    }
    if let Some(r) = plains.first() {
        return DisplayFetch::TextPart {
            section: r.section.clone(),
            encoding: r.encoding.clone(),
            is_html: false,
            charset_hint: r.charset_hint.clone(),
        };
    }
    DisplayFetch::None
}

fn pick_display_mixed_like(children: &[BodyTree]) -> DisplayFetch {
    for ch in children {
        match ch {
            BodyTree::Multipart { subtype, .. } if subtype.eq_ignore_ascii_case("alternative") => {
                let d = pick_display_for_alternative(ch);
                if !matches!(d, DisplayFetch::None) {
                    return d;
                }
            }
            BodyTree::Multipart {
                children: nested, ..
            } => {
                let d = pick_display_mixed_like(nested);
                if !matches!(d, DisplayFetch::None) {
                    return d;
                }
            }
            BodyTree::Part(p) if p.major == "TEXT" && (p.minor == "HTML" || p.minor == "PLAIN") => {
                let is_html = p.minor == "HTML";
                return DisplayFetch::TextPart {
                    section: p.section.clone(),
                    encoding: p.encoding.clone(),
                    is_html,
                    charset_hint: charset_from_params(&p.params),
                };
            }
            BodyTree::Part(p) if p.major == "MESSAGE" && p.minor == "RFC822" => {
                return DisplayFetch::NestedMessage {
                    section: p.section.clone(),
                    encoding: p.encoding.clone(),
                };
            }
            _ => {}
        }
    }
    DisplayFetch::None
}

fn is_attachment_part(p: &PartInfo, display_section: Option<&str>) -> bool {
    if display_section == Some(p.section.as_str()) {
        return false;
    }
    if p.major == "TEXT" && (p.minor == "PLAIN" || p.minor == "HTML") {
        return false;
    }
    if p.major == "MESSAGE" && p.minor == "RFC822" {
        return false;
    }
    if p.disposition.as_deref() == Some("ATTACHMENT") {
        return true;
    }
    if p.filename.is_some() {
        return true;
    }
    matches!(
        p.major.as_str(),
        "APPLICATION" | "IMAGE" | "AUDIO" | "VIDEO" | "FONT" | "MODEL"
    ) || (p.major == "TEXT" && p.minor != "PLAIN" && p.minor != "HTML")
}

fn walk_attachments(node: &BodyTree, display_section: Option<&str>, out: &mut Vec<AttachmentPlan>) {
    match node {
        BodyTree::Part(p) => {
            if p.major == "MESSAGE" && p.minor == "RFC822" {
                return;
            }
            if is_attachment_part(p, display_section) {
                let ct = format!(
                    "{}/{}",
                    p.major.to_ascii_lowercase(),
                    p.minor.to_ascii_lowercase()
                );
                out.push(AttachmentPlan {
                    section: p.section.clone(),
                    encoding: p.encoding.clone(),
                    content_type: ct,
                    filename: p.filename.clone(),
                    size: p.size,
                    content_id: p.content_id.clone(),
                });
            }
        }
        BodyTree::Multipart { children, .. } => {
            for ch in children {
                walk_attachments(ch, display_section, out);
            }
        }
    }
}

fn collect_cid_parts(node: &BodyTree, out: &mut Vec<CidPartInfo>) {
    match node {
        BodyTree::Part(p) => {
            if let Some(cid) = p.content_id.as_ref().filter(|s| !s.is_empty()) {
                let ct = format!(
                    "{}/{}",
                    p.major.to_ascii_lowercase(),
                    p.minor.to_ascii_lowercase()
                );
                out.push(CidPartInfo {
                    cid: cid.clone(),
                    section: p.section.clone(),
                    encoding: p.encoding.clone(),
                    content_type: ct,
                });
            }
        }
        BodyTree::Multipart { children, .. } => {
            for ch in children {
                collect_cid_parts(ch, out);
            }
        }
    }
}

pub fn plan_body_fetch(bodystructure: &ImapBodyStructure) -> BodyFetchPlan {
    let tree = build_tree(bodystructure, "");
    let display = match &tree {
        BodyTree::Part(p) => {
            if p.major == "TEXT" && p.minor == "HTML" {
                DisplayFetch::TextPart {
                    section: p.section.clone(),
                    encoding: p.encoding.clone(),
                    is_html: true,
                    charset_hint: charset_from_params(&p.params),
                }
            } else if p.major == "TEXT" && p.minor == "PLAIN" {
                DisplayFetch::TextPart {
                    section: p.section.clone(),
                    encoding: p.encoding.clone(),
                    is_html: false,
                    charset_hint: charset_from_params(&p.params),
                }
            } else if p.major == "MESSAGE" && p.minor == "RFC822" {
                DisplayFetch::NestedMessage {
                    section: p.section.clone(),
                    encoding: p.encoding.clone(),
                }
            } else {
                DisplayFetch::None
            }
        }
        BodyTree::Multipart {
            subtype, children, ..
        } => {
            if subtype.eq_ignore_ascii_case("alternative") {
                pick_display_for_alternative(&tree)
            } else {
                pick_display_mixed_like(children)
            }
        }
    };
    let disp_sec = match &display {
        DisplayFetch::TextPart { section, .. } | DisplayFetch::NestedMessage { section, .. } => {
            Some(section.as_str())
        }
        DisplayFetch::None => None,
    };
    let mut attachments = Vec::new();
    walk_attachments(&tree, disp_sec, &mut attachments);
    let mut inline_cids = Vec::new();
    collect_cid_parts(&tree, &mut inline_cids);
    let signed = matches!(&tree, BodyTree::Multipart { subtype, .. } if subtype.eq_ignore_ascii_case("signed"));
    BodyFetchPlan {
        display,
        attachments,
        inline_cids,
        signed,
    }
}

pub fn part_bytes_to_string(decoded: &[u8], charset_hint: Option<&str>) -> String {
    let _ = charset_hint;
    String::from_utf8_lossy(decoded).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plan from a `* n FETCH (UID u BODYSTRUCTURE (...))` line, as the servers send it.
    fn plan(line: &str) -> BodyFetchPlan {
        let text = line.split_once("BODYSTRUCTURE ").expect("BODYSTRUCTURE item").1;
        let text = text.strip_suffix(')').expect("closing paren of the FETCH list");
        plan_body_fetch(&ImapBodyStructure::parse(text).expect("parse"))
    }

    #[test]
    fn a_signed_message_is_known_by_its_top_level_type() {
        // PGP/MIME: the text, then the signature.
        let pgp = r#"* 1 FETCH (UID 1 BODYSTRUCTURE (("TEXT" "PLAIN" ("CHARSET" "UTF-8") NIL NIL "7BIT" 10)("APPLICATION" "PGP-SIGNATURE" NIL NIL NIL "7BIT" 90) "SIGNED" ("BOUNDARY" "x" "PROTOCOL" "application/pgp-signature")))"#;
        assert!(plan(pgp).signed);
        // Signed content with an attachment inside is still signed at the top.
        let nested = r#"* 1 FETCH (UID 1 BODYSTRUCTURE ((("TEXT" "PLAIN" NIL NIL NIL "7BIT" 10)("APPLICATION" "PDF" NIL NIL NIL "BASE64" 99) "MIXED" ("BOUNDARY" "in"))("APPLICATION" "PKCS7-SIGNATURE" NIL NIL NIL "BASE64" 90) "SIGNED" ("BOUNDARY" "x")))"#;
        assert!(plan(nested).signed);
        // Neither an ordinary message nor one that merely contains a signed part is.
        let plain = r#"* 1 FETCH (UID 1 BODYSTRUCTURE ("TEXT" "PLAIN" NIL NIL NIL "7BIT" 42))"#;
        assert!(!plan(plain).signed);
        let alt = r#"* 1 FETCH (UID 1 BODYSTRUCTURE (("TEXT" "PLAIN" NIL NIL NIL "7BIT" 10)("TEXT" "HTML" NIL NIL NIL "7BIT" 20) "ALTERNATIVE" ("BOUNDARY" "x")))"#;
        assert!(!plan(alt).signed);
    }

    #[test]
    fn parse_simple_plain() {
        let line = r#"* 1 FETCH (UID 1 BODYSTRUCTURE ("TEXT" "PLAIN" ("CHARSET" "UTF-8") NIL NIL "QUOTED-PRINTABLE" 42))"#;
        match plan(line).display {
            DisplayFetch::TextPart {
                section, is_html, ..
            } => {
                assert_eq!(section, "1");
                assert!(!is_html);
            }
            _ => panic!("expected text part"),
        }
    }

    #[test]
    fn parse_alternative_prefers_html() {
        let line = r#"* 1 FETCH (UID 1 BODYSTRUCTURE (("TEXT" "PLAIN" ("CHARSET" "UTF-8") NIL NIL "7BIT" 10)("TEXT" "HTML" ("CHARSET" "UTF-8") NIL NIL "QUOTED-PRINTABLE" 20) "ALTERNATIVE" ("BOUNDARY" "x")))"#;
        match plan(line).display {
            DisplayFetch::TextPart {
                section, is_html, ..
            } => {
                assert_eq!(section, "2");
                assert!(is_html);
            }
            _ => panic!("expected html"),
        }
    }

    /// Extended multipart BODYSTRUCTURE (extra NILs after boundary) must not break planning - this
    /// used to make `plan_body_fetch` fail and force full `BODY[]` on message open.
    #[test]
    fn parse_alternative_extended_trailing_nil_prefers_html() {
        let line = r#"* 683 FETCH (UID 3529 BODYSTRUCTURE (("TEXT" "plain" ("charset" "UTF-8") NIL NIL "QUOTED-PRINTABLE" 12279 158 NIL NIL NIL NIL)("TEXT" "html" ("charset" "UTF-8") NIL NIL "QUOTED-PRINTABLE" 64748 832 NIL NIL NIL NIL) "alternative" ("boundary" "_----/hALBbAWiPaRTdEKOe/0Uw===_D5/F2-17034-7AF5DC96") NIL NIL NIL))"#;
        match plan(line).display {
            DisplayFetch::TextPart {
                section, is_html, ..
            } => {
                assert_eq!(section, "2");
                assert!(is_html);
            }
            _ => panic!("expected html part"),
        }
    }

    #[test]
    fn mixed_with_attachment_and_inline_image() {
        let line = r#"* 2 FETCH (UID 2 BODYSTRUCTURE ((("TEXT" "PLAIN" ("CHARSET" "UTF-8") NIL NIL "7BIT" 10 1)("TEXT" "HTML" ("CHARSET" "UTF-8") NIL NIL "7BIT" 20 1) "ALTERNATIVE")("IMAGE" "PNG" NIL "<logo@x>" NIL "BASE64" 300)("APPLICATION" "PDF" ("NAME" "a.pdf") NIL NIL "BASE64" 400 NIL ("ATTACHMENT" ("FILENAME" "a.pdf"))) "MIXED"))"#;
        let p = plan(line);
        match p.display {
            DisplayFetch::TextPart { section, is_html, .. } => {
                assert_eq!(section, "1.2");
                assert!(is_html);
            }
            _ => panic!("expected html part"),
        }
        let sections: Vec<_> = p.attachments.iter().map(|a| a.section.as_str()).collect();
        assert_eq!(sections, ["2", "3"]);
        assert_eq!(p.attachments[1].filename.as_deref(), Some("a.pdf"));
        assert_eq!(p.inline_cids.len(), 1);
        assert_eq!(p.inline_cids[0].cid, "logo@x");
        assert_eq!(p.inline_cids[0].section, "2");
    }
}
