# Feature: memory-mcp

## Git Setup

- Branch: `feat/memory-mcp/work`
- Base: `initiative/my-harness`
- Worktree: `mAId-worktrees/memory-mcp`

## Feature Brief

My personal memory is served by an MCP server, `pensieve`, bound to my
personal store. After `just install`, Claude Code has it registered as
`personal-memory`: it recalls from the store when it needs to, saves into
it what belongs there (the store's `about.md` says what does), and asks me
when it cannot tell. Claude's own automatic memory is off, so nothing goes
to it instead. Another part registers its own server for its own store the
same way. `just memory-harvest` copies the memories Claude already keeps,
chosen by project, into a store.

Part of initiative: [[my-harness]]

## Handoff

- **Stage:** closed
- **Ready for:** squash-merge into `initiative/my-harness` by the
  ringmaster
- **Carry forward:** the local pensieve pin must be re-pinned to the
  published commit before any PR: backlog `repin-pensieve-published`.
- **Deliberately left:** judged not wanted, so not filed: migration from
  the old shared store config (only installs from this initiative's own
  branch had it), keeping a user's empty `env: {}` on uninstall, and
  harvest re-copying a memory pensieve later edited. codex, kiro-cli and
  agy registration is the initiative's Next round.

### Crossings

<!-- Append one line per crossing; never edit or delete a line. -->

- research → planning
- planning → dev
- dev → review
- review → closure
- closure → closed

## Requirements

### The memory server (Claude Code)

- After `just install`, `claude mcp list` (and `~/.claude.json`) shows a
  user-scope server `personal-memory` that runs the profile's `pensieve`
  on my personal store. A new session recalls from it and saves into it.
- The store path comes from mAId's own config. `just memory-store <path>`
  sets it and refuses a path that does not exist. With nothing set,
  install says "no personal store set; run `just memory-store <path>`" and
  registers nothing.
- No store at that path: install says so, registers nothing, and records
  the skip. It never creates the store or writes anything into it,
  `about.md` included. An earlier mAId registration is then removed, so
  Claude does not start a server on a store that is gone.
- A `personal-memory` that is not mAId's (another command), or that
  another part's list records, is left alone and reported.
- The store path changed: install re-registers on the new path.
- `just uninstall` removes the registration only while it is mAId's.
- claude not on PATH: skipped and recorded, nothing written.

### Claude's automatic memory

- After `just install`, while mAId's server is registered, Claude's
  automatic memory is off for every project, through Claude's user
  settings, under rules 2 and 3: set when absent or
  already mAId's, adopted when already set to exactly what mAId writes and
  no other part's list records it, left alone (and reported) when it holds
  anything else or another part claims it.
- `just uninstall` removes it only while it is mAId's.
- With no mAId server registered after install (no store, store gone,
  another part's), the setting is left alone, or mAId's value is removed.
- A value of Claude's memory dir setting that mAId recorded is removed by
  install and by uninstall; any other value is left alone.

### Status

- `just status` shows the configured store (and whether it is there), the
  registration (mAId's or not), and the automatic-memory setting.
- `just status --whole` shows `mcp personal-memory` and the setting for
  claude as present after install.

### Harvest

- `just memory-harvest --to <dir> [--project <path-prefix>]` copies
  Claude's per-project memories (and codex's, when no `--project`) into
  `<dir>/memory/`, as today: originals untouched, a rerun changes nothing,
  a same-name different memory lands as `<stem>-N.md`. With `--project`,
  only projects whose working directory starts with the prefix. `<dir>`
  must exist; `memory/` is created inside it if missing.

### Removed

- The `memory` skill and its smoke fixtures.
- The shared memory config (`my-harness/memory/*.json`), its default, and
  `memory-store <name> <path> [--default]` / `memory-store-remove`.

## Test Strategy

- **Unit (`just test`, load-bearing):** the registration decision (absent,
  mAId's same, mAId's other path, not mAId's, another part's, store gone,
  no claude); the automatic-memory setting (absent, mAId's, identical and
  adopted, identical but another part's, on); legacy memory-dir removal;
  harvest `--project` prefix matching against Claude's project dir names;
  the store config read/write.
- **Integration (free, temp HOME):** build-tool install and uninstall with
  a stub `claude` that records its `mcp add/remove` arguments and edits a
  temp `~/.claude.json`; asserts the part list, the settings file, and
  that `status --whole` reports both entries present. Nothing under the
  real HOME or store is touched: HOME, XDG dirs, parts dir and the store
  config are all temp.
- **Profile:** `nix build .#default` has `bin/pensieve`, and it answers
  `initialize` on a temp store with that store's `about.md` as
  instructions (free, no model).
- **Paid:** none planned. At most one `claude --print` run, only if a gate
  asks for proof that a registered server is used (the research runs
  already showed it with the prototype).

## Design

- **flake.** New input `pensieve` (`git+file` at 146f867 for now,
  `inputs.nixpkgs.follows = "nixpkgs"`); the profile links
  `${pensieve}/bin/pensieve` as `$out/bin/pensieve`. Same shape as the
  browser runtime: a pinned closure, no runtime fetch.
- **Store config.** One plain file, `${XDG_CONFIG_HOME:-~/.config}/maid/memory-store`,
  holding the store path, beside the browser allowlist. The Justfile
  exports its path as `MAID_MEMORY_STORE` so tests and the rollback test
  can move it. Absent or empty file: no store. `build-tool memory
  set-store <path>` writes it atomically; `just memory-store <path>` wraps
  it.
- **Registration** lives in build-tool (`memory.rs`, rewritten), not shell:
  it needs the store config, the part list and ownership checks, which
  build-tool already has. It reads `mcpServers.personal-memory` from
  `~/.claude.json` and decides; it changes claude only through `claude mcp
  remove -s user` / `claude mcp add -s user personal-memory --
  <profile>/bin/pensieve --store <path>`, as the browser server does
  (claude's `mcp add` refuses a name that exists, so remove first). The
  entry is mAId's when its `command` is the profile's `bin/pensieve`
  (the kiro browser rule); `--store` differing means re-register. Another
  part's list recording `mcp personal-memory` placed means skip, `another
  part's`. Recorded with `part::record` as kind `mcp`.
- **The setting** is `env.CLAUDE_CODE_DISABLE_AUTO_MEMORY = "1"` in
  `~/.claude/settings.json`, recorded under that dotted name with value
  `"1"`, so rule 14 checks it as a string. Same read/atomic-write helpers
  as `reads.rs`, and the ownership and adoption logic of today's
  memory-dir setting (recorded value, `adopted`, `claimed_by_others`).
  Uninstall drops `env` when removing mAId's key leaves it empty. A
  non-object `env` is left alone and recorded skipped.
- **An unreadable `~/.claude.json`** (not JSON, not an object, cannot be
  read) fails install's registration step, as an unreadable part list
  does.
- **Legacy memory dir.** Install and uninstall remove
  `autoMemoryDirectory` only when it equals the value mAId's list
  recorded, then drop that entry from the list. Nothing else of the old
  setting code stays.
- **Harvest.** `harvest(home, codex_home, dir, project)`; the candidate
  walk is today's. A prefix matches a project when Claude's encoded dir
  name (every character that is not a letter or digit as `-`) starts with
  the prefix encoded the same way. The encoding is lossy (`a-b` and `a/b` look alike), so a prefix can
  over-match; harvest prints each project it takes.
- **Removed code.** `Config` (the union, `default`, `resolve`),
  `add_store`/`remove_store`, `MY_HARNESS_MEMORY` and
  `memory_config_dir`, the skill and five `memory*.smoke` fixtures.
- **Contract text.** project.md "Memory" is rewritten: each part registers
  its own memory server bound to its own store (no shared store config);
  the server and its store format are pensieve's; Claude's automatic
  memory is turned off by whichever part gets there first, the others
  adopt or skip under rules 2 and 3.

## Implementation Plan

- [x] 1. flake: `pensieve` input pinned to the local commit; `bin/pensieve`
      in the profile; profile check that it answers `initialize`.
- [x] 2. Remove the `memory` skill and its smoke fixtures.
- [x] 3. Store config: `MAID_MEMORY_STORE` file, default path,
      `memory set-store` and `just memory-store <path>`; remove the shared
      config, the default and the old verbs.
- [x] 4. Registration of `personal-memory` on install, uninstall and
      status; part-list record; stub-claude integration test.
- [x] 5. `env.CLAUDE_CODE_DISABLE_AUTO_MEMORY` on install, uninstall and status; legacy
      `autoMemoryDirectory` removal.
- [x] 6. Harvest `--to <dir> [--project <prefix>]`.
- [x] 7. `status --whole` integration check for both entries.
- [x] 8. project.md "Memory" and README rewritten; rollback test's env.

## Research

All experiments ran under `/tmp` with a temp HOME and temp stores. Versions
are as of 2026-10-12. "Docs" means read, not run.

### 1 · Is there a server we can just depend on?

| | basic-memory | reference `server-memory` | mem0 / OpenMemory | Letta MCP | mcp-memory-service | reference `server-filesystem` |
|---|---|---|---|---|---|---|
| Version | 0.23.2 (PyPI) | npm 1.0.0; nixpkgs `mcp-server-memory` 2026.1.26 (reports 0.6.3) | mem0ai 2.2.1 | letta-mcp-server 3.0.3 (npm), 1.0.5 (PyPI) | 11.15.0 | nixpkgs `mcp-server-filesystem` 2026.1.26 |
| Storage | markdown files plus a SQLite index; adds its own frontmatter | one JSONL knowledge graph (entities, relations, observations) | vector store plus LLM fact extraction | a Letta agent server's memory blocks | SQLite with sqlite-vec | any files |
| Two instances, own store | yes, run: `BASIC_MEMORY_CONFIG_DIR` + `BASIC_MEMORY_HOME` per instance | yes, run: `MEMORY_FILE_PATH` per instance | needs a server stack each (docs) | needs a Letta server (docs) | one DB path each (docs) | yes: allowed dirs are args (docs) |
| State in HOME | yes, run: fastmcp writes `~/.local/share/fastmcp/version_cache.json`; HF cache dirs under `~/.cache` | none, run | - | - | - | none (docs) |
| Offline | no by default, run: downloads an embedding model at first index, checks PyPI for updates, telemetry on unless `BASIC_MEMORY_NO_PROMOS=1`, auto-update on unless `auto_update: false` | yes | no: needs an LLM (OpenAI default) and embedder (docs) | no: LLM agents (docs) | yes, local ONNX (docs) | yes |
| License | AGPL-3.0-or-later | MIT | Apache-2.0 | Apache-2.0 | Apache-2.0 | MIT |
| Pinned, no runtime fetch | not in nixpkgs; 170 Python packages (603 MB venv), `fastmcp==4.0.0b1` beta pin, upstream install is `uvx --prerelease=allow`; its manylinux wheels did not load on a nix Python without adding `libstdc++` | yes: in nixpkgs, built from our locked rev | no | no | no (PyPI only) | yes: in nixpkgs |
| Tools | 21: `write_note`, `read_note`, `edit_note`, `search_notes`, `list_directory`, `recent_activity`, `build_context`, project and schema tools… | 9 graph verbs: `create_entities`, `search_nodes`, `open_nodes`, `read_graph`… | add/search memories | agent and block verbs | store/retrieve | read/write/list files; search is by file name |
| `instructions` settable or from the store | no, run: one hard-coded string, "the user's personal knowledge base", same for every instance | none sent, run | - | - | - | no |
| Maturity | active, frequent releases, beta deps | reference sample | active, cloud-first | moved to a new harness repo | active | reference sample |

What I ran:

- basic-memory: two concurrent instances, each its own config dir and
  store. A wrote "Coffee grinder", B wrote "Standup time". A's search for
  "standup" and B's for "grinder" both returned "No results"; each read its
  own note. Isolated. A Claude-format memory dropped into A's store was
  found after its background index caught up (the first search, right after
  start, missed it), and basic-memory **rewrote the file**, adding
  `permalink:` to its frontmatter. Its own notes are written with `title`,
  `type`, `permalink`, `tags`, not Claude's `name`, `description`,
  `metadata.type`.
- reference server: two concurrent instances, two JSONL files. Each search
  saw only its own entity; HOME stayed empty. No `instructions` in the
  `initialize` reply.

**Finding: nothing fits.** The two runnable candidates isolate cleanly, but
neither lets a server state its persona: basic-memory tells every instance
it is "the user's personal knowledge base" (wrong for the work server, and
paid run 2 below shows the instructions are what route a save), and the
reference server says nothing. Neither keeps Claude's format: one rewrites
the files, the other is a graph in JSONL. basic-memory also fails the
profile's rules (runtime model fetch, update checks, HOME state, a beta
dependency, not in nixpkgs). The rest need an LLM or a server stack.

What does exist to build on: the MCP SDKs. nixpkgs at our locked rev has
`python3Packages.mcp` 1.26.0; crates.io has `rmcp` 3.5.1 (the Rust SDK).
A throwaway stdlib-Python prototype of the server below was about 100
lines.

### 2 · Bringing memory into the agent loop

| Mechanism | What it does | Coupling | Cost per turn |
|---|---|---|---|
| Server `instructions` | Sent at `initialize`; Claude puts it in the system prompt even with tools deferred (docs; and this session's own prompt shows a "MCP Server Instructions" section). Claude truncates at 2,048 chars; codex reads it, keep the first 512 chars self-contained (codex docs) | none: plain MCP, the server writes it | in the cached system prompt; a few hundred tokens per server |
| Tools, deferred | Claude loads only tool names; the model fetches a schema with tool search when it needs it (docs; seen in both paid runs) | none | names only until used |
| Tool descriptions | Say which persona each tool serves | none | only once fetched |
| MCP prompts | User-run slash commands, e.g. `/mcp__personal-memory__remember` (docs) | none | zero unless run |
| MCP resources | User `@`-mentions, e.g. the index (docs) | none | zero unless used |
| SessionStart hook | `command` hook adds `additionalContext` before the first prompt; an `mcp_tool` hook is skipped at launch because servers are not up yet (docs) | a hook entry in each harness's settings; codex has the same event (docs) | once per session |
| UserPromptSubmit hook | Runs on every prompt; can inject top hits as `additionalContext` (an `mcp_tool` hook can call the server's search with `${prompt}`); 30 s default timeout, output capped at 10,000 chars (docs) | per-harness hook config; codex has it (docs); kiro unverified | a server call and injected tokens on every prompt, uncached |
| PreToolUse / PostToolUse | Context around a tool call | per harness | per tool call |
| Stop hook | Can return `additionalContext` and keep the turn going: a "save what you learned" nudge (docs) | per harness; codex and kiro have a stop event | one more model turn when it fires |
| Sub-agent | A memory agent with its own context | an agent file per harness | a sub-agent run per use |

**Recommendation: instructions and tools only, no hooks in the first cut.**

- (a) Standing instruction and profile: the server builds its
  `instructions` at start from a file in its store (what belongs here, a
  short profile), plus a fixed line on when to search and when to save.
- (b) Recall on demand: the agent calls the server's search when the
  instructions tell it to. Both paid runs did this with nothing else.
- (c) Where a new memory goes: each server's instructions say what belongs
  in it and what does not, and say to ask when it is unclear. Run 2 split
  two facts correctly. An ambiguous fact (one that should make it ask) was
  not tested.

This is plain MCP, so the same servers serve codex, kiro-cli and agy with
only a registration. Hooks stay as later options, each a backlog item to
add only if recall or saving is seen to fail: a Stop nudge to save, then a
per-prompt auto-recall.

### 3 · Paid confirmation (2 runs, `claude --print`, temp HOME)

Setup: two stores (`personal`, `work`), each with a persona file and one
seeded memory in Claude's format; the prototype server registered twice
with `claude mcp add -s user`, as `personal-memory` and `work-memory`.
Claude's automatic memory was off (`autoMemoryEnabled: false`). The prompts
did not mention memory or servers. Claude Code 2.1.296.

- **Run 1, recall** ("What grind setting do I use for pour-over, and when
  is my team's standup?"): tool search for both search tools, then
  `mcp__personal-memory__memory_search {"query":"pour-over grind setting"}`
  and `mcp__work-memory__memory_search {"query":"team standup time"}`.
  Answer: "18 clicks … 09:30 on weekdays." 4 turns, USD 0.063.
- **Run 2, save** ("Remember … on-call handoff moved to Mondays at 10:00,
  and I've started learning the cello"):
  `mcp__work-memory__memory_write` (on-call, type `project`) and
  `mcp__personal-memory__memory_write` (cello, type `user`). Each file
  landed only in its own store, with its index line. 4 turns, USD 0.068.

Both servers' logs show only the calls above. Two of the 25 paid runs used.

## Session Log

- 2026-10-12: fast-forwarded the branch to `initiative/my-harness` (it was
  cut one commit before the M10 ruling).
- 2026-10-12: basic-memory and the reference server each run twice side by
  side in temp HOMEs; prototype server and two paid runs as above. ARCC
  guidance was not queried (its MCP server is not available in this
  session); the server design follows the usual rule of confining file
  access to the store.
- 2026-10-12: research accepted; the server is the standalone package
  pensieve, not ours. Rebased onto `initiative/my-harness` (6d220d7).
- 2026-10-12: built. Free checks: `just ci` (fmt, clippy, all tests); the
  browser MCP registration test; `build-tool install/whole/uninstall
  --agent claude` against the real claude CLI in a temp HOME (registered,
  `--whole` present for both entries, uninstall left only what was there);
  the profile's `bin/pensieve` answered `initialize` with the temp store's
  `about.md`. Mutation checks: dropping the claimed-by-another-part guard,
  the missing-store guard, or the uninstall removal each fails a test.
- 2026-10-12: found while building: `part::record` replaced every `mcp`
  entry of a harness, so the browser step and the memory step would erase
  each other's entry. Added `record_named` and `build-tool record --only`;
  the browser script now passes `--only chrome-devtools`.
- 2026-10-12: Code Review Gate (correctness lens, the path rule's single
  lens): PASS WITH NOTES. Fixed: the medium (a skipped `autoMemoryDirectory`
  record from an older install was never cleared) and three lows (a failed
  `claude mcp remove` aborted the step; uninstall removed an entry another
  part claims; re-point after a failed remove). Left as noted in Handoff.
- 2026-10-12: ringmaster rulings applied (no default store; Claude's memory
  off only with mAId's server). Final checks on 9f2850e: `just ci` green;
  `resources/tests/rollback` (free, real claude and codex in a temp HOME)
  all checks passed.
- 2026-10-12: closure. Backlog answer: no existing item is closed by this
  feature; filed `repin-pensieve-published` for the local pin.

## Decision Log

- Considered depending on basic-memory (the closest fit) or the reference
  server; chose a server of our own, because neither lets a server state its
  persona or keeps Claude's format, and basic-memory fetches at runtime and
  is not in nixpkgs.
- Superseded by the user's ruling: the server is `pensieve`, a standalone
  public package both parts depend on, like the browser runtime.
- Registration in build-tool, not shell like the browser server: it needs
  the part list and ownership checks build-tool already has.
- Claude's automatic memory is turned off with
  `env.CLAUDE_CODE_DISABLE_AUTO_MEMORY = "1"`, not `autoMemoryEnabled:
  false`: rule 14 compares setting values only as strings, so the boolean
  would need a contract change in both readers. Checked: Claude's settings
  reference says user-settings `env` applies at startup; the binary turns
  auto memory off when that variable is truthy (`1`, `true`, `yes`, `on`);
  one paid run (temp HOME, USD 0.08) saved a fact through
  `personal-memory` and left no Claude memory dir.
- `just memory-store <path>` stays as the way to set the store (user).
- An unreadable `~/.claude.json` fails install (user).
- The part list's writer replaced every `mcp` entry of a harness at once,
  so the browser step and the memory step would erase each other's entry.
  Added `part::record_named` and `build-tool record --only <name>`; the
  browser script passes `--only chrome-devtools`. Considered a second kind
  for memory servers; chose a name filter because the format stays as is.
- Harvest's prefix encoding follows Claude's own (checked in its binary:
  every character that is not a letter or digit becomes `-`), not the `/`
  and `.` the plan first named.
- Ruling (ringmaster): no built-in default store. A personal path does not
  belong in a public repo's code; with none set, install says so and
  registers nothing.
- Ruling (ringmaster): Claude's own memory is turned off only while this
  part's own server is registered after install; otherwise mAId leaves the
  setting, or gives back its own value. A part with its own server sets it
  the same way, so one of them does whenever any server is present.
