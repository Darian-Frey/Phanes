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
}
