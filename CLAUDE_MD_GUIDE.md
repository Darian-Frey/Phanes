# How to structure CLAUDE.md

*A drop-in primer for a chat session. Paste this when you want Claude to create or
refresh a project's `CLAUDE.md`. It distils the relevant parts of the full
Development Documentation Standard so you don't have to paste the whole thing.*

---

## What CLAUDE.md is

The **handoff file for AI-assisted development sessions** — the one document a new
session reads first to know where things stand. It describes **current state, not
history**: rewrite the stale parts each significant session rather than appending
a log. Keep it tight (one screen or so); it orients toward the *next* action, not
the past.

## Required sections (in this order)

1. **Project** — 2–3 sentences: what it is, who it's for, what's distinctive.
2. **Current state** — what works, what's stubbed, what's broken, at **file/module
   granularity**. The most valuable section; be specific (`store.rs: done`,
   `enrich.rs: stubbed`).
3. **Active task / next milestone** — the immediate work item with acceptance
   criteria. Reference feature IDs (`F-012`) so it ties back to FEATURES.md.
4. **Invariants** — the rules that must not be violated, each one named/numbered
   (`INV-1`, …) so they can be cited. These are the things that quietly break if a
   future session doesn't know them.
5. **Build & test** — the exact commands to verify a change (copy-pasteable).
6. **Conventions** — naming, formatting, comment style, commit-message format, and
   when to commit (e.g. "commit only when asked").
7. **Pitfalls** — non-obvious traps specific to this codebase.
8. **Out of scope** — what the AI must not change without asking.

For a **Complete** project, the shape shifts from handoff-for-continuation to
handoff-for-revival: "Current state" becomes the frozen shipped state, "Active
task" becomes **revival triggers** (the conditions under which work should
re-open), and add a **What to read first on revival** section.

## Skeleton (copy, fill in, delete the hints)

````markdown
# CLAUDE.md — {Project} handoff

## Project
{2–3 sentences.}

## Current state
- {file/module}: {done | stubbed | broken — and the detail that matters}

## Active task
{Immediate next work item + acceptance criteria. Reference F-/D- IDs.}

## Invariants (non-negotiable)
- **INV-1 — {short name}.** {The rule, and why it's load-bearing.}

## Build & test
```bash
{exact commands}
```

## Conventions
- {naming / style / commit-message format / when to commit}

## Pitfalls
- {non-obvious trap}. (See ATTACK_VECTORS.md for the canonical list, if present.)

## Out of scope
- {do not change without asking}
````

## The few rules that matter most

- **Current state, not history.** Rewrite when state changes; don't accumulate a
  changelog here — that's what CHANGELOG.md is for.
- **Refresh at the end of every significant session**, so the next one starts
  oriented.
- **Reference stable IDs** (`F-`/`D-`/`BUG-`/`IMP-`) instead of restating
  details — one source of truth per fact; link, don't duplicate.
- **Docs are part of the commit.** A code change that invalidates CLAUDE.md but
  doesn't update it is an incomplete commit.
- **Log, don't silently fix.** If you spot a bug or improvement while doing
  something else, record it in BUGS.md / IMPROVEMENTS.md (if they exist) and let
  the user decide — don't fold it into an unrelated change.

## Supporting docs CLAUDE.md points at (so its ID references resolve)

| Doc | Holds | ID prefix |
|-----|-------|-----------|
| `README.md` | Front door; status header, build, structure. | — |
| `FEATURES.md` | Capabilities, priorities, acceptance criteria. | `F-` |
| `DECISIONS.md` | Design choices + rationale + reversal conditions. | `D-` |
| `ARCHITECTURE.md` | Module boundaries, data flow, invariants. | — |
| `CHANGELOG.md` | Version history (the *history* CLAUDE.md omits). | references `F-`/`D-` |
| `BUGS.md` / `IMPROVEMENTS.md` | Logged-when-found catalogues. | `BUG-` / `IMP-` |

All ID-bearing docs use **append-only** IDs: withdrawn entries get a status flag,
never deletion, because other docs may still cite them.

---

*Want the full standard (tiers, creation order, every document spec, lifecycle
transitions)? That lives in `development_documentation.md`. This primer covers
only how to shape CLAUDE.md.*
