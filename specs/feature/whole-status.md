# Feature: whole-status

## Git Setup

- Branch: `feat/whole-status/work`
- Base: `initiative/my-harness`
- Worktree: `mAId-worktrees/whole-status`

## Feature Brief

`just status --whole` shows my harness as a whole: for each base harness,
every installed part's list of what it placed, and whether each thing is
there, read from the harness's own listing or config. Any part's list
counts, not only mAId's, so a second part installed beside mAId is checked
the same way.

Part of initiative: [[my-harness]]

## Handoff

- **Stage:** closed
- **Ready for:** squash-merge into `initiative/my-harness` by the
  ringmaster
- **Carry forward:** none open. How presence is read per harness, the
  refusal rules and the reader pattern are in project.md, Architecture
  "My harness" (Whole-status); the rulings are in the Decision Log.
- **Deliberately left:** no backlog item. The not-checkable cells
  (codex settings and sub-agents, kiro plugins and marketplaces, claude
  non-`user` plugin scopes, agy beyond skills) have no part listing them
  today; no timeout and the `CLAUDE_CONFIG_DIR` / `CODEX_HOME` split were
  accepted at review. A present MCP end to end and agy on a real install
  belong to the initiative's Macro test and its agy stream.

### Crossings

<!-- Append one line per crossing; never edit or delete a line. -->

- planning → dev
- dev → review
- review → closure
- closure → closed

## Requirements

- `just status --whole` prints, per base harness (`claude`, `kiro-cli`,
  `codex`, `agy`, then any other key a list uses), each part with an
  entry there, and each of that part's entries as one of:
  - **present**: the harness lists it; **present (disabled)** where the
    listing says it is turned off, which does not fail the run;
  - **missing**: the harness does not list it;
  - **skipped (why)**: the part says it meant to place it and did not;
    not checked;
  - **not checkable (why)**: there is no way to read this kind on this
    harness, the harness's CLI is not on PATH, or the kind or harness is
    one this reader does not know;
  - **unreadable (why)**: the harness's listing or config could not be
    read, including an agent file that does not parse.
- Part-list files the reader refuses (not JSON, an unknown `format`, not
  the format's shape, a `part` that is not the file's name) are each listed with the reason. Dotfiles and
  non-`.json` files in the parts dir are not read. Unknown keys are
  ignored.
- Exit status is non-zero when any entry is missing or unreadable, or any
  file was refused; otherwise zero. Skipped and not-checkable entries do
  not fail it.
- `just status --whole <agent>` (`claude|kiro|codex|agy`) scopes to that
  harness. With no part lists at all it says so and exits zero.
- `just status` with no flag prints what it prints today.
- Nothing is written: status reads files and runs the harnesses'
  read-only listing commands.

## Test Strategy

### Unit (`just test`, load-bearing)

Fixture part lists in a temp parts dir, a temp HOME, and stub CLIs (shell
scripts printing fixed JSON) in a temp bin dir passed as the PATH. No real
HOME, no real CLI.

- Reader: two parts are both read; a dotfile and a `.txt` are not; bad
  JSON, `format: 2`, and a list with no `harnesses` are each refused with
  a reason; an unknown key is ignored; an unknown kind or harness key
  comes back as not checkable, not refused.
- Each checkable (harness, kind) gives present for a listed name and
  missing for another, through the stub listing or the temp-HOME file
  (table under Design).
- A skipped entry is reported with its reason and its harness's listing
  is not run (the stub fails if called).
- A harness whose CLI is not on the PATH gives not checkable for its CLI
  kinds and still checks its file kinds.
- A stub that exits non-zero or prints non-JSON gives unreadable.
- Exit code: 0 for present, skipped and not checkable alone; 1 with one
  missing, one unreadable, or one refused file.
- The agent selector shows only that harness.

### Integration (free, temp HOME)

