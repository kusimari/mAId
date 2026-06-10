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
the dev loop (SKILL.md §7) then has a verifiable target, not
a sketch to validate after the fact.

1. **Requirements (the experience layer).** How does the
   user experience the capability — what do they touch, what
   do they observe? For a CLI, flags and output. For an app,
   screens and visible state. For a skill change, the cues
   the agent recognises and the artefacts it produces. For a
   service, request shape and response. Library names,
   internal file paths, function/schema names, and protocol
   verbs go in Design — not here. (See SKILL.md §6's
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
   was considered and rejected. Then the technical approach:
   components, interactions, trade-offs. A reader shouldn't
   reach the end of Design before learning why it's shaped
   the way it is.
4. **Implementation plan.** Ordered tasks + risk notes.

The interviews are scaffolding — the actual spec layout
adapts to what the feature needs. The skill's strictness
lives in the gates (§6 / §7 / §8), not in heading shape.

After the four interviews, write the feature spec from the
template below, then return to SKILL.md §6's Plan-commit rule
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

<!-- Optional, populated by SKILL.md §6 auto-link when this
     feature is a stream of an active initiative:
Part of initiative: [[<name>]]
-->

## Requirements

<!-- The experience layer — what the user touches and
     observes. CLI: flags and output. App: screens and
     visible state. Skill change: the cues the agent
     recognises and the artefacts it produces. Service:
     request shape and response.

     Smell test (SKILL.md §6): library names, internal
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

<ordered task list with risk notes>

## Session Log

<!-- append: date · what was done · decisions made -->

## Decision Log

<!-- append: decision · rationale · alternatives rejected -->
```

## Backlog item template

When the user describes wanted-but-not-now work, write to
`$SPEC_ROOT/backlog/<item-name>.md` using this template. One
file per item; never consolidate into a single `FIXES.md` or
`TODO.md`. Closure-time cleanup of resolved items lives in
SKILL.md §8 step 3.

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

When the user runs the `start initiative <name>` verb (see
SKILL.md §10), write `$SPEC_ROOT/initiative/<name>.md` from
this template:

```markdown
# Initiative: <name>

## Why

<!-- The realization or external trigger. One paragraph. -->

## Streams

<!-- Ordered list. Each stream = one branch / one CR.
     Format: 1. **<name>** (`<branch>`) — <one-line intent>.
              Prereq: <previous stream id, or "none"> -->

## Decisions taken at the initiative level

<!-- Anything that binds *all* streams. Per-stream decisions
     belong in that stream's feature spec. -->

## Status

| Stream | Branch | CR | Status | Shipped | Learnings |
|---|---|---|---|---|---|
| 1 | ... | ... | planning | — | — |
```

## Initiative interview shape

When writing a fresh initiative, walk three short interviews
(parallel to the feature four-interview shape, but cut for the
initiative tier):

1. **Why.** What's the realization or external trigger? One
   paragraph captures the motivation; future sessions read
   this as the persistent root.
2. **Streams.** Order them. Each stream = one branch / one
   CR. Capture each as `<name> (<branch>) — <one-line
   intent>`, plus a prereq pointer (`Prereq: <previous
   stream id, or "none">`). Sequential ordering is the
   contract; if streams aren't sequential, this isn't an
   initiative — it's a backlog of independent features.
3. **Initiative-level decisions.** Anything that binds *all*
   streams: shared interfaces, data shapes, naming
   conventions, rollout strategy. Per-stream decisions
   belong in each stream's feature spec, not here.

After the three interviews, write the file from the template
above, append a one-line entry to `project.md`'s
`## Active initiatives` index, and commit as
`plan(<initiative>): initial spec`. Open the Planning Review
Gate per SKILL.md §6 / §9; the gate's phase-specific body
content is **Why** + **Streams** + **Decisions taken at the
initiative level**.

## "stream `<n>` for `<initiative>`" template-fill steps

When the user runs the `stream <n> for <initiative>` verb
(see SKILL.md §10), write the new feature spec using the
feature file template above with these populated:

- `## Git Setup > Branch:` — the stream's named branch from
  the parent initiative's Streams list.
- `## Git Setup > Base:` — `main` (current commit-ish) unless
  the parent initiative declares a different base.
- The optional `Part of initiative: [[<initiative>]]` line —
  populated automatically (per SKILL.md §6 auto-link rule).
- The four-interview content (Requirements / Test Strategy /
  Design / Implementation Plan) — fill via the four interviews
  above, scoped to this stream's intent (the parent
  initiative's stream entry is the seed).

After the spec is written, return to SKILL.md §6's
Plan-commit rule.
