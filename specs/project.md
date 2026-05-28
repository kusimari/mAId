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
    `test`, `validate`, `deploy`, `undeploy`, `status`,
    `setup`, `teardown`. There is no installed binary on
    `$PATH`; `maid` is invoked through `deno run` /
    `deno task` from the checkout.
  - **Cold-start (env-side bootstrap):** `./install`
    (3-line pass-through into `deno task setup`);
    `./install --uninstall` → `deno task teardown`. The
    user's env-workplace driver invokes `./install` after
    cloning this repo.

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

Four-layer test surface; each layer is a `deno task` so the
§8 Test Gate can pick the right one for the situation.

- **`deno task test:unit`** (alias: `deno task test`) — 22
  tests covering schema parsing and deploy/undeploy
  invariants against a fake `$HOME`. ~100ms. Load-bearing.
  This is the §8 Test Gate default. Catches malformed
  frontmatter and broken deploy logic before any push.
- **`deno task test:smoke`** — structural smoke, no tools
  required (`./tests/functional/run --no-tools`). Asserts
  the managed symlinks actually resolve in real `$HOME`
  after a deploy and each skill is reachable through them.
  Cheap, no API credits, no PATH dependencies. Run after
  `deno task deploy` to confirm the symlinks landed.
- **`deno task test:functional`** — tool-driven smoke
  (`./tests/functional/run`). Drives `claude --print` and
  `kiro-cli chat --no-interactive` with the `.smoke`
  fixtures under `tests/functional/skills/`. Two fixture
  styles share the harness: substring fixtures
  (`expect_substr:`) for cheap load-checks, and judge
  fixtures (`expected_narrative:`) that run a second tool
  call to evaluate whether the primary answer covers the
  expected behavior. Slow (minutes), costs API credits,
  requires both `claude` and `kiro-cli` on PATH. Use for
  revision passes that touch SKILL.md prose where you need
  evidence the cut preserved behavior.
- **`deno task test:all`** — chains unit → smoke →
  functional. Use before merging a SKILL.md or harness
  change.

The §8 Test Gate uses `test:unit` by default. SKILL.md prose
revisions add `test:functional` (judge mode) as their A/B
evidence. The §9 close-out can run `test:smoke` after a
deploy to confirm symlinks resolved.

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
- **No global state mutation** on install. Deno comes from
  the repo-local flake; `maid` is invoked through `deno
  task <verb>` from the checkout — no shim under
  `~/.local/bin`, no `nix profile install` anywhere in the
  install path. (Install/uninstall of a `maid` binary is
  reserved for the future flake-package shape; see
  `specs/backlog/maid-as-flake-package.md`.)
- **No changes to the user's env-workplace** from this
  repo. mAId stays a pure-content workspace; bootstrap
  drivers belong on the env side.
- **Public repo — no internal references in any
  artefact.** Skills, specs, commit messages, PR
  descriptions, and project docs must not name internal
  products, teams, tickets, code reviews, repos, or
  stores. Use generic placeholders or hobbyist-flavoured
  examples. When asked to capture work that mentions
  internal names, route it to a corporate spec tree
  rather than letting names land here. The `kdevkit`
  skill encodes this rule for every project; this bullet
  declares mAId as a public repo so the rule fires.
