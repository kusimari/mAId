# Feature: setup-as-installable

Part of initiative: [[installable]] (stream 1)

## Git Setup

- Branch: feat/setup-as-installable
- Base: initiative/installable
- Worktree: `../mAId-worktrees/setup-as-installable`

## Feature Brief

Installing mAId becomes a real install. What it puts in front of the coding
agents (skills today, the browser MCP server, later tools and agents) runs
from user space alone: delete the checkout it came from and everything keeps
working. Any checkout or worktree can install, and the most recent install
wins. Stream 1 of the `installable` initiative: it settles the install shape
that plugins (stream 2) and rollback (stream 3) build on.

## The picture

```
              environment (given)        mAId installs (owned)               mAId dev (per checkout)
              ───────────────────        ─────────────────────               ───────────────────────
what          coding-agent CLIs, git,    skills + runtime closures           rust, just
              Chrome, nix (install only) (node + chrome-devtools-mcp)
how           host, home-manager, ...    one nix build into a mAId profile   flake devShell (direnv)
mAId cares    only that they're on PATH  ~/.local/state/maid/profile         flake.nix, flake.lock

just install  =  nix build --profile ~/.local/state/maid/profile .#default
                 then point each agent at the profile (once; paths are stable)

~/.claude/skills ─────────┐
~/.kiro/steering/skills ──┤
~/.codex/skills/<name> ───┼──→ ~/.local/state/maid/profile/share/maid/skills
~/.gemini/config/skills ──┘
claude/kiro/codex MCP  ────→ ~/.local/state/maid/profile/bin/maid-browser-mcp
```

Agents name the profile path, never a store path or a checkout. An install
only moves the profile to a new generation, so the agents' config is written
once and every later install takes effect without touching it.

## Handoff

- **Stage:** closed

### Crossings

<!-- Append one line per crossing; never edit or delete a line. -->

- research → planning
- planning → dev · EXCEPTION · skipping: the Planning Review Gate (no PR yet)
  · why: the user set the design directly and asked for the build
- dev → review
- review → dev · RETURN · fault: implementation · issue: `just install`
  always passed --force, so it replaced any symlink at a managed path,
  including ones mAId did not write · fix: replace a symlink without --force
  only when it points at a mAId skills tree; anything else is a skip naming
  --force; unit-test both cases · done when: `just ci` passes and `just
  install` from this worktree reports every link ok
- dev → review
- review → closed

## Requirements

### Install

- `just install`, from any checkout or worktree, builds everything mAId ships
  (skills and the browser MCP server) into the mAId profile and points every
  agent at it: claude, kiro and codex by link (and agy, moved untested).
  Re-running updates in place.
