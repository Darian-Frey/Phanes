> **Status:** Active
> **Provenance:** Shane Hartley (decisions), Claude (recording)
> **Last reviewed:** 2026-06-10
> **Why this status:** Living log; appended as decisions are made.

# Decisions

Append-only log of significant design decisions. Each entry: D-NNN, with Decided
and Recorded dates (ISO 8601), status, context, options, decision, consequences,
and reversal conditions.

Status vocabulary: Proposed | Accepted | Superseded by D-NNN | Deprecated.

Entries D-001..D-006 capture decisions baked into the original scaffold; they
were recorded together on 2026-06-10 when this log was created. D-007..D-009 were
made during active development that same day.

### D-001 Enrichment model runs at index time only
**Decided:** 2026-06-10
**Recorded:** 2026-06-10
**Status:** Accepted
**Authors:** Shane Hartley
**Related:** F-001, F-008, ARCHITECTURE.md §Key invariants (INV-1)

**Context.** Phanes is meant to be a fast, offline daily-driver. An LLM in the
query path would make `search`/`stale`/`related` slow and network-dependent.

**Options.**
- **A. Enrich lazily at query time.** Rejected: every query waits on the model;
  not offline; non-deterministic results.
- **B. Enrich at index time, cached by content hash.** Chosen.

**Decision.** The model is invoked only in `indexer::run`, only for files whose
content hash changed (or under `--force`). Queries read cached, resolved records.

**Consequences.**
- Queries are instant and offline; a re-index of an unchanged corpus costs ~zero
  model calls.
- Enrichment freshness is bounded by index passes, which is acceptable.

**Reversal conditions.** Revisit if instant/offline queries cease to be a
requirement, or if per-query enrichment becomes cheap enough to be invisible.

### D-002 Proposed never overwrites asserted
**Decided:** 2026-06-10
**Recorded:** 2026-06-10
**Status:** Accepted
**Authors:** Shane Hartley
**Related:** F-004, F-008, ARCHITECTURE.md (INV-2)

**Context.** Deterministic facts (title, links, dates, author-declared status)
must remain authoritative even when the model also has an opinion.

**Options.**
- **A. Trust model output as canonical.** Rejected: silently corrupts known facts.
- **B. Tag every value with provenance; proposed fills gaps only.** Chosen.

**Decision.** `Sourced<T>` carries `Asserted | Proposed`. `merge_proposed` and
`store::upsert` only let proposed values fill absent fields; provenance is
persisted (`status_source`, `summary_source`, per-tag `source`).

**Consequences.**
- The asserted/proposed boundary survives a round trip through SQLite and is
  surfaceable in `show`.
- Slightly more schema and merge logic.

**Reversal conditions.** Revisit only if model output becomes trusted enough to
be treated as canonical for a given field — which would itself be a new decision.

### D-003 Relationships computed at query time, not stored
**Decided:** 2026-06-10
**Recorded:** 2026-06-10
**Status:** Accepted
**Authors:** Shane Hartley
**Related:** F-005, ARCHITECTURE.md (INV-3)

**Context.** Shared-tag adjacency changes whenever any note's tags change; stored
adjacency would go stale.

**Options.**
- **A. Materialise a neighbours table at index time.** Rejected: stale on any tag
  edit; needs invalidation logic.
- **B. Compute shared-tag neighbours by JOIN at query time.** Chosen. Only the
  explicit `links` table is persisted.

**Decision.** `related` derives shared-tag neighbours from a `tags` self-JOIN at
query time; explicit out-links are the only stored relationship.

**Consequences.**
- Relationships can never be stale.
- `related` pays a JOIN per call (negligible at expected corpus sizes).

**Reversal conditions.** Revisit if the corpus grows large enough that the
query-time JOIN is too slow to be interactive.

### D-004 SQLite + FTS5 for storage and search
**Decided:** 2026-06-10
**Recorded:** 2026-06-10
**Status:** Accepted
**Authors:** Shane Hartley
**Related:** F-001, F-002

