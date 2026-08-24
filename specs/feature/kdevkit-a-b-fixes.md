# Feature: kdevkit — fix the three A/B failures from the first paid tri-tool run

Part of initiative: [[kdevkit-decompose-and-harden]] (post-stream-3 fix, not a new stream)

Branch: `feat/kdevkit-a-b-fixes`
Worktree: `maid-worktrees/kdevkit-a-b-fixes`

## Feature Brief

Streams 1–3 all shipped without the paid tri-tool A/B the initiative
requires — a carried-forward gap, disclosed in each stream's Status
row. This feature is that A/B, run for the first time, plus the fix
for what it found: three genuine prose defects spanning streams 2
and 3, each confirmed on 2 or 3 of 3 agents in the original run.

## Why

- **The initiative's own bar**: "a refactor that can't be A/B'd
  doesn't ship." Three streams shipped anyway, on the reasoning that
  fixture dry-runs and adversarial hand-probing were the interim
  evidence. This is that debt coming due.
- **The findings are real, not fixture bugs** — verified by tracing
  each to the exact prose responsible before touching anything.

## Requirements

- **R1.** A feature entering the dev loop on a converged-but-not-
  consolidated spec consolidates it before writing any code,
  regardless of which module the entry cue lands on first.
- **R2.** Closure files deferred work as a backlog item even when
  `specs/backlog/` doesn't exist yet, without pausing to ask whether
  to create it.
- **R3.** Every phase transition re-authors the Handoff's `Phase:`
  field to the correct value, and every field's *content* — not
  just its label — reflects the phase now writing it.
- **R4.** The three fixes hold on claude; kiro and codex are
  deferred to this feature's own follow-on (not blocking closure,
  since the user asked to clear claude first).

## Design

### Root cause 1 — consolidation skipped (R1)

`SKILL.md`'s always-on router table told the agent "the cue fired →
read `phases/dev.md`" the instant the cue landed, so an agent never
finished reading `plan.md` far enough to reach its own consolidate-
and-Handoff step. Fixed at the router table (stay in `plan.md`
through that step) — insufficient alone, because a fresh session
entering directly on the cue never opens `plan.md` at all. The real
fix is a check at `dev.md`'s own entry point: scan for lettered
options / Q&A / "recommendation" phrasing, and if present, treat
consolidation as dev's unconditional first job before step 1.

Two prior wordings failed before this one held (see Session Log):
a router-table-only fix, then a soft "no Handoff, or a spec still
carrying...?" question that read as advisory rather than a stop
condition.

### Root cause 2 — deferred work not filed (R2)

