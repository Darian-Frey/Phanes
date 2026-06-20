> **Status:** Active
> **Provenance:** Shane Hartley (owner/architect), Claude (drafting)
> **Last reviewed:** 2026-06-14
> **Why this status:** Phases 1–2, the Phase 4 UI, and the P3 AI layer (enrichment, embeddings, near, graph/gaps, bridges, ask) shipped through F-015. Candidates F-016–F-024 logged 2026-06-14 from a peer-tool gap analysis.

# Features

The **what**, not the how. See [ARCHITECTURE.md](ARCHITECTURE.md) for structure
and [DECISIONS.md](DECISIONS.md) for rationale. Phases live in
[ROADMAP.md](ROADMAP.md).

## Target users

Shane, and anyone who keeps a folder of project-idea markdown notes following the
scaffold convention (four-field blockquote header, one idea per file) and wants
durable, offline search and relationship surfacing over them — deliberately more
than `rg`, without a cloud service.

## Out of scope

- Splitting one file into multiple ideas — one file = one idea (D-006).
- Acting as a general note-taking app beyond capture (`new`) and edit-in-place.
- Non-markdown source formats; cloud sync; multi-user collaboration.
- Making the local model load-bearing for anything deterministic code can do
  (titles, links, dates, status) — the hub-and-spoke methodology forbids it.

## Features

### F-001 Deterministic indexing
**Priority:** Must
**Acceptance:**
- `phanes index --root <dir>` walks `*.md`, hashes each file (blake3), and skips
  unchanged files on a content-hash match (or reprocesses all under `--force`).
- Frontmatter **and** blockquote-header metadata (status, last_reviewed, tags)
  are parsed as *asserted* facts (D-008).
- Idea records are upserted to SQLite with provenance columns; files that
  vanished since the last pass are pruned.
**Status:** Complete (Phase 1)
**Notes:** The hash gate is INV-1. See ARCHITECTURE.md §indexer. Related: D-001, D-004, D-008.

### F-002 Full-text search with filters
**Priority:** Must
**Acceptance:**
- `phanes search <query>` returns FTS5-ranked hits with highlighted snippets.
- `--status`, `--tag`, `--stale-days`, `--limit` narrow results.
- Multi-word queries AND their terms; arbitrary punctuation never errors.
**Status:** Complete (Phase 1)
**Notes:** Porter stemming matches word variants. Related: D-004.

### F-003 Stale view
**Priority:** Must
**Acceptance:**
- `phanes stale --days N` lists ideas whose `last_reviewed` (or `mtime` fallback)
  is older than N days, oldest first.
**Status:** Complete (Phase 1)

### F-004 Provenance model (asserted vs proposed)
**Priority:** Must
**Acceptance:**
- Every field that can originate from the model carries `Asserted | Proposed`.
- Proposed values fill gaps only and never overwrite asserted ones.
- Provenance is persisted in the DB and surfaced in `show`.
**Status:** Complete (Phase 2) — surfaced in `show` with per-field asserted/proposed flags
**Notes:** INV-2. Related: D-002.

### F-005 Related ideas
**Priority:** Must
**Acceptance:**
- `phanes related <id|title>` lists explicit links first, then shared-tag
  neighbours ranked by tag-overlap count.
**Status:** Complete (Phase 2) — link targets resolve to ids; self-links excluded
**Notes:** Shared-tag links are computed at query time, never stored (INV-3). Related: D-003.

### F-006 Show single idea
**Priority:** Must
**Acceptance:**
- `phanes show <id|title>` renders one idea: metadata, provenance flags per
  field, explicit links, and shared-tag neighbours.
**Status:** Complete (Phase 2) — resolves id-or-fuzzy-title via `query::resolve`

### F-007 New idea capture
**Priority:** Should
**Acceptance:**
- `phanes new <title> [--tag ...]` writes a scaffold note with the blockquote
  header pre-filled (Status: Concept, last_reviewed: today; tags in frontmatter),
  refuses to overwrite an existing note, then indexes and shows it.
**Status:** Complete (Phase 2)
**Notes:** Output follows the scaffold standard (D-006, D-008); new notes default
to Concept (D-011). Generator lives in `scaffold.rs`, round-trips through `parser`.

