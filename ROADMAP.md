# ROADMAP — Phanes

Phased so the deterministic core is useful before any model is involved.

## P1 — Deterministic core
- `store`: hash lookup, upsert, prune.
- `parser::parse`: frontmatter + title + links + dates.
- `index`, `search`, `stale`, `show` working end to end.
- Table output (tabled + owo-colors status tints).
- **Exit:** `phanes index && phanes search foo` returns ranked hits offline.

## P2 — Relationships
- `links` persistence + dangling-target tolerance.
- `related`: explicit links, then shared-tag neighbours ranked by overlap.
- `resolve`: id-or-fuzzy-title to a single id.
- **Exit:** `phanes related <idea>` shows linked and tag-adjacent notes.

## P3 — Enrichment (opt-in)
- `--features enrich`: llama-server client (done), prompt + grammar tuning.
- Provenance surfaced in `show`; proposed tags visibly distinct from asserted.
- `--force` re-enrich; verify the hash gate skips unchanged files.
- **Exit:** freeform notes get a usable summary, tags, and status guess, and a
  re-index of an unchanged corpus costs ~zero model calls.

## P4 — Later (not committed)
- `new` template polish; per-idea `open` in `$EDITOR`.
- A graph/map view of the relationship layer (petgraph → export, or a TUI),
  matching the spatial-first preference. Embedding-based semantic "near this"
  search would slot in here as a second enrichment, separate from extraction.