**Context.** Need durable local storage plus ranked full-text search, offline,
with no external services.

**Options.**
- **A. Plain files + `rg`/grep.** Rejected: no ranked FTS, no metadata queries —
  the whole point is to do more than grep.
- **B. A dedicated search engine (e.g. tantivy).** Rejected for now: heavier, and
  we also need relational metadata.
- **C. SQLite with the bundled FTS5 amalgamation.** Chosen: one embedded file,
  porter stemming, relational metadata and FTS in the same store.

**Decision.** `rusqlite` with `bundled` features; schema in `sql/schema.sql`;
search via an `ideas_fts` FTS5 virtual table.

**Consequences.**
- Single-file index at `<root>/.phanes/index.db`; offline; stemmed search.
- FTS sync is manual (virtual table has no FK cascade) — handled in `upsert`.

**Reversal conditions.** Revisit if semantic search becomes the primary need, or
the corpus outgrows single-file SQLite.

### D-005 egui for the three-panel desktop UI
**Decided:** 2026-06-10
**Recorded:** 2026-06-10
**Status:** Accepted
**Authors:** Shane Hartley
**Related:** F-009, F-010

**Context.** Shane wants a three-panel app (explorer / editor / info) rather than
a CLI-only tool, while keeping the project pure Rust.

**Options.**
- **A. ratatui TUI.** Terminal-native and keyboard-first, but limited markdown
  rendering and in-place editing.
- **B. egui (native, immediate-mode).** Chosen: pure Rust, single binary, native
  three-panel layout, `egui_commonmark` for the centre pane.
- **C. Tauri/Dioxus webview.** Best rendering, but adds a JS/web toolchain and
  departs from pure-Rust durability.

**Decision.** Build the UI as a second binary behind a `ui` feature, over the
same `query`/`indexer` API. Centre pane edits in place and re-indexes on save
(F-010), preserving INV-1.

**Consequences.**
- The default CLI build stays lean; the UI is opt-in.
- Markdown editing is a textarea + rendered view rather than a rich editor.

**Reversal conditions.** Revisit if rich markdown rendering/editing needs exceed
what egui can comfortably provide.

### D-006 One file = one idea
**Decided:** 2026-06-10
**Recorded:** 2026-06-10
**Status:** Accepted
**Authors:** Shane Hartley
**Related:** F-001, F-007

**Context.** Some notes in the test corpus packed many ideas into one file. The
id, title, links, and schema all assume a single idea per file.

**Options.**
- **A. Split multi-idea files into separate ideas.** Rejected for now: ripples
  through ids, link resolution, and the schema; the multi-idea files were an
  extreme edge case, and the corpus will follow the scaffold standard going
  forward.
- **B. Treat each file as exactly one idea.** Chosen.

**Decision.** Each `*.md` file maps to one `Idea` (id from path, title from first
H1). Multi-idea files index as one idea with sub-ideas as searchable body text.

**Consequences.**
- Simple, stable ids and a clean schema.
- Multi-idea files are under-represented until/unless split.

**Reversal conditions.** Revisit if the corpus shifts to predominantly
multi-idea files such that per-file granularity loses real information.

### D-007 Add Concept and Draft to the idea Status enum
**Decided:** 2026-06-10
**Recorded:** 2026-06-10
**Status:** Accepted
**Authors:** Shane Hartley
**Related:** F-001, F-008

**Context.** The real corpus's most common status is `Concept` (11 of 28 notes),
plus a `Draft`; the original six-value enum mapped both to `Unknown`.

**Options.**
- **A. Keep the enum; map Concept/Draft → Unknown.** Rejected: the single most
  common status would read as Unknown everywhere until enrichment.
- **B. Extend the enum with Concept and Draft.** Chosen.