- Deleting the checkout afterwards changes nothing about what is installed.
  Nothing installed (links, MCP registrations, the profile's closure) names a
  checkout.
- A later install from another checkout or worktree takes over by adding a
  profile generation; agent config is not rewritten.
- The browser MCP server, started as the agents' config starts it, runs with
  only `HOME` set (it finds the user's allowlist there): node and
  chrome-devtools-mcp come with the profile, nothing is fetched at run time.
- `just uninstall` removes the profile, the links and the MCP registrations,
  and leaves user data (browser allowlist, learned writing rules) alone.
  `just status` reports the live generation, each link, and whether each
  agent has the browser registration.

### Dependencies

| Kind | What | Provided by |
|---|---|---|
| Runtime prerequisites | the coding-agent CLIs, `git`, a graphical Chrome (browser only) | the environment, however it likes |
| Runtime-provided | node, chrome-devtools-mcp, the skills | mAId's install, as nix closures |
| Dev prerequisites | `nix` with flakes, `direnv` (optional) | the developer |

mAId does not check how a prerequisite was installed, only that it is there
when a step needs it. `nix` is needed to install, not at run time.

## Test Strategy

- Unit (`just test`): registry rows resolve to profile paths, not checkout
  paths; install, status and uninstall against a temp profile and temp HOME
  round-trip and are idempotent; install with no profile names the fix.
- Integration (by hand, needs nix): `nix build --profile` into a temp profile
  from a throwaway clone; delete the clone; skills readable, the launcher
  answers MCP `initialize` with only `HOME` set; no closure path contains the
  clone's path; a second clone's install moves the generation.
- Acceptance (by hand): `just install` from this worktree; `just status`
  shows every link on the profile. Uninstall keeping user data and new agent
  sessions are the initiative's Macro test, scripted in stream 3.

## Design

- **Flake outputs.** `packages.chrome-devtools-mcp`: the npm tarball by
  `fetchurl` with a pinned hash, wrapped with `nodejs_22` (the package has no
  runtime dependencies, so no npm install). `packages.default` (`maid`):
  `share/maid/skills` copied from `resources/content/skills`, and
  `bin/maid-browser-mcp`, the allowlist launcher with the server's
  store path baked in. `devShells.default` stays the dev closure.
- **Profile.** A dedicated profile at `~/.local/state/maid/profile`, not the
  user's default (home-manager owns that here). `nix build --profile` gives
  generations, GC roots and rollback for free.
- **Agent pointers.** The registry's sources become profile paths. `Symlinks`
  keeps its job (Link / FanOut) with a different source root; the browser
  registration passes the profile's launcher path to `manage`. Neither changes
  on later installs.
- **Launcher.** `launch` loses the `nix develop` re-exec and becomes a
  `writeShellApplication` in the flake; the allowlist logic is unchanged.
- **Project constraints that change.** "Registry destinations are symlinks
  back into the checkout" and "edits are live" go; "no global state mutation"
  gains one owned path, the mAId profile.

## Implementation Plan

- [x] Flake: `chrome-devtools-mcp` and `maid-browser-mcp` packages, `default`
      bundling skills and launcher; drop node from the dev shell.
- [x] build-tool: registry sources read from the profile; `validate` verb;
      install validates what it links.
- [x] Verbs: `just install` / `uninstall` / `status`; skills and browser
      verbs build the profile first; `isolated-verify` installs its clone
      into a temp profile.
- [x] writing-style: learned rules in a user-owned file.
- [x] Docs: README, project.md.
- [x] Acceptance: `just install` from this worktree, then the worktree's
      own checkout path is absent from everything installed.

## Session Log

- 2026-10-08 · Research.
  - Today nothing survives the checkout: every registry destination is a
    symlink into `resources/content/skills` (on this host `~/.claude/skills`
    resolves into the main checkout), and the browser launcher re-enters the
    repo flake with `nix develop path:<repo>` and runs `npx
    chrome-devtools-mcp@latest`, a run-time download.
  - chrome-devtools-mcp is not in nixpkgs (checked the repo's lock and
    nixos-unstable). Its npm tarball (1.10.1) declares no dependencies, so it
    packages with `fetchurl` and a node wrapper.
  - Spike, in a throwaway clone with the two flake outputs above:
    `nix build --profile <tmp>/profile .#default` built in 2s and made
    generation 1. A second clone with one skill edited made generation 2 and
    took over. Both clones then deleted: skills still there, the edited one
    showing; no closure path contains the clone path; both generations are
    listed as GC roots; `env -i .../bin/maid-chrome-devtools-mcp --help` runs;
    `nix profile rollback --profile` returns to generation 1. Closure 259 MiB,
    mostly node, shared with anything else on node 22.
  - Skills expect `git` at run time; nothing else.
  - Not checked: an agent loading skills through a read-only store path (it
    already follows a symlink today, so expected to work); a flake build from
    a worktree with untracked files (nix ignores them, so new files need
    `git add` before install).

- 2026-10-08 · Dev. `just ci` green (167 + 53 + 3 tests); the new install
  guard test fails with the guard removed. `just install` from this worktree:
  every agent link moved from the main checkout to the profile; browser MCP
  skipped (no Chrome on this host). Claude Code re-listed skills from the
  store-backed path mid-session. No path in the installed closure names any
  checkout.
  - Acceptance: a throwaway clone with one skill edited ran `just install`
    (generation 2), then the clone was deleted. The edited skill was still
    served, and `maid-browser-mcp` under `env -i` answered an MCP
    `initialize`. Reinstalling from this worktree took over again
    (generation 3).
  - Not run: the paid skill tests and `isolated-verify` (user-driven), and the
    attended browser test (needs Chrome).

- 2026-10-08 · Dev, as stream 1 of `installable`, rebased onto it. `just ci`
  green (167 + 53 + 3); `verify-skills-dry` passes. A throwaway clone built
  into a temp profile, then deleted: skills present, no path in the closure
  names the clone or any checkout, `maid-browser-mcp` answered an MCP
  `initialize` under `env -i HOME=...`. `just install` from this worktree
  (generation 4) and `just status` show every link on the profile; browser
  MCP skipped, no Chrome. `isolated-verify` with an unmatched selector built
  its throwaway profile, exited 2, and put the links back.
  - Code review, two cycles, correctness lens only (no named-risk paths),
    both PASS WITH NOTES. Taken: `status` shows the generation number;
    `isolated-verify` refuses without a profile to restore to and exits 4 on
    a failed restore (before, it could leave the links on a missing profile
    and print success); the smoke verbs refuse a stale profile; the pinned
    chrome-devtools-mcp no longer checks npm for updates; stale verb names
    in messages and docs.

- 2026-10-08 · Return from review: `just install` took over foreign links.
  Rebased onto `initiative/installable` (7b55771). `install` without
  --force now replaces only a link into a mAId skills tree; `just install`
  no longer passes --force, and keeps going past a skipped link to register
  the browser server, then exits 1. New deploy tests: a link another mAId
  install left (checkout and profile shapes) is replaced without --force,
  and fails with the narrowing removed; the existing foreign-link test fails
  with the narrowing widened to every link; `is_maids` boundary table. `just
  ci` green (169 + 53 + 3). `just install` from this worktree (generation
  6): all nine links ok. In a temp HOME with a foreign `.claude/skills`
  link: kept, the other links done, browser step ran, exit 1. Code review,
  correctness lens: PASS WITH NOTES; took the keep-going and boundary-test
  notes.
  - Paid smoke, isolated: `verify-skills-isolated smoke writing-style`
    (writing-style's text changed in this stream, so it shows the profile
    serves the new copy). 6/6 pass: discovery and integration on claude,
    kiro and codex; links restored to the profile, no leak.

- 2026-10-08 · Closure. No unticked plan items. Handoff resolved: plugins
  are stream 2, rollback, keeping one generation and the Macro test script
  are stream 3, kaimux joining the profile is in project.md ("Later
  installables join here"); a dev-mode checkout link and a `profile_dir()`
  unit test are not wanted. Filed to backlog: `install-link-takeover-gaps`,
  `smoke-stale-profile-guard-in-binary`. project.md Architecture now says
  install takes over only mAId's own links. No backlog item closed.

## Decision Log

- Research's open questions take the recommended answers · the user set the
  design and did not override them: learned writing rules move to
  `${XDG_CONFIG_HOME:-~/.config}/maid/writing-style/learned.md`; no live-edit
  mode, a checkout edit needs `just install`; chrome-devtools-mcp is pinned in
  the flake; generations older than 30 days are wiped on install.
- One profile path, written into agent config once · an install only adds a
  generation, so no agent config changes per install.
- Validation runs twice: `validate` on the checkout before the build, and
  install on the profile it links · the second checks what agents will read.
- Scoped to stream 1 of `installable` · the initiative split plugins and
  rollback into streams 2 and 3: the `just rollback` requirement moved to
  stream 3, and "every agent loads the skills" now means by link, which
  stream 2 replaces with plugins for claude and codex.
- The stale-profile guard is a Just recipe, not a build-tool check · it
  compares the store path the checkout would build with the installed one,
  which needs nix, and stream 2 adds a second `Deploy` impl that a new trait
  method would have to carry. Cost: a direct `cargo run -- smoke` is not
  guarded.
- Integration and acceptance stay hand-run, not tests · they need nix and
  the real agents, and stream 3 scripts the Macro test that covers them; the
  plan's unit tests for "a second install moves the generation" and
  "uninstall leaves user data" were nix and shell behaviour, not build-tool's.
- `install` takes over a link without --force only when it points at a mAId
  skills tree · ringmaster return: "latest install wins" covers mAId's own
  links, the initiative forbids touching agent config mAId did not write;
  matched by path shape, so a fork with the same layout counts as mAId's.
