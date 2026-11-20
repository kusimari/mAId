# Feature: claude-plugin-reads

## Git Setup

- Branch: `feat/claude-plugin-reads/work`
- Base: `initiative/my-harness`
- Worktree: `mAId-worktrees/claude-plugin-reads`

## Feature Brief

A Claude Code session reads every file of mAId's skills (kdevkit's
`phases/*.md`, which its `SKILL.md` tells the agent to Read) with no
permission prompt, in any permission mode. Install grants it, as one
element of a Claude setting that mAId owns and uninstall removes.

Part of initiative: [[my-harness]]

## Handoff

- **Stage:** closed
- **Ready for:** squash-merge into `initiative/my-harness` by the
  ringmaster
- **Carry forward:** none open. The grant, its ownership and adoption,
  and reader rule 14 are in project.md ("Skill reads (claude)", the part
  list's `setting` and `adopted`, the reader rules). `just verify-install`
  (real HOME) is the user's, with the initiative's Macro test.
- **Deliberately left:** codex, kiro and agy are the initiative's Next
  round, not a backlog item. Uninstall drops `permissions` or the list
  when mAId's element was its last entry, rather than restoring an empty
  container (accepted, ringmaster ruling). The user's hand-added Read
  rule stays theirs; it only matters in auto mode, where reads pass anyway.

### Crossings

<!-- Append one line per crossing; never edit or delete a line. -->

- research → planning
- planning → dev
- dev → review
- review → closure
- closure → closed

## Requirements

- After `just install`, a Claude Code session in default (manual) mode
  reads any file under mAId's plugin (every skill's `SKILL.md` and the
  modules beside it) without asking. A file of the marketplace outside
  mAId's plugin still asks.
- Install adds the grant only when it is not there. If the identical
  grant is already there (put by hand), install adopts it: it says
  `already present, adopted` and from then on treats it as its own.
- `just uninstall` removes the grant when mAId placed or adopted it,
  and nothing else in Claude's settings. Any other entry beside it (the
  user's own, a different path, a broader rule) is kept on install and
  uninstall.
- With claude not on PATH, install writes nothing and says so.
- `just status --whole` lists the grant for claude as present, or
  missing once someone removes it by hand.

## Test Strategy

- Unit (`just test`), temp HOME, no claude: install adds the element to
  an empty, absent or populated list and keeps every other key and
  element; a rerun leaves the bytes alone; an identical element already
  there is adopted (output and part list say so) and uninstall removes it;
  a different element is never touched; uninstall removes only the
  recorded element and drops the list (then `permissions`) when that
  leaves it empty; a recorded element from an older profile path is
  removed when the path changes; a list that is not a list is left alone
  and reported; no claude on PATH writes nothing; dry run writes nothing.
- Memory's setting entry survives this step's record and the other way
  round (both are kind `setting` on claude).
- whole-status: a dotted setting name reads a nested key; a list key is
  present when it holds the entry's `value`, missing when not.
