# Backlog: kiro-loads-my-harness-frontmatter

## What

Verify that kiro-cli still loads, lists and activates a skill whose
frontmatter carries `metadata` with `my-harness.*` keys (all six of
mAId's skills do since `turn-bring-ins`). Use a free or isolated-HOME
check when one is possible: a kiro command that renders its skill list
without a model call, or a smoke-stage run that never repoints the real
HOME's links.

Also unproven: writing-style activating on codex and following its
two-file promote flow there. Codex is only shown to list the skills
(`codex debug prompt-input`, no model call).

## Why

The Agent Skills spec allows a string map in `metadata`, and codex's
reader proved lenient, but nothing has shown kiro's loader accepts it. A
check-stage run carries the skill's text inline, so it never exercises
the loader; the only paid route today (`verify-skills-isolated smoke`)
repoints the real HOME's skill links, which the harness-core initiative
ruled out. kiro is mapped, not built, in that initiative.

## Open questions

- Does kiro-cli have, or gain, a way to show its loaded skills without
  a login or a model call?
- Can `verify-skills-isolated` run with a temp HOME that carries a kiro
  login, so it never touches the real one?
