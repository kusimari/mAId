# Initiative: installable

- Branch: `initiative/installable`, on `main` (rebased after the
  ringmaster feature it was stacked on merged)
- Status: closed 2026-10-08; five streams merged; Macro test passed.
  Reaches `main` as one merge commit (`git merge --no-ff`, or the
  forge's "Create a merge commit"), with this message:

  ```
  feat(installable): a true install the checkout can disappear from

  Installing mAId used to mean symlinks into a checkout, and a browser
  server that re-entered the repo and downloaded its runtime on start.
  Now `just install` builds the checkout into a nix profile in user
  space and each coding agent takes it through its own tooling where
  it has some; any checkout or worktree can install, the latest wins,
  and `just rollback` returns to the one before.

  - install into a nix profile the checkout can disappear from
  - claude and codex get mAId as a plugin from the profile
  - one command returns every agent to the previous install
  - kiro lists mAId's skills from ~/.kiro/skills
  - uninstall keeps going past a location it leaves alone
  ```

## Goal

Installing mAId becomes a true install. Today every agent reads mAId
through symlinks into a checkout, and the browser MCP server re-enters
the repo and downloads its runtime when it starts, so deleting or
switching the checkout breaks what is installed. After this, what mAId
installs into the coding agents is self-contained, any checkout or
worktree can install it, and the latest install wins.

## Experience

- From any checkout or worktree, `just install` installs everything
  mAId ships (today: skills and the browser MCP server) into claude,
  kiro and codex. A new session in each finds mAId's skills, and
  where Chrome is present, the browser tools.
- claude and codex receive mAId through their own plugin commands:
  mAId shows up in their plugin list and can be disabled there. kiro,
  which has no such command, gets the same content from
  mAId's own user-space install.
- Skills that hand work to each other by name (kdevkit to kyodakit and
  kreviewkit) still find each other.
- Deleting the checkout afterwards changes nothing about what is
  installed. Nothing installed names a checkout.
- Whatever an installed resource needs at run time (node, for the
  browser server) comes with mAId's install. Nothing depends on the
  agent's PATH or downloads at run time.
- A later install from another checkout or worktree takes over, and
  one command returns to the install before it.
- `just status` says what is installed and from which install;
  `just uninstall` removes everything mAId installed and keeps the
  user's own data (browser allowlist, learned writing rules).
- What mAId needs from the environment (the agent CLIs, git, Chrome,
  nix) is written down, and mAId does not care how it was provided.

## Macro test

Run by the ringmaster on the user's real machine, from the initiative
branch, before it goes to `main`:

1. Clone the branch to a temp dir, `just install` from the clone, then
   delete the clone.
2. claude and codex list mAId in their plugin lists; kiro points at
   mAId's install. A non-interactive session in each of
   claude, kiro and codex names mAId's skills when asked what skills
   it has.
3. The browser server, started exactly as the agents' config starts
   it, with no environment but `HOME` (where the user's allowlist
   lives), answers an MCP `initialize`.
4. A search of every agent config mAId wrote, and of mAId's install,
   finds no checkout path.
5. Install from a second clone with one visible skill change: the
   agents see it. Return to the previous install: they see the old
   text.
6. `just uninstall`: no agent lists mAId; the allowlist and
   learned-rules files are untouched. Then `just install` from the
   initiative worktree, to leave the machine installed.

### Result

2026-10-08, `just verify-install-paid` on `initiative/installable`
after stream 4 (9 model calls): all 26 checks pass. Step 1: install
from a clone, clone deleted. Step 2: claude and codex list `maid@maid`
enabled; kiro links each skill; claude, kiro and codex each name every
mAId skill. Step 3: the browser server answers `initialize` with only
`HOME` set. Step 4: no checkout path in anything mAId installed. Step 5:
a second clone takes over (all three quote its marker); `just rollback`
returns all three to the previous text. Step 6: uninstall leaves no
mAId entry and the user-data files byte-identical; reinstall from the
initiative worktree.

## Constraints

- The user space is a nix profile, and so is the dev environment (the
  flake's dev shell). No abstraction for mise or a plain OS yet.
- agy is not on this machine: leave its existing install as it is and
  do no new work for it.
- Use an agent's own install tooling wherever it has some; mAId's own
  layout knowledge only where it has none.
- Touch only what mAId owns: never the user's default nix profile
  (home-manager owns it), never agent config mAId did not write.
- Public repo: no internal names in any artefact.
- Prior work: `feat/setup-as-installable` (the nix profile, the
  browser server as a closure, `just install`/`status`/`uninstall`)
  is built and installed on this machine; reuse it rather than redo
  it.
- Spend: paid skill tests are allowed, up to three full
  `verify-skills` sweeps plus single-fixture reruns.

## Streams

<!-- Ringmaster-owned. One row per feature. -->

| # | Feature | Ships | Needs | Status |
|---|---|---|---|---|
| 1 | setup-as-installable | the nix profile with the skills and the browser server as closures; `just install` / `status` / `uninstall`; every agent linked at the profile; nothing installed names a checkout | none | merged |
| 2 | native-plugins | claude and codex get mAId as a plugin from a marketplace inside the profile: listed, disableable, updated per install; skills that call each other still resolve; their old links removed; `status` / `uninstall` cover plugins | 1 | merged |
| 3 | install-rollback | one command returns every agent to the previous install; the Macro test as a script anyone can re-run | 2 | merged |
| 4 | kiro-skills-path | kiro finds mAId's skills as skills: its link moves from `~/.kiro/steering/skills` to where kiro-cli reads skills (`~/.kiro/skills/<name>`), old link reaped | 1 | merged |
| 5 | uninstall-keeps-going | `just uninstall` finishes every step past a location it must leave alone, then fails at the end, as install does; the plugin era's stale doc lines fixed (from the initiative briefing) | 2 | merged |

## Decision Log

<!-- Ringmaster rulings, newest last:
     - Ruling: <what> · why: <why> · cost if wrong: <cost> -->

- Ruling: three sequential streams, not more · why: each ships
  something the Macro test can see, and 2 and 3 build on the install
  shape 1 settles · cost if wrong: a stream grows large; split it then.
- Ruling: stream 1 is the existing `feat/setup-as-installable`,
  rebased onto this branch · why: the constraint says reuse it · cost
  if wrong: none, its gates still run.
- Ruling: agy stays in stream 1's registry, linked at the profile,
  untested · why: "leave its existing install as it is" is best kept by
  moving it with the others rather than leaving a link into a checkout
  that may vanish · cost if wrong: agy breaks unnoticed; one registry
  row to revert.
- Ruling: Macro test step 3 runs the browser server with only `HOME`
  set, not an empty environment · why: the allowlist is user data under
  `HOME`; the intent (nothing from PATH, nothing fetched) is unchanged
  · cost if wrong: one wording; the user can tighten it.
- Ruling: stream 1 returns once to narrow `just install --force` to
  links mAId wrote (into a checkout's `resources/content/skills` or the
  profile's `share/maid/skills`) · why: the Constraints forbid touching
  agent config mAId did not write · cost if wrong: a little code.
- Ruling: stream 1 spends one paid single-fixture isolated smoke run ·
  why: smoke is the only stage that reads skills through the new
  store-backed link; within budget · cost if wrong: one run.
- Ruling: stream 3 decides whether rollback keeps at least one previous
  generation past the 30-day wipe · why: rollback is its job · cost if
  wrong: none now.
- Ruling: the stale pre-initiative `origin/feat/setup-as-installable`
  is left alone until close · why: under a ringmaster, feature
  branches are not pushed; close removes it · cost if wrong: none.
- Ruling: the browser MCP server stays registered by `just install`,
  not bundled in the `maid` plugin · why: it is registered only where
  Chrome is present and kiro needs that route anyway; a bundled server
  would start, and fail, in every session without Chrome · cost if
  wrong: disabling the plugin leaves the browser server registered.
- Ruling: in claude and codex, mAId's skills become `maid:<name>` (a
  typed `/kdevkit` becomes `/maid:kdevkit`; bare names still resolve) ·
  why: it is how both agents name plugin skills, and the user chose
  their plugin tooling · cost if wrong: retyping; raised with the user
  at the end.
- Ruling: at stream 2's review, accept its five recommendations: the
  name handoff stands on the planning spike plus the paid run, with
  Macro test step 2 as the end check; a disabled codex plugin falls
  behind with a skip line, not a failure; install stays fail-fast on an
  agent CLI error; the codex repair keys on codex's error text (a
  reword fails loud); `maid` / `maid@maid` are mAId's by name · why:
  each is the least that serves the Experience, and each failure mode
  is loud · cost if wrong: small, local fixes.
- Ruling: at stream 3's planning, accept its six recommendations: no
  extra keep-one-generation floor (nix keeps the previous live one; a
  test pins it); the Macro test writes a comment-only allowlist or
  learned-rules file only where one is missing, and deletes only those;
  step 3 uses a temp HOME with a one-pattern allowlist; a Macro test run
  leaves one install in the rollback history, said in its prompt;
  `just rollback` takes no agent selector; one paid run of 9 model
  calls · why: each is the least that proves the Experience without
  touching user data · cost if wrong: small.
- Ruling: the paid Macro test showed kiro does not list mAId's skills
  (kiro-cli reads `~/.kiro/skills/<name>`; stream 1 linked
  `~/.kiro/steering/skills`). Stream 3 ships with that red, since its
  script reports a real defect correctly; a new stream 4 fixes the kiro
  row · why: the fault is stream 1's layout, and stream 1 is merged ·
  cost if wrong: one small stream.
- Ruling: the one remaining paid Macro test run is spent at Finish, on
  the initiative branch, after stream 4 · why: one run then covers
  stream 3's late changes and the kiro fix · cost if wrong: none.
- Ruling: rollback's three profile-layout paths living in the Justfile
  as well as `deploy.rs` is accepted · why: drift makes rollback refuse,
  not break · cost if wrong: a refused rollback until both agree.
- Ruling: at stream 4's planning, kiro gets one link per skill in
  `~/.kiro/skills`, beside the user's own skills there, which another
  tool manages; the risk that tool prunes mAId's links is accepted and
  shown by `just status` · why: a steering fallback would put every
  skill's full text back in every kiro session · cost if wrong: kiro
  loses mAId's skills until the next `just install`.
- Ruling: at stream 4's review, the ringmaster read the post-review
  ownership fixes (`ours`, `force_reaches`) instead of a third panel
  cycle; a user's own same-named skill in `~/.kiro/skills` is kept with
  a "not mAId's (kept)" line and exit 0 · why: both are small and
  correct; failing on a normal state would break install · cost if
  wrong: kiro shows the user's skill instead of mAId's, as reported.
- Ruling: the initiative briefing's two defects become stream 5 · why:
  Finish routes briefing defects back as stream fixes · cost if wrong:
  none.
