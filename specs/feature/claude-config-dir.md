# Feature: claude-config-dir

## Git Setup

- Branch: `feat/claude-config-dir/work`
- Base: `initiative/harness-core`

## Feature Brief

Fix R3-1 from Macro test run 3: with `CLAUDE_CONFIG_DIR` set, install,
uninstall and status still read and write Claude Code's user config under
`$HOME`, so a run cannot be isolated. They now use the dir Claude Code
itself uses. The paid tests isolate the same way: the shell's own `HOME`
and model access, a scratch `CLAUDE_CONFIG_DIR` and the ghost's own
locations, never a scratch HOME. Scope: the report's run 3 section
(`/tmp/harness-core-macro/report.md`).

Part of initiative: [[harness-core]]

## Handoff

- **Stage:** closed
- **Ready for:** nothing; the ringmaster squash-merges into
  `initiative/harness-core`
- **Carry forward:** none open. Recording which config dir an entry went
  to is `backlog/claude-config-dir-in-records.md`. The config-dir rule and
  the paid test's isolation are in project.md.
- **Deliberately left:** the reap of an old `~/.claude/skills` link stays
  at `$HOME` (where older mAId made it; not wanted elsewhere). A relative
  `CLAUDE_CONFIG_DIR` fails a non-Claude install too (ruled acceptable).

### Crossings
<!-- Append one line per crossing. Never edit or delete a line. -->
- start → planning
- planning → dev
- dev → review
- review → dev · RETURN · fault: design · issue: plugin CLI calls inherited `CLAUDE_CONFIG_DIR` from the process, a second mechanism · fix: route them through `claude_env` with the passed value · done when: a test shows a plugin call removes a dir the value does not name, and `just ci`, `claude_installed`, `claude_live` pass
- dev → closure
- closure → closed

## Requirements

- R1. With `CLAUDE_CONFIG_DIR` set, `install`, `uninstall`, `status`
  (`whole`) and `memory status` / `memory harvest` read and write Claude's
  user settings, its global config and its project dirs in that dir.
  Nothing is read from, or written under, `$HOME/.claude` or
  `$HOME/.claude.json`.
- R2. With it unset or empty, every path is as today.
- R3. A relative `CLAUDE_CONFIG_DIR` is refused with a message, as the
  core already does.
- R4. The paid tests run on the shell's own `HOME` and model access. They
  refuse to start, before any model call, if any location they write
  (Claude's config dir, the ghost's state and parts, their temp world)
  would land under the real `HOME`.
- R5. The report's repro (`install --agent claude --dry-run` under
  `env3.sh`) names no real-HOME path in its plan, and finds the run's own
  memory server state, not the user's.

## Test Strategy

- Unit, core: the resolver with the variable set, unset, empty, relative,
  and with no `HOME`. Each assertion names the path Claude Code was seen
  to use.
- Integration, build-tool (new `tests/claude_config_dir.rs`): through the
  binary, a temp root with `home/` and `claude-config/`, a stub `claude`
  that keeps its files under `$CLAUDE_CONFIG_DIR` as the real one does.
  `install --agent claude`, then `whole --agent claude`, then
  `uninstall --agent claude`. Asserts the grants land in
  `claude-config/settings.json`, the memory server is seen in
  `claude-config/.claude.json`, status reports them present, uninstall
  takes them back, and no `.claude*` entry exists under `home/` at any
  point. Watched failing before the fix.
- Unchanged and green: `plugins`, `conformance`, `resources/tests/rollback`.
  Tests that call the changed functions directly (`reads_install`,
  `memory_install`, unit tests) pass the unset case.
- Every test that spawns the binary removes `CLAUDE_CONFIG_DIR` (beside
  `CODEX_HOME`), so a shell that sets it is never reached.
- Paid: a free, non-ignored test of the refusal (a location under HOME is
  refused, one outside passes). The paid file is dry-run against a
  loopback stub of the model API, with a scratch HOME so the dry run
  itself touches nothing real: the request carries the genome and the
  marker rule, and nothing is written under the scratch HOME. Not run with
  a model.
- Gates: `just ci`, `resources/tests/rollback`, `claude_installed` and
  `claude_live` with `--ignored` on 2.1.314, and the R3-1 repro.

## Design

- **One resolver, in the core.** `maid_core::claude::UserConfig`, built
  from `HOME` and `CLAUDE_CONFIG_DIR` by the core's existing variable rule
  (empty is unset, relative refused):
  - `dir()`: the variable, else `$HOME/.claude`;
  - `global()`: `dir()/.claude.json` when the variable is set, else
    `$HOME/.claude.json`;
  - `settings()`, `projects()`: under `dir()`.
  It keeps whether the variable was set, because the default is not the
  same as setting the variable to `$HOME/.claude` (the global file moves).
  `Config::claude` becomes `UserConfig::from_vars(..).map(projects)`.
