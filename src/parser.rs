//! Deterministic extraction. Nothing here touches the LLM.
//!
//! These are the facts a parser can know for certain: the content hash, the
//! title, explicit links, and any frontmatter the author wrote. The enrichment
//! model fills the *gaps* (summary, inferred tags, topics, a status guess); it
//! never replaces what this module asserts.

use std::path::{Component, Path, PathBuf};
use std::str::FromStr;

use chrono::NaiveDate;
use gray_matter::{engine::YAML, Matter};
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use serde::Deserialize;

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

/// Resolve a raw link target — a relative `.md` path or a `[[wikilink]]`/bare
/// name — to the id of the idea it points at, matching how [`id_from_path`]
/// derives ids. Path links are resolved relative to the source file's directory;
/// wikilinks and bare names are slugified as a stem.
///
/// The result may not correspond to any indexed idea (a dangling link); that's
/// fine — it simply won't join at query time.
pub fn link_target_to_id(target: &str, source_rel: &Path) -> String {
    let target = target.trim();
    if target.to_ascii_lowercase().ends_with(".md") {
        let base = source_rel.parent().unwrap_or_else(|| Path::new(""));
        id_from_path(&normalize_lexically(&base.join(target)))
    } else {
        id_from_path(Path::new(target))
    }
}

/// Resolve `.` and `..` components without touching the filesystem.
fn normalize_lexically(path: &Path) -> PathBuf {
    let mut parts: Vec<&std::ffi::OsStr> = Vec::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop();
            }
            Component::Normal(c) => parts.push(c),
            Component::RootDir | Component::Prefix(_) => {}
        }
    }
    parts.iter().collect()
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

/// Optional YAML frontmatter. Every field is optional so a note with partial or
/// no frontmatter still deserializes cleanly.
#[derive(Debug, Default, Deserialize)]
struct FrontMatter {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    tags: Option<Vec<String>>,
    #[serde(default)]
    last_reviewed: Option<String>,
}

/// Parse a markdown document into its deterministic fields.
///
/// Two asserted-metadata conventions coexist in the corpus and both are honored:
///   * YAML frontmatter (`---\nstatus: ...\n---`), split off by gray_matter; and
///   * the project's four-field **blockquote header** (`> **Status**: ...`,
///     `> **Last reviewed**: ...`), which is what the real notes actually use.
/// Frontmatter wins where present; the blockquote header fills the gaps. Both
/// are asserted sources — nothing here is proposed.
pub fn parse(filename_stem: &str, raw: &str) -> ParsedDoc {
    let entity = Matter::<YAML>::new().parse(raw);
    let content = entity.content;
    let fm: FrontMatter = entity
        .data
        .as_ref()
        .and_then(|pod| pod.deserialize().ok())
        .unwrap_or_default();

    let (bq_status, bq_reviewed) = parse_blockquote_header(&content);

    let status = fm
        .status
        .as_deref()
        .and_then(status_token)
        .or(bq_status);

    let last_reviewed = fm
        .last_reviewed
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok())
        .or(bq_reviewed);

    let tags = fm
        .tags
        .unwrap_or_default()
        .into_iter()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();

    let title = first_h1(&content).unwrap_or_else(|| filename_stem.to_string());

    let mut link_targets = extract_links(&content);
    link_targets.extend(extract_wikilinks(&content));

    ParsedDoc {
        title,
        body: content,
        status,
        tags,
        last_reviewed,
        link_targets,
    }
}

/// Read `Status` and `Last reviewed` from the blockquote header convention.
///
/// Lines look like `> **Status**: Concept` or `> **Status:** Active — v0.1`.
/// We strip the `>` and the bold markers, then match the field name at the
/// *start* of the line so a prose line such as `> **Why this status**: ...`
/// can't masquerade as the status field. The first match of each field wins.
fn parse_blockquote_header(markdown: &str) -> (Option<Status>, Option<NaiveDate>) {
    let mut status = None;
    let mut last_reviewed = None;

    for line in markdown.lines() {
        let Some(rest) = line.trim_start().strip_prefix('>') else {
            continue;
        };
        // Drop bold markers so `**Status**:` and `**Status:**` both normalize.
        let cleaned = rest.replace("**", "");
        let cleaned = cleaned.trim_start();
        let lower = cleaned.to_ascii_lowercase();

        if status.is_none() {
            if let Some(val) = strip_ci_prefix(cleaned, &lower, "status:") {
                status = status_token(val);
            }
        }
        if last_reviewed.is_none() {
            if let Some(val) = strip_ci_prefix(cleaned, &lower, "last reviewed:") {
                last_reviewed = find_iso_date(val);
            }
        }
        if status.is_some() && last_reviewed.is_some() {
            break;
        }
    }

    (status, last_reviewed)
}

