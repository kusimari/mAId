# mAId

Tool-agnostic source of truth for agentic resources — skills,
agents, commands, steering docs — compiled into whatever AI tool
happens to be in use (Claude Code, Kiro, Gemini CLI, future
tools).

The repo is the checked-in source. Installing mAId creates
symlinks from `$HOME` into this tree, so edits to `sources/`
are live for the next AI session. All work on mAId itself
goes through `deno task <verb>` — there is no separate
binary on `PATH`.

## Develop

```
direnv allow              # loads deno via the repo-local flake
deno task test            # full suite
deno task fmt             # format
deno task lint            # lint
deno task check           # typecheck
```

Full task list lives in [`deno.json`](./deno.json). A bare
`deno test` fails with permission errors by design — use
`deno task test` or `deno test -A`.

The development methodology (spec-driven, phase-gated) is encoded
in the [`kdevkit` skill](./sources/skills/kdevkit/SKILL.md).
Project context lives in [`specs/project.md`](./specs/project.md);
feature specs live in [`specs/feature/`](./specs/feature/);
open future work sits in [`specs/backlog/`](./specs/backlog/).

## Install

```
./install              # deno task setup:   validate + deploy
./install --uninstall  # deno task teardown: undeploy
```

What happens on install:

1. Deno picks up from the repo-local flake (direnv active)
   or via `nix develop --command` if nix is present.
2. `deno task validate && deno task deploy` symlinks every
   entry in [`maid/registry.ts`](./maid/registry.ts) — today
   `~/.claude/CLAUDE.md`, `~/.claude/skills`,
   `~/.claude/agents`, `~/.claude/commands`,
   `~/.kiro/steering/KIRO.md`, and
   `~/.kiro/steering/skills`, each pointing into
   [`sources/`](./sources/).

Uninstall is idempotent. Hand-written files at a managed
destination are preserved unless you pass `--force`.

## Where to look next

- Everything that gets deployed: [`sources/`](./sources/).
- How deployment is decided: [`maid/registry.ts`](./maid/registry.ts).
- Reference shape for a new skill:
  [`sources/skills/development/SKILL.md`](./sources/skills/development/SKILL.md)
  (and its three siblings).
- Full dev-verb list: [`deno.json`](./deno.json) `tasks` block.
