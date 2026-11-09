# Feature: sleep-and-adaptation

## Git Setup

- Branch: `feat/sleep-and-adaptation/work`
- Base: `initiative/harness-core`
- Worktree: `mAId-worktrees/sleep-and-adaptation`

## Feature Brief

A session can now teach the next one. When the person closes a session,
or asks for a reflection mid-way, or the session simply ends, it is marked
for sleep; the core packs it (and its sub-agents) into one experience; a
separate reflector that knows how the harness is built reads it and
proposes what should change; the person's guards check every proposal;
and the person decides them in one short batch. Habits they accept are
written where each bring-in's config points; changes to a bring-in, a
skill or the harness become proposals on their source; nothing borrowed
is touched. A ledger counts corrections, refusals and findings by thing,
so the ghost can tell whether it is getting better.

Part of initiative: [[harness-core]]

## Handoff

- **Stage:** closed
- **Ready for:** the ringmaster's squash-merge into
  `initiative/harness-core`
- **Carry forward:** none open here. Retention of stored sessions and
  marks is `backlog/adaptation-retention.md`; the harness's proposal
  source is `backlog/harness-source-declared.md`. Macro step 3's skill
  edit (0 of 4 real reflector runs proposed one) is the ringmaster's, at
  the Macro test.
- **Deliberately left:** a scheduled sweep and other shells' session
  sources are the initiative's "later"; `promote: auto` acting as
  `propose` and the core leaving contradiction-by-meaning to the
  reflector are choices, not wanted work.

### Crossings

<!-- Append one line per crossing; never edit or delete a line. -->

- planning → dev
- dev → review · EXCEPTION · skipping: the SessionEnd key in the
  plugin's hooks file (plan item open) and the push · why: the hooks
  file is stream 2's and not merged; the ringmaster pushes nothing
- review → dev · RETURN · fault: implementation · issue: a habit's entry
  can be longer than a one-line batch read covers, marked sessions can
  only leave by a paid reflection, the briefed copy is kept forever, and
  the ledger's "proposed" counts only decided items · fix: cap an entry
  at 200 characters, a "skip" answer and `{"dismiss": ...}`, delete the
  briefed copy once a batch is decided or dismissed, rename the column
  "decided" · done when: one test per part, and the integration test
  passes with only the expected files changed
- dev → review · EXCEPTION · skipping: the SessionEnd key (plan item
  still open) and the push · why: stream 2's hooks file is not merged;
  the ringmaster pushes nothing
- resolved: the dev → review EXCEPTIONs above · the SessionEnd key
  landed after stream 2 merged (`4ffdbb3`), through its hook harness
  and the live stub test; the push stays the ringmaster's
- review → closure
- closure → closed

## Requirements

### What the person does and sees

- **Close or reflect.** In a Claude Code session, the person says "close
  this session", "reflect", "reflect on this session" or "what did we
  learn". The reflection skill marks the session for sleep, packs it, has
  the reflector read it, and shows **one decision batch**: numbered
  items, one line each, every item saying what it changes, where, for
  which persona, and the evidence it quotes from the session. The same
  works mid-session and routes the same way (the experience is what has
  happened so far).
- **The session ends without a word.** When a Claude Code session ends,
  it is marked for sleep and nothing else happens. The next session's
  genome says how many sessions wait to be reflected on; "reflect on the
  waiting sessions" takes them one at a time.
- **Deciding.** The person answers the batch: "ok" (every item that does
  not need its own answer), item numbers ("1 and 3 yes, 2 no"), a persona
  for an item that asks which persona it belongs to, or "none". Items
  that need their own answer say so in the batch: anything learned only
  from tool or web content, a contradiction with what a memory already
  holds, and a guess about which persona a learning belongs to.
- **What is applied after a yes**, and nothing else:
  - a **persona habit** is written to (or removed from) that persona's
    memory through the memory's own server, so the store is left exactly
    as the server's own write leaves it; once the person syncs the store,
    another environment that pulls it sees the habit;
  - a **bring-in's habit** is written to (or removed from) that
    bring-in's local tier, at the location its part binds;
  - a **change to a bring-in, a skill or the harness** (the reflector and
    the core included) becomes a proposal on its source: a backlog item,
    or a worktree-to-PR proposal naming the source repo and revision. No
    repo, checkout or skill file is changed;
  - a learning about a **borrowed** bring-in becomes a note for its
    lender, or a proposal to stop borrowing it. The borrowed bring-in and
    its tiers are never changed;
  - a "no" changes nothing and is remembered as declined.
- **Already there.** A proposed habit already in its tier (for example a
  rule the person taught writing-style during the session) is listed as
  `already in <tier>` with no action, so it is seen and not dropped.
- **Guarded before the person sees it.** A proposal whose evidence is
  not found word for word in the session, that names nothing in the
  harness, or that would write where its target has no tier, is dropped,
  and the batch says how many were dropped and why. A proposal the
  reflector cannot place on a persona is never written anywhere; it is
  asked.
