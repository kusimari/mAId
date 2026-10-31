# Feature: macro-fixes

## Git Setup

- Branch: `feat/macro-fixes/work`
- Base: `initiative/harness-core`

## Feature Brief

Fix what Macro test run 1 found: current Claude Code loads mAId's ghost
from the plugin as install places it; a second prompt with nothing changed
gets nothing; and "For any persona part" in project.md is enough on its
own to add a persona repo. Scope is the run's report
(`/tmp/harness-core-macro/report.md`): D1, D2, D3 and the 13 dry-run gaps.

Part of initiative: [[harness-core]]

## Handoff

- **Stage:** closed
- **Ready for:** nothing; the ringmaster squash-merges into
  `initiative/harness-core`
- **Carry forward:** none open. The live tests after a Claude Code
  upgrade are in project.md Testing.
- **Deliberately left:** codex on the links dir (ruled: it moves only if
  it starts refusing). A live read under the moved grant belongs to
  `specs/backlog/skill-files-readable-by-agents.md`, whose path is
  updated. The three upgrade side effects and the 2.1.169 compaction
  failure are accepted; nothing filed.

### Crossings
<!-- Append one line per crossing. Never edit or delete a line. -->
- start → planning
- planning → dev
- dev → review
- review → closure
- closure → closed

## Requirements

- R1 (D1). After `just install`, a Claude Code session on 2.1.314 and on
  2.1.169 gets the genome at start, each turn's expression, and the sleep
  mark at end, and its MCP server connects. The debug log shows no
  "escapes plugin directory" and no `path-traversal`. After `just
  rollback` or a new install, Claude Code runs the live generation.
- R2 (D2). A second prompt with nothing changed gets nothing. A session
  never told a record is never told `record: none here`; one never told a
  persona is never told `persona: none known here`. Each is said once on
  leaving, as before.
- R3 (D3 and gaps). "For any persona part" says `serve` is a string
  holding the JSON array. Each of the 13 gaps is closed in the section or
  by a pointer to the one project.md paragraph that holds it. The section
  stays lean and names no other part.
- R4. A proposal for a bring-in whose source records no remote says so
  plainly, instead of "on its source".

## Test Strategy

- D1, live (ignored, needs Claude Code; run with `CLAUDE_BIN` on both
  binaries): a new test in `resources/build-tool/tests/` builds two
  profile generations (a profile link to dirs outside the state dir, as
  nix places them), runs `Plugins::install` with the real Claude CLI in a
  temp HOME, then starts `claude -p` against a loopback stub with no
  `--plugin-dir`. It asserts the plugin's MCP server connects and the
  first request carries the genome once. Then it moves the profile link
  back a generation, as `just rollback` does, and asserts, with no
  re-registration, that the next session's hooks are the previous
  generation's. It must fail on 2.1.314 before the fix.
