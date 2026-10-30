# Feature: harness-contract

## Git Setup

- Branch: `feat/harness-contract/work`
- Base: `initiative/my-harness`
- Worktree: `mAId-worktrees/harness-contract`

## Feature Brief

mAId states what it is in the agent's terms (agent, base harness, my
harness, part), the rules a part follows so it never fights another part,
and the format of the list a part leaves behind. `just install` now
leaves that list for mAId's part, so a later whole-status can read what
mAId placed on each base harness beside what any other part placed.

Part of initiative: [[my-harness]]

## Handoff

- **Stage:** closed
- **Ready for:** squash-merge into `initiative/my-harness` by the
  ringmaster
- **Carry forward:** none open. The contract (terms, rules, format,
  reader rule) is in project.md, Architecture "My harness"; the review
  rulings are in the Decision Log.
- **Deliberately left:** no stub-CLI test of the MCP script's record
  paths, and no end-to-end run of a kiro sub-agent MCP; the rollback
  test with the real CLIs and scratch runs cover them, so no backlog
  item. Dropping unknown keys and the per-record `cargo run` were
  accepted at review.

### Crossings

<!-- Append one line per crossing; never edit or delete a line. -->

- planning → dev
- dev → review
- review → closure
- closure → closed

## Requirements

- After `just install`, `${XDG_STATE_HOME:-~/.local/state}/my-harness/parts/maid-harness.json`
  exists and names, per base harness, what mAId placed there: claude and
  codex the `maid` marketplace and the `maid@maid` plugin; kiro and agy
  each mAId skill; claude, codex (and kiro, in the named sub-agent) the
  browser MCP server, when it was registered.
- The list says what is there, and what install skipped and why. A
  harness whose CLI is missing, a skill link install had to leave alone,
  a browser MCP with no Chrome or no kiro sub-agent named: each is an
  entry marked skipped, with the reason.
- `just install <agent>` and `just uninstall <agent>` change only that
  harness's entries. Running install twice leaves the file byte-identical.
- After `just uninstall` the file is gone. Skipped entries go with it. If
  something mAId placed could not be removed (a kiro MCP whose sub-agent
  was not named), its entry stays, and so does the file.
- Nothing else under `my-harness/parts/` is read, written or removed,
  beyond a temporary dotfile while the list is written.
- `project.md` documents the vocabulary, the additivity rules and the
  part-list format in generic terms, enough for another repo to write its
  own part's list.

## Test Strategy

### Unit (`just test`, load-bearing)

- Part list: recording a harness creates the file with the part name and
  entries; recording again with the same entries leaves the bytes the
  same; recording one harness keeps the others; recording one kind keeps
  other kinds (an MCP entry survives a skill re-record); entries in
  another scope are kept; a skipped entry carries its reason; recording
  nothing for the last entries removes the file; another part's file in
  the same dir is untouched.
- What symlinks placed (fake HOME): after install every shipped skill is
  listed for kiro and agy; a user's own skill in kiro's dir is not; a
  location left alone is listed as skipped; after uninstall nothing is.
- What plugins placed with no CLI on PATH: both entries skipped, naming
  the CLI (`no_cli.rs`).
- Install and uninstall stages write and remove the list in a temp parts
  dir, and a dry run writes nothing.

### Integration (free, temp HOME)

- `nix develop -c resources/tests/rollback` gains two checks: after its
  install the list names claude and codex's plugin and marketplace, and
  the MCP as placed or skipped; after uninstall it is gone. Real CLIs, temp HOME, no model calls.
- `cargo test -p build-tool --test plugins -- --ignored`: what plugins
  placed, read back from the real CLIs.

### Paid

- None. Nothing here changes what an agent sees in a session.

## Design

**One writer, in the build-tool.** The format lives in a new
`part.rs` (serde structs, `serde_json`, already a dependency), and only
it writes the file: load, replace the entries it is told about, write
atomically (`tempfile::NamedTempFile::persist`, already a dependency)
or remove the file when empty. Considered letting `manage` write its
MCP entries with `jq`: two writers in two languages would drift on the
format.

**What was placed is read back, not assumed.** A new `Deploy::placed`
returns, per harness, what of mAId's is there now, and what is not there
and why: `Symlinks` from each managed link (each FanOut child, or each
child of a Link dir) that resolves to the profile, or is skipped with
the state that stopped it; `Plugins` from the CLI's own plugin and
marketplace listings, or both skipped when the CLI is not on PATH;
`NoDeploy` nothing. `cmd_install` records it after acting for the
selected harnesses; `cmd_uninstall` records only the entries still
there, so uninstall clears skips. The list cannot claim what a refused
step did not place, and never stays silent on one.

**The MCP reaches the list through a verb.** `build-tool record
--agent <a> --kind mcp [--scope <s>] [--skipped <why>] [names...]`
replaces that harness's entries of that kind and scope. `manage` calls
it on install after a successful add (the name), a failed add, a
missing CLI, no kiro sub-agent, or no Chrome (the name, skipped with the
reason), and on uninstall with no names. A skip with no sub-agent named
carries no scope, so an uninstall that names none clears it.

**Where.** The Justfile exports `MY_HARNESS_PARTS`, defaulting to
`${XDG_STATE_HOME:-$HOME/.local/state}/my-harness/parts`, as it does
`MAID_PROFILE`; the build-tool reads it like `profile_dir()`. Tests
pass a temp dir.

**Format.**

```json
{
  "format": 1,
  "part": "maid-harness",
  "harnesses": {
    "claude": [
      { "kind": "marketplace", "name": "maid" },
      { "kind": "plugin", "name": "maid@maid" },
      { "kind": "mcp", "name": "chrome-devtools", "skipped": "no graphical Chrome" }
    ],
    "kiro-cli": [
      { "kind": "skill", "name": "kdevkit" },
      { "kind": "mcp", "name": "chrome-devtools", "scope": "my-sub" }
    ]
  }
}
```

