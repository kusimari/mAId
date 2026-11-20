# Feature: ghost-in-shell-layout

## Git Setup

- Branch: `feat/ghost-in-shell-layout/work`
- Base: `initiative/harness-core`

## Feature Brief

Lay the core's code out as the concept, so a reader can follow the design
in the code: `ghost-in-shell/` with the crates `ghost` (what forms the
ghost), `infusion` (the five entry points), `shell/claude` (the wiring into
Claude Code, with the ghost's Claude plugin assets) and `tests` (every test
that spans files or runs Claude Code or the binary). Only moved and
renamed: no behaviour change, reviewable as such.

Part of initiative: [[harness-core]]

## Handoff

- **Stage:** closed
- **Ready for:** the ringmaster's squash-merge into
  `initiative/harness-core`
- **Carry forward:** for stream 12: `build-tool` takes `shell-claude` as
  a library only for `ConfigDir`, so the plain installer compiles
  `infusion` too; when stream 12 splits the plain and ghost installs it
  decides whether `ConfigDir` moves into a small crate the plain
  installer can take alone (user's ruling: accepted for now). The
  initiative spec needs three edits the ringmaster owns: in "System design > The layout", memory is in
  `infusion/adaptation/`, not `ghost` (the user's ruling), and its
  "*Built today:* one crate, `resources/core`…" paragraph is now stale;
  in "Building the packages", the diagram's
  `resources/core ─► maid-core` is `ghost-in-shell/shell/claude ─►
  maid-core`. Four files show as an add and a delete rather than a
  rename, because they were split:
  `core/src/{config,entry,source,lib}.rs`.
- **Deliberately left:** the binary name `maid-core`, the `@maid-core@`
  placeholder and the `maid-core map` subcommand (stream 12 renames
  them). D11 lists the three outward differences. Four things the review
  raised are left as the user ruled: build-tool taking `shell-claude` for
  the config dir (Carry forward, above), Claude's own `source` field
  sharing `Where.source` (Decision Log), `Ghost::from_roots` with one
  caller, and the fixture copy helper existing twice (a crate cannot
  depend on the tests crate that depends on it).

### Crossings
<!-- Append one line per crossing. Never edit or delete a line. -->
- planning → dev
- dev → review
- review → dev · RETURN · fault: implementation · issue: the code-review
  panel and the briefing found a behaviour change on malformed input, a
  test that asserted nothing, a nested build that could rewrite the lock,
  two drift holes the two-root reading and the CLI mirror opened, and
  stale prose · fix: refuse a location of the wrong type, assert fixed
  values, pass `--locked`, guard both drifts, correct the prose · done
  when: the gates pass again and the plugin tree still matches
- dev → review
- review → closure

## Requirements

- R1. Every command, hook answer, MCP answer and file the core reads or
  writes is the same as before, from the same binary name.
- R2. `nix build .#default` gives a profile whose `bin/maid-core` and
  plugin files are the same as before, apart from the core's store path
  and the plugin version that hashes it.
- R3. Install, uninstall, status, rollback and the skill checks behave as
  before, the reflect skill included.
- R4. Each crate and file is named for the concept it holds, as the
  initiative's System design maps it.

## Test Strategy

- Every test that passed before passes after, same count or better.
  Before (at 96c01d0, `cargo test --workspace`): maid-core 141 unit + 28
  integration passed, 8 ignored; build-tool 373 passed, 4 ignored; kaimux
  53. After: the same tests in their new crates, plus one (D4).
- `just ci` and `nix develop -c resources/tests/rollback` pass.
- On Claude Code 2.1.319: `claude_live` (now `-p ghost-in-shell-tests`)
  and `claude_installed` pass.
- Built plugin tree, before and after: `diff -r` with the core's store
  path normalised; only the core path and the version may differ.
- A grep for `resources/core`, `maid_core`, `maid-core` as a crate,
  `resources/content/adapters` and `content/agents` finds no stale path.

## Design

```
ghost-in-shell/
  README.md        a few lines: what this is, and a link to the System design
  ghost/           crate ghost: no shell, no entry point
  infusion/        crate infusion: depends on ghost; knows no shell
  shell/claude/    crate shell-claude: depends on ghost and infusion; builds bin maid-core
    plugin/        the ghost's Claude plugin assets
  tests/           crate ghost-in-shell-tests: lib.rs (helpers) + one file per test target
```

Dependencies run `ghost` ← `infusion` ← `shell-claude`; `build-tool`
takes `ghost` (part list schema, contract, readiness, time) and
`shell-claude` (the config dir, the project dir name) as libraries.

### File map

| Old | New |
|---|---|
| `resources/core/Cargo.toml` | `ghost-in-shell/{ghost,infusion,shell/claude}/Cargo.toml` |
| `src/lib.rs` (module list) | each crate's `lib.rs`: its modules and re-exports |
| `src/lib.rs` (`fixtures`, test only) | `shell/claude/src/lib.rs` (test only) |
| `src/config.rs` `Config`, `path_var` | `ghost/src/config.rs` (no `claude` field; `path_var` public) |
| `src/config.rs` `ClaudeConfig` | `shell/claude/src/config_dir.rs` `ConfigDir` |
| `src/contract.rs` | `ghost/src/contract.rs` |
| `src/parts.rs` | `ghost/src/parts.rs` |
| `src/map.rs` | `ghost/src/ghost.rs`: `Ghost`, `Ghost::from_roots` |
| `src/environment.rs` | `ghost/src/environment.rs` |
| `src/time.rs` | `ghost/src/time.rs` |
| `src/entry.rs` `Call`, `Carrier`, `Slice`, `Item`, `Ctx` | `infusion/src/call.rs`, with `Where`, `Close` and `sub_session` |
| `src/entry.rs` `genome`, `Picture`, `Line`, `Pending`, `picture*`, `line`, `pending`, `render*` | `infusion/src/genome.rs` |
| `src/entry.rs` `expression` and its reinforce, rules, clip, fit id | `infusion/src/expression.rs` |
| `src/entry.rs` `deliver`, `deliver_within`, `sent` | `infusion/src/expression/delivery.rs` |
| `src/delivery.rs` | `infusion/src/expression/delivery.rs` |
| `src/fit.rs` | `infusion/src/expression/fit.rs` |
| `src/select.rs` | `infusion/src/expression/select.rs` |
| `src/record.rs` | `infusion/src/expression/record.rs` |
| `src/here.rs` | `infusion/src/expression/here.rs` |
| `src/experience.rs` | `infusion/src/experience.rs` |
| `src/entry.rs` `experience`, `locate`, `pack`, `summary` | `infusion/src/experience.rs` |
| `src/source.rs` `SessionSource`, `Located` | `infusion/src/experience/source.rs`, plus the `Sources` trait (D1) |
| `src/source.rs` `named`, `DEFAULT` | `shell/claude/src/transcript.rs` (`Transcripts`) |
| `src/entry.rs` `Call::session_from_input` | `shell/claude/src/hook.rs` (D9) |
| `src/sleep.rs` | `infusion/src/sleep.rs` |
| `src/entry.rs` `sleep` | `infusion/src/sleep.rs` |
| `src/adapt.rs` | `infusion/src/adaptation.rs` |
| `src/entry.rs` `adaptation`, `briefed` | `infusion/src/adaptation.rs` |
| `src/adapt/tests.rs` | `infusion/src/adaptation/tests.rs` (`mod tests`, moves' tests with it: they share `World`) |
| `src/adapt/moves.rs` | `infusion/src/adaptation/moves.rs` |
| `src/ledger.rs` | `infusion/src/adaptation/ledger.rs` |
| `src/memory.rs` | `infusion/src/adaptation/memory.rs` (D2) |
| `src/files.rs` | `infusion/src/files.rs` |
| `src/claude.rs` | `shell/claude/src/transcript.rs` |
| `src/mcp.rs` | `shell/claude/src/mcp.rs` |
| `src/main.rs` | `shell/claude/src/main.rs` (bin `maid-core`, hook output) |
| `tests/*.rs` | `ghost-in-shell/tests/tests/*.rs`, with `tests/src/lib.rs` for what they share |
| `tests/fixtures/claude/` | `ghost-in-shell/tests/fixtures/claude/` |
| `src/entry.rs` tests of sleep and experience over a transcript | `tests/tests/entry_points.rs` (they span crates) |
| `resources/content/adapters/claude/{hooks,mcp,my-harness}.json` | `shell/claude/plugin/` |
| — | `infusion/fixtures/session.json`: the fixture session packed (D4) |
| `resources/content/agents/reflector.md` | `shell/claude/plugin/agents/reflector.md` |
| `resources/content/skills/reflect/` | `shell/claude/plugin/skills/reflect/` |

Unit tests go with the code they test: the entry-point tests in
`entry.rs` split with their functions, and the context and call helpers
they share sit in a test-only `testing` module in `infusion/src/lib.rs`.
The three that read a transcript span crates, so they go to the tests
crate (`tests/tests/entry_points.rs`).

### Decisions

- D1. **Session sources come from the shell.** `infusion` defines
  `experience::source::Sources` (the source a call names); `Ctx`
  gains `sources: Box<dyn Sources>`. The shell's own holds Claude's
  projects dir and is today's `source::named`, word for
  word, as `shell_claude::transcript::Transcripts`. `Config` loses its
  `claude` field; `main` reads the config dir at start as before, so a
  relative `CLAUDE_CONFIG_DIR` still fails every command.
- D3. **Whole files move with `git mv`;** only `entry.rs`, `config.rs`,
  `source.rs`, `lib.rs` and `adapt/tests.rs` split, so rename detection
  keeps each moved file's diff to its `use` lines.
- D4. **Adaptation's tests read the fixture session already packed**
  (`infusion/fixtures/session.json`), since `infusion` cannot use the
  Claude packer. A `shell-claude` test asserts the packer gives exactly
  that file for the fixture transcript, with its temp root written as
  `/fixtures/claude`, so the two cannot drift. One test more.
- D5. **The tests crate builds the binary** with `cargo build -p
  shell-claude --bin maid-core`, once per test binary, as
  `claude_installed` does today, since `CARGO_BIN_EXE_*` is only for the
  bin's own package.
- D6. **The reflect skill moves; build-tool reads two skill roots.** The
  flake copies it into the plugin's `skills/` as before. `validate`,
  `checkout_skill` (check-skills) and the shipped-content tests also
  read `ghost-in-shell/shell/claude/plugin/skills`, so reflect is checked
  as before.
- D7. **Renames**, behaviour identical:
  `maid_core::config::ClaudeConfig` → `shell_claude::config_dir::ConfigDir`;
  `maid_core::claude` → `shell_claude::transcript`;
  `maid_core::source::DEFAULT` → `shell_claude::transcript::DEFAULT`;
  `maid_core::entry::*` → `infusion::{call, genome, expression,
  experience, sleep, adaptation}`, each entry point re-exported at the
  crate root; `maid_core::adapt` → `infusion::adaptation`;
  `ghost::config::path_var` made public for `ConfigDir`.
- D8. **`Ghost::from_roots` is the one constructor** (user's ruling).
  `map.rs` becomes `ghost/src/ghost.rs`, `Map` becomes `Ghost`, and
  `fuse` is gone: callers pass the parts dirs they have. For the one
  parts dir there is today the answer is byte for byte what `fuse` gave —
  same order, same clashes, same refusals, and a test holds one root
  equal to it.
- D9. **The call is shell-neutral** (user's ruling). `infusion::Call`
  carries typed fields — `at: Where` (source, transcript, cwd),
  `prompt`, `close: Option<Close>` — and `input` only for what the
  person's own input says (`decide`, `dismiss`, `findings`,
  `corrections`). `shell-claude`'s `hook::read` turns Claude's JSON into
  them and is the only place those field names appear; the sub-session id
  rule stays `infusion::call::sub_session`. The CLI's clap enums live in
  `shell/claude/src/main.rs` and map to `infusion`'s plain ones. A test in
  `hook.rs` reads every file of `infusion/src` and fails on any of those
  names or a `clap` dependency. Outwardly nothing changes: the same hook
  answers, the same CLI.
- D11. **Three outward differences, each deliberate.** The stream is
  otherwise byte-identical (see the Session Log's side-by-side run).
  1. A location of the wrong type is refused instead of dropped. Before,
     `experience --input '{"cwd": 5}'` read the value as naming a session
     and failed in the Claude source ("name the session, or the working
     dir it ran in"); the typed call would have made it answer as a call
     that named nothing. `hook::read` now refuses `source`,
     `transcript_path` or `cwd` of another type than a string: "cwd must
     be a string (got 5)". Both exit 1; only the message differs.
  2. `maid-core --help` and `maid-core map --help` say "the ghost" where
     they said "the map" and "the harness map". The `map` subcommand's
     name is unchanged; stream 12 renames the verbs.
  3. `maid-core expression --help` no longer says "not filled yet:
     answers nothing", which has been false since stream 2.
- D10. **`memory.rs` goes with adaptation** (user's ruling): the
  memory-server client that writes a habit is
  `infusion/src/adaptation/memory.rs`; what the ghost knows about memory
  (store, index, readiness) stays in `ghost`, in parts, fusion and
  environment. Adaptation is the client's only caller, and its test stub
  is shared with adaptation's tests, which a test-only item in `ghost`
  could not reach.

## Implementation Plan

- [x] 1. Workspace and the three crates: `git mv` each whole module, split
  `entry.rs`, `config.rs`, `source.rs`, `lib.rs`; `Sources` (D1); fold
  `adapt/tests.rs`; the briefed fixture and its sync test (D4).
- [x] 2. The tests crate (D5), the plugin assets, build-tool's imports and
  second skill root (D6), the flake's fileset, paths and `-p`.
- [x] 3. project.md Layout, Testing, Tech Stack and every path naming an
  old location; README; `ghost-in-shell/README.md`.
- [x] 4. Verify: test counts, `just ci`, rollback, the live tests on
  2.1.319, the built plugin diff, the stale-path grep.

## Session Log

- 2026-11-09, closure: no backlog item is closed by this stream (it came
  from the initiative's Streams table, not the backlog), and the one it
  touches, `harness-source-declared`, gains a line: the `claude-adapter`
  literal now also breaks `infusion`'s "knows no shell". project.md was
  verified section by section: Architecture, Tech Stack, Layout and
  Testing were rewritten with the move; Hard constraints now names the
  ghost's plugin dir as a second place a shipped skill is authored;
  Mission, Deployment and Agent Development are unchanged (the verbs,
  the install path and the kdevkit block did not move).
- 2026-11-09: the Code Review Gate panel (correctness, security,
  comment-hygiene, fresh context, read-only) and the review briefing both
  came back PASS WITH NOTES with no Must Fix. Acted on every note: the
  malformed-location refusal (D11, with a test), fixed expected values in
  the several-roots test, `--locked` on both nested `cargo build` calls, a
  guard refusing one skill name under two roots (with a test), a test
  holding the CLI's enums to infusion's own (an exhaustive match over
  infusion's, so a new value does not compile), `ghost-in-shell/` added to
  the repo-pointer list a shipped skill may not name, the README kept out
  of the core's build inputs (checked: a README edit leaves the core's
  store path the same), and the prose the panel named. Re-ran: `just ci`
  605 passed / 0 failed / 12 ignored, the plugin tree unchanged but for
  the version hash.
- 2026-11-09: verified after the move, on the rebase onto
  `initiative/harness-core` (8b49da5). `just ci` passes: **602 tests
  passed, 0 failed, 12 ignored** (before: 595 / 0 / 12). No test was
  lost — the two names that changed are renames
  (`the_map_does_not_depend_on_the_order_files_are_written` →
  `the_ghost_…`, `checkout_skill_is_agent_independent` →
  `…_and_reads_either_root`); seven are new (the several-roots ghost, the
  sub-session id, three for Claude's hook input, the infusion guard, the
  packed-fixture sync, and a session with no Claude dir). `resources/tests/rollback`:
  35 PASS. On Claude Code 2.1.319: `claude_live` 6/6,
  `claude_installed` 1/1. `nix build .#default` →
  `/nix/store/wk6y91rgg…-maid`: the plugin tree has the same paths and
  file modes as before, and every file is byte-identical with the core's
  store path normalised, except the two plugin manifests' `version`,
  which hashes that path. Every `maid-core <cmd> --help` is identical
  except `map`'s one-line description. `cargo run -p build-tool --
  validate` reads 7 skills (6 under `resources/content`, reflect with the
  ghost), and `just resources::verify-skills-dry reflect` passes.
- 2026-11-09: the old binary and the new one answered identically, side by
  side on the same kind of temp world (output with the root normalised,
  and the exit code): `genome` plain, `--json`, twice on an append
  carrier, `--slice sub-session` from a sub-agent payload, `expression`
  plain and as a `UserPromptSubmit` hook, `genome` as a `SessionStart`
  hook, `sleep` from a `SessionEnd` payload and for an unknown session,
  `experience`, `adaptation`, `ledger --json`, `map`, `environment`
  (both forms), an unknown subcommand, a relative `CLAUDE_CONFIG_DIR`
  (still refused, same message, exit 1) and no `HOME` at all.
- 2026-11-09: baseline at 96c01d0. `cargo test --workspace`: maid-core lib
  141, adaptation 9, claude_hooks 6, cli 7, mcp 2, shipped_fit 3,
  claude_paid 1 (2 ignored), claude_live 6 ignored; build-tool 357 + 16
  integration (4 ignored); kaimux 53. Claude Code 2.1.319: `claude_live`
  6/6, `claude_installed` 1/1. `nix build .#default` →
  `/nix/store/s8fnzdmzbi1lsb0by3infrabdrr6kl32-maid` (kept as
  `/tmp/gis-before` for the diff). `resources/tests/rollback`: 35 PASS,
  "all rollback checks passed".

## Decision Log

- Ruling (user, via the ringmaster, 2026-11-09): no README table; the
  initiative spec's System design holds the crate and file map, and this
  stream reports its map and renames there rather than editing that spec.
- Ruling (user, 2026-11-09): `Ghost::from_roots` is the one constructor
  and takes several roots, though one caller passes one today; the
  memory-server client goes with adaptation; the call `infusion` takes is
  shell-neutral and this shell maps its own input onto it, guarded by a
  test · why: the code should read as the concept, and a shell's field
  names are the shell's · cost if wrong: the typed call is a wider change
  to undo than a move
- Ruling (user, 2026-11-09): Claude's own `source` field (on
  SessionStart: `startup`, `resume`, `clear`, `compact`) stays in
  `Where.source`, which `infusion` reads as the session source's name,
  as it was before the move · why: the move keeps what it found, and no
  call that carries it reaches a session source today · cost if wrong: a
  payload that carries both fails with "no session source for startup
  yet", which stream 11 or the other shells' adapters would surface
- Ruling (user, 2026-11-09): `build-tool` taking `shell-claude` for
  `ConfigDir` is accepted for now; stream 12 decides whether the
  resolver moves into a crate the plain installer can take alone · why:
  one resolver for Claude's config dir is worth the coupling until the
  installs split · cost if wrong: the plain installer compiles the
  ghost's crates it does not use