**Decision.** Add `Concept` and `Draft` to `model::Status` (enum, `as_str`,
`FromStr`) and to `grammars/idea_extract.gbnf`, kept in lockstep.

**Consequences.**
- The deterministic core represents the corpus's real statuses as first-class.
- The enrichment grammar can also emit them.
- This is the *idea-note* status vocabulary; it is distinct from the *project-doc*
  status header vocabulary {Active|Dormant|Complete|Archived|Superseded}.

**Reversal conditions.** Revisit if the corpus stops using these statuses, or if
the vocabulary should instead be data-driven rather than a fixed enum.

### D-008 Parse both YAML frontmatter and the blockquote header
**Decided:** 2026-06-10
**Recorded:** 2026-06-10
**Status:** Accepted
**Authors:** Shane Hartley
**Related:** F-001, F-007

**Context.** The parser stub planned for YAML frontmatter, but 100% of the real
corpus uses the project's four-field blockquote header instead. YAML-only would
extract no status or dates from any real note.

**Options.**
- **A. YAML frontmatter only (the stub plan).** Rejected: useless on the real
  corpus.
- **B. Blockquote header only.** Rejected: the committed `examples/` and `new`
  output use frontmatter; both conventions should work.
- **C. Parse both; frontmatter wins where present, blockquote fills gaps.** Chosen.

**Decision.** `parser::parse` reads YAML frontmatter (gray_matter) and the
blockquote header (`> **Status**:`, `> **Last reviewed**:`), tolerating trailing
prose and parenthetical dates, with field matching anchored at line start.

**Consequences.**
- Both the scaffold standard and frontmatter notes index correctly.
- Two code paths to maintain in the parser.

**Reversal conditions.** Revisit if the corpus standardises on exactly one
convention, making the other dead code.

### D-009 Adopt the Development Documentation Standard
**Decided:** 2026-06-10
**Recorded:** 2026-06-10
**Status:** Accepted
**Authors:** Shane Hartley
**Related:** README.md, development_documentation.md

**Context.** Shane wants Phanes documented to his house standard
(`development_documentation.md`) for legibility to humans and tooling.

**Options.**
- **A. Ad-hoc docs.** Rejected: drifts; no stable IDs or audit trail.
- **B. Full Tier 1 + ARCHITECTURE + DECISIONS now; remaining Tier 2 later.** Chosen.

**Decision.** Create Tier 1 (README, FEATURES, ROADMAP, CLAUDE, CHANGELOG,
LICENSE) plus ARCHITECTURE.md and DECISIONS.md, following the standard's formats
and Maintenance Rules. Defer BUILD.md, BUGS.md, IMPROVEMENTS.md, and SPEC.md
until friction warrants them (Tier 2/3 are case-by-case; no exemption entry
required).

**Consequences.**
- Stable IDs (F-/D-) and append-only history from here on.
- Maintenance Rule 8 now applies once BUGS.md/IMPROVEMENTS.md exist: discoveries
  are logged, not silently acted on.

**Reversal conditions.** Revisit the deferral if a deferred doc's absence starts
costing real time (e.g. build steps multiply → BUILD.md; recurring defects →
BUGS.md).

### D-010 Build the UI (Phase 4) before enrichment (Phase 3)
**Decided:** 2026-06-10
**Recorded:** 2026-06-10
**Status:** Accepted
**Authors:** Shane Hartley
**Related:** F-008, F-009, F-010, ROADMAP.md, D-005

**Context.** The roadmap lists enrichment (Phase 3) before the desktop UI
(Phase 4). Shane prefers to see the three-panel app over the deterministic core
first. A smoke test (2026-06-10) also showed enrichment needs a server-protocol
decision (candidate D-011) that can safely wait.

**Options.**
- **A. Roadmap order — enrichment then UI.** Rejected for now: the UI delivers
  visible value sooner and doesn't depend on the model.
- **B. UI before enrichment.** Chosen. Execution order becomes Phase 1 → 2 → 4 → 3.

