# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com).
Entries reference F- (features) and D- (decisions) IDs for traceability.

## [Unreleased]

### Fixed
- Box-drawing glyphs (`├ └ │ ─`) in rendered markdown no longer show as □ in the
  Dark/Light themes (BUG-005). egui's default proportional font lacks them; the
  bundled DejaVu Serif is now registered as a fallback font in every theme, so
  glyphs the primary font lacks fall through to it.

## [1.0.0] - 2026-06-16

First release. The deterministic core (index / search / stale / related / near /
gaps / show / new), the three-panel desktop app, and the opt-in local-model layer
(enrichment, embeddings, graph analytics, bridges, RAG ask, generated questions)
are all shipped and in daily use. Highlights below; see the full list for detail.

### Added
- F-027 Colour themes. A 🎨 picker in a new top bar switches the whole UI between
  **Dark**, **Light**, **Parchment** (warm sepia + a bundled DejaVu Serif), and
  **Cyberpunk** (neon-on-near-black + the built-in monospace). Each is a full egui
  palette via `apply_theme`; the choice persists to `$XDG_CONFIG_HOME/phanes/theme`.
  Semantic colours (status / cluster / graph edges / proposed) switch bright↔dark
  so they stay legible on light themes. The serif is bundled (`assets/fonts/`) and
  compiled in, so it ships in the AppImage.
- F-024 Generated open questions. `enrich::propose_questions` feeds a cluster's
  note titles + summaries to the model and returns open questions / unexplored
  directions. `phanes questions` runs it over the whole corpus; the Graph tab's
  **❓ Questions** button runs it for the focused node's cluster (or the whole
  corpus) on a background thread, shown in a floating window. The third query-time
  generative action (after bridge, ask) under the D-015/D-016 carve-out — never on
  an instant path; questions are displayed, never written to files. Graceful on
  failure (INV-4).
- F-023 Auto-classify. Enrichment now also proposes a single coarse **category**
  per note (the kind of note — developer-tool, research, creative, spec…), a new
  proposed field stored in the `ideas` table (`category`/`category_source`, added
  by a lightweight `ALTER TABLE` migration so existing indexes upgrade in place).
  `show` and the UI info panel display it with a provenance badge. Index-time and
  proposed (INV-1/INV-2); preserved across deterministic re-indexes (BUG-003).
- F-022 Timeline view. `query::timeline` orders notes by effective date
  (last-reviewed, else modified — same rule as `stale`), newest first. The left
  explorer gains a **Timeline** view (fourth toggle, grouped by month);
  `phanes timeline` prints the same on the CLI. Deterministic (INV-3); built
  lazily, invalidated on re-index.
- Graph: **right-click a node to inspect it** (refines F-013/F-020). Right-click
  focuses the node — highlights it, lights up its edges, rings and labels its
  direct neighbours, and loads its info into the right panel — *without* leaving
  the graph or opening the file in the centre. Left-click still opens the note.
  The focus highlight is cool cyan, deliberately distinct from the Gaps overlay's
  warm gold, so the two read clearly when both are on.
- The right **info panel is now scrollable**, so long relationship lists
  (related / backlinks / unlinked mentions / near) no longer run off the bottom.
- Taxonomy-aware proposed tags (refines F-008). At enrichment time the existing
  tag vocabulary (`query::tag_vocabulary`, most-used first) is fed to the model so
  proposed tags reuse it instead of inventing synonyms — curbing the singleton-tag
  sprawl the tag browser (F-018) exposed. Snapshotted once per pass; converges over
  runs, and `index --enrich --force` re-enriches the whole corpus to consolidate.
  Still index-time only (INV-1); proposed, never overwriting asserted (INV-2).
- F-021 Hybrid search. `query::hybrid` augments full-text results with notes
  semantically near the top keyword matches (cosine over the index-time vectors),
  fused via reciprocal-rank fusion — better recall, surfacing topically-adjacent
  notes that lack the exact keywords. `phanes search --semantic` and a **Semantic**
  checkbox by the explorer filter. The query is never embedded, so search stays
  fully offline/deterministic (INV-1); falls back to plain FTS with no embeddings,
  no hits, or a metadata filter set.
- F-026 In-app manual viewer. A `?` button in the centre toolbar (and the F1 key)
  opens the user manual rendered in the centre pane via the existing markdown
  viewer; Close / F1 dismisses it. `MANUAL.md` is embedded with `include_str!`, so
  it ships inside the binary / AppImage and needs no file at runtime. Read-only —
  never treated as an indexed note.
