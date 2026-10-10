# Feature: memory

## Git Setup

- Branch: `feat/memory/work`
- Base: `initiative/my-harness`
- Worktree: `mAId-worktrees/memory`

## Feature Brief

My memory lives in stores I own: plain markdown in git checkouts, in
Claude Code's own auto-memory format. I name the stores I have and pick
one default. Every base harness reaches the default store: Claude Code
writes its automatic memory straight into it, and in codex, kiro-cli and
agy a `memory` skill writes, recalls and lists. A store other than the
default gets only what I explicitly send it. `just memory-harvest` copies
the memory each harness already keeps on this machine into a store,
leaving the originals alone.

Part of initiative: [[my-harness]]

## Handoff

- **Stage:** closed
- **Ready for:** squash-merge into `initiative/my-harness` by the
  ringmaster
- **Carry forward:** none open. The memory contract (config per part,
  default by explicit choice only, `autoMemoryDirectory` owned by its
  recorded value, the shared index's 200-line limit) is in project.md
  Architecture "Memory"; the part list's optional `value` is in its
  format.
- **Deliberately left:** smoke (discovery, integration) and agy were not
  run here; the initiative's Macro test covers the real-HOME path. A
  settings.json link into a read-only tree makes install error rather than
  skip, and codex registry snapshots would pile up as `-N` copies if codex
  memories were turned on: both judged too unlikely to track, so no
  backlog item. Store git sync is the user's by design; agy's include is
  M4's; kiro's session-start load is the agent owner's.

### Crossings

<!-- Append one line per crossing; never edit or delete a line. -->

- research → planning
- planning → dev
- dev → review
- review → closure
- closure → closed

## Requirements

### Stores

- `just memory-store <name> <path> [--default]` adds or updates a store
  in mAId's memory config. A path that does not exist is refused: clone
  the store first. `--default` makes it the default unless another part
  already names a default. Then it refuses and says which part. This is
  the only way mAId names a default: install never picks one.
- `just memory-store-remove <name>` removes a store mAId configured. A
  store another part configured is refused. Removing the default clears
  the default.
- `just status` lists every configured store (from every part), marks
  the default, and reports a store whose path is gone, a name two parts
  both configure, and two defaults. It also says where Claude's automatic
  memory goes.

### Claude Code

- After `just install`, Claude's automatic memory for every project goes
  to the default store's `memory/` dir, when mAId's config names the
  default and that setting was empty or already mAId's. Otherwise install
  says why it did not set it: no default in mAId's config, the default
  store missing, the setting holds another value, or claude is not
  installed.
- When mAId's config no longer names a default (or names one whose path is
  gone) and the setting is still mAId's, install removes it, so the part
  that now owns the default can claim it on its own install.
- `just uninstall` removes the setting only while it is still mAId's.
- mAId's part list records the setting for claude: placed, or skipped with
  one of those reasons (a fixed phrase, no path).

### The memory skill (every base harness)

- Opens with `[memory] applies`.
- "Remember X" writes one memory into the default store: a file with
  `name`, `description` and `metadata.type` (user, feedback, project or
  reference), plus one line in the store's `MEMORY.md`. An existing memory
  on the same subject is updated, not duplicated.
- "Remember X in <name>" writes to that store only. Nothing goes to a
  non-default store unless the user names it in that request; the skill
  never picks a store from the topic.
- "What do you remember about X" (recall) reads the default store's index
  and the memories it points to. "In <name>" reads that store instead.
- "List my memories [in <name>]" prints the store's index.
- An unknown store name, no default, or a store path that is gone: the
  skill stops and says so. It never creates a store or writes elsewhere.
- In Claude Code, when the session's automatic memory already is the
  default store, the skill leaves writes to it to Claude and handles only
  named stores.

### Harvest

