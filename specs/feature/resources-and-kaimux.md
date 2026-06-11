# Feature: resources-and-kaimux

## Git Setup

- Branch: feat/resources-and-kaimux
- Base: main (48535df), with the deno-to-rust code commits
  cherry-picked as the rust-port baseline.

## Feature Brief

Restructure the repo to make its two halves visible at the
top level: **`resources/`** (markdown content the AI tools
load + the build crate that operates on it) and **`kaimux/`**
(future home for the agent-orch Rust app, landing in a later
feature on the new shape). Drop the residual deno-to-rust
artifacts that don't fit the new shape: phantom `agents/`
and `commands/` registry slots that never had content; the
`xtask` cargo alias; per-tool entrypoint files (CLAUDE.md,
KIRO.md) that duplicate each other. Adopt the cross-tool
`AGENTS.md` standard (Linux Foundation Agentic AI Foundation,
~97k root-level files on GitHub, native support across
Codex / Copilot / Cursor / Kiro / Zed / Windsurf — Claude
Code reads it via `@import` or symlink). Add a Justfile so
the verb surface stays one-word per action.

Net: `transform/` → `resources/build-tool/`; `sources/{skills,
claude,kiro}/` → `resources/content/`; `tests/` →
`resources/tests/`; one merged `agents.md` instead of
CLAUDE.md + KIRO.md; cargo workspace stays at the root with
two members (`resources/build-tool` today, `kaimux` joining
in its own feature).

## Requirements

- The repo's two halves are visible from `ls`: `resources/`
  and `kaimux/`. (`kaimux/` is empty in this branch — added
  with code in a follow-up feature.)
- The build crate moves from `transform/` to
  `resources/build-tool/` and renames its package to
  `build-tool`.
- The `xtask` cargo alias is removed. Verb surface is
  Justfile recipes calling native cargo (`cargo run -p
  build-tool --release --quiet -- <verb>`).
- `Justfile` at the repo root defines: `deploy`, `undeploy`,
  `status`, `validate`, `test-smoke`, `test`, `fmt`, `lint`.
  Future `build-kaimux`, `test-kaimux` slots noted in
  comments but not added until kaimux lands.
- `resources/content/agents.md` is the merged tool-agnostic
  preamble. Tone: "Read your skills directory" rather than
  Claude/Kiro path naming.
- Registry deploys belt-and-suspenders for the AGENTS.md
  transition: `~/.claude/CLAUDE.md`, `~/.claude/AGENTS.md`,
  `~/.kiro/steering/KIRO.md`, `~/.kiro/steering/AGENTS.md`
  all point at `resources/content/agents.md`. Skills:
  `~/.claude/skills` and `~/.kiro/steering/skills` point at
  `resources/content/skills`. **6 entries.**
- Phantom registry slots removed: `~/.claude/agents`,
  `~/.claude/commands`, and the corresponding empty source
  directories (`sources/agents/`, `sources/commands/`). If
  the user later writes a subagent or command, the
  directory + registry entry come back together.
- Functional tests (`resources/tests/`, was
  `tests/functional/`) run as part of this dev loop —
  one-off override against project.md's "agentic runs stop
  at smoke" rule, scoped to **this branch only** because
  the agents.md merge needs empirical proof that
  Claude Code and Kiro both load the merged file. Captured
  as a Decision Log entry below.
- 32 unit tests in build-tool stay green after the rename.
  Path-string assertions get updated for the new layout.
- README rewrite: top-level repo shape, one-line "what is
  resources/, what is kaimux/", Justfile verb table,
  cargo prerequisite, no `./install` mention.
- `specs/project.md` Tech Stack / Layout / Testing /
  Deployment sections updated for the new layout.
- Public-repo hygiene preserved.

## Test Strategy

Mapped onto project.md's Testing section. Deno-task names
gone; Justfile verbs stand in.

- **`cargo test --workspace`** (or `just test`) — 32 unit
  tests in `resources/build-tool/`. Load-bearing. Catches
  schema-validator / sources-walker / deploy-state-machine
  regressions before any push. Plus any new tests this
  branch adds for path migrations.
- **`resources/tests/run --no-tools`** (or `just
  test-smoke`) — structural smoke. Asserts the managed
  symlinks resolve in real `$HOME` after a deploy and each
  skill is reachable through them. Cheap, no API credits.
  Run after `just deploy` to confirm the symlinks landed at
  the new content paths.
- **`resources/tests/run`** (functional, full) — drives
  `claude --print` and `kiro-cli chat --no-interactive` with
  the `.smoke` fixtures. **Run as part of this dev loop
  (one-off override).** The merged `agents.md` needs
  empirical evidence that both tools still load skills
  correctly with the new tool-agnostic phrasing. Three
  candidate prose shapes for `agents.md` will be tested
  against the harness; the one that passes is the one we
  ship.
