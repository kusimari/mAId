# Project: mAId

## What it is

Tool-agnostic source of truth for agentic resources — skills,
agents, commands, MCPs — compiled into whatever AI tool is in use
(Claude Code, Kiro, future tools). The repo is the checked-in
source; `maid deploy` creates the `$HOME`-facing symlinks that
each tool reads from.

## Development methodology

This project follows the spec-driven-development workflow in the
**`kdevkit` skill** — see
[`sources/skills/kdevkit/SKILL.md`](../sources/skills/kdevkit/SKILL.md).
Every non-trivial change starts from a feature file; in-flight
and completed feature records live under `.kdevkit/feature/`;
deferred work lives under `.kdevkit/feature-wip/`.

Two hard rules from the methodology worth repeating here:

- **Feature file stays current** — update Session Log / Decision
  Log after each meaningful unit of work; don't batch.
- **Phase gating** — don't chain phases (requirements → design →
  implementation) without an explicit go-ahead.

## Tech stack

- **Runtime:** Deno (TypeScript) for the `maid` CLI.
- **Isolation:** `flake.nix` + `.envrc` load `deno` via direnv;
  `./install` re-execs through `nix develop --command` as a
  fallback on machines without direnv.
- **Entrypoints:**
  - **Dev loop:** `deno task <verb>` for everything — `fmt`,
    `lint`, `check`, `test`, `install`, `uninstall`, `deploy`,
    `undeploy`, `validate`, `status`, `setup`, `teardown`.
  - **Cold-start / `Gorantls-env`:** `./install` (3-line
    pass-through into `deno task setup`);
    `./install --uninstall` → `deno task teardown`.
  - **Tool shim:** `deno task install` writes
    `$HOME/.local/bin/maid` via `deno install` — no bespoke
    wrapper script.
- **Registry:** `maid/registry.ts` is the authoritative list of
  which source paths get symlinked where. Every registry entry
  reads from `sources/`.

## Hard constraints

- **Never write into `~/.claude/skills/` (or any registry
  destination) directly.** These paths are symlinks back into the
  checkout; a non-symlink file there breaks deploy invariants.
- **Registry is the single source of truth** for deployment.
  Adding a new managed path = a registry change + CR, never an
  ad-hoc edit.
- **No global state mutation** on install. Deno comes from the
  repo-local flake; `maid` comes from a deno-generated shim in
  `~/.local/bin` — no `nix profile install` anywhere in the
  install path.
- **No changes to `env` or `Gorantls-env`** from this repo.

## Toolchain commands

All verbs are `deno task` entries — see
[`deno.json`](../deno.json) for the canonical list. Common ones:

- `deno task test` — full suite (22 tests).
- `deno task fmt` / `deno task lint` / `deno task check` —
  quality gate.
- `deno task setup` / `deno task teardown` — end-to-end install
  and uninstall (what `./install` and `./install --uninstall`
  delegate to).
- `deno task status` — report managed-symlink state.
- `./tests/functional/run` — real-tool round-trip smoke (add
  `--no-tools` for structural-only).

## Layout (for orientation)

```
mAId/
├── install                 3-line pass-through → deno task setup/teardown
├── flake.nix / .envrc      repo-local tooling isolation (direnv)
├── deno.json               task surface + fmt/lint/imports config
├── maid/                   Deno CLI
│   ├── main.ts
│   ├── registry.ts         ← authoritative deployment manifest
│   ├── deploy.ts           ← deploy + undeploy logic
│   ├── sources.ts
│   └── schema.ts
├── sources/                everything the registry points at
│   ├── skills/<name>/SKILL.md
│   ├── agents/<name>.md
│   ├── commands/<name>.md
│   ├── claude/CLAUDE.md    (→ ~/.claude/CLAUDE.md)
│   └── kiro/KIRO.md        (→ ~/.kiro/steering/KIRO.md)
├── tests/
│   ├── schema_test.ts
│   ├── deploy_test.ts
│   └── functional/run      real-tool round-trip smoke
└── .kdevkit/
    ├── project.md          this file
    ├── feature/            in-flight + completed feature records
    └── feature-wip/        scoped-but-deferred feature specs
```