- **build-tool takes it as a value, not from the environment.** main
  resolves it once (`shared::claude_config()`, beside `codex_home()`) and
  passes it in. The claude-only stage functions (`cmd_install_memory`,
  `cmd_uninstall_memory`, `cmd_install_reads`, `cmd_uninstall_reads`,
  `cmd_memory_status`) and `memory.rs` / `reads.rs` take `&UserConfig`
  where they took `home`; `harvest` takes it for Claude's projects.
  `settings::claude_settings` and `memory::claude_json` go. Reading the
  variable deep inside would let a unit test with a temp HOME reach a
  real config dir whenever the shell sets the variable.
- **The `claude` it starts gets the same view.** Every `claude`
  build-tool starts (`claude mcp`, status's probe, the plugin and bring-in
  CLI calls in `deploy.rs`) gets `HOME` and `CLAUDE_CONFIG_DIR` set or
  removed from the value (`shared::claude_env`), so the files it reads and
  the CLI it runs agree.
- **Status.** `whole.rs` takes the value beside `home`; Claude's sources
  (`.claude.json`, `settings.json`, and its agents and skills dirs where
  the stub session shows Claude Code reads them) are paths from it.
  Messages show the full path where they showed `~/<file>`.
- **Paid tests.** `claude_paid.rs` stops clearing the environment: it
  keeps `HOME`, `PATH` and the shell's model access as they are, and sets
  `CLAUDE_CONFIG_DIR`, `MY_HARNESS_STATE`, `MY_HARNESS_PARTS` and
  `XDG_CACHE_HOME` into its temp world. A guard canonicalises each of those and the world and
  refuses any under the canonical `HOME`. `MAID_PAID_ENV` goes: nothing is
  passed by name any more. Its doc comment carries the new command.
  A credential kept only in the real config dir is not seen by a scratch
  one; that follows the ruling and is said in the doc comment.

## Implementation Plan

- [x] Settle on the stub where 2.1.314 reads user agents and skills.
- [x] Core resolver and its unit tests; `Config` uses it.
- [x] New binary-level test, watched failing.
- [x] build-tool: `shared::claude_config()`, signatures, spawned `claude`
      env, status sources, harvest; existing tests pass the unset case;
      `CLAUDE_CONFIG_DIR` removed in tests that spawn the binary.
- [x] Paid test: keep HOME, set the locations, the guard and its free
      test, doc comment; dry run against a stub.
- [x] Gates and the R3-1 repro.

## Session Log

- Grounding: read the report's run 3 section and `env3.sh`, the initiative
  spec (streams, rulings of 2026-11-01), project.md, `core/src/config.rs`,
  `core/src/source.rs`, `build-tool/src/{settings,memory,reads,whole,
  harvest,stages,shared,main}.rs`, `core/tests/claude_paid.rs` and how
  build-tool's tests spawn the binary. The core already resolves the
  variable for transcripts; build-tool depends on the core.
- Checked Claude Code 2.1.314 (launcher-free binary, `env -i`, scratch
  HOME and config dir under `/tmp/ccd-probe.*`, no model call):
  - variable set: `mcp add -s user` wrote `$CLAUDE_CONFIG_DIR/.claude.json`;
    `plugin marketplace add` and `plugin install` wrote
    `$CLAUDE_CONFIG_DIR/settings.json` and `plugins/`; nothing was written
    under HOME;
  - `plugin list --json` read `enabled` from `$CLAUDE_CONFIG_DIR/settings.json`
    and ignored a `$HOME/.claude/settings.json` that disabled the plugin;
  - variable unset: `$HOME/.claude.json` and `$HOME/.claude/settings.json`;
  - variable empty: same as unset.
- Stub session (2.1.314, `-p` against a throwaway loopback stub of the
  model API, scratch HOME and config dir, an agent and a skill seeded in
  each): the request named only the config dir's agent and skill, none of
  `$HOME/.claude/{agents,skills}`; nothing was written under HOME. So
  status reads Claude's agents and skills from the config dir too. Install
  places nothing there itself (mAId reaches Claude as a plugin, through the
  CLI, which follows the variable), so nothing it places is missed.
- Resolver: `maid_core::config::ClaudeConfig`; its tests fail when the
  global file ignores the variable (mutated, watched, restored).
- `tests/claude_config_dir.rs`: the dry-run plan test failed before the
  fix with the report's symptom (`done claude mcp personal-memory`, a
  `+` grant already in the config dir); passes after. The install, status
  and uninstall test fails if `settings()` ignores the variable, or if the
  started `claude` loses it (both mutated, watched, restored).
- `whole::check_in` keeps its signature (the unset case), so the
  conformance test is unchanged; `check` takes the config.
- `just ci` green.
- Paid test reworked. Dry run of both paid tests (`env -i`, scratch HOME,
  `TMPDIR` outside it, `MAID_PAID=1`, 2.1.314 against the loopback stub):
  2 requests, both carry the genome (persona `personal`) and the store
  path, the deploy one carries the `DEPLOY-MARKER-7` rule; both tests fail
  on the stub's reply, as they must. The first dry run found Claude Code
  writing its MCP logs to `$HOME/.cache/claude-cli-nodejs/` whatever its
  config dir; with `XDG_CACHE_HOME` set they go there (checked), so the
  test sets it into the world and guards it. After that, nothing under the
  scratch HOME. With `TMPDIR` under HOME the run panics "refused before any
  model call" and the stub saw no request. The guard's free test fails
  when the check is mutated to equality (watched, restored). A failing
  `claude` now shows its stdout too (it printed nothing to stderr).
