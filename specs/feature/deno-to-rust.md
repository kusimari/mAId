# Feature: deno-to-rust

## Git Setup

- Branch: feat/deno-to-rust
- Base: main (2c27ef8)

## Feature Brief

Replace mAId's Deno+TypeScript build/CLI surface with a Cargo
workspace using xtask for build automation. The `maid/` crate
goes away — its 552 LOC of validate/deploy logic moves into
`xtask/`, a workspace member specifically for build glue.
`sources/agent-orch/` (already Rust on the sister branch)
becomes the second workspace member, getting `cargo build -p`
and `cargo test -p` for free. Native cargo verbs replace the
old `deno task fmt/lint/check/test`; the four custom verbs
that aren't cargo-native — `validate`, `deploy`, `undeploy`,
`status` — become `cargo xtask <verb>`. `./install` becomes
a thin passthrough into `cargo xtask install`. Net result:
one toolchain (Rust), no extra build-system deps (`pkgs.deno`
dropped, `pkgs.just` not added), and a workspace shape that
lets future Rust tools drop in as new sub-packages.

## Requirements

- Every user-facing verb from today's `deno task` surface has
  a cargo-native or `cargo xtask` equivalent. Output shape is
  close enough that a human reading the output still parses
  it the same way.
- `./install` and `./install --uninstall` keep their public
  signature. What they invoke underneath shifts to
  `cargo xtask install` / `cargo xtask uninstall`.
- The 20 existing unit tests (11 deploy + 9 schema/walk) port
  to `cargo test`. Every behavior the TS suite asserts has a
  Rust counterpart asserting the same outcome.
- The frontmatter validator stays but simplifies to four
  checks: header start (`---\n`), header end (closing `---`),
  `name:` present, `description:` present. The `version:` and
  `tags:` type checks drop — they never caught a real failure.
- The functional smoke harness (`tests/functional/run`) keeps
  working unchanged. Bash code and fixture format stay as-is.
- `flake.nix` carries `rust-overlay` only — `pkgs.deno` removed,
  no `pkgs.just` added.
- Workspace verbs: `cargo build --workspace` builds every
  member; `cargo build -p <name>` builds one. Same shape for
  `test`, `clippy`, `check`.
- `dist/` is gitignored and holds binaries that survive the
  build (`dist/agent-orch`). The xtask binary itself is *not*
  copied to `dist/` — it's a build tool, only invoked via
  `cargo xtask`.
- `Cargo.lock` is committed at the workspace root (binary-
  workspace policy; matches agent-orch sister branch's same
  decision). `target/` is gitignored.
- Public-repo hygiene preserved: no internal references in any
  spec, code, commit message, or CR/PR body.

## Test Strategy

Mapped onto `project.md`'s Testing section, with the layer
substitutions noted:

- **Unit tests** — `cargo test --workspace` replaces
  `deno task test:unit`. Ported tests live as
  `#[cfg(test)] mod tests` blocks in
  `xtask/src/{deploy,schema,sources}.rs`. Success criterion:
  every behavior asserted by the 20 TS tests has a Rust
  counterpart that asserts the same outcome (status enum
  variant, symlink target, error case). Load-bearing.
- **Smoke tests** — `tests/functional/run --no-tools` keeps
  working unchanged. New invocation surface: `cargo xtask
  test-smoke` shells into the existing bash. Success
  criterion: structural smoke passes against a fresh
  `cargo xtask install` deploy.
- **Functional tests** — `tests/functional/run` keeps working
  unchanged. New invocation surface: `cargo xtask
  test-functional`. Same user-driven contract from
  `project.md` — agentic runs stop at smoke.
- **Quality Gate** — `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo check --workspace`. Three native verbs replace
  `deno task fmt/lint/check`.

Verification before merge: `cargo test --workspace` green +
`cargo xtask install` against the user's real `$HOME`,
followed by `cargo xtask test-smoke` to confirm the symlinks
resolve, then `cargo xtask uninstall` to leave the host
clean for the next iteration if needed.

## Design

### Workspace shape