`nix develop -c resources/tests/rollback` gains, after its install with
the real CLIs (the Macro test's first two steps):

- `just status --whole` exits 0 and shows mAId's claude and codex plugin
  and marketplace and kiro's and agy's skills as present.
- With a second part's list placed beside mAId's (one present entry, one
  missing), it exits non-zero and reports both parts; removing that list
  brings it back to 0.

### Paid

None. Nothing here changes what an agent sees in a session.

## Design

**Where.** A new `whole.rs` in the build-tool holds the reader, the
per-harness probes, and the report. A new build-tool verb `whole
[--agent <a>]` runs it, reading `MY_HARNESS_PARTS` like install does.
The root Justfile's `status` takes `--whole` in either positional slot
(`just status --whole [agent]`, `just status <agent> --whole`), as
`install-skills` spots a leading flag; set, it runs
`resources::status-whole [agent]`, otherwise today's three steps, now
in one bash body under `set -e` so a failed step still fails the run.

**Reader.** Its own lenient serde structs, beside the writer's strict
ones in `part.rs`: `kind` is read as a string, so a kind this reader does
not know is one not-checkable entry instead of a refused file. A file is
refused for bad JSON, a `format` other than 1, or a missing `part` or
`harnesses`. Files are read in name order, so output is stable.

**Probes.** One per harness key, built lazily and once per run: each CLI
listing is run at most once (`HOME` set, PATH given), each config file
read at most once.

| Harness | skill | agent | mcp | plugin | marketplace | setting |
|---|---|---|---|---|---|---|
| `claude` | `~/.claude/skills/<n>/SKILL.md` | `~/.claude/agents/*.md`, frontmatter `name` | `~/.claude.json` `mcpServers` | `claude plugin list --json`, `id` at `user` scope; other scopes not checkable | `claude plugin marketplace list --json`, `name` | `~/.claude/settings.json` top-level key |
| `codex` | `~/.codex/skills/<n>/SKILL.md` | not checkable | `codex mcp list --json`, `name` | `codex plugin list --json`, `installed[].pluginId` | `codex plugin marketplace list --json`, `marketplaces[].name` | not checkable |
| `kiro-cli` | `~/.kiro/skills/<n>/SKILL.md` | `~/.kiro/agents/*.json`, `name` | `scope` = the agent named so, its `mcpServers`; no scope: `~/.kiro/settings/mcp.json` `mcpServers` | not checkable | not checkable | `~/.kiro/settings/cli.json` key |
| `agy` | `~/.gemini/config/skills/<n>/SKILL.md` | not checkable | not checkable | not checkable | not checkable | not checkable |

- A `scope` on a kind with one place on that harness (anything but
  claude plugin and MCP `user`, kiro MCP) is not checkable.
- claude MCP reads the user-scope config, not `claude mcp list` / `get`,
  which start each server to health-check it (their help says so).
- The skill dirs are the ones each harness reads: kiro and agy from
  mAId's registry, claude and codex the user-level dirs mAId linked into
  before it shipped a plugin.
- Plugin and marketplace JSON shapes are the ones `deploy.rs` already
  parses; the parsers are shared, generalised from "is `maid@maid`
  there" to "the listed names".

**Report.** Lines grouped harness → part → entry, `state kind name`
with `[scope]` and `(why)` where they apply, then refused files. Exit 1
per the rule above.

## Implementation Plan

- [x] `whole.rs`: reader (lenient structs, refusals, dotfiles skipped) with unit tests.
- [x] Probes per harness and kind, with temp-HOME files and stub CLIs; shared plugin/marketplace listing parsers with `deploy.rs`.
- [x] Report and exit code; `whole` verb in `main.rs`; agent selector; unit tests.
- [x] Justfile: `status --whole` flag and `resources::status-whole`.
- [x] Rollback test: `--whole` after install, with and without a second part's list.
- [x] `project.md`: the verb in Tech Stack, `whole.rs` in Layout, the checks in Testing, how presence is read per harness under "My harness".

## Session Log

- 2026-10-10 · grounded: M1's contract in project.md "My harness" and
  `part.rs` (writer only, no reader yet). `deploy.rs` already parses
  claude's and codex's plugin and marketplace listings. Real CLIs (read
  only): `codex mcp list --json` is an array with `name`; claude's MCP
  listings health-check servers; kiro agents in `~/.kiro/agents/*.json`
  carry `name` and `mcpServers`; `~/.kiro/settings/cli.json` is flat
  keys. In a temp HOME: `claude mcp add -s user` writes
  `~/.claude.json` `mcpServers`; `codex mcp add` shows in `codex mcp
  list --json`; `kiro-cli mcp add` refuses without a login. agy is not
  installed here.

- 2026-10-10 · dev: `whole.rs` with 22 unit tests (temp parts dir, temp
  HOME, stub CLIs on a handed PATH). Six mutations (dotfile filter,
  claude scope filter, skip-first, format check, listing cache, refused
  fails) each turned a test red. `nix develop -c just ci` green (239
  build-tool unit tests). `nix develop -c resources/tests/rollback` passed
  all 16 checks, the three new ones included. Seen from the real CLIs in
  its temp HOME: claude and codex plugin and marketplace present, kiro
  and agy skills present, the browser MCP skipped (no Chrome; kiro no
  sub-agent), exit 0.
- 2026-10-10 · Code Review Gate (correctness lens; no named-risk path):
  PASS WITH NOTES. Fixed: plain `just status` exits non-zero again when
  a step fails (the bash body had lost the linewise stop); `just --list`
  shows `--whole`; a kiro agent with no `name` goes by its file name; a
  config whose `mcpServers` is not an object is unreadable; a list whose
  `part` is not its file name is refused; `--agent` with no entries says
  so; the not-checkable tests fail on an empty result. Also fixed: a
  stub-CLI race (ETXTBSY under parallel tests), so stubs wait until they
  can run. Left: `CLAUDE_CONFIG_DIR` / `CODEX_HOME` pass through to the
  CLIs while file checks read `~`, as in the rest of mAId. `just ci`
  (240 build-tool unit tests) green three runs in a row; rollback test
  all 16 checks green.
- 2026-10-10 · second review cycle (correctness; budget 2 of 2): PASS
  WITH NOTES. Fixed: a skills or agents dir that exists but cannot be
  read is unreadable, not missing; a failed listing names its first
  non-blank stderr line; the empty report says "no entries to check";
  claude plugin scopes other than `user` are not checkable. Documented:
  an entry without a string `kind`/`name` refuses its file; `--whole`
  takes the kiro sub-agent from each entry's scope. Left: no timeout on
  the listing commands (an attended command). `just ci` green (242
  build-tool unit tests), rollback test all 16 checks green.
- 2026-10-10 · review briefing defects: the spec said `(scope)` where the
  report prints `[scope]` (spec fixed); a claude MCP at a non-`user`
  scope gave a false reason (now "claude <scope> MCP servers are not
  read", with a test). Plain `just status` in a scratch HOME: stdout
  identical to the base branch's recipe, and a failing step exits 1 on
  both. `just ci` green (242 build-tool unit tests).
- 2026-10-10 · second briefing defects: just refuses a whole Justfile on
  an attribute it does not know (checked on 1.50 with a made-up one), so
  `--whole` is now spotted positionally and the `[arg]` attribute is gone;
  README lists `status --whole` and `resources::status-whole`;
  `cmd_whole` moved into `whole.rs` so `stages` no longer reaches a later
  module; a test for an entry with no `kind` refusing its file. `just
  ci` green (242), rollback all 16 checks green, plain `just status` still
  identical to the base recipe and still exits 1 on a failed step.
- 2026-10-10 · review rulings applied: `present (disabled)` for claude
  and codex plugins and codex MCP listed as off (does not fail the run);
  an agent file that does not parse is unreadable by its file name, not
  missing (one test; three mutations each turned a test red); the
  lenient-reader pattern noted in project.md. On the final code commit
  0d34943: `nix develop -c just ci` passed (244 build-tool unit tests),
  and `nix develop -c resources/tests/rollback` passed all 16 checks.
- 2026-10-10 · closure: backlog answer none. No backlog item covers
  whole-status or reading part lists, so none is closed; none is filed
  (see Handoff, Deliberately left). project.md verify: Architecture (My
  harness, Whole-status), Tech Stack (verbs), Layout (`whole.rs`),
  Testing (unit and rollback) and Deployment were updated during dev and
  match the code; README lists the verbs. No AGENTS.md in this repo.

## Decision Log

- Unreadable listings and refused files fail the run · status cannot say
  the whole is there when it could not read a list or a listing ·
  rejected: folding them into not checkable, which would exit 0 on a
  broken config.
- `--whole` replaces today's status output rather than adding to it ·
  the two answer different questions; `just status` is unchanged.
- An unscoped claude plugin entry means user scope · that is where a
  part installs from the CLI (mAId does); `scope` names another.
- `whole.rs` beside `Deploy`, not a `Deploy::present(item)` method ·
  `Deploy` is how mAId places its own things; reading any part's entries
  is a different job.
- No `toml` dependency for codex settings · no case today needs it;
  reported as not checkable.
- Amended (review cycle 2): a claude plugin scope other than `user` is
  not checkable · claude lists project and local plugins for the dir it
  runs in, so the answer would depend on the cwd · was: read at that
  scope.
- Amended (review cycle 1): a list whose `part` is not its file name is
  refused · two files with one `part` would merge under one heading and
  hide each other; project.md already says they are the same.
- An entry without a string `kind` and `name` refuses its file · the
  format requires both; documented rather than read leniently.
- Amended (briefing): `--whole` is spotted in the positional arguments,
  not declared with just's `[arg]` attribute · an unknown attribute makes
  just refuse the whole root Justfile (checked: `Unknown attribute`), so
  an older just would lose `install` too · was: `[arg("whole", long=...)]`.
- Ruling (ringmaster, review): present means listed; where the listing
  says disabled (claude and codex plugins, codex MCP `enabled: false`)
  the line reads `present (disabled)` and does not fail the run.
- Ruling (ringmaster, review): a kiro agent JSON or claude agent file
  that does not parse is unreadable (why), keyed by its file name · the
  label must point at the real cause · was: treated as absent, so
  missing.
- Ruling (ringmaster, review): no timeout on listing commands, and the
  `CLAUDE_CONFIG_DIR` / `CODEX_HOME` split, accepted as recorded.
- Ruling (ringmaster, review): a lenient reader beside the strict writer
  is the pattern for part-list readers; one line in project.md.
- Ruling (ringmaster, planning): defaults for unreadable/refused failing
  the run, `--whole` replacing the output, and unscoped claude plugin =
  user scope accepted; kiro MCP by file and agy by path only accepted.