### F-008 Local-model enrichment (opt-in)
**Priority:** Should
**Acceptance:**
- Built with `--features enrich` and run with `--enrich`, changed files receive a
  proposed summary, tags, topics, and status guess from a local llama-server,
  grammar-constrained to valid JSON.
- A missing, slow, or malformed response never fails an index pass (INV-4).
- **Taxonomy-aware tags:** the existing tag vocabulary (`query::tag_vocabulary`)
  is fed to the model so proposed tags reuse it instead of inventing synonyms.
**Status:** Complete (Phase 3) — OpenAI-compatible client (D-012); live-verified against LM Studio (qwen2.5-7b-instruct) producing proposed summary/tags/topics, with INV-1/2/4 all holding. Taxonomy-aware tags added 2026-06-15.
**Notes:** Related: D-001, D-002, D-007, D-012. The vocabulary is snapshotted once
per pass; new tags converge over runs, and `index --enrich --force` re-enriches
the whole corpus with the full vocabulary to consolidate a sprawling taxonomy.

### F-009 Three-panel desktop UI
**Priority:** Should
**Acceptance:**
- An egui app with three panels: left file/ideas explorer, centre note
  reader/editor, right idea/provenance/relationship info.
**Status:** Complete (Phase 4) — `phanes-ui` (eframe 0.34): explorer (folder tree + filter + ⟳ Scan / ✨ Scan + AI to re-index in place, the latter running enrich+embed on a background thread), centre editor, and a right info panel (status dropdown to set/change the asserted status, provenance/tags/topics + clickable related)
**Notes:** Frontend over the same `query`/`indexer` API; invariants unchanged. Related: D-005.

### F-010 Edit-in-place with re-index on save
**Priority:** Should
**Acceptance:**
- Editing a note in the centre panel and saving triggers a one-file index pass;
  enrichment fires only on save, under the hash gate (INV-1).
**Status:** Complete (Phase 4) — View/Edit toggle + explicit Save (button / Ctrl+S) → write + one-file re-index; verified interactively
**Notes:** Related: D-005.

### F-011 Tinted table output
**Priority:** Could
**Acceptance:**
- CLI list output is a bordered table with per-status colour tints, emitted only
  when stdout is a TTY (clean when piped).
**Status:** Complete (Phase 1)

### F-012 Semantic "near this"
**Priority:** Should
**Acceptance:**
- `phanes index --embed` (with `--features enrich`) stores one embedding vector
  per changed note via a local embedding model.
- `phanes near <id|title>` lists the most cosine-similar notes (with a `% similar`
  score), excluding itself; the UI info panel shows a "Near (semantic)" section.
- Similarity is computed at query time from stored vectors — no model runs on a
  query (INV-1); a failed embed leaves a note vector-less, never failing the pass
  (INV-4).
**Status:** Complete (post-roadmap) — live-verified on the 28-note corpus (nomic-embed-text, 768-dim)
**Notes:** Vectors are note data; the neighbours are computed, not stored (INV-3).
Related: D-001, D-003, D-012, D-013.

### F-013 Relationship graph view + gap analysis
**Priority:** Could
**Acceptance:**
- The UI `Graph` tab renders the relationship layer (explicit links + shared tags
  + semantic) as a force-directed, status-tinted node graph: pan/zoom, hover
  labels, drag a node (neighbours spring along, then settle), click to select.
- `phanes gaps` lists orphan ideas and candidate bridges (strong semantic pairs
  not explicitly linked); `graph::{components, orphans, bridges}` compute these.
- A "Gaps" toggle overlays orphans (ringed + labelled) and the top candidate
  bridges (dashed, `%`-labelled) directly on the graph.
- `phanes bridge <a> <b>` (and clicking a dashed gap edge in the UI) asks the
  local model for one idea connecting two notes — an explicit, opt-in generative
  action outside the instant query paths (D-015). In the UI the call runs on a
  background thread, so the window stays responsive.
- A canvas stats overlay shows notes · links (· clusters · orphans with Gaps on).
**Status:** Complete (post-roadmap) — hand-rolled (D-014); collision-force layout;
model-proposed bridges via the `bridge` command and by clicking a gap edge in the
UI (`--features ui,enrich`).
**Notes:** The graph is rebuilt from the index, never stored (INV-3); semantic
edges use the stored vectors (INV-1). Related: D-013, D-014.

