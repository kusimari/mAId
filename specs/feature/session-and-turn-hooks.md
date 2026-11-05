# Feature: session-and-turn-hooks

## Git Setup

- Branch: `feat/session-and-turn-hooks/work`
- Base: `initiative/harness-core`
- Worktree: `mAId-worktrees/session-and-turn-hooks`

## Feature Brief

A plain Claude Code session now starts as the ghost and is reminded of
what matters as it works. At start (and again after a compaction, a clear
or a resume) it is handed the genome: the personas fused, which one this
work is most likely for, the session bring-ins and where their state
lives, the turn bring-in catalog, the environment and what is pending. A
sub-agent starts with the slice its job needs. At each prompt it is handed
only what changed: the acting persona and its memory, the first rules of
the turn bring-ins that fit the work, and a few memories that bear on it.
The same genome and expression are there on demand, as an MCP resource, a
prompt and a tool.

Part of initiative: [[harness-core]]

## Handoff

- **Stage:** closed
- **Ready for:** squash-merge into `initiative/harness-core` by the
  ringmaster
- **Carry forward:** paid R1-R4 run at the initiative's end, beside the
  Macro test, from the shell whose model access Claude Code uses (stream
  8 moved its isolation to a scratch `CLAUDE_CONFIG_DIR` and the ghost's
  own locations):
  `MAID_PAID=1 CLAUDE_BIN=<claude> nix develop -c cargo test -p maid-core
  --test claude_paid -- --ignored --test-threads 1` (4 model calls, stops
  at the first failure). The
  live test (`--ignored`, needs `claude`) is the tripwire for Claude Code
  changes and for `extensions` being ignored at load.
- **Deliberately left:** the rules file (superseded for Claude Code); glob
  walks uncached (nothing measured slow); bad index lines skipped, not
  counted; the genome uncut (2,150 bytes for this repo with a second part,
  held under 8,000 by a test); delivery dir pruning stays in
  `backlog/core-delivery-record-upkeep.md`.

### Crossings

<!-- Append one line per crossing; never edit or delete a line. -->

- planning → dev
- dev → review
- review → dev · RETURN · fault: design · issue: moving to a folder with no
  known persona or record said nothing, so the model kept acting through
  a persona and goal that no longer applied · fix: say the change to none
  once, in one line each, only to a session told one before · done when: a
  unit test for leaving a location passes and "nothing changed, nothing
  sent" still does
- dev → review
- review → closure
- closure → closed

## Requirements

### At session start (Macro test step 1)

- A fresh Claude Code session, with mAId installed and nothing else
  configured, has the genome in its context before the first reply: every
  persona, the acting one (or "not known; ask"), each session bring-in with
  where its state lives, the turn bring-in catalog, how many bring-ins are
  ready and why the others are not, and the pending count.
