---
name: kdevkit-triggers-must-be-imperatives-not-questions
description: A trigger condition phrased as a question ("No Handoff, or a spec still carrying...?") reads as advisory to a real agent, even when surrounding prose calls it mandatory. Sweep kdevkit's phase modules for this pattern and rewrite each one as an unconditional imperative with an explicit stop condition, following the shape that actually worked for the consolidation-check fix.
metadata:
  type: backlog
---

# kdevkit — audit trigger conditions for question-phrasing that reads as advisory

## What

The fix for `kdevkit-a-b-fixes`'s root cause 1 (consolidation being
skipped) took three iterations. The first two wordings were
correct in *content* but phrased the trigger as a question:

> "No `## Handoff` block, or a spec still carrying lettered options
> / Q&A / 'my recommendation' phrasing?"

A real agent (claude) read this as advisory and never acted on it,
even with "Finding any of these is a stop condition" stated right
below it. Only rewriting it as an unconditional scan-and-act
imperative — "Scan the Design section... Finding any of these is
a stop condition... this dev-loop entry must run it now" — actually
changed behaviour.

Sweep the four phase modules (`plan.md`, `dev.md`, `review.md`,
`close.md`) and `SKILL.md` §5/§9 for the same shape: any rule that
states a condition as a question, or as a soft "if X, then maybe
Y" construction, where the surrounding prose intends it as
mandatory. Rewrite each as: state the check, state that finding it
is a stop condition, state what must happen before anything else.

## Why

- **Observed live, twice, in the same fix.** Not a one-off model
  quirk — the closure backlog-filing fix independently needed the
  same correction in the opposite direction (an over-hedged
  imperative that read as *too* soft, causing the agent to stop
  and ask three clarifying questions instead of acting).
- **The failure is silent.** An agent that doesn't act on a
  question-phrased trigger doesn't error — it just proceeds as if
  the trigger never fired, producing plausible-looking wrong work.
  That's the same failure class the initiative's own
  `kdevkit-adversarial-assert-discipline` backlog item names for
  test asserts; this is its prose-authoring counterpart.
- **Cheap to check, not cheap to guess at.** A trigger's phrasing
  can be read mechanically (does the sentence end in `?`, does it
  hedge with "may" / "if... then maybe") without needing to predict
  which one will actually misfire on a given agent — unlike the
  test-fixture version of this problem, which needs adversarial
  probing to catch.

## Sketch

- A single audit pass: grep the four phase modules plus `SKILL.md`
  for `?` at the end of a bolded or otherwise rule-shaped sentence,
  and for soft conditionals ("if... consider", "may want to").
  Each hit gets read in context and either left (a genuinely
  optional judgement call) or rewritten to the imperative shape
  that worked here.
- The shape that worked, as a template: **state the check** (what
  to scan/look for) → **state the stop condition explicitly**
  ("finding X is a stop condition") → **state what happens before
  anything else** ("this must run now, before step 1"). Three
  clauses, no question mark, no hedge.
- Watch the other failure direction too (closure's regression): an
  imperative can overcorrect into inviting a clarifying question if
  it's phrased as advice rather than instruction ("create it and
  write the item... an absent directory is not evidence" still let
  an agent stop and ask). The fix there was "File it. Don't ask
  whether to" — a directive plus an explicit boundary on what *is*
  worth asking. Any rewrite needs both halves: the imperative, and
  a stated carve-out for genuine judgement calls, so the fix
  doesn't swing into false confidence on things that really do need
  the user.

## Open questions

- **Does this need adversarial fixture coverage per rule, or is a
  read-through audit enough?** The consolidation bug was caught by
  a paid tri-tool run, not by reading the prose — reading alone
  didn't catch that "reads as advisory" the first two times. An
  audit might miss a rule whose phrasing looks fine on paper but
  still doesn't land with a real agent. Likely: audit first for
  cheap wins, but treat any always-on gate-adjacent rule as needing
  the same probe-with-a-real-agent step this fix used.
- **Scope**: just the four phase modules and §5/§9, or the deferred
  files (`setup.md`, `interviews.md`) too? Those fire less often
  (genesis-time) so the cost of a miss is lower, but the same
  pattern could exist there.

## Trigger to promote

- Natural fit for whichever stream next touches phase-module prose
  broadly — likely alongside the prose-compression pass already
  tracked in `kdevkit-refactor-shrink-always-on-context.md`, since
  both require reading every rule closely anyway.
