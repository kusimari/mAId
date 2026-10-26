# Initiative: agy-install

- Branch: `initiative/agy-install` (cut from `main` at 0881e13)

## Goal

Installing mAId recognizes `agy` as a first-class supported coding agent.
When `agy` is available on `PATH`, `just install` / `./install` deploys mAId
natively using `agy plugin install` from the profile's marketplace, just like
claude and codex use their native plugin installers. Existing symlink installs
under `~/.gemini/config/skills` are cleanly reaped as legacy links, and `just uninstall`
removes the native plugin cleanly.

## Experience

- `just install` (or `./install`) automatically detects if `agy` is available on `PATH`.
  If present, it installs mAId's skills as a native plugin into `agy`.
- `agy plugin list` lists `maid` as an installed plugin.
- A user can target `agy` specifically via `just install agy` or `just resources::install-skills agy`.
- Old symlink installs under `~/.gemini/config/skills` left by prior mAId versions
  are reaped when installing the native plugin.
- If `agy` is not on `PATH`, the install continues gracefully for other agents, reporting `agy not on PATH` rather than failing.
- `just rollback` returns `agy` to the previous profile generation's plugin.
- `just status [agy]` reports the plugin status of `maid` in `agy`.
- `just uninstall [agy]` removes `maid` from `agy` via `agy plugin uninstall maid`.
- The Macro test (`verify-install`) tests `agy` when present on `PATH`.

## Macro test

Run on the initiative branch before closing into `main`:

1. Build and install from a temporary clone, then remove the clone.
2. If `agy` is on `PATH`, verify `agy plugin list` lists `maid` enabled.
3. Verify legacy `~/.gemini/config/skills` symlinks are reaped.
4. Verify no checkout paths leak into the installed plugin.
5. Install from a second clone with a modified skill marker; verify `agy` sees the update. Roll back; verify `agy` sees the previous text.
6. Run `just uninstall`; verify `agy plugin list` no longer lists `maid`. Reinstall from the workspace.

## Constraints

- Public repo: no internal names, tickets, or internal URLs in any artefact.
- Use `agy`'s native CLI (`agy plugin install`, `agy plugin uninstall`, `agy plugin list`).
- When `agy` is not on `PATH`, install and status must not fail; report `agy not on PATH` (graceful skip), matching claude/codex.
- Touch only what mAId owns: mAId's plugin in `agy`, and legacy mAId symlinks.
- Workspace unit tests (`cargo test`) must pass without requiring `agy` on `PATH` (using synthetic mocks / fake HOME or unit isolation).

## Streams

| # | Feature | Ships | Needs | Status |
|---|---|---|---|---|
| 1 | agy-plugin-package | `flake.nix` marketplace outputs root `plugin.json` in the plugin directory so `agy` discovers and installs the plugin natively | none | planned |
| 2 | agy-native-deploy | `build-tool` moves `agy` from `REGISTRY` (symlinks) to `PLUGIN_AGENTS`, implementing `agy plugin install/uninstall/list`, legacy link reaping, and graceful CLI check | 1 | planned |
| 3 | agy-verify-install | `resources/tests/verify-install` and `project.md` updated to verify `agy` native install when on PATH | 2 | planned |

## Decision Log

- Ruling: treat `agy` as a native plugin agent rather than a symlink agent · why: `agy` provides a native `agy plugin` CLI supporting `install`, `uninstall`, and `list`; matching claude and codex fulfills mAId's mission to use native agent installers where available · cost if wrong: low; plugin manifest is standard.
- Ruling: reap `~/.gemini/config/skills` as a legacy link in `PLUGIN_AGENTS` · why: previous installs linked the profile directly into `~/.gemini/config/skills`; migrating to native plugin installation requires cleaning up old symlinks so agents don't see duplicate skills · cost if wrong: none, matches how claude's `.claude/skills` legacy link was reaped.
- Ruling: 3 sequential streams (package → deploy → verify) · why: each delivers a distinct testable slice; package ensures the build artifact exists, deploy adds the Rust state machine, verify ensures end-to-end Macro test and project documentation alignment · cost if wrong: minimal overhead.
