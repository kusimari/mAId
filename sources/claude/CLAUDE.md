# Always-on routing (Claude Code)

On a new session:

1. Read `~/.claude/skills/` — each subdirectory is a skill. When the user's first message matches a
   skill's tags or description, load that skill's SKILL.md and apply it.
2. Play back what you loaded: _"Using skills: A, B."_
3. If intent is ambiguous, ask one disambiguation question.
4. Only then proceed with the task.

# Writing to `~/.claude/`

- **Never write files into `~/.claude/skills/` directly.** The directory is managed — edits land in
  the next Claude session automatically via the source that populates it.
- The same rule applies to `~/.claude/CLAUDE.md`, `~/.claude/agents/`, and `~/.claude/commands/`.
- If you need to add or change a skill, edit the corresponding file that the directory already
  exposes (`~/.claude/skills/<name>/SKILL.md`) — the managing tool picks changes up live.
