# Feature: scaffolding-first-skills-set

## Git Setup

- Branch: `feat/scaffolding-first-skills-set` (off `feature-ai`)
- Base: current tip of `feature-ai` (after `feat/flake-isolation-and-undeploy`)

## Feature Brief

Re-establish mAId's scaffolding so Deno's native tooling is the
whole dev surface: `deno task` runs every verb (fmt / lint / test
/ install / uninstall / deploy / undeploy / validate / status /
setup / teardown); `scripts/maid` is deleted; the top-level
`install` shrinks to a 3-line pass-through into `deno task`. As
part of the same pass, move `CLAUDE.md` and `KIRO.md` into
`sources/` so every registry entry reads from the same tree, and
rewrite their bodies to reference only the deployed paths
(`~/.claude/…`, `~/.kiro/…`) — the checkout is an author-side
detail that shouldn't leak into a user-facing steering doc.

Why "scaffolding-first-skills-set": the three default skills
(`development`, `git`, `writing-style`) plus the newer `kdevkit`
skill are the reference shape every future skill learns from.
Getting the scaffolding right now means downstream authors copy a
clean pattern.

## Requirements

1. `deno task test` runs the full suite with no additional flags.
2. Bare `deno test` still fails with a clear permission error
   (that's Deno's model — don't paper over it).
3. `deno task fmt`, `deno task lint`, `deno task check` each
   exit 0.
4. `deno task install` writes a deno-generated shim at
   `$HOME/.local/bin/maid`; `deno task uninstall` removes it and
   is idempotent.
5. `deno task setup` and `deno task teardown` are the composite
   entrypoints used by the thin shell `install`.
6. `./install` and `./install --uninstall` stay as the
   env-workplace cold-start entrypoints but are 3-line
   pass-throughs into `deno task`.
7. `scripts/maid` is deleted and no live reference survives.
8. `CLAUDE.md` → `sources/claude/CLAUDE.md`;
   `KIRO.md` → `sources/kiro/KIRO.md`; registry updated; tests
   green.
9. The bodies of CLAUDE.md and KIRO.md never mention a checkout
   path. They only reference `~/.claude/…` and `~/.kiro/…`.

## Design

### deno.json is the single source of truth

Tasks: `check`, `fmt`, `lint`, `test`, `install`, `uninstall`,
`validate`, `deploy`, `undeploy`, `status`, `setup`, `teardown`.
The `uninstall` task uses `|| true` to stay idempotent when the
shim is already missing. `imports` block moves JSR specifiers out
of the test files so `deno lint` stops flagging inline prefixes.

### Shell install is 3 lines of pass-through

One `case` for `--uninstall` / `--help`, one branching `exec` into
either `deno task <task>` (direnv users) or `nix develop --command
deno task <task>` (cold-start users). No symlink management, no
FORWARD array.

### `sources/claude/` and `sources/kiro/` hold the steering docs

Registry entries for `.claude/CLAUDE.md` and
`.kiro/steering/KIRO.md` point at the new source paths. Structural
smoke `tests/functional/run` updated for the new expected targets.
Unit-test fixture helper (`makeCheckout` in `deploy_test.ts`)
creates the new directory structure.

### CLAUDE.md / KIRO.md rewritten for the deployed-path reader

Original body had `~/env-workplace/mAId/sources/skills/…` — that's
an author's perspective leaking. Rewritten so a reader of
`~/.claude/CLAUDE.md` sees only `~/.claude/…` paths and a simple
invariant: *don't write directly into the managed directory; edit
the file the directory already exposes*. kdevkit standing-rules
block preserved verbatim.

## Test Strategy

- **Unit + integration:** `deno task test` — 22 existing tests
  still pass with the new fixture paths.
- **Lint / fmt:** `deno task lint` + `deno fmt --check maid/
  tests/` exit 0.
- **Shim round-trip:** remove `~/.local/bin/maid` → `./install` →
  shim re-created → `maid status` reports all 5 entries ok.
- **Teardown round-trip:** `./install --uninstall` → shim removed
  + all 5 symlinks removed → re-run exits 0.
- **Structural smoke:** `./tests/functional/run --no-tools` —
  five registry entries all resolve to `sources/…`, four skills
  visible.
- **Stale-reference grep:** `rg 'scripts/maid' maid/ tests/
  install README.md deno.json` returns no hits.

## Implementation Plan

1. `deno fmt` + `deno lint` baseline (mechanical diff only; surface
   any lint debt and fix it — here: move JSR imports into
   `deno.json` `imports` map).
2. Expand `deno.json` with the full task surface. Add
   discoverability comment at the top of both test files.
3. `git mv CLAUDE.md sources/claude/CLAUDE.md` +
   `git mv KIRO.md sources/kiro/KIRO.md`. Update
   `maid/registry.ts` + the `makeCheckout` helper in
   `tests/deploy_test.ts`. Update structural smoke assertions.
4. Rewrite CLAUDE.md / KIRO.md bodies. Keep the kdevkit
   standing-rules block.
5. Rewrite `install` as the 3-line pass-through. Delete
   `scripts/maid`. Delete empty `scripts/` dir.
6. Seed `.kdevkit/feature/scaffolding-first-skills-set.md` (this
   file) + three deferred feature-wip files.
7. Refresh `sources/skills/kdevkit/SKILL.md` + `.kdevkit/project.md`.
8. Rewrite `README.md` as a pointer doc.

After each step: `deno task check && deno task lint && deno task
test && ./tests/functional/run --no-tools`. All green.

## Session Log

<!-- Newest at top -->

- 2026-05-12 · deno fmt + lint baseline applied; `no-import-prefix`
  fixed by moving `@std/assert` into the `imports` map.
- 2026-05-12 · Full task surface wired in `deno.json` (install,
  uninstall, deploy, undeploy, validate, status, setup, teardown).
  Discoverability comment added to both test files.
- 2026-05-12 · CLAUDE.md and KIRO.md moved to `sources/claude/`
  and `sources/kiro/`. Registry + test fixture + structural smoke
  updated. Force-redeployed symlinks on the live host.
- 2026-05-12 · CLAUDE.md / KIRO.md rewritten to reference only
  deployed paths; kdevkit standing-rules preserved.
- 2026-05-12 · `install` shrunk to 3-line deno-task pass-through;
  `scripts/maid` deleted; empty `scripts/` dir removed. Verified
  round-trip: `./install` → deno shim at `~/.local/bin/maid` + 5
  symlinks. `./install --uninstall` → shim and symlinks removed;
  re-run idempotent.

## Decision Log

<!-- Newest at top -->

- 2026-05-12 · `uninstall` task uses `|| true` to stay idempotent.
  Deno itself returns 1 when asked to uninstall a non-existent
  shim; wrapping with `|| true` keeps `teardown` returning 0 on
  re-runs, matching the `maid undeploy` side.
- 2026-05-12 · Flake at repo root, not under `Nix/` — decision
  carried forward from `feat/flake-isolation-and-undeploy`.
- 2026-05-12 · Permission discoverability chosen over
  deno-native zero-perms tests. Putting `--allow-*` on the `test`
  task plus a one-line comment at the top of each test file is
  the simplest truthful answer. Alternative considered: rewrite
  every `Deno.test(...)` call to declare per-test permissions —
  rejected as invasive for 22 existing tests.

## Future work

Deferred to dedicated feature-dev sessions (tracked under
`.kdevkit/feature-wip/`):

- `maid-as-flake-package.md` — convert mAId into a
  `packages.<system>.default` flake output consumable from
  `~/env`.
- `kiro-side-functional-smoke.md` — `kiro-cli` parity coverage
  for the functional smoke (today only `claude --print` fixtures
  exist).
