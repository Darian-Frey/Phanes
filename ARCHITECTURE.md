> **Status:** Active
> **Provenance:** Shane Hartley (architect), Claude (drafting)
> **Last reviewed:** 2026-06-10
> **Why this status:** Describes the core as built through Phase 1; UI layer (F-009/F-010) is planned, not yet present.

# Architecture — Phanes

Descriptive only — *what the system is*. Rationale (*why*) lives in
[DECISIONS.md](DECISIONS.md); capabilities in [FEATURES.md](FEATURES.md).

## System overview

Hub-and-spoke. The deterministic core is the hub and the single source of truth;
the local model is a bounded spoke that runs at index time only and proposes,
never asserts.

```
                         ┌──────────────┐
   CLI args ─► cli.rs ─► │   main.rs    │ ─► table output (tabled + owo-colors)
                         │  dispatch    │
                         └──────┬───────┘
              ┌─────────────────┼──────────────────┐
              ▼                 ▼                   ▼
        indexer.rs          query.rs            (Store::open)
     walk→hash→parse     search / stale /          │
        →[enrich]→upsert  related / resolve         │
          │     │              │                    │
          ▼     ▼              ▼                    ▼
      parser.rs  enrich.rs  ┌────────────────────────────┐
     (deterministic) (spoke,│  store.rs  →  SQLite + FTS5 │
                  feature)  │  sql/schema.sql (authoritative)
                           └────────────────────────────┘
                  model.rs — types shared by every module
```

The index path is the only writer; the query path is read-only and never touches
the model or the network.

## Module responsibilities

- **model.rs** — shared types: `Provenance`, `Sourced<T>`, `Status` (incl.
  `Concept`/`Draft`), `Idea`, `Enrichment`. Encodes the asserted-vs-proposed
  boundary in the type system so it can't drift.
- **parser.rs** — deterministic extraction; no LLM. Content hash, title (first
  H1 → filename stem), explicit `.md` links and `[[wikilinks]]` (code-block
  aware), and asserted metadata from both YAML frontmatter and the blockquote
  header.
- **store.rs** — SQLite persistence. `open` (+ schema), `hash_for_path` (the
  cache gate), `upsert` (one transaction; writes provenance columns; hand-syncs
  the `ideas_fts` virtual table), `prune_missing`. Reads/writes resolved records
  only — the model never runs here.
- **indexer.rs** — index-time orchestration: walk `*.md` → hash-gate → parse →
  optional enrich → merge (proposed fills gaps) → upsert → prune. The **only**
  place the enrichment model is invoked.
- **query.rs** — read side: `search`, `stale` (done); `related`, `resolve`
  (pending). Deterministic and instant; shared-tag neighbours are a query-time
  JOIN, never stored.
- **enrich.rs** *(feature `enrich`)* — HTTP client for a local llama-server,
  output constrained by `grammars/idea_extract.gbnf`, temperature 0. Returns
  `Result`; the caller logs and degrades on failure.
- **cli.rs / main.rs** — clap command surface, dispatch, and table rendering.

## Data flow

- **Index:** `main` opens `Store` at `<root>/.phanes/index.db` → `indexer::run`
  walks, gates on hash, parses asserted facts, optionally enriches (proposed),
  merges, and upserts; then prunes vanished files.
- **Query:** `main` opens `Store` (read) → `query::*` runs SQL against `ideas` /
  `ideas_fts` / `tags` / `links` → `print_hits` renders.

## Key invariants

- **INV-1 — Index-time-only model.** The model runs only in `indexer::run`, only
  on hash-changed files (or `--force`). No query path invokes it. (D-001)
- **INV-2 — Proposed never overwrites asserted.** `Sourced<T>` tags provenance;
  `merge_proposed` and `store::upsert` only let proposed fill gaps. (D-002)
- **INV-3 — Relationships computed, not stored** (except the explicit `links`
  table). Shared-tag neighbours come from a query-time JOIN. (D-003)
- **INV-4 — Graceful degradation.** A missing/slow/malformed model response
  never fails an index pass; the asserted-only record is kept.

## Cross-cutting concerns

- **Error handling.** `anyhow::Result` throughout; enrichment failures are logged
  and swallowed (INV-4).
- **Offline & instant.** The default build links no HTTP stack; no query path
  performs network or model work.
- **Build features.** `default` (deterministic core), `enrich` (adds `reqwest` +
  the llama-server client), `ui` (planned — adds `eframe`/`egui` for F-009/F-010).
- **Provenance threading.** Carried from parser → indexer merge → store columns →
  `show` output, end to end.

## Storage schema

`sql/schema.sql` is authoritative (loaded via `include_str!`). Tables: `ideas`
(with `status_source` / `summary_source` provenance columns), `tags`
(per-tag `source`), `topics`, `links` (explicit out-links only), and the
`ideas_fts` FTS5 virtual table (porter unicode61) over title/summary/body.
