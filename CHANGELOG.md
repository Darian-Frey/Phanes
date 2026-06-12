# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com).
Phanes is pre-release; all work to date sits under [Unreleased].
Entries reference F- (features) and D- (decisions) IDs for traceability.

## [Unreleased]

### Added
- F-013 Relationship graph view + gap analysis. New `graph.rs` builds the graph
  (links + shared tags + semantic edges) and analyses it (connected components,
  orphans, candidate bridges) — deterministic, rebuilt from the index (INV-3).
  A `phanes gaps` command lists orphan ideas and candidate bridges (strong
  semantic pairs not explicitly linked). The UI gains a `Graph` tab: a
  hand-rolled force-directed, status-tinted node graph (D-014) with pan/zoom,
  hover labels, drag-a-node (neighbours spring along; alpha-cooled to settle),
  and click-to-select. A "Gaps" toggle overlays orphans (ringed) and the top
  candidate bridges (dashed, `%`-labelled) on the canvas. The layout uses a
  collision force (d3 `forceCollide`-style) for even, non-lumpy node spacing.
  No new dependencies.
- F-012 Semantic "near this". `phanes index --embed` (with `--features enrich`)
  stores one embedding vector per changed note via a local embedding model
  (OpenAI `/v1/embeddings`; env `PHANES_EMBED_URL` / `PHANES_EMBED_MODEL`).
  `phanes near <id|title>` and a "Near (semantic)" section in the UI info panel
  rank notes by cosine similarity over stored vectors — computed at query time,
  no model on the query path (INV-1), neighbours not stored (INV-3), failed
  embeds non-fatal (INV-4). Vectors live in a new `embeddings` table (f32 BLOB).
  Verified live on the 28-note corpus (nomic-embed-text, 768-dim). See D-013.
- F-001 Deterministic indexing — `store` (`hash_for_path`, `upsert`,
  `prune_missing`) and `parser::parse` (YAML frontmatter **and** the blockquote
  header convention; title, links, dates, status). `phanes index` works end to
  end and offline.
- F-002 Full-text search with `--status` / `--tag` / `--stale-days` / `--limit`
  filters, FTS5 ranking, and highlighted snippets.
- F-003 Stale view (`phanes stale --days N`), oldest first.
- F-005 `related` — explicit links first, then shared-tag neighbours ranked by
  overlap; self-links excluded. Link targets (relative `.md` paths and
  wikilinks) are resolved to ids at index time so they join at query time.
- F-006 `show` — single-idea view via `query::resolve` (exact id, exact title,
  or unique substring) and `query::get`, rendering metadata, relationships, and
  per-field provenance flags (F-004 surfaced — INV-2 made visible on the CLI).
- F-007 `new` — capture a scaffold note (`scaffold.rs`): blockquote header with
  Status: Concept (D-011), `--tag` values as asserted frontmatter, refuses to
  overwrite, then indexes and shows it. Completes Phase 2 — no command bodies
  remain stubbed.
- F-009 (in progress) Desktop UI — a `ui` feature and a `phanes-ui` binary
  (eframe 0.34) opening a three-panel window over the core; the default CLI build
  stays egui-free (`required-features`). Left explorer is functional: a
  collapsing folder tree of indexed notes (status-tinted), a filter box backed by
  `query::search`, and click-to-select that drives the other panels. Backed by a
  new `query::list`.
- F-010 Centre editor — View (rendered markdown via `egui_commonmark`) / Edit
  (raw textarea) toggle; explicit Save (button or Ctrl+S) writes the file and
  runs a one-file `indexer::run`, then refreshes the tree and selection.
  Enrichment never fires here (INV-1).
- F-009 Right info panel — the GUI counterpart of `show`: status with an
  asserted/proposed badge, review/modified dates, summary, tags (proposed tags
  marked), topics, and the `related` list (links + shared-tag neighbours) with
  click-to-navigate. The three-panel UI is feature-complete.
- `phanes-ui` indexes its root folder on startup (hash-gated, no enrichment), so
  it works when pointed at a never-indexed folder; shows an empty-state hint when
  a folder has no notes.
- F-011 Tinted bordered table output (`tabled` + `owo-colors`, TTY-gated).
- `Status` enum gains `Concept` and `Draft` variants (D-007), kept in lockstep
  with `grammars/idea_extract.gbnf`.
- Project documentation per the Development Documentation Standard: FEATURES.md,
  ARCHITECTURE.md, DECISIONS.md, CHANGELOG.md, LICENSE-MIT, LICENSE-APACHE;
  README and ROADMAP brought to the standard's shape.
- F-008 Enrichment is live end to end (Phase 3 complete). `phanes index --enrich`
  (with `--features enrich`) gives changed notes a proposed summary, tags, and
  topics from a local model; verified against LM Studio (qwen2.5-7b-instruct).
  Proposed values fill gaps only, never overwrite asserted ones, are never
  written back to the source files, and the hash gate keeps re-indexing an
  unchanged corpus at zero model calls (INV-1/2/4 all confirmed). `show` and the
  UI render proposed values distinctly.

### Changed
- F-008 Enrichment client retargeted to the OpenAI-compatible API
  (`/v1/chat/completions` with `response_format` json_schema) instead of
  llama.cpp's native `/completion` + GBNF (D-012). Works with LM Studio / Ollama /
  llama.cpp `--api`; env `PHANES_LLM_URL` / `PHANES_LLM_MODEL`. Char-boundary-safe
  body truncation; graceful degradation unchanged (INV-4). `grammars/idea_extract.gbnf`
  retained for the optional llama.cpp-native path.

### Fixed
- Silenced the indexer's conditional `unused_mut` warning — the `idea` binding is
  only mutated when `--features enrich` is compiled in.
- Wikilink extraction no longer mistakes TOML table-arrays (`[[shaft]]`) or
  inline code spans for links — it now skips fenced code blocks and code spans
  via pulldown-cmark's offset iterator. (To be backfilled as BUG-001 when
  BUGS.md is added.)
