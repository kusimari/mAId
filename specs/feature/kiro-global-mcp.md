# Feature: kiro-global-mcp

## Git Setup

- Branch: `feat/kiro-global-mcp/work`
- Base: `initiative/my-harness`
- Worktree: `mAId-worktrees/kiro-global-mcp`

## Feature Brief

kiro gets mAId's browser MCP server (`chrome-devtools`) from kiro's own
global MCP config, the way claude and codex already get it, rather than
from a sub-agent the user has to name. Any kiro agent that pulls the
global config in sees the server, and an agent file another part
re-renders no longer loses it.

Part of initiative: [[my-harness]]

## Handoff

- **Stage:** closed
- **Ready for:** squash-merge into `initiative/my-harness` by the
  ringmaster
- **Carry forward:** none open. The contract (global config, ownership
  by launcher path, atomic write, uninstall without kiro-cli) is in
  project.md, Architecture "Browser-control MCP".
- **Deliberately left:** old sub-agent entries are not migrated (ruled:
  none exist). Claude/codex ownership is backlog `mcp-remove-ownership`.
  No interrupt trap for the temp file: accepted, a stray dotfile beside
  the config is the worst case.

### Crossings

<!-- Append one line per crossing; never edit or delete a line. -->

- planning → dev
- dev → review
- review → dev · RETURN · fault: design · issue: uninstall needs kiro-cli
  on PATH to remove mAId's own entry, and the config write is in place,
  so an interrupt can truncate a file holding another tool's server ·
  fix: uninstall acts on the config whenever it exists; write a temp
  file beside the resolved target with its mode and rename it over ·
  done when: browser-mcp checks both, and browser-mcp and rollback pass
- dev → review
- review → closure
- closure → closed

## Requirements

- `just install` (and `just install kiro`) registers `chrome-devtools`
  for kiro with no extra argument. kiro is skipped only when `kiro-cli`
  is not on PATH or there is no graphical Chrome, as for claude and
  codex, or when `jq` is not on PATH (the skip names it).
- The kiro sub-agent argument is gone from `just install`, `uninstall`,
  `status` and the `*-browser-mcp` verbs. The attended browser test runs
  kiro's default agent, or the agent named with `--kiro-agent`.
- Any other server in kiro's global MCP config, and any other key in
  that file, keeps its name and value through install and uninstall.
- Running install twice leaves kiro's global config byte-identical.
- A `chrome-devtools` entry in kiro's global config that does not run
  mAId's launcher is not mAId's: install leaves it as it is and records
  the server as skipped, "not mAId's"; uninstall leaves it too; status
  says it is not mAId's.
- `just uninstall` removes mAId's entry even when `kiro-cli` is no longer
  on PATH.
- An install that is interrupted never leaves kiro's global config half
  written, and a symlinked config stays a symlink with its file's mode.
- `just uninstall` removes `chrome-devtools` from kiro's global config
  only when it is mAId's, and nothing else. With no config file, it
  creates none.
- A kiro global config that is not valid JSON is left as it is: install
  (with Chrome present; without, the skip is "no graphical Chrome") says so and records the server as skipped, status says so, and
  uninstall leaves the file and the part list alone.
- A key the user set on mAId's entry (such as `disabled`) survives
  install.
- `just status` reports kiro's registration from the global config.
- The kiro sub-agent argument has no replacement or compatibility form.
- The part list's kiro entry is `{kind: mcp, name: chrome-devtools}` with
  no scope, or the same with a fixed, path-free `skipped` reason.

## Test Strategy

Free tests only; nothing here calls a model.

### Functional (new `resources/tests/browser-mcp`)

A script run as `nix develop -c resources/tests/browser-mcp`, in a temp
HOME, with a stub `google-chrome` and a stub `kiro-cli` on a minimal
PATH. The stub `kiro-cli` writes a marker if anything runs it. It drives
`resources/browser/manage` for kiro and checks:

1. Install into a global config that already holds another server (with
   args, env and a field kiro does not define) and an extra top-level key:
   `chrome-devtools` points at the launcher, and the file minus that entry
   equals the seeded file as parsed JSON. Breaks if the edit rebuilds the
   file or drops keys.
2. The part list's kiro-cli entries include `mcp chrome-devtools` with no
   scope and no skip. Breaks if `--scope` is still passed.
3. An install over a compact config that already holds mAId's entry,
   marked `disabled`, leaves it byte-identical. Breaks if install
   rewrites a file that already holds the entry, or replaces the entry
   rather than merging into it.
4. Status reports kiro as registered.
5. Uninstall removes only `chrome-devtools` (the rest equals the seed),
   and the kiro mcp entry leaves the part list.
6. Install with no config file creates one holding only
   `chrome-devtools`. Uninstall with no config file creates none.
7. A config that is not valid JSON is byte-identical after install, and
   the part list says `skipped: "mcp add failed"`. Status says the
   config is not JSON; uninstall leaves the file and the part list.
7b. A config linked in by a relative symlink stays a link; its target
    gets the entry, keeps its mode (640), and is replaced by a rename
    (a new inode); no temp file is left. Breaks if the write is in
    place or loses the mode.
