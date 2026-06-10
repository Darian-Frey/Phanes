> **Status:** Active
> **Provenance:** Claude (initial 2026-05-07; revised 2026-05-12 adding DECISIONS.md, ATTACK_VECTORS.md, CLAIMS.md formalisation, project lifecycle, tooling integration, cost framing; revised 2026-05-12 tightening tooling integration, harmonising Authors/Provenance distinction, marking tooling references as illustrative, adding standard self-evolution rule; revised 2026-05-13 reconciling Tier 1 framing with the cost note's friction test, adding Documentation-as-deliverable Workflow Variation, softening ATTACK_VECTORS detection rule, adding recursive-application and project-specific-extensions clauses to Evolution section — motivated by first self-audit of host repo; revised 2026-05-13 adding Retroactive completion path, Future-revival friction test, Complete-state CLAUDE.md Workflow Variation, Decided/Recorded date fields for DECISIONS entries, and "not implemented" as first-class ATTACK_VECTORS detection — motivated by second self-audit on Arithmancy; revised 2026-05-21 adding BUGS.md and IMPROVEMENTS.md as Tier 2 document types with BUG-/IMP- stable IDs, Maintenance Rule 8 "Log when found, not silently acted on", and corresponding skeletons — motivated by adoption of tux-ti83's in-repo bug and improvement tracking pattern)
> **Last reviewed:** 2026-05-21
> **Why this status:** Living standard for project documentation. Refresh as conventions evolve.

# Development Documentation Standard

This document defines the **core set of development docs** that should exist in every project, regardless of language or domain. Drop this file into a chat session to give Claude (or any collaborator) the documentation conventions to follow.

Every project also requires its own domain-specific documentation (e.g. physics derivations, protocol specs, file format references). This standard covers only the **structural / process** documents — the scaffolding that lets a project be picked up months later without losing context.

The standard is designed to serve two audiences simultaneously: humans picking the project up after a gap, and tooling (critic tools such as Crucible, AI development partners such as Claude Code) reading the project to act on it. The conventions are chosen so that the same docs work for both — humans get readable prose with structure; tools get stable IDs, fixed status vocabulary, and a known schema.

---

## How This Maps to the Software Development Lifecycle

Industry-standard SDLC phases (planning → requirements → design → implementation → testing → deployment → maintenance) each have characteristic documents. Distilled, the canonical hierarchy is:

| Question | Industry name | This standard |
|----------|--------------|---------------|
| **Why** does this project exist? | BRD (Business Requirements) | `README.md` (vision paragraph) |
| **What** does the tool/system do? | PRD (Product Requirements) | `FEATURES.md` |
| **What** does the research assert? | Claims register | `CLAIMS.md` |
| **When** will each part be built? | Roadmap / Backlog | `ROADMAP.md` |
| **How** is it structured (high level)? | SDD / Architecture Doc | `ARCHITECTURE.md` |
| **Why** were the structural choices made? | ADR log | `DECISIONS.md` |
| **How** does it work (technical detail)? | SRS / TSD | `SPEC.md` (or domain-named) |
| **How** could it break? | Threat model / failure-mode list | `ATTACK_VECTORS.md` |
| **What went wrong (realised)?** | Bug tracker | `BUGS.md` |
| **What could be better (candidate)?** | Tech-debt / refactor backlog | `IMPROVEMENTS.md` |
| **How** do I build it? | Build/Deploy Guide | `BUILD.md` |
| **What changed and when?** | Release notes | `CHANGELOG.md` |
| **How does an AI pick this up?** | (No industry equivalent) | `CLAUDE.md` |

The critical separations:

- **Features (capabilities)** vs **Claims (assertions about the world)** — features describe what the tool does (acceptance criteria); claims describe what the research asserts is true (falsification conditions). Research-software hybrid projects need both.
- **Features (what)** vs **Spec (how)** — features are user-facing capabilities; spec is internal implementation detail.
- **Architecture (structure)** vs **Decisions (rationale)** — ARCHITECTURE describes the system as it is; DECISIONS records why it ended up that way and what alternatives were rejected.
- **Spec (correctness contract)** vs **Attack Vectors (failure modes)** — SPEC says what the system does when working; ATTACK_VECTORS lists how it can fail.
- **Attack Vectors (anticipated failures)** vs **Bugs (realised failures)** — ATTACK_VECTORS is a forward-looking checklist of what the project must guard against, with detection methods; BUGS is the backward-looking log of what actually went wrong, with status flags. A recurring BUGS pattern may warrant a new ATTACK_VECTORS entry; an ATTACK_VECTORS entry that escaped detection in production becomes a BUG.
- **Bugs (broken)** vs **Improvements (works but could be better)** — BUGS catalogues realised defects; IMPROVEMENTS catalogues candidate refactors, performance tweaks, and architectural cleanups that aren't user-facing committed capabilities (FEATURES) and aren't decisions between alternatives (DECISIONS).

---

## Document Tiers

### Tier 1 — Default minimum (every project, with narrow documented exemptions)

| File | Purpose | SDLC phase |
|------|---------|-----------|
| `README.md` | Entry point. What the project is, status, how to build/run. | Planning |
| `FEATURES.md` | Capability list with priorities and acceptance criteria. | Requirements |
| `ROADMAP.md` | Phased development plan with explicit milestones. | Planning |
| `CLAUDE.md` | Handoff document for Claude Code / AI development sessions. | Implementation (continuous) |
| `CHANGELOG.md` | Version history; what changed and when. | Implementation (continuous) |
| `LICENSE` | Legal terms. Pick one before first public commit. | Planning |

Tier 1 is the default minimum below which projects reliably suffer. Legitimate exemptions exist — a documentation-only repo may not need `FEATURES.md` or `ROADMAP.md`; a single-developer toy project may not need `CLAUDE.md`; a private throwaway may not need `LICENSE` — but each exemption must be recorded as a `DECISIONS.md` entry naming the document, the reason it does not apply, and the conditions under which the exemption should be revisited. This keeps Tier 1 disciplined: omissions are deliberate and auditable rather than oversights. The cost-note friction test applies; documenting the exemption is what distinguishes a principled omission from drift.

### Tier 2 — Strongly recommended (most non-trivial projects)

| File | Purpose | SDLC phase |
|------|---------|-----------|
| `ARCHITECTURE.md` | Module boundaries, data flow, key invariants. | Design |
| `DECISIONS.md` | Indexed log of design decisions with rationale and reversal conditions. | Design (continuous) |
| `SPEC.md` (or domain-named, e.g. `PHYSICS.md`, `PROTOCOL.md`) | Authoritative technical specification. | Design |
| `BUILD.md` | Environment setup, toolchain versions, build commands. | Implementation |
| `BUGS.md` | Catalogue of discovered bugs with status (open / fixed / wontfix / deferred). Backward-looking incident log; complements `ATTACK_VECTORS.md`'s forward-looking checklist. | Implementation (continuous) |
| `IMPROVEMENTS.md` | Catalogue of candidate refactors and code-quality changes (suggested / applied / declined / deferred). Tracks "works but could be better" items distinct from bugs and features. | Implementation (continuous) |

### Tier 3 — Conditional (use when applicable)

