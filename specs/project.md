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
├── sources/                everything the registry points at +
│   │                       a self-contained Rust crate (agent-orch)
│   ├── skills/<name>/SKILL.md
│   ├── agents/<name>.md
│   ├── commands/<name>.md
│   ├── claude/CLAUDE.md    (→ ~/.claude/CLAUDE.md)
│   ├── kiro/KIRO.md        (→ ~/.kiro/steering/KIRO.md)
│   └── agent-orch/         tmux session orchestrator — Rust, single-file,
│                           built via `deno task agent-orch:build` to dist/
├── dist/                   gitignored; agent-orch:build's output
├── tests/
│   ├── schema_test.ts
│   ├── deploy_test.ts
│   ├── agent-orch/
│   │   └── integration.sh      ← shell-driven E2E test for agent-orch
│   └── functional/
│       ├── run                 ← harness (see Testing)
│       └── skills/<name>.smoke ← fixtures: prompt + expect_substr or expected_narrative
└── specs/
    ├── project.md          this file
    ├── feature/            in-flight + completed feature records
    └── backlog/            per-item files for wanted future work
```

## Testing

<!-- How this project is tested: unit, integration, smoke,
     manual. Which commands run which suite. Which are
     load-bearing vs. nice-to-have. -->

Test surface is split between **mAId-wide** layers
(deno-driven, content-side) and **agent-orch** layers (Rust +
shell, the tmux orchestrator under `sources/agent-orch/`).
Each layer is a `deno task` so the §8 Test Gate can pick the
right one for the situation.

### mAId-wide

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

### agent-orch (Rust + shell)

- **`deno task agent-orch:test`** — `cargo test`. ~50 unit
  tests covering the state machine, store, wrapper trait
  impls, hook event dispatch, and JSON merge for setup /
  teardown. ~100ms. Load-bearing. Runs in the §8 Test Gate
  when agent-orch code changed.
- **`deno task agent-orch:check`** — `cargo fmt --check` +
  `cargo clippy --all-targets -- -D warnings`. The agent-orch
  quality gate.
- **`deno task agent-orch:integration`** — builds + runs
  `tests/agent-orch/integration.sh`. Drives the compiled
  binary against a **private tmux server** with
  `XDG_STATE_HOME` pointed at a tempdir. Exercises real
  tmux side effects (`set-hook`, `set-option`,
  `switch-client`, `bind-key`) and real argv across
  `execvp`. ~10s. Load-bearing — covers what the in-process
  unit tests can't reach. Skips silently if tmux/jq/the
  dist binary are missing, so CI without tmux still
  passes.
- **`tests/agent-orch/functional-{setup,test,teardown}.sh`**
  — three scripts that drive the **user's real tmux server**
  with real claude/kiro-cli CLIs. `functional-setup.sh
  <KEY>` spawns the four-session fixture (proj-a, proj-b,
  proj-c, agent-orch) and installs hooks + the
  `<prefix> <KEY>` keybind. `functional-test.sh` fires real
  prompts and asserts the registry reflects actual agent
  activity. `functional-teardown.sh` reverses everything.
  Slow (real LLM round-trips), costs API credits.

### The §8 Test Gate

For mAId-wide changes (skills, agents, registry, deploy
logic): `deno task test:unit`. SKILL.md prose revisions add
`test:functional` (judge mode) as A/B evidence.

For agent-orch changes: `deno task agent-orch:check` +
`deno task agent-orch:test` + `deno task agent-orch:integration`.
The functional scripts are not mandatory for the §8 gate —
they're user-driven (see below).

### When the build env has tmux + claude + kiro-cli

If the build environment has `tmux`, `fzf`, `claude`, and
`kiro-cli` available on PATH, the agent-orch dev loop SHOULD
also run `tests/agent-orch/functional-setup.sh O` →
`functional-test.sh` → `functional-teardown.sh` after the
unit + integration gates. This is the only thing that catches
end-to-end regressions like "hooks aren't firing on the
toolbox shim" or "the heartbeat thread quietly stopped". When
those CLIs aren't available (CI, fresh container), the loop
skips this layer with a clear message.

### Functional tests are user-driven

Agentic runs (an AI assistant working through this project)
**must** stop at `test:smoke` for mAId-wide changes and at
`agent-orch:integration` for agent-orch changes. The judge-
mode and functional suites cost API credits and take minutes
per run; whether to spend that budget on a given change is a
human call. The agent prepares the fixture, names the exact
command, and hands off — it does not run them.

Commands the user runs by hand:

- mAId-wide: `deno task test:functional`, or a single
  fixture via `./tests/functional/run <name>` (e.g.
  `./tests/functional/run notes-git-commit`).
- agent-orch: `tests/agent-orch/functional-setup.sh O`
  (or any unbound prefix-suffix key), then `tmux attach -t
  agent-orch` to verify by hand. Run
  `tests/agent-orch/functional-test.sh` for asserted
  scenarios. Tear down with `functional-teardown.sh`.

The fixture file's basename (without `.smoke`) is the
`<name>` for mAId-wide functional fixtures.

Quality gate: `deno task fmt` + `deno task lint` + `deno task
check` for mAId-wide changes; `deno task agent-orch:check` for
agent-orch changes. Run after any implementation slice.

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
  install path.
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

## Agent Development

<!-- Skill-scoped preferences. Each subsection is a skill name. -->

### kdevkit

- `code_review:`
  - `reviewer: host-native` — use the host coding agent's built-in
    code review (Claude Code's `/code-review` skill, Kiro's
    equivalent). No project-specific reviewer skill yet; revisit
    once host-native review proves consistently weak across
    feature work in this repo.
