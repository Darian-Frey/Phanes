> **Status:** Active
> **Provenance:** Shane Hartley (owner), Claude (logging)
> **Last reviewed:** 2026-06-12
> **Why this status:** Live catalogue of bugs found during development.

# Bugs

Catalogue of bugs discovered during development. Per Maintenance Rule 8, bugs are
logged here when found, not silently fixed; Shane decides whether to fix
immediately, defer, or leave alone. Backward-looking incident log; the dual of
[IMPROVEMENTS.md](IMPROVEMENTS.md). Added once friction warranted it (D-009).

Status vocabulary: open | fixed | wontfix | deferred.
Severity vocabulary: low | medium | high.

## Open

(none)

## Fixed

### BUG-003: A deterministic re-index wiped a note's model-proposed data
**Status:** fixed (2026-06-15)
**Found:** 2026-06-15 (changing a note's status from the UI emptied its info panel —
summary/tags/topics gone — and disconnected its node in the graph)
**Location:** [src/indexer.rs](src/indexer.rs) `run`, [src/store.rs](src/store.rs) `upsert`
**Severity:** high (data — silent loss of all enrichment for any edited note)
**Description.** A re-index without `--enrich` rebuilt a note from asserted facts
only (no summary, asserted tags, no topics); `upsert` then replaced the row and
deleted/re-inserted the tag/topic child rows, destroying the model-proposed
summary, proposed tags, and topics. The indexer also `clear_embedding`'d on any
content change, dropping the vector — so the note lost its semantic + shared-tag
edges and fell out of the graph. Because this corpus stores status in the
blockquote header (part of `body`), **every status change** triggered it; so did
Save, accept-mention, the file-watcher, and a plain `phanes index` over edited
notes.
**Reproduction.** Enrich a note, then edit it (e.g. change its status) and
re-index without `--enrich`: its proposed summary/tags/topics and `near` results
vanish.
**Notes.** Fixed by `indexer::preserve_proposed` — on a deterministic pass the
existing proposed summary/tags/topics are carried forward into the freshly-parsed
record (gap-fill only; asserted still wins, INV-2) — and by no longer clearing the
embedding. Model-proposed data now persists until an `--enrich`/`--force` pass
refreshes it. Verified live: a status edit + plain re-index preserved the summary,
all proposed tags, and all `near` results.

### BUG-002: Notes silently lost / never got embeddings (no graph connections)
**Status:** fixed (2026-06-13)
**Found:** 2026-06-13 (two Ananke notes showed disconnected in the graph, with
"Near: none"; 6 of 29 notes had no embedding)
**Location:** [src/store.rs](src/store.rs) `upsert`, [src/indexer.rs](src/indexer.rs) gate
**Severity:** medium (data — affected notes vanished from semantic search, the
graph, and bridges; no crash)
**Description.** Two causes compounded. (1) `upsert` used `INSERT OR REPLACE`,
whose delete step cascade-dropped a note's embedding on every re-index — so a
plain Scan of an edited note silently removed its vector. (2) Enrichment and
embedding ran *inside* the hash gate, so an already-indexed note (unchanged hash)
was skipped wholesale — never enriched/embedded, and `Scan + AI` couldn't fill
it either.
**Reproduction.** Embed a corpus, then add/edit a note and run a plain scan (or
`Scan`, then `Scan + AI`): the note ends with no embedding; `near` and the graph
show no connections for it.
**Notes.** Fixed by: (a) `upsert` now updates in place (`ON CONFLICT(id) DO
UPDATE`), preserving embeddings; the indexer clears a stale vector explicitly
only when content actually changed; (b) enrichment/embedding moved to **gap-fill
passes** that run over every current note and fill any missing layer, not just
hash-changed ones. Added `store::{has_summary, has_embedding, clear_embedding}`.
Restored 29/29 embeddings; the two Ananke notes now relate at 93%.

### BUG-001: Wikilink extraction matched TOML/code as links
**Status:** fixed (2026-06-11, same session as parser::parse — step 2)
**Found:** 2026-06-11 (testing `parser::parse` against the real corpus)
**Location:** [src/parser.rs](src/parser.rs) `extract_wikilinks`
**Severity:** low (spurious relationships, not a crash)
**Description.** `extract_wikilinks` was a raw `[[...]]` byte scan that didn't
respect code. Notes embedding TOML table-arrays (`[[shaft]]`, `[[wheel]]`) in
fenced blocks, or `` `[[period]]` `` in inline code, produced bogus link rows —
5 spurious links on the real 28-note corpus.
**Reproduction.** Index a corpus with `[[x]]` inside a fenced code block or an
inline code span; the `links` table gains dangling `dst_id`s that aren't real
wikilinks.
**Notes.** Fixed by skipping code spans and fenced blocks via pulldown-cmark's
offset iterator (`code_ranges`). Dropped spurious links 5 → 0. Originally flagged
in CHANGELOG as "to be backfilled as BUG-001 when BUGS.md is added."

## Won't Fix

(none)

## Deferred

(none)
