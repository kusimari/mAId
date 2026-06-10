# mAId

Tool-agnostic source of truth for agentic resources — skills,
agents, commands, steering docs — compiled into whatever AI tool
happens to be in use (Claude Code, Kiro, Gemini CLI, future
tools).

The repo is the checked-in source. Installing mAId creates
symlinks from `$HOME` into this tree, so edits to `sources/`
are live for the next AI session. All work on mAId itself
goes through `cargo xtask <verb>` (or native cargo verbs) —
there is no separate binary on `PATH`.

## Develop

```
direnv allow                              # loads rust toolchain via the repo-local flake
cargo test --workspace                    # full unit suite
cargo fmt --all                           # format
cargo clippy --workspace --all-targets    # lint
cargo check --workspace                   # typecheck
```

Without direnv, prefix every command with `nix develop
--command` (e.g. `nix develop --command cargo test
--workspace`). Cargo is a hard prerequisite — there is no
bootstrap shim.

The development methodology (spec-driven, phase-gated) is encoded
in the [`kdevkit` skill](./sources/skills/kdevkit/SKILL.md).
Project context lives in [`specs/project.md`](./specs/project.md);
feature specs live in [`specs/feature/`](./specs/feature/);
open future work sits in [`specs/backlog/`](./specs/backlog/).

## Install

```
cargo xtask install     # validate + deploy + build any sources/<name>/ Rust crates
cargo xtask uninstall   # undeploy
```

What happens on install:

1. The rust toolchain picks up from the repo-local flake
   (direnv active) or via `nix develop --command` if nix is
   present and direnv isn't.
2. `cargo xtask validate` walks `sources/` and runs the
   simplified frontmatter validator.
3. `cargo xtask deploy` symlinks every entry in
   [`transform/src/registry.rs`](./transform/src/registry.rs) —
   today `~/.claude/CLAUDE.md`, `~/.claude/skills`,
   `~/.claude/agents`, `~/.claude/commands`,
   `~/.kiro/steering/KIRO.md`, and `~/.kiro/steering/skills`,
   each pointing into [`sources/`](./sources/).
4. Any Rust workspace members under `sources/<name>/` are
   built in release mode and copied to `dist/<name>` (e.g.
   `sources/agent-orch/` → `dist/agent-orch`).

Uninstall is idempotent. Hand-written files at a managed
destination are preserved unless you pass `--force`.

## Where to look next

- Everything that gets deployed: [`sources/`](./sources/).
- How deployment is decided:
  [`transform/src/registry.rs`](./transform/src/registry.rs).
- Reference shape for a new skill:
  [`sources/skills/kdevkit/SKILL.md`](./sources/skills/kdevkit/SKILL.md)
  (live siblings: [`notes/`](./sources/skills/notes/SKILL.md),
  [`writing-style/`](./sources/skills/writing-style/SKILL.md)).
- Full verb list: `cargo xtask --help` or
  [`transform/src/main.rs`](./transform/src/main.rs).
