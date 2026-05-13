# Always-on routing (Kiro)

On a new session:

1. Read `~/.kiro/steering/` — `KIRO.md` at the root plus the `skills/` subtree, where each
   subdirectory is a skill. When the user's first message matches a skill's tags or description,
   load that skill's SKILL.md and apply it.
2. Play back what you loaded: _"Using steering: A, B."_
3. If intent is ambiguous, ask one disambiguation question.
4. Only then proceed with the task.

# Writing to `~/.kiro/`

- **Never write files into `~/.kiro/steering/` directly.** The directory is managed — edits land in
  the next Kiro session automatically via the source that populates it.
- To change a steering doc or skill, edit the file already visible under `~/.kiro/steering/`; the
  managing tool picks changes up live.
