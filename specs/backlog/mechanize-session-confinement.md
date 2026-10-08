# Backlog: mechanize-session-confinement

## What

A ringmaster's feature session must stay in its own worktree and never
push or merge. codex now enforces that through its sandbox (the spawn
line in kdevkit's `tiers/initiative.md`). Claude Code and kiro still
rely on the brief's instruction. Make the boundary a mechanism wherever
a host offers one, and say plainly where it cannot be.

## Why

Push and merge cannot be undone, so a success rate is not enough; this
is a safety boundary, unlike the phase-adherence question the
deterministic-phasing initiative settled in favour of prose (tag
`research/kdevkit-durable-adherence`). That research rated sandboxing
strictly stronger and portable. Its lesson that does carry over: these
mechanisms fail open silently, so each one must fail closed and be
checked by a run.

## Findings so far (2026-10-08, no model calls)

- **codex 0.161:** `codex exec` defaults to read-only, and with
  on-request approvals a session escalates out. `workspace-write`
  confines writes but a linked worktree then cannot commit (its gitdir
  is in the main repo). A `-c` permission profile extending
  `:workspace` with write on the common gitdir's `objects`,
  `refs/heads/feat`, `logs/refs/heads/feat` and `worktrees/<name>`,
  plus `approval_policy="never"` and no `--sandbox`, lets it commit on
  `feat/*` and blocks push (no network), other refs, hooks and config.
  Leaks: `/tmp` is writable; sibling `feat/*` refs are writable.
- **Claude Code:** the Agent tool's `isolation: worktree` makes its own
  worktree from the default branch, cannot take an existing one, and
  does not stop push. The workable route is a headless `claude -p` in
  the worktree with a `--settings` sandbox (fail closed, no unsandboxed
  retry, empty network allowlist) and deny rules for push and merge as
  a second layer. It needs `socat` on Linux (absent on the machine
  checked, where it correctly refuses to start). Local ref moves stay
  possible, since the shared gitdir is writable.
- **kiro-cli 2.28:** no OS sandbox. Agent-config command denies and
  write paths are bypassable through the shell, and it is undocumented
  whether they hold under `--trust-all-tools`. It stays an instruction.
- `resources/build-tool/src/harness.rs` (codex invocation comment, "no
  config lifts it") is stale for 0.161 in the linked-worktree case.

## Done when

- One paid run of the codex spawn line: a feature session commits in
  its worktree and a push attempt fails.
- The Claude Code route is tried in one paid run where `socat` is
  present, and either adopted in the spawn step or recorded as not
  workable.
- kiro's denies are measured under `--trust-all-tools`, or kiro is
  recorded as instruction-only in the module.
- Related: `test-runner-workdir-containment`, `test-runner-sandbox-asymmetry`.
