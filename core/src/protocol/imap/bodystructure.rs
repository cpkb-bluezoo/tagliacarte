/*
 * bodystructure.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

//! Parse IMAP `BODYSTRUCTURE` (RFC 3501) and plan which `BODY.PEEK[section]` parts to fetch.

// --- S-expression subset (IMAP FETCH line) --------------------------------

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SExpr {
    Nil,
    List(Vec<SExpr>),
    Str(String),
    Atom(String),
    Number(u64),
}

#[derive(Debug, Clone)]
enum Tok {
    LParen,
    RParen,
    Nil,
    Str(String),
    Atom(String),
    Number(u64),
}

fn tokenize_bodystructure(inner: &str) -> Option<Vec<Tok>> {
    let bytes = inner.as_bytes();
    let mut i = 0usize;
    let mut out = Vec::new();
    while i < bytes.len() {
        match bytes[i] {
            b' ' | b'\r' | b'\n' | b'\t' => i += 1,
            b'(' => {
                out.push(Tok::LParen);
                i += 1;
            }
            b')' => {
                out.push(Tok::RParen);
                i += 1;
            }
            b'"' => {
                i += 1;
                let mut s = String::new();
                while i < bytes.len() {
                    if bytes[i] == b'"' {
                        i += 1;
                        break;
                    }
                    if bytes[i] == b'\\' && i + 1 < bytes.len() {
                        s.push(bytes[i + 1] as char);
                        i += 2;
                    } else {
                        s.push(bytes[i] as char);
                        i += 1;
                    }
                }
                out.push(Tok::Str(s));
            }
            b'0'..=b'9' => {
                let start = i;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                let n: u64 = inner.get(start..i)?.parse().ok()?;
                out.push(Tok::Number(n));
            }
            _ => {
                if i + 3 <= bytes.len()
                    && bytes[i..i + 3].eq_ignore_ascii_case(b"nil")
                    && (i + 3 >= bytes.len()
                        || matches!(bytes[i + 3], b' ' | b'\r' | b'\n' | b'\t' | b'(' | b')'))
                {
                    out.push(Tok::Nil);
                    i += 3;
                } else {
                    let start = i;
                    while i < bytes.len()
                        && !matches!(bytes[i], b' ' | b'\r' | b'\n' | b'\t' | b'(' | b')')
                    {
                        i += 1;
                    }
                    if start == i {
                        return None;
                    }
                    out.push(Tok::Atom(inner.get(start..i)?.to_string()));
                }
            }
        }
    }
    Some(out)
}

fn parse_expr(toks: &[Tok], idx: &mut usize) -> Option<SExpr> {
    let t = toks.get(*idx)?;
    match t {
        Tok::Nil => {
            *idx += 1;
            Some(SExpr::Nil)
        }
        Tok::Number(n) => {
            *idx += 1;
            Some(SExpr::Number(*n))
        }
        Tok::Str(s) => {
            *idx += 1;
            Some(SExpr::Str(s.clone()))
        }
        Tok::Atom(a) => {
            *idx += 1;
            Some(SExpr::Atom(a.clone()))
        }
        Tok::LParen => {
            *idx += 1;
            let mut items = Vec::new();
            while *idx < toks.len() && !matches!(toks[*idx], Tok::RParen) {
                items.push(parse_expr(toks, idx)?);
            }
            if matches!(toks.get(*idx), Some(Tok::RParen)) {
                *idx += 1;
            }
            Some(SExpr::List(items))
        }
        Tok::RParen => None,
    }
}

/// Extract the parenthesized `BODYSTRUCTURE` argument from a single `* n FETCH (...)` line.
pub(crate) fn extract_bodystructure_list(line: &str) -> Option<&str> {
    let key = "BODYSTRUCTURE ";
    let i = line.find(key)?;
    let mut depth = 0i32;
    let mut started = false;
    let bytes = line.as_bytes();
    let mut start_paren = None;
    let mut j = i + key.len();
    while j < bytes.len() {
        match bytes[j] {
            b'(' => {
                if !started {
                    started = true;
                    start_paren = Some(j);
                    depth = 1;
                    j += 1;
                    continue;
                }
                depth += 1;
            }
            b')' => {
                if started {
                    depth -= 1;
                    if depth == 0 {
                        let s = start_paren?;
                        return Some(line.get(s..=j)?);
                    }
                }
            }
            _ => {}
        }
        j += 1;
    }
    None
}

pub(crate) fn parse_bodystructure(line: &str) -> Option<SExpr> {
    let slice = extract_bodystructure_list(line)?;
    let toks = tokenize_bodystructure(slice)?;
    let mut idx = 0usize;
    parse_expr(&toks, &mut idx)
}

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

fn s_string(e: &SExpr) -> Option<String> {
    match e {
        SExpr::Str(s) => Some(s.clone()),
        SExpr::Atom(a) => Some(a.clone()),
        SExpr::Nil => Some(String::new()),
        _ => None,
    }
}

fn parse_params(e: &SExpr) -> Vec<(String, String)> {
    let SExpr::List(items) = e else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < items.len() {
        let k = s_string(&items[i]).unwrap_or_default();
        let v = s_string(&items[i + 1]).unwrap_or_default();
        if !k.is_empty() {
            out.push((k, v));
        }
        i += 2;
    }
    out
}

fn find_filename_in_disp_params(e: &SExpr) -> Option<String> {
    let SExpr::List(items) = e else {
        return None;
    };
    let mut i = 0;
    while i + 1 < items.len() {
        let k = s_string(&items[i])?.to_ascii_uppercase();
        if k == "FILENAME" {
            return s_string(&items[i + 1]);
        }
        i += 2;
    }
    None
}

fn find_disposition(items: &[SExpr]) -> (Option<String>, Option<String>) {
    for e in items.iter().skip(7) {
        let SExpr::List(l) = e else {
            continue;
        };
        if l.len() < 2 {
            continue;
        }
        let dt = s_string(&l[0]).map(|s| s.to_ascii_uppercase());
        let Some(dt) = dt else {
            continue;
        };
        if dt == "ATTACHMENT" || dt == "INLINE" {
            let fname = l.get(1).and_then(|p| find_filename_in_disp_params(p));
            return (Some(dt), fname);
        }
    }
    (None, None)
}

fn charset_from_params(params: &[(String, String)]) -> Option<String> {
    params
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("charset"))
        .map(|(_, v)| v.clone())
}

fn build_tree(expr: &SExpr, prefix: &str) -> Option<BodyTree> {
    let SExpr::List(items) = expr else {
        return None;
    };
    if items.is_empty() {
        return None;
    }
    // Multipart: `(part part ... subtype extension ...)` per RFC 3501 — one or more body `List`s,
    // then `media-subtype`, then any `body-extension` atoms/lists/NILs (servers often send several).
    if matches!(&items[0], SExpr::List(_)) {
        let mut i = 0usize;
        while i < items.len() && matches!(&items[i], SExpr::List(_)) {
            i += 1;
        }
        if i == 0 || i >= items.len() {
            return None;
        }
        let subtype = s_string(&items[i])?.to_ascii_lowercase();
        if subtype.is_empty() {
            return None;
        }
        let children_exprs = &items[..i];
        let mut children = Vec::new();
        for (idx, ch) in children_exprs.iter().enumerate() {
            let sec = if prefix.is_empty() {
                format!("{}", idx + 1)
            } else {
                format!("{}.{}", prefix, idx + 1)
            };
            children.push(build_tree(ch, &sec)?);
        }
        Some(BodyTree::Multipart { subtype, children })
    } else {
        let major = s_string(items.get(0)?)?.to_ascii_uppercase();
        let minor = s_string(items.get(1)?)?.to_ascii_uppercase();
        let params = items.get(2).map(parse_params).unwrap_or_default();
        let content_id = items
            .get(3)
            .and_then(|e| s_string(e))
            .filter(|s| !s.is_empty())
            .map(|mut s| {
                let t = s.trim();
                if t.len() >= 2 && t.starts_with('<') && t.ends_with('>') {
                    s = t[1..t.len() - 1].to_string();
                }
                s
            });
        let encoding = s_string(items.get(5)?)?.to_ascii_uppercase();
        let size = match items.get(6) {
            Some(SExpr::Number(n)) => *n,
            _ => 0,
        };
        let (disposition, filename) = find_disposition(items);
        let sec = if prefix.is_empty() {
            "1".to_string()
        } else {
            prefix.to_string()
        };
        Some(BodyTree::Part(PartInfo {
            section: sec,
            major,
            minor,
            params,
            content_id,
            encoding,
            size,
            disposition,
            filename,
        }))
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

pub fn plan_body_fetch(line: &str) -> Option<BodyFetchPlan> {
    let expr = parse_bodystructure(line)?;
    let tree = build_tree(&expr, "")?;
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
    Some(BodyFetchPlan {
        display,
        attachments,
        inline_cids,
    })
}

pub fn part_bytes_to_string(decoded: &[u8], charset_hint: Option<&str>) -> String {
    let _ = charset_hint;
    String::from_utf8_lossy(decoded).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_plain() {
        let line = r#"* 1 FETCH (UID 1 BODYSTRUCTURE ("TEXT" "PLAIN" ("CHARSET" "UTF-8") NIL NIL "QUOTED-PRINTABLE" 42))"#;
        let p = plan_body_fetch(line).expect("plan");
        match p.display {
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
        let p = plan_body_fetch(line).expect("plan");
        match p.display {
            DisplayFetch::TextPart {
                section, is_html, ..
            } => {
                assert_eq!(section, "2");
                assert!(is_html);
            }
            _ => panic!("expected html"),
        }
    }

    /// Extended multipart BODYSTRUCTURE (extra NILs after boundary) must not break planning — this
    /// used to make `plan_body_fetch` fail and force full `BODY[]` on message open.
    #[test]
    fn parse_alternative_extended_trailing_nil_prefers_html() {
        let line = r#"* 683 FETCH (UID 3529 BODYSTRUCTURE (("TEXT" "plain" ("charset" "UTF-8") NIL NIL "QUOTED-PRINTABLE" 12279 158 NIL NIL NIL NIL)("TEXT" "html" ("charset" "UTF-8") NIL NIL "QUOTED-PRINTABLE" 64748 832 NIL NIL NIL NIL) "alternative" ("boundary" "_----/hALBbAWiPaRTdEKOe/0Uw===_D5/F2-17034-7AF5DC96") NIL NIL NIL))"#;
        let p = plan_body_fetch(line).expect("plan");
        match p.display {
            DisplayFetch::TextPart {
                section, is_html, ..
            } => {
                assert_eq!(section, "2");
                assert!(is_html);
            }
            _ => panic!("expected html part"),
        }
    }
}
