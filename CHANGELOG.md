# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com).
Phanes is pre-release; all work to date sits under [Unreleased].
Entries reference F- (features) and D- (decisions) IDs for traceability.

## [Unreleased]

### Added
- F-001 Deterministic indexing — `store` (`hash_for_path`, `upsert`,
  `prune_missing`) and `parser::parse` (YAML frontmatter **and** the blockquote
  header convention; title, links, dates, status). `phanes index` works end to
  end and offline.
- F-002 Full-text search with `--status` / `--tag` / `--stale-days` / `--limit`
  filters, FTS5 ranking, and highlighted snippets.
- F-003 Stale view (`phanes stale --days N`), oldest first.
- F-011 Tinted bordered table output (`tabled` + `owo-colors`, TTY-gated).
- `Status` enum gains `Concept` and `Draft` variants (D-007), kept in lockstep
  with `grammars/idea_extract.gbnf`.
- Project documentation per the Development Documentation Standard: FEATURES.md,
  ARCHITECTURE.md, DECISIONS.md, CHANGELOG.md, LICENSE-MIT, LICENSE-APACHE;
  README and ROADMAP brought to the standard's shape.

### Fixed
- Wikilink extraction no longer mistakes TOML table-arrays (`[[shaft]]`) or
  inline code spans for links — it now skips fenced code blocks and code spans
  via pulldown-cmark's offset iterator. (To be backfilled as BUG-001 when
  BUGS.md is added.)
