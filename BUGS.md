> **Status:** Active
> **Provenance:** Shane Hartley (owner), Claude (logging)
> **Last reviewed:** 2026-06-12
> **Why this status:** Live catalogue of bugs found during development.

# Bugs

Catalogue of bugs discovered during development. Per Maintenance Rule 8, bugs are
logged here when found, not silently fixed; Shane decides whether to fix
immediately, defer, or leave alone. Backward-looking incident log; the dual of
[IMPROVEMENTS.md](IMPROVEMENTS.md). Added once friction warranted it (D-009).

Status vocabulary: open | fixed | wontfix | deferred.
Severity vocabulary: low | medium | high.

## Open

(none)

## Fixed

### BUG-001: Wikilink extraction matched TOML/code as links
**Status:** fixed (2026-06-11, same session as parser::parse — step 2)
**Found:** 2026-06-11 (testing `parser::parse` against the real corpus)
**Location:** [src/parser.rs](src/parser.rs) `extract_wikilinks`
**Severity:** low (spurious relationships, not a crash)
**Description.** `extract_wikilinks` was a raw `[[...]]` byte scan that didn't
respect code. Notes embedding TOML table-arrays (`[[shaft]]`, `[[wheel]]`) in
fenced blocks, or `` `[[period]]` `` in inline code, produced bogus link rows —
5 spurious links on the real 28-note corpus.
**Reproduction.** Index a corpus with `[[x]]` inside a fenced code block or an
inline code span; the `links` table gains dangling `dst_id`s that aren't real
wikilinks.
**Notes.** Fixed by skipping code spans and fenced blocks via pulldown-cmark's
offset iterator (`code_ranges`). Dropped spurious links 5 → 0. Originally flagged
in CHANGELOG as "to be backfilled as BUG-001 when BUGS.md is added."

## Won't Fix

(none)

## Deferred

(none)