**Decision.** After Phase 2 (relationships, `show`, `new`), build the egui UI
(Phase 4) before enrichment (Phase 3). Phase numbers are unchanged (append-only);
only execution order changes.

**Consequences.**
- The UI ships over the fully deterministic core; enrichment's proposed data
  later lands in an already-built interface.
- The enrichment server-protocol decision (D-011) is deferred alongside Phase 3.

**Reversal conditions.** Revisit if UI work stalls, or if enrichment becomes
needed before the UI (e.g. proposed tags are wanted to make `related` useful on
the tag-sparse corpus).

### D-011 New notes use the blockquote scaffold format with Status: Concept
**Decided:** 2026-06-10
**Recorded:** 2026-06-10
**Status:** Accepted
**Authors:** Shane Hartley
**Related:** F-007, D-006, D-007, D-008

**Context.** The `new` command must emit a note `parser::parse` reads back. The
original stub suggested YAML frontmatter with `status: active`, but the corpus
convention (D-008) is the blockquote header, and a freshly captured idea is best
described as a Concept (D-007, the corpus's most common status).

**Options.**
- **A. YAML frontmatter, `status: active` (the stub plan).** Rejected: doesn't
  match the corpus style, and `active` overstates a just-captured idea.
- **B. Blockquote header, `Status: Concept`; `--tag` values as frontmatter.**
  Chosen. Tags need a channel the parser reads as asserted, which is frontmatter;
  with no tags the note is pure blockquote, matching the corpus exactly.

**Decision.** `scaffold::note_body` emits the four-field blockquote header
(Status: Concept, Provenance, Last reviewed: today, Why) plus the H1 title, with
an optional YAML frontmatter block for asserted tags. `scaffold::filename`
sanitizes the title; `new` refuses to overwrite an existing note, then indexes
and shows it.

**Consequences.**
- New notes match the corpus style and default to a realistic status.
- Tag-bearing notes carry a small frontmatter block (the only asserted-tag
  channel), so they are not byte-identical to pure-blockquote corpus notes.
- The generator lives in the library (`scaffold.rs`) and is unit-tested by
  round-tripping through `parser::parse`.

**Reversal conditions.** Revisit if `active` turns out to be the better default,
or if asserted tags should be expressed some other way than frontmatter.

### D-012 Enrichment targets the OpenAI-compatible API, not llama.cpp native
**Decided:** 2026-06-11
**Recorded:** 2026-06-11
**Status:** Accepted
**Authors:** Shane Hartley
**Related:** F-008, ARCHITECTURE.md, INV-4

**Context.** `enrich.rs` was written against llama.cpp's native `/completion`
endpoint with a GBNF grammar, but a 2026-06-10 smoke test found the only local
stack on the machine is LM Studio, which exposes the **OpenAI-compatible** API
(`/v1/chat/completions`, structured output via `response_format` json_schema) and
ships no `llama-server` binary.

**Options.**
- **A. Install llama.cpp `llama-server`** and keep `enrich.rs` + GBNF unchanged
  (token-level grammar guarantee). Rejected for now: extra software to install
  and run; doesn't use the stack already present.
- **B. Retarget `enrich.rs` to the OpenAI-compatible API** with json_schema
  structured output. Chosen: works with LM Studio (present), and also Ollama and
  llama.cpp's own OpenAI mode — far more portable.

**Decision.** `enrich::enrich` POSTs to an OpenAI-compatible chat endpoint
(default LM Studio `http://127.0.0.1:1234/v1/chat/completions`, override via
`PHANES_LLM_URL` / `PHANES_LLM_MODEL`) with a `response_format` json_schema that
mirrors `model::Enrichment`. The reply is `choices[0].message.content`, parsed to
`Enrichment`. Graceful degradation (INV-4) is unchanged.

**Consequences.**
- No new software to install; uses the existing LM Studio + model.
- json_schema is now the active output constraint; `grammars/idea_extract.gbnf`
  is retained only for the optional llama.cpp-native path. Both must stay in
  lockstep with `model::Status` (a small ongoing cost — two schema definitions).
- Enrichment needs an OpenAI-compatible server running; LM Studio's daemon must
  be started from the desktop app (the `lms` CLI can't bootstrap it headless).

**Reversal conditions.** Revisit if json_schema structured output proves
unreliable on the chosen server (fall back to A, llama.cpp native + GBNF), or if
maintaining two schema definitions becomes error-prone (derive one canonical
schema, e.g. via `schemars`).

### D-013 Semantic search: one vector per note, brute-force cosine at query time
**Decided:** 2026-06-12
**Recorded:** 2026-06-12
**Status:** Accepted
**Authors:** Shane Hartley
**Related:** F-012, D-001, D-003, D-012

**Context.** Semantic "near this" (F-012) needs note embeddings and a similarity
search. The design choices are where vectors live and how the search runs.

**Options.**
- **A. A dedicated vector store / ANN index** (LanceDB, FAISS, hnsw). Rejected
  for now: a heavy dependency for a personal corpus of hundreds–low-thousands of
  notes, and it would split the index out of the single SQLite file.
- **B. One embedding per note as a SQLite BLOB; brute-force cosine in Rust at
  query time.** Chosen: trivial, no new dependency, exact, and fast enough at
  this scale (N × 768 floats per query).

**Decision.** Compute one vector per note at index time (`--embed`, behind the
`enrich` feature, via the OpenAI `/v1/embeddings` endpoint — D-012). Store it in
an `embeddings` table (little-endian f32 BLOB, FK cascade to `ideas`).
`query::near` loads all vectors and ranks by cosine — deterministic, no model on
the query path (INV-1); the neighbours are computed, not stored (INV-3).

**Consequences.**
- No vector-DB dependency; the index stays one SQLite file.
- Brute-force is O(N·dim) per query — comfortable to low thousands of notes.
- One vector per note (no chunking): a long note is summarised by a single
  embedding.
- A changed note's vector is cascade-cleared on re-index and only recomputed
  under `--embed`.

**Reversal conditions.** Revisit if the corpus outgrows brute-force comfort (add
an ANN index) or if per-note granularity proves too coarse (chunk + average, or
store multiple vectors per note).

### D-014 Hand-roll the graph view rather than use egui_graphs/petgraph
**Decided:** 2026-06-12
**Recorded:** 2026-06-12
**Status:** Accepted
**Authors:** Shane Hartley
**Related:** F-013, D-005

**Context.** The relationship graph view (F-013) needs node-edge rendering, a
layout, and structural analysis (components, orphans, bridges).

**Options.**
- **A. `egui_graphs` crate.** Rejected: its latest (0.30) tracks egui 0.30, but
  we're on egui 0.34 (via eframe) — pulling it in would dual-version egui and
  fail to compile.
- **B. `petgraph` for the algorithms + a custom renderer.** Rejected for now: the
  algorithms we need (connected components, degree, bridge = semantic-not-linked)
  are a few lines of union-find; petgraph would be a dependency for little gain.
- **C. Fully hand-rolled.** Chosen: a Fruchterman-Reingold force-directed layout
  drawn with `egui::Painter`, union-find for components, alpha-cooling to settle,
  and node-drag interaction. Zero new dependencies.

**Decision.** `graph.rs` builds and analyses the graph (deterministic, in the
core); the UI's `Graph` tab renders it with a hand-rolled FR layout. No new deps.

**Consequences.**
- Full control over look and interaction; matches egui 0.34 exactly.
- More code to own; layout constants (spring length, repulsion, cooling rate,
  edge thresholds) are hand-tuned rather than provided by a library.

**Reversal conditions.** Revisit if `egui_graphs` catches up to egui's version,
or if richer analysis (centrality, community detection for cluster-level gaps)
makes `petgraph` worth the dependency.
