# Feature: kdevkit — the spec is the phase handoff record

Part of initiative: [[kdevkit-decompose-and-harden]] (stream 2 of 6)

## Git Setup

- Branch: `feat/kdevkit-phase-handoff`
- Base: `main` @ `34ad980`

## Feature Brief

A phase can now be handed to a fresh agent — or resumed after a
session dies — because the outgoing phase writes what the next one
needs into the checked-in feature spec. Today crossing a boundary
keeps working only if the *same* session continues; the modules
land the right rules, but nothing carries the state.

Two additions, and one subtraction:

- **A `## Handoff` section** in the feature spec, rewritten (not
  appended) at each phase boundary: current phase, what the next
  phase must know, and what was deliberately left.
- **A consolidation step** at planning → dev, so what the dev
  phase inherits is a spec rather than a transcript.
- **Nothing new on disk.** No new artefact and no new file — both
  live in the spec that already exists.

## Requirements

- **R1.** Crossing a phase boundary writes the handoff into the
  feature spec *before* the next phase begins. A phase that ended
  without one did not finish.
- **R2.** A fresh agent given only the repo, the branch, and the
  spec can resume mid-feature and know: which phase is live, what
  the previous phase concluded, what is deliberately unresolved,
  and what to do next.
- **R3.** The handoff is one section, rewritten in place — never an
  append-only log. Stale handoff content is deleted, not
  accumulated.
- **R4.** The mechanically-derivable parts (branch, unticked plan
  items, gate status, open review findings) are read from git and
  the spec, not recalled from memory.
- **R5.** At planning → dev the spec is consolidated: decisions
  stated as decisions, superseded options removed, open questions
  separated from settled ones. A reader who saw no discussion can
  implement from it.
- **R6.** Consolidation never deletes rationale that constrains
  future work; it relocates it (Decision Log, or up into
  `project.md` at closure) rather than dropping it.
- **R7.** Both fire only where they earn it: a trivial change with
  no iteration has nothing to consolidate, and a single-session
  feature still writes handoffs because sessions die.

## Design

- **`## Handoff` sits directly after `## Feature Brief`** in the
  template — high enough that a resuming agent reads it before the
  detail, and adjacent to the initiative link that establishes
  context. Four fixed fields keep it short and skimmable:
  `Phase:` · `Ready for:` · `Carry forward:` · `Deliberately
  left:`.
- **Budget: 15 lines.** Long enough to carry judgement, short
  enough that it cannot become a second Session Log. The
  initiative's stated target (800–1500 tokens) is the ceiling;
  this is the practical shape.
- **Written by the outgoing phase, read by the incoming one.** So
  the instruction belongs in each phase module at its exit, plus
  one always-on rule in §5 stating the invariant. Placing it only
  in the core would leave it to be remembered at exactly the
  moment context is being dropped.
- **Derivable-vs-judgement split** (the initiative's code/prose
  line, applied): branch, ticked/unticked plan items, and gate
  results are *observations* the agent reads off git and the spec;
  "what to watch for" and "why this was left" are judgement it
  authors. Prose says which is which so a later code accelerator
  (stream 4) has a clean seam.
- **Consolidation is a rewrite, not an edit pass**, and it lands
  as its own `plan(<feature>): consolidate spec` commit so the
  diff is reviewable as a separate act.
- **The archive is the PR/CR conversation** — the discussion
  already happened there and is durable. The spec simply stops
  carrying it. No `feature/<name>.planning.md` sibling (§6 refused
  `research.md` on the same grounds).
- **Placement:** the `## Handoff` template block and the
  consolidation checklist are one-shot templates, so they go in
  `interviews.md`; the always-on invariant goes in `SKILL.md` §5;
  the per-phase write instructions go in each phase module. This
  is the skill's own placement rule.

### What this deliberately does not do

Does not orchestrate anything — no dispatching, no session
management, no deciding *whether* to hand off. It makes the
handoff *possible* and *complete*; who runs which phase where is
streams 4–5. That boundary is what keeps this stream shippable
without the code-vs-prose question being settled.

## Test Strategy