- **Quality Gate** — `just fmt` (cargo fmt --all --check),
  `just lint` (cargo clippy --workspace --all-targets --
  -D warnings).

Verification before merge: full `just test` green, `just
test-smoke` green, `just deploy` against my real `$HOME`
followed by a Claude Code session start that successfully
loads kdevkit + notes + writing-style skills.

## Design

### Final layout

```
mAId/
├── Cargo.toml                  [workspace] members = ["resources/build-tool"]
│                               kaimux added in its own feature
├── Cargo.lock
├── flake.nix / .envrc / rust-toolchain.toml
├── Justfile                    verb surface
├── README.md                   rewritten for the new shape
├── resources/
│   ├── build-tool/             Rust crate (workspace member)
│   │   ├── Cargo.toml          package = "build-tool"
│   │   └── src/
│   │       ├── main.rs         clap dispatch (4 verbs after xtask drop)
│   │       ├── deploy.rs       symlink state machine + 13 tests
│   │       ├── registry.rs     6 entries (4 instructions + 2 skills)
│   │       ├── schema.rs       4-check validator + 12 tests
│   │       └── sources.rs      walks resources/content/ + 7 tests
│   ├── content/
│   │   ├── agents.md           merged tool-agnostic preamble
│   │   └── skills/<name>/SKILL.md
│   └── tests/                  was tests/functional/ at repo root
│       ├── run                 bash harness (path updates only)
│       ├── conversational-stream.txt
│       └── skills/<name>.smoke
├── kaimux/                     EMPTY in this branch — placeholder
│                               for the agent-orch landing as a
│                               separate feature on this shape
├── dist/                       gitignored
├── target/                     gitignored
└── specs/                      unchanged at root
```

### Verb surface — Justfile

```just
# resources
deploy:        cargo run -p build-tool --release --quiet -- deploy
undeploy:      cargo run -p build-tool --release --quiet -- undeploy
status:        cargo run -p build-tool --release --quiet -- status
validate:      cargo run -p build-tool --release --quiet -- validate

# tests
test:          cargo test --workspace
test-smoke:    resources/tests/run --no-tools
test-functional: resources/tests/run

# quality
fmt:           cargo fmt --all
fmt-check:     cargo fmt --all --check
lint:          cargo clippy --workspace --all-targets -- -D warnings
check:         cargo check --workspace
```

The `cargo xtask` alias and `.cargo/config.toml` are deleted.
Native cargo + Justfile carry the load.

### Registry shape

```
home_subpath              source_subpath
─────────────────────────────────────────
.claude/CLAUDE.md         resources/content/agents.md
.claude/AGENTS.md         resources/content/agents.md
.claude/skills            resources/content/skills
.kiro/steering/KIRO.md    resources/content/agents.md
.kiro/steering/AGENTS.md  resources/content/agents.md
.kiro/steering/skills     resources/content/skills
```