- Gates on `2eff1d0`..`f0629b2`: `just ci` green; `resources/tests/rollback`
  "all rollback checks passed"; `claude_installed` 1/1 and `claude_live`
  6/6 with `CLAUDE_BIN` 2.1.314.
- R3-1 repro (`env3.sh`, real HOME, own target dir, from this worktree):
  `install --agent claude --dry-run` plans `done claude mcp personal-memory
  -> run3/store` (it was `skip … (not mAId's)`, read from the real
  `~/.claude.json`); the grants are planned against the run's own
  settings, which hold none; no path under the real HOME appears in the
  plan.
- Code Review Gate (correctness lens, the single lens this diff's paths
  get): PASS WITH NOTES, no high. Fixed: a set `CLAUDE_CONFIG_DIR` no
  longer fails on a relative HOME it does not need; the paid test's doc
  says exactly which four locations it redirects; a test now covers the
  started `claude` losing an inherited variable (watched failing).
  Filed: records naming their config dir (medium,
  `backlog/claude-config-dir-in-records.md`). Left: kiro-only install
  resolving Claude's config; Plugins' CLI env (Decision Log). `just ci`
  green after.
- Review briefing's defects, fixed: uninstall now runs through the binary
  in `tests/claude_config_dir.rs`; status with the variable set is tested
  on another part's Claude skill and agent in the config dir (fails when
  either path goes back under HOME; watched, restored); the paid test's
  doc says its guard test runs in `just test`. All gates re-run on the
  clean HEAD after (below).
- Gates on the clean tree at `a75f9fb`: `just ci` green; rollback "all
  rollback checks passed"; `claude_installed` 1/1 and `claude_live` 6/6
  on 2.1.314; the R3-1 repro plans no path under the real HOME.
- Ringmaster review: one fix. `deploy.rs`'s plugin and bring-in CLI calls
  now go through `cli_command`, which gives `claude` the passed value via
  `claude_env`; `Plugins` and `BringIns` carry the config. Test
  `a_plugin_call_gives_claude_only_the_config_dir_its_value_names` (fails
  with the `claude_env` call removed; watched, restored). Gates at
  `df9e7ab`/`79bd752`: `just ci` green (clean tree), `claude_installed` 1/1,
  `claude_live` 6/6 on 2.1.314, rollback all checks passed.
- Ringmaster rulings on the briefing's questions: a relative variable
  failing a non-Claude install is accepted; the four-location guard is
  enough, Macro run 3 is the first real run and snapshots the real HOME;
  the upgrade-path backlog item stands; install's real writes stay tested
  in-process; dropping a bad HOME when the variable is set is fine.
- Closure: project.md gets the config-dir rule (beside Skill reads), the
  paid test (Testing) and the two test files (Layout). Backlog: this
  stream closes none of the existing items (the two that mention a "config
  dir" mean mAId's own); it adds `claude-config-dir-in-records`.

## Decision Log

- Decision: `ClaudeConfig` sits in `maid_core::config`, beside the other
  locations, not in `claude.rs` (the session source).
- Decision: Plugins' own CLI calls keep inheriting the process env; only
  the starts that set `HOME` from the passed value (`claude mcp`, status's
  probe) set the variable from it too · why: in a run both are the same
  env, and Plugins never reads Claude's files directly.
- Decision (Test Strategy amended): install's writes are tested through
  the stage functions with the variable set, and through the binary only
  as a dry-run plan · why: install's plugin step needs a `claude` that
  really installs plugins, which a stub does not; status and uninstall do
  run through the binary.
- Decision: the paid command is updated in the test's doc comment and in
  stream 2's Handoff, the pointer the initiative's end runs from;
  project.md's Testing gets it at closure.
- Decision (ringmaster review, supersedes the Plugins line above): every
  `claude` build-tool starts takes the passed value, the plugin CLI calls
  included · why: one mechanism, so the CLI and the files mAId reads agree
  by construction.
- Decision: the resolver lives in the core and build-tool receives it as
  a value · considered reading the variable inside `claude_settings(home)`
  (one line, no signature change); chose the value because that read
  would make every unit test with a temp HOME follow a real config dir
  whenever the shell sets the variable.
- Ruling (ringmaster, planning review): the four defaults accepted: the
  config dir passed as a value; the old `~/.claude/skills` link reaped at
  `$HOME`; harvest in scope (same cause); the paid-test auth note stands
  (stream 3's P1 reached the model with a scratch `CLAUDE_CONFIG_DIR`).
  Where 2.1.314 reads user skills and agents is settled on the stub, and
  anything install places that the variable would hide is in scope.
