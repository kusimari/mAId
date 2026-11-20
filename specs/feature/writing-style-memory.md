# Feature: writing-style-memory

## Git Setup

- Branch: `feat/writing-style-memory/work`
- Base: `initiative/harness-core`

## Feature Brief

Fix F1, F2 and F3 from Macro test run 3 (`/tmp/harness-core-macro/report.md`,
"Run 3, second attempt"), to the user's ruling of 2026-11-01. A rule the
person teaches writing-style ("new rule: …") is saved to the persona's
memory during the session, through the memory server's own tool. That is
its temporary tier. At close, the batch says what was saved, and offers one
item: move those rules into the skill's checked-in text, as a proposal on
its source. writing-style's files `learned.md` and `promoted.md` stop being
written; an older one is still read, and its rules go with the move. The
reflector quotes only the person's words as a habit's evidence (F2), and
the held item's label says what the check found (F3).

Part of initiative: [[harness-core]]

## Handoff

- **Stage:** closed
- **Ready for:** the ringmaster's squash-merge into
  `initiative/harness-core`
- **Carry forward:** none open here. The `Edit` grant narrowing is
  `backlog/writing-style-grant-reads-only.md`; the real-model check of
  R1, R2 and F2 is the initiative's Macro re-run.
- **Deliberately left:** none.

### Crossings
<!-- Append one line per crossing. Never edit or delete a line. -->
- start → planning
- planning → dev
- dev → review
- review → closure
- closure → closed

## Requirements

- R1. **Teach.** "new rule: …" (or "add rule:", "style rule:") saves the
  rule as a memory of the persona the session acts through, with that
  memory server's own tool, during the session, and says so in one line.
  It is marked as a writing-style rule. Nothing is written to `learned.md`,
  `promoted.md` or any skill file. With no memory tool in the session, the
  rule holds for this session only, and the skill says so. With several
  memories and none acting, it asks which.
- R2. **Apply.** In a later session, a saved rule applies when
  writing-style is used (the skill finds its rules in memory), and it can
  also reach a turn through the existing recall when the prompt is about
  it. An older `learned.md` / `promoted.md` still applies, as before.
- R3. **Close.** The decision batch lists each writing-style rule saved in
  memory this session as "saved in memory", with where, and that it stays
  there until the person removes it. It offers one item: move those rules,
  and any still in an older `learned.md` / `promoted.md`, into
  writing-style's text, as a backlog item on its source that carries the
  text to add. With nothing to move, there is no item.
- R4. **"ok".** The move proposal is written where the route says (under
  the core's proposals, backlog-item shaped, naming the source and
  revision), and nothing else changes for it: the memories and the older
  files stay as they are. The proposal ends with one line naming the
  memories to remove once it lands.
- R4a. **Once decided, not again.** A move the person accepts or declines
  is recorded, and the same rules are not offered again: a saved rule
  once decided, or an older file's rules until that file changes. A new
  rule saved, or an older file changed, brings the move back with just
  what is new.
- R5. **F2.** The reflector quotes only the person's own words as a
  habit's evidence; the agent's or a tool's text may be named in `change`
  as context, never as evidence. It sets `guess` only when the persona is
  unknown. The core's guard is unchanged.
- R6. **F3.** An item held because a quote is the agent's words says "a
  quote is not your own words". One held because a quote is only in tool
  or web content keeps "learned only from tool or web content".
