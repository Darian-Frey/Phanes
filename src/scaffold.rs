//! Generating new idea notes — the inverse of [`crate::parser`]. Produces a
//! scaffold-standard note (four-field blockquote header, one idea per file) that
//! `parser::parse` reads straight back. Asserted `--tag` values, when given, go
//! in a small YAML frontmatter block, since that is the channel the parser reads
//! asserted tags from.

use chrono::NaiveDate;

use crate::model::Status;

/// Filename (no directory) for a new note with this title: the title with
/// filesystem-reserved characters replaced by `-`, plus the `.md` extension.
pub fn filename(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c => c,
        })
        .collect();
    let stem = cleaned.trim();
    let stem = if stem.is_empty() { "untitled" } else { stem };
    format!("{stem}.md")
}

/// Body of a new scaffold-standard note: the four-field blockquote header with
/// `Status: Concept` (a freshly captured idea is a concept), an optional YAML
/// frontmatter block carrying asserted tags, and the title as an H1.
pub fn note_body(title: &str, tags: &[String], today: NaiveDate) -> String {
    let mut out = String::new();

    if !tags.is_empty() {
        let quoted = tags
            .iter()
            .map(|t| format!("\"{t}\""))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("---\ntags: [{quoted}]\n---\n\n"));
    }

    out.push_str("> **Status:** Concept\n");
    out.push_str("> **Provenance:** Shane Hartley\n");
    out.push_str(&format!("> **Last reviewed:** {today}\n"));
    out.push_str("> **Why this status:** New idea captured; not yet developed.\n\n");
    out.push_str(&format!("# {title}\n\n"));
    out
}

/// Set (or insert) a note's asserted status in its raw text, returning the new
/// content. Replaces an existing blockquote `> **Status:** …` line or a YAML
/// frontmatter `status:` key; if the note has neither, prepends a blockquote
/// status line. The result round-trips through `parser::parse`. Used by the UI's
/// status dropdown.
pub fn set_status(raw: &str, status: Status) -> String {
    let label = capitalize(status.as_str());
    let trailing_newline = raw.ends_with('\n');
    let mut lines: Vec<String> = raw.lines().map(str::to_string).collect();

    // 1. Replace an existing blockquote status line.
    for line in lines.iter_mut() {
        if is_blockquote_status(line) {
            *line = format!("> **Status:** {label}");
            return join_lines(&lines, trailing_newline);
        }
    }

    // 2. Replace (or insert) a `status:` key inside leading YAML frontmatter.
    if lines.first().map(|l| l.trim()) == Some("---") {
        if let Some(rel_end) = lines.iter().skip(1).position(|l| l.trim() == "---") {
            let end = rel_end + 1; // index of the closing `---`
            for line in &mut lines[1..end] {
                if line.trim_start().to_ascii_lowercase().starts_with("status:") {
                    *line = format!("status: {}", status.as_str());
                    return join_lines(&lines, trailing_newline);
                }
            }
            lines.insert(1, format!("status: {}", status.as_str()));
            return join_lines(&lines, trailing_newline);
        }
    }

    // 3. No status anywhere — prepend a blockquote status line.
    format!("> **Status:** {label}\n\n{raw}")
}

/// Set the asserted tags in a note's raw text (the YAML frontmatter `tags:`
/// key — the channel the parser reads asserted tags from), returning the new
/// content. Updates an existing `tags:` key, inserts one into existing
/// frontmatter, or prepends a frontmatter block if the note has none. An empty
/// list drops the key. Round-trips through `parser::parse`. Used by the UI's
/// tag editor (add / remove / accept).
pub fn set_tags(raw: &str, tags: &[String]) -> String {
    let trailing_newline = raw.ends_with('\n');
    let key_line = format!(
        "tags: [{}]",
        tags.iter().map(|t| format!("\"{t}\"")).collect::<Vec<_>>().join(", ")
    );
    let mut lines: Vec<String> = raw.lines().map(str::to_string).collect();

    // Existing leading YAML frontmatter?
    if lines.first().map(|l| l.trim()) == Some("---") {
        if let Some(rel_end) = lines.iter().skip(1).position(|l| l.trim() == "---") {
            let end = rel_end + 1; // index of the closing `---`
            let tags_idx = lines[1..end]
                .iter()
                .position(|l| l.trim_start().to_ascii_lowercase().starts_with("tags:"))
                .map(|i| i + 1);
            match (tags_idx, tags.is_empty()) {
                (Some(i), true) => {
                    lines.remove(i);
                }
                (Some(i), false) => lines[i] = key_line,
                (None, true) => {} // nothing to remove
                (None, false) => lines.insert(1, key_line),
            }
            return join_lines(&lines, trailing_newline);
        }
    }

    // No frontmatter: prepend a block (unless there are no tags to write).
    if tags.is_empty() {
        return raw.to_string();
    }
    format!("---\n{key_line}\n---\n\n{raw}")
}

