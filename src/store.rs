//! SQLite persistence. The enrichment model never runs here — this layer only
//! reads and writes already-resolved [`Idea`] records, so every query is
//! instant and offline.

use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

use crate::model::Idea;

pub const SCHEMA: &str = include_str!("../sql/schema.sql");

pub struct Store {
    pub conn: Connection,
}

impl Store {
    /// Open (creating if absent) the index db and ensure the schema exists.
    /// DB lives at `<root>/.phanes/index.db`.
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        // WAL (set in the schema) plus a busy timeout lets a second connection —
        // e.g. the UI's background "Scan + AI" worker — read/write concurrently
        // without immediate "database is locked" errors.
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch(SCHEMA)?;
        // Lightweight migration for columns added after the initial schema:
        // `CREATE TABLE IF NOT EXISTS` won't touch an existing table, so add them
        // here. ADD COLUMN errors with "duplicate column" once present — ignore it.
        let _ = conn.execute("ALTER TABLE ideas ADD COLUMN category TEXT", []);
        let _ = conn.execute("ALTER TABLE ideas ADD COLUMN category_source TEXT", []);
        Ok(Self { conn })
    }

    /// Stored hash for a path, if indexed. Used to skip unchanged files so
    /// enrichment never re-runs needlessly.
    pub fn hash_for_path(&self, path: &str) -> Result<Option<String>> {
        let hash = self
            .conn
            .query_row(
                "SELECT content_hash FROM ideas WHERE path = ?1",
                params![path],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(hash)
    }

    /// Insert or replace one idea and its tags/topics/links atomically.
    /// Wrap in a transaction; replace child rows wholesale (delete-then-insert)
    /// and re-sync the FTS row.
    ///
    /// Provenance is written alongside every value that can carry it
    /// (`status_source`, `summary_source`, per-tag `source`) so the
    /// proposed-vs-asserted boundary survives a round trip through SQLite.
    pub fn upsert(&mut self, idea: &Idea) -> Result<()> {
        let path = idea.path.to_string_lossy().into_owned();
        let mtime = idea.mtime.to_rfc3339();
        let last_reviewed = idea.last_reviewed.map(|d| d.to_string());
        let (summary, summary_source) = match &idea.summary {
            Some(s) => (Some(s.value.clone()), Some(s.source.as_str())),
            None => (None, None),
        };
        let (category, category_source) = match &idea.category {
            Some(c) => (Some(c.value.clone()), Some(c.source.as_str())),
            None => (None, None),
        };

        let tx = self.conn.transaction()?;

        // Upsert in place (ON CONFLICT … DO UPDATE) rather than INSERT OR REPLACE:
        // REPLACE deletes the old row first, which cascade-drops the note's
        // embedding. Updating in place preserves it, so re-indexing a note (e.g.
        // to add enrichment) doesn't lose its vector. Stale embeddings on a real
        // content change are cleared explicitly by the indexer instead.
        tx.execute(
            "INSERT INTO ideas
                (id, path, title, status, status_source,
                 summary, summary_source, category, category_source,
                 last_reviewed, mtime, content_hash, body)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT(id) DO UPDATE SET
                path = excluded.path, title = excluded.title,
                status = excluded.status, status_source = excluded.status_source,
                summary = excluded.summary, summary_source = excluded.summary_source,
                category = excluded.category, category_source = excluded.category_source,
                last_reviewed = excluded.last_reviewed, mtime = excluded.mtime,
                content_hash = excluded.content_hash, body = excluded.body",
            params![
                idea.id,
                path,
                idea.title,
                idea.status.value.as_str(),
                idea.status.source.as_str(),
                summary,
                summary_source,
                category,
                category_source,
                last_reviewed,
                mtime,
                idea.content_hash,
                idea.body,
            ],
        )?;

        // Replace child rows wholesale (the upsert above updates in place and no
        // longer cascades, so these explicit deletes are what refresh them).
        // OR IGNORE on insert absorbs duplicates within a single document
        // (e.g. the same wikilink or tag written twice).
        tx.execute("DELETE FROM tags   WHERE idea_id = ?1", params![idea.id])?;
        tx.execute("DELETE FROM topics WHERE idea_id = ?1", params![idea.id])?;
        tx.execute("DELETE FROM links  WHERE src_id  = ?1", params![idea.id])?;

        {
            let mut stmt = tx
                .prepare("INSERT OR IGNORE INTO tags (idea_id, tag, source) VALUES (?1, ?2, ?3)")?;
            for t in &idea.tags {
                stmt.execute(params![idea.id, t.value, t.source.as_str()])?;
            }
        }
        {
            let mut stmt =
                tx.prepare("INSERT OR IGNORE INTO topics (idea_id, topic) VALUES (?1, ?2)")?;
            for topic in &idea.topics {
                stmt.execute(params![idea.id, topic])?;
            }
        }
        {
            let mut stmt =
                tx.prepare("INSERT OR IGNORE INTO links (src_id, dst_id) VALUES (?1, ?2)")?;
            for dst in &idea.links {
                stmt.execute(params![idea.id, dst])?;
            }
        }

        // ideas_fts is a virtual table with no foreign key, so the cascade does
        // not reach it — sync it by hand.
        tx.execute("DELETE FROM ideas_fts WHERE id = ?1", params![idea.id])?;
        tx.execute(
            "INSERT INTO ideas_fts (id, title, summary, body) VALUES (?1, ?2, ?3, ?4)",
            params![idea.id, idea.title, summary, idea.body],
        )?;

        tx.commit()?;
        Ok(())
    }

    /// Remove ideas whose source file no longer exists (called after a walk).
    /// `seen_ids` are the ids encountered this pass; anything else is stale and
    /// is deleted. The foreign-key cascade clears tags/topics/links; the FTS
    /// virtual table is cleaned manually. Returns the number of ideas removed.
    pub fn prune_missing(&mut self, seen_ids: &[String]) -> Result<usize> {
        let tx = self.conn.transaction()?;

        // Stage the surviving ids in a temp table so the NOT IN check scales and
        // we avoid building a giant SQL string.
        tx.execute("CREATE TEMP TABLE IF NOT EXISTS _seen (id TEXT PRIMARY KEY)", [])?;
        tx.execute("DELETE FROM _seen", [])?;
        {
            let mut stmt = tx.prepare("INSERT OR IGNORE INTO _seen (id) VALUES (?1)")?;
            for id in seen_ids {
                stmt.execute(params![id])?;
            }
        }

        // Delete ideas first (cascade reaches tags/topics/links), then the FTS
        // rows for those same ids.
        let pruned = tx.execute("DELETE FROM ideas WHERE id NOT IN (SELECT id FROM _seen)", [])?;
        tx.execute("DELETE FROM ideas_fts WHERE id NOT IN (SELECT id FROM _seen)", [])?;

        tx.commit()?;
        Ok(pruned)
    }

    /// Store (or replace) a note's embedding vector, encoded as little-endian
    /// f32s. Called at index time only (the model runs there). A re-indexed
    /// idea's old vector is cleared by the `INSERT OR REPLACE` cascade on `ideas`,
    /// so this writes the fresh one after `upsert`.
    pub fn set_embedding(&self, idea_id: &str, vector: &[f32]) -> Result<()> {
        let bytes: Vec<u8> = vector.iter().flat_map(|f| f.to_le_bytes()).collect();
        self.conn.execute(
            "INSERT OR REPLACE INTO embeddings (idea_id, dim, vector) VALUES (?1, ?2, ?3)",
            params![idea_id, vector.len() as i64, bytes],
        )?;
        Ok(())
    }

    /// Replace a note's *asserted* tags (leaving proposed ones intact). Used by
    /// the UI tag editor: removing the old asserted set and re-inserting the new
    /// one with `INSERT OR REPLACE` also promotes an accepted proposed tag (same
    /// `(idea_id, tag)` key) to asserted. Keeps the DB in sync with the file's
    /// frontmatter without a full re-index (which would wipe proposed tags).
    pub fn set_asserted_tags(&mut self, idea_id: &str, tags: &[String]) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "DELETE FROM tags WHERE idea_id = ?1 AND source = 'asserted'",
            params![idea_id],
        )?;
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO tags (idea_id, tag, source) VALUES (?1, ?2, 'asserted')",
            )?;
            for tag in tags {
                stmt.execute(params![idea_id, tag])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Drop a note's embedding (the indexer calls this when content changed, so
    /// the now-stale vector is replaced on the next embed pass).
    pub fn clear_embedding(&self, idea_id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM embeddings WHERE idea_id = ?1", params![idea_id])?;
        Ok(())
    }

    /// Whether a note already has an embedding vector. Drives the embed gap-fill.
    pub fn has_embedding(&self, idea_id: &str) -> Result<bool> {
        let n: i64 = self.conn.query_row(
            "SELECT count(*) FROM embeddings WHERE idea_id = ?1",
            params![idea_id],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    /// Whether a note has a (model-proposed) summary — a proxy for "already
    /// enriched", since enrichment always sets one. Drives the enrich gap-fill.
    pub fn has_summary(&self, idea_id: &str) -> Result<bool> {
        let n: i64 = self.conn.query_row(
            "SELECT count(*) FROM ideas WHERE id = ?1 AND summary IS NOT NULL",
            params![idea_id],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    /// Every stored embedding as `(id, vector)`. The whole set is small enough to
    /// scan in memory for cosine similarity (see [`crate::query::near`]).
    pub fn all_embeddings(&self) -> Result<Vec<(String, Vec<f32>)>> {
        let mut stmt = self.conn.prepare("SELECT idea_id, vector FROM embeddings")?;
        let rows = stmt
            .query_map([], |r| {
                let id: String = r.get(0)?;
                let bytes: Vec<u8> = r.get(1)?;
                Ok((id, decode_vector(&bytes)))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}

/// Decode a little-endian f32 BLOB back into a vector.
fn decode_vector(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Sourced, Status};
    use chrono::{NaiveDate, TimeZone, Utc};
    use std::path::PathBuf;

    fn mem_store() -> Store {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        Store { conn }
    }

    fn sample_idea() -> Idea {
        Idea {
            id: "spatial-canvas".into(),
            path: PathBuf::from("/tmp/ideas/spatial-canvas.md"),
            title: "Spatial Canvas".into(),
            status: Sourced::asserted(Status::Active),
            summary: Some(Sourced::proposed("A pan-and-zoom canvas for ideas.".into())),
            category: None,
            // one asserted tag, one proposed — provenance must survive the round trip.
            tags: vec![
                Sourced::asserted("ui".into()),
                Sourced::proposed("graph".into()),
            ],
            topics: vec!["visualization".into()],
            last_reviewed: NaiveDate::from_ymd_opt(2026, 5, 28),
            mtime: Utc.with_ymd_and_hms(2026, 6, 10, 12, 0, 0).unwrap(),
            content_hash: "hash-v1".into(),
            body: "nodes and edges and links on a canvas".into(),
            // duplicate target on purpose: INSERT OR IGNORE must collapse it.
            links: vec!["llm-idea-graph".into(), "llm-idea-graph".into()],
        }
    }

    fn count(store: &Store, sql: &str) -> i64 {
        store.conn.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn upsert_then_hash_lookup_and_children() {
        let mut store = mem_store();
        store.upsert(&sample_idea()).unwrap();

        assert_eq!(
            store
                .hash_for_path("/tmp/ideas/spatial-canvas.md")
                .unwrap()
                .as_deref(),
            Some("hash-v1")
        );
        assert_eq!(store.hash_for_path("/nope.md").unwrap(), None);

        // provenance preserved per tag
        assert_eq!(
            count(&store, "SELECT count(*) FROM tags WHERE source='asserted'"),
            1
        );
        assert_eq!(
            count(&store, "SELECT count(*) FROM tags WHERE source='proposed'"),
            1
        );
        // duplicate link collapsed to one row
        assert_eq!(count(&store, "SELECT count(*) FROM links"), 1);
        assert_eq!(count(&store, "SELECT count(*) FROM topics"), 1);
        // status provenance column written
        assert_eq!(
            count(&store, "SELECT count(*) FROM ideas WHERE status_source='asserted'"),
            1
        );
        // FTS is searchable
        assert_eq!(
            count(&store, "SELECT count(*) FROM ideas_fts WHERE ideas_fts MATCH 'nodes'"),
            1
        );
    }

    #[test]
    fn reupsert_replaces_children_without_leftovers() {
        let mut store = mem_store();
        store.upsert(&sample_idea()).unwrap();

        let mut changed = sample_idea();
        changed.content_hash = "hash-v2".into();
        changed.tags = vec![Sourced::asserted("ui".into())]; // dropped the proposed tag
        store.upsert(&changed).unwrap();

        // exactly one ideas row (REPLACE, not a second insert)
        assert_eq!(count(&store, "SELECT count(*) FROM ideas"), 1);
        assert_eq!(
            store
                .hash_for_path("/tmp/ideas/spatial-canvas.md")
                .unwrap()
                .as_deref(),
            Some("hash-v2")
        );
        // stale proposed tag is gone, no leftovers
        assert_eq!(count(&store, "SELECT count(*) FROM tags"), 1);
    }

    #[test]
    fn prune_removes_unseen_ideas_and_fts() {
        let mut store = mem_store();
        store.upsert(&sample_idea()).unwrap();

        // nothing seen this pass => the idea is stale and pruned
        let pruned = store.prune_missing(&[]).unwrap();
        assert_eq!(pruned, 1);
        assert_eq!(store.hash_for_path("/tmp/ideas/spatial-canvas.md").unwrap(), None);
        // cascade + manual FTS cleanup leave nothing behind
        assert_eq!(count(&store, "SELECT count(*) FROM tags"), 0);
        assert_eq!(count(&store, "SELECT count(*) FROM links"), 0);
        assert_eq!(count(&store, "SELECT count(*) FROM ideas_fts"), 0);
    }

    #[test]
    fn set_asserted_tags_replaces_asserted_and_promotes_proposed() {
        let mut store = mem_store();
        let mut idea = sample_idea(); // asserted: ui, spatial; proposed: graph
        idea.tags = vec![
            Sourced::asserted("ui".into()),
            Sourced::asserted("spatial".into()),
            Sourced::proposed("graph".into()),
        ];
        store.upsert(&idea).unwrap();

        // Accept "graph" (now asserted) and keep "ui"; drop "spatial".
        store
            .set_asserted_tags("spatial-canvas", &["ui".into(), "graph".into()])
            .unwrap();

        let got = crate::query::get(&store, "spatial-canvas").unwrap().unwrap();
        let asserted: Vec<&str> = got
            .tags
            .iter()
            .filter(|t| t.source == crate::model::Provenance::Asserted)
            .map(|t| t.value.as_str())
            .collect();
        let proposed: Vec<&str> = got
            .tags
            .iter()
            .filter(|t| t.source == crate::model::Provenance::Proposed)
            .map(|t| t.value.as_str())
            .collect();
        assert!(asserted.contains(&"ui") && asserted.contains(&"graph"));
        assert!(!asserted.contains(&"spatial"));
        // "graph" was promoted, so it's no longer a duplicate proposed row
        assert!(proposed.is_empty());
    }

    #[test]
    fn has_embedding_and_has_summary() {
        let mut store = mem_store();
        let mut idea = sample_idea(); // sample has a proposed summary
        idea.summary = None; // start without enrichment
        store.upsert(&idea).unwrap();
        assert!(!store.has_summary("spatial-canvas").unwrap());
        assert!(!store.has_embedding("spatial-canvas").unwrap());

        store.set_embedding("spatial-canvas", &[0.1, 0.2]).unwrap();
        assert!(store.has_embedding("spatial-canvas").unwrap());

        idea.summary = Some(Sourced::proposed("a summary".into()));
        store.upsert(&idea).unwrap();
        assert!(store.has_summary("spatial-canvas").unwrap());
        // upsert now updates in place, so re-indexing preserves the embedding
        assert!(store.has_embedding("spatial-canvas").unwrap());
        // ...until the content actually changes, when the indexer clears it
        store.clear_embedding("spatial-canvas").unwrap();
        assert!(!store.has_embedding("spatial-canvas").unwrap());
    }

    #[test]
    fn prune_keeps_seen_ideas() {
        let mut store = mem_store();
        store.upsert(&sample_idea()).unwrap();

        let pruned = store.prune_missing(&["spatial-canvas".to_string()]).unwrap();
        assert_eq!(pruned, 0);
        assert!(store
            .hash_for_path("/tmp/ideas/spatial-canvas.md")
            .unwrap()
            .is_some());
    }
}
