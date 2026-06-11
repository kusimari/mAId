# Project: mAId

## Mission

<!-- What this project exists to do, and who it serves. One
     paragraph. Change only when the goal itself changes. -->

Tool-agnostic source of truth for my agentic resources — skills
and the AGENTS.md preamble — compiled into whatever AI tool I'm
using (Claude Code, Kiro, future tools). The repo is the
checked-in source; `just deploy` creates the `$HOME`-facing
symlinks that each tool reads from. One canonical set of
artefacts, many consumer surfaces. Apps that ship binaries
(today: future `kaimux/`, the agent-orch successor) live as
sibling workspace members with their own native cargo verbs.

## Architecture

<!-- The logical shape: components, responsibilities, how they
     talk. Diagrams optional; words mandatory. Not a file-layout
     listing — see Layout for that. -->

Two halves at the top level:

- **`resources/`** — markdown content the AI tools load,
  plus the build crate that operates on it.
  - `resources/content/` — the deployable content.
    `agents.md` is the merged tool-agnostic preamble (one
    source of truth, replacing per-tool CLAUDE.md / KIRO.md
    duplicates). `skills/<name>/SKILL.md` are the skill
    definitions.
  - `resources/build-tool/` — Rust crate that owns
    validate / deploy / undeploy / status. Reads the
    registry, creates/reconciles/removes symlinks between
    `$HOME` and the checkout, runs the simplified
    frontmatter validator before any write.
  - `resources/tests/` — bash harness that drives Claude
    Code and Kiro against the deployed content.
- **`kaimux/`** — future home for the kaimux app (was
  agent-orch). Sibling workspace member; built and tested
  via native cargo. Empty placeholder at this branch's
  merge; lands with code in its own feature.

Both halves are members of one cargo workspace at the
root, so `cargo build --workspace` covers everything.

**Registry** lives at `resources/build-tool/src/registry.rs`.
Static list mapping `$HOME`-facing paths to source paths.
The authoritative manifest for what gets deployed where.
Tool adaptation lives here: adding a new coding-agent tool
= adding its expected `$HOME` paths as registry entries,
not rewriting content. Content stays tool-agnostic; the
registry translates it into each tool's expected layout.

mAId follows the cross-tool **AGENTS.md** standard (Linux
Foundation Agentic AI Foundation; native support across
Codex, Copilot, Cursor, Kiro, Zed, Windsurf). Belt-and-
suspenders symlinks during the transition: legacy
`CLAUDE.md` and `KIRO.md` symlink at the same merged
`agents.md` source alongside the modern `AGENTS.md`. Drop
the legacy filenames when Claude Code adds AGENTS.md as a
default-read location.

## Tech Stack

<!-- Languages, runtimes, frameworks, key libraries. Versions
     only where version matters. -->

- **Runtime:** Rust (cargo workspace) + Just for the verb
  surface. `resources/build-tool` is today's only
  workspace member; future Rust crates (e.g. `kaimux/`)
  join as additional members.
- **Isolation:** `flake.nix` + `.envrc` load the rust
  toolchain + `just` via direnv (rust-overlay). Cargo and
  just are hard prerequisites — no `./install` shim. Users
  on machines without them enter `nix develop` themselves.
- **Entrypoints:** all dev verbs go through Just.
  `Justfile` recipes:
  - **Resources:** `just validate`, `just deploy`,
    `just undeploy`, `just status`.
  - **Tests:** `just test` (workspace unit tests),
    `just test-smoke` (structural smoke; no API credits),
    `just test-functional` (tool-driven; costs API
    credits), `just test-fixture <name>` (single fixture).
  - **Quality:** `just fmt`, `just fmt-check`, `just lint`,
    `just check`, `just ci` (full gate).
  Each recipe is a one-liner over native cargo or the test
  harness — `just --list` shows everything. There is no
  installed binary on `$PATH`; the build-tool is invoked
  through `cargo run -p build-tool` from the checkout
  (wrapped by Just).

## Layout

<!-- Directory tree with a one-line annotation per entry. What
     lives where, not why. -->

