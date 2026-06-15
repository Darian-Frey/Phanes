//! Read-side operations. All deterministic, all instant. Shared-tag
//! relationships are computed here at query time and never stored, so they
//! cannot go stale.

use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, OptionalExtension};

use crate::model::{Idea, Provenance, Sourced, Status};
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

/// One indexed idea, enough to render and group in the UI explorer.
#[derive(Debug, Clone)]
pub struct ListItem {
    pub id: String,
    pub title: String,
    pub status: Status,
    /// The note's stored (root-prefixed) path; the explorer strips the root to
    /// build the folder tree.
    pub path: String,
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

/// Hybrid search (F-021): the full-text results, augmented for recall with notes
/// **semantically near the top keyword matches** (cosine over the index-time
/// embeddings), the two rankings fused via reciprocal-rank fusion. Fully
/// deterministic and offline — the query is *never* embedded, so no model runs on
/// the path (INV-1 holds). Falls back to plain FTS when there are no embeddings,
/// no keyword hits to seed from, or a metadata filter is set (expansion would
/// bypass it).
pub fn hybrid(store: &Store, query: &str, filter: &SearchFilter) -> Result<Vec<Hit>> {
    let fts = search(store, query, filter)?;
    let filtered = filter.status.is_some() || filter.tag.is_some() || filter.stale_days.is_some();
    if fts.is_empty() || filtered {
        return Ok(fts);
    }
    let embeddings = store.all_embeddings()?;
    if embeddings.is_empty() {
        return Ok(fts);
    }
    let vmap: HashMap<&str, &Vec<f32>> =
        embeddings.iter().map(|(id, v)| (id.as_str(), v)).collect();

    // Seeds: the strongest few keyword matches that have a vector.
    let seeds: Vec<&Vec<f32>> =
        fts.iter().take(3).filter_map(|h| vmap.get(h.id.as_str()).copied()).collect();
    if seeds.is_empty() {
        return Ok(fts);
    }
    // Affinity = max cosine to any seed; keep the clearly-related notes.
    let mut affinity: Vec<(&str, f32)> = embeddings
        .iter()
        .map(|(id, v)| (id.as_str(), seeds.iter().map(|s| cosine(s, v)).fold(0.0, f32::max)))
        .filter(|&(_, a)| a >= 0.6)
        .collect();
    affinity.sort_by(|x, y| y.1.total_cmp(&x.1));

    // Fuse the two rankings (FTS relevance + semantic affinity).
    let fts_ids: Vec<&str> = fts.iter().map(|h| h.id.as_str()).collect();
    let aff_ids: Vec<&str> = affinity.iter().map(|&(id, _)| id).collect();
    let fused = rrf(&[&fts_ids, &aff_ids]);

    // Hydrate: reuse the FTS hit (keeps its snippet) where present, else fetch.
    let fts_by_id: HashMap<&str, &Hit> = fts.iter().map(|h| (h.id.as_str(), h)).collect();
    let limit = if filter.limit == 0 { 20 } else { filter.limit };
    let mut out = Vec::new();
    for id in fused.iter().take(limit) {
        if let Some(h) = fts_by_id.get(id.as_str()) {
            out.push((*h).clone());
        } else if let Some(h) = hydrate_hit(store, id)? {
            out.push(h);
        }
    }
    Ok(out)
}

/// Reciprocal-rank fusion of several ranked id lists: `score(id) = Σ 1/(60 + rank)`
/// across the lists it appears in, highest first; ties broken by id for
/// determinism. The standard `k = 60` damps the contribution of low ranks.
fn rrf(lists: &[&[&str]]) -> Vec<String> {
    let mut score: HashMap<&str, f32> = HashMap::new();
    for list in lists {
        for (rank, &id) in list.iter().enumerate() {
            *score.entry(id).or_insert(0.0) += 1.0 / (60.0 + rank as f32 + 1.0);
        }
    }
    let mut ranked: Vec<(&str, f32)> = score.into_iter().collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(b.0)));
    ranked.into_iter().map(|(id, _)| id.to_string()).collect()
}