8. A `chrome-devtools` entry whose command is not the launcher is
   byte-identical after install, the part list says `skipped: "not
   mAId's"`, and status says not mAId's. Breaks if install writes
   without the ownership check.
9. Uninstall over that entry leaves the file byte-identical. Breaks if
   uninstall removes by name alone.
10. The stub `kiro-cli` was never run.
11. `just status --whole kiro` reports `chrome-devtools` present after
    install. Breaks if the entry is recorded where whole-status does not
    read it.
12. With no `kiro-cli` on PATH, uninstall removes mAId's entry (seen
    present first) and only that, and clears the part-list entry.
    Breaks if uninstall is gated on `kiro-cli`.
13. A new config gets the umask's mode (644 under umask 022). Breaks if
    the temp file's 600 is kept.
14. A config that is a link cycle fails install (no hang) and records
    `skipped: "mcp add failed"`. Breaks if link resolution has no cap.

### Existing

- `just test`, `just lint`, `just fmt-check`: unchanged Rust code, so
  these just stay green.
- `nix develop -c resources/tests/rollback`: it already runs
  `install-browser-mcp` with the real kiro-cli in a temp HOME. Kiro's MCP
  is now placed there too, so its `placed_on_all` check adds kiro's mcp
  entry.

## Design

**Why a direct edit, and not `kiro-cli mcp add --scope global`.** kiro-cli
has a global scope (`kiro-cli mcp add --help`, 2.29.0, under a temp HOME).
It was rejected for three reasons:

- In a temp HOME, `mcp add` and `mcp list` exit 1 with "You are not
  logged in". Install would then depend on a login, and no free test
  could run it.