- `just memory-harvest [--to <name>]` copies each harness's own memory
  into the store's `memory/` dir (the default store without `--to`):
  Claude's per-project memories, and codex's memories if codex ever kept
  any. Each copied memory gets one line in the store's `MEMORY.md`.
- The originals are never changed or removed.
- A second run with nothing new changes nothing and says so.
- Two different memories with the same file name both land: the second
  gets a numbered name, and the report says which went where.
- `--to` a name that is not configured, or a store whose path is gone, is
  refused before anything is written.
- The report says per source what was copied, skipped as already there,
  and renamed, and warns when the index is longer than Claude loads.

## Test Strategy

### Unit (`just test`, load-bearing)

- Config: the union of every part's file; a duplicate store name and two
  defaults are reported and leave that name (or the default) unusable; a
  file of another format or another part is not rewritten; dotfiles are
  ignored.
- `memory-store`: adds, updates, refuses a missing path, refuses
  `--default` while another part holds it, sets it while empty or mAId's;
  remove refuses another part's store.
- Claude setting: empty -> set; mAId's -> repointed to the new default;
  another value -> left, skipped; other keys and their order unchanged;
  uninstall removes only mAId's value. Each skip reason reaches the part
  list.
- Harvest: copies, second run byte-identical with "nothing new"; originals
  byte-identical (checked by hash before and after); a same-name different
  memory lands as `<stem>-2.md`, deterministically; index lines carried
  over (link rewritten on rename) or generated from frontmatter; an
  existing index line is not repeated; unconfigured `--to` and a missing
  store refused with nothing written; codex's files wrapped as memories.
- Runner: a fixture's `config_home:` reaches the agent's environment.

### Integration (free, temp HOME)

- `tests/memory.rs` runs the build-tool binary under a temp HOME and temp
  config/state dirs: store verbs, status and harvest over seeded native
  memory dirs. `tests/memory_install.rs` calls install's and uninstall's
  claude step against a seeded `settings.json` (its own binary, since it
  sets PATH), and checks whole-status reads the setting as present.

### Skill fixtures (paid, `verify-skills-isolated`)

Each runs claude, kiro and codex against a seeded config under the
workdir (never the real one):

- `memory.smoke` playback: routing (automatic -> default only, named only
  when named, never by topic) and the refusals.
- `memory-write.smoke` enact, behavioral: "remember X" lands one file with
  the frontmatter and one index line in the default store; the second
  store is untouched.
- `memory-named.smoke` enact, behavioral: "remember X in work" lands only
  in `work`.
- `memory-recall.smoke` enact: a seeded memory is recalled
  (`expect_substr`), and a memory only in the non-default store is not.

Activation and discovery come with each. Budget: up to 10 fixture runs;
plus one `claude --print --settings` run that saves a memory into a temp
store, to confirm Claude writes into `autoMemoryDirectory` directly.

## Design

**Config: one file per part, like the part list.** Stores live in
`${XDG_CONFIG_HOME:-$HOME/.config}/my-harness/memory/<part>.json`. A part
writes only its own file (rule 2); readers take the union of every
non-dotfile `*.json`. The default is one value across all files, so a part
sets `default` in its own file only when no other file holds one, or its
own file already does (rule 3). That keeps the work part's store and
default untouched by mAId, and needs no lock or shared file.

```json
{ "format": 1, "part": "maid-harness",
  "default": "personal",
  "stores": { "personal": "/home/me/stores/personal" } }
```

Paths are stored absolute. A store is `<path>`; its memories are
`<path>/memory/`. A missing `<path>` is reported everywhere; `memory/` is
created (and said) on the first write into an existing store. A name in
two files, or two defaults, is a conflict: that name, or the default, is
unusable until fixed, so nothing is routed by guess. The Justfile exports
the dir as `MY_HARNESS_MEMORY`, as it does `MY_HARNESS_PARTS`.