```
mAId/
├── Cargo.toml              workspace root: members = ["resources/build-tool"]
├── Cargo.lock              committed (binary-workspace policy)
├── Justfile                verb surface (validate/deploy/test/fmt/lint/...)
├── rust-toolchain.toml     stable + clippy + rustfmt
├── flake.nix / .envrc      repo-local rust toolchain + just (direnv + rust-overlay)
├── resources/              the markdown side
│   ├── build-tool/         the build crate
│   │   ├── Cargo.toml      deps: clap, anyhow, duct; dev: tempfile
│   │   └── src/
│   │       ├── main.rs     clap dispatch (4 verbs)
│   │       ├── registry.rs ← authoritative deployment manifest
│   │       ├── deploy.rs   ← deploy + undeploy state machines
│   │       ├── sources.rs  walk resources/content/<kind>/
│   │       └── schema.rs   simplified frontmatter validator
│   ├── content/            the deployable markdown
│   │   ├── agents.md       merged AGENTS.md preamble (→ CLAUDE.md, AGENTS.md, KIRO.md)
│   │   └── skills/<name>/SKILL.md
│   └── tests/              bash harness that drives Claude Code + Kiro
│       ├── run             harness entrypoint
│       └── skills/<name>.smoke   fixtures: prompt + expect_substr or expected_narrative
├── kaimux/                 (future) sibling workspace member for the kaimux app
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

- **`just test`** — Rust unit tests covering the
  simplified frontmatter validator, the sources walker, and
  the deploy/undeploy state machines against a fake `$HOME`
  via `tempfile`. Fast (sub-second). Load-bearing. This is
  the §8 Test Gate default.
- **`just test-smoke`** — structural smoke, no tools
  required (`resources/tests/run --no-tools`). Asserts the
  managed symlinks actually resolve in `$HOME` after a
  deploy and each skill is reachable through them. Cheap,
  no API credits, no extra PATH dependencies. Run after
  `just deploy` to confirm the symlinks landed.
- **`just test-functional`** — tool-driven smoke
  (`resources/tests/run`). Drives `claude --print` and
  `kiro-cli chat --no-interactive` with the `.smoke`
  fixtures under `resources/tests/skills/`. Two fixture
  styles share the harness: substring fixtures
  (`expect_substr:`) for cheap load-checks, and judge
  fixtures (`expected_narrative:`) that run a second tool
  call to evaluate whether the primary answer covers the
  expected behavior. Slow (minutes), costs API credits,
  requires `claude` (and optionally `kiro-cli`) on PATH.

The §8 Test Gate uses `just test` by default. SKILL.md
prose revisions add `test-functional` (judge mode) as their
A/B evidence. The §9 close-out can run `test-smoke` after a
deploy to confirm symlinks resolved.

### Functional tests are user-driven

Agentic runs (an AI assistant working through this project)
**must** stop at `test-smoke`. The judge-mode functional
suite costs API credits and takes minutes; whether to spend
that budget on a given change is a human call. The agent
prepares the fixture, names the exact command, and hands
off — it does not run it.

Commands the user runs by hand:

- All functional fixtures: `just test-functional`
- A single fixture: `just test-fixture <name>` (e.g.
  `just test-fixture notes-git-commit`).

The fixture file's basename (without `.smoke`) is the
`<name>`.

Quality gate: `just fmt-check` + `just lint` + `just check`
(or the bundled `just ci`). Run after any implementation
slice.

## Deployment

<!-- How code reaches users. Build, release, install, symlink,
     container, whatever applies. If the project isn't deployed
     in a traditional sense, describe how it's consumed. -->

Not a service — consumed locally. `just deploy` reads the
registry and creates the `$HOME`-facing symlinks;
`just undeploy` reverses them. `just status` reports
current managed-symlink state. App workspace members (the
future `kaimux/`) build via native cargo:
`cargo build -p kaimux --release && cp target/release/kaimux
dist/`.

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
  and `just` come from the repo-local flake; `build-tool` is
  invoked through `cargo run -p build-tool` (wrapped by Just)
  from the checkout — no shim under `~/.local/bin`, no
  `cargo install` anywhere in the install path.
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
