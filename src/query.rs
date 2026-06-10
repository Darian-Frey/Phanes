//! Read-side operations. All deterministic, all instant. Shared-tag
//! relationships are computed here at query time and never stored, so they
//! cannot go stale.

use std::str::FromStr;

use anyhow::Result;
use rusqlite::params_from_iter;
use rusqlite::types::Value;

use crate::model::Status;
use crate::store::Store;

/// One row in any list output.
#[derive(Debug, Clone)]
pub struct Hit {
    pub id: String,
    pub title: String,
    pub status: Status,
    pub snippet: Option<String>,
    /// For `related`: shared-tag count or link weight.
    pub score: Option<i64>,
}

/// Filters that narrow a search.
#[derive(Debug, Clone, Default)]
pub struct SearchFilter {
    pub status: Option<Status>,
    pub tag: Option<String>,
    pub stale_days: Option<i64>,
    pub limit: usize,
}

/// Full-text search with optional metadata filters.
///
/// The query text is rewritten into an FTS5 MATCH expression by [`fts_match`]
/// so ordinary punctuation can't trip the FTS grammar. Filters are appended as
/// optional `AND` clauses; results are ranked by FTS relevance.
pub fn search(store: &Store, query: &str, filter: &SearchFilter) -> Result<Vec<Hit>> {
    let match_expr = fts_match(query);
    if match_expr.is_empty() {
        return Ok(Vec::new());
    }

    let mut sql = String::from(
        "SELECT i.id, i.title, i.status, \
                snippet(ideas_fts, 3, '[', ']', '…', 12) \
           FROM ideas_fts \
           JOIN ideas i ON i.id = ideas_fts.id \
          WHERE ideas_fts MATCH ?",
    );
    let mut params: Vec<Value> = vec![Value::Text(match_expr)];

    if let Some(status) = filter.status {
        sql.push_str(" AND i.status = ?");
        params.push(Value::Text(status.as_str().to_string()));
    }
    if let Some(tag) = &filter.tag {
        sql.push_str(" AND EXISTS (SELECT 1 FROM tags t WHERE t.idea_id = i.id AND t.tag = ?)");
        params.push(Value::Text(tag.clone()));
    }
    if let Some(days) = filter.stale_days {
        sql.push_str(" AND COALESCE(i.last_reviewed, date(i.mtime)) < date('now', ? || ' days')");
        params.push(Value::Text(format!("-{}", days.abs())));
    }

    sql.push_str(" ORDER BY rank LIMIT ?");
    let limit = if filter.limit == 0 { 20 } else { filter.limit };
    params.push(Value::Integer(limit as i64));

    let mut stmt = store.conn.prepare(&sql)?;
    let hits = stmt
        .query_map(params_from_iter(params.iter()), |row| {
            Ok(Hit {
                id: row.get(0)?,
                title: row.get(1)?,
                status: status_from_row(row.get::<_, String>(2)?),
                snippet: row.get::<_, Option<String>>(3)?,
                score: None,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(hits)
}

/// Ideas not reviewed (or, failing a date, not modified) within `days`.
/// The "what's quietly rotting" view. Oldest first.
pub fn stale(store: &Store, days: i64) -> Result<Vec<Hit>> {
    let mut stmt = store.conn.prepare(
        "SELECT id, title, status \
           FROM ideas \
          WHERE COALESCE(last_reviewed, date(mtime)) < date('now', ? || ' days') \
          ORDER BY COALESCE(last_reviewed, date(mtime)) ASC",
    )?;
    let cutoff = format!("-{}", days.abs());
    let hits = stmt
        .query_map([cutoff], |row| {
            Ok(Hit {
                id: row.get(0)?,
                title: row.get(1)?,
                status: status_from_row(row.get::<_, String>(2)?),
                snippet: None,
                score: None,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(hits)
}

/// Rewrite free text into an FTS5 MATCH expression: each whitespace-separated
/// term is double-quoted (escaping embedded quotes) and the terms are implicitly
/// ANDed. Quoting keeps stray punctuation from being read as FTS operators.
/// Returns an empty string for an all-whitespace query.
fn fts_match(raw: &str) -> String {
    raw.split_whitespace()
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The stored status string is always one of the known variants, but fall back
/// to `Unknown` rather than panicking if the DB ever holds something unexpected.
fn status_from_row(s: String) -> Status {
    Status::from_str(&s).unwrap_or(Status::Unknown)
}

/// Related ideas: explicit links first, then shared-tag neighbours ranked by
/// overlap count. The relationship layer that justifies the tool over `rg`.
pub fn related(_store: &Store, _id_or_title: &str) -> Result<Vec<Hit>> {
    // Explicit:  SELECT dst_id FROM links WHERE src_id = ?;
    // Shared tag: SELECT t2.idea_id, COUNT(*) AS shared
    //               FROM tags t1 JOIN tags t2 ON t1.tag = t2.tag
    //              WHERE t1.idea_id = ? AND t2.idea_id <> ?
    //              GROUP BY t2.idea_id ORDER BY shared DESC;
    todo!()
}

/// Resolve a user-supplied id or fuzzy title to a single idea id.
pub fn resolve(_store: &Store, _id_or_title: &str) -> Result<Option<String>> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Idea, Sourced};
    use crate::store::{Store, SCHEMA};
    use chrono::{NaiveDate, Utc};
    use rusqlite::Connection;
    use std::path::PathBuf;

    fn mem_store() -> Store {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        Store { conn }
    }

    /// Minimal idea with controllable status, tags, body, and review date.
    fn idea(id: &str, title: &str, status: Status, tags: &[&str], body: &str, reviewed: NaiveDate) -> Idea {
        Idea {
            id: id.into(),
            path: PathBuf::from(format!("/ideas/{id}.md")),
            title: title.into(),
            status: Sourced::asserted(status),
            summary: None,
            tags: tags.iter().map(|t| Sourced::asserted(t.to_string())).collect(),
            topics: Vec::new(),
            last_reviewed: Some(reviewed),
            mtime: Utc::now(),
            content_hash: format!("h-{id}"),
            body: body.into(),
            links: Vec::new(),
        }
    }

    fn seed() -> Store {
        let mut store = mem_store();
        let today = Utc::now().date_naive();
        let long_ago = NaiveDate::from_ymd_opt(2000, 1, 1).unwrap();
        store
            .upsert(&idea(
                "spatial-canvas",
                "Spatial Canvas",
                Status::Active,
                &["ui", "spatial"],
                "a pan and zoom canvas of nodes and edges",
                today,
            ))
            .unwrap();
        store
            .upsert(&idea(
                "old-synth",
                "Old Synth",
                Status::Concept,
                &["audio"],
                "a rotting synthesizer for chiptune music",
                long_ago,
            ))
            .unwrap();
        store
    }

    fn ids(hits: &[Hit]) -> Vec<&str> {
        hits.iter().map(|h| h.id.as_str()).collect()
    }

    #[test]
    fn search_matches_body_and_returns_snippet() {
        let store = seed();
        let hits = search(&store, "canvas", &SearchFilter::default()).unwrap();
        assert_eq!(ids(&hits), vec!["spatial-canvas"]);
        assert!(hits[0].snippet.as_deref().unwrap().contains("canvas"));
    }

    #[test]
    fn search_terms_are_anded() {
        let store = seed();
        // both terms appear only in the synth note
        assert_eq!(ids(&search(&store, "synth music", &SearchFilter::default()).unwrap()), vec!["old-synth"]);
        // "canvas music" appears together in neither note
        assert!(search(&store, "canvas music", &SearchFilter::default()).unwrap().is_empty());
    }

    #[test]
    fn search_status_and_tag_filters_narrow() {
        let store = seed();
        let by_status = SearchFilter { status: Some(Status::Active), ..Default::default() };
        assert_eq!(ids(&search(&store, "a", &by_status).unwrap()), vec!["spatial-canvas"]);

        let by_tag = SearchFilter { tag: Some("audio".into()), ..Default::default() };
        assert_eq!(ids(&search(&store, "a", &by_tag).unwrap()), vec!["old-synth"]);
    }

    #[test]
    fn search_punctuation_does_not_break_fts() {
        let store = seed();
        // a bare punctuation query would be invalid FTS syntax unquoted; here it
        // simply matches nothing instead of erroring.
        assert!(search(&store, "\"", &SearchFilter::default()).is_ok());
    }

    #[test]
    fn stale_lists_only_old_ideas_oldest_first() {
        let store = seed();
        let hits = stale(&store, 180).unwrap();
        assert_eq!(ids(&hits), vec!["old-synth"]);
        // snippet is absent for stale (drives the table's column choice)
        assert!(hits[0].snippet.is_none());
    }
}
