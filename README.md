> **Status:** Active
> **Provenance:** Claude (scaffold) · Shane Hartley (owner/architect)
> **Last reviewed:** 2026-06-14
> **Why this status:** The deterministic core, the three-panel desktop app, and the optional local-model layer (enrichment, embeddings, semantic "near", graph + gaps, bridges, and RAG "ask") are all shipped and in use.

---

# Phanes

A durable, fully-local tool for indexing, searching, and surfacing relationships
across a folder of project-idea markdown notes — a command-line tool and a
three-panel desktop app over the same core. Named for the Orphic primordial of
manifestation: bringing latent ideas into the light.

It is deliberately more than `rg` over a folder. Because the notes follow a known
convention (project-scaffold headers, optional frontmatter), Phanes parses real
metadata and answers questions grep can't: what's quietly rotting, what relates
to what, what's semantically *near* what — and, with the optional local model,
what a freeform note is actually about and how two ideas might connect.

**New here?** The [Manual](MANUAL.md) is the practical, task-first guide.

## Two rules that hold the design together

1. **The model runs at index time only.** Enrichment and embedding happen once
   per file, cached by content hash, and re-run only when the file changes.
   `search`, `related`, `stale`, `near`, and `show` read cached records and are
   always instant and offline. The sole exception is the explicit, user-invoked
   generative actions — `bridge` and `ask` — which are opt-in and clearly
   separate from the daily-driver queries (see [DECISIONS.md](DECISIONS.md),
   D-015/D-016).
2. **Model output is proposed, never canonical.** Deterministic facts (title,
   links, dates, the tags you wrote) are *asserted* and authoritative. Anything
   the model infers is *proposed*, carries a provenance flag, and never
   overwrites asserted data. You can accept a proposed value to promote it.

## Commands

```text
phanes index [--enrich] [--embed] [--force]   # (re)build the index
phanes search <query> [--status --tag --stale-days --limit --semantic]
phanes stale [--days 180]            # what hasn't been reviewed lately
phanes related <id|title>            # explicit links + shared-tag neighbours
phanes near <id|title>               # semantically similar notes (needs --embed)
phanes gaps                          # orphans + candidate bridges + hubs/clusters
phanes tags                          # the tag vocabulary with counts
phanes timeline                      # notes by date, newest first
phanes bridge <a> <b>                # propose an idea connecting two notes (AI)
phanes ask "<question>"              # answer a question from your notes (RAG, AI)
phanes show <id|title>               # one idea, with provenance and relationships
phanes new <title> [--tag ...]       # capture a new note, frontmatter pre-filled
```

`--root <dir>` (default `.`) points at your ideas folder; the index lives at
`<root>/.phanes/index.db`. The AI commands (`bridge`, `ask`) and the AI index
flags (`--enrich`, `--embed`) need the `enrich` build and a local model server.

## Desktop app

```bash
cargo run --features ui --bin phanes-ui -- <root>          # without AI
cargo run --features ui,enrich --bin phanes-ui -- <root>   # with AI features
```

Three panels over the same index: a **left** explorer (folder tree, filter,
⟳ Scan / ✨ Scan + AI), a **centre** pane with View / Edit / Graph / Ask tabs,
and a **right** info panel (status dropdown, editable/acceptable tags, summary,
related, and semantic "near"). See the [Manual](MANUAL.md#the-desktop-app).

**Portable build (Linux):** package the desktop app as a single AppImage. Needs
`linuxdeploy` and `appimagetool` on `PATH` (and FUSE to run the result).

```bash
packaging/build-appimage.sh                 # → dist/Phanes-<version>-x86_64.AppImage
./dist/Phanes-*.AppImage [folder]           # run it (defaults to ./ideas)
```

Caveats and details are in [packaging/README.md](packaging/README.md).

## Build

```bash
cargo build --release                       # core: deterministic, no model, no HTTP
cargo build --release --features enrich     # + local-model client
cargo run --features ui --bin phanes-ui     # desktop app (add ,enrich for AI)
```

The AI layer talks to a local OpenAI-compatible server (LM Studio / Ollama /
llama.cpp `--api`) over HTTP — nothing leaves your machine. Setup and the
`PHANES_LLM_*` / `PHANES_EMBED_*` overrides are in the
[Manual](MANUAL.md#the-local-model-ai-features).

## Project structure

```text
src/
  main.rs            CLI entry, dispatch + table rendering
  lib.rs             module root
  cli.rs             clap command definitions
  model.rs           core types (Provenance, Sourced<T>, Status, Idea, Enrichment)
  parser.rs          deterministic extraction (no model)
  scaffold.rs        new-note generation + status/tag edits (inverse of parser)
  indexer.rs         walk → hash-gate → parse → [enrich/embed] → upsert
  store.rs           SQLite + FTS5 persistence
  query.rs           search / stale / related / near / resolve / get / list
  graph.rs           relationship graph + gap analysis (F-013)
  enrich.rs          chat client: enrichment + bridges (feature `enrich`)
  embed.rs           embeddings client (feature `enrich`)
  ask.rs             RAG "ask" mode (feature `enrich`)
  bin/phanes-ui.rs   three-panel desktop app (feature `ui`)
sql/schema.sql               tables + FTS5 (authoritative)
grammars/idea_extract.gbnf   optional llama.cpp-native output constraint
ideas/        your local idea notes (gitignored)
examples/     sample notes documenting the convention
```

## Documentation

- [Manual](MANUAL.md) — the practical, task-first user guide (CLI + desktop app)
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
