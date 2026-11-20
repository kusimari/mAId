# Feature: session-bring-ins

## Git Setup

- Branch: `feat/session-bring-ins/work`
- Base: `initiative/harness-core`
- Worktree: `mAId-worktrees/session-bring-ins`

## Feature Brief

The genome now shows the ghost's session bring-ins as what they are. The
personal persona's memory is a declared bring-in: it names the memory
package it runs, its store is a config value bound here, it is ready or
not by that store, and it records its true source (the person's own
package, at its pinned commit), so what is learned about it goes to that
package and not to mAId. The core's adapter on Claude Code shows as a
session bring-in too. The memory's index has a stated shape, so the
selector built next can read it without knowing the package.

Part of initiative: [[harness-core]]

## Handoff

- **Stage:** closed
- **Ready for:** squash-merge into `initiative/harness-core` by the
  ringmaster
- **Carry forward:** none open here. The ringmaster tells stream 2 that its
  selector enforces the index containment rule and that it adds a test for
  Claude Code ignoring `extensions`.
- **Deliberately left:** the store mismatch on a non-claude install, filed
  as `backlog/memory-store-record-follows-registration.md`.

### Crossings

<!-- Append one line per crossing; never edit or delete a line. -->

- planning → dev
- dev → review
- review → closure
- closure → closed

## Requirements

### What the person sees

