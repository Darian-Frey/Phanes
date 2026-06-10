> **Status:** Active
> **Provenance:** Shane Hartley (owner/architect), Claude (drafting)
> **Last reviewed:** 2026-06-10
> **Why this status:** Phase 1 complete; Phase 2 next.

# Roadmap

Phased so the deterministic core is useful before any model is involved. Phases
are append-only; mark Complete with an ISO date. Features are defined in
[FEATURES.md](FEATURES.md).

**Execution order:** Phase 4 (UI) is being built before Phase 3 (enrichment) —
see [D-010](DECISIONS.md). Phase numbers are unchanged; only the order of work is.

## Phase 1 — Deterministic core
**Goal:** Index a folder of notes and answer search/stale queries, offline.
**Status:** Complete (2026-06-10)
**Features delivered:** F-001, F-002, F-003, F-011
**Deliverables:**
- [x] `store`: hash lookup, upsert, prune ([src/store.rs](src/store.rs))
- [x] `parser::parse`: frontmatter + blockquote header + title + links + dates ([src/parser.rs](src/parser.rs))
- [x] `query::search` + `stale` ([src/query.rs](src/query.rs))
- [x] Table output — `tabled` + `owo-colors` status tints ([src/main.rs](src/main.rs))
**Acceptance:** `phanes index && phanes search foo` returns ranked hits offline. ✓

## Phase 2 — Relationships
**Goal:** Surface explicit and tag-adjacent relationships; single-idea view.
**Status:** Complete (2026-06-10)
**Features delivered:** F-004 (surfacing), F-005, F-006, F-007
**Deliverables:**
- [x] `links` persistence + dangling-target tolerance; link targets resolved to ids
- [x] `query::related`: explicit links, then shared-tag neighbours ranked by overlap
- [x] `query::resolve`: id-or-fuzzy-title → a single id (unique-match)
- [x] `show` command rendering metadata, provenance flags, and relationships
- [x] `new` command (scaffold note via `scaffold.rs`, then index and show it)
**Acceptance:** `phanes related <idea>` shows linked and tag-adjacent notes. ✓ (on tagged/linked corpora)

## Phase 3 — Enrichment (opt-in)
**Goal:** Freeform notes gain a proposed summary, tags, topics, and status guess.
**Status:** Not started
**Features delivered:** F-008
**Deliverables:**
- [ ] `--features enrich`: llama-server client (done) + prompt and grammar tuning
- [ ] Provenance surfaced in `show`; proposed tags visibly distinct from asserted
- [ ] `--force` re-enrich; verify the hash gate skips unchanged files
**Acceptance:** freeform notes get a usable summary, tags, and status guess, and a
re-index of an unchanged corpus costs ~zero model calls.

## Phase 4 — Desktop UI and later (not fully committed)
**Goal:** A three-panel egui app over the core, plus a spatial relationship view.
**Status:** In progress
**Features delivered:** F-009, F-010
**Deliverables:**
- [x] egui three-panel scaffold: explorer / editor / info ([D-005](DECISIONS.md)) — window opens over the core (`phanes-ui`)
- [ ] edit-in-place; save → one-file re-index (preserves INV-1)
- [ ] `new` template polish; per-idea `open` in `$EDITOR`
- [ ] graph/map view of the relationship layer (petgraph export, or a TUI)
- [ ] embedding-based semantic "near this" search as a second enrichment
**Acceptance:** the UI opens a note, shows its relationships, and re-indexes on save.
