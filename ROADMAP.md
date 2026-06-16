> **Status:** Active
> **Provenance:** Shane Hartley (owner/architect), Claude (drafting)
> **Last reviewed:** 2026-06-16
> **Why this status:** All roadmap phases complete; v1.0.0 shipped. Further work is feature-by-feature (see FEATURES.md / CHANGELOG.md), not phased.

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
**Status:** Complete (2026-06-11)
**Features delivered:** F-008
**Deliverables:**
- [x] `--features enrich`: OpenAI-compatible client (D-012); prompt + json_schema
- [x] Provenance surfaced in `show` and the UI; proposed tags visibly distinct (`~`)
- [x] `--force` re-enrich; hash gate verified to skip unchanged (zero model calls)
**Acceptance:** freeform notes get a usable summary, tags, and status guess, and a
re-index of an unchanged corpus costs ~zero model calls. ✓ (live against LM Studio)

## Phase 4 — Desktop UI and later
**Goal:** A three-panel egui app over the core, plus a spatial relationship view.
**Status:** Complete (2026-06-16)
**Features delivered:** F-009, F-010, F-012, F-013
**Deliverables:**
- [x] egui three-panel app: explorer (folder tree + filter), editor, info panel ([D-005](DECISIONS.md))
- [x] edit-in-place; save → one-file re-index (preserves INV-1)
- [x] graph/map view of the relationship layer (hand-rolled force-directed, D-014)
- [x] embedding-based semantic "near this" search (F-012)
- [ ] per-idea `open` in `$EDITOR` — deferred (a remaining FEATURES.md candidate)
**Acceptance:** the UI opens a note, shows its relationships, and re-indexes on save. ✓

## Phase 5 — Beyond the roadmap (1.0)
**Goal:** Close the peer-tool gap analysis and round out the daily-driver UX.
**Status:** Complete (2026-06-16)
**Features delivered:** F-014–F-027 (plus taxonomy-aware tags); bug fixes BUG-003,
BUG-004.
**Deliverables:**
- [x] Editable/acceptable tags (F-014); backlinks + unlinked mentions (F-016)
- [x] RAG ask (F-015); model-proposed bridges; generated open questions (F-024)
- [x] Quick switcher (F-017); Tag browser (F-018); Timeline (F-022); Files view (F-025)
- [x] Live file-watching (F-019); graph hubs + clusters (F-020); hybrid search (F-021)
- [x] Auto-classify (F-023); in-app manual (F-026); colour themes (F-027)
- [x] AppImage packaging; preserve enrichment across re-index (BUG-003); rustls (BUG-004)
**Acceptance:** the candidate backlog (F-016…F-027) is shipped and the AppImage runs
the full feature set. ✓
