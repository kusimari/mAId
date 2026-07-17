# Project: mAId

## Mission

<!-- What this project exists to do, and who it serves. One
     paragraph. Change only when the goal itself changes. -->

Tool-agnostic source of truth for my agentic skills — compiled
into whatever AI tool I'm using (Claude Code, Kiro, Codex, future
tools). The repo is the checked-in source; `just resources::install`
creates the `$HOME`-facing symlinks each tool reads from. Every
supported tool discovers skills natively at its own skills path, so
skills are all that's installed — no global instruction preamble.
One canonical set of skills, many consumer surfaces. Apps that ship
binaries (today: future `kaimux/`, the agent-orch successor) live as
sibling workspace members with their own native cargo verbs.

## Architecture

<!-- The logical shape: components, responsibilities, how they
     talk. Diagrams optional; words mandatory. Not a file-layout
     listing — see Layout for that. -->

Two halves at the top level:

- **`resources/`** — three layers, working together:
  1. **Content** (`resources/content/skills/<name>/SKILL.md`) —
     the skill definitions the AI tools load. Skills are the only
     deployed artefact; each tool auto-discovers them at its own
     skills path.
  2. **Tooling** (`resources/build-tool/`) — Rust crate
     (single-file) that does the install. Validates content
     and creates/removes/reports the `$HOME`-facing
     symlinks. Plus a small bash script
     (`resources/tests/run`) that drives the AI tools
     against the installed content. Rust where types help
     (the symlink state machine and content validator);
     bash where shelling out to other tools is the job
     (driving `claude --print` / `kiro-cli`).
  3. **Verbs** (Justfile recipes that use the tooling) —
     `just resources::install`, `just resources::uninstall`,
     `just resources::status`, `just resources::verify`
     (single-fixture: `just resources::verify-one <name>`).
     These are how a human or another tool consumes the
     tooling.
