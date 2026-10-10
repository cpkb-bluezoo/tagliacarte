/*
 * parser.rs
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

//! [`XmlParser`]: Tractrix underneath, [`XmlContentHandler`] on top.

use std::io::Read;

use bytes::Bytes;
use tractrix::{FeatureSet, ParseResult, Parser, XmlHandler};

use super::error::XmlError;
use super::handler::XmlContentHandler;

const CHUNK: usize = 8 * 1024;

/// Push XML parser. Build once per document; feed it a reader or a
/// buffer together with the handler that receives the events.
///
/// Whitespace before the XML declaration is skipped: hand-edited config
/// files often have a leading blank line, and nothing is lost by it.
pub struct XmlParser {
    namespace_aware: bool,
}

/// Drops whitespace ahead of the document's first byte.
struct LeadingWhitespace {
    seen_content: bool,
}

impl LeadingWhitespace {
    fn new() -> Self {
        Self { seen_content: false }
    }

    fn trim<'a>(&mut self, chunk: &'a [u8]) -> &'a [u8] {
        if self.seen_content {
            return chunk;
        }
        let start = chunk.iter().position(|b| !b.is_ascii_whitespace());
        match start {
            Some(i) => {
                self.seen_content = true;
                &chunk[i..]
            }
            None => &[],
        }
    }
}

impl XmlParser {
    /// `namespace_aware` leaves `xmlns` declarations out of the attribute
    /// events; names are always delivered as written.
    pub fn new(namespace_aware: bool) -> Self {
        Self { namespace_aware }
    }

    pub fn namespace_aware(&self) -> bool {
        self.namespace_aware
    }

    fn features(&self) -> FeatureSet {
        let mut f = FeatureSet::default();
        f.namespaces = self.namespace_aware;
        f
    }

    /// Read `reader` to EOF in chunks, parsing as the bytes arrive.
    pub fn parse_reader_to_close<R: Read, H: XmlContentHandler + ?Sized>(
        &mut self,
        reader: &mut R,
        handler: &mut H,
    ) -> Result<(), XmlError> {
        let mut bridge = Bridge::new(handler);
        let features = self.features();
        let mut parser = Parser::new(&mut bridge, &features, None, None, None)?;
        let mut lead = LeadingWhitespace::new();
        let mut buf = [0u8; CHUNK];
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            let data = lead.trim(&buf[..n]);
            if !data.is_empty() {
                parser.receive(Bytes::copy_from_slice(data))?;
            }
        }
        parser.close()?;
        Ok(())
    }

    /// Parse a whole document already in memory.
    pub fn parse_buffer_complete<H: XmlContentHandler + ?Sized>(&mut self, data: &[u8], handler: &mut H) -> Result<(), XmlError> {
        let mut bridge = Bridge::new(handler);
        let features = self.features();
        let mut parser = Parser::new(&mut bridge, &features, None, None, None)?;
        let data = LeadingWhitespace::new().trim(data);
        parser.parse_all(Bytes::copy_from_slice(data))?;
        Ok(())
    }

    /// [`Self::parse_buffer_complete`] for a `&str`.
    pub fn parse_document_from_str<H: XmlContentHandler + ?Sized>(&mut self, xml: &str, handler: &mut H) -> Result<(), XmlError> {
        self.parse_buffer_complete(xml.as_bytes(), handler)
    }
}

/// Turns Tractrix's streamed events into whole attributes and whole text
/// runs, and remembers element names for `end_element`.
struct Bridge<'h, H: XmlContentHandler + ?Sized> {
    handler: &'h mut H,
    elements: Vec<String>,
    attr_name: String,
    attr_value: String,
    text: String,
    comment: String,
}

impl<'h, H: XmlContentHandler + ?Sized> Bridge<'h, H> {
    fn new(handler: &'h mut H) -> Self {
        Self {
            handler,
            elements: Vec::new(),
            attr_name: String::new(),
            attr_value: String::new(),
            text: String::new(),
            comment: String::new(),
        }
    }

    fn flush_text(&mut self) {
        if !self.text.is_empty() {
            let t = std::mem::take(&mut self.text);
            self.handler.characters(&t);
        }
    }
}

impl<H: XmlContentHandler + ?Sized> XmlHandler for Bridge<'_, H> {
    fn start_document(&mut self) -> ParseResult<()> {
        self.handler.start_document();
        Ok(())
    }

    fn end_document(&mut self) -> ParseResult<()> {
        self.flush_text();
        self.handler.end_document();
        Ok(())
    }

    fn start_element(&mut self, q_name: &str) -> ParseResult<()> {
        self.flush_text();
        self.elements.push(q_name.to_string());
        self.handler.start_element(None, q_name);
        Ok(())
    }

    fn start_attribute(&mut self, name: &str, _ty: &str, _declared: bool, _specified: bool) -> ParseResult<()> {
        self.attr_name.clear();
        self.attr_name.push_str(name);
        self.attr_value.clear();
        Ok(())
    }

    fn attribute_value_content(&mut self, value: &str, end: bool) -> ParseResult<()> {
        self.attr_value.push_str(value);
        if end {
            let name = std::mem::take(&mut self.attr_name);
            let value = std::mem::take(&mut self.attr_value);
            self.handler.attribute(None, &name, &value);
        }
        Ok(())
    }

    fn characters(&mut self, text: &str, _ignorable: bool, end: bool) -> ParseResult<()> {
        self.text.push_str(text);
        if end {
            self.flush_text();
        }
        Ok(())
    }

    fn end_element(&mut self) -> ParseResult<()> {
        self.flush_text();
        if let Some(name) = self.elements.pop() {
            self.handler.end_element(None, &name);
        }
        Ok(())
    }

    fn comment_data(&mut self, text: &str, end: bool) -> ParseResult<()> {
        self.comment.push_str(text);
        if end {
            let c = std::mem::take(&mut self.comment);
            self.handler.comment(&c);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Events(Vec<String>);

    impl XmlContentHandler for Events {
        fn start_element(&mut self, _ns: Option<&str>, local_name: &str) {
            self.0.push(format!("<{local_name}>"));
        }
        fn attribute(&mut self, _ns: Option<&str>, local_name: &str, value: &str) {
            self.0.push(format!("@{local_name}={value}"));
        }
        fn characters(&mut self, text: &str) {
            self.0.push(format!("'{text}'"));
        }
        fn comment(&mut self, text: &str) {
            self.0.push(format!("<!--{text}-->"));
        }
        fn end_element(&mut self, _ns: Option<&str>, local_name: &str) {
            self.0.push(format!("</{local_name}>"));
        }
    }

    #[test]
    fn whole_attributes_text_and_names_on_end() {
        let xml = r#"<?xml version="1.0"?><a x="1 &amp; 2" y='z'><!-- c --><b>hi &lt;there&gt;</b><c/></a>"#;
        let mut ev = Events::default();
        XmlParser::new(false).parse_document_from_str(xml, &mut ev).unwrap();
        assert_eq!(
            ev.0,
            vec!["<a>", "@x=1 & 2", "@y=z", "<!-- c -->", "<b>", "'hi <there>'", "</b>", "<c>", "</c>", "</a>"]
        );
    }

    #[test]
    fn chunked_input_gives_the_same_events() {
        let xml = b"<root><item name=\"first\">some text here</item></root>";
        let mut whole = Events::default();
        XmlParser::new(false).parse_buffer_complete(xml, &mut whole).unwrap();
        struct OneByte<'a>(&'a [u8]);
        impl Read for OneByte<'_> {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                if self.0.is_empty() {
                    return Ok(0);
                }
                buf[0] = self.0[0];
                self.0 = &self.0[1..];
                Ok(1)
            }
        }
        let mut chunked = Events::default();
        XmlParser::new(false).parse_reader_to_close(&mut OneByte(xml), &mut chunked).unwrap();
        assert_eq!(whole.0, chunked.0);
        assert_eq!(whole.0[3], "'some text here'");
    }

    #[test]
    fn leading_whitespace_before_the_declaration_is_tolerated() {
        let mut ev = Events::default();
        XmlParser::new(false).parse_document_from_str("\n  <?xml version=\"1.0\"?>\n<a/>", &mut ev).unwrap();
        assert_eq!(ev.0, vec!["<a>", "</a>"]);
    }

    #[test]
    fn malformed_input_is_an_error() {
        let mut ev = Events::default();
        assert!(XmlParser::new(false).parse_document_from_str("<a><b></a>", &mut ev).is_err());
    }
}