/// If `lower` (an ASCII-lowercased copy of `s`) starts with `prefix`, return the
/// remainder of `s` past the prefix, trimmed. ASCII-lowercasing preserves byte
/// length, so the offset is valid in the original.
fn strip_ci_prefix<'a>(s: &'a str, lower: &str, prefix: &str) -> Option<&'a str> {
    lower
        .starts_with(prefix)
        .then(|| s[prefix.len()..].trim_start())
}

/// Recognize a status from the leading word of a value, tolerating trailing
/// prose like `Active — foundational document`. Returns `None` for anything
/// unrecognized (including bare `unknown`) so the value is left unasserted and
/// enrichment may still propose one.
fn status_token(value: &str) -> Option<Status> {
    let token: String = value
        .trim_start()
        .chars()
        .take_while(|c| c.is_alphabetic())
        .collect();
    match Status::from_str(&token) {
        Ok(Status::Unknown) => None,
        Ok(status) => Some(status),
        Err(_) => None,
    }
}

/// First `YYYY-MM-DD` date anywhere in the string, tolerating trailing text such
/// as `2026-05-17 (rev 1)`.
fn find_iso_date(s: &str) -> Option<NaiveDate> {
    for (i, _) in s.char_indices() {
        let end = i + 10;
        if end <= s.len() && s.is_char_boundary(end) {
            if let Ok(date) = NaiveDate::parse_from_str(&s[i..end], "%Y-%m-%d") {
                return Some(date);
            }
        }
    }
    None
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

/// `[[wikilink]]` targets. pulldown-cmark ignores these, so scan by hand — but
/// skip anything inside a code span or fenced code block, otherwise TOML
/// table-array syntax (`[[shaft]]`) and inline code (`` `[[period]]` ``) in a
/// note get mistaken for links. Real prose wikilinks live outside code.
pub fn extract_wikilinks(markdown: &str) -> Vec<String> {
    let code = code_ranges(markdown);
    let in_code = |pos: usize| code.iter().any(|r| r.contains(&pos));

    let mut out = Vec::new();
    let bytes = markdown.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'[' && bytes[i + 1] == b'[' && !in_code(i) {
            if let Some(end) = markdown[i + 2..].find("]]") {
                let target = markdown[i + 2..i + 2 + end].trim().to_string();
                if !target.is_empty() {
                    out.push(target);
                }
                i = i + 2 + end + 2;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Byte ranges covered by inline code spans and fenced code blocks, so callers
/// can ignore markup that only looks like a link.
fn code_ranges(markdown: &str) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let mut block_start: Option<usize> = None;
    for (ev, range) in Parser::new(markdown).into_offset_iter() {
        match ev {
            Event::Start(Tag::CodeBlock(_)) => block_start = Some(range.start),
            Event::End(TagEnd::CodeBlock) => {
                if let Some(start) = block_start.take() {
                    ranges.push(start..range.end);
                }
            }
            Event::Code(_) => ranges.push(range),
            _ => {}
        }
    }
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blockquote_header_is_the_real_corpus_convention() {
        // Mirrors the actual notes: blockquote header, bold field names, a title
        // H1 after the header, trailing prose on the status, a date with a
        // parenthetical, a markdown link and a wikilink.
        let raw = "\
> **Status**: Concept  \n\
> **Provenance**: Shane, Claude  \n\
> **Last reviewed**: 2026-05-17 (rev 1)  \n\
> **Why this status**: Dormant ideas should not trip the status parser.\n\
\n\
---\n\
\n\
# CircuitMind — Visual-First Electronics\n\
\n\
See [MindSim](mindsim.md) and [[Eigenspace]].\n";
        let doc = parse("circuitmind", raw);

        assert_eq!(doc.title, "CircuitMind — Visual-First Electronics");
        assert_eq!(doc.status, Some(Status::Concept));
        assert_eq!(doc.last_reviewed, NaiveDate::from_ymd_opt(2026, 5, 17));
        assert_eq!(doc.link_targets, vec!["mindsim.md", "Eigenspace"]);
        // No YAML frontmatter => no asserted tags here.
        assert!(doc.tags.is_empty());
    }

    #[test]
    fn status_with_trailing_prose_keeps_just_the_status() {
        let raw = "> **Status**: Active — foundational document\n\n# Locus\n";
        let doc = parse("vision", raw);
        assert_eq!(doc.status, Some(Status::Active));
    }

    #[test]
    fn why_this_status_line_does_not_set_status() {
        // The only blockquote line mentions "status" inside prose; nothing should
        // be asserted, and the title falls back to the filename stem.
        let raw = "> **Why this status**: Concept work is paused.\n\nbody text\n";
        let doc = parse("note", raw);
        assert_eq!(doc.status, None);
        assert_eq!(doc.title, "note");
    }

    #[test]
    fn yaml_frontmatter_convention_still_works() {
        let raw = "\
---\n\
status: dormant\n\
tags: [llm, embeddings]\n\
last_reviewed: \"2026-02-10\"\n\
---\n\
\n\
# LLM Idea Graph\n\
\n\
body\n";
        let doc = parse("llm-idea-graph", raw);
        assert_eq!(doc.title, "LLM Idea Graph");
        assert_eq!(doc.status, Some(Status::Dormant));
        assert_eq!(doc.last_reviewed, NaiveDate::from_ymd_opt(2026, 2, 10));
        assert_eq!(doc.tags, vec!["llm", "embeddings"]);
        // Frontmatter is stripped from the body.
        assert!(!doc.body.contains("status: dormant"));
    }

    #[test]
    fn no_metadata_falls_back_to_stem_and_unset() {
        let doc = parse("loose-note", "just some prose, no header, no h1\n");
        assert_eq!(doc.title, "loose-note");
        assert_eq!(doc.status, None);
        assert_eq!(doc.last_reviewed, None);
        assert!(doc.tags.is_empty());
        assert!(doc.link_targets.is_empty());
    }

    #[test]
    fn draft_status_is_recognized() {
        let raw = "> **Status**: Draft v0.2 — pre-submission\n\n# Paper\n";
        assert_eq!(parse("p", raw).status, Some(Status::Draft));
    }

    #[test]
    fn angle_bracket_link_with_spaces_is_extracted() {
        // Markdown link destinations with spaces must be wrapped in <…>; the UI
        // does this when accepting an unlinked mention (F-016). Confirm the path
        // (and only it) is extracted, and resolves back to the target id.
        let md = "see [Threshold](<../Project Threshold /Threshold.md>) here";
        let links = extract_links(md);
        assert_eq!(links, vec!["../Project Threshold /Threshold.md"]);
        assert_eq!(
            link_target_to_id(&links[0], Path::new("Interesting ideas/Ideas.md")),
            "project-threshold--threshold"
        );
        // Without the angle brackets the space breaks the destination → not a link.
        assert!(extract_links("see [T](../Project Threshold /T.md) here").is_empty());
    }

    #[test]
    fn link_target_resolves_to_id() {
        let src = Path::new("Interesting ideas/Locus/VISION.md");
        // same-directory path link
        assert_eq!(
            link_target_to_id("ARCHITECTURE.md", src),
            "interesting-ideas-locus-architecture"
        );
        // parent-relative path link (`..` resolved lexically)
        assert_eq!(link_target_to_id("../Ideas.md", src), "interesting-ideas-ideas");
        // wikilink / bare name slugified as a stem
        assert_eq!(
            link_target_to_id("Spatial Canvas", Path::new("spatial-canvas.md")),
            "spatial-canvas"
        );
    }

    #[test]
    fn wikilinks_ignore_code_blocks_and_spans() {
        // Real prose wikilink should be found; TOML table-arrays in a fenced
        // block and a `[[period]]` in an inline code span must not.
        let raw = "\
See [[Real Note]] in prose.\n\
\n\
Run `verify [[period]]` checks.\n\
\n\
```toml\n\
[[shaft]]\n\
[[wheel]]\n\
```\n";
        assert_eq!(extract_wikilinks(raw), vec!["Real Note"]);
    }
}