- The acting persona is the only one when there is one. With several, it is
  the one whose bring-ins this location points at (the folder or the files
  in it, or a store of that persona's); otherwise not known.
- The genome is placed once per session. After a compaction or a clear it
  is placed again; after a resume only if the resumed history does not hold
  it (probe P2).
- A sub-agent starts with its own slice: the personas, the acting one, the
  session bring-ins and the turn catalog, without the environment detail or
  the pending decisions, which are the person's.
- The on-demand genome (MCP resource and prompt) returns the same text the
  session start placed.

### At each prompt (Macro test step 2)

- In a location where a turn bring-in fits, the prompt is followed by that
  bring-in's first rules and a few pointers to memories that bear on the
  work, each pointer a title, a one-line hook and where the file is.
- A second prompt with nothing changed gets nothing new. A prompt that
  makes another bring-in fit gets only that one. A memory pointer already
  given in this session is not given again.
- When nothing fits and no memory bears on the work, the prompt gets
  nothing: Claude Code works as it would without mAId.
- The goal and next step, when the work's own record gives them, and the
  acting persona and its memory, when there are several personas, are
  restated only when they change (the session moves to another persona's
  location, or the record moves on). Moving to where no persona or no
  record is known is said once, in one short line each. With one persona
  and no record there is nothing to restate: the genome has said it.
- Never: the turn catalog again, the same text twice because two parts
  carry it, or a change to what the session was given at its start.
- At most 3 bring-ins fit a turn and at most 3 memories are recalled.
- The on-demand tool answers the same expression for a prompt and a folder
  given to it.

### Always

- A failure in the core never blocks or changes a prompt: the session goes
  on without the ghost's context for that event.
- A memory index that points outside its store is not read, and the genome
  says that memory is not ready, and why; an index line whose file is
  outside its store is skipped.
- Installing or rolling back mAId keeps the hooks and the core in step: a
  rollback refuses a generation without the core.

## Test Strategy

### Unit (`just test`, load-bearing)

Each names the change that would break it, and runs in temp dirs only.

- **Turn fit**: each signal kind fits on its own (a file pattern in the
  folder, a location, a job or outcome word in the prompt, a path named in
  the prompt); a declaration without signals fits on two job words, not
  one; an undeclared bring-in never fits; session bring-ins never fit; the
  cap of 3 keeps the best scores; nothing fits gives no item.
- **Persona guess**: one persona is acting anywhere; two personas, a
  location that one's bring-ins point at gives it; inside one persona's
  store gives it; a location both point at, or neither, gives none.
- **Selector**: the index path is `values[permanent]` joined with
  `values.index`; an absolute `index`, a `..` component, a symlink leaving
  the store, and an index line naming a file outside the store are each
  refused (no pointer, the reason kept); ranking by shared words with ties
  in index order; the cap; the acting persona's memories only when one is
  known, every persona's (labelled) when not; the `Selector` trait takes a
  second implementation in a test.
- **Expression**: a first fitting prompt sends fit and recall items, and
  the reinforcement when there is a record or several personas; the same
  prompt again sends nothing (append); a new fit sends only it; the same
  rules from two parts are one item and do not use up the cap; the same
  rules fitting through another word are not sent again; a moved record or
  persona re-sends the reinforcement only; transient sends all every time;
  on demand returns all; the turn budget leaves out what the session holds
  before it trims.
- **Rules**: a fit item carries its unit's rules section and nothing
  after it.
- **Shipped declarations** (`tests/shipped_fit.rs`): the six skills, the
  browser server and the document-skills overlay as this repo ships them:
  kdevkit fits planning in a spec repo with its rules, writing-style a
  rewrite, the browser a form, the document skills a named `.xlsx`, nothing
  a greeting; kdevkit's `record` finds the branch's feature spec.
- **Work record**: `record` read from each slot; the first glob that
  matches under the location, the file chosen by the branch name when
  several match, else none; the heading's section, at most 30 lines; a
  pattern leaving the location is refused; the goal, next step and state
  taken from it.
- **Claude adapter**: with no `--session`, the input's `session_id` is the
  session, and with an `agent_id` the sub-session `<session_id>.<agent_id>`
  (hashed when too long for the id rule); `--claude-event <Event>` prints
  Claude's `hookSpecificOutput` JSON with `additionalContext`, nothing when
  there are no items, and exits 0 with no stdout on any error; the shipped
  hooks file names each event with the right entry point and reset.
- **MCP server**: through the binary over stdio: `initialize`, the lists,
  `resources/read` and `prompts/get` returning the genome text, `tools/call
  expression` with a prompt and a folder; an unknown method is a JSON-RPC
  error, not an exit.

### Integration (free)

- `resources/core/tests/claude_live.rs`, `#[ignore]` (needs `claude` on
  PATH; no network, no model): a stub of the model API on `127.0.0.1` at an
  ephemeral port records every request and answers from a script. Claude
  Code runs in a temp HOME with `--plugin-dir` on a temp plugin built from
  the repo's manifest (with its `extensions`), hooks and MCP config and the
  cargo-built core, and a temp state dir with mAId's list (a turn bring-in
  and a memory) and a fixture second part. It asserts at the request level:
  - Macro step 1: the first request holds the genome; the MCP resource,
    read through Claude Code's own resource tool, returns the text the
    session started with.
  - Macro step 2: in the fixture's folder the prompt's request holds its
    rules and a memory pointer; a second prompt adds nothing new.
  - a sub-agent's first request holds the slice and not the pending line.
  - after `/compact` the next request holds the genome again, once, and a
    fitting prompt's rules again, once.
  - the plugin with `extensions` in its manifest loads and its hooks fire:
    the test that fails if Claude Code stops ignoring the field at load.
  Run it after any change to the adapter, the hooks or the plugin manifest.
- `nix develop -c resources/tests/rollback` gains: the profile's plugin has
  `hooks/claude.json` and `claude-mcp.json` naming the profile build's core,
  a start hook answers, and `just rollback` refuses a generation with
  everything but `bin/maid-core`.

### Paid (4 granted of the initiative's 30)

Spent only after every free gate and probe passes; on the first failure,
stop and report before any re-run. A model run settles only what the
request cannot show, that the model acts on what it is given:

- **R1-R2, genome read** (claude, 2 runs): a fresh session asked "who do
  you work for and where do you keep memories" names the acting persona
  and the store. Settles that a SessionStart placement is read as standing
  context.
- **R3-R4, first rules followed** (claude, 2 runs): in a plain folder, a
  deploy task, where a fixture turn bring-in fits by its job word and its
  first rule asks for a marker line, gets the marker.
  Settles that a fit item changes behaviour, not just the request.

## Design

**Why this shape.** Claude Code already offers every carrier this needs, so
the adapter is plugin files plus one small subcommand of the core: hooks in
mAId's plugin (SessionStart, SubagentStart, UserPromptSubmit) and an MCP
server in the same plugin. The hooks carry Claude's payload to the core and
its items back; the core decides what to send. Turn fit, the selector and
the reinforcement live in the core, carrier-neutral, so codex and the
frameworks later reach the same functions. Considered: a shell script per
hook (Claude-specific logic in shell, untestable in `just test`); a
launcher (forbidden, "embedded, not wrapped").

**Claude Code facts this rests on** (checked on Claude Code 2.1.314, from
the hook input and output schemas in its binary and a stub-API run; the
behaviour across turns is the probes'):

- SessionStart input: `session_id`, `transcript_path`, `cwd`, `source`
  (`startup`, `resume`, `clear`, `compact`, `fork`), optional `agent_type`;
  output `hookSpecificOutput.additionalContext`.
- SubagentStart input: the common fields plus `agent_id`, `agent_type`;
  output `additionalContext`, placed in the sub-agent.
- UserPromptSubmit input: the common fields plus `prompt`; output
  `additionalContext`. Exit 2 blocks the prompt, so the adapter never exits
  non-zero.
- `--plugin-dir <path>` loads a plugin for one session; `ANTHROPIC_BASE_URL`
  points Claude Code at any endpoint, so a local stub sees every request
  body (checked: the request reached a stub on `127.0.0.1`).

**What the probes settle** (slice 1, all free, on the stub):

- P1: UserPromptSubmit context is kept in the history of later requests
  (expected). If kept, expression's carrier is `append`; if not,
  `transient`.
- P2: after `--resume`, is the earlier SessionStart context in the
  restored history, and is `session_id` the same? Kept: resume calls
  genome without reset; not kept: with reset. `fork`: same question.
- P3: after `/compact`, the SessionStart (`compact`) context is in the
  next request, and the earlier copy is gone (no double).
- P4: a sub-agent's request: does it hold the parent's SessionStart
  context, user rules files, or only SubagentStart's? Sets whether the
  slice is needed at all, and `agent_id`'s shape for the sub-session id.
- P5: the MCP server's environment: is a session id there
  (`CLAUDE_CODE_SESSION_ID` or similar) and does PATH differ from the
  hooks'? Without a session id, on-demand calls are sessionless (all
  returned, nothing recorded). Since every genome append happens at a
  SessionStart, and every one after `startup` resets, a readiness that
  differs between the two never re-sends the genome; P5 records it.
- P6: how large an `additionalContext` Claude Code places whole, so the
  genome stays one item unless it is near the limit.

Results go in the Session Log and the decisions they settle in the
Decision Log.

**The adapter** is the hooks file plus two things in the CLI, so each hook
calls an entry point directly and the Claude mapping is data, not code:
- **Session from the input.** With no `--session`, the call's session is
  the input's `session_id`; with an `agent_id` too, the sub-session
  `<session_id>.<agent_id>`, or `<session_id>.a-<hash>` when that would
  break the id rule. Every entry point gets this, so stream 3's SessionEnd
  passes the payload as it comes.
- **`--claude-event <Event>`.** Prints `{"hookSpecificOutput":
  {"hookEventName": "<Event>", "additionalContext": "<items' texts>"}}`, or
  nothing when there are no items, and turns any error into a stderr line
  and exit 0, since UserPromptSubmit exit 2 would block the prompt.

The hooks file (`resources/content/adapters/claude/hooks.json`):

| Event | matcher | Command (after `@maid-core@`) |
|---|---|---|
| SessionStart | `startup\|resume\|fork` | `genome --carrier append --input - --claude-event SessionStart` |
| SessionStart | `clear\|compact` | the same with `--reset` |
| SubagentStart | | `genome --carrier append --input - --slice sub-session --claude-event SubagentStart` (`--slice` lands with the sub-agent slice) |
| UserPromptSubmit | | `expression --carrier append --input - --claude-event UserPromptSubmit` |

