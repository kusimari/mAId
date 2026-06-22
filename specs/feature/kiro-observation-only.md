# Feature: kiro-observation-only

## Git Setup

- Branch: `feat/kiro-observation-only`
- Base: `feat/kaimux` (stacked PR — merges into the parent feat
  branch first, then the parent ships to main)
- Worktree: `/local/home/gorantls/tool-workplace/ai-workspace/mAId-kiro`

## Feature Brief

The kaimux spec calls Kiro **observation-only in v1**: register
the pane in the dashboard + clean up on pane-exit, but do not
inject hooks. The code carried over from agent-orch still writes
a Claude-shape JSON to `<cwd>/.kiro/agents/kaimux.json` on every
Kiro `wrap`. Kiro logs `invalid agent config` on every prompt
because that file isn't a valid Kiro persona.

This slice closes the spec ↔ code drift by dropping the write
entirely. The cleanup path stays — users upgrading from the old
binary still get their orphan `kaimux.json` files removed when
they exit the affected Kiro panes.

## Requirements

- `kaimux wrap kiro -- kiro-cli ...` registers the pane and
  execvps the agent, and **does not write any file under
  `<cwd>/.kiro/`**.
- `unregister` (called by tmux's `pane-exited` hook on Kiro
  panes) still removes a stale `<cwd>/.kiro/agents/kaimux.json`
  and its empty parent dirs, so users carrying that file from
  prior installs see it cleaned up on next pane exit.
- The user-visible dashboard UX for Kiro panes is unchanged:
  registers, shows the icon (no hook events ⇒ stays in `Done`
  → `Idle` decay), unregisters on pane-exit. The inline pane
  snippet still surfaces what Kiro is doing visually.
- No regression on Claude or Other (`Other(<name>)`) wrap paths.

## Test Strategy

Unit tests against a tempdir `Store` and tempdir `cwd`:

- `kiro_prepare_does_not_write_any_file` — call
  `Kiro.prepare(ctx)`, assert no file under
  `<cwd>/.kiro/agents/`.
- `kiro_unregister_removes_stale_kaimux_json` — pre-place a
  bogus `<cwd>/.kiro/agents/kaimux.json`, register a kiro pane,
  unregister; assert the file + empty parent dirs are gone.
- `kiro_unregister_keeps_kaimux_json_when_sibling_kiro_present` —
  same setup, but with two kiro panes in the same cwd; close one,
  assert the file remains.
- `kiro_unregister_no_op_when_no_stale_file` — no kaimux.json
  on disk, just unregister; assert no error.

The existing `wrap_kiro_stamps_created_flag_and_writes_project_config`
test is **deleted** along with the `created_kiro_config` field.

## Implementation Plan

- [x] `Kiro::prepare` becomes `Ok(passthrough(ctx))` — same as
      `Claude::prepare`.
- [x] `Kiro::cleanup` keeps its body (it already removes the
      file on no-sibling-kiro), drops the `created_kiro_config`
      reasoning from its comment.
- [x] Delete `build_kiro_config` (only caller was the dropped
      write).
- [x] Drop `created_kiro_config: bool` from `Prepared`.
- [x] Drop `created_kiro_config: bool` from `Session`. The
      `#[serde(default)]` makes older registries deserialize
      cleanly (the extra on-disk field is ignored by serde).
- [x] In `wrap()`, drop the `created_kiro_config: prepared.created_kiro_config`
      line of the `Session` literal.
- [x] In `passthrough()`, drop the `created_kiro_config: false`
      line.
- [x] Update tests per Test Strategy.
- [x] `just ci` green (53 unit tests).

## Migration

Users who already have `<cwd>/.kiro/agents/kaimux.json` from the
old binary will see it removed on next Kiro pane-exit (kaimux's
`pane-exited` hook runs `unregister`, which still has the
file-removal path). No manual user action required for cleanup;
the worst case is one extra `invalid agent config` log line
between upgrade and the next pane close in that cwd.

## Provenance

Tracked through PR #24 review as the deferred Kiro drift item;
explicitly named in `specs/feature/kaimux.md` → "Open issues
deferred to post-PR-#24 slices".
