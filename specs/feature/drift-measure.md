# Feature: drift-measure

## Git Setup

- Branch: `feat/drift-measure/work`
- Base: `initiative/harness-core`

## Feature Brief

From the final Macro run (D-1): on current Claude Code a session's
transcript grows by Claude Code's own bookkeeping entries, about 240 KB a
turn, so the 200 KB drift limit sent the short form of the essentials
again on a prompt where nothing changed. Drift is now measured on the
conversation only. Beside it, `ghost install --dry-run` says what the
install would do and where, and writes nothing.

Part of initiative: [[harness-core]]

## Handoff

- **Stage:** closed
- **Ready for:** the ringmaster's squash-merge into
  `initiative/harness-core`
- **Carry forward:** none open here. The initiative spec's "On Claude
  Code" still says "200 KB of transcript"; that edit is the ringmaster's.
  The real-model proof is the initiative's re-run of Macro steps 2 to 6.
- **Deliberately left:** the ghost's every-agent plain install is backlog
  `ghost-install-scope`; sessions running at the upgrade drift on the
  10-prompt rule until their mark moves, which needs no work.

### Crossings
<!-- Append one line per crossing. Never edit or delete a line. -->
- planning → dev
- dev → review
- review → closure
- closure → closed

## Requirements

- R1. A prompt where nothing changed gets nothing new, however much
  Claude Code writes of its own into the transcript between prompts.
- R2. The short form still goes once the conversation itself has grown
  200 KB since the essentials were last placed (the person's prompts, the
  agent's text, tool calls and their results), or 10 prompts on.
- R3. `just ghost install --dry-run` prints what the install would do
  and where: the profile and the ghost's version, whether plain is reused
  or installed (and that installing it writes for every agent), the
  plugin Claude Code would get and from where, the record's path. It
  writes nothing to HOME, the profile, Claude Code or the records.

## Test Strategy

- Unit (`transcript.rs`): 240 KB of Claude Code's bookkeeping lines
  (`api-request-blob`, `api-request-shape`, `prompt_snapshot` and
  `skill_listing` attachments) count 0 bytes of conversation; 240 KB of
  agent text or a tool result count it.
- Unit (`prefix.rs`, a shell-neutral fake source): the transcript growing
  by lines the source does not count gives nothing; growing by
  conversation gives the short form, at 200 KB and not just under.
- Repro (`claude_hooks.rs`, the built binary as the hook runs it): one
  session, two prompts with 240 KB of bookkeeping between them gives
  nothing; then 240 KB of agent text gives the short form.
- `claude_live`: a Stop hook appends 240 KB of bookkeeping lines to the
  real transcript after each turn; the second prompt gets nothing new, and
  a turn whose stub reply is 240 KB of text gets the short form after it.
- Dry run (`claude_installed`, or `tests/rollback`): `ghost install
  --dry-run` in a temp HOME prints the plan, and HOME, the profile and the
  records are byte-for-byte unchanged.
- `just ci`, `claude_live`, `claude_installed` on the current Claude
  Code, and `ghost-in-shell/tests/rollback`.

## Design

- `Bundle::conversation_bytes()` (infusion, shell-neutral): the bytes of
  what was said, each tool call's input and result, and tool text in the
  person's turn; not what the agent was told (skills, hook context,
  summaries), not sub-sessions. Which transcript lines are which stays the
  packer's (`shell/claude/src/transcript.rs`): its `type` match is the
  allowlist (`user`, `assistant`; `system` and `attachment` are told), so
  a new bookkeeping type cannot count, and a new conversation type falls
  back to the turn count.
- `prefix.rs` asks the call's session source to locate and pack the
  transcript (without sub-agent files) and drifts on
  `conversation_bytes()`; where there is no transcript, or the source
  cannot read it, on the prompts' bytes as before. N = 10, B = 200 KB.
- `--dry-run` on `ghost-claude install` (not with `--prefix`): validate,
  stage the ghost's package into a temp dir for its version, the reuse
  check against Claude Code's records, the ghost plugin's live version from
  those records (no `claude` run), each step printed `(dry-run)`. With nix
  it builds this checkout's plain package to learn its version, as install
  does (`nix build --no-link`: the nix store only). `just ghost install`
  passes flags through.

## Implementation Plan

- [x] 1. Drift on the conversation: `Bundle::conversation_bytes`, the
  prefix's measure, unit tests, the hook repro; `just ci`.
- [x] 2. `claude_live` case with bookkeeping entries.
- [x] 3. `ghost install --dry-run`, its test, the Justfile flag.
- [x] 4. Docs (project.md's short form and the ghost's install), backlog
  `ghost-install-scope`; `claude_installed`, `tests/rollback`.

## Session Log