Belt-and-suspenders: legacy CLAUDE.md/KIRO.md kept alongside
AGENTS.md for the transition window. Drop legacy after
Claude Code formally adds AGENTS.md as a default-read
location (today's guidance: `@AGENTS.md` import inside
CLAUDE.md, or symlink — we're going symlink).

### Merging CLAUDE.md and KIRO.md → agents.md

Today's two files differ in 4 lines: title, two paths
(`~/.claude/skills/` vs `~/.kiro/steering/`), one verb
(`skills:` vs `steering:`), and a Claude-only rule about
CLAUDE.md / agents/ / commands/. Three candidate shapes for
the merged `agents.md`, to be picked based on functional-
test outcomes:

- **(i) Tool-agnostic phrasing.** "Read your skills
  directory" instead of named paths. The agent knows its
  own paths. Single content block. Smallest, cleanest.
- **(ii) Per-tool sections.** "## When you're Claude Code
  ..." and "## When you're Kiro ...". Explicit but adds
  branching content the agent has to filter.
- **(iii) Skills-only content.** Drop the "writing to" /
  "managed paths" rules entirely from `agents.md` (move
  them to a human-facing `resources/content/README.md`).
  agents.md becomes purely "what skills are, how to load
  them."

The dev loop runs functional tests against (i) first; if
the kdevkit / notes / writing-style fixtures pass on both
Claude and Kiro, ship (i). If a fixture fails because
tool-agnostic phrasing left the agent without enough cue,
fall back to (ii). (iii) is the escape if both lose
behavior.

### What changes in the rust code

- `transform/` directory rename → `resources/build-tool/`.
- `transform/Cargo.toml`: `name = "transform"` →
  `name = "build-tool"`.
- All path strings in `main.rs` / `sources.rs` /
  `registry.rs`:
  - `sources/` → `resources/content/`
  - `tests/functional/run` → `resources/tests/run`
- `repo_root()` logic: `CARGO_MANIFEST_DIR` is
  `<root>/resources/build-tool/`; walk up two levels, not
  one.
- `REGISTRY` static drops `agents/` and `commands/` slots,
  adds AGENTS.md slots, points all instructions at
  `agents.md`.
- `cmd_install` and `cmd_uninstall` were already vestigial
  (umbrella verbs); drop both. Verbs after the cut:
  `validate`, `deploy`, `undeploy`, `status`. The build-tool
  binary shrinks ~150 LOC.
- Deploy result enum + tests track path-string changes.

### What gets deleted

- `transform/` (renamed, not deleted in place).
- `sources/agents/` and `sources/commands/` (empty
  directories).
- `sources/claude/CLAUDE.md` (merged into
  `resources/content/agents.md`).
- `sources/kiro/KIRO.md` (merged).
- `sources/skills/` (moved).
- `tests/functional/` (renamed).
- `.cargo/config.toml` (xtask alias gone).
- The `Cmd::Install` and `Cmd::Uninstall` clap subcommands
  + `cmd_install` / `cmd_uninstall` functions +
  `discover_members` helper.

## Implementation Plan

Ordered. Quality + Test gates green between each stage; Code
Review Gate fires once at the end.

1. **Spec landed.** This file. Push, but **do not wait for
   review** — user instruction.

2. **Rename `transform/` → `resources/build-tool/`.** Move
   the directory; update workspace `members` in root
   `Cargo.toml`; update `transform/Cargo.toml`'s `name`
   field to `build-tool`; verify `cargo check --workspace`.

3. **Move content + tests.**
   - `sources/skills/` → `resources/content/skills/`
   - `sources/claude/CLAUDE.md` → temp staging
   - `sources/kiro/KIRO.md` → temp staging
   - `tests/functional/*` → `resources/tests/*`
   - Empty `sources/{agents,commands}/` → `git rm`.
   - `sources/` itself disappears.

4. **Author merged `agents.md` (option (i)).** Plain
   tool-agnostic preamble at `resources/content/agents.md`.
   Drops Claude/Kiro-specific path naming.

5. **Update `registry.rs`.** 6 entries, AGENTS.md
   double-symlink, no agents/ or commands/.

6. **Update path strings throughout.** `main.rs` repo_root
   walk, `sources.rs` content paths, schema/deploy unchanged
   logically. Run `cargo test -p build-tool` — fix any
   path-string assertions that broke.

7. **Drop xtask alias.** Delete `.cargo/config.toml`.

8. **Drop `cmd_install` / `cmd_uninstall` /
   `discover_members`.** Remove `Cmd::Install` and
   `Cmd::Uninstall` clap variants. The build-tool binary
   loses ~150 LOC.

9. **Add Justfile.** Recipes per the verb table above.

10. **Verify quality gate.** `just fmt-check`, `just lint`,
    `just check`, `just test`. All green.

11. **End-to-end against fake $HOME.** `HOME=$(mktemp -d)
    just deploy` → `just status` → `just undeploy`. Confirm
    6 entries deploy, all resolve correctly.

12. **Functional test loop — agents.md option (i).**
    `just test-functional` against the real $HOME (after a
    real deploy). Subset to the 3 stable smokes
    (kdevkit.smoke, notes.smoke, writing-style.smoke) for
    the first round. If any fail, examine the failure: was
    it a content-loading problem or a content-comprehension
    problem?
    - **Content-loading**: agents.md isn't being read.
      Check the symlink, check the registry, fix.
    - **Content-comprehension**: the agent loaded agents.md
      but the tool-agnostic phrasing left a behavior gap.
      Fall back to option (ii) (per-tool sections), re-run.

13. **Update specs/project.md.** Tech Stack, Architecture,
    Layout, Testing, Deployment. Drops xtask, install,
    uninstall, ./install. Adds Justfile.

14. **README rewrite.** Top-level repo shape, two-halves
    intro, Justfile verb table, cargo prerequisite, the
    AGENTS.md standard noted.

15. **Final Quality + Test + Code Review Gate.** Push.
    Hand off to user for the Agent-dev Review Gate.

## Open questions

None blocking. Resolved during planning:

- **Build-tool naming** → `build-tool` (verb-prefixed,
  Bevy convention, dodges all reserved names). Confirmed.
- **CLAUDE.md / KIRO.md retention** → belt-and-suspenders
  symlinks during the transition.
- **agents.md content shape** → option (i) tool-agnostic;
  fall-back (ii) per-tool sections; (iii) skills-only as
  last resort. Empirical pick via functional tests.
- **xtask** → dropped entirely.
- **Justfile vs cargo-only** → Justfile (user opened the
  door with "ok to bring in just if that simplifies the
  commands").
- **Empty agents/ + commands/** → removed. Phantom slots
  go.
- **Cherry-pick provenance** → 3 code commits from
  feat/deno-to-rust (cbd5e9c, 465f6f2, 7fc87a3); deno-to-rust
  branch + PR #20 will be closed without merge once this
  ships.
- **Functional tests in dev loop** → one-off override
  scoped to this feature; permanent rule revisit at
  closure.
- **Sequencing** → restructure first; agent-orch as
  `kaimux/` lands in a follow-up feature; `feat/agent-orch-fix`
  branch rebases onto the new shape when its work is ready.

## Session Log

- 2026-06-11 · spec drafted in one pass per user
  instruction "don't wait on me for plan review." Cherry-
  picked the 3 deno-to-rust code commits (cbd5e9c, 465f6f2,
  7fc87a3) onto a fresh branch from main. The deno-to-rust
  spec file landed via cherry-pick and was then `git rm`'d
  (this branch's spec supersedes it).

## Decision Log

- **Restructure shape: `resources/` + `kaimux/`.** Top-level
  folders match the project's two halves — markdown
  resources for the AI tools to load, and apps that provide
  functionality. The build-tool is co-located with the
  content it operates on (`resources/build-tool/`) rather
  than at the workspace root, mirroring the Bevy
  `generate-*` convention. Rejected: keeping today's flat
  shape (transform sat at root pretending to be a peer of
  sources, but wasn't); shipping the build crate at root
  with a generic xtask name (functional naming wins —
  matches kaimux's pattern).

- **Adopt AGENTS.md as the canonical instruction file.**
  Web research: Linux Foundation Agentic AI Foundation
  governance, 97k+ root-level AGENTS.md files on GitHub,
  native support across Codex / Copilot / Cursor / Kiro /
  Zed / Windsurf / Amp. Claude Code's official guidance is
  symlink or `@import`. Belt-and-suspenders symlinks during
  the transition (CLAUDE.md, KIRO.md, AGENTS.md all point
  at the same source) eliminate any tool-detection branch
  and let us drop the legacy filenames whenever Claude Code
  formally adopts AGENTS.md.

- **Drop empty `agents/` and `commands/` registry slots.**
  Phantom slots — never had content. Today's behavior:
  registry deploys symlinks to empty source directories,
  both AI tools see empty subagent / command lists. If
  either kind of content shows up later, the directory +
  registry entry come back together. Registry stays the
  single source of truth.

- **Drop the `xtask` cargo alias.** With Justfile naming
  the verbs and `cargo run -p build-tool --release --quiet
  -- <verb>` doing the work, the alias is one indirection
  too many. The user's framing: "everything we are doing
  is done using cargo itself." Rejected: keeping xtask
  for muscle-memory continuity (the verbs are renaming
  anyway with the rename of `transform` → `build-tool`,
  so muscle memory is breaking either way).

- **Drop `cmd_install` and `cmd_uninstall` from build-tool.**
  Both were umbrellas — `install` did validate+deploy+build+
  copy; `uninstall` aliased to undeploy. After the
  restructure, building apps is `cargo build -p kaimux
  --release && cp target/release/kaimux dist/` — there's
  nothing for an umbrella to bundle. The build-tool's job
  is symlink management; that's the four verbs that
  remain.

- **Functional tests run in the dev loop for this branch
  only.** project.md's existing rule says agentic runs
  stop at smoke (judge-mode functional costs API credits
  and takes minutes; spending that budget is a human
  call). For this feature, the agents.md merge needs
  empirical evidence that the merged file works in both
  Claude Code and Kiro — substring smokes alone can't
  prove the AI tool actually loaded the right content.
  One-off override, not a permanent rule change. Revisit
  at §8 closure.

- **Cherry-pick provenance, not merge.** 3 commits from
  feat/deno-to-rust replicate as new commits on this
  branch; PR #20 closes without merge once this ships.
  Cleaner final history (one feature lands as one squash-
  commit on main) at the cost of two abandoned commits in
  reflog. Rejected: waiting for deno-to-rust to merge
  first then branching from it (forces a wait; the
  restructure is non-trivially different from the
  deno-to-rust shape and rebasing on top adds noise).