- **Pending stays true.** Until the person decides, each undecided item
  counts in the genome's pending line; deciding clears it.
- **Getting better.** `maid-core ledger` shows, per thing (a bring-in, a
  skill, the harness, a tool), corrections, refusals and interruptions
  seen, and findings proposed and accepted, across sessions.

### What a carrier calls

- `maid-core sleep`, `maid-core experience` and `maid-core adaptation`
  take stream 1's call shape (`--session`, `--carrier`, `--reset`,
  `--input`, `--json`). A hook that hands its own JSON on stdin
  (`--input -`) need not name the session: its `session_id` is used.
- `sleep` marks a session (from the end event, or from a close or a
  reflect), and says so; marking twice keeps the first mark's source and
  records the later reason.
- `experience` packs a marked or named session into its experience and
  answers where it is and what it holds, in a few lines; with no session,
  it lists the sessions waiting to be reflected on.
- `adaptation` with no input answers the reflector's brief for the
  session; with findings, the decision batch; with decisions, what was
  applied.
- On a shell with no session source yet, each says so and changes
  nothing.

## Test Strategy

Free first: everything below except "Paid" runs in `just test`, against
temp dirs and fixture transcripts, with a reflector stub (a fixed findings
file) in place of the model. Each test names the change that would break
it and is watched failing before it passes.

### Unit (`just test`, load-bearing)

- **Claude Code packer**, on real-shaped fixture transcripts
  (`resources/core/tests/fixtures/claude/`): the person's words and the
  agent's text land in *said*; skill bodies, hook context and compaction
  summaries in *told*; tool calls in *did*; file paths and URLs in
  *touched*; interrupts, rejected tool uses, tool errors and a repeated
  failing call in *friction*. Sub-agent transcripts (their own files, and
  inline side chains) become sub-sessions. A text over the cap keeps its
  head and a pointer (file and line). An unknown line type is skipped; a
  line that is not JSON is counted, not fatal.
- **Locating a session**: by the end event's transcript path (refused
  outside the transcripts root, or not named `<session>.jsonl`); by id
  under the root; with no id, the newest transcript for the working dir.
- **Sleep marks**: written once per session, the later reason recorded;
  a mark for a session id that could escape its dir refused; the waiting
  list holds marked sessions whose batch is not decided.
- **Guards**, one test each: no evidence, or a quote not in the session →
  dropped; a quote under 8 characters → dropped; a target not in the map
  → dropped; a habit for a target with no tier → dropped; a nature
  finding is never written, only proposed; a borrowed target gets a note
  only, whatever the finding asks; evidence only from tool or web content
  → needs its own yes; a persona other than the target's, or no target
  and several memories → asked, never written; a contradiction → needs
  its own yes; more than 10 findings → the rest dropped as over the cap.
- **Routes**, after a yes: a persona habit is sent to the bound server
  as memory-tool calls (a stub server records them: one create, one index
  insert; a remove sends a delete and an index edit), and a server that
  fails or is not bound leaves the item pending with the reason; a
  local-tier habit puts one dated entry under the file's heading, newest
  first, a remove takes out only that entry; an entry already in its tier is listed `already in <tier>` and
  changes nothing; a nature finding writes one backlog-shaped proposal naming the
  source and revision; a borrowed one appends to the lender's note.
- **Deciding**: "ok" applies every item that does not need its own answer
  and leaves those pending; per-item answers; a persona answer places an
  asked item and writes it; "none" declines all; the pending count is
  the number of undecided items, 0 once all are decided.
- **Ledger**: a reflection appends what it saw, a decision one record per
  item; counts by thing
  take the last friction and corrections per session and every finding;
  a session reflected twice is not counted twice.
- **Content**: the shipped hooks file runs `sleep` on SessionEnd with the
  hook's input; the reflection skill and the reflector agent validate
  (`shipped_content_validates`), and the skill carries its declaration.

### Integration (free)

