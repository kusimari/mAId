# Project: mAId

## Mission

<!-- What this project exists to do, and who it serves. One
     paragraph. Change only when the goal itself changes. -->

Tool-agnostic source of truth for my agentic resources — skills,
agents, commands, MCPs — compiled into whatever AI tool I'm
using (Claude Code, Kiro, future tools). The repo is the
checked-in source; `cargo xtask deploy` creates the
`$HOME`-facing symlinks that each tool reads from. One
canonical set of artefacts, many consumer surfaces.

## Architecture

<!-- The logical shape: components, responsibilities, how they
     talk. Diagrams optional; words mandatory. Not a file-layout
     listing — see Layout for that. -->

Three moving parts:

- **Sources** — everything under `sources/`. Skills, agents,
  commands, the tool-specific entrypoint files (`CLAUDE.md`,
  `KIRO.md`), and any Rust workspace members that ship as
  binaries (e.g. `agent-orch`). Content and code coexist;
  `transform` discovers Rust members at install-time by
  walking `sources/<name>/Cargo.toml`.
- **Registry** — `transform/src/registry.rs`. A static list
  mapping `$HOME`-facing paths to source paths. The
  authoritative manifest for what gets deployed where.
- **Build crate (`transform`)** — Rust binary that owns
  validate / deploy / undeploy / status / install /
  uninstall / test orchestration. Reads the registry,
  creates/reconciles/removes symlinks between `$HOME` and
  the checkout, runs the simplified frontmatter validator
  before any write, and shells out to `cargo build` for
  Rust workspace members during install.

Tool adaptation lives in the registry: adding a new coding-agent
tool = adding its expected `$HOME` paths as registry entries,
not rewriting sources. Sources remain tool-agnostic; the
registry translates them into each tool's expected layout.

## Tech Stack

<!-- Languages, runtimes, frameworks, key libraries. Versions
     only where version matters. -->

- **Runtime:** Rust (cargo workspace). The `transform` crate
  owns build automation; future Rust crates (e.g.
  `sources/agent-orch/`) are workspace members.
- **Isolation:** `flake.nix` + `.envrc` load the rust
  toolchain via direnv (rust-overlay). Cargo is a hard
  prerequisite — no `./install` shim. Users on machines
  without cargo on `$PATH` enter `nix develop` themselves.
- **Entrypoints:**
  - **Dev loop:** native cargo verbs — `cargo fmt`,
    `cargo clippy --workspace --all-targets -- -D warnings`,
    `cargo check --workspace`, `cargo test --workspace`.
    Custom verbs go through `cargo xtask <verb>` (alias in
    `.cargo/config.toml`): `validate`, `deploy`, `undeploy`,
    `status`, `install`, `uninstall`, `test-smoke`,
    `test-functional`. There is no installed binary on
    `$PATH`; `transform` is invoked through `cargo xtask`
    from the checkout.

## Layout

<!-- Directory tree with a one-line annotation per entry. What
     lives where, not why. -->

```
mAId/
├── Cargo.toml              workspace root: members = ["transform"]
├── Cargo.lock              committed (binary-workspace policy)
├── .cargo/config.toml      [alias] xtask = run -p transform --release --
├── rust-toolchain.toml     stable + clippy + rustfmt
├── flake.nix / .envrc      repo-local rust toolchain (direnv + rust-overlay)
├── transform/              the build crate
│   ├── Cargo.toml          deps: clap, anyhow, duct, shell-words; dev: tempfile
│   └── src/
│       ├── main.rs         clap dispatch
│       ├── sh.rs           sh!() helper over duct + shell-words
│       ├── registry.rs     ← authoritative deployment manifest
│       ├── deploy.rs       ← deploy + undeploy state machines
│       ├── sources.rs      walk sources/{skills,agents,commands}/
│       └── schema.rs       simplified frontmatter validator
├── sources/                everything the registry points at, plus Rust members
│   ├── skills/<name>/SKILL.md
│   ├── agents/<name>.md
│   ├── commands/<name>.md
│   ├── claude/CLAUDE.md    (→ ~/.claude/CLAUDE.md)
│   ├── kiro/KIRO.md        (→ ~/.kiro/steering/KIRO.md)
│   └── <crate>/            Rust workspace member (built into dist/<crate>)
├── tests/
│   └── functional/
│       ├── run                 ← harness (see Testing)
│       └── skills/<name>.smoke ← fixtures: prompt + expect_substr or expected_narrative
├── dist/                   gitignored — built binaries land here
├── target/                 gitignored — cargo's build dir
└── specs/
    ├── project.md          this file
    ├── feature/            in-flight + completed feature records
    └── backlog/            per-item files for wanted future work
```