/// Turn the first plain-text occurrence of `phrase` in `raw` into a markdown link
/// `[phrase](target)`, returning the new content — or `None` if no clean
/// occurrence exists. "Clean" means: a whole-word match, outside fenced code
/// blocks, and not already part of a link. Used by the UI to **accept an unlinked
/// mention** (F-016): it writes a real, resolvable link into the mentioning note.
pub fn link_mention(raw: &str, phrase: &str, target: &str) -> Option<String> {
    if phrase.trim().is_empty() {
        return None;
    }
    let lower_raw = raw.to_lowercase();
    let lower_phrase = phrase.to_lowercase();
    let code = fenced_ranges(raw);

    let mut from = 0;
    while let Some(rel) = lower_raw[from..].find(&lower_phrase) {
        let start = from + rel;
        let end = start + lower_phrase.len();
        from = start + 1;

        // Whole-word: neighbours must not be alphanumeric.
        let prev = raw[..start].chars().next_back();
        let next = raw[end..].chars().next();
        if prev.is_some_and(|c| c.is_alphanumeric()) || next.is_some_and(|c| c.is_alphanumeric()) {
            continue;
        }
        // Skip if inside a fenced code block.
        if code.iter().any(|r| r.contains(&start)) {
            continue;
        }
        // Skip if it already looks linked (`[phrase`, `(phrase`, `phrase]`, `phrase)`).
        if matches!(prev, Some('[') | Some('(')) || matches!(next, Some(']') | Some(')')) {
            continue;
        }

        let matched = &raw[start..end]; // preserve original casing
        let mut out = String::with_capacity(raw.len() + target.len() + 4);
        out.push_str(&raw[..start]);
        out.push('[');
        out.push_str(matched);
        out.push_str("](");
        out.push_str(target);
        out.push(')');
        out.push_str(&raw[end..]);
        return Some(out);
    }
    None
}

/// Byte ranges of fenced code blocks (``` … ```), so [`link_mention`] can skip
/// mentions that only appear in code. Line-based and forgiving of an unclosed
/// fence (treats the rest of the document as code).
fn fenced_ranges(raw: &str) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let mut in_fence = false;
    let mut fence_start = 0;
    let mut offset = 0;
    for line in raw.split_inclusive('\n') {
        if line.trim_start().starts_with("```") {
            if in_fence {
                ranges.push(fence_start..offset + line.len());
                in_fence = false;
            } else {
                in_fence = true;
                fence_start = offset;
            }
        }
        offset += line.len();
    }
    if in_fence {
        ranges.push(fence_start..raw.len());
    }
    ranges
}

