> **Status:** Active
> **Provenance:** Shane Hartley (owner/architect), Claude (drafting)
> **Last reviewed:** 2026-06-10
> **Why this status:** Capability set defined; P1 features shipped, P2–P4 pending.

# Features

The **what**, not the how. See [ARCHITECTURE.md](ARCHITECTURE.md) for structure
and [DECISIONS.md](DECISIONS.md) for rationale. Phases live in
[ROADMAP.md](ROADMAP.md).

## Target users

Shane, and anyone who keeps a folder of project-idea markdown notes following the
scaffold convention (four-field blockquote header, one idea per file) and wants
durable, offline search and relationship surfacing over them — deliberately more
than `rg`, without a cloud service.

## Out of scope

- Splitting one file into multiple ideas — one file = one idea (D-006).
- Acting as a general note-taking app beyond capture (`new`) and edit-in-place.
- Non-markdown source formats; cloud sync; multi-user collaboration.
- Making the local model load-bearing for anything deterministic code can do
  (titles, links, dates, status) — the hub-and-spoke methodology forbids it.

## Features

### F-001 Deterministic indexing
**Priority:** Must
**Acceptance:**
- `phanes index --root <dir>` walks `*.md`, hashes each file (blake3), and skips
  unchanged files on a content-hash match (or reprocesses all under `--force`).
- Frontmatter **and** blockquote-header metadata (status, last_reviewed, tags)
  are parsed as *asserted* facts (D-008).
- Idea records are upserted to SQLite with provenance columns; files that
  vanished since the last pass are pruned.
**Status:** Complete (Phase 1)
**Notes:** The hash gate is INV-1. See ARCHITECTURE.md §indexer. Related: D-001, D-004, D-008.

### F-002 Full-text search with filters
**Priority:** Must
**Acceptance:**
- `phanes search <query>` returns FTS5-ranked hits with highlighted snippets.
- `--status`, `--tag`, `--stale-days`, `--limit` narrow results.
- Multi-word queries AND their terms; arbitrary punctuation never errors.
**Status:** Complete (Phase 1)
**Notes:** Porter stemming matches word variants. Related: D-004.

### F-003 Stale view
**Priority:** Must
**Acceptance:**
- `phanes stale --days N` lists ideas whose `last_reviewed` (or `mtime` fallback)
  is older than N days, oldest first.
**Status:** Complete (Phase 1)

### F-004 Provenance model (asserted vs proposed)
**Priority:** Must
**Acceptance:**
- Every field that can originate from the model carries `Asserted | Proposed`.
- Proposed values fill gaps only and never overwrite asserted ones.
- Provenance is persisted in the DB and surfaced in `show`.
**Status:** Complete (Phase 2) — surfaced in `show` with per-field asserted/proposed flags
**Notes:** INV-2. Related: D-002.

### F-005 Related ideas
**Priority:** Must
**Acceptance:**
- `phanes related <id|title>` lists explicit links first, then shared-tag
  neighbours ranked by tag-overlap count.
**Status:** Complete (Phase 2) — link targets resolve to ids; self-links excluded
**Notes:** Shared-tag links are computed at query time, never stored (INV-3). Related: D-003.

### F-006 Show single idea
**Priority:** Must
**Acceptance:**
- `phanes show <id|title>` renders one idea: metadata, provenance flags per
  field, explicit links, and shared-tag neighbours.
**Status:** Complete (Phase 2) — resolves id-or-fuzzy-title via `query::resolve`

### F-007 New idea capture
**Priority:** Should
**Acceptance:**
- `phanes new <title> [--tag ...]` writes a scaffold note with the blockquote
  header pre-filled (status, last_reviewed: today, tags), then indexes it.
**Status:** Not started (Phase 2)
**Notes:** Output must follow the scaffold standard (D-006, D-008).

### F-008 Local-model enrichment (opt-in)
**Priority:** Should
**Acceptance:**
- Built with `--features enrich` and run with `--enrich`, changed files receive a
  proposed summary, tags, topics, and status guess from a local llama-server,
  grammar-constrained to valid JSON.
- A missing, slow, or malformed response never fails an index pass (INV-4).
**Status:** Not started (HTTP client done; prompt/grammar tuning pending) (Phase 3)
**Notes:** Related: D-001, D-002, D-007.

### F-009 Three-panel desktop UI
**Priority:** Should
**Acceptance:**
- An egui app with three panels: left file/ideas explorer, centre note
  reader/editor, right idea/provenance/relationship info.
**Status:** Not started (Phase 4)
**Notes:** Frontend over the same `query`/`indexer` API; invariants unchanged. Related: D-005.

### F-010 Edit-in-place with re-index on save
**Priority:** Should
**Acceptance:**
- Editing a note in the centre panel and saving triggers a one-file index pass;
  enrichment fires only on save, under the hash gate (INV-1).
**Status:** Not started (Phase 4)
**Notes:** Related: D-005.

### F-011 Tinted table output
**Priority:** Could
**Acceptance:**
- CLI list output is a bordered table with per-status colour tints, emitted only
  when stdout is a TTY (clean when piped).
**Status:** Complete (Phase 1)

## Candidate features (uncommitted)

- Graph/map view of the relationship layer (petgraph export, or a TUI) — Phase 4.
- Embedding-based semantic "near this" search, as a second enrichment distinct
  from extraction — Phase 4.
- Per-idea `open` in `$EDITOR`.
