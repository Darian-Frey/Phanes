//! Deterministic extraction. Nothing here touches the LLM.
//!
//! These are the facts a parser can know for certain: the content hash, the
//! title, explicit links, and any frontmatter the author wrote. The enrichment
//! model fills the *gaps* (summary, inferred tags, topics, a status guess); it
//! never replaces what this module asserts.

use std::path::Path;

use chrono::NaiveDate;
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};

use crate::model::Status;

/// Outcome of parsing one file, before any enrichment.
#[derive(Debug, Clone, Default)]
pub struct ParsedDoc {
    pub title: String,
    pub body: String,
    /// Frontmatter status, if the author declared one.
    pub status: Option<Status>,
    /// Asserted tags from frontmatter.
    pub tags: Vec<String>,
    pub last_reviewed: Option<NaiveDate>,
    /// Raw link destinations (`*.md` targets and `[[wikilinks]]`).
    pub link_targets: Vec<String>,
}

/// blake3 of the raw bytes, hex-encoded. The cache key for enrichment.
pub fn content_hash(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

/// Derive a stable id slug from a path relative to the ideas root.
pub fn id_from_path(rel_path: &Path) -> String {
    rel_path
        .with_extension("")
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

/// Parse a markdown document into its deterministic fields.
///
/// Splits frontmatter first (gray_matter), then walks the body for the first
/// H1 (title) and link destinations. Wikilinks are scanned separately because
/// pulldown-cmark does not parse `[[...]]`.
pub fn parse(_filename_stem: &str, _raw: &str) -> ParsedDoc {
    // TODO(claude-code):
    //   1. let split = gray_matter::Matter::<gray_matter::engine::YAML>::new().parse(raw);
    //   2. read `status`, `tags`, `last_reviewed` from split.data when present.
    //   3. title  = first_h1(&split.content).unwrap_or_else(|| filename_stem.to_string());
    //   4. links  = extract_links(&split.content) plus extract_wikilinks(&split.content);
    //   5. body   = split.content (plain-ish text is fine for FTS porter tokenizer).
    todo!("assemble ParsedDoc from the helpers below")
}

/// First level-1 heading text, if any. Implemented: this is unambiguous.
pub fn first_h1(markdown: &str) -> Option<String> {
    let mut parser = Parser::new(markdown);
    while let Some(ev) = parser.next() {
        if let Event::Start(Tag::Heading { level: HeadingLevel::H1, .. }) = ev {
            let mut text = String::new();
            for ev in parser.by_ref() {
                match ev {
                    Event::Text(t) | Event::Code(t) => text.push_str(&t),
                    Event::End(TagEnd::Heading(_)) => return Some(text.trim().to_string()),
                    _ => {}
                }
            }
        }
    }
    None
}

/// Markdown link destinations ending in `.md`. Implemented.
pub fn extract_links(markdown: &str) -> Vec<String> {
    let mut out = Vec::new();
    for ev in Parser::new(markdown) {
        if let Event::Start(Tag::Link { dest_url, .. }) = ev {
            let dest = dest_url.to_string();
            if dest.ends_with(".md") {
                out.push(dest);
            }
        }
    }
    out
}

/// `[[wikilink]]` targets. pulldown-cmark ignores these, so scan by hand.
pub fn extract_wikilinks(markdown: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = markdown.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'[' && bytes[i + 1] == b'[' {
            if let Some(end) = markdown[i + 2..].find("]]") {
                out.push(markdown[i + 2..i + 2 + end].trim().to_string());
                i = i + 2 + end + 2;
                continue;
            }
        }
        i += 1;
    }
    out
}
