/*
 * mod.rs
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

//! WebDAV client for CalDAV and CardDAV collections: hopf HTTP underneath
//! (`PROPFIND` is just another method), Tractrix for the request body and
//! the `207 Multi-Status` reply. Nothing here knows about calendars or
//! address books; the callers do.
//!
//! The client is blocking: the sync jobs that use it walk a collection
//! one request after another from an ordinary thread, and each request
//! waits for hopf's reply there. It must not be used from a hopf reactor
//! thread.

use std::io;

use bytes::Bytes;
use tractrix::{FeatureSet, ParseResult, Parser, XmlHandler, XmlWriter};

use crate::protocol::http::{send_collect_blocking, Collected, HttpConnection, HttpUrl, Method};

/// One `DAV:response` of a `PROPFIND`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DavMember {
    /// As the server wrote it: an absolute path or a full URL.
    pub href: String,
    pub etag: Option<String>,
    pub content_type: Option<String>,
}

/// A client bound to one origin, with the credentials to use there. The
/// connection is opened on first use and reopened if it drops.
pub struct DavClient {
    origin: HttpUrl,
    auth: Option<String>,
    conn: Option<HttpConnection>,
}

fn basic(user: &str, pass: &str) -> String {
    use base64::Engine;
    format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", user.trim(), pass)))
}

impl DavClient {
    /// A client for the origin of `url`; the returned target is the URL's
    /// path and query, ready for [`Self::propfind`] or [`Self::get`].
    pub fn for_url(url: &str) -> io::Result<(Self, String)> {
        let u = HttpUrl::parse(url)?;
        let target = u.target.clone();
        Ok((Self { origin: u, auth: None, conn: None }, target))
    }

    pub fn with_basic_auth(mut self, user: &str, pass: &str) -> Self {
        self.auth = Some(basic(user, pass));
        self
    }

    pub fn with_bearer(mut self, token: &str) -> Self {
        self.auth = Some(format!("Bearer {}", token.trim()));
        self
    }

    /// Point the client at `url`'s origin, dropping the connection when
    /// the origin changes (a member `href` on another host).
    pub fn retarget(&mut self, url: &str) -> io::Result<String> {
        let u = HttpUrl::parse(url)?;
        if !u.same_origin(&self.origin) {
            self.conn = None;
            self.origin = HttpUrl { target: "/".into(), ..u.clone() };
        }
        Ok(u.target)
    }

    fn conn(&mut self) -> io::Result<&HttpConnection> {
        if self.conn.as_ref().map(|c| !c.is_alive()).unwrap_or(true) {
            self.conn = Some(self.origin.connect_blocking()?);
        }
        Ok(self.conn.as_ref().expect("connected above"))
    }

    /// One request on this origin, waited for. A request that fails
    /// because the connection went away is retried once on a fresh
    /// connection.
    pub fn request(
        &mut self,
        method: Method,
        target: &str,
        headers: &[(&str, &str)],
        body: Option<Vec<u8>>,
    ) -> io::Result<Collected> {
        let auth = self.auth.clone();
        let mut all: Vec<(&str, &str)> = headers.to_vec();
        if let Some(a) = auth.as_deref() {
            all.push(("Authorization", a));
        }
        let had_conn = self.conn.is_some();
        let first = {
            let conn = self.conn()?;
            send_collect_blocking(conn, method, target, &all, body.clone())
        };
        match first {
            Ok(c) => Ok(c),
            Err(e) if had_conn && e.kind() == io::ErrorKind::ConnectionReset => {
                self.conn = None;
                let conn = self.conn()?;
                send_collect_blocking(conn, method, target, &all, body)
            }
            Err(e) => Err(e),
        }
    }

    /// `PROPFIND` with `Depth: 1`, asking for `getetag` and
    /// `getcontenttype`: the members of a collection.
    pub fn propfind(&mut self, target: &str) -> io::Result<Vec<DavMember>> {
        let body = propfind_body()?;
        let resp = self.request(
            Method::Other("PROPFIND"),
            target,
            &[("Content-Type", "application/xml; charset=utf-8"), ("Depth", "1")],
            Some(body),
        )?;
        if !resp.is_success() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("PROPFIND {target}: HTTP {}", resp.status),
            ));
        }
        parse_multistatus(&resp.body)
    }

    pub fn get(&mut self, target: &str) -> io::Result<Collected> {
        self.request(Method::Get, target, &[], None)
    }

    /// `PUT` with `If-Match` when an entity tag is known.
    pub fn put(
        &mut self,
        target: &str,
        content_type: &str,
        body: Vec<u8>,
        if_match: Option<&str>,
    ) -> io::Result<Collected> {
        let mut headers = vec![("Content-Type", content_type)];
        let etag = if_match.map(str::trim).filter(|t| !t.is_empty());
        if let Some(t) = etag {
            headers.push(("If-Match", t));
        }
        self.request(Method::Put, target, &headers, Some(body))
    }
}

/// `<d:propfind xmlns:d="DAV:"><d:prop><d:getetag/><d:getcontenttype/></d:prop></d:propfind>`
pub fn propfind_body() -> io::Result<Vec<u8>> {
    let mut w = XmlWriter::new_vec();
    w.write_raw("<?xml version=\"1.0\" encoding=\"utf-8\"?>")?;
    w.write_start_element_prefixed("d", "propfind", "DAV:")?;
    w.write_namespace("d", "DAV:")?;
    w.write_start_element_prefixed("d", "prop", "DAV:")?;
    w.write_start_element_prefixed("d", "getetag", "DAV:")?;
    w.write_end_element()?;
    w.write_start_element_prefixed("d", "getcontenttype", "DAV:")?;
    w.write_end_element()?;
    w.write_end_element()?;
    w.write_end_element()?;
    w.flush()?;
    Ok(w.into_inner())
}

/// Collects `href`, `getetag` and `getcontenttype` per `response` element,
/// by local name: DAV servers vary in prefixes, and nothing else in a
/// multistatus uses these names.
#[derive(Default)]
struct Multistatus {
    stack: Vec<String>,
    current: Option<DavMember>,
    text: String,
    capturing: bool,
    members: Vec<DavMember>,
}

fn local_name(q_name: &str) -> &str {
    q_name.rsplit(':').next().unwrap_or(q_name)
}

impl XmlHandler for Multistatus {
    fn start_element(&mut self, q_name: &str) -> ParseResult<()> {
        let local = local_name(q_name).to_ascii_lowercase();
        if local == "response" {
            self.current = Some(DavMember::default());
        }
        self.capturing = self.current.is_some() && matches!(local.as_str(), "href" | "getetag" | "getcontenttype");
        if self.capturing {
            self.text.clear();
        }
        self.stack.push(local);
        Ok(())
    }

    fn characters(&mut self, text: &str, _ignorable: bool, _end: bool) -> ParseResult<()> {
        if self.capturing {
            self.text.push_str(text);
        }
        Ok(())
    }

    fn end_element(&mut self) -> ParseResult<()> {
        let Some(local) = self.stack.pop() else { return Ok(()) };
        if let Some(m) = self.current.as_mut() {
            let value = self.text.trim().to_string();
            match local.as_str() {
                // The first href under a response names the resource;
                // later ones (inside properties) do not.
                "href" if m.href.is_empty() => m.href = value,
                "getetag" if !value.is_empty() => m.etag = Some(value),
                "getcontenttype" if !value.is_empty() => m.content_type = Some(value),
                "response" => {
                    if let Some(done) = self.current.take() {
                        if !done.href.is_empty() {
                            self.members.push(done);
                        }
                    }
                }
                _ => {}
            }
        }
        self.capturing = false;
        self.text.clear();
        Ok(())
    }
}

/// Parse a `207 Multi-Status` body into its members.
pub fn parse_multistatus(xml: &[u8]) -> io::Result<Vec<DavMember>> {
    let mut handler = Multistatus::default();
    {
        let features = FeatureSet::default();
        let mut parser = Parser::new(&mut handler, &features, None, None, None)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("multistatus parser: {e}")))?;
        parser
            .parse_all(Bytes::copy_from_slice(xml))
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("multistatus: {e}")))?;
    }
    Ok(handler.members)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn propfind_body_is_the_expected_document() {
        let body = String::from_utf8(propfind_body().unwrap()).unwrap();
        assert!(body.contains("<d:propfind xmlns:d=\"DAV:\">"), "{body}");
        assert!(body.contains("<d:getetag/>") || body.contains("<d:getetag></d:getetag>"), "{body}");
        assert!(body.ends_with("</d:propfind>"), "{body}");
    }

    #[test]
    fn multistatus_members_are_collected_whatever_the_prefix() {
        let xml = br#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:">
  <D:response><D:href>/book/</D:href><D:propstat><D:prop><D:getetag/></D:prop><D:status>HTTP/1.1 404 Not Found</D:status></D:propstat></D:response>
  <D:response>
    <D:href>/book/a.vcf</D:href>
    <D:propstat><D:prop><D:getetag>"abc"</D:getetag><D:getcontenttype>text/vcard</D:getcontenttype></D:prop>
    <D:status>HTTP/1.1 200 OK</D:status></D:propstat>
  </D:response>
  <response xmlns="DAV:"><href>http://h/book/b.ics</href><propstat><prop><getetag>W/"x"</getetag></prop></propstat></response>
</D:multistatus>"#;
        let m = parse_multistatus(xml).unwrap();
        assert_eq!(m.len(), 3);
        assert_eq!(m[0].href, "/book/");
        assert_eq!(m[0].etag, None);
        assert_eq!(m[1], DavMember { href: "/book/a.vcf".into(), etag: Some("\"abc\"".into()), content_type: Some("text/vcard".into()) });
        assert_eq!(m[2].href, "http://h/book/b.ics");
        assert_eq!(m[2].etag.as_deref(), Some("W/\"x\""));
    }
}