## Testing

<!-- How this project is tested: unit, integration, smoke,
     manual. Which commands run which suite. Which are
     load-bearing vs. nice-to-have. -->

Three-layer test surface; the §8 Test Gate picks the right
one for the situation.

- **`cargo test --workspace`** — Rust unit tests covering
  the simplified frontmatter validator, the sources walker,
  and the deploy/undeploy state machines against a fake
  `$HOME` via `tempfile`. Fast (sub-second). Load-bearing.
  This is the §8 Test Gate default. Catches malformed
  frontmatter and broken deploy logic before any push.
- **`cargo xtask test-smoke`** — structural smoke, no tools
  required (shells `tests/functional/run --no-tools`).
  Asserts the managed symlinks actually resolve in real
  `$HOME` after a deploy and each skill is reachable through
  them. Cheap, no API credits, no extra PATH dependencies.
  Run after `cargo xtask deploy` to confirm the symlinks
  landed.
- **`cargo xtask test-functional`** — tool-driven smoke
  (shells `tests/functional/run`). Drives `claude --print`
  and `kiro-cli chat --no-interactive` with the `.smoke`
  fixtures under `tests/functional/skills/`. Two fixture
  styles share the harness: substring fixtures
  (`expect_substr:`) for cheap load-checks, and judge
  fixtures (`expected_narrative:`) that run a second tool
  call to evaluate whether the primary answer covers the
  expected behavior. Slow (minutes), costs API credits,
  requires both `claude` and `kiro-cli` on PATH. Use for
  revision passes that touch SKILL.md prose where you need
  evidence the cut preserved behavior.

The §8 Test Gate uses `cargo test --workspace` by default.
SKILL.md prose revisions add `test-functional` (judge mode)
as their A/B evidence. The §9 close-out can run
`test-smoke` after a deploy to confirm symlinks resolved.

### Functional tests are user-driven

Agentic runs (an AI assistant working through this project)
**must** stop at `test-smoke`. The judge-mode functional
suite costs API credits and takes minutes; whether to spend
that budget on a given change is a human call. The agent
prepares the fixture, names the exact command, and hands
off — it does not run it.

Commands the user runs by hand:

- All functional fixtures: `cargo xtask test-functional`
- A single fixture: `./tests/functional/run <name>` (e.g.
  `./tests/functional/run notes-git-commit`).

The fixture file's basename (without `.smoke`) is the
`<name>`.

Quality gate: `cargo fmt --all --check` + `cargo clippy
--workspace --all-targets -- -D warnings` + `cargo check
--workspace`. Run after any implementation slice.

## Deployment

<!-- How code reaches users. Build, release, install, symlink,
     container, whatever applies. If the project isn't deployed
     in a traditional sense, describe how it's consumed. -->

Not a service — consumed locally. `cargo xtask deploy` reads
the registry and creates the `$HOME`-facing symlinks;
`cargo xtask undeploy` reverses them. `cargo xtask status`
reports current managed-symlink state. `cargo xtask install`
runs deploy and additionally builds any Rust workspace members
under `sources/<name>/` into `dist/<name>/`.

### Hard constraints

- **Never write into `~/.claude/skills/`, `~/.kiro/steering/`,
  or any registry destination directly.** These paths are
  symlinks back into the checkout; a non-symlink file there
  breaks deploy invariants. Edit the source under `sources/`
  instead — the symlink exposes changes live.
- **Registry is the single source of truth** for deployment.
  Adding a new managed path = a registry change + CR, never an
  ad-hoc edit.
- **No global state mutation** on install. The rust toolchain
  comes from the repo-local flake; `transform` is invoked
  through `cargo xtask <verb>` from the checkout — no shim
  under `~/.local/bin`, no `cargo install` anywhere in the
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