- R7. **Macro step 3** reads as ruled: the persona habit (from the
  correction), the rule saved in memory (the "bring-in local-tier entry",
  shown as saved), and the move into the skill (the "skill edit as a
  backlog item"). After "ok", the habit is written and the proposal filed;
  nothing else changes.

## Test Strategy

Each test names the change that would break it, watched failing first;
temp dirs only.

**Unit (`just test`).**
- Declaration (build-tool, shipped content): writing-style declares no
  `state`, its `learned` and `promoted` config keys are optional, and it
  reads as ready with neither file present. Install binds a skill's config
  keys to `<config>/<skill>/<key>.md` (breaks if the binding still keys on
  `state`).
- Saved rules (`adapt`): a `memory_create` call to a memory bring-in's
  server, whose file is in that store and is tagged with a bring-in's
  name, is listed as saved; one untagged, tagged with an unknown name,
  failed (no file in the store), or to a server that is not a memory, is
  not.
- Move item: one item carrying each saved rule's text and each older-file
  rule; none when there is nothing to move; "ok" writes one proposal
  ending with the memories to remove, and the snapshot shows nothing else
  changed.
- Decline: a second close after "no" offers no move; a new saved rule
  brings back only that rule; an older file changed brings back its
  rules.
- F3: a habit whose second quote is the agent's words is held as "a quote
  is not your own words"; one whose quote is only in a tool result keeps
  the tool/web label.

**Integration (free).**
- `tests/adaptation.rs`, Macro step 3: the fixture session also saves the
  taught rule to memory (the tool call in its transcript, the file in the
  store as the server leaves it), and the HOME has an older `learned.md`
  with one rule. The batch: 2 to decide (the habit, the move as a backlog
  item) and one "saved in memory" line. After "ok", only the habit's
  memory file and index line change under HOME; the proposal holds the
  saved rule and the older file's rule. A second close of the session
  after declining the move offers nothing new.
- `just ci`, `nix develop -c resources/tests/rollback`, the live tests
  (`--ignored`, Claude Code 2.1.314 with `CLAUDE_BIN`).
- `just resources::verify-skills-dry`: writing-style's fixtures, updated
  to teach-to-memory and the older-files case, build.

**Paid.** writing-style's playbacks (`writing-style.smoke`, and the older
files fixture) are the ringmaster's to grant; none run here.

## Design

**The contract says it with no new field.** writing-style drops `state`.
Its `job` says where its rules live: "…and keep the style rules I teach in
my persona's memory until they move into this skill". It keeps two
optional config keys, `learned` and `promoted`, now "an older rules file,
read until its rules move into this skill". Its memories are recognised by
one frontmatter key in the memory file, `metadata.bring-in:
writing-style`; pensieve keeps frontmatter as written, so no server change.

**Install.** `bind_state` binds each skill's config keys (today only
writing-style has any) to `<config>/<skill>/<key>.md`, the same paths as
before. `STATEFUL_SKILLS` and its grant are unchanged.

**Core, at the batch (`adapt::check`).** After the reflector's findings
pass the guards:
- Saved: each `did` in the briefed session whose tool is
  `mcp__<memory>__memory_create` (or a plugin's `…_<memory>__memory_create`)
  for a memory bring-in in the map, whose `path` under `/memories` is a
  file now in that store (beside the index), with `metadata.bring-in`
  naming a declared bring-in. Read with `gray_matter`, already a dep. The
  rule is the file's body.
- Older: each bound config value of a declared own bring-in (never a
  borrowed one) that is a tier-shaped file: its `- ` entries
  under `## ` headings, read as `entry_text` already reads tiers.
- Left out: what a ledger `decided` record already holds as moved
  (accepted or declined), or another session's waiting move holds. Each
  rule offered has a key `rule:<thing>:<text>`, and each older file
  `file:<path>#<hash of the file>`, so an older file is read again only
  once it changes, and then gives only its new rules. The keys ride on the
  pending item (`moves`) and into the ledger record when it is decided.
- Saves counted: `memory_create` or `memory_str_replace`, not failed (the
  packer marks a failed call), in the session or a sub-session.
- If any are left: one finding, `kind: nature`, `thing` that bring-in,
  `change` "move N taught rules into <name>'s text", `edit` the rules as
  `- <rule>` lines each with where it came from, `evidence` each rule with
  its `at`. It is placed by `place()` like any nature finding (own project
  source: backlog item; borrowed: a note). The batch prints the saved
  lines first, then the items. The proposal ends with "Once this lands,
  remove these memories so the rules are not in two places: …".
- Memories stay until the person removes them: the core never deletes
  them and does not watch for the proposal landing.

**F3.** A new `Need::NotOwnWords` (`not-own-words`): a habit's quote not
in the person's words is `ToolContent` when it is found in tool or told
text, else `NotOwnWords`. Older pending files still read.

**Content.** writing-style's Learning loop: save through the memory tool
(`memory_create` of `/memories/writing-style-<slug>.md` with `name`,
`description`, `metadata.type: feedback`, `metadata.bring-in:
writing-style`, then `memory_insert` of its index line); session start:
`memory_search` for `writing-style` and apply what it finds, plus any
older file; "Promotion" becomes "Moving rules into the skill" (at close,
through the reflection). The reflector's prompt gets R5, and "a rule a
bring-in already saved in memory this session is listed by the core;
propose nothing for it". Version 1.3.0.

**Docs.** project.md: Skill state, mAId's bring-ins, Adaptation (the saved
lines and the move item, the new label), Memory (the `bring-in` key), For
any persona part (a bring-in may keep what it learns in the persona's
memory, tagged), Testing. README's adaptation paragraph.

