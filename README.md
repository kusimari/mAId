# mAId

Tool-agnostic source of truth for agentic resources — skills
and the AGENTS.md preamble — compiled into whatever AI tool
happens to be in use (Claude Code, Kiro, Cursor, Codex,
Copilot, Zed, Windsurf, future tools).

The repo has two halves:

- **`resources/`** — three layers in one directory:
  the markdown content (`resources/content/`) the AI tools
  load; the Rust + bash tooling (`resources/build-tool/`,
  `resources/tests/run`) that installs and tests it; and
  the Justfile verbs (`resources::install`,
  `resources::uninstall`, `resources::status`,
  `resources::verify`) that drive the tooling.
- **`kaimux/`** — sibling workspace member for the kaimux
  tmux-pane orchestrator. Built via `kaimux::build`.

`just resources::install` creates symlinks from `$HOME`
into the content tree so edits are live for the next AI
session.

## Develop

The repo-local flake provides `cargo` and `just`:

```
direnv allow              # loads the flake on shell entry
just                      # lists every recipe
```

Without direnv: `nix develop` once per shell (or prefix
`nix develop --command` to each command). Cargo + just are
hard prerequisites; there's no bootstrap shim.

The development methodology (spec-driven, phase-gated) is
encoded in the
[`kdevkit` skill](./resources/content/skills/kdevkit/SKILL.md).
Project context: [`specs/project.md`](./specs/project.md).
Feature specs: [`specs/feature/`](./specs/feature/).

## Verbs

Three groups, namespaced by what they touch:

**`resources::*`** — operate on `$HOME` or the AI tools:

```
just resources::install     # validate content + create $HOME-facing symlinks
just resources::uninstall   # remove install-managed symlinks
just resources::status      # report current symlink state
just resources::verify      # drive `claude --print` against installed content (costs API credits, gated)
just resources::verify-one <name>   # single fixture
```

**`kaimux::*`** — operate on the kaimux crate:

```
just kaimux::build          # release build + copy to dist/kaimux
just kaimux::test           # unit tests
just kaimux::integration    # end-to-end tmux integration test
```

**Workspace hygiene** (no namespace; operates on every
member):

```
just test         # workspace unit tests (sub-second; tempfile-fake-HOME for resources, tempdir Store for kaimux)
just fmt          # rustfmt
just fmt-check    # rustfmt --check
just lint         # clippy --workspace --all-targets -- -D warnings
just check        # cargo check --workspace
just ci           # the full hygiene gate
```

## Install

```
just resources::install
```

What it does:

1. Validates `resources/content/` — the AGENTS.md preamble
   is present + non-empty; each `skills/<name>/SKILL.md`
   has the required frontmatter.
2. Creates `$HOME`-facing symlinks per the registry at the
   top of
   [`resources/build-tool/src/main.rs`](./resources/build-tool/src/main.rs).
   Today: the merged AGENTS.md preamble (deployed as
   `~/.claude/CLAUDE.md`, `~/.claude/AGENTS.md`,
   `~/.kiro/steering/KIRO.md`, and
   `~/.kiro/steering/AGENTS.md` for cross-tool support)
   plus the skills tree (`~/.claude/skills` and
   `~/.kiro/steering/skills`).

`just resources::uninstall` is idempotent. Hand-written files at a
managed destination are preserved unless you pass
`--force`.

mAId follows the cross-tool **AGENTS.md** standard
(Linux Foundation Agentic AI Foundation; native support
across Codex, Copilot, Cursor, Kiro, Zed, Windsurf). The
legacy `CLAUDE.md` and `KIRO.md` filenames symlink at the
same source — drop them when Claude Code makes AGENTS.md a
default-read location.

## Where to look next

- Everything that gets installed:
  [`resources/content/`](./resources/content/).
- How installation is decided: the `REGISTRY` constant at
  the top of
  [`resources/build-tool/src/main.rs`](./resources/build-tool/src/main.rs).
- Reference shape for a new skill:
  [`resources/content/skills/kdevkit/SKILL.md`](./resources/content/skills/kdevkit/SKILL.md)
  (live siblings:
  [`notes/`](./resources/content/skills/notes/SKILL.md),
  [`writing-style/`](./resources/content/skills/writing-style/SKILL.md)).
- Full verb list: `just --list`.
