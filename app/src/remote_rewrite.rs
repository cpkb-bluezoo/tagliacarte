/*
 * remote_rewrite.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * Points the remote images in a message's HTML at the back end's proxy.
 *
 * This only runs once the user has asked for images (the setting, or the
 * button for one message). Until then nothing is rewritten and the body's
 * content security policy refuses everything that is not served by the app,
 * so a failure here can only leave an image blocked, never fetched directly.
 *
 * The scan is streaming: it passes text through as it arrives and holds only
 * a tag that has not ended, a comment's last characters, or the inside of a
 * `<style>` element. It does not build a document; it is not a sanitiser.
 * Links (`href`) are never touched.
 */

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

/// A tag longer than this is not a tag the scanner will hold; it is passed
/// through as it is (and so stays blocked).
const MAX_TAG: usize = 32 * 1024;
/// The same for the inside of a `<style>` element.
const MAX_STYLE: usize = 256 * 1024;

/// `{prefix}{url, base64url}`: the form the proxy is asked for. Only URL-safe
/// characters, so no escaping is needed in an attribute or a path.
pub fn proxy_url(prefix: &str, url: &str) -> String {
    format!("{prefix}{}", URL_SAFE_NO_PAD.encode(url.as_bytes()))
}

/// The address behind the last part of a proxy URL.
pub fn proxied_url(encoded: &str) -> Option<String> {
    if encoded.is_empty() {
        return None;
    }
    let bytes = URL_SAFE_NO_PAD.decode(encoded.as_bytes()).ok()?;
    String::from_utf8(bytes).ok()
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Text,
    Tag,
    Comment,
    Style,
}

pub struct RemoteRewriter {
    prefix: String,
    state: State,
    /// What has arrived but cannot be passed on yet.
    held: String,
    /// How many addresses have been pointed at the proxy.
    count: std::cell::Cell<usize>,
}

impl RemoteRewriter {
    /// `prefix` is where the proxy lives, ending in `/`, for example `/remote/KEY/`.
    pub fn new(prefix: String) -> Self {
        Self { prefix, state: State::Text, held: String::new(), count: std::cell::Cell::new(0) }
    }

    /// How many remote addresses have been pointed at the proxy so far.
    pub fn rewritten(&self) -> usize {
        self.count.get()
    }

    /// The next piece of the document; returns what can be sent on.
    pub fn feed(&mut self, text: &str) -> String {
        let mut input = std::mem::take(&mut self.held);
        input.push_str(text);
        let mut out = String::with_capacity(input.len());
        let mut i = 0;
        loop {
            let rest = &input[i..];
            match self.state {
                State::Text => {
                    let Some(k) = rest.find('<') else {
                        out.push_str(rest);
                        i = input.len();
                        break;
                    };
                    out.push_str(&rest[..k]);
                    i += k;
                    let rest = &input[i..];
                    match rest.as_bytes().get(1) {
                        None => break, // a `<` at the very end: wait to see what follows
                        Some(b'!') if rest.len() < 4 && "<!--".starts_with(rest) => break,
                        Some(b'!') if rest.starts_with("<!--") => self.state = State::Comment,
                        Some(c) if c.is_ascii_alphabetic() || matches!(c, b'/' | b'!' | b'?') => self.state = State::Tag,
                        Some(_) => {
                            out.push('<');
                            i += 1;
                        }
                    }
                }
                State::Comment => match rest.find("-->") {
                    Some(k) => {
                        out.push_str(&rest[..k + 3]);
                        i += k + 3;
                        self.state = State::Text;
                    }
                    None => {
                        // Keep the last two characters: they may be the start of `-->`.
                        let keep = rest.char_indices().rev().nth(1).map_or(0, |(n, _)| n);
                        out.push_str(&rest[..keep]);
                        i += keep;
                        break;
                    }
                },
                State::Tag => match find_tag_end(rest) {
                    Some(end) => {
                        let tag = &rest[..=end];
                        out.push_str(&self.rewrite_tag(tag));
                        i += end + 1;
                        self.state = if opens_style(tag) { State::Style } else { State::Text };
                    }
                    None if rest.len() > MAX_TAG => {
                        out.push_str(rest);
                        i = input.len();
                        self.state = State::Text;
                        break;
                    }
                    None => break,
                },
                State::Style => match find_ci(rest, "</style") {
                    Some(k) => {
                        out.push_str(&self.rewrite_css(&rest[..k], false));
                        i += k;
                        self.state = State::Text;
                    }
                    None if rest.len() > MAX_STYLE => {
                        // Too much to hold: pass it on, apart from a possible start of `</style`.
                        let keep = rest.char_indices().rev().nth(6).map_or(0, |(n, _)| n);
                        out.push_str(&rest[..keep]);
                        i += keep;
                        break;
                    }
                    None => break,
                },
            }
        }
        self.held = input[i..].to_owned();
        out
    }

