---
name: codex-behavioral-fixture-strictness
description: Two behavioral fixtures fail on codex only, on both the pre- and post-inline runner, because codex stops to ask permission mid-task and reports findings in its reply rather than the artefact. Decide per fixture whether to loosen the assert or accept codex as out of scope for them.
metadata:
  type: backlog
---

# Two behavioral fixtures fail on codex, and always have

## What

A full pre-install `check` sweep (55 tests: 50 pass, 2 fail, 3 expected
skips) leaves exactly two failures, both codex-only:

- `kdevkit-closure enact via codex`
- `kreviewkit enact via codex`

Both reproduce identically on the runner *before* the inline-skill change
(commit `03dcd2c`, verified in a scratch worktree), so neither is caused
by how the skill reaches the agent. claude and kiro pass both.

## Why each fails

**`kdevkit-closure`** — codex stops and asks. Its reply: *"Awaiting
confirmation to complete the required local closure commit."* On the path
form it got further, ticking the plan boxes first, then still stopped:
*"which backlog items did this feature close out? Pick any, or `none`."*
The skill's closure step legitimately asks that question; codex treats it
as a blocking prompt where the other two agents proceed. The assert wants
a `close(` commit, which never happens.

**`kreviewkit`** — codex does the work correctly but puts part of it in
the wrong place. It wrote `BRIEFING.md`, emitted the announce marker, and
*correctly identified* the seeded defect (`title_to_slug` reimplementing
`slugify`). But it reported that in its **reply** as a loop-back defect
rather than in the briefing file, and the assert greps `BRIEFING.md` for
`slugify`. Arguably the agent was right and the fixture is too strict —
the kreviewkit contract does say defects route back to the caller rather
than into the published briefing.

## Why this is not simply a bug to fix

`project.md` is explicit: *"when a test fails, check the answer against
`SKILL.md` before concluding the agent misbehaved — a fixture that
contradicts the skill it tests fails honest work."* The `kreviewkit` case
looks exactly like that. And the `kdevkit-closure` case is a real
behavioural difference between agents, not a defect in either.

## Open questions

- **`kreviewkit`:** loosen the assert to accept the defect being named in
  either the briefing or the reply? That matches the skill's own
  two-channel contract. Or is naming it in the artefact genuinely
  required, in which case the skill's prose should say so?
- **`kdevkit-closure`:** is codex's confirmation-seeking something the
  skill should override (an explicit "do not stop to confirm" in the
  closure step), or is it agent behaviour we accept and scope this
  fixture to `claude,kiro`?
- Either way the `tools:` field is the lever, and using it should be a
  deliberate recorded choice rather than a quiet narrowing.
- Worth sampling 3-5 runs per `project.md`'s rule before deciding; the
  above is one sample each on two runner versions.
