---
name: build-tool-language-rust-vs-python
description: Evaluated moving resources/build-tool from Rust to typed, functional Python (toolz) to avoid the compile-before-install step. Verdict — keep it in Rust; the compile cost is amortized and cheaply mitigated, while a port permanently forks the single-workspace toolchain and weakens safety on a $HOME-mutating tool. kaimux also stays Rust. Parked with revisit triggers, not a port task.
metadata:
  type: backlog
---

# build-tool language: Rust vs. typed functional Python

## What was evaluated

Whether `resources/build-tool` (the symlink installer) should move
from Rust to **typed Python written functionally** (e.g. `toolz`),
motivated by: Rust adds a *compile-the-build-tooling-before-you-can-
install* step; bash is too complex to script this safely;
TypeScript would fit but Python is the language AI agents are most
fluent in, and mAId is a repo *for* AI. The port of `kaimux` was
considered alongside (could it also be Python? should it stay
Rust?).

## Verdict

**Keep `build-tool` in Rust. Do not port. Keep `kaimux` in Rust
too.** This is a decision record, not a port task — filed so the
question isn't re-litigated without new signal.

## What the tool actually is (grounds the decision)

`resources/build-tool/src/main.rs` — single file, ~600 LOC of
logic + 44 unit tests. It is a **filesystem symlink state machine**:
validate each skill's YAML frontmatter, then plan → install →
uninstall → status of `$HOME`-facing symlinks, with dry-run, force,
and codex fan-out semantics. Deps: `clap`, `serde`, `gray_matter`,
`anyhow`. Invoked as `cargo run -p build-tool --release` from the
`resources::*` Just verbs. It is a **member of the same cargo
workspace as `kaimux`**.

Two properties matter most: it **mutates the user's `$HOME`**
(including `--force`, which can replace files), and it **changes
rarely** (stable logic, comprehensive test suite).

## The motivating cost is real but amortized

"Rust compiles before it can install" is true, but cargo caches
builds: only a **cold checkout** and **edits to the tool itself**
pay a compile; every other `cargo run` is a sub-second freshness
check. Because the tool changes rarely, edit-recompile almost never
fires. The entire real cost is a one-time-per-machine cold build of
the dep tree (serde + clap derive macros dominate it). That is a
small, one-off cost — and cheaply mitigated without a port (below).

## Why Python loses here (four counts)

1. **Safety on a `$HOME`-mutating tool.** A symlink state machine
   with force/dry-run/fan-out is exactly where Rust's exhaustive
   `match` (over `Kind` / `Comparison`), `Result` propagation, and
   absence of null prevent the costliest bug class — clobbering the
   user's real files. Typed Python recovers only part of this, and
   only if mypy/pyright is *run* — itself a compile-analogue check
   step, reintroducing the step the port set out to remove.
2. **toolz buys little.** Functional utilities shine on data
   pipelines. The tool's pure planning phase (`REGISTRY → filter by
   agent → expand → plan`) is *already* expressed functionally via
   Rust iterator chains; the other ~60% is inherently effectful IO
   (`symlink`, `readlink`, `remove_file`, `ensure_parent`) that
   `toolz` can't prettify. Net legibility gain ≈ zero.
3. **Single-workspace coherence (decisive).** The repo is one cargo
   workspace; `cargo build --workspace` / `just test` / `just ci`
   cover build-tool *and* kaimux with one toolchain, one test
   runner, one fmt/lint/CI path — a property `project.md`
   Architecture explicitly prizes ("cargo build --workspace covers
   everything"). Porting one member to Python permanently forks the
   toolchain: rust + python + node in the flake; cargo + pytest in
   `just test`; clippy + ruff; rustfmt + black. That cost is paid on
   every CI run and every contributor/agent setup — to save an
   amortized one-time compile.
4. **Distribution.** The flake already ships the Rust toolchain and
   cargo vendors deps via `Cargo.lock`; a Python port needs the
   flake to also provide a pinned interpreter + deps closure
   (`toolz`, a frontmatter parser, a CLI lib). More flake surface to
   maintain, no offsetting gain.

## The strongest pro-Python argument, and why it doesn't carry

**"mAId is for AI, and agents are most fluent in Python."** Real,
and the best case for the port. But it matters most for *hot* code
an agent edits every session. The build-tool is stable
infrastructure that changes rarely — agent-edit-friction is not its
dominant cost. When that stops being true (see triggers), the
argument regains force.

## kaimux stays Rust (emphatically)

kaimux is a long-running process that wraps agent CLIs, watches
files (`notify`), file-locks (`fd-lock`), makes syscalls (`nix`),
and manages tmux panes with JSON state. That is systems territory
where a Python port is strictly worse: per-invocation interpreter
startup for a frequently-launched wrapper, GIL on the watch loop,
and daemon-packaging pain. No revisit trigger contemplated.

## Cheaper mitigations for the actual pain (do these instead, if it bites)

All stay in Rust and cost far less than a port:

- **Precompile to `dist/` like kaimux does.** Add a
  `just resources::build` that does `cargo build -p build-tool
  --release` + copies the binary to `dist/`, and have the install
  verbs invoke the built binary directly. First build is explicit
  and one-off; installs afterward skip cargo entirely.
- **Document the cold-build expectation.** One line in the README
  install section: first `install-skills` on a fresh checkout
  compiles the tool (tens of seconds); subsequent runs are instant.
- **Cache `target/` in CI** if CI cold-build time is the real
  complaint.

## Trigger to promote (revisit the port only if)

- **kaimux leaves the workspace** (spun out to its own repo). Then
  build-tool is the *sole* Rust member, the single-workspace
  coherence argument collapses, and "why Rust for one symlink tool
  an agent might want to edit?" becomes fair — re-evaluate then.
- **The build-tool starts changing every session** (orchestration
  logic grows into it) and agent-edit-friction becomes its dominant
  cost, outweighing the safety + coherence wins.
- A **third workspace member arrives that is naturally Python**
  (not Rust), making the flake already multi-runtime — at which
  point the marginal cost of Python for build-tool drops.

Absent one of these, the answer is Rust.
