# Feature: kiro-skills-path

## Git Setup

- Branch: `feat/kiro-skills-path`
- Base: `initiative/installable`
- Worktree: `mAId-worktrees/kiro-skills-path`

## Feature Brief

kiro lists mAId's skills as skills, beside the user's own, instead of
reading their full text as steering context.

Part of initiative: [[installable]]

## Handoff

- **Stage:** closed
- **Ready for:** squash-merge into `initiative/installable` by the
  ringmaster
- **Carry forward:** none open. The ownership rule in kiro's dir is in
  `project.md` (Architecture). verify-install's kiro checks first run
  at the initiative's Finish.
- **Deliberately left:** agy's row (Constraints).

### Crossings

<!-- Append one line per crossing; never edit or delete a line. -->

- planning → dev
- dev → review
- review → closure
- closure → closed

## Requirements

- After `just install`, a new kiro session lists each mAId skill
  (`kdevkit`, `kyodakit`, `notes`, ...) among its skills, next to the
  user's own kiro skills, and can use one when a task calls for it.
- The user's own entries in kiro's skills dir survive install and
  uninstall. One with the same name as a mAId skill is kept and
  reported, never replaced.
- The steering link an earlier mAId install left is removed by
  install, and reported by `just status` until then. Anything else in
  kiro's steering dir is left alone.
- `just status kiro` shows one line per mAId skill in kiro's skills
  dir. `just uninstall kiro` removes exactly those links.
- A skill added or removed in a later install (or a rollback) shows up
  or goes in kiro on that install.

## Test Strategy

### Unit (`just test`, load-bearing)

In a fake `$HOME` against a fake profile with two skills:

- install makes one link per skill under `.kiro/skills`, and a user's
  own skill dir there survives install and uninstall.
- install removes an old `.kiro/steering/skills` link into a mAId
  skills tree, keeps a foreign link at that path, and dry-run removes
  nothing; status lists the old link.
- a user dir named like a mAId skill is kept and reported.
- `skills_roots_match_each_agents_deployed_layout` spells the new path
  literally.
- Existing Symlinks tests that named `.kiro/steering/skills` move to
  the new layout (agy keeps covering the `Link` shape).

### Free checks against the real CLI

- Done in planning (see Session Log): kiro's ACP `session/new` reports
  its skills with no model call. Used again after `just install` from
  this worktree: mAId's six skills listed, the old steering link gone,
  the user's own skills still listed.

### Paid (at most one cheap call)

- One `kiro-cli chat --no-interactive` asking which skills it has,
  after install, only if the ACP listing leaves doubt. The full Macro
  test runs at the initiative's Finish.

## Design

kiro reads skills from `~/.kiro/skills/*/SKILL.md` (and the workspace's
`.kiro/skills/*/SKILL.md`), one level deep, following symlinks. That
dir already holds the user's own skills, so kiro's row becomes
`FanOut`: one link per mAId skill, the shape `Symlinks` already has
and tests. A `Link` of the whole dir would need the dir to be mAId's;
a link to the skills tree inside it is not read (checked, below).

The old link is reaped the way plugin agents' old links already are:

- `shared.rs`: kiro's REGISTRY row becomes
  `(".kiro/skills", "share/maid/skills", Kind::FanOut, Agent::Kiro)`.
  A new `LEGACY_LINKS` table, `(Agent, home_subpath, Kind)`, names
  `.kiro/steering/skills` (Link) for kiro. The header comment drops
  "No row uses it now".
- `deploy.rs`: `Plugins::reap` becomes a free function over
  `(home, home_sub, kind, dry_run)`, still matched by `is_maids`.
  `Symlinks` calls it for the selected agents' `LEGACY_LINKS` rows:
  after creating the new links on install (so a failed install keeps
  kiro's old link), and on uninstall, and dry-run on status. `Plugins`
  keeps calling it with `PLUGIN_AGENTS`' paths. The two tables stay
  separate because the plugin table also says which agents are plugin
  agents.
- `resources/tests/isolated-verify` finds the owning profile from
  kiro's link; it reads `~/.kiro/skills/kdevkit` instead.
- After the rebase: `resources/tests/verify-install`'s kiro check
  reads the same link.
- Docs: README install step 3 and `project.md` (Architecture, Hard
  constraints) name `~/.kiro/skills/<name>`.

## Implementation Plan

- [x] Registry row, `LEGACY_LINKS`, shared `reap`, Symlinks reaping on
  install / uninstall / status; unit tests above. `just ci` green.
- [x] `isolated-verify`, README, `project.md`.
- [x] `just install` from this worktree; ACP listing and `just status`
  checked; Session Log records both.
- [x] Move `verify-install`'s kiro check to the new link (the branch
  is already on install-rollback).

- *Risk note (accepted):* the tool that manages `~/.kiro/skills` may
  prune entries it did not write when it syncs. Not checked (it has no
  dry run here).
  If it does, kiro loses mAId's skills until the next `just install`,
  and `just status` shows them missing.

## Session Log