- `resources/core/tests/adaptation.rs` drives the binary under a bare
  environment (`env -i HOME=…`) through Macro test steps 3 and 4 with
  the reflector stub: a fixture HOME with mAId's part (personal memory on
  a temp store, a skill, a writing-style-shaped bring-in with pending and
  permanent tier files, a borrowed overlay) and a fixture second part; a
  fixture transcript where the person corrects the agent once and teaches
  a rule; `sleep` fed a SessionEnd-shaped stdin; `adaptation` gives the
  brief, then (stub findings) one batch of a persona habit, a local-tier
  entry and a skill edit as a backlog item; "ok" writes the memory and the
  entry where their config points (the memory through the real pensieve
  from the dev shell's flake input, run as the bound server), files the
  proposal, and a snapshot of the temp tree shows nothing else changed.
- **Travel** (Macro step 5's half this stream owns): the fixture store is
  a git repo with a bare remote; after "ok", the test does the person's
  sync (commit, push), a second clone pulls, and its index and memory
  file hold the habit. Step 4: the same from a
  "reflect" mark mid-session routes the same way.
- `nix develop -c resources/tests/rollback` gains: the built plugin holds
  the SessionEnd hook and the reflector agent, and the reflection skill
  names a core that runs;
  `claude plugin validate` passes the built plugin (as before, not
  `--strict`).

### Paid (approved: P1 and P2, 4 runs, after the free gates pass)

- **P1 (1 run):** one short `claude -p` session in a scratch HOME with the
  built plugin: settles that the real transcript packs with no unknown
  shape the packer depends on, that SessionEnd fires under `-p` and marks
  it, and that the skill's session-id substitution reaches the core.
- **P2 (3 runs):** the reflector agent on the Macro step-3 fixture
  experience: settles that a real reflector returns findings the guards
  keep (verbatim quotes, the right targets), 3 runs for the ratio.
- A failed run stops the series; the ringmaster decides any re-run. P3
  (the close flow by resume) is not run: Macro step 3 covers it.

## Design

**Why this shape.** The guards are the person's, so they are code in the
core, not prose an agent may skip: the reflector only proposes, as data;
the core checks, batches, routes and writes. The reflector is the one
part that needs a model, so it is a sub-agent the session dispatches with
a read-only toolset; it never writes. Everything else is deterministic
and testable with a stub. Routing is read off the harness map stream 1
derives (each bring-in's part, persona, source, tenure, route, state keys
and bound values), so no route is configured twice. Considered running
the reflector from the SessionEnd hook (`claude -p` in the background):
rejected, it would put a launcher behind an end event, spend a model run
per session unasked, and decide nothing without the person.

**Where things are.** All under the core's state dir
(`MY_HARNESS_STATE`), so every location stays config:

```
<state>/sleep/<session>.json          the mark: source, transcript, cwd, reasons, decided
<state>/experience/<session>.json     the packed experience (the bundle)
<state>/pending/<session>-<n>.json    one undecided item each (stream 1's count reads these)
<state>/proposals/<thing>/<date>-<slug>.md   nature proposals, backlog-item shaped
<state>/notes/<lender>.md             notes for lenders
<state>/briefed/<session>.json        the session in full as the brief showed it
<state>/ledger.jsonl                  what each reflection saw; each decided item
```

Claude Code's transcripts root is Claude Code's own config:
`$CLAUDE_CONFIG_DIR/projects`, default `$HOME/.claude/projects`.

**Modules** (in `resources/core/src`, dependencies down the list, after
stream 1's):

- `source.rs`: the session-source interface. `trait SessionSource { fn
  locate(&self, session: Option<&str>, input: &Value) -> Result<Located>;
  fn pack(&self, &Located) -> Result<Bundle>; }`, and `source::for_name`
  (`claude-code`, the default and only one; another name answers "no
  session source for <name> yet"). `Located` is the session id, the main
  transcript and its sub-session files.
- `claude.rs`: the Claude Code source. Locate: the input's
  `transcript_path` must be a regular file named `<session>.jsonl` under
  the root; else `<root>/*/<session>.jsonl`; with no id, the newest
  `*.jsonl` in `<root>/<project dir name of cwd>/` (the naming rule moves
  here from `harvest.rs`, which re-exports it, as `find_on` moved).
  Sub-sessions: `<root>/<proj>/<session>/subagents/agent-<id>.jsonl`
  (documented), plus any inline lines with `isSidechain` (older
  versions, undocumented), grouped by agent id. A project dir name over
  200 characters is truncated with a hash by Claude Code, so the
  no-id fallback finds nothing there and says so. Pack, line by line, with
  `serde_json::Value` (the transcript format is not a stable schema, so no
  strict structs): `user` text not meta → said (person), with interrupt
  markers → friction; `user` meta (a skill body), `attachment`, `system`
  (hook output, compaction boundary) and compaction summaries → told;
  `assistant` text → said (agent); `tool_use` → did, its `file_path`,
  `path`, `notebook_path`, `url` inputs → touched, a `Skill` call → told;
  `tool_result` with `is_error` → friction error, or refusal when it
  carries Claude Code's rejection text; a tool call equal to an earlier
  failed one → friction retry. Other types skipped.
- `experience.rs`: the bundle. `Bundle { session, source, transcript,
  said, told, did, touched, friction, sub_sessions: Vec<Bundle>,
  skipped_lines }`; every entry has `at` (`<file>:<line>`) and a text
  capped at 400 chars (head kept, `truncated: true`). `pack(session)`
  writes `<state>/experience/<session>.json` atomically and answers a
  summary item. `texts(located)` re-reads the full texts with where each
  came from (person, agent, told, tool), for the evidence guard.
- `sleep.rs`: marks. `mark(session, source, transcript, cwd, reason)`
  creates the mark with `create_new` (first source wins) or adds the
  reason; `waiting()` lists marks not `decided`. The genome's pending
  line gains "n sessions to reflect on" when there are any (one line in
  `entry::render`).
- `adapt.rs`: findings, guards, batch, routes.
  - The brief (no input): packs if needed, and answers the reflector's
    packet: the bundle path, the targets (every bring-in in the map with
    its part, persona, scope, tenure, route, tiers and their bound
    locations, plus `harness`), what this session already decided (from
    the ledger), and the findings schema.
  - Findings (`{"findings": [...], "corrections": [...]}`), each `{thing,
    kind: habit|nature, tier: local|permanent, op: add|remove,
    persona, guess, title, type, change, entry, edit, evidence: [{quote,
    at}], contradicts, route: backlog|worktree}`. The guards run in
    order: cap (10), evidence (every quote at least 8 chars, found in the
    session as the brief stored it, after whitespace is collapsed), shape
    (one-line change, entry, title, type; a known type; no `[`/`]` in a
    title, no `](` in a change), target (in the map, or `harness`), tier
    (a habit's tier is a state key its target declares and binds), tenure
    (borrowed → note only), persona (target's part persona, else ask),
    source of evidence (a habit with any quote not the person's → own
    yes), contradiction (own yes). The route comes from the map: habit → write; nature → the
    recorded route, `worktree` only when the finding carries a concrete
    edit, else backlog; borrowed or report → note. Survivors become
    pending files (any earlier undecided ones for the session are
    replaced); the batch is rendered from them.
  - Decisions (`{"decide": "ok" | "none" | {"<n>": "yes"|"no"|{"persona":
    "<name>"}}}`): applies each yes by its route, removes decided pending
    files, sets the mark `decided` when none is left, appends one ledger
    record per decided item. Every answer is checked before any is
    applied, and each item is placed again on the map as it is then.
  - Writes. A persona memory (a bring-in of standard `mcp` whose
    permanent tier is a store) is written through its own server: the
    core starts the command the part binds as `serve` (a JSON array of
    strings), speaks MCP over stdio (`initialize`, then `tools/call` of
    the memory tool's `memory_create`, `memory_insert`,
    `memory_str_replace`, `memory_delete`), one request at a time,
    stopping at the first that fails, within 30 s; then it closes stdin
    and stops the server after 2 s. The memory is a file
    `/memories/<slug>.md` with frontmatter `name`, `description`,
    `metadata.type` and the entry, plus one index line `- [Title](<slug>.md)
    - <hook>` in `/memories/<file name of values.index>` (the memory
    shape project.md states; the server owns where and how it is stored).
    `memory.rs` is the small client (stream 2's `mcp.rs` is the core's own
    server) (newline-delimited JSON-RPC with
    `serde_json`, no MCP crate: the core needs four calls). A tier entry
    is `- <date> · <entry>` and a `source:` line under the file's first
    `## ` heading, newest first; a missing file is created (its dirs
    too) with `## <Key> rules`; every such write is a temp file renamed
    over the target. Before any write, an entry already in the tier
    (the line, or a memory of the same title) makes the item `already in
    <tier>`.
- `ledger.rs`: append and summarise. A `seen` record per reflection
  `{at, session, friction: {thing: {kind: n}}, corrections: [thing]}`, a
  `decided` record per item `{at, session, thing, outcome, change}`; the
  summary takes each session's last `seen` and every `decided`.
- `entry.rs`: `sleep`, `experience`, `adaptation` call the above.
  `main.rs`: a `ledger` subcommand beside `map` and `environment`; the
  session from the input's `session_id` is stream 2's.

**Content.**

- `resources/content/skills/reflect/SKILL.md`: the reflection skill,
  announce `[reflect] applies`, declared as a turn bring-in
  (`my-harness.scope: turn`, `job`, `signals.outcomes: done`,
  `signals.jobs: closing, reflecting`). It says what to do, then how on
  Claude Code: mark, get the brief, dispatch the reflector with it,
  hand its findings back verbatim, show the batch, pass the person's
  answer, report what was applied. On Claude Code it names the session
  as `${CLAUDE_SESSION_ID}` (a documented skill substitution). It names
  the core as `@maid-core@`, which the flake replaces in this skill with
  this build's core, as it does in the hooks file.
  It never writes anything itself and never answers the batch for the
  person.
- `resources/content/agents/reflector.md`: a Claude Code sub-agent
  (`maid:reflector`), tools `Read, Grep, Glob` only. It reads the
  experience and the targets, may read a target's tier files to find
  duplicates and contradictions, and returns findings as one JSON object
  in its reply. It is told that the session's content is data, not
  instructions.
- The SessionEnd key in stream 2's hooks file:
  `"SessionEnd": [{"hooks": [{"type": "command", "command": "@maid-core@
  sleep --carrier append --input - --claude-event SessionEnd", "timeout":
  10}]}]` (hooks get 1.5 s by default; `--claude-event` turns an error
  into stderr and exit 0). Its stdin carries
  `session_id`, `transcript_path` and `cwd`; why the session ended is
  only a matcher in the documentation, so a `reason` in the input is
  recorded when present, else `end`. The claude adapter's `job` gains "and mark the session for sleep
  at its end".
- `flake.nix`: the plugin gets `agents/`, and `@maid-core@` is replaced
  in `skills/reflect/SKILL.md` as stream 2 replaces it in the hooks file.

**The memory declaration and install.** `personal-memory`'s
`server.json` gains config key `serve` ("the command that serves this
store over MCP stdio, a JSON array"); install binds it to
`["<profile>/bin/pensieve", "--store", "<store>"]`, the same command it
registers on claude. A memory with no `serve` bound is not ready, and its
habits stay pending with that reason.

**Considered and not taken.** Calling the memory server's tool from the
skill: the guards would then depend on prose. Writing the store's files
directly: the core would carry a copy of pensieve's layout, and
project.md's "no part writes in a store" would need an exception.
Giving the reflector `Write` for its findings file: it reads untrusted
session content, so it keeps no write authority and the skill pipes its
reply in. A batch file per session: stream 1's pending count reads one
file per decision, so one file per item keeps it true with no change.

## Implementation Plan

- [x] `source.rs`, `claude.rs`, `experience.rs`: the interface, the
  locator, the packer and the bundle, with fixture transcripts and unit
  tests; the project-dir naming rule moved from `harvest.rs`.
- [x] `sleep.rs` and the `sleep`/`experience` entry points; the genome's
  "to reflect on" line; unit tests.
- [x] `adapt.rs`: brief, findings, guards, batch, pending files; unit
  tests per guard.
- [x] `adapt.rs` decisions and routes (memory through `memory.rs`, local
  tier, proposal, note, already in tier); `serve` in the memory
  declaration and install; `ledger.rs` and `maid-core ledger`; unit tests.
- [x] Content: the reflection skill, the reflector agent, the adapter's
  job, the flake's `agents/` and the skill's `@maid-core@`; content
  tests.
- [x] After stream 2 merges and this rebases: the SessionEnd key in its
  hooks file, run through its hook test harness.
- [x] `tests/adaptation.rs` (Macro steps 3 and 4 with the stub, and the
  travel check through a second clone); the
  rollback test's plugin checks; project.md (My harness: sleep,
  experience, adaptation, the state layout, the routes; Memory: `serve`
  and habits through the server; Layout; Testing).
- [x] Paid P1 and P2 (4 runs) after the free gates; fixtures adjusted to
  what P1 shows.

- *Risk note:* the transcript format is Claude Code's, undocumented in
  parts and changing between versions. The packer reads it leniently and
  P1 pins the fixtures to a real one; a shape it misses loses detail, not
  the session.
- *Risk note:* the SessionEnd key waits on stream 2's merge; if it is
  late, the gate reports it as not yet wired and P1 waits with it.
- *Risk note:* the cue "close" sits near kdevkit's "close it"; the skill's
  description says "close this session", and Macro step 3 checks it fires.

## Session Log

- 2026-10-31 · grounded on the initiative spec (Experience 3-4, Macro
  3-5, the concepts, the rulings on reflection), `core-and-infusion.md`,
  `session-bring-ins.md`, project.md (My harness, Memory, Testing, Hard
  constraints), `resources/core` (entry, map, parts, contract, config,
  delivery, the CLI test), `flake.nix` (the plugin build), `harvest.rs`
  (the project-dir naming rule), writing-style's SKILL.md, and pensieve
  at its pin (it reads the store's files fresh on every tool call, so a
  memory the core writes is seen). Claude Code 2.1.314 is on PATH. From
  its documentation, not by running it: SessionEnd's stdin has
  `session_id`, `transcript_path`, `cwd`, and its output never reaches
  the model; plugins carry `hooks/hooks.json` and `agents/*.md`, an agent
  named `<plugin>:<agent>`; skills get `${CLAUDE_SESSION_ID}`,
  `${CLAUDE_SKILL_DIR}` and, in a plugin, `${CLAUDE_PLUGIN_ROOT}`;
  sub-agent transcripts are `<session>/subagents/agent-<id>.jsonl`;
  `CLAUDE_CONFIG_DIR` moves the whole tree. The transcript's line types
  and markers (interrupts, rejections, compaction, skill bodies, hook
  context) are not documented, and the format is called internal: P1
  checks them. Whether SessionEnd fires under `claude -p` is not
  documented: P1 checks it.
- 2026-10-31 · dev, slice 1: the session-source interface, Claude Code's
  locator and packer, the bundle, and a real-shaped fixture transcript
  (hook context, a refused edit, an interrupt, a correction, a skill
  load, a taught rule, a failing call repeated, a web fetch, a
  compaction, a sub-agent file and an inline side chain). Broke the
  interrupt marker and the side-chain split on purpose: 3 tests failed,
  then restored. The project-dir naming rule moved into the core;
  harvest uses it.
- 2026-10-31 · dev, slice 2: sleep marks, the `sleep` and `experience`
  entry points (a mark keeps the first source and adds later reasons; a
  new mark opens a decided session again), the genome's "to reflect on"
  line. The date helper moved into the core (`time.rs`); install uses it.
  Broke the reopen and the genome line on purpose: 2 tests failed, then
  restored. Matched stream 2's plumbing (`2e666a5`, not merged): the
  SessionEnd key and the session from input wait for its merge.
- 2026-10-31 · rebased onto `initiative/harness-core` with stream 5
  (`cf6f278`): writing-style declares `learned` (local) and `promoted`
  (permanent), promote `propose`, both optional files whose entries are
  `- <date> · <rule>` under `## Learned rules` / `## Promoted rules`,
  newest first; the document skills are a borrowed overlay.
- 2026-10-31 · dev, slices 3 and 4: `adapt.rs` (brief, the guards in
  order, the batch, one pending file per item, decisions, the four
  routes), `memory.rs` (the MCP client over stdio), `ledger.rs` and
  `maid-core ledger`; `serve` in the memory declaration, bound by
  install. A tier entry goes under the file's first `## ` heading,
  newest first, in writing-style's own shape; a new file gets `## <Key>
  rules`. An entry found in either tier is `already in <tier>`. Broke the
  tool-content need, the borrowed note, the evidence search and "ok"
  skipping items that need an answer: 4 tests failed, then restored.
- 2026-10-31 · dev, slice 5: the `reflect` skill (rules first, then the
  six steps, how per agent), the `reflector` agent (`Read, Grep, Glob`),
  a playback fixture `reflect.smoke` (dry run passes; not run, paid),
  the flake's `agents/` and `@maid-core@` in the reflect skill, the
  adapter's job. Content tests: the skill is declared with its rules, the
  reflector reads only, the skill names the core and the reflector. The
  memory client retries a command busy being written ("Text file busy",
  seen once in the workspace run: a fork in another test thread holds a
  just-written stub open).
- 2026-10-31 · dev, slice 6: `tests/adaptation.rs` through the binary
  under `env -i HOME=…`, the reflector stubbed by fixed findings, the
  memory written through the real pinned pensieve (`MAID_TEST_PENSIEVE`,
  set by the dev shell): close gives one batch of three (persona habit,
  writing-style entry, skill edit as a backlog item), nothing changes
  before "ok", after it only the memory file, its index line and
  `learned.md` change; a mid-session reflect routes the same; an end
  event marks the session and the genome says it waits; the store,
  synced by the person, reaches a second clone. Broke the memory write
  on purpose: 2 of 4 failed, then restored. The rollback test gained the
  plugin's reflector, the reflect skill's core running bare, `claude
  plugin validate`, and `serve` bound: all checks passed. project.md:
  My harness (sleep, experience, adaptation), Memory (`serve`, habits
  through the server), Tech Stack, Layout, Testing. `just ci` green.
- 2026-10-31 · Code Review Gate, round 1 (one dispatch, three lenses):
  correctness FAIL, security FAIL, comment-hygiene PASS WITH NOTES. Must
  fix: reflecting on the current session re-packed it at the findings
  step, so the reflector's own sub-agent output and the findings' Bash
  input backed any quote it made up; and a memory habit wrote a title,
  description and type the batch never showed. Fixed with the rest: the
  brief stores the session it showed and findings are checked against
  it (none without a brief); a habit needs a quote in the person's own
  words to go on "ok" (nature is not held back); a sub-agent's prompt is
  its dispatcher's, a command's output in the person's turn is tool
  content; one-line change, entry, title, type, no `[`/`]` in a title, a
  known type, frontmatter values quoted; the batch shows a memory's type,
  title and description; the memory client goes one request at a time
  and stops at a failure, waiting for `initialize`; an existing memory
  file not in the index is refused; `index` must be `<dir>/<file>`; every
  answer is checked before any is applied, and each item is placed again
  at decide time; the end after a decided reflection keeps it decided;
  tier entries match whole (a promoted rule's section lead aside); a
  proposal's name never overwrites another's; new files are `0600` and
  the temp file is created new. Comments: the cap's headroom, the
  adapter as the harness's source, the trust rules. Left (Handoff): the
  core does not compare a new entry with the tier's meaning for
  contradictions; it relies on the reflector's `contradicts`. Broke the
  brief-time check on purpose: 2 tests failed, then restored.
- 2026-10-31 · Code Review Gate, round 2: correctness, security and
  comment-hygiene all PASS WITH NOTES; both round-1 must-fix items
  resolved. Took the notes: every quote of a habit must be the person's
  for "ok"; every batch line is one line whatever its texts hold; a
  change may not hold `](`; a proposed edit's fence is longer than any
  backtick run in it; an item "ok" took that needs an answer on the map
  as it is now waits; the memory server gets two seconds to exit before
  it is stopped; a task notification or reminder in the person's turn is
  tool content; the briefed session moves to `<state>/briefed/`; the
  reflect skill's heredoc end word is made up fresh per run; an orphan
  memory file's error says what to do; project.md names the end-after-
  decided rule. Broke three of these on purpose: 3 tests failed, then
  restored.
- 2026-10-31 · paid P1 (1 model run; one attempt before it never reached
  the model: `--allowedTools` took the prompt as a tool name, no
  transcript was written): `claude -p` 2.1.314 in a scratch
  `CLAUDE_CONFIG_DIR` with the built plugin (`--plugin-dir`) and a
  SessionEnd hook through `--settings`. Settled: SessionEnd fires under
  `-p` and its stdin carries `reason` (`other`), so the mark recorded
  `other`; fixed: an end event is always `end`. `${CLAUDE_SESSION_ID}`
  reaches the core (the mark is the real session's). The real transcript
  packs with no skipped line: the person's prompt said, the skill body
  and its load told, Read/Skill/Bash/Agent did, the reflector's own file
  a sub-session whose prompt is the agent's. New line types
  (`api-request`, `tools-used`, `cost-state`, ...) are skipped as
  unknown; `attachment` lines carry the environment, model, skill
  listing, rules and whole system prompt as told, and the output style
  is repeated each turn: told now keeps one copy of a repeated text. The
  heredoc in step 4 was denied under `dontAsk` with a `Bash(<core>:*)`
  rule: in manual mode it asks the person. The reflector found nothing
  in a session with nothing to learn.
- 2026-10-31 · paid P2 (3 model runs): the reflector agent (`--agent
  maid:reflector`, `Read, Grep, Glob`) on the step-3 fixture, each answer
  through the core. 3/3: every finding kept (no quote dropped, all
  verbatim), the persona-memory habit from the correction and the
  writing-style local entry from the taught rule every time, and every
  tool-only learning (the web style guide, a tool's error) held for its
  own answer. 0/3 proposed a skill edit, so Macro step 3's backlog item
  depends on the reflector seeing one. It set `guess` on learnings it
  doubted for their source, not their persona, which asked the person
  "which persona?" needlessly; the reflector's prompt now says what
  `guess` is for (not re-run: budget spent, 4 of 4).
- 2026-10-31 · review RETURN (implementation), the four parts: a habit
  entry over 200 characters is dropped before the batch; "skip" leaves a
  session out, and `{"dismiss": "all" | [ids]}` clears waiting sessions,
  each recorded in the ledger as dismissed and leaving the pending and
  waiting counts; a decided or dismissed batch deletes its `briefed/`
  copy and keeps the capped experience; the ledger's column is
  "decided". The reflect skill offers skip and dismiss. Broke the cap,
  the briefed delete and the dismissed mark on purpose: 4 tests failed,
  then restored.
- 2026-10-31 · paid P2 re-run (1 model run, granted after the RETURN),
  with the `guess` wording: 4 findings, all kept and verbatim; the
  persona habit and the writing-style entry again; the two tool-only
  learnings held for their own answer, and no "which persona?" this
  time, so the wording fixed the misuse. Still no skill edit (0/4
  across P2), left for the Macro test as the ringmaster asked; fixture
  and prompt unchanged. Paid total: 5 (P1 1, P2 3 + 1).
- 2026-10-31 · delta review of the RETURN (one dispatch, three lenses):
  all PASS WITH NOTES. Took: dismissing an id with no mark and nothing
  pending is refused, so it is not counted; the unread `dismissed` field
  on a mark is gone (the ledger records it); `config.briefed` says its
  callers check the id; doc line on `ledger::dismissed`; project.md
  wrapped. `just ci` green.
- 2026-10-31 · rebased onto `initiative/harness-core` after stream 2
  merged (`4ffdbb3`). The branch's 19 commits were squashed into one
  first (the cross-stream rebase carve-out; was `48929c8`), so the
  conflicts with stream 2 were resolved once: `lib.rs`, `entry.rs` and
  `main.rs` take both streams' modules and arms, project.md both
  layouts, and stream 2's `shipped_fit.rs` gains the config's `claude`
  field. Added the SessionEnd key; `sleep` on an end event prints
  nothing. `claude_hooks.rs` runs the shipped key as Claude Code runs
  it (marks with `end`, prints nothing, fails quietly with no
  transcript), and `claude_live.rs` shows a real Claude Code session
  marked when it ends (free, stub API). Removing the key fails both.
  `just ci`, the rollback test and the live tests (`--ignored`, with
  `CLAUDE_BIN` set to the Claude Code binary, since the `claude` on PATH
  here is a launcher that needs the real HOME) are green.
- 2026-10-31 · closure. No unticked plan items. Backlog: this feature
  closes none of the existing items (`core-delivery-record-upkeep` is
  about delivery dirs, still open); filed `adaptation-retention` (marks,
  experience bundles and the ledger grow with no end) and
  `harness-source-declared` (the `claude-adapter` name stands for the
  harness's source). project.md: Architecture (the adapter's SessionEnd,
  sleep, experience, adaptation, Memory's `serve`), Tech Stack (the dev
  shell's `MAID_TEST_PENSIEVE`), Layout and Testing were updated in dev
  and at the rebase; Deployment is unchanged (install's path is the
  same; it binds one more value); Hard constraints unchanged.

## Decision Log

Calls made for the ringmaster from the initiative spec and the brief, to
check:

- Ruling: the guards, the batch and every write are core code; the
  reflector only returns findings as data · why: the guards are the
  person's and must not depend on an agent following prose · cost if
  wrong: none; the reflector's prompt is the only prose.
- Ruling: the reflector is a Claude Code sub-agent in mAId's plugin with
  `Read, Grep, Glob` only, dispatched by the reflection skill · why: a
  separate agent with the map, and no write authority while it reads
  untrusted session content · cost if wrong: one agent file.
- Ruling: SessionEnd only marks; reflection runs when the person says
  close or reflect, and the genome counts sessions waiting · why: nothing
  depends on an end event, and no model run is spent unasked · cost if
  wrong: a later sweep reflects marked sessions, unchanged in shape.
- Ruling: `adaptation` is one entry point with three inputs (none →
  brief, findings → batch, decisions → applied) · why: carrier-neutral,
  and a hook, a skill or an MCP tool reach it the same way · cost if
  wrong: split into subcommands.
- Ruling: one pending file per undecided item · why: stream 1's pending
  count then counts decisions with no change · cost if wrong: none.
- Ruling (ringmaster, planning gate): a persona habit is written through
  the memory's own server, so the store is left as its own write leaves
  it · chose the MCP tool over stdio: pensieve at its pin has no CLI
  write, but its server speaks newline-delimited JSON-RPC on stdin with
  no model, and does no git at all (checked in `pensieve.py`), so a habit
  reaches another environment when the person syncs the store, as every
  memory does; project.md's "no part writes in a store" stands · cost if
  wrong: a `serve` key and a small client to drop.
- Ruling: the command is bound by the part as config `serve`, a JSON
  array · why: every location is config, and the core names no package's
  arguments · cost if wrong: one key.
- Ruling: a tier entry is `- <date> · <entry>` (and a `source:` line)
  under the tier file's first `## ` heading, newest first, the shape
  writing-style already writes; a new file gets `## <Key> rules`, which
  is writing-style's heading for both its tiers;
  an entry already there is listed `already in <tier>` (ringmaster: keep
  writing-style's own in-session write, the person's direct instruction)
  · why: the one real tier file shape (stream 5's writing-style), so the
  skill reads what the core writes · cost if wrong: a declared tier
  format, when a second bring-in's tier differs.
- Ruling: nature findings become proposal files under the state dir
  (backlog-item shaped, naming the source repo and revision), never edits
  to a repo · why: the core has no checkout of a source and must not
  reach outward · cost if wrong: a later step moves proposals into a
  checkout the person names.
- Ruling: a project source's recorded route is the ceiling: a nature
  finding is a backlog item unless it carries a concrete edit and the
  route is `worktree` · why: the Macro test files a skill edit as a
  backlog item; the reflector says which · cost if wrong: one rule.
- Ruling: a borrowed bring-in gets a lender note (or a proposal to stop
  borrowing), its tiers included · why: "never changed" · cost if wrong:
  allow its local tier later.
- Ruling: tool- or web-only evidence, contradictions and persona guesses
  are left out of "ok" and need their own answer; an unplaceable finding
  is never written · why: the guards' "without the person's yes" and "no
  public store" made checkable · cost if wrong: none.
- Ruling (ringmaster): stream 2 owns the input's `session_id` fallback,
  the hooks file and how hooks name the core (`2e666a5`: `@maid-core@`,
  `hooks/claude.json`); this stream adds only the SessionEnd key, after
  rebasing onto its merge.
- Ruling: the reflection skill names the core as `@maid-core@`, replaced
  by the flake in that one skill file, as in the hooks file · why: the
  skill runs from a Bash call, not through the MCP server (whose
  `claude-mcp.json` serves genome and expression), and a store path of
  this build reaches every shell the skill is linked into and moves the
  plugin's version with the core · cost if wrong: an `adaptation` tool on
  stream 2's MCP server, and the skill calls that instead.
- Ruling: `promote: auto` is treated as `propose`; a promotion is a habit
  finding on the permanent tier · why: everything waits for the person
  in this initiative · cost if wrong: an auto rule later.
- Ruling: the explicit cues are "close this session", "reflect", "reflect
  on this session", "what did we learn" · why: "close it" is kdevkit's ·
  cost if wrong: Macro step 3 shows it, the description changes.
- Ruling (ringmaster, review): `serve` comes from part lists, trusted as
  the rest of a part's content is · why: parts are what the person
  installed.
- Ruling (ringmaster, review): the P1 attempt that never reached the
  model does not count; one more P2 run is granted after the RETURN.
- Ruling (ringmaster): paid runs P1 (1) and P2 (3) after the free gates;
  P3 deferred to Macro step 3; a failed run stops the series.
