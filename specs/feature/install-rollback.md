# Feature: install-rollback

## Git Setup

- Branch: feat/install-rollback
- Base: initiative/installable
- Worktree: `../mAId-worktrees/install-rollback`

## Feature Brief

Part of initiative: [[installable]] (stream 3)

One command puts every agent back on the install before the current one,
and the initiative's Macro test becomes a script anyone can re-run on
their own machine to prove the install end to end.

## The picture

```
mAId profile generations      ... 8   9 (live)
                                  ^
just rollback  ───────────────────┘  switch the profile one generation back,
                                     then re-run the plugin install so codex's
                                     cache and claude's listed version follow
                                     (kiro and agy follow the profile link at once)
               a generation the agents cannot use is refused before the switch

just verify-install          the Macro test, steps 1-6, free
just verify-install-paid     the same, plus asking each agent (9 model calls)
```

## Handoff

- **Stage:** closed
- **Carry forward (stream 4):** the Macro test's step 2 fails for kiro
  when the agents are asked (`just verify-install-paid`): kiro-cli reads
  skills from `~/.kiro/skills/<name>`, and the registry links
  `~/.kiro/steering/skills`. That failure is stream 4's
  (kiro-skills-path) to turn green; the script itself is correct.

### Crossings

<!-- Append one line per crossing; never edit or delete a line. -->

- planning → dev
- dev → review
- review → closed

## Requirements

### Rollback

- `just rollback`, from any checkout or worktree, returns claude, kiro
  and codex (and agy, untested) to the install before the live one. A new
  session in each sees the older skill text; `claude plugin list` and
  `codex plugin list` show the older install's version. It ends by
  printing which generation is live.
- Running it again steps one further back. A later `just install` moves
  forward again as usual and replaces the installs rolled back from, so a
  rollback after it returns to the install that was live before it.