- D1, unit: build-tool's Claude marketplace path is the profile's
  marketplace dir; codex's stays the links dir; a Claude marketplace
  recorded at the old links dir is re-added at the new path; the
  `additionalDirectories` grant follows it (the old element is swept as
  mAId's).
- D1, end to end: `resources/tests/rollback` updated for the grant path;
  it runs real installs, generations and rollbacks.
- D2, unit: in `entry.rs`, a session with an acting persona and no record:
  turn 1 says the persona, turn 2 says nothing; then leaving says
  `persona: none known here` only, never `record: none here`. The
  existing record test keeps its "leaving is said once" checks.
- D2, live: a `claude_live.rs` case with an acting persona (the fixture's
  second part gets a location signal on the work folder); the second
  prompt's request carries no new `[my-harness]` line.
- D3: reading checks. Every key, path and command the section names is
  grepped against the code; the `serve` wording is checked against
  `/tmp/harness-core-macro/probe-serve-array/`.
- Repros: the report's D1 and D2 repros re-run after the fix; results in
  the Session Log.

## Design

- **D1.** Claude Code 2.1.314 refuses a file named by the manifest
  (the manifest itself, `hooks`, `mcpServers`) that resolves outside the
  plugin dir and its marketplace dir, the marketplace dir taken as
  resolved. The links dir resolves to itself, its files to the store, so
  the hooks file is refused. Registering Claude's `maid` marketplace at
  `<profile>/share/maid/marketplace` passes on both versions: the dir and
  its files resolve into the same store path. Claude records the path as
  given, through the profile link, so it follows each generation. So
  `Plugins::root` becomes per agent: Claude the profile's marketplace dir,
  codex the links dir (`marketplace_root`, unchanged). `reads::plugin_dir`
  follows Claude's. install already re-adds a marketplace recorded
  elsewhere, and already sweeps mAId's old grant element. No copy, no
  write into the profile or the plugin copies.
- **D2.** The reinforcement becomes two items, `reinforce.persona` and
  `reinforce.record`, each with its own "was this session told it"
  (`sent_any` on its own id). Each is the known line, or its "none here"
  line only when that item was sent before, or nothing.
- **D3.** Edit only `## For any persona part`: fix `serve`; add short
  clauses or `(Architecture, "<paragraph>")` pointers per gap. The three
  unclear gaps get the least answer, each a Decision Log line:
  - 10: a part with no public remote records `source` with `kind` and
    `rev` and no `at`; its proposals say no remote is recorded.
  - 11: a part places each unit on each shell by that shell's own way and
    records it under `harnesses`; `bring_ins` says what it is to the
    ghost. The core loads no unit.
  - 13: a bring-in with nothing to bind omits `values` (absent, null and
    `{}` read the same).
- **R4.** In `adapt.rs`, the two "its source" fallbacks for a source with
  no `at` say `its source (no remote recorded)`. Nothing else: install
  already refuses to record a local path, and the route is unchanged.

## Implementation Plan

- [x] D1: the live test, watched failing on 2.1.314.
- [x] D1: per-agent marketplace root, grant path, unit tests, rollback
      test path; live test green on both binaries.
- [x] D2: split the reinforcement; unit test; live acting-persona case.
- [x] D3: "For any persona part" edits; R4 message.
- [x] Gates: `just ci`, `resources/tests/rollback`, live tests with
      `--ignored` on both binaries, the report's D1 and D2 repros.

## Session Log

- Grounding: read the report, its scripts, project.md (My harness,
  Deployment, For any persona part), `deploy.rs` `Plugins`, `reads.rs`,
  `entry.rs` expression and reinforce, `fit::persona`, `claude_live.rs`,
  `adapt.rs` proposals, `stages.rs` sources.
- D1 tried in scratch HOMEs under `/tmp/macro-fixes/exp` (`try.sh`), the
  real profile store of the Macro run, one `-p` session per variant
  against the stub:
  - links dir (today): 2.1.314 "escapes plugin directory" for
    `./hooks/claude.json`, `path-traversal` for MCP, 0 hooks, no genome;
    skills (7) and the agent still load; the MCP server still connects.
    2.1.169: 5 hooks, genome.
  - `<profile>/share/maid/marketplace`: 2.1.314 and 2.1.169, 5 hooks,
    genome, MCP connected, no errors. `known_marketplaces.json` keeps the
    path through the profile link.
  - the store path itself: passes, but pins one generation.
  - a real copy: passes, but install would keep a second copy in step.
  - plugin entries linked one level deeper: 2.1.314 refuses the manifest
    itself ("outside the plugin directory and its marketplace
    directory"); the plugin does not load.
  - So MCP, skills and agents are not refused on their own; the
    `path-traversal` MCP line is the plugin's hooks error reported again.
- D1 test (`build-tool/tests/claude_installed.rs`): the existing
  build-tool tests reach the core only as a library, and a hook needs the
  binary, so the test builds it with `$CARGO build -p maid-core` (nested
  cargo works from a test, about a second when built) and takes it beside
  build-tool's binary. The stub is copied from `claude_live.rs`, text
  answers only: sharing it means a module both crates include by path.
  Before the fix: 2.1.314 FAIL (genome 0 times in the first request);
  2.1.169 PASS, the rollback half included. After: both PASS.
- D1 fix: `shared::agent_marketplace`; `Plugins::root` per agent, the
  links dir only for codex; `reads::plugin_dir` follows Claude's.
  `tests/plugins.rs` now expects claude on the profile path and moves a
  claude marketplace left on the links dir (green with the real CLIs);
  `reads_install.rs` and `resources/tests/rollback` take the new grant
  path; the old grant element is swept by the existing "older profile
  path" case. project.md "Plugins" and README say where each agent points.
- Not checked: that Claude reads a skill module under the moved
  `additionalDirectories` grant without asking (needs a model turn that
  reads a file; the old grant had the same shape, a profile-side path
  whose files resolve into the store).
- D2: `reinforce_persona` and `reinforce_record`, ids
  `reinforce.persona` and `reinforce.record`. New unit test
  `a_session_told_a_persona_and_never_a_record_is_never_told_none_here`
  failed on the old code at "a second prompt with nothing changed gets
  nothing", passes now; the record test takes the two items. New live
  case `with_a_persona_acting_a_second_prompt_with_nothing_changed_gets_nothing`
  (`World::acting`, the second part's skill names the work folder): on
  the old code, 2.1.314 FAIL at the `[my-harness]` count; now PASS on both
  binaries. project.md "Expression" says the two are tracked apart.
- `claude_live.rs` on 2.1.169, before this stream (the base tree, run
  apart): `a_session_starts_with_the_genome...` failed only on the MCP
  status line, which on 2.1.169 has no `"source":"plugin"`; the check
  now matches name and status only. `a_compaction_places_the_genome_again_once`
  fails there before and after: 2.1.169 sends no request after `/compact`
  in stream-json input. Left as found.
- D3: "For any persona part" now closes each gap. In the text: 4 `serve`
  as a string, with an example; 5 and 11 placing units and `harnesses`;
  6 `claude mcp add -s user`; 10 `at` omitted with no public remote; 13
  `values` omitted when nothing is bound, every value a string; 3 the
  standard's own fields. By pointer: 1 and 12 "Expression" (with the flat
  key example), 2 "The bring-in contract", 7 "Claude's own automatic
  memory is turned off", 8 "The memory index, as a reader such as the
  selector takes it", 9 "Memory". Checked against code: `values` is a map
  of strings (`parts.rs`), and the Macro probe's array `serve` reads
  `unreadable: invalid type: sequence, expected a string` (re-run on
  `/tmp/harness-core-macro/probe-serve-array/parts`); absent and null
  `values` read as none; `claude mcp add -s user` is what install runs.
- R4: `adapt.rs` `source_at`, one place for the batch line and the
  proposal file; unit test `a_proposal_says_when_its_source_records_no_remote`.
- Gates (on `5c0679a`..HEAD; `initiative/harness-core` still at 73eb872,
  so no rebase):
  - `just ci`: green.
  - `resources/tests/rollback` (claude 2.1.314 on PATH): 35 PASS, 0 FAIL.
  - Live, `CLAUDE_BIN` 2.1.314: `claude_live` 6/6, `claude_installed` 1/1.
    2.1.169: `claude_live` 5/6 (the compaction case, failing before
    this stream too), `claude_installed` 1/1.
  - `tests/plugins.rs` install case with the real CLIs: PASS. Its two
    vendor cases were not run: they clone the vendor repo over the
    network.
  - D1 repro, in a fresh scratch HOME (`/tmp/macro-fixes/r`) installed by
    `just resources::install-skills claude` from this branch's build:
    2.1.314 and 2.1.169 each "Registered 5 hooks", no "escapes" or
    `path-traversal` line, the genome once in the request, MCP connected,
    a sleep mark per session. Claude's marketplace is recorded at
    `<profile>/share/maid/marketplace`.
  - D2 repro, on HOME A's parts with a scratch delivery dir: the Macro
    run's core says turn 2 `acting through hobby ... record: none here`;
    this branch's core says nothing on turns 2 and 3.
- Code Review Gate (one `correctness` lens; no named-risk path): PASS
  WITH NOTES, no Must Fix. Applied:
  - What's Missing 1 (medium): an install that only moves claude's
    marketplace runs no plugin update. `claude_installed.rs` now starts
    from an older install (claude on a links dir, plugin installed),
    runs install on the same generation, and asserts the genome and MCP.
    PASS on 2.1.314 and 2.1.169.
  - Should Fix 1: install and uninstall make and remove the links dir
    only when codex is selected.
  - Should Fix 2: the "no re-registration" check compares only the `maid`
    entry's source and install location.
  - Should Fix 3: `core()` says the test runs under the dev profile.
  - Not applied, Should Fix 4: a session in flight at upgrade holds a
    `reinforce@` mark, so its next turn sends the reinforcement once
    more. Accepted: at most one re-send per old session.
  - What's Missing 2 (the moved grant read live): left, as "Not checked"
    above says.
  - After: `just ci` green, `tests/plugins.rs` install case PASS,
    clippy clean, `resources/tests/rollback` 35 PASS, 0 FAIL again.
- Review briefing (kreviewkit, fresh context): two defects, both fixed.
  `claude_installed.rs` inherited `CLAUDE_CONFIG_DIR` into install's CLI
  calls, which set only HOME; the test now unsets it. The backlog item
  `skill-files-readable-by-agents` named the old marketplace path; it now
  names Claude's new one. Re-ran `claude_installed` on both binaries.
- The 2.1.169 compaction failure in `claude_live` predates this stream
  and shows only on that older version; nothing filed (ringmaster).
- project.md Testing: `claude_installed` described, and both live tests
  named as the run after any Claude Code upgrade (ringmaster's ask at the
  review gate). Layout lists the new test.
- Closure: no `- [ ]` left. project.md verify, per section touched:
  Architecture ("Expression", "Plugins"), Testing (the live tests),
  Layout (the new test) and "For any persona part" carry this feature's
  edits; Deployment and Hard constraints are still true (install stays
  checkout-free, writes nothing into the plugin copies); Tech Stack,
  Mission and Agent Development untouched; no AGENTS.md. Backlog answer
  (my judgement, under the ringmaster): none closed out. The nearest,
  `skill-files-readable-by-agents`, is about reads prompting, which this
  stream did not check live, so it stays, with its path updated.
  `harness-source-declared` is about the harness's source by name, not
  a missing remote, so it stays.

## Decision Log

- Decision: register Claude's marketplace at the profile's marketplace dir
  · why: passes on both versions, follows generations through the profile
  link, writes nothing new · considered: a real copy in the links dir
  (passes, but a second copy install must keep in step and rollback must
  redo), the store path (pins a generation).
- Decision (ringmaster, at the planning gate): the installed-plugin live
  test lives in `resources/build-tool/tests/` and calls install directly;
  no dev-dependency cycle · why: build-tool owns install, and a cycle
  between the two crates is a cost for one test · considered: in
  `claude_live.rs` with build-tool as a dev-dependency (compiles, but a
  cycle), a check in `resources/tests/rollback` (no stub outside Rust).
- Decision (gap 10): a part with no public remote records `source` with
  `kind` and `rev`, no `at` · why: `at` never holds a local path, and the
  route needs only the kind · cost if wrong: its proposals name no place.
- Decision (gap 11): a part places each unit on each shell itself and
  lists it under `harnesses`; `bring_ins` only says what it is to the
  ghost · why: the core loads no unit, and the turn catalog only points
  at units the shell already has.
- Decision (gap 13): a bring-in with nothing bound omits `values` · why:
  absent, null and `{}` read the same, so the least is to write nothing.
- Decision (mAId from a clone with a local origin): only a clearer
  message, `its source (no remote recorded)` · why: install already
  refuses to record a local path, and the route is unchanged; a person
  who wants a place sets a public origin.
- Ruling (ringmaster, review gate): keep the two layouts; codex moves only
  if it starts refusing too. The three upgrade side effects are accepted.
  The gap 10, 11 and 13 answers are ratified.
- Decision (amendment): `claude_live.rs`'s MCP check drops
  `"source":"plugin"` · why: 2.1.169 omits it, and the brief runs the live
  tests on both binaries; the other assertions of that test were hidden
  behind it there.