    /// The end of the document; anything still held goes out as it came.
    pub fn finish(self) -> String {
        self.held
    }

    /// An address from an attribute or from CSS, as the proxy form, or `None`
    /// if it is not a web address.
    fn candidate(&self, raw: &str, decode: bool) -> Option<String> {
        let decoded;
        let s = if decode {
            decoded = decode_entities(raw);
            decoded.trim()
        } else {
            raw.trim()
        };
        let lower = s.get(..8).unwrap_or(s).to_ascii_lowercase();
        let proxied = if lower.starts_with("http://") || lower.starts_with("https://") {
            Some(proxy_url(&self.prefix, s))
        } else if s.starts_with("//") {
            Some(proxy_url(&self.prefix, &format!("https:{s}")))
        } else {
            None
        };
        if proxied.is_some() {
            self.count.set(self.count.get() + 1);
        }
        proxied
    }

    fn rewrite_tag(&self, tag: &str) -> String {
        let attrs = parse_attributes(tag);
        let Some(name) = attrs.name.as_deref() else { return tag.to_owned() };
        let is_image_input = name == "input"
            && attrs.list.iter().any(|(n, r)| n == "type" && tag[r.clone()].trim().eq_ignore_ascii_case("image"));
        let mut out = String::with_capacity(tag.len() + 64);
        let mut at = 0;
        for (attr, range) in &attrs.list {
            let value = &tag[range.clone()];
            let new = match (attr.as_str(), name) {
                ("src", "img" | "source") => self.candidate(value, true),
                ("src", "input") if is_image_input => self.candidate(value, true),
                ("srcset", "img" | "source") => Some(self.rewrite_srcset(value)),
                ("poster", "video") => self.candidate(value, true),
                ("background", "body" | "table" | "tr" | "td" | "th") => self.candidate(value, true),
                ("style", _) => Some(self.rewrite_css(value, true)),
                _ => None,
            };
            if let Some(new) = new {
                out.push_str(&tag[at..range.start]);
                out.push_str(&new);
                at = range.end;
            }
        }
        out.push_str(&tag[at..]);
        out
    }