| File | When to include |
|------|-----------------|
| `CLAIMS.md` | **Required for research projects** making empirical or theoretical assertions. Optional otherwise. Pairs with `FEATURES.md` for research-software hybrid projects. |
| `ATTACK_VECTORS.md` | Project has well-defined failure modes worth enumerating (physics validity, performance budgets, security, correctness). Most valuable when paired with a critic tool (e.g. Crucible). |
| `VOCABULARY.md` | When the project shares a contract with a sibling project (terms, types, IDs that must match). |
| `TESTING.md` | When test strategy is non-obvious or spans multiple harnesses. |
| `SECURITY.md` | Security-sensitive projects, or any repo accepting vulnerability reports. |
| `CONTRIBUTING.md` | Public repos accepting external contributions. |
| `CITATION.cff` / `CITATION.md` | Research code intended to be cited in publications. |
| `BENCHMARKS.md` | Performance-critical projects; record baseline numbers and methodology. |

---

## Creation Order (which doc first?)

Documents are not independent. They feed each other. The recommended order for a new project:

1. **`README.md`** (skeleton) — captures the vision paragraph and status. Even one sentence is enough to start.
2. **`FEATURES.md`** — list capabilities before designing anything. If you can't list what it does, you don't yet know what to build.
3. **`ROADMAP.md`** — group features into phases. Forces realism about scope and ordering.
4. **`ARCHITECTURE.md`** — high-level structure that can deliver the Phase 1 features.
5. **`DECISIONS.md`** — initialised alongside ARCHITECTURE.md with the first major design choices logged as D-001, D-002…
6. **`SPEC.md`** — technical detail follows architecture. Constants, formats, equations.
7. **`ATTACK_VECTORS.md`** — added when the first failure mode is identified (often during SPEC drafting or first review).
8. **`BUILD.md`** — written when the first build succeeds, captured immediately so it isn't lost.
9. **`CLAUDE.md`** — created at the start of the first AI-assisted coding session.
10. **`CHANGELOG.md`** — initialised with the first commit, appended continuously.
11. **`LICENSE`** — before first public commit.

Tier 3 documents are added when their trigger condition fires (see the Quick Decision Guide at the end).

### Workflow variations

**AI-first projects.** When Claude Code (or another AI coding partner) is the primary development environment from the start, create `CLAUDE.md` at step 2, immediately after `README.md`, so the AI partner has project state to read from day one. The remaining conceptual order is unchanged.

**Research-driven projects.** When the project is primarily research with code as a deliverable (e.g. a theoretical physics paper with accompanying simulation code), create `CLAIMS.md` at step 2 or 3, before or alongside `FEATURES.md`. The claims define what the code must demonstrate; features then describe the tooling required to test the claims. `ARCHITECTURE.md` and `SPEC.md` may also start earlier than usual, since the mathematical framework often precedes the code.

**Documentation-as-deliverable projects.** When the project's primary output is documentation itself (a style guide, a standard, an RFC, a long-form essay or book, a dataset's documentation), several Tier 1 documents need adapting rather than applying verbatim:

- `README.md` — the "Quick Start" section may become a "How to use" paragraph (there is no "running output"); "Build requirements" and "Project structure" may be reduced to a one-line directory note or omitted as trivial. The README still leads with the status header and one-paragraph description.
- `CLAUDE.md` — Build & test commands and Architectural invariants sections will be empty or near-empty; emphasis shifts to Conventions (writing style, terminology), Out of scope, and any rules about how the documentation itself is maintained (e.g. "refresh the Provenance log on every material edit").
- `FEATURES.md` and `ROADMAP.md` — often legitimate Tier 1 exemptions (record via DECISIONS), because the deliverable's "features" are the document's structure, and revisions happen inline rather than in phases. If the project does grow companion artifacts (validators, examples, sibling documents) that warrant stable IDs, the exemption stops being free and these documents come back in.
- `SPEC.md` — the documentation deliverable usually *is* the spec, possibly under a domain-fitting name.
- `ATTACK_VECTORS.md` — failure modes for documentation projects (drift, contradiction, staleness, cross-reference rot) typically lack automated detection. "Manual / structural review" is a valid detection method; the standard's intent is that detection is *defined*, not that it is *automated*.
- Project Lifecycle transitions — checklist items that reference exempt documents (e.g. "all `FEATURES.md` Must-priority entries Complete") are themselves exempt for that project; the DECISIONS entry recording the original exemption is sufficient.

This variation also covers the recursive case where the standard is its own deliverable. See **Evolution of this standard** below.

