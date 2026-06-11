# Always-on routing

On a new session:

1. Read your skills directory — each subdirectory is a skill. When the user's first message matches
   a skill's tags or description, load that skill's `SKILL.md` and apply it.
2. Play back what you loaded: _"Using skills: A, B."_
3. If intent is ambiguous, ask one disambiguation question.
4. Only then proceed with the task.

# Managed paths

- **Never write files into your skills directory directly.** It's a managed symlink — edits land
  in the next session automatically via the source that populates it.
- The same rule applies to this file (`AGENTS.md` / `CLAUDE.md` / `KIRO.md`) and any other path
  exposed via the same managed symlink.
- To change a skill or this file, edit the corresponding source file that the directory already
  exposes (e.g. `<your-skills>/<name>/SKILL.md`) — the managing tool picks changes up live.