| Success criterion | Layer | How |
|---|---|---|
| Handoff written at a boundary (R1) | functional | extend `kdevkit-phase-boundary.smoke`: assert a `## Handoff` block exists with `Phase:` naming dev after the planning→dev cue |
| Fresh agent resumes correctly (R2) | functional | new fixture: seed a mid-dev spec *with* a handoff, give a bare "continue this", assert it resumes the named phase rather than re-planning |
| Rewritten not appended (R3) | functional | same fixture: seed a stale handoff naming planning; assert exactly one `## Handoff` and that it no longer says planning |
| Consolidation happens (R5) | functional | new fixture: seed a spec carrying options + Q&A, give the planning→dev cue, assert options are gone and a `consolidate` commit exists |
| Rationale survives (R6) | functional | same fixture: assert a load-bearing "chose X because Y" line is still present after consolidation |
| Content still validates (all) | unit | `just test` |

Fixture authoring is agentic; the paid tri-tool run is the human's
call per `project.md`. **Stream 1 shipped without it and that is
now a known gap** — this stream adds fixtures to the same A/B set
rather than assuming it ran.

## Implementation Plan

- [x] 1 · Add the `## Handoff` block + field semantics to the
      feature file template in `interviews.md`.
- [x] 2 · Add the consolidation checklist to `interviews.md`
      (what to strip, what to keep, what to relocate).
- [x] 3 · `SKILL.md` §5: the always-on invariant — a phase writes
      its handoff before the boundary; an absent handoff means the
      phase did not finish.
- [x] 4 · `phases/plan.md`: consolidate + write handoff as the
      last steps before the planning→dev cue; extend the
      Plan-commit rule's numbered sequence.
- [x] 5 · `phases/dev.md`: read the handoff on entry; write one at
      dev→review.
- [x] 6 · `phases/review.md`: read on entry; write at
      review→closure.
- [x] 7 · `phases/close.md`: read on entry; §8.1 reconcile also
      clears the handoff (the feature is done; a live handoff
      would be stale).
- [x] 8 · Extend `kdevkit-phase-boundary.smoke` with the R1
      assertion.
- [x] 9 · New `kdevkit-handoff-resume.smoke` (R2, R3).
- [x] 10 · New `kdevkit-consolidate.smoke` (R5, R6).
- [x] 11 · Gates + `verify-skills-dry`; confirm every new fixture
      fails a no-op agent.
- [x] 12 · Close the consolidation backlog item; note in
      `project.md` if the spec-template shape is described there.

## Handoff

- **Phase:** dev — complete, gates green.
- **Ready for:** human review; the Agent-dev Review Gate is next.
- **Carry forward:** three fixture bugs surfaced while proving
  discrimination, all mine, all the same class — an assertion that
  passes without the agent doing anything. Two were unreachable
  criteria (`wc -l` can't see a trailing newline through command
  substitution) and one was assuming git's default branch name.
  The seeded repos now name their base branch explicitly. A
  reviewer should weigh whether every new assert really fails a
  no-op, since that is where I keep erring.
- **Deliberately left:** the closure-clears-handoff question from
  planning is now decided — `phases/close.md` sets
  `Phase: closed` rather than deleting the block, so a merged spec
  reads as finished rather than as missing a section. No fixture
  covers closure clearing it; the closure fixture is untouched by
  this stream.

## Session Log

- **2026-08-04 · Code review: FAIL. Five blockers, all in my
  fixtures, all the same class.** The reviewer's verdict was right
  and the finding is uncomfortable: the fixture meant to prove the
  headline behaviour was largely vacuous — it could not fail the
  exact mistake the consolidation rule exists to prevent. Verified
  each blocker empirically before fixing.

  - **The "rationale survived" guard was vacuous.** I seeded the
    load-bearing *why* in **two** places — inside the strippable D1
    block *and* the Decision Log — then asserted it existed
    somewhere. An agent that deleted the entire Design body passed.
    Now seeded in exactly one strippable place, and the assert
    requires it to have **relocated into the Decision Log**, which
    is what the checklist actually specifies.
  - **The R2 criterion was unsatisfiable.** `wc -l` cannot observe
    a trailing newline through command substitution, so an agent
    following the spec's own decided idiom got `1` where I demanded
    `2`. Correct agents would have failed. Replaced with a
    character count, where the natural implementation gives the
    expected value and char-vs-word counts still differ.
  - **`awk '\<dev\>'` is a GNU extension.** Under mawk or BusyBox
    it matches nothing, so the strongest assert in two fixtures
    would have *silently passed*. Now a portable character-class
    pattern.
  - **Every negative check was satisfiable by deletion.** "The
    block no longer says planning" passes if the block has no
    `Phase:` field at all. Each negative is now paired with a
    positive existence check.
  - **The consolidate fixture never checked the handoff** — and
    planning→dev is precisely where the block is first authored, so
    the invariant's genesis case was untested.

  Also fixed: a seeded "constraint" that was **factually false**
  (GNU sed does support `\+`, so a diligent agent would correctly
  discover the handoff was lying, and the skill tells it to trust
  the repo); an exact tick-count that would fail an agent
  legitimately adding a slice; `Phase: closed` missing from the
  template enum; no version bump. Added closure coverage for
  clearing the block, and a `playback` for the two guardrails that
  leave no artefact.

  **The pattern is now unmistakable across two streams:** every
  defect in this work has been an assertion that passes without the
  agent doing anything. Writing the test after the behaviour is
  what produces it, which is the argument the repo's own
  test-first-per-slice backlog item makes. Worth acting on in a
  later stream rather than continuing to catch these in review.

  Re-verified all four fixtures against purpose-built
  non-compliant agents — **consolidate**: no-op ✗,
  not-consolidated ✗, rationale-deleted ✗, no-handoff ✗,
  compliant ✓. **resume**: no-op ✗, stale ✗, Phase-deleted ✗,
  breaks-R1 ✗, compliant ✓. **boundary**: no-op ✗, stale ✗,
  Phase-deleted ✗, compliant ✓. **closure**: no-op ✗, left-at-review
  ✗, block-deleted ✗, compliant ✓.