**Complete-state projects (handoff for revival).** When a project transitions to Complete, the `CLAUDE.md` document's role shifts from handoff-for-continuation to handoff-for-revival. The standard `CLAUDE.md` shape implicitly assumes Active state — fields for "Current state" (what's in progress), "Active task / next milestone" (what's being worked on now), "Out of scope" (what the AI should not change without asking). For Complete projects these sections still exist but their meanings shift, and one new section becomes load-bearing:

- **Current state → Frozen state at completion.** Still file-level, but the verbs change from "in progress" / "stubbed" to "shipped" / "deferred" / "documented but not implemented." The intent is to describe what the project actually ships, not what it might one day become.
- **Active task → Revival triggers.** The conditions under which a future session would legitimately re-open the project. Examples: a candidate result confirmed externally; hardware that the design targets becoming accessible; a citation or bug report requiring a response; a successor project incorporating this one's results. If revival happens for a reason not on this list, the trigger probably warrants its own DECISIONS entry before work resumes.
- **Out of scope** carries extra weight, because a future session — yours or another agent's — will be tempted to mis-scope on revival. State explicitly what is settled and not to be re-litigated.
- **New section: What to read first on revival.** Pointers into the highest-leverage documents for someone re-entering the project cold. Typically: the sealing DECISIONS entry (which records scope-delivered-vs-scope-planned); the CLAIMS.md status flags (which separate realised from aspirational pillars); any ATTACK_VECTORS entries with `Detection: not implemented` (failure modes the codebase doesn't yet guard against).

The Complete-state `CLAUDE.md` is short — typically half the length of an Active-state one — because it is not orienting toward work but toward a decision to begin work. Its job is to make that decision honest and informed.

**Rule of thumb:** if you find yourself writing code before `FEATURES.md` (or `CLAIMS.md`, for research) exists, stop. You're guessing at scope rather than defining it.

---

## The README Status Header Standard

Every `README.md` begins with a four-line blockquote header:

```markdown
> **Status:** {Active | Dormant | Complete | Archived | Superseded}
> **Provenance:** {role tags — e.g. "Claude (primary auditor)", "Gemini (early scaffolding)"}
> **Last reviewed:** YYYY-MM-DD
> **Why this status:** {cause, forward intent, or resumption condition}
```

**Status vocabulary (use exactly these words):**

- **Active** — under current development; commits expected within weeks.
- **Dormant** — paused but intended to resume. The "Why" line states the resumption condition.
- **Complete** — feature-complete and stable. No further work planned, but bug fixes possible.
- **Archived** — abandoned or read-only. Not maintained.
- **Superseded** — replaced by another project. The "Why" line names the successor.

The transitions between these states have their own discipline — see **Project lifecycle** below.

---

## Per-Document Specifications

### `README.md`

The front door. Optimised for someone who has never seen the project before.

**Required sections (in order):**

1. **Status header** (the blockquote above).
2. **One-paragraph description** — what the project is, who it's for, what makes it distinctive.
3. **Quick start** — the shortest path from clone to running output. Real commands, not prose.
4. **Build requirements** — toolchain, OS targets, key dependencies with versions. Link to `BUILD.md` if non-trivial.
5. **Project structure** — top-level directory tree with one-line annotations.
6. **Documentation map** — links to `FEATURES.md`, `ARCHITECTURE.md`, `DECISIONS.md`, `ROADMAP.md`, `SPEC.md`, `ATTACK_VECTORS.md`, `CLAIMS.md` as applicable.
7. **License** — one line stating the license and linking to `LICENSE`.

### `FEATURES.md`

The **what**, not the how. Lists every capability the project offers (or intends to offer), prioritised, with acceptance criteria.

**Required sections:**

1. **Target users** — who this is for, in one or two sentences.
2. **Feature list** — grouped by area, each with priority and acceptance criteria.
3. **Out of scope** — explicit list of things this project will not do.
4. **Future / candidate features** — ideas not committed to.

**MoSCoW priorities:** Must / Should / Could / Won't.

**Feature entry format:**

```markdown
### F-012 Procedural hovercraft physics
**Priority:** Must
**Acceptance:**
- Hovercraft responds to thrust, yaw, pitch inputs at 120 Hz fixed timestep
- Four flight-assist levels selectable at runtime
**Status:** Complete (Phase 2)
**Notes:** See ARCHITECTURE.md §Physics for integration scheme.
```

Stable IDs (`F-012`) let `ROADMAP.md`, `CHANGELOG.md`, `DECISIONS.md`, and commit messages reference features unambiguously. IDs are append-only; withdrawn features get `Status: Withdrawn` rather than being deleted.

**Relationship to `CLAIMS.md`:** for research-software hybrid projects (most theoretical work with accompanying code), `FEATURES.md` and `CLAIMS.md` are both required. They cover orthogonal axes:

- `FEATURES.md` describes what the tool/system *does*, checked by **acceptance criteria** (tests, code execution).
- `CLAIMS.md` describes what the research *asserts is true*, checked by **falsification conditions** (data, proof, observation).

A worked example: for a modified-gravity paper with accompanying simulation code, FEATURES covers "test suite must pass 38/38", "SPARC analysis script runs end-to-end", "LaTeX builds cleanly"; CLAIMS covers "the free function predicts a sign-reversal observable at angle θ≈8.5 arcmin in Euclid Wide Survey data". The first set is engineering deliverables; the second is scientific commitments. Pure code projects with no scientific claims need only FEATURES. Pure research notes with no code may need only CLAIMS.

### `CLAIMS.md`

The research counterpart to `FEATURES.md`. Lists every empirical, theoretical, or mathematical assertion the project makes, with the conditions under which each would be considered refuted.

**Required sections:**

1. **Scope** — what kinds of claims this document covers (empirical predictions, mathematical proofs, theoretical derivations).
2. **Claims list** — grouped by domain or paper section, each with status and falsification conditions.
3. **Withdrawn / refuted claims** — claims that did not survive review, kept for audit trail.

**Status vocabulary:** Proposed | Supported | Refuted | Withdrawn.

**Claim entry format:**

```markdown
### C-003 Gravitational wave speed compatibility
**Status:** Supported
**Domain:** Empirical
**Authors:** Shane Hartley (with Crucible review 2026-03-15)
**Related:** F-027 (test_gw_speed.py), D-004 (mimetic conformal framing), AV-003

**Statement.** The scalar-tensor sector preserves c_GW = c at λ = 0 to within the GW170817 bound |c_GW/c − 1| < 5×10⁻¹⁶.

**Falsification conditions.** Either (a) symbolic derivation in `derivations/causality.ipynb` produces a non-zero coefficient on the (∂ϕ)² gradient term at λ = 0, or (b) any future multimessenger event tightens the GW170817 bound by ≥1 order of magnitude and the prediction fails it.

**Current evidence.** 38/38 unit tests pass; LAM_BOUNDS tightened to 0.35 after V4.2 review found marginal violation at λ = 0.44.

**Reversal conditions.** Refuted if either falsification condition fires.
```

Stable IDs (`C-001`, `C-002`, …) let papers, commits, and DECISIONS entries reference claims unambiguously. IDs are append-only; refuted or withdrawn claims keep their IDs and gain a status flag.

**Maintenance rules:**

1. **Falsification conditions are not optional.** A claim without a falsification condition is a belief, not a claim. Reviewers (human or Crucible) should reject entries that fail this.
2. **Status changes get a DECISIONS entry.** Moving a claim from Supported to Refuted is a significant project event and must be logged.
3. **Cite from papers.** Published or preprinted work referencing the project should cite by claim ID, so reviewers can trace the source of any specific assertion.

### `ARCHITECTURE.md`

The structural document. With `DECISIONS.md` carrying rationale, ARCHITECTURE.md is purely descriptive — it describes the system as it currently is.

**Recommended sections:**

1. **System overview** — diagram or ASCII art showing module boundaries and data flow.
2. **Module responsibilities** — one paragraph per top-level module.
3. **Key invariants** — rules the system must maintain.
4. **Cross-cutting concerns** — logging, error handling, concurrency model, threading model.

The "why did we do it this way" content lives in `DECISIONS.md`, not here. ARCHITECTURE describes; DECISIONS justifies.

### `DECISIONS.md`

The indexed log of design decisions. Each entry captures what was decided, what alternatives were considered, why this option was chosen, and what evidence would reverse it. It is the audit trail that survives six-month gaps in attention.

This is the project's ADR (Architecture Decision Record) log. When a critic tool such as Crucible is part of the workflow, sealed conclusions from review sessions are recorded here — Crucible suggests entries; the author adjudicates and merges. Without Crucible, the author writes entries directly; the structural role is identical.

**File header:**

```markdown
# Decisions

Append-only log of significant design decisions.
Each entry: D-NNN, with Decided and Recorded dates (ISO 8601; equal for normal entries, divergent for retroactive ones — see Date fields explained below), status, context, alternatives, decision, consequences, and reversal conditions.
Status vocabulary: Proposed | Accepted | Superseded by D-NNN | Deprecated.
```

**Entry format:**

```markdown
### D-007 Fixed 120 Hz physics timestep
**Decided:** 2026-03-14
**Recorded:** 2026-03-14
**Status:** Accepted
**Authors:** Shane Hartley (with Crucible review 2026-03-15)
**Related:** F-012, F-018, SPEC.md §Physics

**Context.** terra-siege physics needs deterministic replay and stable integration for a hovercraft with aggressive control inputs near terrain. Three options were considered.

**Options.**
- **A. Variable timestep tied to frame rate.** Simplest. Rejected: non-determinism breaks replay; integration instability at low frame rates.
- **B. Fixed 60 Hz timestep with interpolated rendering.** Standard game-physics pattern. Considered but rejected for this project — proportional-navigation missile guidance shows visible jitter at 60 Hz.
- **C. Fixed 120 Hz timestep with interpolated rendering.** Chosen.

**Decision.** Option C. Physics steps at 120 Hz regardless of frame rate; rendering interpolates between the two most recent physics states.

**Consequences.**
- Replay determinism guaranteed if inputs are recorded at physics-tick granularity.
- ~2× physics CPU vs 60 Hz baseline. Measured 0.4 ms/tick on target hardware; budget is 8 ms.
- Missile guidance and ground-effect controllers can use tighter loop gains.

**Reversal conditions.** Revisit if (a) physics cost exceeds 30% of frame budget on minimum-spec hardware, or (b) replay determinism is no longer a requirement.
```

**Date fields explained.** `Decided:` is when the choice was actually made; `Recorded:` is when this entry was written. For decisions made and recorded in the same session, both dates are the same. For **retroactive entries** (decisions made during earlier development and being captured later, e.g. during a retroactive completion pass — see §Project lifecycle), the two diverge:

- If the original date is recoverable from a commit, issue, or other artifact, use it: `**Decided:** 2026-02-04 (from commit cca21cf)`.
- If the original date is genuinely lost, omit `Decided:` and rely on `Recorded:` alone, optionally annotating: `**Decided:** not recorded — backfilled from V2.0.0-GOLD source state`.
- Either field alone is acceptable when the other is unknown; both fields together provide stronger audit traceability when both are recoverable.

The append-only rule applies to both fields. Neither is rewritten after the entry is sealed.

**Maintenance rules:**

1. **Append-only.** Decisions are never deleted. When a decision is reversed, write a new entry with `Status: Accepted` and update the old entry to `Status: Superseded by D-NNN`.
2. **Stable IDs.** `D-001`, `D-002`, … sequentially. Referenced from commits, CHANGELOG entries, and other documents.
3. **One decision per entry.** If a session produces three sealed conclusions, that's three entries.
4. **Reversal conditions are not optional.** Every entry must state what evidence would change the decision. A decision without a reversal condition is a belief, not a decision.

### `SPEC.md` (or domain-specific spec)

The authoritative technical reference. For a game: physics constants, input mappings, save format. For a protocol: message types, state machines. For a research framework: equations, parameters, observable predictions.

Use the name that fits the domain (`PROTOCOL.md`, `FORMAT.md`, `PHYSICS.md`).

**Distinction from `FEATURES.md`:** a feature says "the hovercraft has four flight-assist levels." The spec says "Assist level 3 applies PD controller gains Kp=4.2, Kd=0.8 to roll axis."

### `ATTACK_VECTORS.md`

The project-specific failure-mode checklist. Lists the ways this particular system can be wrong, broken, or compromised — distinct from generic security or testing concerns.

This is where domain knowledge about *how things go wrong* is captured so it can be checked routinely rather than rediscovered after a regression. When a critic tool such as Crucible is in use, this document is co-authored: Crucible proposes additions when it discovers a new failure mode during review; the author adjudicates and merges. Without Crucible, the author maintains it directly from review and post-mortem activity.

**Distinction from `TESTING.md`:** ATTACK_VECTORS lists *what to check*; TESTING describes *how the test infrastructure is organised*. ATTACK_VECTORS entries often correspond to test cases, but the document is the canonical list of concerns, not the test inventory.

**Distinction from `SECURITY.md`:** ATTACK_VECTORS covers all failure modes (correctness, performance, numerical stability, domain validity); SECURITY is specifically about adversarial threats and reporting policy. For a physics project, ATTACK_VECTORS is the right home for "GW170817 constraint violation" — it isn't a security issue, it's a domain failure mode.

**File header:**

```markdown
# Attack Vectors

Project-specific failure modes the project must be resilient against.
Grouped by category. Each vector lists detection method and severity.
Severity: Critical (must hold) | Major (regression on release blocks) | Minor (track only).
```

**Entry format:**

```markdown
## Physics validity

### AV-003 Gravitational wave speed deviation
**Severity:** Critical
**Description.** Any modification to the scalar-tensor sector must preserve c_GW = c at λ = 0 to within the GW170817 bound |c_GW/c - 1| < 5×10⁻¹⁶.
**Detection.** `tests/test_gw_speed.py::test_gw170817_bound`. Symbolic check in `derivations/causality.ipynb`.
**Related decisions.** D-004 (mimetic conformal framing), D-012 (LAM_BOUNDS = 0.35 ceiling).
**Related claims.** C-003.
**History.** Re-tightened to λ ≤ 0.35 after V4.2 review found marginal violation at λ = 0.44.

## Numerical stability

### AV-007 Hot-path heap allocation
**Severity:** Major
**Description.** No allocations inside `Physics::step()` or `Renderer::draw()`. Allocator pressure causes frame-time spikes that violate the 8 ms physics budget.
**Detection.** AddressSanitizer with allocation hook on hot paths; `tools/check_allocs.sh`.
**Related decisions.** D-007 (fixed 120 Hz timestep), D-015 (pre-sized object pools).
```

**Maintenance rules:**

1. **Stable IDs.** `AV-001`, `AV-002`, …, append-only. Referenced from tests, commits, and DECISIONS entries.
2. **Co-authored when a critic tool is in use.** Crucible (or equivalent) suggests; the author adjudicates. Without such a tool, the author maintains it from review activity.
3. **Cross-reference both directions.** ATTACK_VECTORS entries name the related DECISIONS and CLAIMS; those documents reference back. (See Maintenance Rules below for tooling.)
4. **Detection is defined, not necessarily automated, not necessarily implemented.** Every vector must state how it is or would be checked. Acceptable values fall into three categories:
   - **Implemented automated detection.** A test path, tool invocation, CI check, or other mechanical verification: `Detection: tests/test_gw_speed.py::test_gw170817_bound`.
   - **Implemented manual detection.** A documented review procedure executed on a known cadence: `Detection: manual review during quarterly audit; checklist in TESTING.md §quarterly`.
   - **Acknowledged-but-not-implemented detection.** The vector is real, but the project has not built or run the check that would detect it. Use the form `Detection: not implemented (would require X); see CLAIMS C-NNN.` This is especially common in Complete-state projects whose claimed verification machinery was never operationalised, and in early-Active projects where vectors are identified before detection tooling exists.

The third option exists because an undetected vector is itself signal — the gap between the claim and its verification is information a reader needs. Recording "no detection" honestly is more useful than either omitting the vector or pretending a check exists. A vector with no defined detection at all (none of the three categories) is a worry, not a vector. When a vector moves from category three (not implemented) to category one or two (implemented), update the entry rather than creating a new one; the implementation event itself can be noted in the **History** field.

### `BUGS.md`

The catalogue of bugs discovered during development. Backward-looking incident log; the dual of `ATTACK_VECTORS.md` (forward-looking checklist of anticipated failure modes with detection methods).

The document is most valuable in workflows where the bug history should live in-repo rather than in an external tracker — solo development, AI-partner sessions, projects that need bug history to survive forge migrations, and projects where the discipline-rule below is hard to enforce across an out-of-repo tool. Projects using GitHub Issues, Jira, Linear, or equivalent may legitimately exempt `BUGS.md` per the Tier 2 friction test in §A note on cost.

**File header:**

```markdown
# Bugs

Catalogue of bugs discovered during development. Per the project workflow,
bugs are **logged here when found, not silently fixed** (see Maintenance
Rule 8). The author decides whether to fix immediately, defer, or leave
alone.

Status vocabulary: open | fixed | wontfix | deferred.
Severity vocabulary: low | medium | high.
```

**Entry format:**

```markdown
### BUG-019: Bare `.` in the buffer crashes the engine via uncaught `std::stod` exception
**Status:** fixed (2026-04-18, same session as IMP-021)
**Location:** [core_math/src/core_math.cpp](core_math/src/core_math.cpp) `evaluate()` digit-flush lambda
**Severity:** high (process crash, not just an error)
**Description.** Pre-existing latent crash. The digit-coalescing pass collected `Token::Decimal` characters and called `std::stod` on flush. For a bare `.`, `std::stod` throws `std::invalid_argument`, which propagated up uncaught and aborted the process via `terminate()`.
**Reproduction.** `./build/tux_ti83_cli '.'` aborts with `std::invalid_argument: stod`.
**Notes.** Wrapped the `std::stod` call in `try/catch`, set a `parseFailed` flag, return `ERR:SYNTAX` from `evaluate()` if any parse failed. Bare `.` now produces `ERR:SYNTAX` (matching TI-83 behaviour) instead of crashing.
```

**Required fields per entry.** Status; Found (the entry's discovery date in YYYY-MM-DD, with session/commit context if useful); Location (file path with line number if available, or "cross-cutting" for project-wide bugs); Severity; Description (what's wrong, and why it matters); Reproduction (when known — minimum steps to trigger); Notes (related context, suggested fix, links to BUGS/IMPROVEMENTS/DECISIONS/ATTACK_VECTORS entries).

The `Status:` line carries the bug's lifecycle. When status changes from `open` → `fixed`, append the fix date in parentheses (`fixed (YYYY-MM-DD)`) and migrate the entry's section in the file accordingly. Sections by status (Open / Fixed / Won't Fix / Deferred) keep the file readable; the `Status:` field is the source of truth and parseable by tooling.

**Maintenance rules:**

1. **Stable IDs.** `BUG-001`, `BUG-002`, …, append-only. Referenced from commits, CHANGELOG `### Fixed`, ATTACK_VECTORS (when a bug pattern warrants a new vector), DECISIONS (when a fix is significant enough to be a decision), and IMPROVEMENTS (when a bug fix surfaces an improvement candidate).
2. **Log when found, not silently fixed.** See Maintenance Rule 8 below — the discipline that makes BUGS.md useful as a catalogue rather than a decaying after-the-fact record.
3. **Reproduction is not optional for open bugs.** An open entry without reproduction steps is a report, not a bug; mark such entries explicitly (e.g. "Reproduction: not yet isolated — see Notes for the symptom pattern") rather than leaving the field blank.
4. **Cross-reference both directions.** When a BUG entry references an ATTACK_VECTORS, DECISIONS, IMPROVEMENTS, or another BUG entry, the target should reference back. Same tooling caveat as Maintenance Rule 6.

### `IMPROVEMENTS.md`

The catalogue of code-quality improvements, refactors, and architectural changes proposed during development. The dual of `BUGS.md`: bugs are things that are *broken*; improvements are things that *work but could be better* (clarity, reuse, maintainability, performance, future flexibility).

Improvements are distinct from `FEATURES.md` candidate-feature entries (which are uncommitted *user-facing capabilities*) and from `DECISIONS.md` proposed-status entries (which record a *choice between alternatives*). An IMP entry is at an earlier stage than either — the question isn't "which alternative?" or "is this user-visible?" but "is this internal change worth doing at all?"

**File header:**

```markdown
# Improvements

Catalogue of code-quality improvements, refactors, and architectural
changes proposed during development. Per the project workflow,
improvements are **logged here when noticed, not silently applied**
(see Maintenance Rule 8). The author decides whether to apply, defer,
or decline.

This is the dual of BUGS.md: bugs are things that are broken,
improvements are things that work but could be better.

Status vocabulary: suggested | applied | declined | deferred.
Effort vocabulary: trivial | small | medium | large.
```

**Entry format:**

```markdown
### IMP-021: `:` as a statement separator
**Status:** applied (2026-04-29)
**Location:** [core_math/src/core_math.cpp](core_math/src/core_math.cpp), [graph_ui/src/ui_controller.cpp](graph_ui/src/ui_controller.cpp)
**Effort:** small
**Description.** With Variables A–Z + STO landed (IMP-014), the natural next step was chained statements: `5→A:A+1→A`. The `.` CalcKey already had a `:` ALPHA corner label (placeholder); wiring it kept the layout honest.
**Proposal.** Add `Token::Colon`. `evaluate()` short-circuits on Colon — splits the token stream into segments, recurses per segment, returns the last non-empty result. Errors abort the chain immediately, but earlier Sto mutations commit (matching TI-83 per-statement semantics).
**Trade-offs.** Recursive `evaluate()` over flatter sequential-loop approach: chose recursion because the existing function has heavy local state and refactoring for non-recursion would have been a bigger change for no behavioural difference. Recursion depth bounded by number of `:` tokens — won't blow the stack.
**Notes.** Surfaced BUG-019 (latent `std::stod` crash on bare `.`) during testing; fixed transparently in the same session since it blocked GUI verification.
```

**Required fields per entry.** Status; Found (discovery date in YYYY-MM-DD, with session/commit context if useful); Location (file path or "cross-cutting"); Effort; Description (what could be improved and why); Proposal (how to do it); Trade-offs (what we'd give up or risk); Notes (related context, dependencies on other work).

**Trade-offs are not optional.** An entry without `Trade-offs:` is a feature request, not an improvement candidate. The trade-off field is what makes the entry useful for later adjudication — without it, the user revisiting the entry can't evaluate whether to apply it, since the reasons-not-to are missing.

**Maintenance rules:**

1. **Stable IDs.** `IMP-001`, `IMP-002`, …, append-only. Referenced from commits, CHANGELOG `### Changed` (when applied), BUGS (when a bug fix surfaces an improvement, or vice versa), DECISIONS (when applying needs a recorded choice), and FEATURES (when applying produces a user-visible behavior change worth surfacing).
2. **Log when noticed, not silently applied.** See Maintenance Rule 8 — the same discipline that makes BUGS.md useful applies here.
3. **Trade-offs are not optional.** Reject entries without trade-offs at review time; the rule is what keeps IMPROVEMENTS from collapsing into a wish list.
4. **Cross-reference both directions.** When an IMP references a BUG, DECISION, or another IMP, the target references back.

### `BUILD.md`

1. **Supported platforms** — OS, architecture, toolchain versions.
2. **Dependencies** — exact versions, install commands per platform.
3. **Build commands** — debug, release, test, package.
4. **Cross-compilation** — if applicable.
5. **Troubleshooting** — known build failures and fixes.

### `CLAUDE.md`

The handoff document for AI-assisted development sessions.

**Required sections:**

1. **Project summary** — 2–3 sentences.
2. **Current state** — what works, what's stubbed, what's broken. File-level specificity.
3. **Active task / next milestone** — immediate work item with acceptance criteria. Reference feature IDs.
4. **Architectural invariants** — rules that must not be violated.
5. **Build & test commands** — exact commands to verify changes.
6. **Conventions** — naming, formatting, comment style, commit message format.
7. **Known pitfalls** — non-obvious traps. May reference `ATTACK_VECTORS.md` for the canonical list.
8. **Out of scope** — explicit list of things the AI should not change without asking.

Refresh `CLAUDE.md` at the end of every significant session. Current state, not history.

### `ROADMAP.md`

```markdown
## Phase 1 — {Phase name}
**Goal:** {one sentence}
**Status:** {Not started | In progress | Complete}
**Features delivered:** F-001, F-003, F-007
**Deliverables:**
- [ ] {item with file/module reference}
**Acceptance:** {how we know this phase is done}
```

Phases are append-only; mark Complete with an ISO date.

### `CHANGELOG.md`

Follow [Keep a Changelog](https://keepachangelog.com).

```markdown
## [Unreleased]
### Added
- F-014 ego-centric radar with altitude strip
### Changed
- D-007 confirmed under load testing (no reversal triggered)
- IMP-021 applied — `:` statement separator
### Fixed
- AV-007: removed stray std::vector growth in Physics::step()
- BUG-019: bare `.` crash via uncaught `std::stod` exception
```

Reference F-, C-, D-, AV-, BUG-, and IMP- IDs for full traceability.

### Other Tier 3 docs

- **`VOCABULARY.md`** — shared contract with a sibling project; single source of truth.
- **`TESTING.md`** — test pyramid, harness setup, fixture conventions, how to add a new test.
- **`SECURITY.md`** — threat model, reporting policy, supported versions.
- **`CONTRIBUTING.md`** — branch model, PR checklist, review expectations, code style.
- **`BENCHMARKS.md`** — methodology, hardware, baseline numbers, regression policy.

---

## Integration with tooling

This standard is designed so the same documents serve both humans and tooling. The interface contract is summarised here; detailed protocols for specific tools (Crucible, Cairn, Claude Code) belong in those tools' own documentation as they mature.

**For critic tools** (review, stress-test assumptions, identify failure modes): READ from `FEATURES.md` / `CLAIMS.md` (gates), `ARCHITECTURE.md` (invariants to attack), `DECISIONS.md` (do not re-litigate Accepted decisions unless reversal conditions are met), `SPEC.md` (symbolic ground truth), `ATTACK_VECTORS.md` (do not duplicate known vectors). WRITE proposed entries into `DECISIONS.md`, `ATTACK_VECTORS.md`, or suggested status transitions on `CLAIMS.md` / `FEATURES.md`. All writes require author adjudication.

**For AI development partners** (act on the active task): READ `CLAUDE.md` first as the contract, then other docs as needed. The properties of this standard that matter most to AI partners are stable IDs, fixed status vocabularies, reversal/falsification conditions as triggers for revisiting commitments, and append-only history. If an AI partner is also acting as a critic, it writes into the structured docs (DECISIONS, ATTACK_VECTORS) rather than scattering observations across `CLAUDE.md`.

**Provenance fields are parallel but distinct.** Use the same role-tag vocabulary (e.g. "Crucible", "Claude (review)", "Gemini (scaffolding)") in both, but the fields mean different things:

- `README.md` **Provenance** is project-scoped: who/what contributes to the project overall. Format: list of role-tagged contributors. Updated when the contributor mix changes.
- `DECISIONS.md` / `CLAIMS.md` **Authors** is entry-scoped: who made this specific decision or claim, and which tools reviewed it. Format: primary author (with reviewers and dates if applicable). Set when the entry is written.

A reader looking at "who built this project" reads Provenance; a reader looking at "who decided this" reads Authors on the entry.

This integration section is deliberately brief — a fuller specification, including the combined-workflow dataflow, lives with Crucible's own documentation when that tool is built. The standard reserves the slots (entry formats with Authors lines, fixed status vocabularies, append-only IDs); the protocol details that operate over those slots are versioned with the tool, not with this document.

---

## Maintenance Rules

1. **Status header refresh.** Whenever you open a project after >2 weeks away, update `README.md`'s Last reviewed date and Status if drifted.
2. **CLAUDE.md is current state, not history.** Rewrite when state changes; don't append.
3. **Append-only IDs.** FEATURES (F-), CLAIMS (C-), DECISIONS (D-), ATTACK_VECTORS (AV-), BUGS (BUG-), and IMPROVEMENTS (IMP-) all use append-only sequential IDs. Withdrawn/superseded entries get a status flag, never deletion — outside references may still point at them.
4. **ROADMAP phases are append-only.** Mark complete, don't delete.
5. **One source of truth per fact.** A constant defined in `SPEC.md` should not be duplicated in `README.md`. Link instead.
6. **Cross-references both directions, with tooling.** When DECISIONS-N cites ATTACK-VECTOR-M, ATTACK-VECTOR-M should cite DECISIONS-N back. Same for FEATURES ↔ DECISIONS, CLAIMS ↔ DECISIONS, and CLAIMS ↔ ATTACK_VECTORS where applicable. This rule decays without enforcement: a pre-commit hook walking all docs, building the cross-reference graph, and reporting broken or unidirectional links is the canonical fix (a `tools/check_xrefs.py` script is one illustrative implementation, not part of this standard). Without such tooling, treat the rule as aspirational rather than enforced — and acknowledge in `CLAUDE.md` that cross-references may be stale.
7. **Docs are part of the commit.** A code change that invalidates documentation but doesn't update it is an incomplete commit.
8. **Log when found, not silently acted on.** When a bug is discovered or an improvement candidate is noticed during work on something else, log it in `BUGS.md` / `IMPROVEMENTS.md` rather than fix or apply it inline. The author (or, for AI-partner workflows, the user) decides whether to act immediately, defer, or decline. This rule is what makes BUGS and IMPROVEMENTS useful catalogues: their value is completeness, and completeness requires that in-flight discoveries are recorded before they evaporate into commit-message footnotes. This is especially load-bearing for AI partners, which default to acting on discoveries rather than logging them. Applies only when `BUGS.md` and/or `IMPROVEMENTS.md` exist in the project; where neither exists, the rule is moot.

### Evolution of this standard

This document is itself subject to its own rules. Material changes are logged inline in the Provenance line of the status header with a date and short description (e.g. "revised 2026-05-12 adding DECISIONS.md, ATTACK_VECTORS.md, CLAIMS.md formalisation"). The status header's Last reviewed date is updated on every material change.

When the standard reaches sufficient complexity to warrant it — multiple contributors, contested changes, or formal versioning needs — it becomes a project in its own right with its own `DECISIONS.md` recording the rationale for each material revision and what alternatives were rejected. Until then, the inline Provenance log in the header is the audit trail.

Backward compatibility is preserved by the append-only ID rule: no revision of this standard removes or renumbers a document type. New document types may be added (as CLAIMS.md was in the 2026-05-12 revision), but `D-007` in any project always means the same entry it did when written.

**Recursive application to the host repo.** The repository that hosts the standard should itself follow the standard. Where the standard's prescriptions don't apply cleanly to a documentation-only repo (see the Documentation-as-deliverable Workflow Variation above), each exemption is recorded as a `DECISIONS.md` entry in the host repo's own DECISIONS log, naming the exempt document and the reason. The pattern of a periodic self-audit (running the audit prompt in `PROMPTS.md` or equivalent against the host repo) is the recommended mechanism for catching drift, internal contradictions, and meta-level gaps. The first such audit on this repo (2026-05-13) discovered the original "Tier 1 — no exceptions" / cost-note contradiction and motivated this revision; this is the canonical case for what the recursive check is for.

**Project-specific extensions.** Projects may add document types beyond those the standard names (this repo, for example, adds `PROMPTS.md` for cold-start aid; another project might add `BUDGET.md`, `RELEASE_PROCESS.md`, or domain-specific files). Such extensions are not violations of the standard provided that (a) the new document is recorded in the project's `DECISIONS.md` with the reason for adding it, (b) it does not collide with reserved names (`README`, `FEATURES`, `CLAIMS`, `DECISIONS`, `ARCHITECTURE`, `SPEC`, `ATTACK_VECTORS`, `BUGS`, `IMPROVEMENTS`, `BUILD`, `CHANGELOG`, `CLAUDE`, `ROADMAP`, `VOCABULARY`, `TESTING`, `SECURITY`, `CONTRIBUTING`, `BENCHMARKS`, `CITATION`), and (c) it follows the same conventions as analogous standard documents (stable IDs if it lists entries; status header if it has lifecycle state). The standard deliberately does not enumerate every possible document type; project extension is the supported escape hatch.

---

## Project lifecycle

Projects don't only get created; they transition between states. The five Status values (Active, Dormant, Complete, Archived, Superseded) only carry meaning if transitions are recorded. Without explicit procedure, status drift goes unmarked and audit trails break.

### Transitions

**Active → Dormant.** Paused with intent to resume.

- Update `README.md` header: Status, Last reviewed, Why (state the resumption condition explicitly).
- Add a `DECISIONS.md` entry recording the pause: context, resumption trigger, what state was captured.
- Ensure `CLAUDE.md` reflects current state, not aspirations. If the AI partner would be misled by stale content, rewrite it.
- If using Cairn or equivalent session-state tooling, seal a capsule at the pause point.

**Dormant → Active.** Resumption.

- Update `README.md` header (Status, Last reviewed, Why).
- Add a `DECISIONS.md` entry noting the resumption and any context shift since the pause (priorities changed, prior assumption invalidated by external work, etc.).
- Refresh `CLAUDE.md` from the resumption point. Treat it as a new handoff, not a continuation.

**Active → Complete.** Feature-complete and stable.

- All `FEATURES.md` Must-priority entries marked Complete or explicitly Withdrawn.
- All `CLAIMS.md` entries have current status recorded (Supported / Refuted / Withdrawn).
- Final `DECISIONS.md` entry sealing the completion: scope as delivered vs scope as planned, known limitations, expected maintenance posture.
- Update `ROADMAP.md` to mark all phases Complete (with ISO dates) or explicitly Withdrawn.

**Active or Complete → Archived.** Abandoned or read-only.

- Update `README.md` header: Why line must state the reason (superseded, no longer relevant, technical debt overwhelming, lost interest).
- Final `DECISIONS.md` entry: archival decision with context.
- Audit other projects' docs for references; update or mark stale. (If A is archived and B references A, B's CLAUDE.md or relevant doc should note that A is no longer maintained.)

**Any → Superseded.** Replaced by another project.

- Update `README.md` header: Why line names the successor (link if internal).
- The successor's `README.md` should reference the predecessor.
- Final `DECISIONS.md` entry recording the succession and any lessons or assets carried over.
- For research projects, `CLAIMS.md` entries should be ported to the successor (with new IDs) or formally retired with the reason recorded.

### Retroactive completion

Projects that finish development without having adopted this standard during their Active phase cannot tick the Active → Complete checklist cleanly — every checklist item references a document the standard would have built up over the project's life (`FEATURES.md` Must entries, `CLAIMS.md` status, the final `DECISIONS.md` sealing entry, `ROADMAP.md` phases marked Complete). For a late-adopting project, all four are vacuously unmet.

This case is foreseeable enough to deserve explicit guidance. The minimum acceptable retroactive completion path is:

1. **Add the README status header** with `Status: Complete`, `Last reviewed` set to today's date, and a `Why this status` line stating both the completion event (e.g. "V2.0.0 shipped 2026-05-13; no further development planned") and the maintenance posture (e.g. "bug fixes possible; no new features").
2. **Create `DECISIONS.md` with a single sealing entry** that records: scope delivered vs scope originally planned (if a roadmap ever existed informally); any known divergences between existing docs and the as-shipped code; the maintenance posture; and a list of standard-prescribed documents being bulk-exempted with one-sentence reasons per document.
3. **Apply the Future-revival friction test** (see **A note on cost** below) to the remaining standard documents. For Complete projects, the question shifts from "is this absence causing confusion now?" to "would this absence force the next revival to be archaeology rather than execution?" Mandatory: anything whose absence would block revival entirely (a working build recipe, a record of which version produced which result, a record of which claims are realised vs aspirational). Strongly recommended: anything whose absence would cost more than half a day on revival (design decision rationale, architectural map, known failure modes).

If the project asserts performance numbers, integrity claims, or other empirically falsifiable statements in its README or commit history, **a CLAIMS audit pass is strongly recommended even as part of retroactive completion**. This is the single document whose absence does the most damage on revival, because the rationale for "what we know works vs what we hoped would work" is what evaporates first. Each lifted claim is recorded with its status flag (Supported / Proposed / Refuted / Withdrawn) and falsification condition.

Retroactive completion is not a discount on the standard; it is a recognised path that produces a smaller but honest set of documents, with the omissions explicitly recorded rather than silent. A project that retroactively completes following this path is standard-compliant; a project that simply asserts `Status: Complete` without sealing is not.

### Why this matters

A project that drifts to Dormant without documentation looks identical to an Active project from the outside — same files, same README claiming work in progress. When you return six months later, you can't reconstruct where you stopped or why. The transition discipline ensures the documents tell the truth about the project's state regardless of how long ago the last commit was.

For projects shared with collaborators, AI partners, or future-you, this is the difference between resuming work in an hour and losing a day to archaeology.

---

## Minimal File Skeletons

### `README.md` skeleton

````markdown
> **Status:** Active
> **Provenance:** {your name} (initial commit, YYYY-MM-DD)
> **Last reviewed:** YYYY-MM-DD
> **Why this status:** Initial scaffolding.

# {Project Name}

{One-paragraph description.}

## Quick Start

```bash
{exact commands}
```

## Documentation

- [Features](FEATURES.md)
- [Claims](CLAIMS.md)  <!-- research projects only -->
- [Architecture](ARCHITECTURE.md)
- [Decisions](DECISIONS.md)
- [Roadmap](ROADMAP.md)
- [Build instructions](BUILD.md)
- [Changelog](CHANGELOG.md)

## License

{License name} — see [LICENSE](LICENSE).
````

### `FEATURES.md` skeleton

```markdown
# Features

## Target users
{One or two sentences.}

## Out of scope
- {explicit non-goal}

## Features

### F-001 {Feature name}
**Priority:** Must
**Acceptance:**
- {measurable condition}
**Status:** {Not started | In progress | Complete | Withdrawn}

## Candidate features (uncommitted)
- {idea worth tracking}
```

### `CLAIMS.md` skeleton

```markdown
# Claims

Empirical, theoretical, or mathematical assertions made by this project, with falsification conditions.

Status vocabulary: Proposed | Supported | Refuted | Withdrawn.

## Scope
{What kinds of claims this document covers.}

## Claims

### C-001 {Claim title}
**Status:** Proposed
**Domain:** {Empirical | Theoretical | Mathematical}
**Authors:** {author} (with {critic tool} review {date} if applicable)
**Related:** F-NNN, D-NNN, AV-NNN

**Statement.** {The claim, precise enough to be falsifiable.}

**Falsification conditions.** {What evidence or proof would refute this claim.}

**Current evidence.** {Tests passed, papers cited, datasets analysed.}

**Reversal conditions.** {What would move this claim to Refuted or Withdrawn.}

## Withdrawn / refuted claims
{Kept for audit trail.}
```

### `DECISIONS.md` skeleton

```markdown
# Decisions

Append-only log. Status: Proposed | Accepted | Superseded by D-NNN | Deprecated.

### D-001 {Decision title}
**Decided:** YYYY-MM-DD
**Recorded:** YYYY-MM-DD
**Status:** Accepted
**Authors:** {author} (with {critic tool} review {date} if applicable)
**Related:** F-NNN, C-NNN, AV-NNN

**Context.** {Why this decision was needed.}

**Options.**
- **A. {Option}.** {Why rejected or chosen.}
- **B. {Option}.** {Why rejected or chosen.}

**Decision.** {Chosen option, restated.}

**Consequences.**
- {Implication.}

**Reversal conditions.** {What evidence would change this decision.}
```

### `ATTACK_VECTORS.md` skeleton

```markdown
# Attack Vectors

Project-specific failure modes. Severity: Critical | Major | Minor.

## {Category}

### AV-001 {Vector title}
**Severity:** Critical
**Description.** {What can go wrong.}
**Detection.** {How it is checked — test path, tool, manual review.}
**Related decisions.** D-NNN
**Related claims.** C-NNN
**History.** {When/how this vector was identified, prior incidents.}
```

### `BUGS.md` skeleton

```markdown
# Bugs

Catalogue of bugs discovered during development. Per Maintenance Rule 8,
bugs are logged here when found, not silently fixed. The author decides
whether to fix immediately, defer, or leave alone.

Status vocabulary: open | fixed | wontfix | deferred.
Severity vocabulary: low | medium | high.

## Open

### BUG-001: {short title}
**Status:** open
**Found:** YYYY-MM-DD ({session/commit context})
**Location:** {path/to/file.ext:line, or "cross-cutting"}
**Severity:** {low | medium | high}
**Description.** {What's wrong and why it matters.}
**Reproduction.** {Minimum steps to trigger.}
**Notes.** {Related context, suggested fix, links to BUG/IMP/D/AV entries.}

## Fixed
{Entries with `Status: fixed (YYYY-MM-DD)`.}

## Won't Fix
{Entries with `Status: wontfix`.}

## Deferred
{Entries with `Status: deferred`.}
```

### `IMPROVEMENTS.md` skeleton

```markdown
# Improvements

Catalogue of code-quality improvements, refactors, and architectural
changes proposed during development. Per Maintenance Rule 8, improvements
are logged here when noticed, not silently applied. The author decides
whether to apply, defer, or decline.

This is the dual of BUGS.md: bugs are broken; improvements work but
could be better.

Status vocabulary: suggested | applied | declined | deferred.
Effort vocabulary: trivial | small | medium | large.

## Suggested

### IMP-001: {short title}
**Status:** suggested
**Found:** YYYY-MM-DD ({session/commit context})
**Location:** {path/to/file.ext:line, or "cross-cutting"}
**Effort:** {trivial | small | medium | large}
**Description.** {What could be improved and why.}
**Proposal.** {How to do it.}
**Trade-offs.** {What we'd give up or risk. Required — without this the entry is a feature request, not a candidate improvement.}
**Notes.** {Related context, dependencies on other work.}

## Applied
{Entries with `Status: applied (YYYY-MM-DD)`.}

## Declined
{Entries with `Status: declined`, kept as audit trail.}

## Deferred
{Entries with `Status: deferred`.}
```

### `CLAUDE.md` skeleton

````markdown
# CLAUDE.md

## Project
{2–3 sentences.}

## Current state
- {file/module}: {status}

## Active task
{Immediate next work item. Reference feature IDs.}

## Invariants
- {rule that must hold}

## Build & test
```bash
{commands}
```

## Conventions
- {naming, style, commit format}

## Pitfalls
- See ATTACK_VECTORS.md for the canonical list.
- {session-specific gotcha}

## Out of scope
- {do not change without asking}
````

### `ROADMAP.md` skeleton

```markdown
# Roadmap

## Phase 1 — Foundation
**Goal:** {one sentence}
**Status:** In progress
**Features delivered:** F-001, F-002
**Deliverables:**
- [ ] {item}
**Acceptance:** {criterion}
```

### `CHANGELOG.md` skeleton

```markdown
# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com).

## [Unreleased]
### Added
- Initial scaffolding.
```

---

## A note on cost

Documentation is not free. Every doc added is a doc that must be maintained, kept consistent with the code, and refreshed as understanding evolves. Stale docs are worse than missing docs because they actively mislead — a reader who finds no document knows they have to ask; a reader who finds an outdated document acts on bad information.

The Quick Decision Guide below leans towards adding documents because the absence of a needed doc usually costs more than the maintenance burden of a present one — but this is a default, not a rule. Skip a Tier 2 or Tier 3 doc when:

- The project is small enough that the README plus code comments capture everything (toy scripts, single-file utilities, throwaway experiments).
- A specific doc's content is genuinely covered by another (e.g. `ARCHITECTURE.md` may be unnecessary for a single-file project; `DECISIONS.md` may be unnecessary if no significant choices were made).
- Maintenance burden would exceed signal value (don't write a CHANGELOG for a project no one else will ever read; don't write CLAIMS for code that makes no scientific assertions).

The same friction test applies to **Tier 1**, with one additional requirement: any Tier 1 omission must be recorded as a `DECISIONS.md` entry (see the Tier 1 table above). This keeps the discipline — Tier 1 omissions are principled exemptions with stated reasoning, not silent gaps. Examples: a documentation-only repo may legitimately exempt `FEATURES.md` and `ROADMAP.md` because the deliverable has no features or phases in the conventional sense; a solo-developer toy project may exempt `CLAUDE.md` if no AI-assisted sessions are planned. In each case the friction test is the operational threshold, and the DECISIONS entry is the audit trail.

The operational test is **friction**: are you currently or recently confused about something the absent doc would have answered? If yes, add it. If no, the absence isn't costing you anything yet. Adding documents pre-emptively against confusion that never arrives is itself a form of waste.

**Future-revival friction test (Complete-state mode).** For projects transitioning to Complete, the present-tense friction test fires for nothing — no one is currently developing, so no one is currently confused. But Complete is precisely the state in which future-revival cost matters most, because the project is about to enter a long quiet period during which context evaporates. For Complete projects (and retroactive completion in particular — see **Retroactive completion** above), replace the present-tense friction test with a forward-tense variant:

> *If you (or a successor) returned to this project in 6–18 months — because of a candidate result, a hardware change, a citation request, or a bug report — which absent document would force the revival to be archaeology rather than execution?*

Threshold tiers for the forward-tense test:

- **Mandatory:** any document whose absence would prevent revival from happening at all. A broken build recipe, no record of which version produced which result, no record of which performance / correctness claims were realised vs aspirational.
- **Strongly recommended:** any document whose absence would cost more than half a day of revival time. Design decision rationale, an architectural map, the list of known failure modes.
- **Case-by-case:** documents that only marginally accelerate revival.

The forward-tense test is not a relaxation of the standard for Complete projects — it is the same friction test applied to a different temporal horizon. Adopting it makes the Complete-state cost analysis honest about the structural difference (friction is predicted, not experienced).

The Tier 1 set is the default minimum below which projects reliably suffer; exemptions are narrow and documented. Tier 2 and Tier 3 are case-by-case.

---

## Quick Decision Guide

When starting a new project, ask:

1. Will I (or anyone else) need to pick this up after >1 month away? → **Tier 1 mandatory.**
2. More than one or two non-trivial design choices expected? → Add **`DECISIONS.md`** from day one.
3. Does the build involve more than one command, or non-trivial dependencies? → Add **`BUILD.md`**.
4. Are there >3 modules or non-obvious design decisions? → Add **`ARCHITECTURE.md`**.
5. Is there a formal spec (file format, protocol, equations, parameters)? → Add **`SPEC.md`** (or domain name).
6. Does the project have well-defined failure modes (physics validity, performance budgets, numerical stability, correctness invariants)? → Add **`ATTACK_VECTORS.md`**.
7. Is this a research project with empirical, theoretical, or mathematical claims? → Add **`CLAIMS.md`** (effectively mandatory for research).
8. Does this project share terms/types with a sibling repo? → Add **`VOCABULARY.md`**.
9. Will external contributors or security researchers interact with the repo? → Add **`CONTRIBUTING.md`** / **`SECURITY.md`**.
10. Is performance a stated requirement? → Add **`BENCHMARKS.md`**.
11. Do you want bug history in-repo (rather than relying solely on GitHub Issues / Jira / Linear)? → Add **`BUGS.md`**. Especially valuable for solo-dev and AI-partner workflows; redundant with most external trackers.
12. Do you want a tracked, persistent list of candidate refactors and code-quality improvements (distinct from features and decisions)? → Add **`IMPROVEMENTS.md`**. Pairs with `BUGS.md` under the same "log when found, not silently acted on" discipline (Maintenance Rule 8).

If in doubt, start with Tier 1 + `ARCHITECTURE.md` + `DECISIONS.md` and add others as the project grows and friction reveals which are needed.
