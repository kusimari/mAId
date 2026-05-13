# Feature (deferred): gorantls-agents-hand-select

> **Status: deferred.** Pick this up in a dedicated feature-dev
> session by promoting this file to `specs/feature/` and
> promoting its Implementation Plan.

## Feature Brief

`Gorantls-agents/install` today has a `--uninstall` flag and a
default validate-only path. What's missing: a hand-selection
surface so users can ask for specific agents instead of the
package-as-a-whole.

## Requirements (draft)

1. `./install --list` prints available agents (name + one-line
   description) from `agents/*.agent-spec.json`. Read-only; no
   side effects; no `aim` invocation.
2. `./install --agent <name>` (repeatable) installs just the
   named agent(s).
3. `./install --uninstall --agent <name>` removes just the named
   agent(s).
4. Logs clearly surface the current aim-granularity caveat (see
   Design below).

## Design (draft)

### aim granularity caveat

`aim agents install --local .` installs *every* agent spec the
package declares. aim doesn't expose a per-agent install path
via the local-install verb today. Options:

- **Option A (near-term):** `--agent <name>` becomes *advisory*
  — it does a package-level install and logs a clear note that
  all agents were registered. The user can pick up the right
  ones with `aim agents install Gorantls-agents --agents <name>`
  once the package is published via the registry flow.
- **Option B (later):** split `Config` into per-agent targets so
  `aim agents install --local . --target connected-workspace-1.0`
  works. Requires understanding aim's target semantics — likely
  a `build-tools` + `targets` pattern in the Config file. Needs
  reading `/home/gorantls/workplace/env/...` (Amazon internal docs)
  first.

Pick A for the first pass; flag B as follow-up.

### `--agent <name>` for uninstall

Similar caveat; same resolution. On uninstall, aim's `uninstall`
verbs run at the package level. The flag becomes advisory in the
short term.

## Test Strategy (draft)

- `--list` with a clean repo → both current agents listed.
- `--list` after adding a third spec → the new one shows up.
- `--agent connected-workspace` → package installed; log shows
  caveat; `aim agents list` confirms.
- `--uninstall --agent connected-workspace` → package removed;
  log shows caveat; `aim agents list` clean.

## Implementation Plan

_Deferred — to be picked up by a dedicated feature-dev session.
Promote this file to `.kdevkit/feature/gorantls-agents-hand-select.md`
and start from the Requirements above._

## Session Log

<!-- empty; populated when the feature is picked up -->

## Decision Log

<!-- empty; populated when the feature is picked up -->