```
mAId/
├── Cargo.toml              workspace root: members = ["xtask"]
├── .cargo/config.toml      [alias] xtask = "run -p xtask --release --"
├── flake.nix               rust-only via rust-overlay
├── rust-toolchain.toml     stable + clippy + rustfmt
├── .gitignore              + target/, + dist/
├── install                 3-line passthrough → cargo xtask install
├── README.md               Develop section rewritten for cargo verbs
├── xtask/
│   ├── Cargo.toml          deps: clap, anyhow, duct, shell-words; dev: tempfile
│   └── src/
│       ├── main.rs         CLI dispatch (clap)
│       ├── sh.rs           ~10 LOC sh!() helper over duct + shell-words
│       ├── deploy.rs       symlink state machine (port of deploy.ts)
│       ├── registry.rs     static REGISTRY: &[Entry] (port of registry.ts)
│       ├── schema.rs       simplified frontmatter validator
│       └── sources.rs      walk sources/{skills,agents,commands}/
├── sources/                ← content (skills/, agents/, commands/, claude/, kiro/)
│                              plus future Rust workspace members (e.g. agent-orch
│                              when its sister branch lands — see "Merge-order
│                              independence" below)
├── tests/
│   └── functional/         ← unchanged bash harness
├── specs/                  unchanged
└── dist/                   gitignored — built binaries land here
```

### `cargo xtask` verb surface

| Verb | What it does | Equivalent today |
|---|---|---|
| `cargo xtask validate` | walk sources/, validate frontmatter | `deno task validate` |
| `cargo xtask deploy` | validate, then symlink per registry | `deno task deploy` |
| `cargo xtask undeploy` | remove managed symlinks | `deno task undeploy` |
| `cargo xtask status` | report each managed symlink state | `deno task status` |
| `cargo xtask install` | build agent-orch, copy to dist/, then deploy | `deno task setup` |
| `cargo xtask uninstall` | undeploy | `deno task teardown` |
| `cargo xtask test-smoke` | shell into tests/functional/run --no-tools | `deno task test:smoke` |
| `cargo xtask test-functional` | shell into tests/functional/run | `deno task test:functional` |

Native cargo handles the rest — `cargo build`, `cargo test`,
`cargo check`, `cargo fmt`, `cargo clippy`.

### Orchestration: duct + shell-words

`xtask/src/sh.rs` exposes a tiny `sh!(...)` macro that parses
a shell-style string at runtime via `shell_words::split` and
wraps the result in `duct::cmd`. Used wherever xtask shells
out to another program (`cargo build`, `cp`,
`tests/functional/run`).

The symlink state machine in `deploy.rs` does **not** use
`sh!` — it uses plain `std::fs` so the outcome can be a
typed enum and the branches are exhaustive `match`.

duct (1.x, last release Nov 2025) and shell-words (1.1.x,
last release Dec 2025) are both actively maintained;
xshell was rejected because its last release was Oct 2023.

### Symlink state machine

Direct port of `deploy.ts`. `DeployStatus` enum encodes the
six outcomes (`Created`, `AlreadyOk`, `Replaced`,
`SkippedWrongSymlink { current }`, `SkippedNonSymlink { kind }`,
`SkippedMissingSource`); `match fs::symlink_metadata(&target)`
drives the branches. `UndeployStatus` is the symmetric enum
for undeploy. Tests assert by enum variant rather than parsed
strings.

### Frontmatter validator (simplified)

Four checks:

1. File begins with `---\n`.
2. A closing `---` exists later.
3. Between them, a `name:` line is present and non-empty.
4. A `description:` line is present and non-empty.

Drops the YAML-flow-array parser path and the `version`/`tags`
type checks. Net: ~30 LOC of Rust replaces ~109 LOC of TS.

### `flake.nix` rewrite

Borrowed from agent-orch's sister flake, Deno removed:

```nix
buildInputs = [ rustToolchain pkgs.pkg-config ];
```

`rust-overlay` provides the toolchain. `pkg-config` covers
crates that need it (none today, kept as cheap insurance).

### `./install` shape

Three-line passthrough, same as today, with the inner verb
shifted from `deno task` to `cargo xtask`:

```bash
if command -v cargo >/dev/null 2>&1; then
    exec cargo xtask "$verb"
elif command -v nix >/dev/null 2>&1; then
    exec nix develop --command cargo xtask "$verb"
else
    echo "ERROR: need cargo (or nix, for the repo-local flake)." >&2
    exit 1
fi
```

### Merge-order independence

This branch and `feat/agent-orch-fix` (which adds
`sources/agent-orch/` as a Rust crate) are developed in
parallel. Neither blocks the other; whichever merges first
wins and the other rebases. To keep that possible, **this
branch's `Cargo.toml` lists only `xtask` as a workspace
member** — it does not assume `sources/agent-orch/` exists.

Three concrete rules that make the parallelism work:

1. **Workspace members list = `["xtask"]` at merge time.**
   Adding a new member is the *second* branch's responsibility,
   in its rebase. This holds for any future Rust crate landing
   in `sources/`, not just agent-orch.

