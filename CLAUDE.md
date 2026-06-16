# CLAUDE.md — Phanes handoff

Working notes for Claude Code. Read the invariants first; they are the things
that must not drift.

## Invariants (non-negotiable)

- **INV-1 — Model stays out of the hot path.** Automatic enrichment and
  embedding run *only* at index time (`indexer::run`, hash-gated). The instant
  query paths — `search` / `stale` / `related` / `show` / `near` — never invoke
  the model and stay offline. The one exception is **explicitly user-invoked
  generative actions** (the `bridge` command and the RAG `ask` mode): opt-in,
  clearly separate from the daily-driver queries, and graceful on failure
  (D-015, D-016). The hash gate in `indexer.rs` is load-bearing; do not bypass it, and
  never wire the model into a path a user expects to be instant.
- **INV-2 — Proposed never overwrites asserted.** `model::Sourced<T>` tags every
  value with `Provenance::{Asserted, Proposed}`. Deterministic facts are
  asserted; model output is proposed. `indexer::merge_proposed` already enforces
  the merge rules — proposed only fills gaps. `store::upsert` writes the
  `*_source` columns (done); still surface the flag in `show` output.
  **A deterministic re-index must never destroy proposed data** (BUG-003):
  `indexer::preserve_proposed` carries existing proposed summary/tags/topics
  forward on a non-enrich pass, and the embedding is no longer cleared — so
  editing a note (status, typo) keeps its enrichment until `--enrich`/`--force`
  refreshes it. Don't reintroduce a wipe.
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
  **Taxonomy-aware tags** (refines F-008): `indexer::run` snapshots
  `query::tag_vocabulary(store, 80)` once per pass and passes it to
  `enrich::enrich(title, body, &vocab)`, which appends it to the prompt so proposed
  tags reuse the vocabulary. `--force` re-enriches all to consolidate.
- **Done (F-013):** relationship graph + gap analysis. `graph.rs` builds/analyses
  the graph (links + shared tags + semantic edges; components/orphans/bridges);
  `phanes gaps` lists orphans + candidate bridges; the UI `Graph` tab is a
  hand-rolled force-directed view (D-014 — no egui_graphs/petgraph) with a
  "Gaps" overlay (orphans + dashed candidate bridges) and a collision force for
  even spacing. Left-click a node opens it; **right-click** inspects it in place
  (`GraphAction::Inspect` → `inspect()` = `select` but keep the mode; highlights
  the node, its incident edges, and neighbour rings/labels).
- **Done (model-proposed bridges):** `enrich::propose_bridge` + the `bridge a b`
  command generate an idea connecting two notes — the first query-time model use,
  an explicit opt-in generative action (D-015, INV-1 carve-out). Also invocable by
  clicking a dashed gap edge in the UI graph; the model call runs on a background
  thread (mpsc channel → floating result window) so the UI never freezes. Build
  the UI with bridges via `cargo run --features ui,enrich --bin phanes-ui -- ideas`.
  A canvas stats overlay shows notes/links/clusters/orphans. Live-verified.
- **Done (F-014 editable/acceptable tags):** the info panel's tags section edits
  asserted tags in place — `×` removes, `✓` accepts a proposed tag (promote to
  asserted), an "add tag" field appends. Writes the frontmatter `tags:` key via
  `scaffold::set_tags` and syncs the DB via `store::set_asserted_tags` (no full
  re-index, so other proposed tags survive — INV-2). Propose→accept for tags; the
  link form remains a candidate.
- **Done (F-015 RAG "Ask" mode):** `ask.rs` (feature `enrich`) + the `phanes ask`
  command and UI **Ask** tab. Embeds the question, ranks the stored embeddings
  (`ask::rank`, deterministic), and runs one on-demand generation over the
  retrieved notes with `[title]` citations + clickable sources. The second
  query-time generative action under the D-015 carve-out — boundary recorded in
  D-016; never wired into the instant query paths. UI call is threaded (own read
  DB connection). `enrich::chat` is `pub(crate)` for reuse. Live-verified.
- **Done (F-021 hybrid search):** `query::hybrid` fuses (RRF, `rrf` helper) FTS
  hits with notes semantically near the top keyword matches (cosine over stored
  vectors). Query is never embedded → search stays offline (INV-1). `search
  --semantic` CLI flag + a **Semantic** checkbox by the explorer filter
  (`semantic_search`, `run_filter` switches between `search`/`hybrid`). Falls back
  to FTS on no embeddings / no hits / a filter set.
- **Done (F-026 in-app manual):** `MANUAL.md` embedded via `include_str!`
  (`MANUAL` const), shown in the centre pane by `egui_commonmark` behind a `?`
  button / F1 toggle (`show_manual`); read-only, not an indexed note. Ships in the
  AppImage with no runtime file dependency.
