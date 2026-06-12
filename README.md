> **Status:** Active
> **Provenance:** Claude (scaffold) · Shane Hartley (owner/architect)
> **Last reviewed:** 2026-06-10
> **Why this status:** Phase 1 (deterministic core — index, search, stale) is implemented and tested; Phase 2 relationships and the `show`/`new` commands are next. See [ROADMAP.md](ROADMAP.md).

---

# Phanes

A durable CLI for indexing, searching, and surfacing relationships across a folder of project-idea markdown notes. Named for the Orphic primordial of manifestation — bringing latent ideas into the light.

It is deliberately more than `rg` over a folder. Because the notes follow a known convention (project-scaffold headers, optional frontmatter), Phanes parses real metadata and answers questions grep can't: what's quietly rotting, what relates to what, and — with the optional local model — what a freeform note is actually about.

## Two rules that hold the design together

1. **The model runs at index time only.** Enrichment happens once per file, cached by content hash, and re-runs only when the file changes. Search, `related`, and `stale` read cached records and are always instant and offline.
2. **Model output is proposed, never canonical.** Deterministic facts (title, links, dates) are *asserted* and authoritative. Anything the model infers is *proposed*, carries a provenance flag, and never overwrites asserted data.

## Commands

```text
phanes index [--enrich] [--force]   # (re)build the index
phanes search <query> [--status --tag --stale-days --limit]
phanes stale [--days 180]           # what hasn't been reviewed lately
phanes related <id|title>           # explicit links + shared-tag neighbours
phanes show <id|title>              # one idea, with provenance and relationships
phanes new <title> [--tag ...]      # capture a new note, frontmatter pre-filled
```

The index lives at `<root>/.phanes/index.db`. Point `--root` at your ideas folder.

## Build

```bash
cargo build --release                 # core: deterministic, no model, no HTTP
cargo build --release --features enrich   # adds the local-model client
```

Enrichment talks to a local OpenAI-compatible server (LM Studio / Ollama / llama.cpp `--api`) over HTTP — see CLAUDE.md.

## Project structure

```text
src/
  main.rs      CLI entry + table rendering
  lib.rs       module root
  cli.rs       clap command definitions
  model.rs     core types (Provenance, Sourced<T>, Status, Idea, Enrichment)
  parser.rs    deterministic extraction (no LLM)
  indexer.rs   walk → hash-gate → parse → [enrich] → upsert
  store.rs     SQLite + FTS5 persistence
  query.rs     search / stale / related / resolve
  enrich.rs    llama-server client (feature `enrich`)
sql/schema.sql               tables + FTS5 (authoritative)
grammars/idea_extract.gbnf   constrains the model's JSON output
ideas/        your local idea notes (gitignored)
examples/     sample notes documenting the convention
```

## Documentation

- [Features](FEATURES.md) — capabilities, priorities, acceptance criteria
- [Architecture](ARCHITECTURE.md) — modules, data flow, invariants
- [Decisions](DECISIONS.md) — design rationale (D-001…) with reversal conditions
- [Roadmap](ROADMAP.md) — phased plan
- [Bugs](BUGS.md) · [Improvements](IMPROVEMENTS.md) — logged when found (Rule 8)
- [Changelog](CHANGELOG.md) — what changed and when
- [CLAUDE.md](CLAUDE.md) — AI-session handoff
- [Development Documentation Standard](development_documentation.md) — the doc conventions this project follows

## Licence

Dual-licensed under either [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
