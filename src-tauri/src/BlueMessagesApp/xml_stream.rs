use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::collections::HashMap;
use std::io::BufRead;

#[derive(Debug, Clone, Default)]
pub struct XmlElement {
    /// Local tag name with any namespace prefix stripped (`"message"`,
    /// not `"stream:message"`) — good enough for this module's callers,
    /// which only ever branch on a handful of well-known local names
    /// and don't need to disambiguate same-named elements from
    /// different namespaces.
    pub name: String,
    pub attrs: HashMap<String, String>,
    pub children: Vec<XmlElement>,
    /// Concatenation of this element's direct text nodes (not
    /// descendants') — sufficient for every stanza this client reads,
    /// none of which mix text and child elements at the same level in
    /// a way that would need proper mixed-content handling.
    pub text: String,
}

impl XmlElement {
    pub fn child(&self, local_name: &str) -> Option<&XmlElement> {
        self.children.iter().find(|c| c.name == local_name)
    }

    pub fn child_text(&self, local_name: &str) -> Option<&str> {
        self.child(local_name).map(|c| c.text.as_str())
    }
}

fn local_name(qname: quick_xml::name::QName) -> String {
    let bytes = qname.as_ref();
    let name = match bytes.iter().position(|&b| b == b':') {
        Some(idx) => &bytes[idx + 1..],
        None => bytes,
    };
    String::from_utf8_lossy(name).to_string()
}

/// Reads exactly one complete top-level element from `reader` — an
/// open tag through its matching close tag (arbitrarily nested), or a
/// single self-closing tag. Returns `Ok(None)` if the stream ends (EOF)
/// or the enclosing `</stream:stream>` closes before another element
/// starts, both of which mean "nothing more to read", not an error.
///
/// Blocks (via the underlying blocking `Read`/`BufRead`) until either a
/// complete element or EOF is available — this is what gives this
/// parser its main advantage over the substring-scan approach it
/// replaces: a stanza split across TCP packets simply causes this call
/// to keep blocking for the rest of it, rather than something this
/// module has to detect and handle specially.
pub fn read_one_element<R: BufRead>(reader: &mut Reader<R>) -> Result<Option<XmlElement>, String> {
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf).map_err(|e| format!("XML parse error: {e}"))? {
            Event::Eof => return Ok(None),
            Event::Start(start) => {
                // Found the opening tag of the element we're reading —
                // hand off to the recursive collector for everything up
                // to and including its matching close tag.
                let name = local_name(start.name());
                let attrs = collect_attrs(&start);
                let (children, text) = read_children_until_close(reader, &start.name().as_ref().to_vec())?;
                return Ok(Some(XmlElement { name, attrs, children, text }));
            }
            Event::Empty(start) => {
                // Self-closing (`<proceed/>`, `<starttls/>`, ...) — a
                // complete element with no body to recurse into.
                let name = local_name(start.name());
                let attrs = collect_attrs(&start);
                return Ok(Some(XmlElement { name, attrs, children: Vec::new(), text: String::new() }));
            }
            // A bare </stream:stream> at this level means the server is
            // closing the whole session — same "nothing more to read"
            // outcome as EOF.
            Event::End(end) if local_name(end.name()) == "stream" => return Ok(None),
            // Whitespace/comments/decls/other top-level noise between
            // stanzas (including XMPP's own "whitespace ping"
            // keepalive, a bare space character) — skip and keep
            // waiting for the next real element.
            _ => continue,
        }
    }
}

fn collect_attrs(start: &quick_xml::events::BytesStart) -> HashMap<String, String> {
    let mut attrs = HashMap::new();
    for attr in start.attributes().flatten() {
        let key = local_name(attr.key);
        if let Ok(value) = attr.unescape_value() {
            attrs.insert(key, value.to_string());
        }
    }
    attrs
}