### F-014 Editable / acceptable tags (propose → accept)
**Priority:** Should
**Acceptance:**
- The info panel's tags section is editable: each asserted tag has a `×` to
  remove it, each proposed (`~`) tag has a `✓` to **accept** it (promote to
  asserted), and an "add tag" field appends a new asserted tag.
- Asserted tags are the file's frontmatter `tags:` list. An edit writes the new
  set there via `scaffold::set_tags` (updates an existing key, inserts into
  existing frontmatter, or prepends a frontmatter block for a header-only note),
  applied to the live buffer so open edits persist.
- The DB stays in sync via `store::set_asserted_tags` (replace asserted rows;
  `INSERT OR REPLACE` promotes an accepted proposed tag in place) — no full
  re-index, so the note's other proposed tags survive (INV-2).
**Status:** Complete (post-roadmap). The tag sibling of "Propose → accept links";
the link version remains a candidate.
**Notes:** Accept = write the proposed value to the file, making it asserted on
the model's next pass too (`merge_proposed` skips already-asserted tags). Related:
F-004, F-008, INV-2.

### F-015 RAG "Ask" mode
**Priority:** Could
**Acceptance:**
- `phanes ask "<question>"` (and a UI **Ask** tab) answers a natural-language
  question from the notes: embed the question, retrieve the `k` most cosine-
  similar notes from the stored vectors (`ask::rank`), and have the local model
  answer from those excerpts, citing note titles in `[brackets]`.
- The answer lists its source notes (with `%` similarity); in the UI they are
  click-through to the note. Needs the `enrich` build, a model server, and a prior
  `index --embed`.
- It is the **only** feature that puts the model on a query path, so it is a
  deliberately separate, user-invoked mode — never wired into `search`/`near`/
  `show`. The INV-1 carve-out (see D-016, extending D-015). Graceful on any
  failure (no embeddings, server down) — `Err` reported, never a crash (INV-4).
- In the UI the model call runs on a background thread (its own read DB
  connection), so the window stays responsive.
**Status:** Complete (post-roadmap). The one candidate flagged as breaking INV-1;
shipped as a bounded, opt-in mode per the boundary recorded in D-016.
**Notes:** Retrieval is deterministic over the index-time embeddings (INV-1 holds
for retrieval); only generation runs on demand. Related: F-008, F-012, D-016.

### F-026 In-app manual viewer
**Priority:** Should
**Acceptance:**
- A `?` button in the centre toolbar (and the F1 key) opens the user manual,
  rendered in the centre pane via the markdown viewer; Close / F1 dismisses it.
- The manual is read-only and never treated as an indexed note.
**Status:** Complete (post-roadmap). `MANUAL.md` is embedded with `include_str!`,
so it ships inside the binary / AppImage (no dependency on the repo file at
runtime); rendered with `egui_commonmark`, kept separate from the View/Edit/Graph/
Ask views.
**Notes:** Reuses the existing markdown viewer (F-010). Related: MANUAL.md.

### F-027 Colour themes
**Priority:** Could
**Acceptance:**
- A theme picker (top-bar 🎨 dropdown) switches the whole UI between **Dark**,
  **Light**, **Parchment**, **Cyberpunk**, **Orphic**, **Nord**, **Solarized**,
  and **Gruvbox**; the choice persists across runs.
- Each theme is a full palette (`apply_theme` → egui `Visuals`); Parchment adds a
  bundled **serif** font, Cyberpunk uses the built-in **monospace**.
- Semantic colours (status / cluster / graph edges / proposed) stay legible on
  every theme — they switch bright↔dark via a thread-local background flag.
**Status:** Complete (post-roadmap). Themes applied via `apply_theme(ctx, theme)`;
persisted to `$XDG_CONFIG_HOME/phanes/theme` (global). Parchment serif is
DejaVu Serif, bundled (`assets/fonts/`, redistributable) and compiled in with
`include_str!`/`include_bytes!` so it ships in the AppImage.
**Notes:** A new top bar hosts the picker. The graph canvas accents (focus,
gaps, edges, labels) are theme-aware too. Orphic is the on-brand one (cosmic
indigo + luminous gold — the primordial-light identity of the name); Nord /
Solarized / Gruvbox are well-known palettes. New palettes are one match arm in
`apply_theme` each.