- Integration (`tests/memory_install.rs` style: the install and uninstall
  steps in-process, beside memory's, under a temp HOME with a stub claude
  on PATH): install then uninstall leaves settings.json as it was, the
  part list holds both settings, whole-status reads present then missing.
  `main.rs`'s wiring is exercised by `resources/tests/rollback` and `just
  verify-install`, which run the real install.
- Evidence already run for the design (temp HOME, default mode, real
  claude 2.1.296, 4 paid runs): see Session Log.

## Design

- **The grant**: `permissions.additionalDirectories` in
  `~/.claude/settings.json`, element `<marketplace root>/plugins/maid`
  (the profile-side marketplace path, e.g.
  `~/.local/state/maid/marketplace/plugins/maid`; never a store path or
  a checkout). Claude reads files in an additional directory without
  asking, following the symlinks into the profile. The store is
  read-only, so the edit half of a working directory grants nothing.
- **Writer**: a `reads.rs` step beside memory's, reusing memory's
  settings read/write (atomic, key order kept, linked file written
  through). Install and uninstall run it after the memory step, claude
  only.
- **Ownership** (rule 2, for one element of a list): mAId owns the
  element its part list records (`value`). Install: recorded and present
  → done; present but unrecorded and identical → adopted; absent → added.
  A recorded element that differs from today's path is removed (ours, now
  stale). Uninstall removes the recorded element only.
- **Part-list entry**: `{"kind": "setting", "name":
  "permissions.additionalDirectories", "value": "<path>"}`, plus
  `"adopted": true` when adopted. `part::record` replaces entries by kind
  *and name* for a setting, so memory's and this entry do not erase each
  other.
- **project.md** says the grant widens Claude's working directories only
  to a read-only tree.
- **Contract text** (project.md "My harness"): a setting's `name` is its
  key, dotted for a nested one; for a list setting the entry is one
  element, `value` is that element, and the reader checks the list holds
  it. `adopted`, optional: the part took over an identical element it
  found, and owns it as its own.

## Implementation Plan

- [x] `part.rs`: optional `adopted`; record replaces settings by name.
- [x] `reads.rs` + stages/main wiring for install and uninstall, with
  unit tests.
- [x] `whole.rs`: dotted names and list elements, with tests.
- [x] After M8 merges: rebase; the two reading rules as numbered rules
  in M8's reader-rule list; one conformance case for a list setting
  (present, missing, adopted); mAId's reader passes the set.
- [x] Integration test of the install steps under a temp HOME.
- [x] project.md: Architecture ("My harness" contract, a short "Skill
  reads" paragraph), Deployment, Layout, Hard constraints.

## Session Log

- 2026-10-11 · Cause, read from claude 2.1.296's bundle: a plugin whose
  source is a relative string in a `file`/`directory` marketplace is
  loaded live from the marketplace path; any other is copied to
  `~/.claude/plugins/cache`. Why another directory marketplace on this
  machine reports cache paths: not checked; it does not matter, since a
  cache read asks too (below).
- 2026-10-11 · Claude auto-allows reads of `~/.claude/plugins/**` only
  under `blockReadsOutsideWorkingDirectories`. Plugin `settings.json`
  takes only `agent` and `subagentStatusLine`, so a plugin cannot grant
  itself reads. A skill's `allowed-tools` (docs: skills) grants only for
  the turn that invokes it, so a module read in a later turn still asks.
- 2026-10-11 · Paid runs, temp HOME, `--permission-mode default`:
  (1) auto mode: every Read passes (Read is on the classifier's safe
  list); (2) a Read rule on the link path only: asks; link path plus a
  store glob: passes; `additionalDirectories` on the link path: passes;
  no rule: asks; a file in Claude's plugin cache: asks. (3) the real
  install: `additionalDirectories` = `.../marketplace/plugins/maid`
  reads kdevkit's `phases/plan.md` and kyodakit's `SKILL.md`; the
  marketplace's own `marketplace.json` still asks.

- 2026-10-11 · Code review panel (correctness, security, comment-hygiene):
  all PASS WITH NOTES, nothing high. Fixed: a dotted name broke kiro's
  own dotted keys (a literal key now wins); `"permissions": null` lost the
  element; a top-level non-object read as missing; `build-tool record
  --kind setting` would erase settings entries (now refused); a recorded
  value is mAId's only in `plugin_dir`'s shape; no plugin in the profile
  writes nothing. Kept: uninstall removes every copy of mAId's exact
  element (an identical copy is mAId's under the adoption ruling).

- 2026-10-11 · Rebased onto M8 (reader rules, conformance set): the
  setting reading is rule 14, rules 3 and 9 point at it, and the
  `list-setting` case pins it (one part per list entry, so each state is
  keyed; adopted, non-bool `adopted`, a number `value`, a literal dotted
  key beside a nested one). Second review cycle: all PASS WITH NOTES;
  test gaps it named are closed, and a recorded value must be absolute
  with no `..`.

- 2026-10-11 · `resources/tests/rollback` (temp HOME, real claude and
  codex CLIs) passed every check, including two new ones: the grant is in
  the temp settings and the part list after install, and gone after
  uninstall.
- 2026-10-11 · Paid run 5 on the final code, manual mode, temp HOME
  installed by `just resources::install-skills claude` from a profile
  built from this branch: install printed `done claude
  permissions.additionalDirectories + <home>/.local/state/maid/marketplace/plugins/maid`;
  Claude loaded the skills from `.../marketplace/plugins/maid/skills`;
  kdevkit's `phases/plan.md` read with no prompt, and the marketplace's
  own `marketplace.json` was refused.
- 2026-10-11 · Backlog at closure: none closed. No existing item covers
  skill-file read prompts, and nothing found here is wanted later beyond
  the initiative's Next round.

## Decision Log

- Chose `permissions.additionalDirectories` over Read allow rules: one
  element naming the profile path; allow rules need a second rule on the
  store path (`//nix/store/*-maid/...`), which ties settings to the store
  layout and names a store path.
- Considered a skill's `allowed-tools: Read(${CLAUDE_SKILL_DIR}/**)`:
  no settings at all, but it lasts one turn.
- User ruling (2026-10-11): an element identical to the one mAId writes,
  found already there, is adopted (recorded placed, `adopted: true`,
  removed by uninstall); a different element stays the user's. The
  user's hand-added Read rule is not identical to this element, so it
  stays theirs.
- Ruling (ringmaster): `additionalDirectories` on the profile-side path
  accepted; the hand rule stays the user's; M8's rule list and
  conformance set carry this stream's reader rules.
- Ruling (ringmaster, review): uninstall dropping empty containers and
  the wider-than-reads grant (read-only store) accepted.
