//! SQLite persistence. The enrichment model never runs here — this layer only
//! reads and writes already-resolved [`Idea`] records, so every query is
//! instant and offline.

use anyhow::Result;
use rusqlite::Connection;
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
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    /// Stored hash for a path, if indexed. Used to skip unchanged files so
    /// enrichment never re-runs needlessly.
    pub fn hash_for_path(&self, _path: &str) -> Result<Option<String>> {
        // SELECT content_hash FROM ideas WHERE path = ?1
        todo!()
    }

    /// Insert or replace one idea and its tags/topics/links atomically.
    /// Wrap in a transaction; replace child rows wholesale (delete-then-insert)
    /// and re-sync the FTS row.
    pub fn upsert(&mut self, _idea: &Idea) -> Result<()> {
        // INSERT OR REPLACE INTO ideas (...) VALUES (...);
        // DELETE FROM tags   WHERE idea_id = ?; then INSERT each (tag, source);
        // DELETE FROM topics WHERE idea_id = ?; then INSERT each topic;
        // DELETE FROM links  WHERE src_id  = ?; then INSERT each dst_id;
        // DELETE FROM ideas_fts WHERE id = ?; then INSERT (id,title,summary,body).
        todo!()
    }

    /// Remove ideas whose source file no longer exists (called after a walk).
    pub fn prune_missing(&mut self, _seen_ids: &[String]) -> Result<usize> {
        todo!()
    }
}