- **2026-08-04 · Dev complete; dogfooded the feature on itself.**
  Six prose edits (template + checklist in `interviews.md`, the §5
  invariant, four module read/write points) and three fixtures.
  This spec now carries its own `## Handoff` block, written by the
  phase that just ended — the feature applied to its own delivery.

  **Fixture discrimination is where the work actually was.** Every
  new assert was run against a purpose-built non-compliant agent,
  and three of my own assertions failed that check before passing
  it:
  - `wc -l` cannot observe a trailing newline through command
    substitution, so the R2 criterion I first wrote was
    *unsatisfiable* — no agent could have passed it. Replaced with
    a two-line/three-word input that also distinguishes
    line-counting from word-counting.
  - Both new fixtures assumed `git init` produces `main`; it
    produces `master` here, so a `main..branch` range silently
    errored and the assertion never ran. Fixed by seeding with
    `git init -b base` and anchoring on that.
  - The boundary fixture's new handoff check needed to catch an
    agent that does the work but leaves the block stale — verified
    it does.

  Final matrices: **boundary** — no-op ✗, stale-handoff ✗,
  compliant ✓. **resume** — no-op ✗, stale-handoff ✗, re-planned ✗,
  compliant ✓. **consolidate** — no-op ✗, not-consolidated ✗,
  rationale-deleted ✗, compliant ✓.

  Gates: `fmt-check`, `lint`, 98 tests, full dry-run — green.

- **2026-08-04 · Stream 2 opened.** Grounded on `main` @ `34ad980`
  (stream 1 merged): read the core's boundary-crossing prose, the
  existing "Keep the feature file current" rule (the nearest thing
  today — it keeps logs current but carries no state across a
  boundary), the feature template in `interviews.md`, and the
  consolidation backlog item this stream folds in.

  **Finding: the handoff has a natural home next to the
  initiative link**, and the template already has the right shape for
  a fixed-field block. Also: the existing §8.1 reconcile is the
  model for "an absent artefact is itself a signal" — reused as
  R1's failure semantics rather than inventing a new mechanism.

## Decision Log

- **2026-08-04 · Handoff is a rewritten section, not an appended
  log.** Rationale: its job is "what the *next* phase needs," which
  is current-state, not history — and an append-only handoff grows
  into a second Session Log, which is the bloat this initiative
  exists to remove. History already has two homes (Session Log,
  and the PR conversation). Alternative rejected: append-with-
  timestamps, which reads as an audit trail nobody consumes.

- **2026-08-04 · Write instructions live in the phase modules, not
  only in the core.** Rationale: the write happens at the moment
  the outgoing phase ends, which is exactly when its module is
  still loaded and the core may have been in context for a long
  time. Keeping the invariant in §5 *and* the instruction at each
  exit is the redundancy the trigger-row defect in stream 1 argues
  for. Alternative rejected: core-only — relies on remembering a
  rule at the point of highest context pressure.

- **2026-08-04 · The PR conversation is the consolidation
  archive.** Rationale: the discussion already lives there durably,
  so a second copy in the repo is duplication that immediately goes
  stale. Alternatives rejected: a `## Planning Archive` section
  (keeps the file long, defeating the point) and a sibling
  `.planning.md` file (§6 already refused `research.md`).
