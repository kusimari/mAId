# Initiative: installable

- Branch: `initiative/installable` (stacked on `feat/kdevkit-ringmaster`;
  rebase onto `main` once that lands)

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
   it, with an empty environment, answers an MCP `initialize`.
4. A search of every agent config mAId wrote, and of mAId's install,
   finds no checkout path.
5. Install from a second clone with one visible skill change: the
   agents see it. Return to the previous install: they see the old
   text.
6. `just uninstall`: no agent lists mAId; the allowlist and
   learned-rules files are untouched. Then `just install` from the
   initiative worktree, to leave the machine installed.

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

## Decision Log

<!-- Ringmaster rulings, newest last:
     - Ruling: <what> · why: <why> · cost if wrong: <cost> -->
