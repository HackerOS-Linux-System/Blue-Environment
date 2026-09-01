use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub struct FeedItem {
    pub guid: String,
    pub title: String,
    pub link: String,
    /// Plain-text-ish summary — `<description>` (RSS) or `<summary>`
    /// (Atom), HTML tags NOT stripped (a real news reader UI needs to
    /// decide whether to render it as HTML or strip it itself; this
    /// layer doesn't make that call). Empty string, not `None`, when
    /// absent — matches `title`/`link`'s existing "empty means missing"
    /// convention in this parser rather than introducing a second way
    /// to express the same thing.
    pub description: String,
    /// `<pubDate>` (RSS, RFC 2822) or `<published>`/`<updated>` (Atom,
    /// ISO 8601) verbatim, whichever was present — NOT normalized to a
    /// single format, since RSS and Atom use genuinely different date
    /// formats and this layer isn't a date-parsing library. The
    /// frontend is responsible for displaying whatever string shows up
    /// here (or attempting `new Date(published)`, which handles both
    /// formats well enough in every modern JS engine for display
    /// purposes).
    pub published: String,
}

/// Extracts the text between the first `<tag>`/`<tag ...>` and its
/// matching `</tag>` inside `xml`, starting the search at byte offset
/// `from`. Deliberately not a real XML parser (no namespace handling,
/// no entity decoding beyond the five predefined XML entities, no
/// nested-same-tag awareness) — RSS/Atom feeds are simple enough in
/// practice that this naive approach reliably extracts `<title>`,
/// `<link>`, `<guid>`, and `<id>` from a well-formed feed, which is all
/// this needs; a feed exotic enough to defeat this (CDATA with a
/// literal `</title>` substring inside it, for instance) is rare enough
/// that a full XML parser dependency isn't justified for either caller
/// (a "did something new get published" checker, or a news reader that
/// re-fetches and re-parses on demand rather than needing perfect
/// fidelity). Returns `None` past the last occurrence of `tag` in
/// `xml`, `Some((text, end_offset))` otherwise — `end_offset` lets the
/// caller keep scanning forward for repeated tags (every
/// `<item>`/`<entry>` block).
pub fn extract_tag(xml: &str, tag: &str, from: usize) -> Option<(String, usize)> {
    let open_needle = format!("<{tag}");
    let close_needle = format!("</{tag}>");
    let rest = xml.get(from..)?;
    let open_rel = rest.find(&open_needle)?;
    let after_open_tag = rest.get(open_rel..)?.find('>')? + open_rel + 1;
    let close_rel = rest.get(after_open_tag..)?.find(&close_needle)?;
    let text = &rest[after_open_tag..after_open_tag + close_rel];
    let end = from + after_open_tag + close_rel + close_needle.len();
    Some((decode_entities(text.trim()), end))
}

pub fn decode_entities(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"")
        .replace("&apos;", "'").replace("&amp;", "&")
}

/// Splits `xml` into item/entry blocks (RSS `<item>` or Atom `<entry>`)
/// and pulls `title`/`link`/`guid`(or Atom `id`)/`description`/
/// `published` out of each. RSS's `<link>` is plain text content;
/// Atom's is an empty `<link href="...">` element instead, which
/// `extract_tag` (built for text-content tags) can't read — handled
/// with a small regex just for that one shape (`regex` is already a
/// dependency elsewhere in this crate, not added solely for this).
/// Caps at 100 items — a sane bound against a pathological feed, not a
/// pagination mechanism.
pub fn parse_feed_items(xml: &str) -> Vec<FeedItem> {
    let is_atom = xml.contains("<feed") && xml.contains("xmlns=\"http://www.w3.org/2005/Atom\"");
    let item_tag = if is_atom { "entry" } else { "item" };
    let mut items = Vec::new();
    let mut pos = 0usize;
    let atom_link_re = regex::Regex::new(r#"<link[^>]*href="([^"]+)""#).unwrap();

    loop {
        let Some((block, next_pos)) = extract_tag(xml, item_tag, pos) else { break };
        pos = next_pos;
        let title = extract_tag(&block, "title", 0).map(|(t, _)| t).unwrap_or_default();
        let link = if is_atom {
            atom_link_re.captures(&block).map(|c| c[1].to_string()).unwrap_or_default()
        } else {
            extract_tag(&block, "link", 0).map(|(t, _)| t).unwrap_or_default()
        };
        let guid = extract_tag(&block, if is_atom { "id" } else { "guid" }, 0)
            .map(|(t, _)| t)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| link.clone());
        let description = extract_tag(&block, if is_atom { "summary" } else { "description" }, 0)
            .map(|(t, _)| t)
            .unwrap_or_default();
        let published = extract_tag(&block, "pubDate", 0)
            .or_else(|| extract_tag(&block, "published", 0))
            .or_else(|| extract_tag(&block, "updated", 0))
            .map(|(t, _)| t)
            .unwrap_or_default();
        if !title.is_empty() && !guid.is_empty() {
            items.push(FeedItem { guid, title, link, description, published });
        }
        if items.len() >= 100 { break } // sane cap against a pathological feed
    }
    items
}