- With no earlier install left, it says so and changes nothing.
- If the agents cannot be pointed at the earlier install (it predates
  something this checkout's install needs), it says which install and
  what it lacks, changes nothing, and exits non-zero.
- A location rollback leaves alone (a link mAId did not write) is
  reported and makes it exit non-zero, as `just install` does; the
  agents still move.
- The 30-day cleanup in `just install` always leaves the install before
  the live one, so there is always something to roll back to after an
  install.

### Macro test script

- `just verify-install` runs the initiative's Macro test steps 1-6 on
  the real machine, free (no model calls). Each step prints `PASS` or
  `FAIL` with what it saw; the run exits non-zero on any failure.
- `just verify-install-paid` runs the same steps and also asks each agent
  in a non-interactive session which skills it has (step 2), and whether
  the changed skill carries the second clone's marker, after the second
  install and after rollback (step 5).
- Both ask for confirmation first: they reinstall mAId several times and,
  because step 6 uninstalls, leave only one install in the history. The
  paid one also says it costs model calls.
- It touches only mAId-owned state. However it ends (pass, fail, or
  interrupt), it leaves mAId installed from the checkout it was run from,
  with the user's allowlist and learned-rules files as they were.
- It tests the committed `HEAD`: uncommitted edits are not in the clones,
  and it says so when the tree is dirty.

## Test Strategy

### Rollback test (free; nix and the claude and codex CLIs; temp HOME)

`resources/tests/rollback`, run by hand, with a temp `HOME`, a temp
profile and no model calls:

- Two installs (this checkout's flake, then a copy with one skill line
  added): rollback leaves claude and codex listing the first install's
  version, codex's cached skill and kiro's linked skill without the line,
  and `status` current. Fails if rollback skips the plugin install
  (codex stays on the second version).
- A second rollback with no earlier install: non-zero, the error names
  it, the live generation and both plugin versions unchanged.
- An earlier generation without a plugin marketplace: non-zero, the
  profile and both plugins stay on the live install. Fails if the check
  of the earlier generation is removed.
- A foreign link at a managed path: rollback exits non-zero, the agents
  move, the link is kept.
- An install whose build fails keeps the generations it would drop.
- Rollback, then an install of new content, then rollback: back on the
  install that was live, not the one the first rollback left. Fails if
  install keeps the generations above the live one.
- Cleanup floor: generations dated older than 30 days, then an install:
  the generation that was live before the install is kept and rollback
  reaches it. Fails if nix's wipe stops keeping it.

### Macro test (this machine)

- `just verify-install` once, all steps PASS; then `just
  verify-install-paid` once (inside the initiative's budget). `just
  status` afterwards shows the install from this worktree.
- Each check has a positive pair: step 5 sees the marker after the
  second install before checking it is gone after rollback; step 6
  checks the files exist before checking they are unchanged.

### Unit (`just test`)

- Unchanged unless dev touches build-tool. `just ci` stays green.

## Design

- **Rollback is nix's rollback plus the plugin install.** `nix profile
  rollback --profile` already switches a profile one generation back,
  steps further on each call and errors when nothing is older (checked);
  stream 2 settled that codex then needs `codex plugin add` and claude
  `claude plugin update`, which `build-tool install` already runs. So
  `resources::rollback-profile` finds the generation below the live
  one, refuses it unless it has the skills, the plugin manifest and the
  browser launcher, switches with `nix profile rollback --to`, runs
  `install-skills` and passes on its exit code. Root `just rollback`
  calls it, as root `install` calls the resources verbs. The browser
  registration names the profile path, so it needs nothing more.
- **Install drops the generations above the live one**, after its build
  succeeds and before it sets the profile, and wipes only when it added a
  generation.
- **No keep-one floor code.** `nix profile wipe-history --older-than`
  keeps the newest generation older than the cutoff ("it existed at the
  requested time, so you can roll back to it"), and never the live one.
  The rollback test pins this, so a nix change that drops it fails there.
- **The script is bash, `resources/tests/verify-install`,** beside
  `isolated-verify`, which it resembles: clone the committed `HEAD`,
  share the build cache, a trap that restores. Agent output is JSON read
  with `jq`, added to the dev shell. Agents are asked through their own
  CLIs (`plugin list --json`, `mcp get`, a non-interactive session with
  the same read-only flags the skill-test harness uses), not mAId's
  status, so the test checks what each agent reports.
- **Step by step:**
  1. Clone `HEAD` to a temp dir, `just install` from it, delete it.
  2. claude and codex list `maid@maid`, enabled, at the profile's plugin
     version; kiro's skills link is the profile's. Paid: each agent's
     reply names every shipped skill.
  3. `env -i HOME=<home> <profile>/bin/maid-browser-mcp` answers an MCP
     `initialize` within 10s. That is the command `manage` registers.
     With no allowlist pattern in the user's file, `<home>` is a temp
     dir holding a one-pattern allowlist; the user's file is not written.
  4. Search mAId's entries in each agent's own reports (marketplace and
     plugin lists, MCP entries), the kiro and agy links, the marketplace
     dir links and the profile's own store tree for the clone's path,
     this checkout's path (both as given and resolved) and
     `resources/content/skills`. No hit.
  5. Second clone, a marker line appended to one skill (uncommitted; nix
     builds the dirty tracked file),
     `just install`, delete the clone: plugin versions change, kiro's
     file has the marker (paid: each agent quotes it). `just rollback`
     from this checkout: versions are step 1's, kiro's file has no marker
     (paid: each agent finds none).
  6. Record the allowlist and learned-rules files; any that is absent is
     written as a comment-only file (the launcher ignores comments, so it
     enables nothing) and deleted at the end. `just uninstall`: neither
     plugin list has `maid@maid`, kiro's link is gone, the files are
     byte-identical. Then `just install` from this checkout.
- **The trap** runs on any exit after step 1 starts: `just install` from
  this checkout, delete the clones and temp homes, delete only the user-
  data files the script created.
- **Project docs that change:** project.md Tech Stack and Deployment
  (`rollback`, `verify-install`), Testing (the rollback test and the
  Macro test script), README's Install section.

## Implementation Plan

- [x] `resources::rollback-profile` and root `just rollback`, with the
      check of the earlier generation; `install-profile` drops generations
      rolled back from;
      `resources/tests/rollback` covering the cases.
- [x] `resources/tests/verify-install` (free steps), `jq` in the dev
      shell, root `verify-install` recipe; one free run on this machine.
- [x] The paid part and `verify-install-paid`; one paid run (kiro's
      skill list fails, see Session Log: a stream 1 registry finding).
- [x] Docs: README, project.md.

- *Risk note:* the Macro test clears the rollback history on every run
  (step 6 uninstalls, which removes the profile). Said in the
  confirmation prompt.
- *Risk note:* each clone's install compiles build-tool; the shared
  build cache keeps that to a rebuild, not a cold build.

## Session Log

- 2026-10-08 · Closure. Rebased onto `initiative/installable` (spec-only
  changes upstream), `just ci` green. No unticked plan items. Handoff
  resolved: the kiro failure is stream 4 (ruling); rolling forward, an
  agent selector and a second-output guard are not wanted (nothing asks
  for them; the selector was ruled out). project.md already carries the
  touched sections from dev: Architecture (rollback, the drop, the
  conditional wipe), Tech Stack (verbs, `jq`), Layout (the two test
  scripts), Testing (both tests), Deployment (`just rollback`). Backlog:
  no item closed (none is about rollback or the Macro test), none filed.

- 2026-10-08 · Review briefing returned defects, fixed: docs said the dev
  shell is rust and just only (now names `jq`); project.md's rollback-test
  case list was missing two cases; an unchanged reinstall with every
  generation over 30 days old wiped the install before the live one (no
  generation is added, so the live one is the newest past the cutoff).
  Install now wipes only when it added a generation; new rollback-test
  case, which fails with the wipe made unconditional. Design's probe
  time corrected to 10s.

- 2026-10-08 · Code review, correctness lens, cycle 2: PASS WITH NOTES.
  Taken: an interrupted run printed "all checks passed" and exited 0
  (the EXIT trap's exit replaced 130); it now records a failure. Checked
  with a TERM 8s in: FAIL interrupted, reinstalled, exit 1. The claude
  filters select user scope, as `deploy.rs` does; the browser probe waits
  5s, not 20s (a full free run is now about 25s); rollback's README line
  says it covers every agent; the rollback test's header says which
  recipes are the clone's. Not taken: a guard for a second package
  output (no such output exists). `resources/tests/rollback` 9/9, `just
  ci` green, `just verify-install` 17/17 at `6246a00`.

- 2026-10-08 · Code review, correctness lens, cycle 1: PASS WITH NOTES.
  Taken: rollback reverted on any `install-skills` failure, including a
  skipped foreign link, and then claimed the revert failed; it now checks
  the earlier generation before switching and passes install's exit code
  on (new tests: foreign link; refusing is caught with the check
  removed, 4 fail). Install dropped generations before its build, so a
  failed build lost them; it now builds first (new test, fails with the
  old order). verify-install: aborts up front on a disabled plugin and
  on a missing tool; the paid skill-list check matches whole lines, not
  substrings; placeholder directories are recorded by walking up, not
  by parsing `mkdir -v`. Not taken: the 20s browser probe (slow, not
  wrong). `resources/tests/rollback` now 9/9; `just ci` green; `just
  verify-install` 17/17 again, `~/.config/maid` still absent after.
  The paid run predates the stricter skill-list match.

- 2026-10-08 · Dev, slices 2-4. `just ci` green (188 build-tool + 53
  kaimux + 4 integration), `verify-skills-dry` passes.
  - `just verify-install`, free, twice on this machine (38s): 17/17
    PASS. The first run left an empty `~/.config/maid/writing-style`
    behind (made for the learned-rules placeholder); removed by hand, and
    the script now removes the directories it made. The second run left
    `~/.config/maid` absent, as before.
  - `just verify-install-paid`, once (3m34s, 9 model calls plus one
    diagnostic kiro call): 26 PASS, 1 FAIL: kiro, asked for its skills,
    lists only the skills under `~/.kiro/skills` and none of mAId's.
    kiro-cli 2.28 reads skills from `~/.kiro/skills/<name>`; mAId links
    `~/.kiro/steering/skills`, which kiro treats as steering files: asked
    for the marker, kiro searched and quoted it, and after rollback found
    none. So kiro reads mAId's text but does not have it as skills. The
    fault is the kiro registry row (stream 1), not rollback; raised to the
    ringmaster, not fixed here.
  - The search in step 4 was checked to hit: a line holding this
    checkout's path matches both the path and the
    `/resources/content/skills` patterns.

- 2026-10-08 · Dev, slice 1. `resources/tests/rollback` passes (7
  checks, about 20s, temp HOME, real claude and codex CLIs). With the
  plugin install skipped after the switch, 5 fail; with the switch-back
  removed, 3 fail; with the drop of rolled-back generations removed, 3
  fail. A first version of the drop check passed with the drop removed:
  re-installing the same content reuses nix's newest generation, so the
  test now installs a third version.

- 2026-10-08 · Planning. Grounded on streams 1 and 2 and the code.
  - Checked in a temp profile with existing store paths: `nix build
    --profile` with the live store path adds no generation; `nix profile
    rollback --profile` steps 3 → 2 → 1, then errors "no profile version
    older than the current (1) exists" (exit 1) and stays on 1; `--to 3`
    goes back to 3.
  - Checked the floor: generations 1-3 dated 40 days back, an install
    made 4, `wipe-history --older-than 30d` removed 1 and 2 and kept 3;
    rollback from 4 reached 3.
  - This machine: generations 1-9; 1-7 have no `share/maid/marketplace`,
    8 and 9 do. `build-tool install` reads the plugin version from the
    profile's manifest and errors naming `just install` when it is
    missing, before any CLI call.
  - No browser allowlist and no learned-rules file exist here; no Chrome,
    so the browser server is not registered with any agent.
  - codex's `config.toml` and claude's own state also hold project paths
    the agents wrote themselves, so step 4 searches mAId's entries, not
    whole files.

## Decision Log

- Install drops generations above the live one (dev, after the planning
  gate) · nix rolls back to the next lower number, and reuses a generation
  only when it matches the newest one, so after rollback then a new
  install, rollback landed on the install rolled back from; considered
  leaving nix's order and documenting it, rejected because the Experience
  says "the install before it". Cost: no roll-forward to those
  generations, which nothing offered.

- Rollback checks the earlier generation before switching, and does not
  switch back on failure (code review, cycle 1) · `install-skills` exits 1
  for a link it left alone as well as for an unusable profile, so the
  switch-back made rollback fail and revert on any machine with a
  foreign link. Cost: the three paths it checks are layout knowledge in
  the Justfile, kept beside the other profile paths it already names.
- No keep-one-previous floor in mAId · nix's `--older-than` already keeps
  it; pinned by a test instead.
- The Macro test reads each agent's own CLI, not `just status` · the test
  should not trust mAId's own report of itself.
- Ruling (ringmaster, planning): no floor code; seed comment-only
  user-data files when absent and delete them after; step 3 uses a temp
  HOME with a one-pattern allowlist when the user has none; the Macro
  test clearing rollback history is accepted and said in the prompt; no
  agent selector on rollback; one paid run of 9 model calls.