- `harnesses` keys are the base harness's command (`claude`, `codex`,
  `kiro-cli`, `agy`): public, and the same in every part's list.
- `kind`: `skill`, `mcp`, `agent`, `plugin`, `marketplace`, `setting`.
  `name` is what the harness's own listing calls it.
- `scope`, optional: where inside the harness, when it has more than one
  place (kiro's MCP servers live per sub-agent).
- `skipped`, optional: install meant to place this and did not, and
  why. A reader reports it and does not check it for presence.
- Entries sorted, keys sorted (BTreeMap), so two writes compare.
- `format` lets a reader refuse a list it does not understand; readers
  ignore keys they do not know.

**Docs.** `project.md` gains a `### My harness` subsection under
Architecture: the four terms, the five additivity rules, the path and
the format above. Generic terms only.

## Implementation Plan

- [x] `part.rs`: the format, `record`, atomic write, remove-when-empty; unit tests.
- [x] `Deploy::placed` for `Symlinks`, `Plugins`, `Deployment`, `NoDeploy`; unit tests (symlinks), ignored test (plugins).
- [x] `cmd_install` / `cmd_uninstall` record what is placed; `record` verb; `MY_HARNESS_PARTS` in the Justfile; `manage` calls `record`.
- [x] Rollback test: list present after install, gone after uninstall.
- [x] `project.md`: vocabulary, additivity rules, part-list format; Layout line for `part.rs`.

- *Risk note:* `Plugins::placed` costs two CLI calls per plugin harness
  on each install and uninstall (as `check_installed` already does).
- *Risk note:* `manage` now needs `cargo` (it already runs only from the
  Justfile, in the dev shell).

## Session Log

- 2026-10-10 · grounded: install is composed in the root Justfile
  (`install-profile`, `install-skills` via the build-tool, then
  `install-browser-mcp` via `resources/browser/manage`). The build-tool
  places skills (kiro, agy), plugin and marketplace (claude, codex);
  `manage` places the `chrome-devtools` MCP (claude, codex global; kiro
  per named sub-agent; skipped with no Chrome). `rollback` test already
  runs install in a temp HOME with `XDG_STATE_HOME` unset.
- 2026-10-10 · dev: `just ci` green (212 build-tool unit tests); the
  ignored `plugins` test (real CLIs, temp HOME) and
  `nix develop -c resources/tests/rollback` pass, including the three new
  part-list checks. A scratch `manage install`/`uninstall` with no CLIs
  and no Chrome wrote skipped MCP entries per harness and cleared them.
  Real `~/.local/state/my-harness` checked absent afterwards.
- 2026-10-10 · Code Review Gate (correctness lens; single lens, no
  named-risk path): PASS WITH NOTES. Fixed: a kiro skip with no scope
  outliving a scoped install; `record` now refuses a list in another
  format; install and uninstall record even when they stop partway; no
  Chrome keeps an earlier registration as placed; `manage` records with
  bash 3.2-safe args, and a failed record fails the run at the end, not
  partway; skip reasons name no path. Documented: placed means listed, in
  any state. Left: an empty `XDG_STATE_HOME` resolves as for
  `MAID_PROFILE` (same expression).
- 2026-10-10 · second review cycle (fix commit): PASS WITH NOTES.
  Fixed: an MCP uninstall reads back before clearing; `reason()` names
  every state; a part-list error beside a failed step is printed; tests
  for another part's file and a foreign link's reason. `just ci`, the
  ignored `plugins` test and the rollback test green again.
- 2026-10-10 · review briefing defect: readers were told to read every
  file in the parts dir, which includes the writer's temporary file.
  Docs now say `*.json`, not dotfiles.
- 2026-10-10 · review rulings applied (fixed `mcp add failed` reason,
  CLI words to stderr). On the final code commit 3d43d0f:
  `nix develop -c just ci` passed (217 build-tool unit tests), and
  `nix develop -c resources/tests/rollback` passed all 13 checks.
  Stub-CLI scratch run: a failed add recorded the fixed reason and
  printed the CLI's line on stderr.
- 2026-10-10 · closure: backlog answer none. No backlog item is closed
  by this feature (none covers the part list or the harness terms), and
  none is filed: what was left is covered or accepted at review.
  project.md Deployment now says install writes and uninstall removes
  the part list.

## Decision Log

- Part list records what is there after acting, read back from each
  mechanism · a list of intent would claim a skipped MCP or a refused
  link · rejected: writing the declared set from the registry.
- A step install skipped is an entry with `"skipped": "<why>"`, reported
  not checked · whole-status must show what should be there against what
  is, and silence reads as nothing expected (ringmaster ruling).
- One writer (Rust), `manage` calls it · rejected: `jq` in `manage`, a
  second implementation of the format.
- `"format": 1` · one line; lets another repo's reader refuse a later
  incompatible shape.
- Ruling (ringmaster, review): the format is ratified as written; the
  other part's reader is built from project.md's text.
- Ruling (ringmaster, review): an uninstall with a harness's CLI off
  PATH clears its plugin, marketplace and MCP entries alike.
- Ruling (ringmaster, review): a failed MCP add records the fixed reason
  `mcp add failed`; the CLI's own words go to stderr · the list stays
  path-free (read by other parts, public repo) and byte-stable.
- Ruling (ringmaster, review): rewriting drops unknown keys while mAId is
  its file's only writer; `manage` pays one `cargo run` per record.
- Harness keys are command names (`kiro-cli`, not mAId's `kiro` token) ·
  every part must agree on them without reading mAId's code.
