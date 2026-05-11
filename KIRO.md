# Always-on routing (Kiro)

On a new session:

1. Read `~/.kiro/steering/` — each subdirectory/file is a steering
   document. When the user's first message matches, load it and
   apply.
2. Play back what you loaded: *"Using steering: A, B."*
3. If intent is ambiguous, ask one disambiguation question.
4. Only then proceed with the task.

# Authoring rules

- New skills are authored at
  `~/env-workplace/mAId/sources/skills/<name>/SKILL.md` (public)
  or in a private sibling repo that installs into the same tree.
- **Never write files into `~/.kiro/steering/` directly** — that
  path is a symlink back into the mAId checkout.

# Self-update protocol

- Markdown edits are live immediately.
- Structural changes require a CR on the mAId repo.