/// True for a blockquote line whose field is `Status:` (anchored, so a prose line
/// like `> **Why this status**:` doesn't match).
fn is_blockquote_status(line: &str) -> bool {
    let Some(rest) = line.trim_start().strip_prefix('>') else {
        return false;
    };
    rest.replace("**", "")
        .trim_start()
        .to_ascii_lowercase()
        .starts_with("status:")
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn join_lines(lines: &[String], trailing_newline: bool) -> String {
    let mut s = lines.join("\n");
    if trailing_newline {
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Status;
    use crate::parser;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 6, 10).unwrap()
    }

    #[test]
    fn note_round_trips_through_parser() {
        let body = note_body("My New Idea", &["ui".into(), "spatial".into()], today());
        let doc = parser::parse("my-new-idea", &body);
        assert_eq!(doc.title, "My New Idea");
        assert_eq!(doc.status, Some(Status::Concept));
        assert_eq!(doc.last_reviewed, Some(today()));
        assert_eq!(doc.tags, vec!["ui", "spatial"]);
    }

    #[test]
    fn note_without_tags_has_no_frontmatter() {
        let body = note_body("Solo", &[], today());
        assert!(!body.contains("---")); // pure blockquote, matches the corpus style
        let doc = parser::parse("solo", &body);
        assert_eq!(doc.status, Some(Status::Concept));
        assert!(doc.tags.is_empty());
    }

    #[test]
    fn filename_sanitizes_reserved_chars_and_empties() {
        assert_eq!(filename("A/B: C"), "A-B- C.md");
        assert_eq!(filename("   "), "untitled.md");
        assert_eq!(filename("CircuitMind"), "CircuitMind.md");
    }

    #[test]
    fn set_status_replaces_blockquote() {
        let raw = "> **Status**: Concept\n> **Why this status**: x\n\n# T\n";
        let out = set_status(raw, Status::Active);
        assert!(out.contains("> **Status:** Active"));
        assert!(!out.contains("Concept"));
        assert!(out.contains("Why this status")); // the prose line is untouched
        assert_eq!(parser::parse("t", &out).status, Some(Status::Active));
    }

    #[test]
    fn set_status_inserts_when_missing() {
        let raw = "# Just a title\n\nsome body\n";
        let out = set_status(raw, Status::Draft);
        assert!(out.starts_with("> **Status:** Draft"));
        assert!(out.contains("# Just a title"));
        let doc = parser::parse("t", &out);
        assert_eq!(doc.status, Some(Status::Draft));
        assert_eq!(doc.title, "Just a title");
    }

    #[test]
    fn set_status_replaces_frontmatter_key() {
        let raw = "---\nstatus: dormant\ntags: [\"x\"]\n---\n\n# T\n";
        let out = set_status(raw, Status::Complete);
        assert_eq!(parser::parse("t", &out).status, Some(Status::Complete));
        assert!(out.contains("tags: [\"x\"]")); // preserved
    }

    #[test]
    fn set_status_inserts_frontmatter_key_when_absent() {
        let raw = "---\ntags: [\"x\"]\n---\n\n# T\n";
        let out = set_status(raw, Status::Active);
        assert_eq!(parser::parse("t", &out).status, Some(Status::Active));
    }

    #[test]
    fn set_tags_prepends_frontmatter_to_blockquote_note() {
        let raw = "> **Status:** Concept\n\n# T\n";
        let out = set_tags(raw, &["ui".into(), "spatial".into()]);
        let doc = parser::parse("t", &out);
        assert_eq!(doc.tags, vec!["ui", "spatial"]);
        assert_eq!(doc.status, Some(Status::Concept)); // header preserved
    }

    #[test]
    fn set_tags_updates_existing_frontmatter_key() {
        let raw = "---\nstatus: active\ntags: [\"old\"]\n---\n\n# T\n";
        let out = set_tags(raw, &["new".into()]);
        let doc = parser::parse("t", &out);
        assert_eq!(doc.tags, vec!["new"]);
        assert_eq!(doc.status, Some(Status::Active)); // other keys preserved
    }

    #[test]
    fn set_tags_empty_drops_the_key() {
        let raw = "---\ntags: [\"a\"]\n---\n\n# T\n";
        let out = set_tags(raw, &[]);
        assert!(parser::parse("t", &out).tags.is_empty());
    }

    #[test]
    fn link_mention_wraps_first_clean_occurrence() {
        let raw = "I like the Spatial Canvas a lot. Spatial Canvas rocks.";
        let out = link_mention(raw, "Spatial Canvas", "spatial-canvas.md").unwrap();
        // first occurrence linked, original casing preserved
        assert!(out.starts_with("I like the [Spatial Canvas](spatial-canvas.md) a lot."));
        // only the first is linked
        assert_eq!(out.matches("](spatial-canvas.md)").count(), 1);
        // and it round-trips: the parser extracts a link to the target
        assert!(parser::parse("t", &out)
            .link_targets
            .iter()
            .any(|t| t == "spatial-canvas.md"));
    }

    #[test]
    fn link_mention_is_case_insensitive_and_whole_word() {
        // case-insensitive match…
        assert!(link_mention("about spatial canvas here", "Spatial Canvas", "x.md").is_some());
        // …but not a partial-word match
        assert!(link_mention("Spatialise the canvas", "Spatial", "x.md").is_none());
    }

    #[test]
    fn link_mention_skips_code_and_already_linked() {
        // inside a fenced code block → no link
        let code = "```\nSpatial Canvas\n```\n";
        assert!(link_mention(code, "Spatial Canvas", "x.md").is_none());
        // already a link → skipped
        let linked = "[Spatial Canvas](spatial-canvas.md)";
        assert!(link_mention(linked, "Spatial Canvas", "x.md").is_none());
    }

    #[test]
    fn link_mention_absent_phrase_is_none() {
        assert!(link_mention("nothing to see", "Spatial Canvas", "x.md").is_none());
    }
}