- With a personal store set and the memory server registered, `maid-core
  genome` lists, under **session bring-ins**:
  - `personal-memory`: its job, `[personal, own]`, and `state:` the store's
    path;
  - `claude-adapter`: its job (the core's adapter on Claude Code),
    `[personal, own]`, while mAId's plugin is placed on Claude Code.
- `personal-memory` is **ready** when its store is there, and **not ready**
  with "store at `<path>` not there" when it is not; it is no longer
  **undeclared**. `just status --whole` reports the same.
- With no store set, `personal-memory` is not in the genome at all, as
  today (nothing registered, nothing recorded).
- mAId's part list records `personal-memory` with its source as the memory
  package's published repo and pinned commit, own, its store bound, and
  where its declaration is. Nothing about it names mAId as its source.
- Uninstall removes both records with what they stand for: the memory
  record when the server goes, the adapter record when mAId's plugin leaves
  Claude Code.

### What the next streams read

- The selector finds every memory to recall from in the harness map: each
  bring-in that binds an index beside its permanent tier, with the persona
  of the part that records it. The index's path comes from the bring-in's
  bound values; its shape is stated once, in project.md, independent of the
  package that writes it.
- Adaptation finds where a persona-memory habit is written (the store) and
  where a finding about the memory package goes (its own repo) in the same
  map.

## Test Strategy

### Unit (`just test`, load-bearing)

Each names the change that would break it, and runs in temp dirs only.

- **The shipped memory declaration**: the checked-in server manifest reads
  through the core's MCP slot reader as a session declaration naming the
  package, required `store` and `index` keys, and `store` as its
  permanent tier. Breaks if the file, its slot, or a field goes.
- **Pensieve's source from the lock**: a lock whose pensieve node is a
  github pin gives `project` at the repo's https URL and the pinned rev; a
  path pin or no pensieve node gives no source (so route falls to report).
  And the repo's own `flake.lock` gives `kusimari/pensieve` at a 40-char
  rev, so pinning a local path fails the test.
- **Install's records**: with a store and the server placed,
  `personal-memory` has the profile's server manifest as its unit, the
  pensieve source, own tenure, `store` and `index` (`memory/MEMORY.md`)
  bound; with the server not placed it
  is absent. `claude-adapter` is recorded with the profile's Claude plugin
  manifest as its unit only while `maid@maid` is placed on claude (not when
  it is only on codex). Uninstall keeps each only while that holds.

### Integration (free)

- `tests/memory_install.rs` (stub claude, temp HOME): after install, the
  part list's `personal-memory` carries the unit and source, and `status
  --whole` reports it **ready**; with the store removed, **not ready**.
- `nix develop -c resources/tests/rollback` (real claude and codex, temp
  HOME, temp store): the built profile holds the server manifest and the
  Claude manifest's declaration; `env -i HOME=… maid-core genome` lists
  `personal-memory` with the store as its state and `claude-adapter`, both
  under session bring-ins; the part list's `personal-memory` source is
  `https://github.com/kusimari/pensieve` at the flake's pinned rev.

### Paid

- None (0 of the initiative's 30 runs). No session sees anything new until
  stream 2's hooks place the genome.

## Design

**Why this shape.** Stream 1 left the memory server undeclared and
recorded it as mAId's own project. The least that fixes both uses what is
already there: the MCP standard's own manifest (`server.json`) carries the
declaration in `_meta["my-harness"]`, which the core already reads; install
already records the server's store as `values.store`; and the pensieve pin
already lives in `flake.lock`. The pensieve repo is not touched: the
manifest is mAId's description of its registration, `personal-memory`, and
names `pensieve` as its package. The adapter is declared where Claude Code
hooks ship, a plugin's manifest, through the slot stream 1 defined.

**The memory declaration.** `resources/content/servers/personal-memory/server.json`,
copied by the flake to `<profile>/share/maid/servers/personal-memory/server.json`:

```json
{
  "name": "io.github.kusimari/personal-memory",
  "description": "The personal persona's memory: pensieve on the personal store.",
  "version": "0.1.0",
  "repository": { "url": "https://github.com/kusimari/pensieve", "source": "github" },
  "_meta": {
    "my-harness": {
      "scope": "session",
      "job": "the personal persona's memory: what agents keep across sessions",
      "package": "pensieve",
      "config": {
        "store": { "about": "the persona's store, a git checkout the person owns" },
        "index": { "about": "the memory index, a path relative to store" }
      },
      "state": { "permanent": "store", "promote": "off" }
    }
  }
}
```

- `store` is the location pensieve takes as `--store`, bound per
  environment from mAId's store setting as today. `index` is bound by
  install to `memory/MEMORY.md`, pensieve's layout, kept as one constant
  beside the server's name in `memory.rs`. `store` is the permanent
  tier: persona memories are habits, written directly (the initiative's
  guards), so there is no local tier and nothing to promote.
- The store root is the bound location, not `memory/` under it: pensieve
  creates `memory/` at start and no part writes in a store, so a fresh
  store would read not ready for a dir nobody else may make.

**Source, tenure, route.** Install reads `flake.lock` in the checkout it
runs from (as it reads `HEAD`): a `nodes.pensieve.locked` of type `github`
gives `{ kind: project, at: "https://github.com/<owner>/<repo>", rev }`.
Any other pin gives no source, so the route falls to report and no path on
this machine is written. Tenure is own: pensieve is the person's own
package. The route is the project default, worktree: a finding about the
server is proposed as a PR on pensieve. A habit is not routed by this: it
is written to the store the state key binds.

**The memory index, as the selector reads it.** A file shape, not an
interface: the shape already exists (Claude Code's auto-memory format,
which pensieve's store uses), and the selector is stream 2's. Its path
comes from the bring-in, inside stream 1's contract with no new field: a
config key named `index`, its value a relative path under the permanent
tier's location. project.md's Memory section states the rule and the
shape:

- a bring-in whose declaration has a `state.permanent` key and a config
  key `index` has a memory index at `values[permanent]` joined with
  `values.index`; `index` is relative and never leaves that location;
- the index has one memory per line, `- [Title](file.md) - hook`, each
  file named relative to the index's own dir;
- each memory file has frontmatter `name`, `description` and
  `metadata.type` (`user`, `feedback`, `project`, `reference`), then the
  body;
- the persona is the one of the part that records the bring-in.

The selector walks the map's bring-ins and reads the indexes this rule
gives; a bring-in without an `index` key is not read until its stream
gives its shape.

**The adapter.** One session bring-in per shell adapter; on Claude Code it
is `claude-adapter`, declared in the Claude manifest of mAId's plugin
(`flake.nix`'s `claudePlugin`): `extensions."my-harness"` is read from
`resources/content/adapters/claude/my-harness.json`, `{ scope: session,
job: "the core's adapter on Claude Code: the maid plugin's hooks place the
genome and each turn's expression" }`, so a unit test reads the same file. Its hooks go in that
plugin (stream 2). `claude plugin validate` passes the manifest with a
warning for `extensions`; `--strict` fails on it, which is known and
accepted, since Claude Code ignores the field at load time and nothing
here runs `--strict`. Install records it while `maid@maid` is placed on claude
(read from claude's own entries, since codex places a plugin of the same
id), unit `<profile>/share/maid/marketplace/plugins/maid/.claude-plugin/plugin.json`,
source this repo, own. The codex manifest is unchanged.

**No other session bring-in.** Status is already in the genome as its
environment section, derived by the core on every call; reflection is
stream 3's.

**Build-tool changes** (`stages.rs`): `bring_ins` takes the pensieve source
and the Claude placement; `personal-memory` gains its unit and source;
`claude-adapter` is added; `still_placed` learns the adapter. A small
reader of the lock beside `Checkout::read`. No change to the core.

## Implementation Plan

- [x] The memory declaration: `server.json`, the flake copies it into the
  profile; install records `personal-memory` with its unit, the pensieve
  source from `flake.lock`, own; unit tests.
- [x] The adapter: the Claude manifest's declaration; install records
  `claude-adapter` while `maid@maid` is on claude; uninstall keeps it only
  then; unit tests.
- [x] `memory_install.rs` and the rollback test gain the checks above;
  project.md: Memory (the declaration, source, tier, the index shape), My
  harness (the adapter as a session bring-in), Layout, Testing.

- *Risk note:* a reflector finding about the declaration text itself (in
  mAId) would be proposed on pensieve, the recorded source. Rare; the
  declaration is a few fields.
- *Risk note:* stream 5 ships the browser server's `server.json` in the same
  place, `resources/content/servers/<name>/`; the flake copies the whole
  `servers` dir, so whichever stream lands second rebases onto one copy
  line.

## Session Log

- 2026-10-31 · grounded on the initiative spec, `core-and-infusion.md`,
  project.md (My harness, Memory), `resources/core` (contract, environment,
  entry), `stages.rs` (install's bring-in records), `flake.nix`, the
  rollback test, and pensieve at its pin (`4ff98e7`: README, project.md,
  `pensieve.py`). Pensieve's store layout is `about.md` plus `memory/` with
  `MEMORY.md` as the index; `Store.__init__` makes `memory/` at start.
  Probed in a temp HOME: `claude plugin validate` passes a plugin.json with
  `extensions` (warning "Unknown field … ignores it at load time"), and
  `--strict` fails it; nothing in the repo runs `--strict`.

- 2026-10-31 · dev: rulings from the planning cue folded in (servers dir,
  `index` key, `--strict` note); `server.json`, the flake copy, install's
  records for `personal-memory` (unit, pensieve source, `store`, `index`)
  and `claude-adapter`; unit tests, `memory_install.rs`, rollback checks;
  project.md. Broke the memory source and the adapter's claude check on
  purpose: 3 tests failed, then restored. The rollback test builds from a
  clone of HEAD, so it saw the new files only once committed. Rebased onto
  `initiative/harness-core` (spec-only commits); `just ci` and the
  rollback test green after.
- 2026-10-31 · Code Review Gate (one lens, correctness: no path in the diff
  is a named-risk path): PASS WITH NOTES, no must fix. Took: the lock is
  read without git; the lock's node is resolved through the root's
  `inputs`; the adapter declaration moved into a JSON file the flake reads,
  with a unit test; a unit test for reading the lock from a dir. Left: the
  adapter's job names hooks stream 2 adds (below); `install --agent
  <other>` after a store change records the new store while claude's
  server keeps the old one, a stream-1 behaviour (Handoff).
- 2026-10-31 · review briefing sent back three defects, fixed: two stale
  doc comments in `stages.rs`, and a unit test that a memory record with
  no GitHub pin has no source and only reports (watched it fail with the
  pensieve source dropped). `just ci` and the rollback test green.
- 2026-10-31 · closure. No unticked plan items. The ringmaster accepted
  the briefing's §4 items: `extensions` relies on Claude Code ignoring
  unknown fields (stream 2 adds a test); declaration-text findings route
  to pensieve, rarely; the index containment rule is stream 2's selector's
  to enforce; the adapter's job names stream 2's hooks; `server.json`'s
  version and name are left. Backlog: this feature closes none of the
  existing items; filed `memory-store-record-follows-registration`.
  project.md: Architecture (My harness: session bring-ins; Memory: the
  declaration, source and index rule), Layout and Testing were updated in
  dev; Tech Stack and Deployment are unchanged (no new dependency; install
  path the same).

## Decision Log

Calls made for the ringmaster from the initiative spec and the brief, to
check:

- Ruling: the memory bring-in keeps its name `personal-memory`; its
  declaration is mAId's own `server.json`, `_meta["my-harness"]`, naming
  package `pensieve` · why: a bring-in names its package, and the brief
  keeps the pensieve repo untouched · cost if wrong: move the file; the
  core reads any path.
- Ruling: its source is `https://github.com/kusimari/pensieve` at the rev
  `flake.lock` pins, read at install; any non-github pin records no source
  · why: one place holds the pin; a local path never reaches a part list ·
  cost if wrong: a constant beside the vendor pins.
- Ruling: tenure own, route the project default (worktree to a PR on
  pensieve) · why: the person's own package; nature is only proposed, on
  its source; habits go to the store by state, not by route · cost if
  wrong: one field (`backlog` instead).
- Ruling: `store` is the one config key and the permanent tier; no local
  tier; promote off · why: persona memory is a habit, written directly ·
  cost if wrong: add a local key later.
- Ruling: "readable by the selector" is a file shape (pensieve's store
  layout, which is Claude Code's auto-memory format), stated in project.md
  · why: the shape exists and the selector is stream 2's; an interface now
  would be code with no caller · cost if wrong: stream 2 wraps it in a
  reader.
- Ruling (ringmaster's ask: the index path from the bring-in): chose the
  declaration: a config key `index`, bound by the part to a path relative
  to the permanent tier, so the selector reads `values[permanent]` joined
  with `values.index` and hardcodes no path; project.md states only the
  rule · why: stays inside stream 1's contract (config keys are open), and
  stating the path in project.md would be the same hardcode in another
  place · cost if wrong: a contract field `state.index` later, same value.
- Ruling: the core's adapter shows as one session bring-in per shell;
  `claude-adapter` is declared in the Claude manifest of mAId's plugin and
  recorded while that plugin is on claude; the hooks go in that plugin ·
  why: hooks ship through a plugin, and that is the slot stream 1 defined ·
  cost if wrong: stream 2 moves the declaration with the hooks.
- Ruling: no other session bring-in · why: status is the genome's
  environment section already; reflection is stream 3's · cost if wrong:
  one more record.
- Ruling: no paid tests · why: nothing reaches a session until stream 2.
- Decision (review): the adapter's job names the hooks stream 2 puts in
  the plugin, though the plugin ships none yet · why: the initiative lands
  whole, and stream 2 needs no wording change · considered a job true only
  today; it would be rewritten in stream 2.

- Ruling (ringmaster): every MCP server declaration in this repo lives at
  `resources/content/servers/<name>/server.json`.

## Open questions

- For streams 2 and 5: a bring-in's own tiers in another shape
  (writing-style's `learned.md`) bind no `index` and are not read by the
  rule above; their shape for the selector is theirs to state.
