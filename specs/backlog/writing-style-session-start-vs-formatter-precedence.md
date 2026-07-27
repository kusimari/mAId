# Backlog: writing-style-session-start-vs-formatter-precedence

## What

Give the `writing-style` skill an explicit precedence rule for when
its session-start behavior collides with a body-behavior's output
contract. Concretely: at session start, if `## Learned rules` has
pending entries, the skill opens with a promotion offer *before
answering* (SKILL.md session-start check). Separately, the Formatter
section requires the response to *lead with the rewritten passage, no
preamble*. When a user's first message of a session is "format this
in my style," these two contracts collide and the skill specifies no
winner — a compliant agent leads with the promotion offer, violating
the formatter's no-preamble rule.

Add a precedence line to SKILL.md that resolves the collision — e.g.
"a body task that mandates a specific opening (Formatter, Strict
mode) suppresses the session-start promotion offer; surface the
pending-rules reminder *after* the task output instead."

## Why

Surfaced by the `writing-style-behavioral-verification` feature: the
tri-tool functional run caught claude honoring the session-start
offer and failing the formatter fixture, while kiro/codex didn't fire
the offer. That feature fixed it *at the fixture level* (the formatter
prompt now tells the agent to skip the promotion offer for the scoped
task), which is correct for isolating the behavior under test — but it
does not fix the underlying skill ambiguity. A live user whose first
message is a format request hits the same collision, unmitigated,
with no fixture prompt to steer them. This is a skill-contract fix,
deliberately kept out of the verification feature (which by design
left SKILL.md unchanged).

## Open questions

- Which body behaviors claim opening precedence — Formatter and
  Strict mode for sure; does Learning loop's confirmation line also
  count?
- Where should the deferred reminder land — a trailing line after the
  task output, or silently skipped until a non-task turn?
- Does this interact with the strict-mode fixture deferred by the same
  feature? Worth building both skill change + a strict-mode fixture
  together.
