> **Status:** Active
> **Provenance:** Shane Hartley (owner), Claude (logging)
> **Last reviewed:** 2026-06-12
> **Why this status:** Live catalogue of code-quality improvements; appended as they surface.

# Improvements

Catalogue of code-quality improvements, refactors, and architectural changes
proposed during development. Per Maintenance Rule 8, improvements are logged here
when noticed, not silently applied; Shane decides whether to apply, defer, or
decline. The dual of [BUGS.md](BUGS.md): bugs are broken; improvements work but
could be better. Added once friction warranted it (D-009 reversal trigger).

Status vocabulary: suggested | applied | declined | deferred.
Effort vocabulary: trivial | small | medium | large.

## Suggested

### IMP-002: UI editor discards unsaved edits when switching notes
**Status:** suggested
**Found:** 2026-06-10 (during the F-010 editor build)
**Location:** [src/bin/phanes-ui.rs](src/bin/phanes-ui.rs) `select`
**Effort:** small
**Description.** Selecting a different note while the centre editor has unsaved
changes silently replaces the buffer — the edits are lost. Only the `● unsaved`
marker warns beforehand.
**Proposal.** On `select` with a dirty buffer, either confirm (keep editing /
discard / save) or stash per-note buffers so switching back restores them.
**Trade-offs.** A confirm step interrupts fast browsing; per-note stashing adds
state. Low urgency since the dirty marker is visible and Save is one keystroke.
**Notes.** Surfaced during F-010; noted in that step's handoff.

## Applied

### IMP-001: Retry model requests on cold-load transport failure
**Status:** applied (2026-06-12)
**Found:** 2026-06-11 (enrichment live test; recurred for embeddings and bridges)
**Location:** [src/enrich.rs](src/enrich.rs) `post_json`; [src/embed.rs](src/embed.rs)
**Effort:** small
**Description.** The first model request after the server JIT-loads a model fails
with a transport error (the connection is refused while the model loads), then
succeeds once warm. This bit enrichment, embedding (F-012), and bridge proposal
(D-015) in turn — each needed a manual re-run.
**Proposal.** Funnel all model POSTs through one `post_json` helper that retries
on a transport or 5xx failure with a short backoff (0 / 1.5 / 3 s), and set a
connect timeout (5 s) + request timeout (180 s).
**Trade-offs.** A cold start now costs up to ~4.5 s of retries on the *first*
call (only on failure); a genuinely-down server fails after the attempts rather
than instantly. Retrying could in theory mask a flaky server, but the final error
is still surfaced. Accepted — model calls are already slow, opt-in paths.
**Notes.** Shared by chat (enrich/bridge) and embed, so all model paths benefit.

## Declined

(none)

## Deferred

(none)
