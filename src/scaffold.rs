//! Generating new idea notes — the inverse of [`crate::parser`]. Produces a
//! scaffold-standard note (four-field blockquote header, one idea per file) that
//! `parser::parse` reads straight back. Asserted `--tag` values, when given, go
//! in a small YAML frontmatter block, since that is the channel the parser reads
//! asserted tags from.

use chrono::NaiveDate;

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
}