## Implementation Plan

- [x] F3: `Need::NotOwnWords` and its label; unit tests.
- [x] Core: saved rules, older rules, the move item, the batch lines;
  unit tests.
- [x] Contract and install: writing-style's declaration, `bind_state` on
  config keys; shipped and install tests.
- [x] Content: writing-style's teach and session start, the reflector
  (F2), the two writing-style fixtures.
- [x] `tests/adaptation.rs` to Macro step 3 as ruled; project.md, README;
  backlog `writing-style-grant-reads-only`; `just ci`, rollback, live
  tests, `verify-skills-dry`.

## Session Log

- 2026-11-01 · grounded on the run 3 report, the initiative's newest
  rulings, `turn-bring-ins.md`, `sleep-and-adaptation.md`, project.md
  (Skill state, My harness, Memory, For any persona part), `adapt.rs`,
  `experience.rs`, `claude.rs`, `select.rs`, `contract.rs`,
  `stages.rs::bind_state`, `reads.rs`, the writing-style and reflect
  skills, the reflector, `tests/adaptation.rs`, and pensieve at its pin
  (`/memories` is `<store>/memory/`; `memory_search` matches every word,
  frontmatter included). Found: the briefed bundle keeps each tool call's
  input in full; only writing-style among skills has config keys.
- 2026-11-01 · dev, slices 1-5. F3: `Need::NotOwnWords`; broke nothing
  else, and the old agent-words test now asserts both labels. Core:
  `adapt/moves.rs` (saved rules, older files, the move item, keys);
  `ledger::moved`; the batch's saved lines; the proposal's remove line.
  Broke the server match, the decided filter, the `source:` skip and the
  remove line on purpose: one test failed each, then restored. Install
  binds a skill's config keys; with the old state-key loop the binding
  test failed. `tests/adaptation.rs` to Macro step 3 as ruled, plus the
  decline test; dropping the keys in `entry.rs` failed it. writing-style
  1.3.0 saves to memory and reads older files; the reflector quotes only
  the person for a habit; the promote fixture is now
  `writing-style-older-rules.smoke`. `just ci` green,
  `resources/tests/rollback` all checks pass, `claude_live` 6/6 on
  2.1.314 (`CLAUDE_BIN`, loopback stub), `verify-skills-dry` all build.
  No paid run.