- **`kaimux/`** — tmux-pane orchestrator for coding-agent
  sessions. Single-binary Rust crate (workspace member).
  Wraps `claude` / `kiro-cli` calls so each running agent
  registers itself as a tracked tmux pane; a top-level
  dashboard pane shows the inventory, status, and a
  one-key jump to any of them. State lives in
  `$XDG_STATE_HOME/kaimux/sessions.json`. No symlinking;
  built by `just kaimux::build` to `dist/kaimux` and
  invoked directly (typically via a tmux keybind that
  `kaimux setup` installs into the user's tmux config).

Both halves are members of one cargo workspace at the
root, so `cargo build --workspace` covers everything.

**Registry** lives inline at the top of
`resources/build-tool/src/main.rs` (a `&[(&str, &str)]`
slice of `(home_subpath, source_subpath)` tuples). The
authoritative manifest for what gets installed where.
Tool adaptation lives here: adding a new coding-agent tool
= adding its expected `$HOME` paths as registry entries,
not rewriting content. Content stays tool-agnostic; the
registry translates it into each tool's expected layout.

mAId installs **skills only** — each supported tool discovers them
natively at its own skills path (`~/.claude/skills`,
`~/.kiro/steering/skills`, `~/.codex/skills`), verified to load with
no extra preamble. mAId deliberately installs no global instruction
file: `AGENTS.md` is a repo-root convention (per-project, alongside
README.md), not a global per-tool preamble, and "load the project's
AGENTS.md / project.md" is kdevkit's work-time instruction rather
than something deployed here.

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
- **Entrypoints:** Justfile organised as a root file with
  `mod` declarations per area, so verbs are namespaced by
  what they touch:
  - **`resources::*`** (operate on `$HOME` or the AI tools):
    `just resources::install`, `just resources::uninstall`,
    `just resources::status`, `just resources::verify`
    (drives `claude --print` against installed content;
    costs API credits, gated behind a confirmation prompt),
    `just resources::verify-one <name>`.
  - **`kaimux::*`** (operate on the kaimux crate):
    `just kaimux::build` (release + copy to `dist/`),
    `just kaimux::test`, `just kaimux::integration`.
  - **Workspace hygiene** at the root (no namespace —
    operates on every member): `just test`, `just fmt`,
    `just fmt-check`, `just lint`, `just check`,
    `just ci` (full gate).
  Each recipe is a one-liner over native cargo or a bash
  fixture-runner — `just --list` shows the root verbs,
  `just --list <module>` drills into a module. There is
  no installed binary on `$PATH`; the build-tool is
  invoked through `cargo run -p build-tool` from the
  checkout (wrapped by Just).

## Layout

<!-- Directory tree with a one-line annotation per entry. What
     lives where, not why. -->

```
mAId/
├── Cargo.toml              workspace root: members = ["resources/build-tool"]
├── Cargo.lock              committed (binary-workspace policy)
├── Justfile                root verb surface (workspace hygiene + `mod resources` / `mod kaimux`)
├── rust-toolchain.toml     stable + clippy + rustfmt
├── flake.nix / .envrc      repo-local rust toolchain + just (direnv + rust-overlay)
├── resources/
│   ├── Justfile            `resources::*` verb surface (install/uninstall/status/verify)
│   ├── build-tool/         single-file Rust crate (install/uninstall/status)
│   │   ├── Cargo.toml      deps: clap, anyhow; dev: tempfile
│   │   └── src/main.rs     registry + content checks + symlink core + clap + tests
│   ├── content/            the deployable skills
│   │   └── skills/<name>/SKILL.md   (the only deployed artefact)
│   └── tests/              bash fixture-runner (drives `claude --print` against installed content)
│       ├── run             entrypoint (`just resources::verify` calls this)
│       └── skills/<name>.smoke   fixtures: prompt + expect_substr or expected_narrative
├── kaimux/                 tmux-pane orchestrator for coding-agent sessions
│   ├── Justfile            `kaimux::*` verb surface (build/test/integration)
│   ├── Cargo.toml          deps: clap, anyhow, fd-lock, nix, notify, serde, serde_json
│   ├── src/main.rs         single-file, typeclass-shaped (Session/Store/Wrapper/Loop)
│   └── tests/              bash integration tests against real tmux
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

Two test layers, each scoped to what they verify.

**`just test` — workspace unit tests.** Rust unit tests
covering the content validator and the symlink state
machine against a `tempfile`-fake `$HOME`, plus the kaimux
crate's 54 unit tests against a tempdir `Store`. Fast
(sub-second). No real `$HOME` side effects, no API credits.
Load-bearing — this is the §8 Test Gate default. Includes
a structural integration test (`structural_install_to_real_directory_layout`)
that runs a full install→status→uninstall round-trip in
the fake $HOME, replacing the older bash structural smoke.

**`just resources::verify` — AI-tool functional tests.** Drives
`claude --print` (and `kiro-cli` when available) with the
`.smoke` fixtures under `resources/tests/skills/`. Two
fixture styles share the harness: substring fixtures
(`expect_substr:`) for cheap load-checks, and judge fixtures
(`expected_narrative:`) that run a second tool call to
evaluate whether the primary answer covers the expected
behavior. Slow (minutes), costs API credits, requires the
managed symlinks already deployed (i.e., run
`just resources::install` first). Gated behind a
confirmation prompt in the Justfile.

The §8 Test Gate uses `just test` by default. SKILL.md
prose revisions add `just resources::verify` (judge mode)
as their A/B evidence. The §9 close-out can run
`just resources::status` after an install to confirm
symlinks resolved.

### Functional tests are user-driven

Agentic runs (an AI assistant working through this project)
**must** stop at `just test`. `just resources::verify` costs
API credits and takes minutes; whether to spend that budget
on a given change is a human call. The agent prepares the
fixture, names the exact command, and hands off — it does
not run it. The Justfile's `[confirm]` gate on `verify`
provides a second line of defense.

Commands the user runs by hand:

- All fixtures: `just resources::verify`
- A single fixture: `just resources::verify-one <name>`
  (e.g. `just resources::verify-one notes-git-commit`).

The fixture file's basename (without `.smoke`) is the
`<name>`.

Quality gate: `just fmt-check` + `just lint` + `just check`
(or the bundled `just ci`). Run after any implementation
slice.

## Deployment

<!-- How code reaches users. Build, release, install, symlink,
     container, whatever applies. If the project isn't deployed
     in a traditional sense, describe how it's consumed. -->

Not a service — consumed locally.
`just resources::install` validates content and creates the
`$HOME`-facing symlinks; `just resources::uninstall`
reverses them. `just resources::status` reports current
managed-symlink state. `just resources::verify` drives the
real AI tools against the installed content. App workspace
members (`kaimux/`) build via `just kaimux::build` (a
one-liner over `cargo build -p kaimux --release` + copy
into `dist/`).

### Hard constraints

- **Never write into `~/.claude/skills/`, `~/.kiro/steering/skills/`,
  `~/.codex/skills/`, or any registry destination directly.** These
  paths are symlinks back into the checkout; a non-symlink file there
  breaks deploy invariants. Edit the source under
  `resources/content/` instead — the symlink exposes changes
  live. (This guardrail is mAId-project context — it protects mAId's
  own deploy invariant — which is why it lives here, not in a
  globally-installed preamble.)
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