- 2026-10-08 · closure · Ringmaster read `ours()`, `force_reaches()`
  and the `act()` mapping in place of a third review cycle, and
  accepted the non-failing kept line. Rebased onto
  `initiative/installable`; `just ci` green (197). Backlog: none
  closed (no item covers kiro's skills path). Dropped as unwanted, not
  filed: a test for a symlinked `~/.kiro/skills`, a kiro-link check in
  isolated-verify's restore, `reap`'s dry-run `acted` flag.
- 2026-10-08 · review · Regenerated briefing: two more defects, fixed (a
  misplaced doc comment; no test for uninstall removing an unshipped
  kiro link). `just ci` 197 green.
- 2026-10-08 · review · The briefing returned two defects, fixed:
  a checkout-shaped link in kiro's dir with a shipped skill's name was
  still replaced and removed (two tests asserted it, carried over from
  the old steering path); and a kept entry counted as a failing skip,
  so `just uninstall` stopped before the MCP and profile steps.
  `just ci`, plugins test, rollback test 10/10, reinstall: 56 skills.
- 2026-10-08 · dev · Code review cycle 2: all three lenses PASS WITH
  NOTES, nothing at high. Fixed after it: the unshipped-link reap
  matched a checkout-shaped link, which in kiro's dir only the user
  makes; a same-named foreign entry printed a `--force` hint that no
  longer applied; verify-install's steering check missed a
  checkout-era link; three stale comments. Tests added for each, plus
  uninstall reaping the steering link and a dry run keeping an
  unshipped link. `just ci` green (196 unit tests), ignored plugins
  test green, rollback test 10/10, reinstall: 56 skills listed.
- 2026-10-08 · dev · Code review cycle 1: all three lenses PASS WITH
  NOTES, nothing at high. Fixed as a slice anyway, since the first
  finding broke a requirement: a skill dropped by a later install or a
  rollback (or linked from another profile, as isolated-verify does)
  left a dangling kiro link reported `ok`. Also: `--force` replaced or
  deleted a user's entry named like a mAId skill in kiro's dir;
  verify-install step 4 scanned the user's own kiro links. Re-pin:
  owner is `Symlinks` in deploy.rs; reuses `reap` and `is_maids`;
  install, uninstall and status all cover it. After the fix: rollback
  test passes, reinstall leaves 56 skills listed.
- 2026-10-08 · dev · `just install` from this worktree: six
  `.kiro/skills/<name>` links made, the old `.kiro/steering/skills`
  link removed, the user's 13 skill dirs there untouched. `just status
  kiro` shows the six links ok. ACP listing: 56 skills (the 50 from
  before plus mAId's six); context used at session start fell from 44%
  to 24%. No model call spent: the listing settles it, and the Macro
  test asks kiro at Finish. `resources/tests/rollback` (free) passes,
  including kiro returning to the first install.
- 2026-10-08 · planning · Checked on this machine with kiro-cli
  2.28.0, all free (no prompt sent):
  - The binary's default agent resources include
    `skill://.kiro/skills/*/SKILL.md` and the global
    `~/.kiro/skills/*/SKILL.md`; steering is read as file context.
  - `kiro-cli acp`, `initialize` then `session/new`, returns
    `_kiro.dev/commands/available` whose `prompts` carry each skill
    (`serverName: skill:config`). Real HOME: 50 skills, none of
    mAId's; context 44% used at start with the steering link in place.
  - A workspace with `.kiro/skills/{kdevkit,notes}` linked at the
    profile's skills and `.kiro/skills/wholetree` linked at the whole
    skills tree: `kdevkit` and `notes` listed, `wholetree` not. So
    per-skill links are followed, and a tree link inside the dir is not.
  - The same workspace with kiro started as the harness's
    `maid-readonly` agent: same skills listed, so custom agents inherit
    the skill resources and the kiro smoke stage will see mAId's skills.
  - A temp HOME cannot be used: kiro's login lives under the real
    HOME's data dir and is not moved by `XDG_DATA_HOME`.

## Decision Log

- Decision: FanOut into `~/.kiro/skills`, not Link · why: the dir is a
  real one with the user's skills; a tree link inside it is not read ·
  rejected: Link (would need the dir), both paths (steering costs
  context for nothing).
- Decision: the old steering link is removed, not kept beside the new
  links · why: it puts every mAId skill's full text into every kiro
  session (44% context used at start here) and adds nothing once the
  skills are listed.
- Decision: accept that the tool managing `~/.kiro/skills` may prune
  mAId's links · why: ringmaster ruling at planning; the failure shows
  in `just status` and the next install repairs it.
- Decision: `verify-install`'s kiro check moves in this stream · why:
  install-rollback merged first; ringmaster ruling at planning.
- Decision: a FanOut row reaps every mAId link (by `is_maids`) to a
  skill the profile does not ship, on install and uninstall; status
  lists them · why: a rollback, or isolated-verify's throwaway
  profile, would otherwise leave dangling links in a dir mAId does not
  own · rejected: matching only links into this profile (the earlier
  `expand` rule), which misses other profiles' links.
- Decision: `--force` acts only on Link rows · why: a FanOut dir is the
  agent's, so a same-named foreign entry is the user's.
- Decision: in a FanOut dir only a link into a profile's skills is
  mAId's; a checkout-shaped one is the user's · why: mAId never linked
  a checkout into kiro's dir, and a developer trying out a skill would.
- Decision: an entry mAId did not write in a FanOut dir is reported
  "not mAId's (kept)" and does not fail install or uninstall · why: in
  a dir shared with the user it is expected, and a failing skip
  stopped `just uninstall` partway.