2. **`cargo xtask install` builds members conditionally.**
   The install verb walks `sources/<name>/Cargo.toml` and
   builds each present member with `cargo build -p <name>
   --release`, then copies `target/release/<name>` to
   `dist/<name>`. Empty `sources/` (no Rust members) → install
   only runs validate + deploy, no `cargo build` step. This
   way the verb works the day this branch merges (no Rust
   crates yet) *and* the day after agent-orch's branch lands
   (one Rust crate to build).

3. **No assumptions about other branches' deno.json.** The
   sister branch's `feat/agent-orch-fix` carries 4 deno-task
   wrappers (`agent-orch:build`/`test`/`check`/`integration`)
   in its `deno.json`. **This branch deletes `deno.json`
   entirely** — those wrappers vanish on merge regardless of
   sequence. Whoever lands second handles the conflict in
   their rebase: if agent-orch lands first, this branch's
   `deno.json` deletion already includes those wrappers; if
   this lands first, agent-orch's rebase replaces those
   wrappers with cargo-native invocations.

Conflict surface for either rebase order is bounded:
`Cargo.toml` (workspace members), `flake.nix` (build inputs),
`deno.json` / `deno.lock` (deleted by this branch). Both
branches touch all three. No source code conflict is expected.

### Trade-offs taken