/// Title + status for one id, as a `Hit` tagged as a semantic (non-keyword)
/// match. `None` if the id no longer exists.
fn hydrate_hit(store: &Store, id: &str) -> Result<Option<Hit>> {
    let row = store
        .conn
        .query_row(
            "SELECT title, status FROM ideas WHERE id = ?1",
            params![id],
            |r| Ok((r.get::<_, String>(0)?, status_from_row(r.get::<_, String>(1)?))),
        )
        .optional()?;
    Ok(row.map(|(title, status)| Hit {
        id: id.to_string(),
        title,
        status,
        snippet: Some("≈ related".to_string()),
        score: None,
    }))
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

/// Every indexed idea, ordered by path. Powers the UI explorer's folder tree.
pub fn list(store: &Store) -> Result<Vec<ListItem>> {
    let mut stmt = store
        .conn
        .prepare("SELECT id, title, status, path FROM ideas ORDER BY path")?;
    let items = stmt
        .query_map([], |r| {
            Ok(ListItem {
                id: r.get(0)?,
                title: r.get(1)?,
                status: status_from_row(r.get::<_, String>(2)?),
                path: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(items)
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

/// Load one fully-resolved idea, with provenance, tags, topics, and links.
/// `None` if no idea has that id. This is the read used by `show` and (later)
/// the UI's info panel.
pub fn get(store: &Store, id: &str) -> Result<Option<Idea>> {
    let row = store
        .conn
        .query_row(
            "SELECT path, title, status, status_source, summary, summary_source, \
                    last_reviewed, mtime, content_hash, body \
               FROM ideas WHERE id = ?1",
            params![id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,         // path
                    r.get::<_, String>(1)?,         // title
                    r.get::<_, String>(2)?,         // status
                    r.get::<_, String>(3)?,         // status_source
                    r.get::<_, Option<String>>(4)?, // summary
                    r.get::<_, Option<String>>(5)?, // summary_source
                    r.get::<_, Option<String>>(6)?, // last_reviewed
                    r.get::<_, String>(7)?,         // mtime
                    r.get::<_, String>(8)?,         // content_hash
                    r.get::<_, String>(9)?,         // body
                ))
            },
        )
        .optional()?;

    let Some((path, title, status, status_src, summary, summary_src, last_reviewed, mtime, content_hash, body)) =
        row
    else {
        return Ok(None);
    };

    let tags = collect(store, "SELECT tag, source FROM tags WHERE idea_id = ?1 ORDER BY source, tag", id, |r| {
        Ok(Sourced {
            value: r.get::<_, String>(0)?,
            source: Provenance::from_db(&r.get::<_, String>(1)?),
        })
    })?;
    let topics = collect(store, "SELECT topic FROM topics WHERE idea_id = ?1 ORDER BY topic", id, |r| {
        r.get::<_, String>(0)
    })?;
    let links = collect(store, "SELECT dst_id FROM links WHERE src_id = ?1 ORDER BY dst_id", id, |r| {
        r.get::<_, String>(0)
    })?;

    let summary = summary.map(|value| Sourced {
        value,
        source: Provenance::from_db(summary_src.as_deref().unwrap_or("proposed")),
    });

    Ok(Some(Idea {
        id: id.to_string(),
        path: PathBuf::from(path),
        title,
        status: Sourced {
            value: Status::from_str(&status).unwrap_or(Status::Unknown),
            source: Provenance::from_db(&status_src),
        },
        summary,
        tags,
        topics,
        last_reviewed: last_reviewed.and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
        mtime: DateTime::parse_from_rfc3339(&mtime)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        content_hash,
        body,
        links,
    }))
}

/// Run a single-`id`-parameter query and collect the mapped rows.
fn collect<T>(
    store: &Store,
    sql: &str,
    id: &str,
    f: impl Fn(&rusqlite::Row) -> rusqlite::Result<T>,
) -> Result<Vec<T>> {
    let mut stmt = store.conn.prepare(sql)?;
    let rows = stmt
        .query_map(params![id], |r| f(r))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Related ideas: explicit links first, then shared-tag neighbours ranked by
/// overlap count. The relationship layer that justifies the tool over `rg`.
/// Shared-tag neighbours are computed here at query time and never stored
/// (INV-3). Returns an empty list if the idea can't be resolved.
pub fn related(store: &Store, id_or_title: &str) -> Result<Vec<Hit>> {
    let Some(id) = resolve(store, id_or_title)? else {
        return Ok(Vec::new());
    };

    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Explicit out-links, resolved to indexed ideas. A dangling target (not yet
    // indexed) simply doesn't join, so it's silently skipped.
    let mut link_stmt = store.conn.prepare(
        "SELECT i.id, i.title, i.status \
           FROM links l JOIN ideas i ON i.id = l.dst_id \
          WHERE l.src_id = ?1 AND l.dst_id <> l.src_id ORDER BY i.title",
    )?;
    let links = link_stmt
        .query_map(params![id], |r| {
            Ok(Hit {
                id: r.get(0)?,
                title: r.get(1)?,
                status: status_from_row(r.get::<_, String>(2)?),
                snippet: Some("linked".to_string()),
                score: None,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for h in links {
        seen.insert(h.id.clone());
        out.push(h);
    }

    // Shared-tag neighbours ranked by overlap, excluding the idea itself and any
    // neighbour already shown as an explicit link.
    let mut tag_stmt = store.conn.prepare(
        "SELECT i.id, i.title, i.status, COUNT(*) AS shared \
           FROM tags t1 \
           JOIN tags t2 ON t1.tag = t2.tag AND t2.idea_id <> t1.idea_id \
           JOIN ideas i ON i.id = t2.idea_id \
          WHERE t1.idea_id = ?1 \
          GROUP BY t2.idea_id \
          ORDER BY shared DESC, i.title ASC",
    )?;
    let neighbours = tag_stmt
        .query_map(params![id], |r| {
            let shared: i64 = r.get(3)?;
            Ok(Hit {
                id: r.get(0)?,
                title: r.get(1)?,
                status: status_from_row(r.get::<_, String>(2)?),
                snippet: Some(format!("{shared} shared tag{}", if shared == 1 { "" } else { "s" })),
                score: Some(shared),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for h in neighbours {
        if seen.insert(h.id.clone()) {
            out.push(h);
        }
    }

    Ok(out)
}

/// Resolve a user-supplied id or fuzzy title to a single idea id. Tries, in
/// order: exact id, exact title (case-insensitive), then a unique substring
/// match on title or id. Ambiguous or absent → `None`.
pub fn resolve(store: &Store, id_or_title: &str) -> Result<Option<String>> {
    let exact_id: Option<String> = store
        .conn
        .query_row("SELECT id FROM ideas WHERE id = ?1", params![id_or_title], |r| r.get(0))
        .optional()?;
    if exact_id.is_some() {
        return Ok(exact_id);
    }

    let exact_title: Option<String> = store
        .conn
        .query_row(
            "SELECT id FROM ideas WHERE title = ?1 COLLATE NOCASE LIMIT 1",
            params![id_or_title],
            |r| r.get(0),
        )
        .optional()?;
    if exact_title.is_some() {
        return Ok(exact_title);
    }

    // Unique substring match. LIMIT 2 so we can tell unique from ambiguous.
    let pattern = format!("%{id_or_title}%");
    let mut stmt = store.conn.prepare(
        "SELECT id FROM ideas WHERE title LIKE ?1 COLLATE NOCASE OR id LIKE ?1 COLLATE NOCASE LIMIT 2",
    )?;
    let matches = stmt
        .query_map(params![pattern], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if matches.len() == 1 {
        Ok(matches.into_iter().next())
    } else {
        Ok(None) // zero matches, or ambiguous
    }
}

/// Semantically similar ideas by cosine similarity over stored embeddings
/// (F-012). Deterministic — the vectors were computed at index time, so no model
/// runs here (INV-1). Returns the top `limit` ideas most similar to the target,
/// excluding itself; empty if the note has no embedding or can't be resolved.
pub fn near(store: &Store, id_or_title: &str, limit: usize) -> Result<Vec<Hit>> {
    let Some(id) = resolve(store, id_or_title)? else {
        return Ok(Vec::new());
    };

    let embeddings = store.all_embeddings()?;
    let Some(target) = embeddings.iter().find(|(eid, _)| *eid == id).map(|(_, v)| v.clone()) else {
        return Ok(Vec::new());
    };

    let mut scored: Vec<(String, f32)> = embeddings
        .iter()
        .filter(|(eid, _)| *eid != id)
        .map(|(eid, v)| (eid.clone(), cosine(&target, v)))
        .collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    scored.truncate(if limit == 0 { 10 } else { limit });

    // Hydrate the ranked ids into Hits (title + status), preserving order.
    let mut out = Vec::new();
    for (eid, sim) in scored {
        let row = store
            .conn
            .query_row(
                "SELECT title, status FROM ideas WHERE id = ?1",
                params![eid],
                |r| Ok((r.get::<_, String>(0)?, status_from_row(r.get::<_, String>(1)?))),
            )
            .optional()?;
        if let Some((title, status)) = row {
            out.push(Hit {
                id: eid,
                title,
                status,
                snippet: Some(format!("{:.0}% similar", (sim * 100.0).clamp(0.0, 100.0))),
                score: Some((sim * 1000.0) as i64),
            });
        }
    }
    Ok(out)
}

/// One tag and the notes carrying it, split by provenance — the unit of the tag
/// browser (F-018).
#[derive(Debug, Clone)]
pub struct TagGroup {
    pub tag: String,
    pub asserted: usize,
    pub proposed: usize,
    pub notes: Vec<Hit>,
}

/// The whole tag vocabulary: every tag with its asserted/proposed counts and the
/// notes that carry it, sorted by total use (desc) then name. Deterministic — a
/// single grouped read over the `tags` table (INV-3). Powers the `tags` command
/// and the UI tag browser.
pub fn tag_index(store: &Store) -> Result<Vec<TagGroup>> {
    let mut stmt = store.conn.prepare(
        "SELECT t.tag, t.source, i.id, i.title, i.status \
           FROM tags t JOIN ideas i ON i.id = t.idea_id \
          ORDER BY t.tag, i.title",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?, // tag
                r.get::<_, String>(1)?, // source
                r.get::<_, String>(2)?, // id
                r.get::<_, String>(3)?, // title
                r.get::<_, String>(4)?, // status
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    // Rows arrive grouped by tag (ORDER BY). Fold consecutive rows into groups.
    let mut groups: Vec<TagGroup> = Vec::new();
    for (tag, source, id, title, status) in rows {
        if groups.last().map(|g| g.tag.as_str()) != Some(tag.as_str()) {
            groups.push(TagGroup { tag: tag.clone(), asserted: 0, proposed: 0, notes: Vec::new() });
        }
        let g = groups.last_mut().unwrap();
        if Provenance::from_db(&source) == Provenance::Asserted {
            g.asserted += 1;
        } else {
            g.proposed += 1;
        }
        g.notes.push(Hit {
            id,
            title,
            status: status_from_row(status),
            snippet: None,
            score: None,
        });
    }
    groups.sort_by(|a, b| {
        (b.asserted + b.proposed).cmp(&(a.asserted + a.proposed)).then(a.tag.cmp(&b.tag))
    });
    Ok(groups)
}

/// Incoming links: notes that explicitly link **to** this one (the dual of the
/// out-links in [`related`]). Deterministic — a JOIN on the `links` table by
/// `dst_id`, computed at query time (INV-3). Self-links excluded. Empty if the
/// note can't be resolved. The Obsidian-style "Linked mentions" half of F-016.
pub fn backlinks(store: &Store, id_or_title: &str) -> Result<Vec<Hit>> {
    let Some(id) = resolve(store, id_or_title)? else {
        return Ok(Vec::new());
    };
    let mut stmt = store.conn.prepare(
        "SELECT i.id, i.title, i.status \
           FROM links l JOIN ideas i ON i.id = l.src_id \
          WHERE l.dst_id = ?1 AND l.src_id <> l.dst_id \
          ORDER BY i.title",
    )?;
    let hits = stmt
        .query_map(params![id], |r| {
            Ok(Hit {
                id: r.get(0)?,
                title: r.get(1)?,
                status: status_from_row(r.get::<_, String>(2)?),
                snippet: Some("links here".to_string()),
                score: None,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(hits)
}

/// Unlinked mentions: notes whose text contains this note's **title** as a phrase
/// but which don't already link to it — candidate links the user can accept
/// (F-016). Deterministic — an FTS phrase match minus the notes already in the
/// `links` table; no model (INV-1/INV-3). Excludes the note itself. Empty if the
/// note can't be resolved or has a blank/too-short title.
pub fn unlinked_mentions(store: &Store, id_or_title: &str) -> Result<Vec<Hit>> {
    let Some(id) = resolve(store, id_or_title)? else {
        return Ok(Vec::new());
    };
    let title: Option<String> = store
        .conn
        .query_row("SELECT title FROM ideas WHERE id = ?1", params![id], |r| r.get(0))
        .optional()?;
    let Some(title) = title else {
        return Ok(Vec::new());
    };
    let phrase = fts_phrase(&title);
    if phrase.is_empty() {
        return Ok(Vec::new());
    }

    let mut stmt = store.conn.prepare(
        "SELECT i.id, i.title, i.status, \
                snippet(ideas_fts, 3, '[', ']', '…', 10) \
           FROM ideas_fts \
           JOIN ideas i ON i.id = ideas_fts.id \
          WHERE ideas_fts MATCH ?1 \
            AND i.id <> ?2 \
            AND NOT EXISTS (SELECT 1 FROM links l WHERE l.src_id = i.id AND l.dst_id = ?2) \
          ORDER BY rank LIMIT 20",
    )?;
    let hits = stmt
        .query_map(params![phrase, id], |r| {
            Ok(Hit {
                id: r.get(0)?,
                title: r.get(1)?,
                status: status_from_row(r.get::<_, String>(2)?),
                snippet: r.get::<_, Option<String>>(3)?,
                score: None,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(hits)
}

/// Rewrite a title into an FTS5 **phrase** match (the whole title, contiguous),
/// quoting the terms so punctuation can't trip the grammar. Empty for a blank
/// title.
fn fts_phrase(raw: &str) -> String {
    let terms: Vec<String> = raw
        .split_whitespace()
        .map(|t| t.replace('"', ""))
        .filter(|t| !t.is_empty())
        .collect();
    if terms.is_empty() {
        String::new()
    } else {
        format!("\"{}\"", terms.join(" "))
    }
}

/// Cosine similarity of two vectors. Returns 0 on a dimension mismatch or a
/// zero-norm vector.
pub(crate) fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
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

    // --- step 4: get / resolve / related ---

    use crate::model::Provenance;

    fn upsert(store: &mut Store, mut idea: Idea, links: &[&str]) {
        idea.links = links.iter().map(|s| s.to_string()).collect();
        store.upsert(&idea).unwrap();
    }

    fn related_store() -> Store {
        let mut store = mem_store();
        let today = Utc::now().date_naive();
        // alpha links to beta; alpha shares "ui" with beta+delta and "spatial"
        // with gamma+delta.
        upsert(&mut store, idea("alpha", "Alpha", Status::Active, &["ui", "spatial"], "a", today), &["beta"]);
        upsert(&mut store, idea("beta", "Beta", Status::Concept, &["ui"], "b", today), &[]);
        upsert(&mut store, idea("gamma", "Gamma", Status::Concept, &["spatial", "audio"], "c", today), &[]);
        upsert(&mut store, idea("delta", "Delta", Status::Active, &["ui", "spatial"], "d", today), &[]);
        store
    }

    #[test]
    fn resolve_exact_id_title_and_fuzzy() {
        let store = related_store();
        assert_eq!(resolve(&store, "alpha").unwrap().as_deref(), Some("alpha"));
        // exact title, case-insensitive
        assert_eq!(resolve(&store, "BETA").unwrap().as_deref(), Some("beta"));
        // unique substring
        assert_eq!(resolve(&store, "gam").unwrap().as_deref(), Some("gamma"));
        // no match
        assert_eq!(resolve(&store, "nope").unwrap(), None);
    }

    #[test]
    fn resolve_ambiguous_substring_is_none() {
        let mut store = mem_store();
        let today = Utc::now().date_naive();
        upsert(&mut store, idea("note-one", "Note One", Status::Active, &[], "x", today), &[]);
        upsert(&mut store, idea("note-two", "Note Two", Status::Active, &[], "y", today), &[]);
        // "note" matches both → ambiguous → None
        assert_eq!(resolve(&store, "note").unwrap(), None);
    }

    #[test]
    fn related_links_first_then_shared_tags_ranked() {
        let store = related_store();
        let hits = related(&store, "alpha").unwrap();
        // beta appears once (as the explicit link, not duplicated by its shared
        // "ui" tag); then delta (2 shared) ranks above gamma (1 shared).
        assert_eq!(ids(&hits), vec!["beta", "delta", "gamma"]);
        assert_eq!(hits[0].snippet.as_deref(), Some("linked"));
        assert_eq!(hits[1].score, Some(2));
        assert_eq!(hits[2].score, Some(1));
    }

    #[test]
    fn related_unresolvable_is_empty() {
        let store = related_store();
        assert!(related(&store, "does-not-exist").unwrap().is_empty());
    }

    #[test]
    fn related_excludes_self_links() {
        let mut store = mem_store();
        let today = Utc::now().date_naive();
        // a note whose only link points at itself (e.g. a `[[Self]]` wikilink)
        upsert(&mut store, idea("solo", "Solo", Status::Active, &[], "x", today), &["solo"]);
        assert!(related(&store, "solo").unwrap().is_empty());
    }

    #[test]
    fn get_round_trips_provenance() {
        let mut store = mem_store();
        let mut idea = idea("p", "Provenance", Status::Active, &["ui"], "body", Utc::now().date_naive());
        // add a proposed tag and a proposed summary alongside the asserted tag
        idea.tags.push(Sourced::proposed("ai".into()));
        idea.summary = Some(Sourced::proposed("auto summary".into()));
        idea.topics = vec!["viz".into()];
        store.upsert(&idea).unwrap();

        let got = get(&store, "p").unwrap().expect("idea exists");
        assert_eq!(got.status.source, Provenance::Asserted);
        assert_eq!(got.summary.as_ref().unwrap().source, Provenance::Proposed);
        let ui = got.tags.iter().find(|t| t.value == "ui").unwrap();
        let ai = got.tags.iter().find(|t| t.value == "ai").unwrap();
        assert_eq!(ui.source, Provenance::Asserted);
        assert_eq!(ai.source, Provenance::Proposed);
        assert_eq!(got.topics, vec!["viz"]);

        assert!(get(&store, "missing").unwrap().is_none());
    }

    #[test]
    fn list_returns_all_ideas_with_paths() {
        let store = related_store(); // alpha, beta, gamma, delta
        let items = list(&store).unwrap();
        assert_eq!(items.len(), 4);
        let alpha = items.iter().find(|i| i.id == "alpha").unwrap();
        assert_eq!(alpha.title, "Alpha");
        assert_eq!(alpha.status, Status::Active);
        assert_eq!(alpha.path, "/ideas/alpha.md");
    }

    #[test]
    fn near_ranks_by_cosine_similarity() {
        let store = related_store(); // alpha, beta, gamma, delta
        // Hand-place 2-D vectors: alpha points "right"; delta is nearly identical,
        // gamma is at 45°, beta is orthogonal.
        store.set_embedding("alpha", &[1.0, 0.0]).unwrap();
        store.set_embedding("delta", &[0.99, 0.10]).unwrap();
        store.set_embedding("gamma", &[0.70, 0.70]).unwrap();
        store.set_embedding("beta", &[0.0, 1.0]).unwrap();

        let hits = near(&store, "alpha", 10).unwrap();
        // self excluded; ordered most-similar first: delta, gamma, beta
        assert_eq!(ids(&hits), vec!["delta", "gamma", "beta"]);
        // scores are descending
        assert!(hits[0].score.unwrap() >= hits[1].score.unwrap());
        assert!(hits[1].score.unwrap() >= hits[2].score.unwrap());
    }

    #[test]
    fn near_without_embedding_is_empty() {
        let store = related_store(); // no vectors stored
        assert!(near(&store, "alpha", 10).unwrap().is_empty());
    }

    #[test]
    fn backlinks_lists_incoming_links_only() {
        let store = related_store(); // alpha links to beta
        // beta's backlink is alpha (the out-link, reversed)
        assert_eq!(ids(&backlinks(&store, "beta").unwrap()), vec!["alpha"]);
        // alpha has no incoming links
        assert!(backlinks(&store, "alpha").unwrap().is_empty());
    }

    #[test]
    fn backlinks_excludes_self_links() {
        let mut store = mem_store();
        let today = Utc::now().date_naive();
        upsert(&mut store, idea("solo", "Solo", Status::Active, &[], "x", today), &["solo"]);
        assert!(backlinks(&store, "solo").unwrap().is_empty());
    }

    #[test]
    fn unlinked_mentions_finds_phrase_and_excludes_linked_and_self() {
        let mut store = mem_store();
        let today = Utc::now().date_naive();
        // The target note.
        upsert(&mut store, idea("spatial", "Spatial Canvas", Status::Active, &[], "the canvas itself", today), &[]);
        // A note that mentions the title in prose but doesn't link it.
        upsert(&mut store, idea("mentioner", "Mentioner", Status::Concept, &[],
            "I keep coming back to the Spatial Canvas idea.", today), &[]);
        // A note that mentions AND already links it — excluded.
        upsert(&mut store, idea("linker", "Linker", Status::Concept, &[],
            "See the Spatial Canvas for details.", today), &["spatial"]);
        // A note that doesn't mention it at all.
        upsert(&mut store, idea("other", "Other", Status::Concept, &[], "unrelated text", today), &[]);

        let hits = unlinked_mentions(&store, "spatial").unwrap();
        assert_eq!(ids(&hits), vec!["mentioner"]); // not linker (already links), not self, not other
    }

    #[test]
    fn unlinked_mentions_unresolvable_is_empty() {
        let store = related_store();
        assert!(unlinked_mentions(&store, "nope").unwrap().is_empty());
    }

    #[test]
    fn rrf_rewards_appearing_in_both_lists() {
        // "a" tops both lists → highest fused score; b and c each appear once.
        let l1 = ["a", "b"];
        let l2 = ["a", "c"];
        let fused = rrf(&[&l1[..], &l2[..]]);
        assert_eq!(fused[0], "a");
        assert!(fused.contains(&"b".to_string()) && fused.contains(&"c".to_string()));
        assert_eq!(fused.len(), 3);
    }

    #[test]
    fn hybrid_adds_semantic_neighbours_of_keyword_hits() {
        let mut store = mem_store();
        let today = Utc::now().date_naive();
        // "alpha" matches the keyword; "beta" does NOT, but is semantically near
        // alpha; "gamma" is unrelated.
        store.upsert(&idea("alpha", "Alpha", Status::Active, &[], "a spatial canvas of nodes", today)).unwrap();
        store.upsert(&idea("beta", "Beta", Status::Concept, &[], "panning and zooming a board", today)).unwrap();
        store.upsert(&idea("gamma", "Gamma", Status::Concept, &[], "a rotting synth", today)).unwrap();
        store.set_embedding("alpha", &[1.0, 0.0]).unwrap();
        store.set_embedding("beta", &[0.97, 0.20]).unwrap(); // near alpha
        store.set_embedding("gamma", &[0.0, 1.0]).unwrap(); // far

        // Plain FTS finds only the keyword match.
        let plain = search(&store, "canvas", &SearchFilter::default()).unwrap();
        assert_eq!(ids(&plain), vec!["alpha"]);

        // Hybrid pulls in beta (semantic neighbour) but not gamma (unrelated).
        let hits = hybrid(&store, "canvas", &SearchFilter::default()).unwrap();
        let got = ids(&hits);
        assert!(got.contains(&"alpha"));
        assert!(got.contains(&"beta"));
        assert!(!got.contains(&"gamma"));
    }

    #[test]
    fn hybrid_without_embeddings_is_plain_fts() {
        let store = seed(); // no vectors
        let hits = hybrid(&store, "canvas", &SearchFilter::default()).unwrap();
        assert_eq!(ids(&hits), vec!["spatial-canvas"]);
    }

    #[test]
    fn tag_index_counts_provenance_and_sorts_by_use() {
        let mut store = mem_store();
        let mut a = idea("a", "A", Status::Active, &["ui", "spatial"], "x", Utc::now().date_naive());
        a.tags.push(Sourced::proposed("ml".into()));
        store.upsert(&a).unwrap();
        // b also asserts "ui"; c proposes "ui"
        store.upsert(&idea("b", "B", Status::Concept, &["ui"], "y", Utc::now().date_naive())).unwrap();
        let mut c = idea("c", "C", Status::Concept, &[], "z", Utc::now().date_naive());
        c.tags.push(Sourced::proposed("ui".into()));
        store.upsert(&c).unwrap();

        let groups = tag_index(&store).unwrap();
        // "ui" is the most-used tag → first; 2 asserted (a,b) + 1 proposed (c)
        assert_eq!(groups[0].tag, "ui");
        assert_eq!(groups[0].asserted, 2);
        assert_eq!(groups[0].proposed, 1);
        assert_eq!(groups[0].notes.len(), 3);
        // "ml" is proposed-only
        let ml = groups.iter().find(|g| g.tag == "ml").unwrap();
        assert_eq!((ml.asserted, ml.proposed), (0, 1));
    }
}