    fn rewrite_srcset(&self, value: &str) -> String {
        value
            .split(',')
            .map(|candidate| {
                let c = candidate.trim();
                let (url, descriptor) = match c.find(char::is_whitespace) {
                    Some(k) => (&c[..k], &c[k..]),
                    None => (c, ""),
                };
                match self.candidate(url, true) {
                    Some(p) => format!("{p}{descriptor}"),
                    None => c.to_owned(),
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// `url(...)` references in a piece of CSS. In an attribute the text is
    /// still HTML (`&quot;` for a quote); in a `<style>` element it is not.
    fn rewrite_css(&self, css: &str, in_attribute: bool) -> String {
        let mut out = String::with_capacity(css.len());
        let mut at = 0;
        while let Some(k) = find_ci(&css[at..], "url(") {
            let open = at + k + 4;
            let Some(close) = css[open..].find(')') else { break };
            let inner = &css[open..open + close];
            let lead_ws = inner.len() - inner.trim_start().len();
            let trail_ws = inner.len() - inner.trim_end().len();
            let mut body = &inner[lead_ws..inner.len() - trail_ws.min(inner.len() - lead_ws)];
            let (mut before, mut after) = (&inner[..lead_ws], &inner[inner.len() - trail_ws.min(inner.len() - lead_ws)..]);
            for q in ["\"", "'", "&quot;", "&#39;", "&apos;"] {
                if let Some(unq) = body.strip_prefix(q).and_then(|b| b.strip_suffix(q)) {
                    let qlen = q.len();
                    before = &inner[..lead_ws + qlen];
                    after = &inner[inner.len() - trail_ws.min(inner.len() - lead_ws) - qlen..];
                    body = unq;
                    break;
                }
            }
            out.push_str(&css[at..open]);
            match self.candidate(body, in_attribute) {
                Some(p) => {
                    out.push_str(before);
                    out.push_str(&p);
                    out.push_str(after);
                }
                None => out.push_str(inner),
            }
            at = open + close;
        }
        out.push_str(&css[at..]);
        out
    }
}

/// The index of the `>` that ends the tag at the start of `s`; a `>` inside a
/// quoted attribute value does not end it.
fn find_tag_end(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let mut quote: Option<u8> = None;
    let mut after_equals = false;
    for (i, &c) in b.iter().enumerate().skip(1) {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
            }
            None => match c {
                b'>' => return Some(i),
                b'"' | b'\'' if after_equals => quote = Some(c),
                b'=' => after_equals = true,
                c if c.is_ascii_whitespace() => {}
                _ => after_equals = false,
            },
        }
    }
    None
}

/// Whether the tag is `<style ...>` (not a closing tag, not self-closing).
fn opens_style(tag: &str) -> bool {
    let b = tag.as_bytes();
    b.len() >= 7
        && tag[1..6].eq_ignore_ascii_case("style")
        && (b[6].is_ascii_whitespace() || b[6] == b'>')
        && !tag.ends_with("/>")
}

struct Attributes {
    name: Option<String>,
    /// Attribute name (lower case) and where its value is in the tag, without quotes.
    list: Vec<(String, std::ops::Range<usize>)>,
}

fn parse_attributes(tag: &str) -> Attributes {
    let b = tag.as_bytes();
    let n = b.len();
    let mut i = 1;
    if b.get(1) == Some(&b'/') || b.get(1) == Some(&b'!') || b.get(1) == Some(&b'?') {
        return Attributes { name: None, list: Vec::new() };
    }
    let ws = |c: u8| c.is_ascii_whitespace();
    while i < n && !ws(b[i]) && b[i] != b'/' && b[i] != b'>' {
        i += 1;
    }
    let name = tag[1..i].to_ascii_lowercase();
    let mut list = Vec::new();
    loop {
        while i < n && (ws(b[i]) || b[i] == b'/') {
            i += 1;
        }
        if i >= n || b[i] == b'>' {
            break;
        }
        let start = i;
        while i < n && !ws(b[i]) && b[i] != b'=' && b[i] != b'>' && b[i] != b'/' {
            i += 1;
        }
        if start == i {
            i += 1;
            continue;
        }
        let attr = tag[start..i].to_ascii_lowercase();
        while i < n && ws(b[i]) {
            i += 1;
        }
        if i < n && b[i] == b'=' {
            i += 1;
            while i < n && ws(b[i]) {
                i += 1;
            }
            let (vs, ve);
            if i < n && (b[i] == b'"' || b[i] == b'\'') {
                let q = b[i];
                i += 1;
                vs = i;
                while i < n && b[i] != q {
                    i += 1;
                }
                ve = i;
                if i < n {
                    i += 1;
                }
            } else {
                vs = i;
                while i < n && !ws(b[i]) && b[i] != b'>' {
                    i += 1;
                }
                ve = i;
            }
            list.push((attr, vs..ve));
        }
    }
    Attributes { name: Some(name), list }
}

/// Where `needle` (lower case ASCII) first occurs in `hay`, ignoring ASCII case.
fn find_ci(hay: &str, needle: &str) -> Option<usize> {
    let (h, n) = (hay.as_bytes(), needle.as_bytes());
    if n.is_empty() || h.len() < n.len() {
        return None;
    }
    (0..=h.len() - n.len()).find(|&i| h[i..i + n.len()].eq_ignore_ascii_case(n))
}

/// The character references that turn up in a URL in an attribute.
fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(k) = rest.find('&') {
        out.push_str(&rest[..k]);
        rest = &rest[k..];
        let end = rest.find(';').filter(|&e| e <= 10);
        let decoded = end.and_then(|e| {
            let name = &rest[1..e];
            let ch = match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ => name
                    .strip_prefix('#')
                    .and_then(|n| match n.strip_prefix(['x', 'X']) {
                        Some(hex) => u32::from_str_radix(hex, 16).ok(),
                        None => n.parse().ok(),
                    })
                    .and_then(char::from_u32),
            };
            ch.map(|c| (c, e))
        });
        match decoded {
            Some((c, e)) => {
                out.push(c);
                rest = &rest[e + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: &str = "/remote/K/";

    fn u(url: &str) -> String {
        proxy_url(P, url)
    }

    /// The whole document in one piece.
    fn whole(input: &str) -> String {
        let mut r = RemoteRewriter::new(P.to_owned());
        let mut out = r.feed(input);
        out.push_str(&r.finish());
        out
    }

    /// The same document a character at a time, which puts a chunk boundary
    /// inside every tag, attribute, URL and closing tag.
    fn pieces(input: &str) -> String {
        let mut r = RemoteRewriter::new(P.to_owned());
        let mut out = String::new();
        let mut buf = [0u8; 4];
        for c in input.chars() {
            out.push_str(&r.feed(c.encode_utf8(&mut buf)));
        }
        out.push_str(&r.finish());
        out
    }

    /// Both ways must agree, and give `expected`.
    fn rewrites(input: &str, expected: &str) {
        assert_eq!(whole(input), expected, "in one piece: {input}");
        assert_eq!(pieces(input), expected, "a character at a time: {input}");
    }

    #[test]
    fn an_image_source_goes_through_the_proxy_however_it_is_written() {
        let want = u("https://a.example/p.png");
        rewrites(r#"<img src="https://a.example/p.png">"#, &format!(r#"<img src="{want}">"#));
        rewrites("<img src='https://a.example/p.png' alt='x'>", &format!("<img src='{want}' alt='x'>"));
        rewrites("<IMG SRC=https://a.example/p.png width=1>", &format!("<IMG SRC={want} width=1>"));
        rewrites(r#"<img alt="a > b" src="https://a.example/p.png"/>"#, &format!(r#"<img alt="a > b" src="{want}"/>"#));
        rewrites(r#"<img  src = "https://a.example/p.png" >"#, &format!(r#"<img  src = "{want}" >"#));
        rewrites(r#"<img src="HTTP://A.example/p.png">"#, &format!(r#"<img src="{}">"#, u("HTTP://A.example/p.png")));
    }

    #[test]
    fn entities_in_the_address_are_decoded_before_it_is_fetched() {
        rewrites(
            r#"<img src="https://a.example/p?a=1&amp;b=2&#38;c=3&#x26;d=4">"#,
            &format!(r#"<img src="{}">"#, u("https://a.example/p?a=1&b=2&c=3&d=4")),
        );
    }

    #[test]
    fn a_protocol_relative_address_is_fetched_securely() {
        rewrites(r#"<img src="//cdn.example/p.png">"#, &format!(r#"<img src="{}">"#, u("https://cdn.example/p.png")));
    }

    #[test]
    fn links_and_local_references_are_left_alone() {
        for s in [
            r#"<a href="https://a.example/x">go</a>"#,
            r#"<img src="cid:logo@example">"#,
            r#"<img src="data:image/png;base64,AAAA">"#,
            r#"<img src="relative/p.png">"#,
            r#"<img src="/remote/K/abc">"#,
            r#"<img src="ftp://a.example/p.png">"#,
            r#"<img src="javascript:alert(1)">"#,
            r#"<img data-src="https://a.example/p.png">"#,
            r#"<link rel="stylesheet" href="https://a.example/s.css">"#,
            r#"<a href="https://a.example/x"><b>bold</b></a> 1 < 2 and 3 > 2"#,
            "plain text only",
            "",
        ] {
            rewrites(s, s);
        }
        rewrites(
            r#"<a href="https://a.example/x"><img src="https://b.example/y.gif"></a>"#,
            &format!(r#"<a href="https://a.example/x"><img src="{}"></a>"#, u("https://b.example/y.gif")),
        );
    }

    #[test]
    fn every_place_an_image_can_be_named_is_covered() {
        let a = u("https://a.example/1.png");
        let b = u("https://a.example/2.png");
        rewrites(
            r#"<img srcset="https://a.example/1.png 1x, https://a.example/2.png 2x">"#,
            &format!(r#"<img srcset="{a} 1x, {b} 2x">"#),
        );
        rewrites(
            r#"<picture><source srcset="https://a.example/1.png"><img src="x"></picture>"#,
            &format!(r#"<picture><source srcset="{a}"><img src="x"></picture>"#),
        );
        rewrites(r#"<td background="https://a.example/1.png">"#, &format!(r#"<td background="{a}">"#));
        rewrites(r#"<body BACKGROUND='https://a.example/1.png'>"#, &format!(r#"<body BACKGROUND='{a}'>"#));
        rewrites(r#"<video poster="https://a.example/1.png">"#, &format!(r#"<video poster="{a}">"#));
        rewrites(r#"<input type="image" src="https://a.example/1.png">"#, &format!(r#"<input type="image" src="{a}">"#));
    }

    #[test]
    fn style_attributes_and_style_elements_are_rewritten_too() {
        let a = u("https://a.example/1.png");
        rewrites(
            r#"<div style="background:url('https://a.example/1.png') no-repeat;color:red">"#,
            &format!(r#"<div style="background:url('{a}') no-repeat;color:red">"#),
        );
        rewrites(
            r#"<div style='background: URL( https://a.example/1.png )'>"#,
            &format!(r#"<div style='background: URL( {a} )'>"#),
        );
        rewrites(
            r#"<div style="background:url(&quot;https://a.example/1.png&quot;)">"#,
            &format!(r#"<div style="background:url(&quot;{a}&quot;)">"#),
        );
        rewrites(
            "<style>td{background:url(https://a.example/1.png)}\nb{x:url(\"data:image/gif;base64,AA\")}</style><p>after</p>",
            &format!("<style>td{{background:url({a})}}\nb{{x:url(\"data:image/gif;base64,AA\")}}</style><p>after</p>"),
        );
        rewrites(
            "<STYLE type=\"text/css\">a{background:url(https://a.example/1.png)}</STYLE>x",
            &format!("<STYLE type=\"text/css\">a{{background:url({a})}}</STYLE>x"),
        );
    }

    #[test]
    fn comments_text_and_unfinished_tags_pass_through_untouched() {
        rewrites(
            r#"<!-- <img src="https://a.example/1.png"> --><p>hi</p>"#,
            r#"<!-- <img src="https://a.example/1.png"> --><p>hi</p>"#,
        );
        // An unfinished tag at the end is returned as it came.
        rewrites(r#"text <img src="https://a.example/1.png"#, r#"text <img src="https://a.example/1.png"#);
        // A tag too long to be one is not held for ever.
        let long = format!("<img alt=\"{}\" src=\"https://a.example/1.png\">tail", "x".repeat(70_000));
        assert!(whole(&long).ends_with("tail"));
    }

    #[test]
    fn it_counts_what_it_pointed_at_the_proxy() {
        let mut r = RemoteRewriter::new(P.to_owned());
        r.feed(r#"<img src="https://a.example/1.png"><img src="cid:x"><a href="https://b.example/"></a>"#);
        r.feed(r#"<td background="https://a.example/2.png"><div style="background:url(https://a.example/3.png)">"#);
        assert_eq!(r.rewritten(), 3);
        assert_eq!(RemoteRewriter::new(P.to_owned()).rewritten(), 0);
    }

    #[test]
    fn an_address_comes_back_from_the_proxy_form() {
        let url = "https://a.example/p?a=1&b=é#x";
        let made = proxy_url(P, url);
        assert!(made.starts_with(P));
        assert!(made[P.len()..].bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'));
        assert_eq!(proxied_url(&made[P.len()..]).as_deref(), Some(url));
        assert_eq!(proxied_url("!!!not base64"), None);
        assert_eq!(proxied_url(""), None);
    }
}
