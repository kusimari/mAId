---
name: fixtures-drop-load-from-path-narrative
description: Rewrite every skill test fixture to stop naming the skill and its file path in the prompt. Instead of "Load the X skill from ~/.../X/SKILL.md and do Y", pose a bare task ("we are doing Y") and let the agent discover and load the right skill on its own — a far stronger test of whether the skill actually triggers in the wild.
metadata:
  type: backlog
---

# Fixtures — drop the "load skill from path" narrative, pose the bare task

## What

Rewrite the prompt line of every `.smoke` fixture under
`resources/tests/skills/` so it no longer tells the agent *which*
skill to use or *where* the skill file lives. Today most fixtures
open with some variant of:

> Load the `writing-style` skill from
> `~/.claude/skills/writing-style/SKILL.md` or
> `~/.kiro/steering/skills/writing-style/SKILL.md` — whichever path
> exists, and …

Replace that with a bare, natural task framing — "we are doing X" —
that describes only the *work*, not the tool:

> Format this passage in my house style: "…"

> I want to jot a note that links to two topics: …

> Walk me through closing out this feature.

The agent then has to (a) recognize that a skill applies, (b) find
and load the right one from its own skills directory, and (c) follow
it — with no path spoon-fed. That is the behavior we actually care
about in production: skills must *trigger on their own*, not only
when a prompt hands the agent the file.

## Why

The current phrasing tests the wrong thing. "Load `<skill>` from
`<path>` and do X" verifies the agent can follow an explicit
instruction to open a named file — it does **not** verify that the
skill's own trigger/description causes it to fire when a user just
describes a task. A skill whose `description:` front-matter is too
weak to auto-trigger would still pass every current fixture, because
the fixture does the triggering for it. Removing the scaffolding
makes each fixture a genuine end-to-end test of discovery +
activation + behavior, which is the real user experience.

This raises the bar the same way the `writing-style-behavioral-
verification` feature did within a single skill: there, the tri-tool
matrix caught a defect a single-agent run would have missed; here,
dropping the path narrative catches skills that only work when
spoon-fed.

## Open questions

- **Marker load-check tension.** Several fixtures assert a self-
  announce marker (`expect_substr: [writing-style] applies`). Once the
  prompt no longer names the skill, the agent must still choose to
  announce — good (that's exactly the discovery signal we want) — but
  confirm each skill's `description:`/trigger is strong enough that a
  bare task reliably activates it across claude/kiro/codex. Fixtures
  that go flaky here are surfacing a real skill-triggering weakness to
  fix, not a test to loosen.
- **Ambiguity budget.** A bare task must still be unambiguous enough
  that the *intended* skill is the obvious match — avoid prompts that
  could equally invoke two skills. May need to tune wording per
  fixture.
- **Discovery path across harnesses.** claude, kiro, and codex load
  skills from different roots (`~/.claude/skills`,
  `~/.kiro/steering/skills`, `~/.codex/skills`). Dropping the explicit
  dual-path line relies on each harness auto-surfacing its own skills;
  confirm all three do without the hint.
- **Scope / sequencing.** Do all fixtures in one sweep, or pilot on
  one skill (e.g. `writing-style`, freshly at the tri-tool bar) and
  roll out once the pattern proves out? A pilot de-risks the
  triggering questions above before touching every fixture.
- **Keep an explicit-load fixture?** Consider retaining one
  path-explicit fixture per skill purely as a "the file is loadable
  and well-formed" smoke, separate from the bare-task behavior
  fixtures — or decide that `just test`'s content validator already
  covers that and the bare-task form fully replaces the old prompts.