/// Recursively collects everything between an already-consumed opening
/// tag and its matching closing tag (`expected_close_qname`, the raw
/// qualified name bytes so nested same-local-name-different-prefix
/// elements can't be confused for the real close tag).
fn read_children_until_close<R: BufRead>(
    reader: &mut Reader<R>,
    expected_close_qname: &[u8],
) -> Result<(Vec<XmlElement>, String), String> {
    let mut children = Vec::new();
    let mut text = String::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf).map_err(|e| format!("XML parse error: {e}"))? {
            Event::Eof => return Err("stream ended mid-stanza".to_string()),
            Event::End(end) => {
                if end.name().as_ref() == expected_close_qname {
                    return Ok((children, text));
                }
                // A mismatched close tag this deep would indicate a
                // genuinely malformed stream — bail rather than loop
                // forever waiting for a close tag that already passed.
                return Err(format!(
                    "malformed XML: expected closing tag for {:?}, got {:?}",
                    String::from_utf8_lossy(expected_close_qname),
                    String::from_utf8_lossy(end.name().as_ref())
                ));
            }
            Event::Start(start) => {
                let name = local_name(start.name());
                let attrs = collect_attrs(&start);
                let qname = start.name().as_ref().to_vec();
                let (grandchildren, child_text) = read_children_until_close(reader, &qname)?;
                children.push(XmlElement { name, attrs, children: grandchildren, text: child_text });
            }
            Event::Empty(start) => {
                let name = local_name(start.name());
                let attrs = collect_attrs(&start);
                children.push(XmlElement { name, attrs, children: Vec::new(), text: String::new() });
            }
            Event::Text(bytes) => {
                if let Ok(unescaped) = bytes.unescape() {
                    text.push_str(&unescaped);
                }
            }
            Event::CData(bytes) => {
                // CDATA content is raw by definition (no entity
                // escaping to undo) — decode as UTF-8 directly rather
                // than through `BytesText::unescape()`'s unescaping path,
                // which is a different type here in quick-xml's API and
                // isn't meant for this variant.
                text.push_str(&String::from_utf8_lossy(bytes.as_ref()));
            }
            _ => continue, // comments, PIs, etc. — not meaningful to any stanza this client reads
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_first(xml: &str) -> XmlElement {
        let mut reader = Reader::from_reader(xml.as_bytes());
        read_one_element(&mut reader).unwrap().expect("expected an element, got EOF")
    }

    #[test]
    fn parses_self_closing_element_with_attrs() {
        let el = parse_first("<proceed xmlns='urn:ietf:params:xml:ns:xmpp-tls'/>");
        assert_eq!(el.name, "proceed");
        assert_eq!(el.attrs.get("xmlns").map(String::as_str), Some("urn:ietf:params:xml:ns:xmpp-tls"));
    }

    #[test]
    fn parses_nested_message_with_body_text() {
        let el = parse_first("<message to='a@b.com' type='chat'><body>Hello &amp; welcome</body></message>");
        assert_eq!(el.name, "message");
        assert_eq!(el.attrs.get("to").map(String::as_str), Some("a@b.com"));
        assert_eq!(el.child_text("body"), Some("Hello & welcome"));
    }

    #[test]
    fn strips_namespace_prefixes_from_local_names() {
        let el = parse_first("<stream:features xmlns:stream='http://etherx.jabber.org/streams'><starttls/></stream:features>");
        assert_eq!(el.name, "features");
        assert_eq!(el.child("starttls").is_some(), true);
    }

    #[test]
    fn reads_only_one_top_level_element_leaving_the_rest_for_the_next_call() {
        let mut reader = Reader::from_reader("<a/><b/>".as_bytes());
        let first = read_one_element(&mut reader).unwrap().unwrap();
        assert_eq!(first.name, "a");
        let second = read_one_element(&mut reader).unwrap().unwrap();
        assert_eq!(second.name, "b");
        assert!(read_one_element(&mut reader).unwrap().is_none());
    }

    #[test]
    fn deeply_nested_iq_bind_response_extracts_jid() {
        let el = parse_first(
            "<iq type='result' id='bm-bind1'><bind xmlns='urn:ietf:params:xml:ns:xmpp-bind'><jid>alice@example.com/BlueMessages</jid></bind></iq>"
        );
        assert_eq!(el.child("bind").and_then(|b| b.child_text("jid")), Some("alice@example.com/BlueMessages"));
    }

    #[test]
    fn mismatched_close_tag_is_an_error_not_an_infinite_loop() {
        let mut reader = Reader::from_reader("<a><b></c></a>".as_bytes());
        assert!(read_one_element(&mut reader).is_err());
    }
}
