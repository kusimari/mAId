# Feature: ghost-in-shell-name

## Git Setup

- Branch: `feat/ghost-in-shell-name/work`
- Base: `initiative/harness-core`

## Feature Brief

The concept's name is ghost-in-shell (the user's ruling, 2026-11-11); "my
harness" is only what it means to the person. Every `my-harness` name the
ghost reads, writes or shows becomes `ghost-in-shell`, and for one release
the old names still work, with a warning that says what to rename.

Part of initiative: [[harness-core]]

## Handoff

- **Stage:** closed
- **Ready for:** the ringmaster's squash-merge into
  `initiative/harness-core`
- **Carry forward:** none open. The initiative spec's "On Claude Code"
  still names `MY_HARNESS_CANDIDATES`; that edit is the ringmaster's.
- **Deliberately left:** the both-dirs case went to
  `drop-my-harness-compat` as an open question; an empty
  `MY_HARNESS_STATE` in plain's Justfile goes with `drop-old-part-lists`.

### Crossings
<!-- Append one line per crossing. Never edit or delete a line. -->
- planning → dev
- dev → review
- review → closure
- closure → closed

## Requirements

- R1. A bring-in declares under `ghost-in-shell`: a skill's
  `ghost-in-shell.*` metadata keys, a plugin's `extensions["ghost-in-shell"]`,
  a server's `_meta["ghost-in-shell"]`. Everything mAId ships does.
- R2. The prefix's lines start `[ghost-in-shell]`; the ghost's MCP server
  is `ghost-in-shell`, its resource `ghost-in-shell://genome`.
- R3. The ghost reads `GHOST_IN_SHELL_STATE` and
  `GHOST_IN_SHELL_CANDIDATES`; its state defaults to
  `${XDG_STATE_HOME:-$HOME/.local/state}/ghost-in-shell`. Plain reads the
  old part lists' folder as `GHOST_IN_SHELL_PARTS`, still defaulting to
  `my-harness/parts`, where older writers put them.
- R4. For one release, the old names work: an old variable is read when
  the new one is unset, the old state dir is used where it is when the
  new one is not there, and an old declaration is read. Each is named in
  `ghost status` and under the genome's environment line, with what to
  rename it to.
- R5. `ghost-claude validate` refuses a declaration under the old name, so
  nothing ships with it.

## Test Strategy

- Unit: the reader takes each slot under either name, the new one first,
  and marks the old; config reads each old variable and the old state
  dir, saying so; the genome renders the old-name lines; validate
  refuses the old name.
- Conformance: a `ghost status` case with old-name declarations shows
  them read, with their warnings.
- `ghost-in-shell/tests/rollback`: the no-nix ghost install over an old
  state dir (with a pending decision) and an old-name skill shows the
  pending decision, the skill, and both warnings, in `ghost status` and
  the genome.
- `just ci`, `claude_live`, `claude_installed` as they are, with the
  new names.

## Design

- `ghost::NAME` is the concept's name; `contract::NAMESPACE` is it.
- `ghost::old_name` is the whole compatibility path: the old names, the
  variable that is read (new, else old), the state dir used in place, and
  each warning's text. Dropping it is one module and its callers.
- `Declaration::under_old_name` (not serialized) marks a declaration read
  under the old namespace. `Ghost::old_names` says it per bring-in;
  `Config::old_names` says it per variable and for the state dir. The
  genome's picture and `ghost status` show both.
- Plain's Justfile keeps the old part lists' folder where older writers
  put it, `my-harness/parts` (`GHOST_IN_SHELL_PARTS`, else
  `MY_HARNESS_PARTS`), and no longer exports the state dir, so ghost-claude
  resolves it itself whether started by `just` or a hook.
- In a skill, the new keys win unless only the old ones name a contract,
  so an old declaration with one new key added still reads.

## Implementation Plan

- [x] 1. Rename in code, manifests, fixtures and tests; add the
  compatibility path and its tests; `just ci`.
- [x] 2. Docs: project.md, README, the backlog items that describe
  current behaviour; file `drop-my-harness-compat`.
- [x] 3. `claude_live`, `claude_installed`, `tests/rollback` with the
  old-name install check.

## Session Log

- Code review panel (round 1): correctness, security, comment-hygiene all
  PASS WITH NOTES, no high. Fixed: the old part lists' folder had moved to
  `ghost-in-shell/parts`, so lists older writers keep putting in
  `my-harness/parts` went unread, and the rename rewrote that history in
  project.md and the backlog (medium, two lenses); the state-dir warning
  now says to move it once no older ghost is rolled back to; a skill with
  old keys and one new key read as undeclared under the new name; doc
  comments. Left: the old MCP tool names have no compatibility path (a
  permission for them asks again; noted in project.md); old-name lines
  carry names unsanitised, as every other genome line does.
- Code review panel (round 2, the last the budget allows): correctness
  and comment-hygiene PASS WITH NOTES, security PASS, no high. Fixed: a
  `my-harness` dir holding only plain's old `parts/` was taken as the
  ghost's state, and moving it as the warning said would take `parts/`
  along; now only a dir holding more counts, and the warning says to leave
  `parts/`. The rollback test's removal of that dir is gone, so its HOME
  covers the leftover-`parts/` case. Comments and backlog wording. Left:
  nothing is said when both state dirs exist; an empty `MY_HARNESS_STATE`
  in plain's Justfile gives `/parts`, which reads nothing.
- Briefing (round 1): two defects, both untested old-name branches in
  `parts.rs` (an unreadable old declaration still reported; a disabled
  plugin named by its old slot's repository). Tests added, each seen
  failing without its branch.
- Briefing (round 2): no defects. Its questions, for the ringmaster to
  ratify: no compatibility path for the old MCP tool names; plain's
  `MY_HARNESS_PARTS` read without a warning (it belongs to the part lists,
  not R4); nothing said when both state dirs exist; a half-migrated skill
  drops its new keys under the generic warning.
- Closure, backlog: this feature closes no backlog item. Filed
  `drop-my-harness-compat`; renamed `kiro-loads-my-harness-frontmatter`
  to `kiro-loads-ghost-in-shell-frontmatter`. project.md: Architecture
  ("The old name, for one release"; the old part lists' wording) and
  Layout (`old_name.rs`) changed in dev; Testing and Deployment did not.

## Decision Log

- Decision: the old part lists' folder stays `my-harness/parts` · why: it
  holds an old format that older writers keep writing there; renaming it
  would read nothing · cost: none beyond `drop-old-part-lists`.
- Decision: the old state dir is used in place when only it is there,
  not moved · why: no write from a hook, no race between two hooks, and a
  rollback to a generation from before the rename sees the same state;
  nothing is lost if the person never acts · cost: the old dir stays
  until the person moves it (the warning gives both paths) · alternative
  rejected:
  move on first use, which leaves a rolled-back ghost with an empty state.