## Candidate features (uncommitted)

Ideas not committed to. Most come from a 2026-06-11 survey of local-LLM note
tools (Reor, Khoj, Smart Connections, InfraNodus, LM Studio). Grouped by how they
sit with the invariants: most fit **INV-1** (model at index time; queries instant
and offline); generative ones are flagged under the D-015/D-016 carve-out.

The entries below (**F-016–F-024**) carry reserved, append-only IDs with
`Status: Candidate` until committed — logged 2026-06-14 from a gap analysis
against peer tools (Obsidian, Reor, InfraNodus, Mem, Atlas, DEVONthink). The
looser idea bullets further down stay unnumbered until they firm up.

### F-016 Backlinks — linked and unlinked mentions
**Priority:** Should
**Acceptance:**
- A panel/command shows **incoming** links (notes whose links resolve to the
  current note), distinct from `related`'s outgoing links + shared-tag neighbours.
- **Unlinked mentions:** notes that contain the current note's title (or an alias)
  in prose but don't link it, surfaced as *proposed* links the user can accept —
  which writes a real link into the file (INV-2).
- Both deterministic: incoming links are a `links` JOIN on `dst_id`; unlinked
  mentions are an FTS title scan. No model, instant (INV-3).
**Status:** Complete (post-roadmap). `query::backlinks` (incoming links) and
`query::unlinked_mentions` (FTS phrase match minus already-linked, minus self).
`show` prints both; the UI info panel shows a **Backlinks** section and an
**Unlinked mentions** section with a 🔗 accept button. Accept writes a resolvable
markdown link (`scaffold::link_mention`, angle-bracket-wrapped for paths with
spaces) into the *mentioning* note, then re-indexes.
**Notes:** Obsidian's headline feature (Backlinks core plugin = Linked + Unlinked
mentions). The unlinked→accept path is the link form of F-014's propose→accept.
Caveat: a single common-word title (e.g. "Threshold") yields noisy phrase matches
— it's a vetted suggestion list, accept is manual, mirroring Obsidian.

### F-017 Quick switcher / command palette
**Priority:** Should
**Acceptance:**
- A keyboard-driven fuzzy switcher (e.g. `Ctrl+P`) jumps to any note by title or id
  from anywhere in the UI.
- Optionally a command palette for actions (Scan, toggle mode, new note, Ask).
**Status:** Complete (post-roadmap, note-jump). `Ctrl/Cmd+P` opens a centered
overlay over any view: fuzzy-filters all notes (subsequence match on title/id,
`fuzzy_score`), ↑/↓ to move, Enter to open, Esc to close; clicking a row opens it.
Selecting reuses `select` (so it also reveals the note in the explorer, F-013/F-025
polish). The command-palette-for-actions half is not built yet.
**Notes:** Obsidian Quick Switcher. Deterministic; snapshots `query::list` on open.

### F-018 Tag browser
**Priority:** Could
**Acceptance:**
- A pane listing the full tag vocabulary with per-tag note counts; click a tag to
  filter the explorer.
- Distinguishes asserted from proposed tag usage.
**Status:** Complete (post-roadmap). `query::tag_index` returns every tag with its
asserted/proposed counts and the notes carrying it. The left explorer gains a
**Tags** view (third toggle): tags listed by use, each a collapsing header
(`tag · total  (~proposed)`) expanding to its notes (click to open). `phanes tags`
prints the same vocabulary with counts.
**Notes:** Obsidian tag pane. Deterministic (INV-3). Complements the taxonomy-aware
proposed-tags idea — surfacing the vocabulary (e.g. 77 mostly-singleton proposed
tags on the test corpus) is what makes that worth doing.

### F-019 Live file-watching auto-reindex
**Priority:** Could
**Acceptance:**
- The UI watches the root for file create/modify/delete and re-indexes
  incrementally (deterministic, hash-gated), removing the need to press ⟳ Scan.