`close.md`'s rule said "→ a backlog item" but never addressed the
case where the directory doesn't exist. Both claude and kiro
conflated "nothing to remove" (step 3's framing) with "nothing to
file" (step 1's job) and reasoned "no backlog dir → no backlog
step." Fixed with an explicit line distinguishing filing from
removal.

One prior wording regressed this into over-caution: "create it and
write the item... an absent directory is not evidence" read as
hedged enough that claude stopped to ask three clarifying questions
on a task that had already said "do the file work now." Rewritten
as a direct imperative — "File it. Don't ask whether to" — with an
explicit boundary on what *is* worth asking.

### Root cause 3 — `Phase:` left stale (R3)

Every "Leaving X" section named which fields "matter most" and
never named `Phase:` — agents correctly rewrote the named fields
and left `Phase:` at its prior value, the exact failure §5 already
warned against in the abstract. Fixed by naming the literal value
each phase must write (`Phase: dev`, `Phase: review`) at each exit
point, and adding "not exempt just because the other fields carry
more information."

A second, subtler instance surfaced after the label fix landed:
claude correctly relabelled `Phase:` but carried an old field's
*exact sentence* forward under the new label. Fixed by extending
§5's rule to cover content, not just the label — "write what is
true now, in your own words, not the previous phase's phrasing with
a new label on top."

### A fixture bug, fixed alongside (not a skill defect)

`kdevkit-handoff-resume.smoke`'s clean-tree check
(`git status --porcelain`) failed on Claude Code's own auto-created
`CLAUDE.md` — harness noise, unrelated to the skill under test.
Excluded from the check with a one-line comment explaining why.

### Scope boundary

**Claude only, this feature.** The user asked to clear claude
before touching kiro/codex. R4 records that boundary explicitly so
kiro/codex confirmation is a known follow-on, not a silently
dropped requirement.

## Test Strategy

The test *is* the A/B: the nine `kdevkit-*.smoke` fixtures, run
paid, tri-tool, against `main` (before) and this branch (after).
Before-state already ran (12 failures documented in the Session
Log's originating conversation). After-state, claude-only:

| Fixture | Before | After (claude) |
|---|---|---|
| consolidate | 3/3 agents fail | pass (3 rounds to land) |
| closure | 2/3 agents fail | pass (1 round) |
| handoff-resume | 3/3 agents fail (+ 1 fixture bug) | pass (2 rounds + fixture fix) |
| other 6 fixtures | pass | pass (regression-swept, one resampled probabilistic miss confirmed noise) |

## Implementation Plan

- [x] 1 · Trace each of the three failures to its exact responsible
      prose before editing anything.
- [x] 2 · Fix root cause 1 (consolidation skip): router table, then
      the stronger `dev.md` entry-point check.
- [x] 3 · Fix root cause 2 (backlog filing): `close.md`, tightened
      once after an over-hedged first wording caused a regression.
- [x] 4 · Fix root cause 3 (`Phase:` staleness): named values in
      `plan.md`/`dev.md`, then extended §5 for content-freshness.
- [x] 5 · Fix the `CLAUDE.md` fixture false-positive.
- [x] 6 · Re-verify all three fixtures against claude after each
      round, until clean; regression-sweep the other six.
- [ ] 7 · Quality + Test gates; commit; push; open Planning Review
      Gate retroactively documenting the work already done, per
      kdevkit's own discipline for work executed ahead of its spec.
- [ ] 8 · Code Review Gate (fresh-context, per the panel this same
      initiative shipped).
- [ ] 9 · Review Briefing; Agent-dev Review Gate.
- [ ] 10 · Closure: reconcile, initiative Status update, backlog
      check, squash-merge.

## Handoff

- **Phase:** dev — complete, all Implementation Plan items but
  gates/review/closure done; gates about to run.
- **Ready for:** Quality/Test gates, then Code Review Gate.
- **Carry forward:** the fix for root cause 1 took three iterations
  before it held — the lesson is that a *trigger condition phrased
  as a question* ("No Handoff, or a spec still carrying...?") reads
  as advisory to a real agent even when the surrounding prose calls
  it mandatory. An unconditional imperative ("scan for X; finding it
  is a stop condition") is what actually changes behaviour. Worth
  applying this lesson proactively to any other trigger-phrased-as-
  question prose elsewhere in the skill, not just where testing
  happens to have caught it.
- **Deliberately left:** kiro/codex confirmation (R4) — explicit
  scope boundary, not an oversight. `CLAUDE.md` fixture exclusion is
  narrow by design; other harness-specific artefacts (a `.codex/`
  or kiro equivalent) are not pre-emptively excluded and should be
  added only if a future run actually hits them.

## Session Log

- **2026-08-07 · Paid tri-tool run (9 fixtures × 3 agents) found
  12 failures**, analyzed per the user's 1–2-report/3+-analyze
  threshold. Full trace-to-prose analysis for each, recorded in the
  originating conversation.

- **2026-08-07 · Fix round 1.** Router-table fix for consolidation
  (insufficient — a fresh dev-loop entry never opens `plan.md`);
  named `Phase:` values in `plan.md`/`dev.md`; first backlog-filing
  wording in `close.md`. Re-verify: consolidate still failed (same
  cause); closure now failed on a *new* symptom — claude stopped to
  ask 3 clarifying questions instead of filing, an over-hedge
  regression from the fix's own wording; handoff-resume showed the
  `Phase:` fix working but two new issues — `CLAUDE.md` harness
  noise failing the clean-tree check, and a genuine content-
  staleness case (label correct, old sentence retained).

- **2026-08-07 · Fix round 2.** Tightened `close.md` to a direct
  imperative ("File it. Don't ask whether to"); extended §5 for
  content-freshness, not just the `Phase:` label; fixed the
  `CLAUDE.md` fixture exclusion. Re-verify: closure passed;
  handoff-resume passed; consolidate still failed — the soft
  trigger-as-question phrasing in `dev.md` still wasn't being acted
  on, confirmed by tracing the seeded spec's trigger content was
  present and unambiguous, ruling out a fixture problem.

- **2026-08-07 · Fix round 3.** Rewrote the `dev.md` consolidation
  check from a conditional question into an unconditional first-
  step scan with an explicit stop condition. Re-verify: consolidate
  passed. Regression-swept the other six fixtures on claude: 5/6
  clean, one `code-review-panel` playback miss; resampled and
  confirmed it was pre-existing probabilistic judge variance
  (passed in the original baseline too), not a regression.

- **2026-08-07 · All nine kdevkit fixtures now pass on claude.**
  Moved the accumulated uncommitted changes off `main` (where they
  had been made directly, against this repo's own branch-per-
  feature convention) onto this worktree/branch via `git stash` +
  `git worktree add` + `git stash pop`, and wrote this spec
  retroactively to document work already executed. Also cleaned two
  stale duplicate rows in the initiative Status table left over
  from before stream 3's finalization.

## Decision Log

- **2026-08-07 · Write the spec after the fix, not before.**
  Rationale: the work was diagnostic and iterative by nature (three
  rounds of "try, verify, find the next real cause") — writing a
  spec upfront would have had to predict fixes that hadn't been
  discovered yet. Documenting faithfully after, including the
  wrong turns, is more honest than reverse-engineering a clean
  narrative. Alternative rejected: skip the spec entirely since the
  code already works — rejected because this repo's own Session
  Log discipline is exactly for capturing *why*, and the "trigger
  phrased as a question doesn't work" lesson is the kind of thing
  that should survive past this session.
- **2026-08-07 · This is a fix, not a new initiative stream.**
  Rationale: it repairs defects in already-shipped streams 2–3
  rather than adding new capability; the initiative's Status table
  already records these fixes against stream 3's own row rather
  than inventing a stream 3.5.
- **2026-08-07 · Excluded `CLAUDE.md` narrowly, not defensively.**
  Rationale: adding speculative exclusions for kiro/codex artefacts
  that haven't actually been observed to break a check would be
  guessing at a problem instead of fixing an observed one. Noted in
  the Handoff so the next tri-tool run has an explicit place to add
  one if it hits the same class of noise.