- **xtask, not Just.** Everything stays in cargo; no extra
  flake dep. Drove by the dev-verb count being small (~8
  verbs that aren't cargo-native — and 4 of those that *are*).
- **duct + shell-words, not xshell.** xshell is stale; the
  duct/shell-words pair released through Dec 2025. A ~10-LOC
  `sh!()` helper restores xshell's string-literal ergonomics.
- **Validator simplified, not removed.** "kiro-cli still does
  not function" despite valid frontmatter — defense-in-depth
  wasn't real. Keep the cheap checks, drop the type-pedantry.
- **Workspace, not multi-repo.** Future Rust tools drop in as
  workspace members. `cargo xtask install` builds every
  present member's release binary into `dist/`.
- **Workspace members list stays minimal at merge time.**
  `members = ["xtask"]` only. Other Rust crates join via
  their own merging branch's rebase, not this one. Keeps
  this branch merge-order-independent vs. any other in-flight
  feature that adds a Rust crate.

## Implementation Plan

Ordered. Each step ends with the §7 Quality + Test gates green
before the next begins. The §7 Code Review Gate fires once at
the end of the implementation rather than per-step — the
steps are tightly coupled (a half-ported state machine isn't
reviewable on its own).

1. **Workspace scaffold.** Create root `Cargo.toml` with
   `[workspace]` and `members = ["xtask"]` (agent-orch
   joins later — see Open Questions). Create
   `.cargo/config.toml` with the xtask alias. Create
   `rust-toolchain.toml`. Update `flake.nix` (drop deno, add
   rust-overlay). Update `.gitignore` (+ `target/`, +
   `dist/`). Verify: `cargo check --workspace` against an
   empty xtask. **Risk:** rust-overlay flake input drift —
   pin to the version agent-orch's flake uses.

2. **Create xtask crate skeleton.** `xtask/Cargo.toml` with
   `clap`, `anyhow`, `duct`, `shell-words`. `xtask/src/main.rs`
   with clap dispatch over the eight verbs, each stubbed with
   `todo!()`. Verify: `cargo xtask --help` lists all verbs.

3. **Port `registry.rs`.** Static `&[Entry]` array matching
   today's six entries. No tests yet (data only).

4. **Port `schema.rs` (simplified) + tests.** Four checks
   only. Keep file:line error tracking. Tests assert: header
   start, header end, name present, description present.
   Verify: `cargo test -p xtask schema` passes.

5. **Port `sources.rs` + tests.** Walker over
   `sources/{skills,agents,commands}/`. Sorted by (kind,
   name). Schema errors collected, not thrown one-shot.
   Verify: tests pass.

6. **Port `deploy.rs` + tests (the largest unit).** Both
   `deploy` and `undeploy` state machines. `DeployStatus` and
   `UndeployStatus` enums. All 11 deploy tests ported. Verify:
   tests pass. **Risk:** symlink-target-string edge cases —
   `Path::join` normalizes differently than today's
   `${a}/${b}` concat. Tests will surface this; if they do,
   match TS behavior (literal join), not Rust idiom.

7. **Wire CLI dispatch in `main.rs`.** Each clap verb calls
   the right module function and prints results. Output
   shape close enough to today's TS that a human reads it
   the same.

8. **Add `install` and `uninstall` verbs.** `install` =
   `validate` then `deploy`, plus a member-discovery step:
   walk `sources/*/Cargo.toml`, and for each present member,
   `sh!("cargo build -p <name> --release")` then copy
   `target/release/<name>` to `dist/<name>`. With no Rust
   members (the day this branch merges, before agent-orch
   lands), the build/copy loop is empty and install only
   does validate+deploy. `uninstall` = `undeploy`.

9. **Add `test-smoke` and `test-functional` verbs.** `sh!()`
   wraps the existing `tests/functional/run` script.

10. **Rewrite `./install`.** Three-line passthrough: pick
    `cargo` if on PATH, else `nix develop --command cargo`.
    Drop the deno fallback path.

11. **Delete deno surface.** Remove `maid/`, `deno.json`,
    `deno.lock`. Remove deno-only fields from `flake.nix`
    if any remain.

12. **Update `specs/project.md`.** Rewrite Tech Stack,
    Testing, Deployment sections to reflect the new dev
    loop. Update Layout. Land in the same dev-loop slice as
    the code change so `project.md` and the code never
    disagree at HEAD.

13. **Update `README.md`.** Develop section rewritten for
    cargo verbs.

14. **Quality + Test + Code Review Gate.** Run the full
    suite. Push.

15. **Closure.** Per §8 — reconcile in-flight markers, soft
    `project.md` verify, ask backlog cleanup, squash-merge.

## Open questions

None blocking. Resolved during planning:

- **agent-orch sequencing** → resolved as parallel
  development with merge-order independence. See the
  "Merge-order independence" section under Design.

- **Output-shape compatibility** → resolved as "close enough
  that a human reads it the same." Byte-for-byte match not
  required; no external script parses today's output.

- **`maid` as a name on `dist/`** → resolved as no `maid`
  binary under this design. xtask is build-only, never
  installed. The repo's *project name* stays mAId; nothing
  in `dist/` is called maid.

- **`Cargo.lock` policy** → committed at the workspace root
  (binary-workspace policy; matches agent-orch sister
  branch).

## Session Log

- 2026-06-10 · feature spec drafted after a long evaluation
  thread that walked through three shapes (cargo-only / cargo
  + Just / cargo + xtask), then re-evaluated whether
  validate/deploy needed code at all (yes — TOML can hold
  the registry but not the state machine), then picked the
  orchestration library (xshell stale, dax wrong direction,
  duct + shell-words actively maintained). Final shape is
  cargo workspace + xtask + duct + shell-words.

## Decision Log

- **Parallel branches, merge-order-independent.** This branch
  and `feat/agent-orch-fix` develop in parallel. Workspace
  members list stays at `["xtask"]` so neither branch blocks
  the other; the second-merging branch adds its crate to the
  workspace as part of its rebase. `cargo xtask install`
  discovers Rust members at runtime, so the install verb
  works in both pre-sister and post-sister states. Rejected:
  sister-first sequencing (forces a wait), co-merge as one
  CR (large diff, two unrelated concerns).

- **Cargo workspace + xtask, not maid/ as a Rust crate.** The
  user's reframe surfaced that `maid/` was only a CLI in a
  Deno-shaped world. Once the repo is Rust, the validate /
  deploy logic is build glue, not a user-facing tool — xtask
  is the cargo-native pattern for that. Rejected: keeping a
  user-facing maid binary on PATH (project.md's "no installed
  binary" constraint), Just shims (extra flake dep without a
  proportional ergonomic win at 8 verbs).

- **duct + shell-words helper, not xshell.** xshell hasn't
  released since Oct 2023; the author moved to dax (a Deno
  tool — wrong direction for a Rust-only stance). duct
  shipped Nov 2025; shell-words shipped Dec 2025; both have
  high download counts. A ~10-LOC `sh!()` helper restores
  xshell's string-literal ergonomics on top of the active
  pair. Rejected: xshell (stale), cmd_lib (token-tree syntax
  doesn't match the readability target), plain
  `std::process::Command` (verbose for the orchestration
  layer, though still used inside the symlink state machine).

- **Frontmatter validator simplified, not removed.** User
  feedback: defense-in-depth wasn't real ("kiro-cli still
  does not function" despite valid frontmatter). Keep the
  four checks that matter (header bounds, name present,
  description present); drop type-pedantry on `version` and
  `tags`. Rejected: full removal (functional smoke alone
  wouldn't catch a malformed `name:` until tool-load time,
  and even the cheap checks cost ~30 LOC).

- **Plain `std::fs` for the symlink state machine, not duct.**
  duct/shell-words is for orchestration — running other
  programs. The state machine is `match` over filesystem
  state, where exhaustive enums and structured errors are
  the win Rust gives us. Mixing the two would be wrong.