- 2026-11-01 · Code Review Gate (one lens, correctness: no named-risk
  path): PASS WITH NOTES. Took: a changed older file gives only its new
  rules (rule keys per older rule too); keys carry the bring-in; a
  `memory_str_replace` counts as a save; sub-sessions are read; a failed
  call is not a save (the packer marks a call `failed`); the plugin
  server match is anchored; a move waiting in another session's batch is
  not offered twice; `bind_state`'s comment says a skill's config key is
  a file. Each new test watched failing first. Left: "offer older files
  only with a save" (it would drop your ruling that a changed older file
  brings the move back); the cross-session duplicate it named is fixed
  instead.

- 2026-11-01 · closure. No unticked plan items. Gates on `b330d1d`
  before the close commit: `just ci` pass (after the own-only `older()`
  change and its test, watched failing first); rollback, `claude_live`
  6/6 on 2.1.314 and `verify-skills-dry` passed on `2602899`, and the
  later commits change only core logic covered by `just ci` and docs.
  project.md: Architecture (Skill state, mAId's bring-ins, Adaptation's
  "Rules saved in memory", Memory's `bring-in` key), Layout
  (`adapt/moves.rs`), Testing (`tests/adaptation.rs`, the shipped
  contract) and For any persona part (a bring-in keeping what it learns
  in memory) were updated in dev and read true; Tech Stack, Deployment
  and Hard constraints are untouched (no dependency, install path or
  invariant changed). Backlog: this feature closes
  `writing-style-permanent-tier-travels` (rules now travel in the
  persona's store and land in the skill's text; `git rm`), and rewrites
  `writing-style-mcp-server` to what is left (typed formatter, lint,
  persistent strict mode; taught rules out of scope). Filed in dev:
  `writing-style-grant-reads-only`. Paid runs: none.

## Decision Log

- Ruling: the contract says "my temporary tier is the persona's memory"
  with no new field: writing-style declares no `state`, says it in `job`,
  and tags its memories `metadata.bring-in: <name>` · considered
  `state.local: memory` (a reserved value): the reader, validator,
  install binding, readiness and the adaptation tiers would each special-
  case it · cost if wrong: add the value later; the tag still finds the
  memories.
- Ruling: the core, not the reflector, finds the saved rules and makes the
  move item, from the session's own memory tool calls · why: 0 of 5 real
  reflector runs proposed a skill edit; the guards are code · cost if
  wrong: none; the reflector may still propose other nature.
- Ruling: memories stay until the person removes them · why: the least;
  watching for a proposal to land needs a source checkout the core does
  not have · cost if wrong: a later remove item.
- Ruling (ringmaster, planning gate): the `job` wording plus the memory
  tag, no new field; memories stay until the person removes them, and the
  proposal ends with one line naming them; a decided move (accepted or
  declined) is recorded in the ledger and its rules are not offered
  again until a new rule is saved or an older file changes; the `Edit`
  grant stays, with a backlog item to narrow it; Macro step 3 is built to
  the reading, for the user to ratify at the initiative review.
- Ruling (Code Review Gate): keys carry the bring-in and each older
  file's rules, an edit or a sub-agent's save counts, a failed call does
  not, and a move waiting in another session is not offered twice · why:
  a changed file must not re-offer moved rules, and two waiting sessions
  must not file the same move · cost if wrong: none; a key shape in an
  append-only ledger.
- Ruling: older files' rules are offered at a close even with no save ·
  why: the ringmaster's ruling that a changed older file brings the move
  back; riding only with a save would leave a person who never teaches
  again unmoved · cost if wrong: one `continue` when nothing was saved.
- Ruling (ringmaster, review gate): `older()` reads only own bring-ins'
  files; a borrowed one's are never a source for a move. Older files keep
  a move item until it is decided once; prose-only "never write the old
  files" is accepted; no separate paid grant (the Macro re-run checks R1,
  R2 and F2 on a real model).
- Ruling: "not again" is keyed per rule in the ledger, not by scanning
  proposal files · why: one record covers accepted and declined alike,
  and the ledger already holds each decision · cost if wrong: a scan of
  `<state>/proposals/`.
