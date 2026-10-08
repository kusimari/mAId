# kdevkit — interviews and templates (deferred)

This file carries the interview scripts and file templates that
fire only at **feature genesis** (start a fresh feature),
**backlog capture**, or **initiative genesis**. Loaded by main
on demand via inline-Read at the moment of need; not always-on.

## Four short interviews

Run when entering a feature with no spec on disk (start mode,
neither `feature/` nor `backlog/` has the file). One per
topic; skip what existing project context already answers.
Order matters: tests sit immediately after requirements so
success criteria are declared before the design converges —
the dev loop (`phases/dev.md` §7) then has a verifiable target, not
a sketch to validate after the fact.

1. **Requirements (the experience layer).** How does the
   user experience the capability — what do they touch, what
   do they observe? For a CLI, flags and output. For an app,
   screens and visible state. For a skill change, the cues
   the agent recognises and the artefacts it produces. For a
   service, request shape and response. Library names,
   internal file paths, function/schema names, and protocol
   verbs go in Design — not here. (See `phases/plan.md` §6's
   Requirements smell test.)
2. **Test strategy.** Per `project.md`'s Testing section:
   which layers fire for this change, what are the success
   criteria, what's load-bearing vs. nice-to-have? Map onto
   existing test commands; don't invent new layers. The
   V-model pairing — functional/integration cases verify
   Requirements in user-observable terms; unit tests verify
   Design primitives — is the default; situation overrides
   when it doesn't fit.
3. **Design.** Lead with rationale — why this shape, what
   was considered and rejected, including **what well-known
   library or language idiom already does this job** (name it
   before designing a hand-rolled alternative; see `phases/plan.md` §6
   "Reach for what exists"). Then the technical approach:
   components, interactions, trade-offs. A reader shouldn't
   reach the end of Design before learning why it's shaped
   the way it is.
4. **Implementation plan.** Ordered tasks + risk notes.

The interviews are scaffolding — the actual spec layout
adapts to what the feature needs. The skill's strictness
lives in the gates (`phases/plan.md` §6 / `phases/dev.md` §7 / `phases/close.md` §8), not in heading shape.

**These are your own checklist, not a questionnaire for the
user.** Answer each from the grounding pass — `project.md`, the
backlog item, the code you just read — and write the spec. Do
not reply with the four interview answers in chat and stop for
confirmation: the file on disk is the reviewable artefact, and
an agent that asks "do these four read right to you?" before
writing has produced nothing to review. Anything you truly
cannot infer becomes an open question *recorded in the spec*,
not a blocking prompt.

After the four interviews, write the feature spec from the
template below, then return to `phases/plan.md` §6's Plan-commit rule
(commit + push + open Planning Review Gate).

## Feature file template

```markdown
# Feature: <name>

## Git Setup

- Branch: <branch-name>
- Base: <commit-ish or branch>

## Feature Brief

<!-- The capability layer — what can the user now do that
     they couldn't before? Don't describe the experience or
     the design here; those have their own sections. -->

<one paragraph — the new capability>

<!-- Optional, populated by phases/plan.md §6 auto-link when this
     feature is a stream of an active initiative:
Part of initiative: [[<name>]]
-->

## Handoff

<!-- Two parts, behaving differently (§ The handoff record):

     CURRENT STATE is rewritten at every boundary by the phase that
     is ENDING, and read on entry by the phase that is starting.
     Replace all four fields; don't append, and don't leave one
     field stale while updating another. Keep it under ~15 lines:
     it carries what the next phase can't derive, not a summary.

     CROSSINGS is appended to and never edited. One line per
     crossing. The count of RETURN lines is the only record of how
     many times this feature has gone back.

     Stage:             which stage is live now, or `closed` once
                        closure has finished with the feature.
     Ready for:         the next stage, and what gates it.
     Carry forward:     what the next phase would otherwise have
                        to rediscover — a constraint found late, a
                        finding still open, a trap.
     Deliberately left: what was NOT done and why, so the next
                        phase doesn't redo the decision or mistake
                        the gap for an oversight.

     Derivable facts (branch, unticked plan items, gate results)
     are READ from git and this spec at entry — don't copy them
     here and let them rot. This block is judgement only. -->

- **Stage:** <research | planning | dev | review | closure | closed>
- **Ready for:** <next stage, and its gate>
- **Carry forward:** <what the next stage must know>
- **Deliberately left:** <what's unresolved, and why>

### Crossings

<!-- Append one line per crossing; never edit or delete a line.
     A RETURN needs four parts: fault / issue / fix / done when.
     An EXCEPTION needs two: skipping / why. -->

- <from> → <to>

## Requirements

<!-- The experience layer — what the user touches and
     observes. CLI: flags and output. App: screens and
     visible state. Skill change: the cues the agent
     recognises and the artefacts it produces. Service:
     request shape and response.

     Smell test (`phases/plan.md` §6): library names, internal
     file paths, function/schema names, and protocol verbs
     belong in Design, not here.

     Split into ### Launch experience (one-shot) and
     ### Runtime experience (ongoing) when both apply;
     keep it a single block otherwise. -->

<bullet list — what the user observes>

## Test Strategy

<!-- Success criteria mapped onto project.md test layers.
     V-model default: functional/integration tests verify
     Requirements in user-observable terms; unit tests
     verify Design primitives. Group cases under H3
     subheadings (### Functional / Integration, ### Unit
     Tests, ### Smoke, etc.) when the spec has enough test
     surface to warrant it; keep it flat otherwise. -->

<success criteria, mapped onto project.md test layers>

## Design

<!-- The "how" layer — schemas, plumbing, libraries,
     project conventions. Lead with rationale: why this
     shape, what was considered and rejected. The reader
     shouldn't reach the end of Design before learning why
     it's shaped the way it is. -->

<rationale first; then technical approach, components, interactions>

## Implementation Plan

<!-- Markdown task-list shape. One slice per item. Tick
     `- [ ]` to `- [x]` in the same commit that completes
     the slice. Mid-slice work stays unchecked. The closure
     reconcile sweep greps for unchecked boxes. -->

- [ ] <slice 1>
- [ ] <slice 2>
- [ ] <slice 3>

<!-- Risk notes: bullet list under the checklist. -->

- *Risk note:* <consideration>

## Session Log

<!-- append: date · what was done · decisions made -->

## Decision Log

<!-- append: decision · rationale · alternatives rejected -->
```