- **Done (F-027 colour themes):** top-bar 🎨 picker → Dark/Light/Parchment/
  Cyberpunk. `apply_theme(ctx, theme)` sets egui `Visuals` + fonts (Parchment =
  bundled DejaVu Serif via `include_bytes!` in `assets/fonts/`; Cyberpunk = built-in
  mono) + a thread-local `dark_bg` flag the colour helpers (`status_color`,
  `cluster_color`, `edge_color`, `proposed_color`, focus/label) read so they stay
  legible on light themes. Persisted to `$XDG_CONFIG_HOME/phanes/theme`.
- **Done (F-024 open questions):** `enrich::propose_questions(notes)` (freeform,
  newline-split) generates open questions for a cluster. `phanes questions` (whole
  corpus) + a Graph-tab **❓ Questions** button → `start_questions` (focused node's
  community via `self.communities`, else whole corpus), threaded
  (`QuestionsState`/`questions_rx`, floating `questions_window`). Third query-time
  generative action (bridge/ask/questions) under D-015/D-016; never written to files.
- **Done (F-023 auto-classify):** enrichment proposes a coarse **category** (kind
  of note) per note. New `Idea.category`/`Enrichment.category` (proposed), stored in
  `ideas.category`/`category_source` (added via an `ALTER TABLE` migration in
  `store::open` — idempotent, ignores "duplicate column"). `merge_proposed` sets it,
  `preserve_proposed` carries it forward (BUG-003), `show` + the UI info panel show
  it. `Enrichment.category` has `#[serde(default)]`.
- **Done (F-022 timeline):** `query::timeline` (notes by effective date — the
  `stale` COALESCE — newest first). Left explorer **Timeline** view (4th toggle),
  grouped by month; `phanes timeline` CLI. Lazy (`timeline` field), invalidated on
  reload. Deterministic (INV-3).
- **Done (F-018 tag browser):** `query::tag_index` (every tag → asserted/proposed
  counts + its notes, sorted by use). Left explorer has a **Tags** view (third
  toggle): collapsing header per tag (`tag · total (~proposed)`) → its notes.
  `phanes tags` CLI prints the vocabulary. Built lazily (`tag_groups`), invalidated
  on reload. Deterministic (INV-3).
- **Done (F-020 graph analytics):** `graph::betweenness` (Brandes, normalised) +
  `graph::communities` (deterministic weighted label propagation). UI Graph tab has
  a **Clusters** toggle: node colour = community (`cluster_color` palette), size =
  centrality; both cached in the app (`centrality`/`communities`) when the graph is
  built. `phanes gaps` prints Hubs + Clusters. Deterministic (INV-1/INV-3).
- **Done (F-019 live file-watching):** `notify` recursive watcher (UI-only dep,
  `start_watch`) auto re-indexes on external `.md` changes; filters to `.md`
  outside dotfolders (no `.phanes/` loop), pings a channel + `ctx.request_repaint`
  (an idle egui window won't wake on a channel alone — load-bearing). `poll_watch`
  debounces ~500 ms, defers under `ai_rx`, refreshes only on changed/pruned > 0.
  ⟳ Scan stays as manual fallback.
- **Done (F-017 quick switcher):** `Ctrl/Cmd+P` → `quick_switcher` overlay; fuzzy
  (`fuzzy_score`, subsequence) jump to any note by title/id, ↑/↓/Enter/Esc, snapshots
  `query::list` on open, selecting reuses `select` (so it reveals in the explorer).
  Command-palette-for-actions not built.
- **Done (F-016 backlinks + unlinked mentions):** `query::backlinks` (incoming
  links, `links` JOIN on `dst_id`) and `query::unlinked_mentions` (FTS phrase
  match on the title, minus already-linked + self). `show` prints both; the UI
  info panel adds **Backlinks** and **Unlinked mentions** sections, the latter
  with a 🔗 accept button → `accept_mention` writes a relative md link
  (`scaffold::link_mention`; `<…>`-wrapped for spaces) into the mentioning note,
  then re-indexes. Caveat: single common-word titles give noisy mentions.
- **Done (F-025 Files view):** the left panel has an **Ideas/Files** toggle.
  Ideas is the indexed-note tree (`build_tree` from `query::list`); Files is a raw
  `walkdir` tree (`build_file_tree`, dotfiles/`.phanes`/`.git` hidden), rendered by
  `render_file_tree`. Clicking a `.md` → `open_file` (indexed → `select`; else raw
  view). Tree built lazily, invalidated in `reload_after_index`. Filter is
  Ideas-only.
- **Not yet built:** the remaining FEATURES.md candidates (taxonomy-aware tags,
  propose→accept *links*, open-in-$EDITOR, near-duplicate/merge detection,
  title suggestions, stale-triage next steps).

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
  DECISIONS.md, BUGS.md, IMPROVEMENTS.md.
- Commit and push only when Shane asks.
- Crate versions in Cargo.toml are best-effort as of 2026-06; bump as needed.
- Verify changes with `cargo test --lib`, both with and without
  `--features enrich`.

## Methodology note

Hub-and-spoke: the deterministic core is the hub and the single source of truth;
the local model is a spoke doing bounded extraction. If a design choice would
make the model load-bearing for something deterministic code can do (titles,
links, dates), that's the signal to push it back to the parser.
