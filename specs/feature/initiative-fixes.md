# Feature: initiative-fixes

## Git Setup

- Branch: `feat/initiative-fixes/work`
- Base: `initiative/my-harness`
- Worktree: `mAId-worktrees/initiative-fixes`

## Feature Brief

The defects the initiative's review briefing found are fixed: a run with
no claude or codex CLI keeps what mAId brought in as mAId's, the
contract's and README's memory sentences say what the code does, the
fixture runner loses the code only the removed memory skill used, and the
missing tests exist. The defect list is the plan.

Part of initiative: [[my-harness]]

## Handoff

- **Stage:** closed
- **Ready for:** squash-merge into `initiative/my-harness` by the
  ringmaster
- **Carry forward:** none. The window after one part uninstalls (Claude's
  own memory on until another part reinstalls) is the initiative's ruling
  and goes to the user at the initiative briefing.
- **Deliberately left:** a direct test of the unreadable-listing branch of
  `BringIns::placed` (its reason is unchanged and the shared closure is
  now covered end to end by the no-CLI run); a seeded-config mechanism for
  paid fixtures (ruled: nothing to do until a fixture needs one). Neither
  is filed.

### Crossings

<!-- Append one line per crossing; never edit or delete a line. -->

- planning → dev · EXCEPTION · skipping: planning review · why: scope is
  a ruled defect list
- dev → review
- review → closure
- closure → closed

## Requirements

1. **Bring-ins with no CLI.** When claude or codex is not on PATH, install
   and uninstall keep the vendor marketplace and plugins mAId placed
   earlier recorded as mAId's, so a later run with the CLI back still
   removes them. What mAId never placed is recorded skipped, naming the
   missing CLI, as today.
2. **Memory sentence (project.md "Memory").** It says what happens: the
   part that set the memory-off value removes it when it uninstalls (or
   installs with no server of its own); another part with a server takes
   it on at its next install; until then Claude's own memory is on.
3. **README no-store sentence.** It says that install with no store set
   registers nothing and takes back the memory-off value an earlier
   install set.
4. **Dead fixture code.** The `config_home:` fixture field, the per-fixture
   Claude memory redirect and their docs go, since no fixture uses them.
5. **Tests that were missing.** An unreadable, non-JSON or format-2 own
   part list stops `install` and `uninstall` through the binary with no
   change to HOME or the parts dir and no CLI run; a conformance case for
   rule 9's "a parent that is not a dir is unreadable".
6. **Spec text.** `memory.md` says at the top that `memory-mcp` superseded
   it; `memory-mcp.md`'s Test Strategy drops "and its default"; the two
   part.rs scope tests describe scopes, not the removed kiro sub-agent.

## Test Strategy

- R1: a `tests/no_cli.rs` case: a part list naming the bring-in placed on
  claude, no CLI on PATH; `placed()` keeps it placed and records codex's
  skipped. Fails on today's code.
- R5: a new `tests/stops_on_list.rs` driving the binary with a temp HOME,
  profile, parts dir and a logging stub claude/codex: for each bad list,
  `install` and `uninstall` exit 1, the HOME and parts trees are
  byte-identical, and the stub logged nothing. New conformance case
  `config-parent-not-dir`.
- R4: the removed code's own unit tests go with it; `just ci` stays green.
- R2, R3, R6: text; read against the code (`memory.rs` `install_off`,
  `uninstall_off`).
- Gate: `just ci` and `resources/tests/rollback` green on the final
  commit. No paid tests.

## Design

- R1: in `BringIns::placed`, the no-CLI branch uses the same per-item
  ownership the unreadable-listing branch uses: `unread_item` generalised
  to take the reason (`unlisted_item(kind, name, ours, why)`), so both
  branches share one closure, and so do agents with no plugin CLI (none of
  their bring-ins is ever owned, since ownership is loaded for plugin
  agents only).
- R4: remove `Fixture::config_home`, `Plan::config_home`, `CLAUDE_AUTO`,
  `own_claude_memory`, `config_env`, the `Env` alias and the `env`
  parameter of `shell` and `drive` (all added with the memory skill), with
  their tests and the `harness.rs` format comment.
- Also fix the `install_off` doc comment that makes the same false
  "one always does" claim as R2.

## Implementation Plan

- [x] R1 bring-in ownership with no CLI, plus its failing-first test
- [x] R4 dead fixture code and its docs
- [x] R5 stops-on-list binary test and the rule 9 conformance case
- [x] R2, R3, R6 text

## Session Log

- 2026-10-12 · Verified each defect on `initiative/my-harness`: R1's
  no-CLI branch writes `skip_all` regardless of `ours`; `config_home` is
  set by no `.smoke` fixture; `install_off` with no server removes mAId's
  value, so the README sentence is false after an earlier install.
- 2026-10-12 · All four slices in. `just ci` and `resources/tests/rollback`
  green. The R1 test failed on the old code; the stops-on-list test fails
  with the stop removed (checked by swapping `BringIns::new` for
  `reporting` in `main.rs`); the new conformance case fails with its
  expectation flipped. Code review (correctness lens): PASS WITH NOTES;
  applied: no-CLI case in the memory sentences, "a part that follows these
  rules", a dropped comment, the profile dir in the snapshot.
- 2026-10-12 · Review ruling: one binary-level no-CLI run. Added to
  `resources/tests/rollback`: install and uninstall with claude and codex
  off PATH keep the bring-ins placed, and the uninstall after, with the
  CLIs back, removes them. With the old `deploy.rs` those two checks and
  the removal check fail.
- 2026-10-12 · Closure. Backlog answer: none; no item in `specs/backlog/`
  is closed by these fixes. `project.md` sections touched (Architecture
  "Bring-ins" and "Memory", Layout, Testing, the fixture format) were
  edited in dev; nothing further at close.

## Decision Log

- R1 reuses the unreadable branch's ownership rule rather than skipping
  the record step with no CLI · keeps one rule for "listing not seen".
- Amended in dev: agents with no plugin CLI also go through that closure
  instead of keeping `skip_all` · one closure, same output · was the
  planned Design line.
