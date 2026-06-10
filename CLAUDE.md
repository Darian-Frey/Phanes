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
  the merge rules — proposed only fills gaps. Mirror this in `store::upsert`
  (write the `*_source` columns) and in `show` output (surface the flag).
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
 ├─ store.rs   SQLite; open()+schema done; upsert/queries stubbed
 ├─ query.rs   search / stale / related / resolve (stubbed, SQL sketched)
 └─ enrich.rs  llama-server client, behind `enrich` feature (done)
sql/schema.sql      tables + FTS5 (done — load-bearing, treat as authoritative)
grammars/idea_extract.gbnf   constrains model JSON; keep in lockstep with Enrichment
```

## Status: done vs stubbed

- **Done:** Cargo manifest, all type definitions, the schema, the GBNF grammar,
  the deterministic parser helpers (`content_hash`, `id_from_path`, `first_h1`,
  `extract_links`, `extract_wikilinks`), the full index control flow with the
  hash cache gate and provenance-respecting merge, the enrichment HTTP client,
  the CLI, and command dispatch wiring.
- **Stubbed (`todo!()`):** `parser::parse` (assemble the helpers + gray_matter
  frontmatter), `store::{hash_for_path, upsert, prune_missing}`,
  `query::{search, stale, related, resolve}`, and the `show` / `new` command
  bodies. The SQL each needs is written in doc comments at the stub.

## Suggested implementation order

1. `store::hash_for_path` + `store::upsert` — unblocks everything.
2. `parser::parse` — then `phanes index` works end to end (deterministic).
3. `query::search` + `stale` + `print_hits` table formatting.
4. `query::related` + `resolve`, then the `show` command.
5. `new` command (write frontmatter, then index the new file).
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
- Crate versions in Cargo.toml are best-effort as of 2026-06; bump as needed.
- Not yet compiled. First task after filling stubs: `cargo check` both with and
  without `--features enrich`.

## Methodology note

Hub-and-spoke: the deterministic core is the hub and the single source of truth;
the local model is a spoke doing bounded extraction. If a design choice would
make the model load-bearing for something deterministic code can do (titles,
links, dates), that's the signal to push it back to the parser.