Each entry is `{"type": "command", "command": "@maid-core@ <args>",
"timeout": 10}`. **How a hook names the core:** the literal
`@maid-core@` as the command's first word; the flake replaces it with this
build's `maid-core` store path, and a test that builds a plugin replaces it
with the cargo-built binary. A second hook entry (stream 3's SessionEnd)
writes `"@maid-core@ sleep --carrier append --input -"` and adds nothing
else. Considered `${CLAUDE_PLUGIN_ROOT}/bin/maid-core`, a link inside the
plugin: the plugin's version hash counts only regular files, so a core
change would not change the version, and the plugin dir would carry a
binary codex also sees.

**The plugin.** mAId's plugin dir is shared by claude and codex, and codex
reads a plugin's default `hooks/hooks.json` and `.mcp.json` (both named in
its binary, 0.165). So Claude's files get names codex does not look for,
and only the Claude manifest names them (checked: `claude plugin validate`
passes a manifest with `"hooks": "./hooks/claude.json"` and `"mcpServers":
"./claude-mcp.json"`):
- `hooks/claude.json`: the hooks file above, `@maid-core@` substituted.
- `claude-mcp.json`: server `my-harness`, `<core> mcp` (slice 6).
- The Claude manifest gains `hooks` and, in slice 6, `mcpServers`; its
  `extensions["my-harness"]` is unchanged. The codex manifest is unchanged.
`just rollback` adds `bin/maid-core` to what a generation must have.

