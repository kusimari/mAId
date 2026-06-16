---
name: kaimux-functional-tests-in-rust
description: Consider rewriting kaimux/tests/functional-test.sh as a Rust harness so assertions are typed and helpers are sharable with unit tests.
metadata:
  type: backlog
---

# Kaimux functional tests in Rust

## What

Move `kaimux/tests/functional-test.sh` (~550 LOC bash, F1–F8
scenarios driving real claude/kiro-cli on the user's tmux
server) to a Rust harness behind `cargo test --features
functional` (or a `tests/functional.rs` integration test).

## Why now / why not now

The bash form works today and is cheap to poke when adding a
scenario. The case for rewriting:

- **Typed assertions on the multi-line render shape.** Today
  the F-blocks parse render output with `awk`/`grep`; a Rust
  harness can deserialise once and assert on fields.
- **Shared helpers with unit tests.** `mk(...)`, `display_state`,
  `priority` already live in `kaimux::*`; a Rust harness can
  use them directly instead of mirroring the rules in bash.
- **One language to read.** Bash + jq + tmux makes triage
  harder than it needs to be when something flakes.

The case against rewriting *now*:

- Tests are user-driven (cost API credits, run by hand, not
  in CI). Bash being hackable in the moment matters more than
  type-safety.
- ~600 LOC of new Rust + a rebuild loop on every change.
- Bash doesn't bite hard enough yet.

## Triggers to revisit

- A scenario flakes and the bash assertion makes triage hard.
- We need to share non-trivial logic between unit + functional
  tests.
- A new contributor friction-points on `jq`/bash to add F9.

## Provenance

Surfaced on PR #24 in the
[`functional-test.sh:1` review thread](https://github.com/kusimari/mAId/pull/24#discussion_r3415854364).
The agent committed an F1–F8 scenario index in `f701699` and
deferred the Rust rewrite pending a real friction point.
