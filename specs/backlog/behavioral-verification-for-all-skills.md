---
name: behavioral-verification-for-all-skills
description: Apply the agents-md-ecosystem-alignment pattern — behavioral (setup/assert) fixtures run across claude/kiro/codex — to every skill, not just kdevkit. notes and writing-style still rely on recitation/substring smokes.
metadata:
  type: backlog
---

# Behavioral, tri-tool verification for all skills

## What

The `agents-md-ecosystem-alignment` feature reshaped how kdevkit is
verified: recitation probes ("describe what you'd do") were replaced
with **behavioral** fixtures that seed a scratch project, drive the
agent, and assert on the artefacts it produced — run across all three
coding agents (claude, kiro, codex) via the harness's three
verification styles (substring / semantic-judge / behavioral) and the
`--tools` selector.

That pattern currently only covers **kdevkit**. The other skills still
use the older shapes:

- `notes` — substring/judge smokes (`notes.smoke`, `notes-add-note`,
  `notes-git-commit`, `notes-vault-selector`) that mostly check the
  agent *says* the right thing, not that it *writes* the right files
  into a vault.
- `writing-style` — a substring smoke that checks the skill announces
  itself, not that prose actually comes out in the intended style.

Bring every skill up to the behavioral bar where it makes sense: for
each skill, ask "what artefact or observable change proves the agent
carried this out?" and write a `--- setup --- / --- assert ---`
fixture for it, running `tools: claude,kiro,codex`. Keep judge/substring
only where a skill genuinely has no artefact to inspect (as
`kdevkit-dev-loop` does for gate-ordering reasoning).

## Why

This feature proved two things worth generalizing:

1. **Behavioral tests catch what recitation can't.** The
   `kdevkit-closure` fixture initially failed on claude/codex because
   they *correctly refused* an ambiguous seed — a recitation probe
   would have sailed through. Testing the artefact tests the purpose.
2. **Three agents ⇒ robust.** A skill that drives claude, kiro, and
   codex to the same artefacts doesn't lean on one tool's
   prompt-following quirks. The harness already supports this for any
   fixture; only kdevkit's fixtures use it.

Leaving `notes` and `writing-style` on recitation smokes means we don't
actually know they *work* across the three agents — only that the skill
loads. As skills are the sole deployed artefact now (post
skills-only deploy), their verification is the whole safety net.

## Open questions

- **notes needs a seedable vault.** The behavioral fixture must seed an
  Obsidian-shaped vault (and possibly a git remote for the
  `notes-git-commit` flow), then assert the right note file landed with
  the right frontmatter/links. What's the minimal seed? Does the
  git-remote flow need a local bare repo as the "remote"?
- **writing-style is prose, not an artefact.** Its output is *how* text
  reads, which resists a shell `assert`. Is this the one skill that
  legitimately stays judge-mode (semantic), or is there an observable
  proxy (e.g. "rewrite this sentence" → assert the em-dash/spaced-hyphen
  convention appears)?
- **Cost/cadence.** Every skill × 3 agents × behavioral runs multiplies
  the (already user-driven, credit-costing) verify surface. Worth
  scoping which skills are load-bearing enough to warrant the full
  matrix vs. a single-agent behavioral check.
- **Per-skill fixture count.** kdevkit collapsed 17 → 4 phase-keyed
  fixtures. notes/writing-style are smaller; likely 1–2 behavioral
  fixtures each. Confirm the phase/behavior mapping per skill at
  planning time.