- kiro-cli's published source for these commands reads the whole file
  into a typed struct and writes it back. Fields the struct does not
  define (another tool's keys) are dropped, defaults are filled in, and
  servers come back in hash order. That breaks rule 2 for every other
  server. Not checked against 2.29.0 itself, since that needs a login.
- `add` errors on an existing name unless `--force` is given, so the
  current comment that kiro's `add` replaces is wrong.

So `manage` edits `~/.kiro/settings/mcp.json` itself with `jq`, which the
dev shell already provides and the install already runs under (it calls
`cargo run`). `jq` keeps every other key, its value and key order. It
does re-indent the file to its own two-space style. That changes no
server's name or value.

**The edit** (in `resources/browser/manage`, kiro's leaf dispatchers):

- One state read, used by add, remove and get: `absent`, `ours` (the
  entry's `command` equals the launcher path `manage` was given, the
  profile's `bin/maid-browser-mcp`), `other` (any other value), or
  `bad` (the file is not one JSON object). A missing or empty file is
  `absent`.
- add: `absent` or `ours` merges `{command: <launcher>, args: []}` into
  `.mcpServers["chrome-devtools"]`, so any other key on mAId's entry
  stays. `other` writes nothing and records `skipped: "not mAId's"`.
  `bad` writes nothing and records `skipped: "mcp add failed"`.
- remove: `ours` is `del`eted; anything else writes nothing. An empty
  `mcpServers` is left in place. On `bad`, uninstall leaves the part
  list as it is, since whether mAId's entry is in the file is unknown.
- get: placed only when `ours`. kiro-cli is never launched.
- `uninstall-browser-mcp` and `status-browser-mcp` pass the launcher
  path to `manage`, as `install-browser-mcp` already does.
- Write, only when the result differs from the file as JSON: resolve a
  symlinked config to its target, write a temp file beside the target
  with the target's mode (a new file gets the umask's), and rename it
  over the target. The link stays a link, and a reader never sees half
  a file.
- Uninstall acts on the config whenever it exists, with or without
  `kiro-cli` on PATH: removing mAId's own entry is a file edit. Install
  and status stay gated on `kiro-cli`.

**Dropping the named sub-agent path.** Removed, not kept beside the
global path. An entry in a sub-agent file another part re-renders does
not last, and the global path reaches every agent that includes the
global config. Keeping both would be two ways in with no case that
needs the second. `MCP_AGENTS` loses its scope column, `record` loses
`--scope`, and the `kiro_sub` argument goes from both Justfiles.

**The part list.** `record --agent kiro --kind mcp chrome-devtools`, with
no `--scope`. The format and `part.rs` stay as they are: `scope` remains
optional for parts with more than one place. That matches whole-status,
which reads kiro's MCP from the global config.

**Not done.**

- No cleanup of an old sub-agent registration or an old scoped part-list
  entry. Scoped entries exist only from this initiative's unmerged
  branch, and the sub-agent file is re-rendered by its owner.
- claude and codex keep removing `chrome-devtools` by name; the same
  ownership check for them is backlog `mcp-remove-ownership`.

## Implementation Plan

- [x] `manage`: kiro global-config state/add/remove/get with `jq`, with
      the ownership check; drop the scope column, `KIRO_AGENT`,
      `agent_note` and `--scope`; fix the header notes.
- [x] Justfiles: drop `kiro_sub` from `install`/`uninstall`/`status` and
      the `*-browser-mcp` verbs; `verify-browser-mcp` keeps an optional
      kiro agent.
- [x] `browser-functional`: kiro runs the default agent unless
      `--kiro-agent` is given.
- [x] New `resources/tests/browser-mcp` (cases 1-10); `rollback` checks
      kiro's mcp entry.
- [x] Backlog `mcp-remove-ownership` for claude/codex.
- [x] Docs: project.md (Architecture browser MCP, the part-list example
      and `scope` bullet, Tech Stack verbs, Layout, Testing, Deployment),
      README's browser section and verbs, the flake's jq comment.

- *Risk note:* a `jq` older than 1.7 rounds number literals beyond
  double precision. The dev shell pins a current one.

## Session Log

- 2026-10-10 · Closure. Backlog answer: none closed (no existing item
  covers kiro's MCP registration); `mcp-remove-ownership` added for
  claude/codex. project.md: Architecture, Tech Stack, Layout, Testing
  and Deployment updated in dev; Testing extended at close for the
  later cases. Hard constraints unchanged (the MCP registrations were
  already mAId's to own).
- 2026-10-10 · Correctness review of the two changes: PASS WITH NOTES.
  Fixed: the mode check used 600, which mktemp gives anyway (now 640);
  a link cycle looped forever (now capped, fails as "mcp add failed");
  case 12 now checks its precondition; a new file's mode is tested.
  Mutations seen failing: no `cp -p`, no new-file `chmod`, no hop cap.
  Left: no trap for the temp file on an interrupt (it stays beside the
  config; rare).
- 2026-10-10 · Rebased onto `initiative/my-harness` after
  vendor-marketplaces merged (clean); gates rerun, all pass. Review
  rulings applied: uninstall acts on the config without `kiro-cli`;
  the write is atomic (temp beside the resolved target, its mode,
  rename). Mutations seen failing: in-place write (case 7b), uninstall
  gated on `kiro-cli` (case 12).
- 2026-10-10 · Final head gates: fmt-check, lint, `just test` pass;
  `browser-mcp` 20/20; `rollback` all pass. Briefing defects fixed: the
  `jq` skip is now in Requirements and the Decision Log.
- 2026-10-10 · Code Review Gate (correctness lens): PASS WITH NOTES.
  Fixed: mAId's entry is merged, not replaced (a `disabled` key
  survives); a config that is not JSON is reported by status and left
  with its part-list entry by uninstall; a symlinked-config check.
  Mutation seen failing: replacing the entry (case 3). Left as review
  judgement: no migration of old sub-agent entries (ruled out of
  scope), ownership is a literal path match, the write is not atomic.
- 2026-10-10 · Rebased onto `initiative/my-harness` after whole-status
  merged. The root `status` recipe's second argument now takes only
  `--whole` (an unknown value is an error); case 11 added. Gates rerun:
  fmt-check, lint, `just test`, `browser-mcp` (17), `rollback`.
- 2026-10-10 · Dev: `manage`, both Justfiles, `browser-functional`, the
  new `browser-mcp` test (16 checks) and the rollback check. Mutations
  seen failing: always writing (case 3), no ownership check (cases
  8-9), `--scope` still passed (cases 2, 7, 8). `build-tool record`
  needs `MY_HARNESS_PARTS`, so the test sets it.
- 2026-10-10 · Grounding: `manage` registered kiro per named sub-agent
  and skipped without one. `kiro-cli mcp add --help` (temp HOME) shows
  `--scope global`; `mcp add`/`list` need a login there. kiro-cli's
  published source shows a struct round-trip on write. `record` without
  `--scope` already works.

## Decision Log

- Edit kiro's global config with `jq` rather than `kiro-cli mcp add
  --scope global` · kiro-cli needs a login and rewrites other servers
  through its struct (rule 2) · rejected: kiro-cli's command; a Rust
  editor in build-tool (more code for the same result).
- Drop the named-sub-agent path, with no compatibility form · least
  mechanism; its entries do not last, and no caller passes it ·
  rejected: keeping it as an option.
- Install resets `args` on mAId's entry to `[]` while keeping its other
  keys · mAId owns the command line it launches · accepted at review.
- Uninstall needs no `kiro-cli`; install and status do · removing
  mAId's own entry is a file edit, while placing it is only useful
  where kiro runs · ruled at review.
- Atomic write by rename beside the resolved target · the file holds
  other tools' servers · rejected: the in-place copy kiro-cli itself
  uses (an interrupt can truncate it).
- Skip kiro when `jq` is not on PATH, with that reason · a clear skip
  beats a "mcp add failed" whose cause is buried · rejected: letting
  the edit fail. Added in dev; no test, since the test needs `jq`.
- Ownership of kiro's entry is its `command` equal to the launcher path
  · rule 2 now that mAId edits the file itself; the command is the one
  value mAId writes that marks it · rejected: removing by name, as
  claude/codex do (backlog `mcp-remove-ownership`).