- Debounced; never runs the model automatically (INV-1).
**Status:** Complete (post-roadmap). `notify` recursive watcher (UI-gated dep)
filters to `.md` create/modify/remove outside dotfolders (so `.phanes/` DB writes
can't loop), pings a channel, and `ctx.request_repaint`s to wake an idle window.
`poll_watch` debounces ~500 ms, skips while a Scan + AI runs, and only refreshes
when the index actually changed (so the app's own saves cause no churn). The ⟳
Scan button stays as a manual fallback. Live-verified (create → auto-reindex; no
loop on DB writes).
**Notes:** Matches the "always current" expectation Obsidian and peers meet by
default. The watcher callback must `request_repaint` — an idle egui window won't
repaint just because a channel got a message.

### F-020 Graph analytics — hubs and clusters
**Priority:** Could
**Acceptance:**
- Compute and surface centrality (degree / betweenness) to highlight **hub** and
  **bridge** notes in the graph.
- Community detection (e.g. modularity / Louvain) to label **topical clusters**;
  tint or group nodes by cluster.
**Status:** Complete (post-roadmap). `graph::betweenness` (Brandes, normalised) and
`graph::communities` (weighted label propagation, deterministic). The UI Graph tab
gains a **Clusters** toggle: nodes are coloured by community and sized by
centrality (hubs bigger); the stats overlay shows the cluster count. `phanes gaps`
prints a **Hubs** list (top by betweenness) and a **Clusters** summary.
**Notes:** InfraNodus's signature (betweenness + community clusters + structural
gaps). Deterministic extension of F-013; folds in the former "cluster + orphan
overview" idea. Label propagation rather than full Louvain — adequate for tinting,
far simpler.

### F-021 Hybrid search (keyword + semantic)
**Priority:** Could
**Acceptance:**
- `search` optionally blends FTS rank with embedding similarity (reciprocal-rank
  fusion or a weighted mix), improving recall over either alone.
- Retrieval-time only; the vectors are index-time, so no model runs on the path
  (INV-1 holds).
**Status:** Complete (post-roadmap). `query::hybrid` fuses (RRF) the FTS results
with notes **semantically near the top keyword matches** — cosine over the stored
vectors. The query is *never* embedded, so search stays offline (INV-1, the
load-bearing reading of the acceptance). `phanes search --semantic` and a
**Semantic** checkbox by the explorer filter; falls back to plain FTS with no
embeddings / no hits / a metadata filter set.
**Notes:** Not classic BM25 + dense-query retrieval — that would put a model call
on the `search` path, which INV-1 forbids. Instead it expands recall via the
index-time semantic graph: keyword hits seed, their neighbours fill in. For pure
query-meaning retrieval, `near` and `ask` already exist. Builds on F-002 + F-012.

### F-022 Timeline view
**Priority:** Could
**Acceptance:**
- A chronological view of notes by created / last-reviewed / modified date; pairs
  with `stale` to show momentum and rot over time.
**Status:** Complete (post-roadmap). `query::timeline` orders notes by effective
date (last-reviewed, else modified — the same COALESCE as `stale`), newest first.
The left explorer gains a **Timeline** view (fourth toggle) grouped by month;
`phanes timeline` prints the same on the CLI.
**Notes:** Deterministic (INV-3), over existing date metadata; built lazily and
invalidated on re-index. No "created" date is tracked, so effective date =
last-reviewed or file mtime.

### F-023 Auto-classify / "see also" (index-time, proposed)
**Priority:** Could
**Acceptance:**
- At index time the model proposes a category/grouping (and optionally a target
  folder) per note, stored as *proposed* (INV-2) and surfaced in `show` / the info
  panel.
**Status:** Complete (post-roadmap). Enrichment now also proposes a single coarse
**category** (the kind of note — developer-tool, research, creative, spec…), a new
`model::Idea.category` / `Enrichment.category` field stored in the `ideas` table
(`category`/`category_source`; added by a lightweight `ALTER TABLE` migration).
`show` and the UI info panel display it with its provenance badge; preserved
across deterministic re-indexes (BUG-003). Target-folder suggestion not built.
**Notes:** DEVONthink "See Also & Classify"; Mem self-organising. Index-time and
proposed — a sibling of proposed tags (F-008). The category json_schema field is
required; `Enrichment.category` has `#[serde(default)]` for older replies.

### F-024 Generated open questions per cluster (opt-in, generative)
**Priority:** Could
**Acceptance:**
- A user-invoked action asks the model "what's unexplored / what questions does
  this region raise?" for a selected cluster or the whole corpus, presented as
  prompts (not written to files).
- An explicit generative action under the D-015/D-016 carve-out; graceful on
  failure (INV-4).
**Status:** Complete (post-roadmap). `enrich::propose_questions` feeds a cluster's
note titles + summaries to the model and returns open questions. `phanes questions`
runs it over the whole corpus; the Graph tab's **❓ Questions** button runs it for
the focused node's cluster (or the whole corpus), on a background thread with a
floating result window. The third query-time generative action (after bridge,
ask) — never on an instant path; questions are shown, never written to files.
**Notes:** Extends bridges (F-013) from "connect two notes" to "what's missing in a
region." InfraNodus frames structural gaps as "potential for new ideas."
Live-verified: surfaced cross-pollination questions across the corpus.

### F-025 Left panel: Files view alongside Ideas view
**Priority:** Should
**Acceptance:**
- A toggle in the left explorer switches between **Ideas** (the current
  indexed-note tree — status-tinted, built from `query::list`) and **Files** (a
  full recursive filesystem tree of the root, like an IDE explorer, showing every
  file and subfolder including non-`.md` files and attachments).
- Folders expand/collapse; clicking a `.md` file selects it (path → id) and drives
  the centre/right panels as today; non-`.md` files are shown (optionally
  open-externally), so attachments and not-yet-indexed files are visible.
- Deterministic: a `walkdir` of the root; the tree needs no model and no DB.
**Status:** Complete (post-roadmap). An **Ideas/Files** toggle tops the left
panel; Files shows the full `walkdir` tree (dotfiles/`.phanes`/`.git` hidden).
Clicking a `.md` opens it (indexed → full info; not-yet-indexed → raw view with a
"Scan to add" hint); other files open raw and inert. The filter box applies to
Ideas only.
**Notes:** Mirrors the VS Code / Antigravity file explorer. Ideas view stays the
default (semantic, status-aware); Files view is for seeing the raw folder. Pairs
with F-019 (live file-watching) and reuses the explorer's tree renderer with a
filesystem source instead of `query::list`. The tree is built lazily and
invalidated on every re-index.

### Fits the index-time / proposed model (queries stay instant + offline)

- ~~Semantic "near this"~~ — **shipped as F-012.**
- ~~**Taxonomy-aware proposed tags.**~~ — **shipped** (refinement of F-008, 2026-06-15):
  the existing tag vocabulary is fed to the model so proposed tags reuse it rather
  than inventing synonyms.
- **Propose → accept links.** Suggested links (from the model or embeddings) show
  as *proposed*; one action promotes a link to *asserted* and writes it to the
  file. Uses the provenance core directly (INV-2) — the Phanes-specific angle no
  surveyed tool has. The **tag** form of this shipped as F-014; the unlinked-
  mentions source for link suggestions is now **F-016**.
- **Auto-summary / TL;DR** surfaced atop the centre pane and in the info panel
  (part of F-008).
- **Near-duplicate / merge detection** over the embedding vectors (deterministic;
  flags overlapping notes as merge candidates).
- **Title / filename suggestions** for poorly-named notes (proposed).

### Spatial / graph layer (matches the spatial-first preference)

- Graph / map view and gap detection — **shipped as F-013** (force-directed UI
  graph + `gaps` CLI + on-canvas gap overlay). Remaining: optionally have the
  model *propose* a bridging idea per detected gap.
- **Stale triage with a proposed next step** — each rotting note (from `stale`)
  gets a proposed revival prompt / next action.
- **Cluster + orphan overview** — surface dense clusters and unconnected notes
  (deterministic graph metrics). → folded into **F-020** (graph analytics).

### Powerful but breaks INV-1 — only as a bounded, opt-in mode

- ~~**"Ask" / RAG chat over the corpus** with citations~~ — **shipped as F-015**
  (the boundary is recorded in D-016). Retrieval reuses the index-time embeddings;
  only generation runs on demand, in a deliberately separate user-invoked mode.

### Smaller

- Per-idea `open` in `$EDITOR`.
- **AI flashcards** (Reor has these) — generate spaced-repetition cards from a
  note. Index-time, proposed. Noted for completeness but a weak fit: Phanes is a
  project-idea tool, not a study/recall tool.
