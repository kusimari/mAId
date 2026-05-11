# Always-on routing (Claude Code)

On a new session:

1. Read `~/.claude/skills/` — each subdirectory is a skill. When
   the user's first message matches a skill's tags or description,
   load that skill's SKILL.md and apply it.
2. Play back what you loaded: *"Using skills: A, B."*
3. If intent is ambiguous, ask one disambiguation question.
4. Only then proceed with the task.

# Authoring rules

- New skills are authored at
  `~/env-workplace/mAId/sources/skills/<name>/SKILL.md` (public)
  or in a private sibling repo that installs into the same tree.
- **Never write files into `~/.claude/skills/` directly** — that
  path is a symlink back into the mAId checkout, and creating a
  non-symlink there breaks the deploy invariant.

# Self-update protocol

- Markdown edits to an existing `SKILL.md` in the checkout are live
  immediately (the symlink doesn't need to be re-deployed).
- Adding a new SKILL.md under `sources/skills/<name>/` becomes
  visible via the same symlink on the next session.
- Structural changes (new tool target, registry edits, maid CLI
  code) require a CR on the mAId repo.