## Consolidation checklist (planning → dev)

Fires once, at the planning → dev boundary, before the dev loop
starts. The spec stops being the record of *how the plan was
reached* and becomes the contract the dev phase builds from — a
reader who saw none of the discussion must be able to implement
from it.

**Why it can't wait for closure:** once phases run as separate
agents, the dev agent reads the spec *without* the conversation
that disambiguates it. An unresolved option list is then
indistinguishable from a requirement, so the agent may build the
thing that was argued against.

Strip:

- **Superseded options.** Keep the decision, drop the lettered
  alternatives and the "recommended" markers. `(a)/(b)/(c)` in a
  shipped spec is an unmade decision.
- **Round-by-round Q&A.** A reply to a reviewer belongs in the
  PR/CR thread; the *rule it settled* belongs in the spec body.
- **Revision narration.** "Revised after research", "changed from
  X" — the diff and the thread carry that.

Keep, and state as decisions:

- Every settled decision in the imperative — what the build does,
  not what was considered.
- **Rationale that constrains future work.** Relocate it to the
  Decision Log rather than deleting it; §8 closure is what
  promotes the binding ones into `project.md`. Deleting a *why*
  that a later feature needs is the one unrecoverable mistake
  here.
- Open questions, clearly separated from settled ones, with an
  owner. A reader must be able to tell "build this" from "don't
  build this yet".

**The archive is the PR/CR conversation** — it already holds the
discussion durably, so don't copy it into the repo. No
`<feature>.planning.md`, no `## Planning Archive` section.

Then commit as `plan(<feature>): consolidate spec` so the rewrite
is reviewable as its own act, and rewrite the review body from the
consolidated spec (a reviewer reading the pre-consolidation body
is reading a stale artefact).

**Right-size it.** A change with no iteration to consolidate skips
this — there is nothing to strip. The test is whether the spec
carries alternatives, Q&A, or revision narration; if it doesn't,
move on.

## Backlog item template

When the user describes wanted-but-not-now work, write to
`$SPEC_ROOT/backlog/<item-name>.md` using this template. One
file per item; never consolidate into a single `FIXES.md` or
`TODO.md`. Closure-time cleanup of resolved items lives in
`phases/close.md` §8 step 3.

```markdown
# Backlog: <item-name>

## What

<!-- One paragraph; what, not how. -->

## Why

<!-- Motivation; link the conversation/incident. -->

## Open questions

<!-- Blockers, dependencies, unknowns. -->
```

Promoting backlog → feature: `git mv` into
`$SPEC_ROOT/feature/`, then fill Requirements / Design / Test
Strategy / Implementation Plan around the existing What/Why
using the feature file template above.

## Initiative file template

When the user runs `start initiative <name>` (`tiers/initiative.md`
§10), write `$SPEC_ROOT/initiative/<name>.md` from this template. The
user owns the first four sections; the last two belong to whoever
runs the streams (the user when guided, else the ringmaster).

```markdown
# Initiative: <name>

- Branch: `initiative/<name>` (cut from `main` at <sha>)

## Goal

<!-- One paragraph: what changes for the user, and why now. -->

## Experience

<!-- What the user can do and observe when this is done, in their
     terms. Each bullet is something the Macro test checks. -->

## Macro test

<!-- The end-to-end check, run on the initiative branch before it
     goes to main: the commands or steps, and what passing looks
     like. -->

## Constraints

<!-- What binds every stream: compatibility, things not to touch,
     and what the ringmaster may spend (e.g. "paid skill tests: yes,
     up to N runs"; or "none - plain code"). -->

## Streams

<!-- Owned by whoever runs the streams. One row per feature. -->

| # | Feature | Ships | Needs | Status |
|---|---|---|---|---|

## Decision Log

<!-- Calls made for the initiative, newest last:
     - Ruling: <what> · why: <why> · cost if wrong: <cost> -->
```

## Initiative interview shape

Four short interviews, in order. Draft each answer from what the user
already said and the code, then ask only what you cannot infer:

1. **Goal.** What changes for the user, and why now.
2. **Experience.** What they will do and see. Apply the feature
   Requirements smell test (`phases/plan.md`): no internals.
3. **Macro test.** How to prove the Experience end to end, and where
   it runs.
4. **Constraints.** What every stream must respect, and the spend
   allowed (paid tests and their budget, or none).

Do not plan the streams here: the user does when guiding, and the
ringmaster does when it runs.
Then commit and stop for approval, per `tiers/initiative.md`.

## Feature spec for an initiative stream

A feature session for an initiative stream (guided, or started by a
ringmaster) writes its spec from the feature template above, with:

- `## Git Setup > Base:` `initiative/<name>`, and the stream's
  worktree.
- `Part of initiative: [[<name>]]` (`phases/plan.md` §6 auto-link).
- The four interviews scoped to its Streams row; the initiative's
  Experience and Constraints bind it.

Then return to `phases/plan.md` §6's Plan-commit rule.
