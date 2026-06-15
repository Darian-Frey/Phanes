-- Phanes index schema.
--
-- Everything queried from here is cached, deterministic, and offline. The LLM
-- enrichment runs only at index time; `content_hash` is its cache key, so the
-- model re-runs only when a file actually changes.

PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS ideas (
    id             TEXT PRIMARY KEY,   -- stable slug from the relative path
    path           TEXT NOT NULL UNIQUE,
    title          TEXT NOT NULL,      -- asserted
    status         TEXT NOT NULL,      -- active|dormant|complete|archived|superseded|unknown
    status_source  TEXT NOT NULL,      -- asserted|proposed
    summary        TEXT,               -- usually proposed
    summary_source TEXT,               -- asserted|proposed|NULL
    category       TEXT,               -- proposed classification of the note's kind (F-023)
    category_source TEXT,              -- asserted|proposed|NULL
    last_reviewed  TEXT,               -- ISO date, asserted, nullable
    mtime          TEXT NOT NULL,      -- ISO datetime; staleness fallback
    content_hash   TEXT NOT NULL,      -- blake3 hex; enrichment cache key
    body           TEXT NOT NULL
);

-- Tags carry provenance so a proposed tag never masquerades as one the author
-- actually wrote.
CREATE TABLE IF NOT EXISTS tags (
    idea_id TEXT NOT NULL REFERENCES ideas(id) ON DELETE CASCADE,
    tag     TEXT NOT NULL,
    source  TEXT NOT NULL,             -- asserted|proposed
    PRIMARY KEY (idea_id, tag)
);
CREATE INDEX IF NOT EXISTS idx_tags_tag ON tags(tag);

-- Proposed concept labels, kept separate from asserted tags.
CREATE TABLE IF NOT EXISTS topics (
    idea_id TEXT NOT NULL REFERENCES ideas(id) ON DELETE CASCADE,
    topic   TEXT NOT NULL,
    PRIMARY KEY (idea_id, topic)
);

-- Explicit out-links only. Shared-tag relationships are computed at query time
-- (a JOIN on tags) and never stored, so they can never go stale.
CREATE TABLE IF NOT EXISTS links (
    src_id TEXT NOT NULL REFERENCES ideas(id) ON DELETE CASCADE,
    dst_id TEXT NOT NULL,              -- may dangle until the target is indexed
    PRIMARY KEY (src_id, dst_id)
);

-- Full-text search surface. Porter stemming over the human-meaningful fields.
CREATE VIRTUAL TABLE IF NOT EXISTS ideas_fts USING fts5 (
    id UNINDEXED,
    title,
    summary,
    body,
    tokenize = 'porter unicode61'
);

-- Per-note embedding vectors for semantic "near this" (F-012). Computed at index
-- time by an embedding model (a second enrichment spoke), so the model never runs
-- on a query. Similarity is plain cosine over these vectors at query time, so the
-- neighbours are computed, not stored (INV-3). The cascade clears a note's vector
-- when it is re-indexed (content changed) or pruned.
CREATE TABLE IF NOT EXISTS embeddings (
    idea_id TEXT PRIMARY KEY REFERENCES ideas(id) ON DELETE CASCADE,
    dim     INTEGER NOT NULL,
    vector  BLOB NOT NULL              -- `dim` little-endian f32 values
);
