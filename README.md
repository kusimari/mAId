# mAId

Tool-agnostic source of truth for agentic resources — skills
and the AGENTS.md preamble — compiled into whatever AI tool
happens to be in use (Claude Code, Kiro, Cursor, Codex,
Copilot, Zed, Windsurf, future tools).

The repo has two halves:

- **`resources/`** — markdown content the AI tools load,
  plus the build crate that operates on it.
- **`kaimux/`** — a sibling workspace member for the
  kaimux app (placeholder; lands with code in its own
  feature).

Installing mAId creates symlinks from `$HOME` into this
tree so edits are live for the next AI session.

## Develop

The repo-local flake provides `cargo` and `just`:

```
direnv allow              # loads the flake on shell entry
just                      # lists every recipe
```

Without direnv: `nix develop` once per shell (or prefix
`nix develop --command` to each command). Cargo + just are
hard prerequisites; there's no bootstrap shim.

Common verbs:

```
just test          # workspace unit tests
just fmt-check     # rustfmt
just lint          # clippy --workspace -D warnings
just check         # cargo check --workspace
just ci            # full quality + test gate
```

The development methodology (spec-driven, phase-gated) is
encoded in the
[`kdevkit` skill](./resources/content/skills/kdevkit/SKILL.md).
Project context: [`specs/project.md`](./specs/project.md).
Feature specs: [`specs/feature/`](./specs/feature/).

## Install

```
just deploy             # validate + create $HOME-facing symlinks
just status             # report current symlink state
just undeploy           # remove deploy-managed symlinks
```

What `just deploy` does:

1. `just validate` walks `resources/content/` and runs the
   simplified frontmatter validator.
2. `just deploy` symlinks every entry in
   [`resources/build-tool/src/registry.rs`](./resources/build-tool/src/registry.rs)
   — today the merged AGENTS.md preamble (deployed as
   `~/.claude/CLAUDE.md`, `~/.claude/AGENTS.md`,
   `~/.kiro/steering/KIRO.md`, and
   `~/.kiro/steering/AGENTS.md` for cross-tool support)
   plus the skills tree (`~/.claude/skills` and
   `~/.kiro/steering/skills`).

Undeploy is idempotent. Hand-written files at a managed
destination are preserved unless you pass `--force`.

mAId follows the cross-tool **AGENTS.md** standard
(Linux Foundation Agentic AI Foundation; native support
across Codex, Copilot, Cursor, Kiro, Zed, Windsurf). The
legacy `CLAUDE.md` and `KIRO.md` filenames symlink at the
same source — drop them when Claude Code makes AGENTS.md a
default-read location.

## Where to look next

- Everything that gets deployed:
  [`resources/content/`](./resources/content/).
- How deployment is decided:
  [`resources/build-tool/src/registry.rs`](./resources/build-tool/src/registry.rs).
- Reference shape for a new skill:
  [`resources/content/skills/kdevkit/SKILL.md`](./resources/content/skills/kdevkit/SKILL.md)
  (live siblings:
  [`notes/`](./resources/content/skills/notes/SKILL.md),
  [`writing-style/`](./resources/content/skills/writing-style/SKILL.md)).
- Full verb list: `just --list`.
