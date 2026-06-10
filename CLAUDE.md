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
 ├─ query.rs   search + stale (done); related + resolve (stubbed, SQL sketched)
 └─ enrich.rs  llama-server client, behind `enrich` feature (done)
sql/schema.sql      tables + FTS5 (done — load-bearing, treat as authoritative)
grammars/idea_extract.gbnf   constrains model JSON; keep in lockstep with Enrichment
```

## Status: done vs stubbed

- **Done:** the full deterministic core — `store` (`hash_for_path`, `upsert`,
  `prune_missing`), `parser::parse` (frontmatter **and** the blockquote header —
  D-008), and `query::search` + `stale` with tinted table output. Plus the
  original scaffold: types, schema, GBNF grammar, parser helpers, the index
  control flow with hash gate and provenance merge, the enrichment HTTP client,
  and the CLI. Compiles and passes 16 lib tests with and without
  `--features enrich`. `phanes index --root ideas` works end to end.
- **Stubbed (`todo!()`):** `query::{related, resolve}` and the `show` / `new`
  command bodies. The SQL each needs is sketched in doc comments at the stub.

## Suggested implementation order

1. ~~`store::hash_for_path` + `store::upsert`~~ — done.
2. ~~`parser::parse`~~ — done; `phanes index` works end to end.
3. ~~`query::search` + `stale` + `print_hits` table formatting~~ — done.
4. `query::related` + `resolve`, then the `show` command. ← **next**
5. `new` command (write the scaffold header, then index the new file).
6. Build with `--features enrich`, stand up llama-server, test enrichment.

## Enrichment setup (the `enrich` feature)

The model is a scoped spoke: read one note, return one small JSON object. It is
not in the hot path and does not need to be clever.

- Run a small instruct model under llama.cpp in server mode, e.g.
  `llama-server -m <model>.gguf --port 8080`.
- Phanes POSTs to `http://127.0.0.1:8080/completion` (override via
  `PHANES_LLAMA_URL`) with the GBNF grammar, `temperature: 0`, `n_predict: 400`.
  Grammar-constrained decoding guarantees schema-valid JSON at the token level.
- Model choice: the task is light, so the smallest model that reliably fills the
  schema wins on speed. The existing Qwen 2.5 7B Q4_K_M is fine for CPU on the
  T1200 (LOW_VRAM); size up only if tag inference disappoints. Confirm the
  current best small model before settling.

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
