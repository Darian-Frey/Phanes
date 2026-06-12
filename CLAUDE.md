# CLAUDE.md — Phanes handoff

Working notes for Claude Code. Read the invariants first; they are the things
that must not drift.

## Invariants (non-negotiable)

- **INV-1 — Index-time-only model.** The enrichment model is called *only* in
  `indexer::run`, *only* for files whose `content_hash` changed (or under
  `--force`). No query path may invoke it. This is what keeps the daily-driver
  CLI fast and offline. The hash gate is already implemented in `indexer.rs`;
  do not bypass it.
- **INV-2 — Proposed never overwrites asserted.** `model::Sourced<T>` tags every
  value with `Provenance::{Asserted, Proposed}`. Deterministic facts are
  asserted; model output is proposed. `indexer::merge_proposed` already enforces
  the merge rules — proposed only fills gaps. `store::upsert` writes the
  `*_source` columns (done); still surface the flag in `show` output.
- **INV-3 — Relationships are computed, not stored** (except explicit links).
  Shared-tag neighbours come from a JOIN at query time so they cannot go stale.
  Only the `links` table is persisted.
- **INV-4 — Graceful degradation.** A missing, slow, or malformed model response
  must never fail an index pass. `enrich::enrich` returns `Result`; the caller
  logs and keeps the asserted-only record. Keep it that way.

## Architecture

```
main.rs        parse CLI, open Store, dispatch
 ├─ cli.rs     clap definitions (done)
 ├─ model.rs   types: Provenance, Sourced<T>, Status, Idea, Enrichment (done)
 ├─ parser.rs  deterministic extraction (helpers done; parse() assembles them)
 ├─ indexer.rs walk → hash-gate → parse → [enrich] → upsert (control flow done)
 ├─ store.rs   SQLite; open/hash_for_path/upsert/prune_missing (done)
 ├─ query.rs   search / stale / related / resolve / get (done)
 └─ enrich.rs  llama-server client, behind `enrich` feature (done)
sql/schema.sql      tables + FTS5 (done — load-bearing, treat as authoritative)
grammars/idea_extract.gbnf   constrains model JSON; keep in lockstep with Enrichment
```

## Status: done vs stubbed

- **Done:** the whole deterministic CLI (Phases 1–2). `store`
  (`hash_for_path`, `upsert`, `prune_missing`); `parser::parse` (frontmatter
  **and** the blockquote header — D-008) with link-target→id resolution;
  `query::{search, stale, related, resolve, get}`; the `show` command with
  per-field provenance flags; and `new` (`scaffold.rs`, Status: Concept — D-011).
  Plus the original scaffold: types, schema, GBNF grammar, the index control flow
  with hash gate and provenance merge, the enrichment HTTP client, and the CLI.
  Compiles and passes its lib tests with and without `--features enrich` (37
  with enrich). Every command body is implemented — no `todo!()` remains.
- **Done (Phase 4 UI):** the egui three-panel app (F-009/F-010). The `ui`
  feature + `src/bin/phanes-ui.rs` (eframe 0.34): left **explorer** (folder tree,
  a `query::search` filter, and click-to-select, backed by `query::list`),
  centre **editor** (View via `egui_commonmark` / Edit raw toggle; Save button or
  Ctrl+S → write file + one-file `indexer::run`, enrichment off per INV-1), and
  right **info** panel (status/provenance/tags/topics + clickable `related`).
  Build/run with `cargo run --features ui --bin phanes-ui -- ideas`. Note eframe
  0.34's `App` trait uses `fn ui(&mut self, ui: &mut egui::Ui, ..)` (not
  `update`), and panels are `Panel::left/right(...).show_inside(ui, ..)`.
- **Done (Phase 3 + F-012):** enrichment and semantic search. `enrich.rs` +
  `embed.rs` (feature `enrich`) call a local OpenAI-compatible server (D-012):
  `index --enrich` fills proposed summary/tags/topics; `index --embed` stores one
  vector per note (`embeddings` table, D-013); `query::near` ranks cosine-similar
  notes, shown via the `near` command and the UI's "Near (semantic)" panel. Both
  live-verified against LM Studio. Run e.g.
  `cargo run --features enrich -- index --root ideas --enrich --embed --force`.
- **Done (F-013):** relationship graph + gap analysis. `graph.rs` builds/analyses
  the graph (links + shared tags + semantic edges; components/orphans/bridges);
  `phanes gaps` lists orphans + candidate bridges; the UI `Graph` tab is a
  hand-rolled force-directed view (D-014 — no egui_graphs/petgraph) with a
  "Gaps" overlay (orphans + dashed candidate bridges) and a collision force for
  even spacing. Follow-up: model-proposed bridges.
- **Not yet built:** the remaining FEATURES.md candidates (taxonomy-aware tags,
  propose→accept links, RAG "ask" mode, open-in-$EDITOR, the gap overlay).

## Suggested implementation order

1. ~~`store::hash_for_path` + `store::upsert`~~ — done.
2. ~~`parser::parse`~~ — done; `phanes index` works end to end.
3. ~~`query::search` + `stale` + `print_hits` table formatting~~ — done.
4. ~~`query::related` + `resolve` + `get`, then `show`~~ — done.
5. ~~`new` command~~ — done. Phases 1–2 complete.
6. Per D-010: the egui three-panel UI (Phase 4) **before** enrichment (Phase 3). ← **next**

## Enrichment setup (the `enrich` feature)

The model is a scoped spoke: read one note, return one small JSON object. It is
not in the hot path and does not need to be clever. Targets an OpenAI-compatible
server (D-012), not llama.cpp native.

- Start an OpenAI-compatible server. On this machine: launch the **LM Studio**
  desktop app, load a model (e.g. Qwen2.5-Coder-14B), and start its local server
  (Developer tab, or `lms server start` *after* the app is running — the CLI
  can't bootstrap the daemon headless). Ollama or llama.cpp's `--api` mode work
  too.
- `enrich::enrich` POSTs to `http://127.0.0.1:1234/v1/chat/completions` (override
  `PHANES_LLM_URL`; pin a model with `PHANES_LLM_MODEL`) with `temperature: 0`
  and a `response_format` json_schema mirroring `model::Enrichment`. The
  json_schema is the active output constraint; keep its `status` enum in lockstep
  with `model::Status`. `grammars/idea_extract.gbnf` is retained only for the
  optional llama.cpp-native path.
- Run it: `cargo run --features enrich -- index --root <dir> --enrich --force`.
  A missing/slow/malformed reply never fails the pass (INV-4) — the asserted-only
  record is kept and the error logged.

## Conventions

- README carries the four-field blockquote header (Status / Provenance /
  Last reviewed / Why). Refresh it when status changes; propose Status + Why for
  Shane's confirmation rather than committing silently.
- This project follows the Development Documentation Standard
  (`development_documentation.md`): append-only stable IDs (F-/D-/BUG-/IMP-), log
  bugs/improvements when found rather than silently fixing them (Rule 8), and
  treat docs as part of the commit (Rule 7). See FEATURES.md, ARCHITECTURE.md,
  DECISIONS.md.
- Commit and push only when Shane asks.
- Crate versions in Cargo.toml are best-effort as of 2026-06; bump as needed.
- Verify changes with `cargo test --lib`, both with and without
  `--features enrich`.

## Methodology note

Hub-and-spoke: the deterministic core is the hub and the single source of truth;
the local model is a spoke doing bounded extraction. If a design choice would
make the model load-bearing for something deterministic code can do (titles,
links, dates), that's the signal to push it back to the parser.
