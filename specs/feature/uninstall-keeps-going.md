# Feature: uninstall-keeps-going

## Git Setup

- Branch: `feat/uninstall-keeps-going`
- Base: `initiative/installable`
- Worktree: `mAId-worktrees/uninstall-keeps-going`

## Feature Brief

`just uninstall` removes everything mAId installed even when it must
leave one location alone, and the docs describe the plugin-era verbs.
Both from the initiative briefing's defects.

Part of initiative: [[installable]]

## Handoff

- **Stage:** closed
- **Ready for:** squash-merge into `initiative/installable` by the
  ringmaster
- **Carry forward:** none open. Removing the profile after a failed
  step was accepted at review; the keep-going rule is in `project.md`
  (Architecture).
- **Deliberately left:** an automated test for the keep-going shape,
  accepted at review as by-hand, like `install`'s; not wanted, so no
  backlog item.

### Crossings

<!-- Append one line per crossing; never edit or delete a line. -->

- planning → dev
- dev → review
- review → closure
- closure → closed

## Requirements

- When `just uninstall` meets a location it must leave alone (a
  foreign link, or a real dir or file, at a path mAId manages), it
  still removes the browser MCP registration and, with no agent named,
  the profile. It reports the location and exits non-zero at the end,
  as `just install` does.
- With nothing left alone, `just uninstall` behaves as before: every
  step runs and it exits 0.
- The one-line descriptions of `just uninstall`, `just status`, the
  skills verbs and the browser-MCP group say what they do today: the
  skills verbs also install, remove and report the claude/codex plugin,
  and the browser-MCP verbs do not skip on a missing nix.

## Test Strategy

- **Before/after in a temp HOME** (free, by hand): a real dir at
  `.gemini/config/skills` and a stand-in profile file. Before the fix
  `just uninstall` stops after the skills step with the profile still
  there. After it, the MCP and profile steps run, the profile is gone,
  and the exit code is 1.
- **Macro test** (`just verify-install`, free): step 6's uninstall
  still exits 0 on the real machine and reinstalls.
- `just ci` green (no Rust behaviour changes; one doc comment).

No automated test for the recipe shape: `install`'s identical shape
has none, and the only way to make one is a fixture HOME driving real
agent CLIs, which the temp-HOME check above already is.

## Design

- Root `Justfile` `uninstall` becomes a bash recipe in `install`'s
  shape: `set -u`, `rc=0`, each of the three steps `|| rc=1`, then
  `exit $rc`. The profile step stays conditional on no agent. The
  profile step runs even if an earlier one failed: nothing after it
  depends on it, and the user asked for everything removed.
- Text only: `resources/Justfile` browser-MCP header (drop "no nix"),
  `README.md` uninstall/status lines, `resources/build-tool/src/main.rs`
  module doc for the three skills verbs, `specs/project.md` install-skills
  entry.

## Implementation Plan

- [x] `fix(install)`: root `uninstall` keeps going; before/after check
  in a temp HOME.
- [x] `docs(install)`: the four stale descriptions.
- [x] Gates: `just ci`, code review, `just verify-install`.

## Session Log

- 2026-10-08: Defect 1 reproduced in a temp HOME
  (`HOME=$T XDG_STATE_HOME=$T/.local/state just uninstall`, real dir at
  `$T/.gemini/config/skills`, a file at the profile path): the skills
  step prints a `skip .gemini/config/skills (real dir; not ours ...)`
  line, `just` stops with rc 1, the MCP step never runs and the
  profile file is still there. Cause: `stages.rs` `outcome` returns 1
  for any skip; the recipe is three plain lines.
- 2026-10-08: Defect 2 read: `resources/browser/manage` has no nix
  check (skips only on no Chrome, no CLI, no kiro sub-agent, agy); the
  README, `main.rs` and `project.md` lines name only links and MCP,
  though the skills verbs also handle the claude/codex plugin.

- 2026-10-08: After the fix, same temp HOME: the skills step skips,
  the MCP step and profile removal still run (profile gone), rc 1.
  With the real dir removed: rc 0. `just -n uninstall claude` renders
  no profile step.
- 2026-10-08: Gates. `just ci` green (254 tests). Code review, one
  lens (no named-risk path): correctness PASS WITH NOTES, no Must Fix.
  Took both notes: the recipe comment now says any failing step, not
  only a skipped location, lets the rest run (a plugin CLI error also
  lets the profile go; `just install` then `just uninstall` recovers);
  and the comment sits flush on the doc line as `install`'s does.
  `just verify-install` (free): all checks passed, 17 PASS, 3 paid
  skips; step 6 uninstall exit 0, reinstalled from this worktree.
- 2026-10-08: Closure. Backlog: no item closed by this feature.
  `project.md` Architecture gains the keep-going rule for install and
  uninstall; no other section changed.

## Decision Log

- 2026-10-08: two more lines with the same omission fixed beside the
  four listed: README's `just install` line and the `install`
  subcommand's `--help` text in `main.rs` · why: same stale phrase,
  found by grepping for it.
- 2026-10-08: at review, the ringmaster accepted all three: the profile
  goes even after a failed step; the by-hand check stands, like
  install's; the comment wording stays.
