> **Status:** Active
> **Provenance:** Claude (scaffold) · Shane Hartley (owner/architect)
> **Last reviewed:** 2026-06-10
> **Why this status:** Fresh scaffold. Core + relationships + enrichment design is locked and encoded in the types; command bodies are stubbed for implementation.

---

# Phanes

A durable CLI for indexing, searching, and surfacing relationships across a folder of project-idea markdown notes. Named for the Orphic primordial of manifestation — bringing latent ideas into the light.

It is deliberately more than `rg` over a folder. Because the notes follow a known convention (project-scaffold headers, optional frontmatter), Phanes parses real metadata and answers questions grep can't: what's quietly rotting, what relates to what, and — with the optional local model — what a freeform note is actually about.

## Two rules that hold the design together

1. **The model runs at index time only.** Enrichment happens once per file, cached by content hash, and re-runs only when the file changes. Search, `related`, and `stale` read cached records and are always instant and offline.
2. **Model output is proposed, never canonical.** Deterministic facts (title, links, dates) are *asserted* and authoritative. Anything the model infers is *proposed*, carries a provenance flag, and never overwrites asserted data.

## Commands

```
phanes index [--enrich] [--force]   # (re)build the index
phanes search <query> [--status --tag --stale-days --limit]
phanes stale [--days 180]           # what hasn't been reviewed lately
phanes related <id|title>           # explicit links + shared-tag neighbours
phanes show <id|title>              # one idea, with provenance and relationships
phanes new <title> [--tag ...]      # capture a new note, frontmatter pre-filled
```

The index lives at `<root>/.phanes/index.db`. Point `--root` at your ideas folder.

## Build

```
cargo build --release                 # core: deterministic, no model, no HTTP
cargo build --release --features enrich   # adds the local-model client
```

Enrichment talks to a local `llama-server` (llama.cpp) over HTTP — see CLAUDE.md.

## Licence

MIT OR Apache-2.0.