- F-018 Tag browser. `query::tag_index` returns the whole tag vocabulary with
  per-tag asserted/proposed counts and the notes carrying each tag. The left
  explorer gains a **Tags** view (third toggle alongside Ideas/Files): tags listed
  by use, each a collapsing header (`tag · total (~proposed)`) you expand to its
  notes (click to open). `phanes tags` prints the same on the CLI. Deterministic
  (INV-3); built lazily and invalidated on re-index.
- F-020 Graph analytics — hubs and clusters. `graph::betweenness` (Brandes,
  normalised) finds central "bridge" notes; `graph::communities` (deterministic
  weighted label propagation) groups notes into topical clusters. The UI Graph tab
  gains a **Clusters** toggle: nodes are coloured by community and sized by
  centrality (hubs bigger), and the stats overlay shows the cluster count. `phanes
  gaps` now also prints a **Hubs** list and a **Clusters** summary. Deterministic,
  no model — rebuilt from the index (INV-1/INV-3); extends F-013.
- F-019 Live file-watching. The desktop app now watches the root and auto
  re-indexes on external `.md` create/modify/delete — no more pressing ⟳ Scan
  after editing notes outside the app. A `notify` recursive watcher (new UI-only
  dependency) filters to `.md` changes outside dotfolders (so the `.phanes/` index
  DB can't trigger a loop) and wakes the UI via `request_repaint`; `poll_watch`
  debounces ~500 ms, defers while a Scan + AI is running, and refreshes only when
  the index actually changed (the app's own saves cause no churn). Deterministic,
  hash-gated, no model (INV-1). The ⟳ Scan button remains as a manual fallback.
- F-017 Quick switcher. `Ctrl/Cmd+P` opens a centered fuzzy "jump to a note"
  overlay from any view: type to filter all notes (subsequence match on title/id),
  ↑/↓ to move, Enter to open, Esc to close, or click a row. Deterministic
  (snapshots `query::list` on open); selecting reuses `select`, so it also reveals
  the note in the explorer.
- Selecting a note now reveals it in the left explorer: picking a node in the
  Graph tab (or any cross-navigation) expands the containing folders, highlights
  the file, and scrolls to it — in both the Ideas and Files views. A one-shot
  "reveal" pulse on selection (F-013/F-025 polish).
- F-016 Backlinks + unlinked mentions. `query::backlinks` lists incoming links
  (notes linking *to* the current one — the dual of `related`'s out-links), a
  `links` JOIN on `dst_id`. `query::unlinked_mentions` lists notes that mention
  the title as an FTS phrase but don't link it (minus self, minus already-linked).
  Both deterministic/instant (INV-1/INV-3). `show` prints both; the UI info panel
  shows a **Backlinks** section and an **Unlinked mentions** section with a 🔗
  accept button that writes a resolvable markdown link (`scaffold::link_mention`;
  angle-bracket-wrapped for paths with spaces) into the mentioning note, then
  re-indexes. The Obsidian-style headline feature; accept is the link form of
  F-014's propose→accept.
- F-025 Files view in the left panel. An **Ideas/Files** toggle tops the explorer:
  Ideas is the existing indexed-note tree; Files is a full `walkdir` tree of the
  root (like an IDE explorer — every subfolder and file, with dotfiles/`.phanes`/
  `.git` hidden). Clicking a `.md` opens it (indexed → full info; not-yet-indexed
  → raw view with a "Scan to add" hint); other files open raw and inert. The
  filter box applies to the Ideas view only. Deterministic (no model, no DB for
  the tree); the tree is built lazily and invalidated on re-index. The Scan
  buttons moved to their own row to make space for the toggle.
- AppImage packaging for the desktop app. `packaging/build-appimage.sh` builds
  `phanes-ui` (`--features ui,enrich`) and bundles it into a single portable
  `dist/Phanes-<version>-x86_64.AppImage` via `linuxdeploy` + `appimagetool`,
  with a generated icon and `.desktop` entry (`packaging/appimage/`). Docs in
  `packaging/README.md`; `dist/` is git-ignored. Verified to launch on X11.
- F-015 RAG "Ask" mode. `phanes ask "<question>"` and a UI **Ask** tab answer a
  natural-language question from the notes: embed the question, retrieve the `k`
  nearest notes from the stored vectors (`ask::rank`, deterministic), and have the
  local model answer from those excerpts with `[title]` citations and a clickable
  source list. The one feature that puts the model on a query path, so it is a
  deliberately separate, user-invoked mode — never wired into `search`/`near`/
  `show` (the INV-1 carve-out; boundary recorded in D-016, extending D-015). Needs
  `--features enrich`, a model server, and a prior `index --embed`; graceful on
  any failure (INV-4). In the UI the call runs on a background thread (its own read
  DB connection) so the window stays responsive. `enrich::chat` is now
  `pub(crate)` so `ask` reuses the one chat round-trip. Live-verified against LM
  Studio.
- F-014 Editable / acceptable tags (propose → accept). The info panel's tags
  section is now editable: `×` removes an asserted tag, `✓` accepts a proposed
  (`~`) tag (promotes it to asserted), and an "add tag" field appends one.
  Asserted tags are written to the file's frontmatter `tags:` key via
  `scaffold::set_tags` (updates/inserts the key, or prepends a frontmatter block
  for a header-only note), applied to the live buffer so open edits persist. The
  DB is updated in place via `store::set_asserted_tags` (no full re-index), so the
  note's other proposed tags survive (INV-2). The tag sibling of "Propose →
  accept links"; uses the provenance core directly (F-004).
- Model-proposed bridges (F-013 follow-up): `enrich::propose_bridge` and a
  `phanes bridge <a> <b>` command ask the local model for one idea connecting two
  notes. The first query-time model use — an explicit, opt-in generative action
  outside the instant query paths (D-015; INV-1 reworded). Behind `--features
  enrich`; graceful on failure. Live-verified against LM Studio. Also invocable
  by clicking a dashed gap edge in the graph — the model call runs on a background
  thread (channel back to the UI), so the window stays responsive; the result
  shows in a floating panel. Build the UI with `--features ui,enrich`. A graph
  stats overlay shows notes · links (· clusters · orphans with Gaps on).
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
  a folder has no notes. A ⟳ Scan button in the explorer re-indexes in place
  (deterministic, no model), so new/edited/deleted notes appear without a restart.
  A ✨ Scan + AI button runs a background worker (its own SQLite connection; WAL +
  busy-timeout for safe concurrency) that re-indexes with enrichment + embeddings
  on changed notes — so a new note's proposed tags/summary and semantic/graph
  layers fill in without the CLI, while the UI stays responsive (spinner +
  progress, then an auto-refresh). Still index-time/hash-gated (INV-1).
- Set/change a note's status from the UI: the info panel's status field is a
  dropdown that writes the new asserted status into the file via
  `scaffold::set_status` (replaces the blockquote `> **Status:**` line or a
  frontmatter `status:` key, or inserts one if absent), then re-indexes — so a
  note with no status (`unknown`) can be given one in place.
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
- AI features now work in the AppImage (BUG-004). `reqwest` switched from the
  default OpenSSL (`native-tls`) backend to pure-Rust `rustls-tls`, so no
  `libssl`/`libcrypto` is linked or bundled — the bundled OpenSSL failed to
  initialise inside the AppImage, silently killing every model call (Scan + AI,
  Ask, bridges, questions) while the same build worked under `cargo run`. We only
  talk plain HTTP to a localhost server, so TLS is never exercised either way.
- A deterministic re-index no longer wipes a note's model-proposed data (BUG-003).
  Editing a note (e.g. changing its status, which lives in the blockquote header
  for this corpus) used to rebuild it from asserted facts only and destroy its
  proposed summary/tags/topics, and clear its embedding — emptying the info panel
  and disconnecting it from the graph. The indexer now carries existing proposed
  data forward (`preserve_proposed`) and keeps the embedding; proposed values
  persist until an `--enrich`/`--force` pass refreshes them. Affected Save, the
  status dropdown, accept-mention, the file-watcher, and plain `phanes index`.
- Notes no longer silently lose or miss their embeddings (BUG-002). `upsert` now
  updates in place (`ON CONFLICT DO UPDATE`) instead of `INSERT OR REPLACE`, so a
  re-index preserves a note's vector; the indexer clears a stale vector only when
  content actually changed. Enrichment + embedding moved to gap-fill passes that
  fill any note missing the layer (not just hash-changed ones), so `index
  --enrich --embed` / `Scan + AI` now reach already-indexed notes — fixing
  disconnected nodes in the graph and empty `near`. Added `store::{has_summary,
  has_embedding, clear_embedding}`.
- Model requests now retry on a cold-load transport failure (backoff + connect/
  request timeouts), so the first call after the server JIT-loads a model no
  longer fails — affected enrich, embed, and bridge (IMP-001).
- Silenced the indexer's conditional `unused_mut` warning — the `idea` binding is
  only mutated when `--features enrich` is compiled in.
- Wikilink extraction no longer mistakes TOML table-arrays (`[[shaft]]`) or
  inline code spans for links — it now skips fenced code blocks and code spans
  via pulldown-cmark's offset iterator (BUG-001).