- Entry types on Claude Code 2.1.329, a free stub session (3 prompts, one
  MCP tool call), by line bytes: conversation `user` 3.2 KB and
  `assistant` 4.8 KB; bookkeeping `attachment` (`prompt_snapshot` 64 KB in
  2 lines, `skill_listing` 14 KB, `agent_listing_delta`,
  `deferred_tools_delta`, `environment`, `model`, `date`,
  `session_context`, `remote_session_change`, `hook_success`,
  `hook_additional_context`), `api-request-shape` 38 KB,
  `api-request-blob` 14 KB (a copy of a request message, `message.role`
  and `content`, but not `type: user`), `api-request` 5 KB, and small
  `queue-operation`, `last-prompt`, `atis-latch`, `tools-used`,
  `cost-state`. 8 KB of conversation, about 170 KB of bookkeeping. The
  final run's real session: 240 KB of bookkeeping in one turn of 3 tool
  calls, under 20 KB of conversation. B stays 200 KB: that is about 50,000
  tokens of conversation, well inside the window the rules must stay in.
- Measuring packs the transcript on every prompt. A 25 MB transcript
  (100 turns of 240 KB of bookkeeping and 2 KB of conversation) takes the
  release hook 62 ms a prompt; the hook's timeout is 10 s. No faster path
  needed.
- Each new test was seen failing with the old measure (file bytes): the
  prefix test, the hook repro (it returned the same short form the final
  run saw) and the `claude_live` case.
- Dry run tested twice: `cli.rs` without nix (seen failing when the dry
  run wrote its record), and `tests/rollback` with nix, before the first
  ghost install (plain from another revision: installed for every agent)
  and before the second (plain reused, profile unchanged). The rollback
  snapshot leaves out codex's own dot-dirs: its first run failed on a
  codex launcher's background process from an earlier step rewriting its
  state there, not on anything the dry run did.
- Gates on Claude Code 2.1.329: `just ci` green; `claude_live` 9 of 9;
  `claude_installed` 1 of 1; `tests/rollback` all 65 checks.
- Code review panel (round 1): correctness, security, comment-hygiene
  all PASS WITH NOTES, no high. Fixed: the dry run skipped install's two
  refusals (a `ghost-in-shell` marketplace someone else added, read from
  Claude Code's own record; a nix profile with no nix on PATH), now tested
  for the first; the measure now passes the session id, so another
  session's file is not measured; the dry run's plain line names the
  profile and the browser server too; the CLI test names what changed;
  doc comments say the fallback, the told exclusion and the packer's
  allowlist; tests no longer cite the run they came from. Measured the
  latency the panel asked about: about 70 MB of transcript with 2,000
  attachment lines, 185 ms a prompt. Left: three copies of the
  bookkeeping line builder, in three test crates (now the same four
  kinds).
- Briefing (round 1): no defects in the branch. One for the ringmaster:
  the initiative spec's "On Claude Code" line still says "200 KB of
  transcript". Its questions are in the Decision Log below.
- `tests/rollback`'s "writes nothing" snapshot now covers only what an
  install can write (the agents' config, `.local`, mAId's state and
  config): agent launchers from earlier steps kept writing elsewhere in the
  temp HOME in the background. The dry run itself runs no agent.
- Gates after the fixes, on Claude Code 2.1.329: `just ci` green;
  `claude_live` 9 of 9; `claude_installed` 1 of 1; `tests/rollback` 65 of
  65.

- Closure, backlog: this feature closes no backlog item (none is about
  drift or the ghost's dry run). Filed `ghost-install-scope`, with the
  answer: yes, limit the ghost's plain install to Claude Code. project.md:
  Architecture (the short form's 200 KB), Deployment (`ghost install
  --dry-run`, under "The ghost's install") and Testing (`claude_live`'s
  bookkeeping case) changed in dev; Layout and Tech Stack did not.

## Decision Log

- Decision: conversation is what was said and done; nothing the agent
  was told counts, though skill bodies, hook context and summaries reach
  the model too · why: the packer already sorts every line into said,
  did or told, and Claude Code's bookkeeping lands in told or nowhere, so
  this is an allowlist with no list of bookkeeping kinds to keep up ·
  cost if wrong: a session heavy in skill loads gets the short form later
  than its context grows; the 10-prompt rule still bounds it.
- Decision: B stays 200 KB · why: 200 KB of text is about 50,000 tokens,
  well inside the window; the stub and the real run both show under 20 KB
  of conversation a small turn · cost if wrong: in tool-heavy sessions
  the short form goes about half as often as 200 KB of file bytes did.
- Decision: the dry run reads Claude Code's records and never runs
  `claude` · why: a `claude` launcher can write into its config dir, and
  the records carry what install's checks need · cost if wrong: a refusal
  only `claude` would show is missed by the dry run, not by install.
