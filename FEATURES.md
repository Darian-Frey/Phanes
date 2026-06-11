> **Status:** Active
> **Provenance:** Shane Hartley (owner/architect), Claude (drafting)
> **Last reviewed:** 2026-06-11
> **Why this status:** Phases 1–2 and the Phase 4 three-panel UI shipped; P3 enrichment pending. Candidate features captured from a 2026-06-11 survey of local-LLM note tools.

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
  header pre-filled (Status: Concept, last_reviewed: today; tags in frontmatter),
  refuses to overwrite an existing note, then indexes and shows it.
**Status:** Complete (Phase 2)
**Notes:** Output follows the scaffold standard (D-006, D-008); new notes default
to Concept (D-011). Generator lives in `scaffold.rs`, round-trips through `parser`.

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
**Status:** Complete (Phase 4) — `phanes-ui` (eframe 0.34): explorer (folder tree + filter), centre editor, and a right info panel (status/provenance/tags/topics + clickable related)
**Notes:** Frontend over the same `query`/`indexer` API; invariants unchanged. Related: D-005.

### F-010 Edit-in-place with re-index on save
**Priority:** Should
**Acceptance:**
- Editing a note in the centre panel and saving triggers a one-file index pass;
  enrichment fires only on save, under the hash gate (INV-1).
**Status:** Complete (Phase 4) — View/Edit toggle + explicit Save (button / Ctrl+S) → write + one-file re-index; verified interactively
**Notes:** Related: D-005.

### F-011 Tinted table output
**Priority:** Could
**Acceptance:**
- CLI list output is a bordered table with per-status colour tints, emitted only
  when stdout is a TTY (clean when piped).
**Status:** Complete (Phase 1)

## Candidate features (uncommitted)

Ideas not committed to. Most come from a 2026-06-11 survey of local-LLM note
tools (Reor, Khoj, Smart Connections, InfraNodus, LM Studio). Each graduates to
an `F-NNN` entry if/when committed. Grouped by how they sit with the invariants:
most fit **INV-1** (model at index time; queries instant and offline), one (RAG
chat) does not and is flagged.

### Fits the index-time / proposed model (queries stay instant + offline)

- **Semantic "near this".** Embed each note at index time — a second enrichment
  spoke beside extraction — and store the vectors. Query-time similarity is
  deterministic vector math (no model in the query path), so INV-1 holds. Adds a
  *proposed* "related (semantic)" set beside the deterministic links/shared-tags.
  Highest payoff; fixes the empty `related` on a tag-sparse corpus. Pairs with
  D-001/D-003.
- **Taxonomy-aware proposed tags.** Feed the model the existing asserted-tag
  vocabulary so proposed tags stay consistent rather than inventing synonyms.
  Refinement of F-008.
- **Propose → accept links.** Suggested links (from the model or embeddings) show
  as *proposed*; one action promotes a link to *asserted* and writes it to the
  file. Uses the provenance core directly (INV-2) — the Phanes-specific angle no
  surveyed tool has.
- **Auto-summary / TL;DR** surfaced atop the centre pane and in the info panel
  (part of F-008).
- **Near-duplicate / merge detection** over the embedding vectors (deterministic;
  flags overlapping notes as merge candidates).
- **Title / filename suggestions** for poorly-named notes (proposed).

### Spatial / graph layer (matches the spatial-first preference)

- **Graph / map view** of the relationship layer (explicit links + shared-tag +
  semantic), petgraph + egui. Was the Phase 4 spatial item.
- **Gap / blind-spot detection** (InfraNodus-style): compute graph structure
  deterministically (clusters, weakly-connected components, missing bridges,
  orphans), then optionally have the model *propose* a bridging idea or research
  question for a detected gap. Detection is deterministic; the bridge is proposed.
  Strong fit for an *idea* tool — "two clusters that should connect but don't."
- **Stale triage with a proposed next step** — each rotting note (from `stale`)
  gets a proposed revival prompt / next action.
- **Cluster + orphan overview** — surface dense clusters and unconnected notes
  (deterministic graph metrics).

### Powerful but breaks INV-1 — only as a bounded, opt-in mode

- **"Ask" / RAG chat over the corpus** with citations (the most popular feature
  in Reor/Khoj/LM Studio). Retrieval reuses the index-time embeddings; only
  generation runs on demand. Because it puts the model in a query path, it must
  be a deliberately separate, user-invoked mode — never baked into `search`/`show`
  — and warrants its own DECISIONS entry recording the boundary.

### Smaller

- Per-idea `open` in `$EDITOR`.