Considered: TOML (a new dependency for a three-key file), env vars as the
notes skill uses (another part could not add a store without editing the
user's shell), one shared file (two parts would rewrite each other). JSON
matches the part list and is read by every agent and `jq`.

**Claude Code: `autoMemoryDirectory` in `~/.claude/settings.json`.**
Checked in claude 2.1.295: the setting's own description reads "Custom
directory path for auto-memory storage. Supports ~/ prefix ... Ignored if
set in projectSettings ... When unset, defaults to
~/.claude/projects/<sanitized-cwd>/memory/", and the docs say it replaces
the per-project dir with the one dir. So every project shares the store's
one index; project-type memories from different repos sit side by side,
which is what a travelling store wants. `MEMORY.md` beyond 200 lines or
25 KB is not loaded at start, hence harvest's warning.

The value is mAId's only when it equals the `value` mAId's part list
records for the setting, so a default moved or removed by the store verbs is
still mAId's to repoint or remove, and a hand-set value is never touched. mAId wants one value: the default store's `memory/` dir
when mAId's own file names the default and that store exists, else none.
When the key is absent or mAId's, install makes it that value, which
removes the key when mAId wants none (rule 2: it is mAId's to remove).
When it holds anything else, install leaves it and records the skip. The
contract text in project.md states this rule so another part implements
the mirror: it sets the key only when its own file names the default and
the key is empty or its own. claude has
no settings command (checked: `claude --help` lists none), so the file is
edited directly: serde_json with `preserve_order`, so other keys keep
their order, written through a temp file and rename. This runs inside
`build-tool install` for claude and records `{kind: setting, name:
autoMemoryDirectory}` beside the plugin entries.

**codex: the skill only.** `codex features list` (0.162) shows `memories`
stable and off by default; mAId leaves it off and writes no codex config.
No global instruction include: mAId installs no global instruction file,
and the skill already covers recall when asked. When enabled, codex keeps
`$CODEX_HOME/memories/` (`MEMORY.md`, `memory_summary.md`,
`raw_memories.md`, `rollout_summaries/`), a consolidated registry rather
than one file per memory; harvest wraps `MEMORY.md` and
`memory_summary.md` as two `reference` memories (`codex-memory.md`,
`codex-memory-summary.md`) and leaves the raw queue.

**kiro-cli: the skill only.** It has no automatic memory. Loading the
index at session start needs a `resources` entry in an agent config,
which mAId does not own; the part that owns the agent adds
`file://<store>/memory/MEMORY.md`. mAId does nothing there.

**agy: the skill, for now.** agy is not installed here, so its rules
include cannot be checked; M4 (agy-verified) owns it.

**The skill** (`resources/content/skills/memory/SKILL.md`) carries the
routing rule at the top, ranked: write only to a configured, existing
store; only the default unless named in this request. Then the config
location and how to read it, the file format (Claude's own: frontmatter
`name`, `description`, `metadata.type`; index line `- [Title](file.md) -
hook`), and write / recall / list. It runs no git: the store is the
user's checkout and syncing it is theirs.

**Harvest** (`build-tool memory harvest`, wrapped as `just
memory-harvest`): sources are `$HOME/.claude/projects/*/memory/*.md`
(minus `MEMORY.md`, and minus the target dir itself) and
`$CODEX_HOME` (default `~/.codex`) `memories/`. Sources are walked in
sorted path order. For each: if a file with identical bytes exists at
`<stem>.md` or `<stem>-N.md`, skip; else write to the first free one.
Same input, same result, and a rerun skips everything. The index line
comes from the source `MEMORY.md` line linking to that file (link
rewritten on rename), else from the frontmatter (gray_matter, already a
dependency). Lines are appended only when no line links to that file.
Originals are opened read-only.

**Runner: `config_home:` fixture header.** Agents run with the real HOME
(auth needs it), so a fixture names a workdir-relative dir and the runner
sets `XDG_CONFIG_HOME` to it for the agent run, setup and assert. The
memory fixtures can then seed a config pointing at stores inside the
workdir, and never see the user's.

## Implementation Plan

- [x] 1. Config module: read the union, conflicts, `memory-store` /
  `memory-store-remove` verbs, status lines; unit tests.
- [x] 2. Claude setting in install / uninstall / status, part list
  entries and skip reasons; `preserve_order`; unit tests.
- [x] 3. Harvest and `just memory-harvest`; unit tests; `tests/memory.rs`.
- [x] 4. Runner `config_home:` header; unit test.
- [x] 5. The memory skill and its four fixtures; `verify-skills-dry`;
  paid runs within budget; the one `claude --settings` check.
- [x] 6. project.md: memory under "My harness" (config, the per-harness
  ways in, the new verbs).

- *Risk note:* an agent's shell may drop `XDG_CONFIG_HOME` (codex filters
  some env vars); a fixture whose agent reads no config fails closed
  (reports no store) rather than writing anywhere, since no real config
  exists on this machine. Checked: `~/.config/my-harness` does not exist.
- *Risk note:* the stream is one feature. If it must split, harvest
  (slice 3) is the seam: it needs only slice 1.

## Session Log

- 2026-10-10 · closure. Backlog answer: none closed (the one memory-named
  item, `kdevkit-durable-facts-to-repo-not-agent-memory`, is about where
  kdevkit records project facts, not about stores); none filed.
- 2026-10-10 · review rulings applied: ownership narrowed to the recorded
  value; store verbs print "run `just install` to apply to claude" when the
  default changes; `memory-refuse.smoke` (unknown store: nothing written
  anywhere) passed on claude, kiro, codex, then again on claude after the
  fixture runner began sending claude's own automatic memory to
  `<workdir>/claude-auto` (`--settings`) for fixtures with `config_home:`.
  Skill rule 4 added: a named or refused write is not also saved to
  Claude's automatic memory; `memory-named` re-passed on claude. Paid: 8 of
  10 fixture runs, 2 of 3 for the refusal. Code review cycle 3 (everything
  after cycle 2): PASS WITH NOTES; medium items fixed (fixtures reaching
  claude's real automatic memory, `memory.smoke` without its own config,
  an unreadable index replaced by harvest), and the lows (reason
  precedence for a hand-set value, lenient read without claude, unrecorded
  value named in the error, quoted verb arguments, a part-list round trip
  test). Final: `just ci` green on 6b8f45c; `resources/tests/rollback`
  19/19 on the same tree before its rebase over a spec-only initiative
  commit.
- 2026-10-10 · second briefing's defects fixed (stale announce list in
  project.md, Test Strategy naming `tests/memory_install.rs`, status
  printing another part's value quoted). Rebased onto M6
  (kiro-global-mcp): root `status` lost its kiro sub-agent argument, so
  `status-memory` follows `status-browser-mcp "{{ agent }}"`. `just ci`
  and `resources/tests/rollback` green.
- 2026-10-10 · briefing returned two defects: a relative store path was
  taken from the checkout root (root `memory-store` now resolves it from
  where just ran; checked by hand in a temp dir), and the playback did not
  cover "no default" and "store gone" (added; `check memory` 6/6, fixture
  run 5 of 10; its LEAK line was another stream's commit). Rebased onto M3
  (vendor-marketplaces): mAId's recorded value is now read through
  `part::placed`, the reader M3 added. `just ci` and
  `resources/tests/rollback` green.
- 2026-10-10 · Code Review cycle 2: PASS WITH NOTES (no high). Two
  medium items (no-claude install dropped the recorded value; harvest
  replaced a linked index and its mode) and Should Fix (rollback test read
  the real memory config, unreadable own file stopped install, empty key
  rewritten) fixed after the pass. Rebased onto M5; whole-status checks
  `setting autoMemoryDirectory` as a top-level key of
  `~/.claude/settings.json`, which matches; test added.
- 2026-10-10 · Code Review (correctness lens), cycle 1: FAIL. Must Fix:
  (1) ownership of `autoMemoryDirectory` was lost once a store verb moved
  or removed the default store; (2) a name conflict made mAId's value read
  as another's; (3) a linked settings.json was replaced by a file, mode
  not kept; (4) a rerun after an interrupted harvest never wrote the
  missing index line. Fixed: the part list's setting entry records the
  value set (`value`, new optional field), ownership is that value or a
  store in mAId's own file; settings written through a link with the mode
  kept; reruns add a missing index line. Should Fix taken: empty/null value
  counts as empty, other_default filters format and part, generated index
  lines flattened, codex read errors reported, harvested files written
  atomically, dry-run tag on the no-claude line, PATH test in its own
  binary. Left: codex registry snapshots pile up as `-N` copies (codex
  memories stay off, so one-time); number formatting in settings.json may
  normalise (no `arbitrary_precision`).
- 2026-10-10 · dev: slices 1-6 built. `just test` 249 passing. Paid,
  check stage only (no install, so the real skill links are untouched):
  `check 'memory*'` 15/15 on claude, kiro, codex (activation, playback,
  three behavioral enacts); 4 of the 10 fixture runs. Its "LEAK" line was
  this branch's own commit plus other streams committing on their branches
  during the run (shared git dir), not a test escaping. `claude --print
  --settings '{"autoMemoryDirectory":"/tmp/.../store/memory"}'` saved a
  memory straight into that dir (MEMORY.md plus one file, no per-project
  subdir, dir created by Claude). Smoke (discovery, integration) not run:
  it needs `just install` on the real HOME.
- 2026-10-10 · recall fixture made behavioral (answer.txt): with
  read-only tools claude and kiro cannot read `$XDG_CONFIG_HOME`, so a
  reply-scored recall would meet no config.
- 2026-10-10 · grounded: claude 2.1.295 (`autoMemoryDirectory`,
  `autoMemoryEnabled` in its settings schema; no `config` command), codex
  0.162 (`memories` feature off; memory file layout from the binary),
  kiro-cli 2.29 (no memory setting; agent configs not mAId's), agy not
  installed. Read Claude memory files read-only: frontmatter `name`,
  `description`, `metadata.{type, node_type, originSessionId, modified}`.

## Decision Log

- Config is one JSON file per part under `my-harness/memory/`, default
  claimed by at most one file · rules 2 and 3 with no shared file ·
  rejected TOML, env vars, one shared file.
- Superseded at review: `autoMemoryDirectory` was first mAId's only when
  it named a mAId store's memory dir (no extra state); the review showed the
  store verbs lose track of it, so the part list now records the value.
- Ownership of `autoMemoryDirectory` is the value the part list records,
  or a store in mAId's own file · store verbs can move or drop the store
  after install wrote the value · rejected repointing from the store verbs
  alone (a hand edit of the config would still lose track).
- Harvest dedupes by bytes and renames `-N` · deterministic and idempotent
  without a hash dependency · rejected a content-hash suffix (std's hasher
  is not stable across releases).
- No git in the skill or harvest · the store is the user's checkout.
- Ruling (ringmaster): default follows explicit choice only; install never
  names a default, only `just memory-store ... --default` does, so mAId
  never competes with a part that names its own.
- Ruling (ringmaster): `autoMemoryDirectory` follows the default store;
  set only when mAId's config names the default and the key is empty or
  mAId's; removed by install when mAId's config no longer names one and
  the key is mAId's. Extended here to a default whose store path is gone,
  same reason: mAId stands behind no value it cannot point at a store.
- Ruling (ringmaster, at review): ownership narrowed to the recorded value
  only; a hand-set value, even naming a mAId store, is "not mAId's" and
  left. Supersedes "recorded value or a store in mAId's own file".
- Ruling (ringmaster, at review): store verbs print "run `just install` to
  apply to claude" when the default changes; they never write settings.
