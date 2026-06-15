> **Status:** Active
> **Provenance:** Shane Hartley (owner) · Claude (drafting)
> **Last reviewed:** 2026-06-14
> **Why this status:** The end-user guide for Phanes; kept in step with the shipped CLI + UI.

---

# Phanes — User Manual

Phanes indexes a folder of project-idea markdown notes and lets you search them,
see what relates to what, spot what's quietly rotting, and — with an optional
local model — enrich, embed, bridge, and ask questions across them. There is a
command-line tool and a three-panel desktop app over the same core.

This manual is the practical, task-first guide. For *why* the design is shaped
the way it is, see [DECISIONS.md](DECISIONS.md); for the capability list and
acceptance criteria, [FEATURES.md](FEATURES.md).

## Contents

- [The mental model (read this first)](#the-mental-model-read-this-first)
- [Installing and building](#installing-and-building)
- [Quick start](#quick-start)
- [The note convention](#the-note-convention)
- [The command line](#the-command-line)
- [The desktop app](#the-desktop-app)
- [The local model (AI features)](#the-local-model-ai-features)
- [Where your data lives](#where-your-data-lives)
- [Troubleshooting](#troubleshooting)
- [Glossary](#glossary)

## The mental model (read this first)

Two rules explain almost everything about how Phanes behaves.

1. **The model runs at index time only.** Search, `related`, `stale`, `near`,
   and `show` read a cached index and are always instant and offline — they
   *never* call the model. The model is invoked only when you (re)index with the
   AI flags, or when you take an explicit generative action you opted into
   (`bridge`, `ask`, or the equivalent UI buttons).
2. **Model output is *proposed*, never canonical.** Facts the parser extracts
   deterministically (title, links, dates, the tags you wrote) are **asserted**
   and authoritative. Anything the model infers (a summary, extra tags, topics)
   is **proposed**, carries a provenance flag, and never overwrites what you
   asserted. In the UI proposed values are shown distinctly (e.g. a tag prefixed
   `~`); accepting one promotes it to asserted and writes it into your file.

The practical upshot: the daily-driver commands work with no model installed at
all. The AI features are an opt-in layer on top.

## Installing and building

**Prerequisites**

- A Rust toolchain (`rustc` / `cargo`). Install via [rustup](https://rustup.rs).
- *(Optional, for the AI features)* a local OpenAI-compatible model server —
  [LM Studio](https://lmstudio.ai), [Ollama](https://ollama.com), or
  `llama.cpp` in `--api` mode. See [The local model](#the-local-model-ai-features).

**Build profiles** — Phanes is feature-gated so the default build pulls in no
HTTP stack and no GUI:

```bash
# Command-line tool
cargo build --release                       # deterministic core only
cargo build --release --features enrich     # + local-model client (enrich/embed/near*/bridge/ask)

# Desktop app
cargo run --features ui --bin phanes-ui -- <root>          # UI without AI
cargo run --features ui,enrich --bin phanes-ui -- <root>   # UI with AI features
```

\* `near` (semantic similarity) needs vectors created by `index --embed`, which
needs the `enrich` feature. The `near` *query itself* is offline and instant.

The compiled CLI binary is `target/release/phanes`. Throughout this manual
`phanes` means that binary (or `cargo run --features … --bin phanes --`).

## Quick start

```bash
# 1. Put some notes in a folder (see "The note convention"), e.g. ./ideas
# 2. Build the index (deterministic, fast, offline):
phanes --root ideas index

# 3. Use it:
phanes --root ideas search "spatial canvas"
phanes --root ideas stale --days 90
phanes --root ideas related "Spatial Canvas"
phanes --root ideas show spatial-canvas

# 4. Or open the desktop app over the same folder:
cargo run --features ui --bin phanes-ui -- ideas
```

`--root` points at your ideas folder and defaults to `.`. The desktop app takes
the root as a positional argument and defaults to `ideas`.

To layer the AI features on, start a model server, then:

```bash
phanes --root ideas index --enrich --embed     # proposed summaries/tags + vectors
phanes --root ideas near spatial-canvas         # semantically similar notes
phanes --root ideas ask "which ideas are about visualisation?"
```

## The note convention

Phanes is more than `grep` because your notes carry a little structure it can
parse. One idea per file. Each note can declare metadata two ways — use either
or both; the parser reads both.

**Blockquote header** (the project-scaffold style):

```markdown
> **Status:** Active
> **Provenance:** Your Name
> **Last reviewed:** 2026-05-28
> **Why this status:** Actively prototyping the canvas.

# Spatial Canvas for Idea Relationships

A pan-and-zoom canvas that lays ideas out as nodes…
```

**YAML frontmatter** (this is the channel for *asserted tags*):

```markdown
---
status: active
tags: [ui, visualization, spatial]
last_reviewed: 2026-05-28
---

# Spatial Canvas for Idea Relationships
…
```

**Links between notes** are extracted automatically — both relative markdown
links (`[label](other-note.md)`) and `[[wikilinks]]`. They drive `related` and
the graph. Links inside fenced code blocks or inline code are ignored.

**Status vocabulary** (anything else reads as `unknown`):
`concept` · `draft` · `active` · `dormant` · `complete` · `archived` ·
`superseded`.

You don't have to hand-write the header — `phanes new` scaffolds it for you.

## The command line

Every command takes the global `--root <dir>` (default `.`). The index is built
once and read by the query commands.

### `index` — build or refresh the index

```bash
phanes --root ideas index [--enrich] [--embed] [--force]
```

Walks the folder, parses every `.md` file, and stores the result. Unchanged
files are skipped on a content-hash match, so re-indexing an unchanged corpus is
nearly free and makes **zero** model calls.

- `--enrich` — run the model on notes that need it, adding a *proposed* summary,
  tags, and topics (needs the `enrich` build + a server).
- `--embed` — compute an embedding vector per note, enabling `near`, `gaps`, and
  `ask` (same requirements).
- `--force` — re-process every file regardless of hash (e.g. after changing a
  prompt or model).

A missing or slow model never fails an index pass — the asserted-only record is
kept and the error logged.

### `search` — full-text search with filters

```bash
phanes --root ideas search "embedding graph" \
  --status active --tag ui --stale-days 120 --limit 10
phanes --root ideas search "synth" --semantic    # + semantically-near notes
```

Ranked full-text search over title, summary, and body, with highlighted
snippets. All filters are optional. Add `--semantic` (or tick **Semantic** by the
explorer filter) to also surface notes *near* the keyword matches — topically
related notes that don't contain the exact words — fused in by reciprocal-rank.
This stays fully offline (it never sends your query to the model); it needs a
prior `index --embed`, and ignores the metadata filters above.

### `stale` — what hasn't been touched

```bash
phanes --root ideas stale --days 180
```

Notes not reviewed (or, lacking a review date, not modified) within `--days`,
oldest first — the "what's quietly rotting" view.

### `related` — links + shared-tag neighbours

```bash
phanes --root ideas related "Spatial Canvas"
```

Explicit links first, then notes that share tags, ranked by overlap.
Relationships are computed at query time, so they can't go stale.

### `near` — semantically similar notes

```bash
phanes --root ideas near spatial-canvas
```

Ranks notes by cosine similarity over the stored embeddings — surfaces notes
that are *about the same thing* even when they share no tags and link to nothing.
Needs a prior `index --embed`. The ranking itself is offline and instant.

### `gaps` — structural gap analysis

```bash
phanes --root ideas gaps
```

Lists **orphans** (notes connected to nothing) and **candidate bridges**
(note pairs that are semantically close but not explicitly linked), plus **hubs**
(most central notes) and a **clusters** summary. Deterministic; candidate bridges
and hubs need embeddings.

### `tags` — the tag vocabulary

```bash
phanes --root ideas tags
```

Every tag with its note count (and an asserted/proposed split where they differ),
most-used first. The CLI form of the explorer's Tags view.

### `bridge` — propose an idea connecting two notes *(AI)*

```bash
phanes --root ideas bridge spatial-canvas llm-idea-graph
```

Asks the model for one concrete new idea that genuinely draws on both notes. An
explicit, opt-in generative action — not part of the instant query paths. Needs
the `enrich` build + a server; graceful if the model is unavailable.

### `ask` — answer a question from your notes (RAG) *(AI)*

```bash
phanes --root ideas ask "which ideas involve graphs or networks?"
```

Embeds your question, retrieves the most relevant notes, and has the model answer
from those excerpts, citing note titles in `[brackets]` and listing its sources.
Needs the `enrich` build, a server, and a prior `index --embed`. This is the one
feature that puts the model on a question→answer path, so it is a deliberately
separate, user-invoked mode — never wired into `search`.

### `show` — one idea in full

```bash
phanes --root ideas show spatial-canvas
```

Metadata, relationships, and per-field provenance flags for a single note. The
argument resolves by exact id, exact title, or a unique substring.

### `new` — capture a note with the header pre-filled

```bash
phanes --root ideas new "My New Idea" --tag ui --tag spatial
```

Creates a scaffold note (blockquote header, `Status: Concept`, your `--tag`
values as asserted frontmatter), refuses to overwrite an existing file, then
indexes it so it's immediately searchable.

## The desktop app

```bash
cargo run --features ui --bin phanes-ui -- ideas          # without AI
cargo run --features ui,enrich --bin phanes-ui -- ideas   # with AI features
```

Three panels over the same index. The app indexes its folder on startup, so it
works even when pointed at a never-indexed folder.

**Quick switcher.** Press `Ctrl+P` (`Cmd+P` on macOS) from anywhere to fuzzy-jump
to any note by title or id: type to filter, ↑/↓ to move, Enter to open, Esc to
close. The chosen note opens and is revealed in the explorer.

**This manual, in the app.** Click the **?** button in the centre toolbar (or
press `F1`) to read this manual rendered right in the centre pane; Close or `F1`
again dismisses it. It's read-only and never indexed as a note.

### Left — explorer

An **Ideas / Files / Tags** toggle tops the panel:

- **Ideas** (default) — the indexed notes as a status-tinted folder tree, with a
  filter box (backed by search) and click-to-select.
- **Files** — the raw folder tree, like an IDE file explorer: every subfolder and
  file under the root (dotfiles, `.phanes/`, and `.git/` are hidden). Click a
  `.md` to open it — an indexed note shows full info; a not-yet-indexed file opens
  as a raw view with a "Scan to add" hint. Other files open raw and inert.
- **Tags** — the tag vocabulary, most-used first, each shown as `tag · count`
  (with `~N` if some uses are proposed). Expand a tag to list the notes carrying
  it; click one to open it. (`phanes tags` prints the same list on the CLI.)

Two buttons sit below the toggle:

- **⟳ Scan** — re-index in place (deterministic, no model). The app also watches
  the folder and re-indexes automatically when you add, edit, or delete notes
  outside it, so you rarely need this — it stays as a manual fallback (e.g. to
  force a refresh).
- **✨ Scan + AI** — a background re-index *with* enrichment + embeddings, so new
  or changed notes gain their proposed summaries/tags and their semantic/graph
  layers. Runs on a worker thread (the window stays responsive) and needs the
  `enrich` build + a server.

### Centre — editor / graph / ask

A tab bar selects what the centre pane shows:

- **View** — the selected note rendered as markdown.
- **Edit** — the raw file in a text area. **Save** (button or `Ctrl+S`) writes
  the file and re-indexes that one note (deterministically — no model on save).
- **Graph** — a force-directed map of the relationship layer (links + shared
  tags + semantic edges), status-tinted. Scroll to zoom, drag to pan, drag a
  node to rearrange (neighbours spring along, then settle). **Left-click** a node
  to open it (leaves the graph); **right-click** to *inspect* it in place —
  highlights the node, lights up its connections, labels its neighbours, and shows
  its info on the right without opening the file. The **Gaps** toggle overlays orphans (ringed) and the top candidate
  bridges (dashed). With the `enrich` build, clicking a dashed bridge asks the
  model to propose a connecting idea (shown in a floating window; runs on a
  background thread). The **Clusters** toggle colours nodes by topical cluster
  and sizes them by centrality, so hub notes (the ones many connections route
  through) stand out — the same hubs/clusters `phanes gaps` lists on the CLI.
- **Ask** — type a question and get an answer grounded in your most relevant
  notes, with a clickable source list (the UI form of `ask`; needs `enrich` + a
  server + embeddings).

### Right — info panel

The GUI counterpart of `show` for the selected note:

- **Status** — a dropdown. Changing it writes the new asserted status into the
  file and re-indexes, so a note with no status can be given one in place.
- **Summary / topics** — proposed values from enrichment, shown distinctly.
- **Tags** — editable. `×` removes an asserted tag; `✓` accepts a proposed
  (`~`) tag, promoting it to asserted and writing it into the file; the **add
  tag** field appends a new one. Accepting/removing updates the file and the
  index without disturbing the note's other proposed tags.
- **Related**, **Backlinks**, and **Near (semantic)** — click any entry to
  navigate to it. *Related* is this note's outgoing links + shared-tag
  neighbours; *Backlinks* is the reverse — notes that link **to** this one.
- **Unlinked mentions** — notes that mention this note's title in prose but don't
  link it. Click 🔗 to **accept** one: Phanes writes a real link into that
  mentioning note (and re-indexes), turning a loose mention into a tracked link.

## The local model (AI features)

The AI features (`--enrich`, `--embed`, `near`, `gaps` bridges, `bridge`, `ask`,
and the UI's Scan + AI / Ask / bridge actions) talk to a local
**OpenAI-compatible** server over HTTP. Nothing leaves your machine.

**Set up LM Studio (the reference setup):**

1. Launch the LM Studio desktop app and load a chat model (e.g.
   Qwen2.5-Coder-14B) and an embedding model (e.g. `nomic-embed-text`).
2. Start its local server (the **Developer** tab, or `lms server start` once the
   app is running).
3. Build Phanes with `--features enrich` (CLI) or `--features ui,enrich` (app).

Ollama and `llama.cpp --api` work too. Phanes posts to
`/v1/chat/completions` (chat) and `/v1/embeddings` (vectors) at temperature 0.

**Environment overrides** (all optional):

| Variable             | Default                                          | What it sets                |
|----------------------|--------------------------------------------------|-----------------------------|
| `PHANES_LLM_URL`     | `http://127.0.0.1:1234/v1/chat/completions`      | chat endpoint               |
| `PHANES_LLM_MODEL`   | `local-model`                                    | chat model id               |
| `PHANES_EMBED_URL`   | `http://127.0.0.1:1234/v1/embeddings`            | embeddings endpoint         |
| `PHANES_EMBED_MODEL` | `text-embedding-nomic-embed-text-v1.5`           | embedding model id          |

The first request after the server JIT-loads a model can fail while it warms up;
Phanes retries with a short backoff so cold starts are seamless. A genuinely
down server fails fast and never crashes an index pass or a query.

## Where your data lives

- **The index** is a SQLite database at `<root>/.phanes/index.db`. It is derived
  data — delete it and rebuild with `phanes index` any time.
- **Your notes** are just the `.md` files in your root folder; Phanes reads them
  and (only for status/tag edits you make in the UI) writes them back.
- **Privacy.** The repository's `.gitignore` excludes `/ideas/**` and
  `**/.phanes/`, so your personal notes and the index are never committed. Keep
  those entries if you fork or relocate the folder.

## Troubleshooting

- **The explorer is empty.** The folder has no `.md` files, or it wasn't indexed.
  Run `phanes index` (CLI) or press **⟳ Scan** (app).
- **`near` / Ask / Graph semantic edges show nothing.** You haven't created
  embeddings. Run `phanes index --embed` or press **✨ Scan + AI**.
- **A note has no connections / `near` is empty for just one note.** It's missing
  an embedding — run `index --embed` (or Scan + AI), which fills any note missing
  a layer.
- **`ask` / `bridge` says it can't reach the model.** Start your model server and
  confirm the URL/port (`PHANES_LLM_URL`). These need the `enrich` build.
- **A status or tag won't change from the UI.** The file must be writable, and a
  status change re-indexes — check the status line at the top of the centre pane
  for the result message.
- **Re-running `index` does nothing.** That's the hash gate: unchanged files are
  skipped. Use `--force` to re-process everything.

## Glossary

- **Asserted** — a fact you wrote or the parser extracted deterministically;
  authoritative.
- **Proposed** — a value the model inferred; advisory, flagged, never overwrites
  asserted data. Can be accepted (promoted to asserted) in the UI.
- **Enrichment** — the index-time model pass that adds proposed summary/tags/topics.
- **Embedding** — a vector capturing a note's meaning, used for `near`, `gaps`,
  and `ask`.
- **Bridge** — a model-proposed new idea connecting two existing notes.
- **Orphan** — a note connected to nothing in the relationship graph.

---

See also: [README.md](README.md) (overview), [FEATURES.md](FEATURES.md),
[ARCHITECTURE.md](ARCHITECTURE.md), [DECISIONS.md](DECISIONS.md).

Working on Phanes with Claude? [CLAUDE_MD_GUIDE.md](CLAUDE_MD_GUIDE.md) is a
small, droppable primer for structuring a project's `CLAUDE.md` handoff file
(distilled from [development_documentation.md](development_documentation.md), the
full documentation standard this project follows).
