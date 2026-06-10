//! Read-side operations. All deterministic, all instant. Shared-tag
//! relationships are computed here at query time and never stored, so they
//! cannot go stale.

use anyhow::Result;

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
pub fn search(_store: &Store, _query: &str, _filter: &SearchFilter) -> Result<Vec<Hit>> {
    // SELECT i.id, i.title, i.status, snippet(ideas_fts, ...)
    //   FROM ideas_fts JOIN ideas i ON i.id = ideas_fts.id
    //  WHERE ideas_fts MATCH ?1
    //    [AND i.status = ?]  [AND EXISTS (SELECT 1 FROM tags ...)]  [stale clause]
    //  ORDER BY rank LIMIT ?;
    todo!()
}

/// Ideas not reviewed (or, failing a date, not modified) within `days`.
/// The "what's quietly rotting" view.
pub fn stale(_store: &Store, _days: i64) -> Result<Vec<Hit>> {
    // Prefer last_reviewed; fall back to mtime. Order oldest first.
    //   COALESCE(last_reviewed, date(mtime)) < date('now', ?||' days')
    todo!()
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