**The work record** (`record.rs`). Contract field `record`, optional, on a
turn bring-in, read in every slot as `rules` is: one or more `<file
glob>#<heading slug>` (a comma list in skill `metadata`, an array in the
JSON slots), each glob relative to the session's location (the repo root
when the folder is in one, else the folder). When a fitting bring-in has
one, the first glob with matches wins; with several files, the one whose
stem is one `/`-separated part of the branch name (read from `.git/HEAD`,
no git run), else none. The section under the heading (to the next heading of
its level, at most 30 lines) is the record; its `Stage` or `State` line
gives the state, its `Ready for` or `Next` line the next step, and the
file's `# ` title the goal. The section is read with the core's one
section reader (`contract::section`, which `rules` also uses). A glob
with `..` or a root is refused. project.md's contract table gains the
`record` line. kdevkit's value (`specs/feature/*.md#handoff,
docs/specs/feature/*.md#handoff, .kdevkit/feature/*.md#handoff`) lands
after the rebase onto stream 5, which owns kdevkit's declaration.

**Turn fit** (`fit.rs`). Input: the prompt, the folder (`cwd`), and the
map's declared turn bring-ins. Derived per turn, never stored:
- location: each `signals.locations` pattern, as a glob, against the
  folder's path, its name and its repo root's name (never a folder above); each `signals.files` glob against a
  bounded walk of the repo root (five folders down at most, 2,000 entries,
no links, no dot entries) and against
  path-like words in the prompt;
- job and outcome: each `signals.jobs` and `signals.outcomes` phrase whose
  content words (lowercased, stopwords out, a plural `s` dropped) are all
  in the prompt;
- state: each `signals.states` term against the work record's state, when
  a record is found;
- a declaration with no signals fits on two of its job's content words in
  the prompt; an undeclared one never fits (Claude's own skill matching
  stands);
- score: 2 per location or file match, 1 per word match; the best 3 fit.
Globs use `globset`, the matcher ripgrep uses (well-known, small; std has
no glob). Considered `ignore` for the walk: gitignore-aware, but heavier
than a bounded `read_dir` that skips dot-dirs.

**Persona guess** (`fit.rs`, used by the picture and the reinforcement):
one persona, it; several: the persona of every part with a turn bring-in
whose location or file signal matches here, or whose bound state location
contains the folder; exactly one such persona is the guess, otherwise none.

**Rules.** A fit item's first rules are read with stream 5's resolver,
`maid_core::contract::rules(unit, pointer)`, from the bring-in's unit; a
pointer that does not read leaves the fit with its name and job and the
reason in the item's data.

**The selector** (`select.rs`): a `Selector` trait, `select(&Query, &[Memory],
cap) -> Vec<Memory>`, so v1's lookup can be swapped for a local model.
- Memories: for each bring-in whose declaration has `state.permanent` and a
  config key `index` (project.md, Memory), the index at
  `values[permanent]` joined with `values.index`. Enforced: `index`
  relative, no `..`, no root; the joined path canonicalized and still
  under the canonical store; each index line's file the same. An index
  that fails makes its bring-in not ready, with the reason; a line that
  fails is skipped.
- Lines: `- [Title](file.md) - hook`; anything else is skipped.
- `Lookup` (v1): the prompt's content words (lowercased, a short stopword
  list dropped) against each line's title and hook; score is the shared
  word count; at least 1; ties in index order; cap 3. The persona's: the
  acting one's when known, all (labelled) when not.
- Items carry pointers only (title, hook, absolute file path, persona),
  never bodies.

**Expression** (`entry.rs`), items in this order, all on the call's
carrier:
- `reinforce`: the acting persona and its memory (name and store) when
  there are several personas, and the goal and next step when a work record
  is found. Re-sent only when it changes; none when it would say nothing
  the genome has not.
- `fit.<part>.<name>`: per fitting bring-in, its job and first rules. Items
  whose text equals an earlier item's are dropped.
- `recall.<hash of path>`: per recalled memory, its pointer.
The catalog is never in it. Neutral input keys: `prompt`, `cwd`.

**The genome slice.** `genome --slice sub-session` gives the picture
without the environment's detail lines (the ready count stays) and without
pending. The picture takes `cwd` from its input
for the persona guess.

**MCP server** (`mcp.rs`, `maid-core mcp`): line-delimited JSON-RPC 2.0 on
stdio: `initialize`, `resources/list` and `resources/read` for
`my-harness://genome`, `prompts/list` and `prompts/get` for `genome`,
`tools/list` and `tools/call` for `expression` (`prompt`, `cwd`), each on
the on-demand carrier. `--session-env <NAME>` names the variable that holds
the shell's session id (Claude Code: `CLAUDE_CODE_SESSION_ID`, per P5), so
on-demand calls record under the same session as the hooks; the server's
own folder is the location when a call gives none. Considered the
`rmcp` SDK: it brings an async runtime and a macro layer into a hook-path
binary for three read-only methods; serde_json and about 150 lines win
here, and the live test checks Claude Code takes it.

**Dependencies.** `globset` (new). Everything else is in the workspace.

## Implementation Plan

- [x] Shared with stream 3, first: the hooks file and its shape, `@maid-core@`
  substituted by the flake, the Claude manifest's `hooks`, the session
  from the input, `--claude-event`; the rollback guard; tests.
- [x] Probes P1-P4 and P6 on the stub API, results and the decisions they
  settle logged; P5 runs with the MCP server. The durable harness is the
  live test's slice.
- [x] The sub-agent slice (`--slice sub-session`) and the reset rule P2
  settles; unit tests.
- [x] Turn fit and the persona guess, against fixture declarations and the
  shipped ones; unit tests.
- [x] The work record: the `record` field in the three slot readers,
  `record.rs`, project.md's contract line; unit tests against a fixture.
- [x] The selector: `Selector`, `Lookup`, the index rule enforced; unit
  tests.
- [x] Expression filled (reinforce, fit, recall; dedupe; emit on change);
  unit tests.
- [x] The MCP server and the plugin's `claude-mcp.json`; binary tests.
- [x] The live test's Macro steps 1 and 2, sub-agent, compaction and
  `extensions` cases; the rollback test's checks; project.md (My harness:
  the adapter's carriers, turn fit, the selector; Layout; Testing).
- [x] When stream 5 merges: rebase, read rules with its resolver, add
  kdevkit's `record` value, re-run the gates.
- [x] Paid R1-R4: written and dry-run against the stub; running them is
  deferred to the initiative's end by ruling (see Decision Log).

- *Risk note:* Claude Code's hook and plugin surface moves often; the live
  test is the tripwire, so it must run after any Claude Code update that
  touches hooks.
- *Risk note:* the bounded walk runs on every prompt. If it shows in hook
  latency, cache it per session in the state dir.
- *Risk note:* the stub's scripted answers must match the API's streaming
  shape closely enough for Claude Code to accept them; P1 proves it first.

## Session Log

- 2026-10-31 · grounded on the initiative spec, `core-and-infusion.md`,
  `session-bring-ins.md`, project.md (My harness, Memory, Testing),
  `resources/core` (entry, contract, environment, delivery, config, CLI),
  the flake's plugin build and `rollback-profile`. Free probes: Claude Code
  2.1.314's hook schemas read from its binary (fields above);
  `--plugin-dir` and `CLAUDE_CODE_SESSION_ID` exist; `claude -p` in a temp
  HOME with `ANTHROPIC_BASE_URL` at a stub on `127.0.0.1` sends its full
  request bodies there, so every behaviour across turns can be probed with
  no model run.
- 2026-10-31 · dev, slice 1 (shared with stream 3): `hooks.json` with
  `@maid-core@`, the flake's substitution and the Claude manifest's
  `hooks`, the session from the input, `--claude-event`, the rollback
  guard. `tests/claude_hooks.rs` runs each shipped hook command through
  `sh -c` with a payload; with the input fallback removed, 3 of its 4 tests
  failed, then restored. Codex 0.165's binary names a plugin's
  `hooks/hooks.json` and `.mcp.json`, hence Claude's own file names.
  `claude plugin validate` passes the built plugin (the known `extensions`
  warning only). `just ci` green.
- 2026-10-31 · probes on a throwaway stub of the model API (Python, on
  `127.0.0.1`, `/tmp`), Claude Code 2.1.314 with `--plugin-dir` and the
  slice-1 hooks, in a temp HOME. Context from a hook arrives as its own
  `system` message, "<Event> hook additional context: …".
  - P1: UserPromptSubmit context is kept in every later request of the
    session: expression stays `append`.
  - P2: `--resume` keeps the session id and restores the earlier genome;
    Claude Code does not place a SessionStart context identical to one in
    the restored history, and does place a changed one. `--fork-session`
    gives a new id and `source: fork`, with the same handling. So resume and
    fork call genome without reset.
  - P3: `/compact` (as a stream-json prompt) drops everything before it,
    and SessionStart `compact` with reset places the genome once; the reset
    clears the session's whole record, so fit and recall items return when
    they apply.
  - P4: a sub-agent's request holds the user's rules and memory files but
    not the parent's SessionStart context; it holds SubagentStart's. The
    slice is needed. `agent_id` is 17 hex characters.
  - P5 (hooks half): a hook's env has `CLAUDE_CODE_SESSION_ID`, PATH as
    Claude Code's own, `CLAUDE_PLUGIN_ROOT`; the MCP half waits for the
    server (a plugin MCP server stayed `pending` in `-p` until used).
  - P6: Claude Code places an `additionalContext` up to 10,000 characters
    whole; over that, a 2 KB preview and a file. The genome is far under.
- 2026-10-31 · rebased onto `initiative/harness-core` at stream 5's merge
  (`cf6f278`); slice SHAs changed (the shared one is now `7fae033`). Then
  `here.rs`, `record.rs`, `fit.rs`, `select.rs` and expression; the
  `record` field in every slot; kdevkit's `record` value; an index leaving
  its store is a not-ready reason (rule 15). Broke the same-rules dedupe
  and the index check on purpose: each failed its test, then restored
  (the first index check was a silent no-op edit, caught by that). A
  YAML value starting `*` is an alias, so a glob in skill `metadata` must
  be quoted. `just ci` green.
- 2026-10-31 · the MCP server and `claude-mcp.json`. P5 (MCP half): the
  plugin's server connects in `-p`; its env has `CLAUDE_CODE_SESSION_ID`
  equal to the session's and the same PATH as the hooks', so readiness
  is the same on both carriers and on-demand calls share the session's
  record. Claude Code defers MCP tools behind its tool search; a direct
  call of `mcp__plugin_maid_my-harness__expression` answered. `just ci`
  green; `claude plugin validate` passes the built plugin.
- 2026-10-31 · `tests/claude_live.rs`: a Rust stub of the model API on
  `127.0.0.1:0`, Claude Code 2.1.314 with `--plugin-dir`; 3 tests pass
  (start and turns, the MCP tool, compaction). Prompts fed at once can
  arrive mid-turn after a tool call, so the tool and the compaction run in
  their own sessions. Removing `--reset` from the compact row failed the
  compaction test, then restored. The rollback test checks the profile's
  plugin calls its own core and a start hook answers; green. project.md:
  the core's new options, expression, the persona guess, the Claude
  adapter, the rollback refusal, Layout, Testing.
- 2026-10-31 · Code Review Gate, round 1 (one dispatch, three lenses):
  correctness, security and comment-hygiene PASS WITH NOTES, no high.
  Must fix (medium), all fixed: the turn budget counted items the session
  already held, and one item could pass it (now: held items left out
  first, then the budget, then claims; rules cut to 30 lines and 2,000
  bytes, record and memory fields to 300); a file named in the prompt
  scored 1, not 2 (weights now carried with each reason, and tested);
  project.md said the record needs a fitting bring-in (it is read
  whether or not, since states fit on it; project.md now says so); a
  glob walk followed a link in its starting folders (now refused,
  tested with a linked `specs/`). Took the cheap notes: fit ids hashed
  when not plain or too long; the cap counted after the same-rules
  dedupe; the walk bounded before sorting; HEAD and `.git` reads capped;
  the record and index read through the 1 MiB reader, the title outside
  fences; a relative store says so; containment checks the deepest
  existing folder; MCP answers only protocol versions it knows and takes
  only folders under its own; a hook exits 0 on a bad flag or a panic;
  the compaction test now proves the turn's record is reset too;
  comments fixed. Left: caching globs per call (risk note); delivery
  dir pruning is `backlog/core-delivery-record-upkeep.md`. Broke the
  linked-folder check on purpose: its test failed, then restored. `just
  ci` and the live test green.
- 2026-10-31 · `tests/claude_paid.rs` (R1-R4), ignored, gated on
  `MAID_PAID=1`, auth passed through by variable name only, temp HOME.
  Dry run against the stub: R1's request holds the genome and the store,
  R3's holds the marker rule; both fail on the stub's reply, as they
  must. Not run with a model: project.md leaves paid runs to the person.
- 2026-10-31 · Code Review Gate, round 2 (last of 2): all three lenses
  PASS WITH NOTES, no high; round 1's Must Fixes resolved. Fixed the
  medium ones: the budget test could not fail (now `deliver_within` is
  tested with a small budget and a held item); a fit's data held why the
  prompt fit, so the same rules were re-sent when another word matched
  (data is now what the bring-in is); the MCP tool's folder check did not
  resolve `..` or links (now canonical, tested with both). Took: the fit
  header and the reinforcement's list clipped; `--claude-event=` form
  detected; comments. Each new test watched failing with its fix undone.
  project.md: the genome is not cut, and why it stays small.
- 2026-10-31 · the review briefing sent back five defects, fixed: the
  compaction test's change had not applied (an edit matched text the
  formatter had rewrapped), so it now sends a fitting prompt after
  `/compact` and expects its rules once; the live test reads the genome
  resource through Claude Code's own resource tool and checks it is the
  text the session started with; the rollback test refuses a generation
  with everything but the core (before, an earlier check refused first);
  the spec says what happens to a bad index line (skipped) and matches
  the walk's depth, the phrase match and the fixture's parts. `just ci`,
  the live test (4) and the rollback test green.
- 2026-10-31 · the second briefing run: the five earlier defects fixed;
  three more, fixed: the reinforcement broke "nothing fits, nothing sent"
  with one persona (it now restates the persona only when there are
  several; the record still comes when found), the location signal
  matched folders above the location (now the folder's path, its name
  and its repo root's name, tested), and the spec named the slice as an
  input field (it is `--slice`). `just ci` and the live test green.
- 2026-10-31 · review RETURN (design): leaving a location is said once
  (`persona: none known here`, `record: none here`), only to a session
  that was told a persona or record (`Record::sent_any`); a single-persona
  install still sends nothing. Unit test for leaving a location; the
  "nothing changed" and "nothing fits" tests unchanged and green. Added
  the genome size test for this repo's install with a second part:
  2,150 bytes, 23 lines today, limit 8,000. `just ci`, the live test (4)
  and the rollback test green.
- 2026-10-31 · closure. No unticked plan items. Backlog: this feature
  closes none of the existing items (`core-delivery-record-upkeep` stays
  open: this stream adds per-turn items to the same record, so it grows
  faster); nothing new filed, since glob walks showed nothing slow and
  the rest is ruled. project.md: Architecture (the core's options,
  Expression, the persona guess, the Claude Code adapter, the `record`
  contract, rule 15's index reason, rollback's core check), Layout and
  Testing were updated in dev; Tech Stack gains `globset` (below);
  Deployment is unchanged (install's path is the same; the plugin carries
  two more files).

## Decision Log

Calls made from the specs and the brief; the ringmaster accepted all of
them at the Planning Review Gate (2026-10-31), with the changes noted.

- Ruling: no rules file for Claude Code; the initiative's System design row
  "`genome`, static: rules file without path scope, rendered at install"
  is superseded for Claude Code (stream 6 folds this back) · why: nothing
  in the genome is the same for every environment and part, so a file
  rendered at install would be a stored copy of the map, which stream 1
  ruled out; SessionStart fires at every point where context drops
  (compact, clear, resume) · cost if wrong: add it later as one item, one
  file.
- Ruling: the adapter is data plus two CLI features: the hooks file calls
  the entry points directly, the session comes from the input's
  `session_id` (and `agent_id`), and `--claude-event` formats Claude's
  output and never fails a hook · why: the mapping stays readable in one
  file, stream 3 adds one entry with no code, and the core stays
  carrier-neutral · considered a `claude-hook` subcommand that dispatched
  on the event: the same logic, hidden in code.
- Ruling: a hook names the core as `@maid-core@`, replaced by the flake
  with this build's store path · why: the plugin and its core are one
  build, and the plugin's version hash moves with the core · considered
  `${CLAUDE_PLUGIN_ROOT}/bin/maid-core` (stream 3's assumption): a link,
  which the hash does not count · cost if wrong: switch to a hashed link.
- Ruling: Claude's hooks and MCP files are `hooks/claude.json` and
  `claude-mcp.json`, named by the Claude manifest · why: the plugin dir is
  shared with codex, which reads the default `hooks/hooks.json` and
  `.mcp.json` · cost if wrong: codex runs Claude-shaped hooks.
- Ruling: every free question is settled on a stub of the model API on
  loopback, not by paid runs · why: Claude Code sends its whole request
  there · cost if wrong: the stub's streaming answers drift from the API
  and the live test needs updating.
- Ruling: one item for the genome, as stream 1 left it, unless P6 shows a
  size limit near it · why: it is sent only at a SessionStart.
- Ruling: expression is three item kinds (reinforce, one per fit, one per
  recalled memory) on the append carrier · why: each changes on its own.
- Ruling: turn fit v1 is word and glob matching, fixed scores, cap 3 · why:
  cheap, explainable, testable · cost if wrong: a better scorer behind the
  same function.
- Ruling: the selector is a trait with a lookup v1, cap 3, pointers only ·
  why: "cheap lookup, built to move to a local model".
- Ruling: the persona guess comes from bring-in signals and bound stores ·
  why: a persona owns nothing, so only its bring-ins point at a place.
- Ruling: a sub-agent's slice drops the environment detail and pending ·
  why: those are for the person.
- Ruling: the MCP server is hand-rolled over serde_json · considered `rmcp`:
  an async runtime and macros for three read-only methods.
- Ruling: `globset` is the one new dependency.
- Ruling (ringmaster, Q1): the work's own record is an optional contract
  field `record` on a turn bring-in, `<file glob>#<heading slug>`, several
  allowed, resolved relative to the session's location; this stream owns
  its reader and project.md's contract line; kdevkit's value lands after
  the rebase onto stream 5 · why: the record stays the work's, and its
  location is declared by the bring-in that knows it.
- Decision (P2): resume and fork go without reset · why: the restored
  history holds the genome, and the record decides for a changed one ·
  considered reset on every non-startup source: it leans on Claude Code
  dropping an identical copy.
- Decision: the probes ran on a throwaway Python stub, the durable stub is
  the live test's (slice "live test") · why: the probes needed fast
  changes to hooks; the test needs only what they found.
- Decision (P6): the adapter keeps all of a hook's items in one
  `additionalContext`, which must stay under 10,000 characters; expression
  trims recall, then fit items, to stay under it.
- Decision (stream 5 merged): no `Rules` trait; fit reads rules through
  `contract::rules` directly · why: there is one resolver and tests use
  real files, so a trait would have one implementation.
- Decision: the record's goal is its file's `# ` title · why: generic to
  any markdown record; kdevkit's specs open with `# Feature: <name>`.
- Decision: a memory index that leaves its store makes its bring-in not
  ready (rule 15), and recall skips it · why: the person sees why its
  memory is not recalled, in the genome and in status.
- Decision (review): the record is read whether or not its bring-in fits
  · why: `signals.states` fit reads the record's state, so it must come
  first · considered only a fitting bring-in's: no state could ever fit.
- Decision (review): the turn budget applies after held items are left
  out, and per-item cuts keep any one item small · why: a long-running
  session must still get a new fit or memory.
- Decision (briefing): with one persona the reinforcement does not
  restate it · why: the genome already named it and it cannot change, so
  repeating it would send text on a turn where nothing fits.
- Ruling (ringmaster, review): "the few rules that must never drift"
  (initiative §2) are carried by the fitting bring-ins' first rules and by
  the genome placed again at clear and compact; there is no other carrier
  · why: one carrier per item; stream 6 folds this into the docs.
- Ruling (ringmaster, review): the location scope stays the folder or its
  repo root; bad index lines stay skipped and uncounted; glob walks stay
  uncached; paid R1-R4 run at the initiative's end with the person.
- Ruling (ringmaster, review RETURN): a move to where no persona or record
  is known is said once, so later streams never read a stale persona.
- Ruling (ringmaster): 4 paid runs, R1-R4, only after the free gates and
  probes pass; stop and report on the first failure.
