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

<!-- kdevkit:standing-rules -->

## kdevkit Standing Rules

### Git Practices

**Branches:** `<type>/<short-description>` — types: `feat` · `fix` · `chore` · `docs` · `refactor` ·
`test`

**Commits:** Conventional Commits — `type(scope): subject`; imperative mood, lowercase, no trailing
period; subject ≤ 72 chars; body explains _why_ not _what_; every commit must leave the repo in a
working state.

**Scope:** Changes stay local to this project — never modify global git config or write outside the
project root.

**Pull Requests:** Title follows `type(scope): subject`; body explains _why_ and what approach was
chosen; keep PRs small (one concern per PR); squash merge.

**Hygiene:** No commented-out code, debug prints, temporary test files, secrets, or credentials in
commits.

### Session Behaviour

**Feature file:** Update `.kdevkit/feature/<name>.md` after each meaningful unit of work — do not
batch updates.

**Phase gating:** Never chain phases automatically — stop and wait for explicit instruction between
phases.

**Assumptions:** If a phase input is ambiguous, present a brief plan and wait for approval before
proceeding.

**YOLO mode:** "yolo" drops phase gates and assumption plans; "yolo off" restores normal behaviour.

**Feature completion:** When the feature is done, offer to update `.kdevkit/project.md` with what
changed.

<!-- /kdevkit:standing-rules -->
