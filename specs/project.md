# Project: mAId

## Mission

<!-- What this project exists to do, and who it serves. One
     paragraph. Change only when the goal itself changes. -->

Tool-agnostic source of truth for my agentic resources — skills,
agents, commands, MCPs — compiled into whatever AI tool I'm
using (Claude Code, Kiro, future tools). The repo is the
checked-in source; `maid deploy` creates the `$HOME`-facing
symlinks that each tool reads from. One canonical set of
artefacts, many consumer surfaces.

## Architecture

<!-- The logical shape: components, responsibilities, how they
     talk. Diagrams optional; words mandatory. Not a file-layout
     listing — see Layout for that. -->

Three moving parts:

- **Sources** — everything under `sources/`. Skills, agents,
  commands, and the tool-specific entrypoint files
  (`CLAUDE.md`, `KIRO.md`). These are the authored artefacts.
- **Registry** — `maid/registry.ts`. A static list mapping
  `$HOME`-facing paths to source paths. The authoritative
  manifest for what gets deployed where.
- **CLI (`maid`)** — Deno TypeScript entrypoint. Reads the
  registry, creates/reconciles/removes symlinks between `$HOME`
  and the checkout. Ships with a schema validator that runs
  before any write.

Tool adaptation lives in the registry: adding a new coding-agent
tool = adding its expected `$HOME` paths as registry entries,
not rewriting sources. Sources remain tool-agnostic; the
registry translates them into each tool's expected layout.

## Tech Stack

<!-- Languages, runtimes, frameworks, key libraries. Versions
     only where version matters. -->

- **Runtime:** Deno (TypeScript) for the `maid` CLI.
- **Isolation:** `flake.nix` + `.envrc` load `deno` via direnv;
  `./install` re-execs through `nix develop --command` as a
  fallback on machines without direnv.
- **Entrypoints:**
  - **Dev loop:** `deno task <verb>` — `fmt`, `lint`, `check`,
    `test`, `install`, `uninstall`, `deploy`, `undeploy`,
    `validate`, `status`, `setup`, `teardown`.
  - **Cold-start / `Gorantls-env`:** `./install` (3-line
    pass-through into `deno task setup`); `./install --uninstall`
    → `deno task teardown`.
  - **Tool shim:** `deno task install` writes
    `$HOME/.local/bin/maid` via `deno install` — no bespoke
    wrapper script.

## Layout

<!-- Directory tree with a one-line annotation per entry. What
     lives where, not why. -->

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
└── specs/
    ├── project.md          this file
    ├── feature/            in-flight + completed feature records
    └── backlog/            per-item files for wanted future work
```

## Testing

<!-- How this project is tested: unit, integration, smoke,
     manual. Which commands run which suite. Which are
     load-bearing vs. nice-to-have. -->

Three layers:

- **Deno unit tests (`deno task test`)** — 22 tests covering
  schema parsing and deploy/undeploy invariants against a fake
  `$HOME`. Load-bearing. Must be green before any push.
- **Structural smokes (`./tests/functional/run --no-tools`)** —
  asserts the managed symlinks actually resolve in real `$HOME`
  after a deploy. Cheap, runs always.
- **Tool smokes (`./tests/functional/run`)** — drives
  `claude --print` and `kiro-cli chat --no-interactive` with
  one-line prompts that force each skill to announce itself
  (`[<skill>] applies`). Proves the skills actually load in
  live sessions, not just that the files are reachable. One
  `.smoke` fixture per skill; a single fixture can target both
  CLIs via a `tools: claude,kiro` field.

Quality gate: `deno task fmt` + `deno task lint` + `deno task
check`. Run after any implementation slice.

## Deployment

<!-- How code reaches users. Build, release, install, symlink,
     container, whatever applies. If the project isn't deployed
     in a traditional sense, describe how it's consumed. -->

Not a service — consumed locally. `deno task deploy` reads the
registry and creates the `$HOME`-facing symlinks; `deno task
undeploy` reverses them. `deno task status` reports current
managed-symlink state.

### Hard constraints

- **Never write into `~/.claude/skills/`, `~/.kiro/steering/`,
  or any registry destination directly.** These paths are
  symlinks back into the checkout; a non-symlink file there
  breaks deploy invariants. Edit the source under `sources/`
  instead — the symlink exposes changes live.
- **Registry is the single source of truth** for deployment.
  Adding a new managed path = a registry change + CR, never an
  ad-hoc edit.
- **No global state mutation** on install. Deno comes from the
  repo-local flake; `maid` comes from a deno-generated shim in
  `~/.local/bin` — no `nix profile install` anywhere in the
  install path.
- **No changes to `env` or `Gorantls-env`** from this repo.
