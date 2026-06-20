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

### BUG-005: Box-drawing glyphs rendered as □ in the Dark/Light themes
**Status:** fixed (2026-06-16)
**Found:** 2026-06-16 (a note's file-tree block — `├ └ │ ─` — showed as empty
boxes in Dark and Light, but rendered fine in Parchment and Cyberpunk)
**Location:** [src/bin/phanes-ui.rs](src/bin/phanes-ui.rs) `apply_theme`
**Severity:** low (cosmetic — affected glyph coverage, not content)
**Description.** Dark/Light use egui's default proportional font (Ubuntu-Light),
which lacks U+2500 box-drawing glyphs, so file-tree art in rendered markdown showed
as □. Parchment (bundled DejaVu Serif) and Cyberpunk (egui's monospace) both
include those glyphs, so only the two default-font themes were affected.
**Notes.** Fixed by registering the bundled DejaVu Serif as a **fallback** font
(last in both the Proportional and Monospace families) in every theme, so glyphs
the primary font lacks fall through to it. Parchment still uses it as the primary
proportional face.

### BUG-004: AI features didn't work in the AppImage (bundled OpenSSL)
**Status:** fixed (2026-06-16)
**Found:** 2026-06-16 (the compiled AppImage couldn't reach the local LM Studio
server; the same build worked under `cargo run`)
**Location:** [Cargo.toml](Cargo.toml) `reqwest` dependency
**Severity:** medium (all AI features — Scan + AI, Ask, bridges, questions — dead
in the distributed AppImage; the deterministic features were unaffected)
**Description.** `reqwest`'s default `native-tls` backend links the system
OpenSSL (`libssl`/`libcrypto`). `linuxdeploy` bundled those into the AppImage, but
the bundled OpenSSL fails to initialise inside the AppImage's isolated library
environment, so building the blocking HTTP client (or its first request) errored —
and every model call failed silently (graceful degradation, INV-4). It worked
under `cargo run` because that uses the host's OpenSSL. We only ever talk plain
HTTP to a localhost server, so TLS is never actually exercised.
**Reproduction.** Build the AppImage, run it, and trigger Scan + AI / Ask with LM
Studio running: no model calls land, despite the server being up.
**Notes.** Fixed by switching `reqwest` to the pure-Rust `rustls-tls` backend
(`default-features = false`, features `json`/`blocking`/`rustls-tls`) — no OpenSSL
linked or bundled. Verified: `ldd` shows no `libssl`/`libcrypto`, the AppImage
bundles none, and a live `ask`/`questions` call still reaches the model.

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
