# Feature: ghost-package

## Git Setup

- Branch: `feat/ghost-package/work`
- Base: `initiative/harness-core`

## Feature Brief

Split mAId's install in two on Claude Code. mAId is a monorepo of
personal parts: plain resources, ghost bring-ins, plain tooling (simple
wiring) and ghost tooling (install, wire, hook). **Plain** (`plain/`, `just
install`) installs the AI resources (skills, agents, servers, plugins) into
each base harness: the one place that manages what is installed, knowing
nothing of the ghost, a zombie with good tools. **The ghost**
(`ghost-in-shell/shell/claude`, `just ghost install`) sees those resources
as bring-ins, reuses everything plain installed, and installs only the
bring-ins of its own: the adapter, reflect, the reflector, the hooks. When
mAId's plugin is missing, or not this checkout's, it runs plain's install
through `plain` as a library. You launch either plain or the ghost harness;
the ghost is switched on and off with no reinstall. Either side may install
a pinned piece from another repo (pensieve) and records that it does. Each
side validates, stages and installs its package into a prefix in Rust, with
its own profile; nix only supplies the environment and the generations.

Part of initiative: [[harness-core]]

## Handoff

- **Stage:** closed
- **Ready for:** the ringmaster's squash-merge into `initiative/harness-core`
- **Carry forward:** none open; what was still wanted is backlog
  (`justfile-argument-quoting`, `reuse-check-untracked-resources`,
  `installed-cache-upkeep`, `harness-source-declared`), and what
  constrains later work is in project.md.
- **Deliberately left:** codex and kiro carry no ghost
  (`ghost-on-other-shells`); the MCP server keeps the name `my-harness`;
  `install-manifests/` is not migrated to `install-records/` (it never
  reached `main`); the genome does not name plain's revision (the user's
  ruling).

### Crossings
<!-- Append one line per crossing. Never edit or delete a line. -->
- planning → planning · RETURN · fault: design · issue: staging for both
  packages sat in one shared `xtask`, build logic stayed in the flake,
  and the user drew the split differently: `resources/`, `plain/`,
  `ghost-in-shell/` with `shell/claude` installing the ghost and, when
  needed, plain as a library · fix: no xtask; `plain/` (today's
  build-tool) builds, stages, installs into a PREFIX and wires, knowing
  nothing of the ghost; `shell/claude` does the same for the ghost over
  plain's library and reuses or installs plain; the flake only provides
  the environment and runs each Rust install · done when: the layout
  above; no build logic in `flake.nix`; plain builds and installs with
  the ghost absent, and with nix absent (cargo and just into a PREFIX
  under /tmp); the ghost installs both over plain and alone
- planning → planning · RETURN · fault: design · issue: streams 13 and 14
  merged: the ghost reads what Claude Code has installed, never an
  installer's record, so a reuse check on plain's part list, the
  ghost's adapter carried in a record and `Ctx.adapter`, and a git
  revision stamp no longer fit; plain's record is now `InstallRecord`;
  the code must read as the concepts · fix: the reuse check reads the
  `maid` plugin through the adapter and compares its content-hash version
  with this checkout's; the adapter is a declared unit of the ghost's
  plugin; the ghost's own record is an `InstallRecord` written through
  `plain::record`; plain's modules named install, place, record,
  uninstall; the carried-forward renames · done when: the ghost install
  reads no installer record to decide reuse; one test per reason the
  check fails; the rename list holds by grep
- planning → dev
- dev → review
- review → closure

## The layout

```
resources/                 the person's resources: skills/, servers/, overlays/, browser/
                           (today's resources/content flattened, plus resources/browser)
plain/                     crate and bin `plain` (today's resources/build-tool). Uses nothing of the ghost
  src/validate.rs          each resource is what its standard requires, before anything is staged
      stage.rs             the package: copy into a stage dir, manifests, content-hash version
      profile.rs           the profile: nix generations, or a real dir; rollback
      install.rs           install: profile, then place on each agent, then record
      place.rs             how a unit reaches each agent (today's deploy.rs)
      record.rs            `InstallRecord` (today's install_record.rs), any package's
      external.rs          the pinned pieces from other repos plain installs (pensieve, vendor plugins)
      uninstall.rs         remove exactly what the record names
      status.rs            status, and whole-status over every record (today's whole.rs)
      claude_config.rs     `ConfigDir`, `project_dir_name` (from shell-claude)
      check.rs, drive.rs   the skill checks before and after install (today's stages 2 and 4,
                           and harness.rs: driving an agent and scoring its reply)
      memory.rs, settings.rs, reads.rs, harvest.rs, shared.rs   as today; `time`, `find_on` copied in
  Justfile                 `plain::*` verbs (today's resources/Justfile)
  tests/                   its Rust tests, skill fixtures, browser and verify-install scripts
ghost-in-shell/
  ghost/, infusion/        in place; `infusion::call::Ctx` renamed (below)
  shell/claude/            crate `shell-claude`, bin `ghost-claude`
    src/main.rs            the CLI: the five entry points, the on-demand server, the install verbs
        hook.rs, installed.rs, transcript.rs, mcp.rs   as today
        validate.rs        the `my-harness` declarations in resources/ and its plugin
        package.rs         the ghost's package: its plugin staged and copied into a prefix
        reuse.rs           the reuse check: mAId's units in Claude Code, this checkout's or not
        install.rs         the ghost's install: profile, reuse or install plain, its plugin, record
        switch.rs          on and off, at a scope or for one launch
        status.rs          ghost status
        uninstall.rs       remove what its record names, its profile, the adapter's cache
        rollback.rs        its profile one generation back, its plugin updated
    plugin/                the ghost's plugin as authored: .claude-plugin/plugin.json (the
                           adapter's declaration), hooks/hooks.json, .mcp.json,
                           skills/reflect, agents/reflector.md
    Justfile               `ghost::*` verbs
  tests/                   cross-crate tests, and `rollback` (both installs)
Justfile                   shortcuts only
flake.nix                  dev shell, pinned runtimes, the two packages; no build logic
```

Dependencies: `shell-claude → plain`, `shell-claude → infusion → ghost`.
Nothing depends on `shell-claude`; `plain` takes none of `ghost`,
`infusion` or `shell-claude`. Each module's `//!` names its concept and the
spec section it maps to.

## The two installs

```
             PLAIN  (just install)                         GHOST  (just ghost install)
 build      cargo build -p plain                          cargo build -p shell-claude --bin ghost-claude
 package    plain install --prefix P                      ghost-claude install --prefix P
            validate resources/; stage target/stage/plain; validate its plugin; stage target/stage/ghost;
            copy into P                                    copy into P
            P/share/maid/marketplace/ (marketplace maid)   P/bin/ghost-claude (itself)
              plugins/maid/ skills/, servers/, overlays/,  P/share/ghost/marketplace/ (marketplace ghost-in-shell)
              claude and codex manifests (no extensions,     plugins/ghost/ .claude-plugin/plugin.json
              hooks or MCP), version = content hash            (extensions.my-harness: claude-adapter),
            P/share/maid/skills -> the plugin's skills          hooks/hooks.json, .mcp.json
            P/bin/maid-browser-mcp (resources/browser/launch)   (@ghost-claude@ = P/bin/ghost-claude),
            P/bin/<runtime> for each --runtime NAME=PATH        skills/reflect, agents/reflector.md;
                                                                version = content hash
 profile    ~/.local/state/maid/profile                   ~/.local/state/maid/ghost/profile
            nix on PATH: nix build --profile .#default    nix: nix build --profile .#ghost
            else a real dir, P = it (no rollback)         else a real dir, P = it (no rollback)
 wire       claude: maid@maid, personal-memory,           the reuse check (below), plain's install if it fails;
            document-skills, settings, browser MCP        claude: marketplace ghost-in-shell,
            codex: maid@maid, browser MCP                         plugin ghost@ghost-in-shell (user)
            kiro, agy: skill links
 record     install-records/maid.json (InstallRecord)     install-records/ghost.json (InstallRecord,
                                                          through plain::record)
 reads      nothing of the ghost                          each session: what Claude Code has installed,
                                                          through the adapter, and every declaration in it

 flake: plain-bin = cargo build -p plain              ghost-bin = cargo build --bin ghost-claude
        default  = plain-bin/plain install --prefix   ghost = ghost-bin/ghost-claude install --prefix
                   $out --root resources/ --runtime          $out --root its plugin
                   pensieve=… --runtime chrome-devtools-mcp=…
```

**The reuse check** (`shell_claude::reuse`). `ghost-claude install` asks
the adapter (`shell_claude::installed::Claude`, user scope) what Claude Code
has, and finds mAId's units: the plugin `maid` from marketplace `maid`. It
stages plain from this checkout (`plain::stage`, into `target/stage/plain`)
and takes that plugin's content-hash version as **this checkout's
revision**: no git stamp, as the user ruled. Plain is reused when the
plugin is installed, enabled, and at that version. Otherwise the ghost says
which failed (`not installed`, `disabled`, `at 1.0.0-x, this checkout's is
1.0.0-y`, or a Claude Code record whose shape is not understood) and
"installing the resources from this checkout", and runs plain's install
through the library (profile, place, record), before placing its own plugin.
It never reads plain's `InstallRecord`. It records the plugin's version it
reused or installed in its own record. `ghost status` and `ghost rollback`
compare the live plugin's version with that and, when they differ, warn
"plain at X, ghost at Y" and name `just ghost install`. Nothing refuses on
a mismatch, sessions keep working, and the genome does not mention it.

Verbs, each a shortcut over one Rust command run from the checkout:

| Verb | Does |
|---|---|
| `just install`, `uninstall`, `status`, `rollback` | plain, as today; `just status` reports what plain installed, each item still there, registered and runnable |
| `just ghost install` | `ghost-claude validate`; its profile; the reuse check, and plain's install when it fails; place its plugin; write its record |
| `just ghost uninstall` | remove what its record names, then its profile and `<state>/shells/`; plain untouched |
| `just ghost on [user\|project\|local]` | `claude plugin enable ghost@ghost-in-shell --scope …` (default user) |
| `just ghost off [user\|project\|local]` | `claude plugin disable …`; for one launch, `claude --settings '{"enabledPlugins":{"ghost@ghost-in-shell":false}}'` |
| `just ghost status` | installed, which generation; on or off here, at which scope; the revision warning; each bring-in declared, its persona, bound, ready or not; each Claude Code record not understood; pending decisions; sessions waiting |
| `just ghost rollback` | its profile one generation back, its plugin updated; plain untouched; then the revision warning |

## Requirements

- R1. `just install` alone gives Claude Code mAId's skills, MCP servers,
  vendor plugins and settings, with no genome, no prefix and no hooks.
  `claude plugin validate --strict` passes on the `maid` plugin.
- R2. With the ghost installed, every Claude Code session gets the genome
  and each turn's prefix as today, the reflect skill and the reflector,
  and the genome lists the Claude adapter as `[personal, own]`.
- R3. `just ghost install` over plain from the same checkout places
  nothing of plain again. With mAId's plugin absent, disabled, or of
  another revision, it says which and installs plain from this checkout
  first.
- R4. `just ghost off` and `on` switch the ghost for every session, one
  project, or (with `--settings`) one launch, with no reinstall.
- R5. `just ghost uninstall` leaves plain whole: its plugin, servers,
  settings and record unchanged. `just uninstall` with the ghost installed
  removes plain; `ghost status` then says mAId's plugin is not installed
  and that `just install` or `just ghost install` puts it back.
- R6. `just ghost rollback` returns the ghost to its previous install and
  leaves plain; `just rollback` does the same for plain. A generation
  missing what the agents are pointed at is refused.
- R7. With no nix on PATH, `just install` and `just ghost install` work
  through cargo into a real profile dir; only rollback is missing, and it
  says why.
- R8. `just status` reports what plain installed and whether each item is
  still there, registered and runnable, with no bring-ins; `just ghost
  status` reports the same items as the ghost sees them (declared,
  persona, bound, ready) and what the verbs table says, including each
  Claude Code record whose shape is not understood. When the live plugin
  is not the one the ghost was installed with, `ghost status` and `ghost
  rollback` warn "plain at X, ghost at Y"; nothing refuses.
- R9. The binary is `ghost-claude`; nothing installed, documented or
  printed says `maid-core`.
- R10. codex and kiro get plain as today, less the reflect skill (the
  ghost's).
- R11. A `my-harness` declaration that is wrong fails `just ghost install`
  and `just ci`, not `just install`; `just install` refuses only what a
  resource's own standard refuses.
- R12. Each pinned piece from another repo that plain installs
  (pensieve, the vendor plugins) is in plain's record with its repo and
  revision.

## Test Strategy

- `ghost-in-shell/tests/rollback` (moved from `resources/tests`; free,
  real claude and codex CLIs, temp HOME, temp profiles):
  - plain alone, through nix: today's checks less the core's; a spec-only
    commit builds the same store path for each package;
  - plain alone with nix off PATH, into a profile dir under /tmp: the
    plugin listed and current, the record written; `rollback` refuses,
    naming nix;
  - ghost over plain, same checkout: plain's generation and record
    unchanged; the plugin listed; hooks and MCP call the ghost profile's
    binary; a start hook answers with the persona and `claude-adapter`;
  - ghost alone (plain uninstalled first), and ghost over plain from a
    clone one commit behind with a skill changed: each says why and
    installs plain; both then whole;
  - ghost off and on (listed disabled, enabled); ghost rollback (version
    back, plain's unchanged; a generation without `bin/ghost-claude`
    refused);
  - ghost uninstall: plugin, marketplace, record, profile and
    `<state>/shells/` gone, plain whole, its record byte-identical;
  - plain uninstall with the ghost: plain gone, the ghost's plugin still
    listed, `ghost status` says mAId's plugin is not installed.
- `claude_installed` (ignored, real Claude Code, loopback stub, temp HOME),
  on trees staged by `plain` and `ghost-claude` themselves: plain alone
  gets no genome and no `plugin:ghost` hook or server; plain plus ghost
  gets the genome once and the MCP server connects; ghost off, and
  `--settings` off for one launch, give no genome; a ghost generation
  switch and back runs the live generation's hooks, with no
  re-registration.
- `claude_live` 6/6 on 2.1.319 with the renamed binary.
- `claude plugin validate --strict` on the staged `maid` plugin: an
  ignored test in `plain`, and a check in the rollback script.
- Unit: each version changes exactly when a plugin file does; every
  `@ghost-claude@` filled and none left; the staged paths; the reuse
  check, one test per reason (absent, disabled, another version, a record
  not understood) and one where it holds, on Claude-shaped homes; the
  ghost's record round trip through `plain::record`; uninstall steps from
  a record (covered end to end by `rollback`, with and without a
  record); plain's status has no bring-in line and names an MCP item
  whose command is gone; the revision warning, both ways; `ghost-claude
  validate` refuses each bad declaration the conformance set holds, and
  `plain validate` passes them.
- `just ci`; a grep outside `specs/` finds no `maid-core`, `build-tool`,
  `resources/content`, `resources/Justfile`, `@maid-core@`, `maid:reflector`
  or a `Ctx` type; `cargo tree -p plain` has no `ghost`.

## Design

- D1. **Moves, no behaviour change first.** `resources/content/*` →
  `resources/`; `resources/build-tool` → `plain/` (crate and bin `plain`);
  `resources/Justfile` → `plain/Justfile` (`mod plain`, so `just
  resources::x` becomes `just plain::x`); `resources/tests/*` →
  `plain/tests/`, but `rollback` → `ghost-in-shell/tests/rollback`, since
  it drives both installs. `git mv`, so rename detection keeps diffs small.
- D2. **Plain reads as its concepts.** Its modules are renamed and
  `stages.rs` split by stage, as the layout lists; no logic changes in
  this step. `install_record.rs` → `record.rs`, and its package name
  becomes a parameter (`maid` stays mAId's), so another installer writes
  the same format. The records' folder is `install-records/`
  (`INSTALL_RECORDS`). `ConfigDir` and `project_dir_name` move into
  `plain::claude_config`.
- D3. **Plain uses nothing of the ghost.** `plain` drops the `ghost` and
  `shell-claude` crates. `validate` checks only what each resource's
  standard requires (a skill's frontmatter, a manifest's fields); the
  `my-harness` checks (declaration and `rules` pointer) move to
  `ghost-claude validate`. `time::today`/`civil` and `find_on` are copied
  in. whole-status loses the bring-ins and not-understood lines (they move
  to `ghost status`) and gains, for an MCP item, whether its registered
  command still runs (found and executable). Its plugin loses the reflect
  skill, the reflector, the hooks, the MCP config and `extensions`. Its
  skill runner takes extra `--skills <dir>` and `--fixtures <dir>`, so
  `just ghost check-skills` runs reflect's fixture (moved beside the
  ghost's plugin) without plain knowing the ghost.
- D3a. **External pieces** (`plain::external`): one list of what plain
  installs from another repo, each with its repo and pinned revision:
  pensieve from the flake's lock (as today), the vendor marketplace from
  its pinned revision (today's `BRING_INS`). Plain's record names each.
  The ghost's package names none today. A unit's own `repository`
  declaration still decides where its proposals go.
- D4. **`plain install --prefix P`** (`plain::stage`): validate, stage into
  `target/stage/plain` (or `--out`), copy into `P`: the manifests, the
  content-hash version (`1.0.0-` and 12 hex of a sha256 over the plugin's
  sorted paths and bytes; `sha2`, new), the `share/maid/skills` link, the
  browser launcher as `bin/maid-browser-mcp`, each `--runtime NAME=PATH`
  linked into `bin/`. The launcher puts its own dir first on PATH, so a
  linked `chrome-devtools-mcp` is the one it runs. `stage` is public for
  the ghost.
- D5. **`plain::profile`**: with `nix` on PATH, `nix build --profile` of a
  flake attribute, the drop of rolled-back generations, the 30-day wipe,
  and `rollback` with a list of what a generation must hold (today's
  Justfile bash, in Rust). Without nix, a real dir and `install --prefix`
  into it; rollback says it needs nix. One copy, used by both packages.
  `plain install` with no `--prefix` is profile, place, record.
- D6. **`ghost-claude validate`** (`shell_claude::validate`): every
  `my-harness` declaration and `rules` pointer under `resources/` and the
  ghost's plugin, through `ghost::contract`; run by `just ghost install`
  and `just ci`. **`ghost-claude install --prefix P`**
  (`shell_claude::package`): validate the ghost's plugin (reflect through
  plain's skill checks, its declarations as above), stage into
  `target/stage/ghost`, copy into `P`: its own executable as
  `P/bin/ghost-claude`, the plugin with `@ghost-claude@` filled, its
  marketplace, version by `plain::stage`. Default hook and MCP file names:
  no codex shares this dir.
- D7. **The adapter is a unit of the ghost's plugin.** Today's
  `plugin/my-harness.json` becomes `extensions["my-harness"]` in the
  ghost's own `plugin.json` (`name: claude-adapter`, `repository` mAId's),
  and its `job` names the ghost plugin's hooks. `Ghost::from` finds it as
  it finds any declared unit, own; nothing in `ghost` or `infusion`
  changes for it, and no record carries it.
- D8. **`ghost-claude install`** (no prefix, `shell_claude::install`): its
  profile (`plain::profile`, attribute `ghost`), the reuse check
  (`shell_claude::reuse`, above), plain's install when it fails, then
  `claude plugin marketplace add` and `install ghost@ghost-in-shell
  --scope user`, read back, and its record. Its record is an
  `InstallRecord` of package `ghost` in `install-records/`, written
  through `plain::record`: the marketplace and plugin it placed, its
  profile as `root`, and the `maid` plugin's version it was installed
  with (a value on its plugin item). It is the installer's own, for its uninstall and
  status; the ghost's runtime never reads it. whole-status checks it as
  it checks any package's.
- D9. **`uninstall`, `on|off [--scope]`, `status`, `rollback`** beside it,
  one file each. `status` builds the ghost through the adapter, as a
  session does, and prints each bring-in (declared, persona, bound,
  ready), `not_understood()`, and the revision warning.
  `uninstall` removes the items whose marketplace still points at its
  profile, the profile, `<state>/shells/` (backlog
  `installed-cache-upkeep`, that half) and its record.
- D10. **Flake, environment only.** The dev shell; `chrome-devtools-mcp`
  and pensieve pinned; `plain-bin` and `ghost-bin`, cargo builds whose
  sources hold no content or plugin assets; `packages.default` and
  `packages.ghost`, each one Rust install over its content. No
  `writeShellApplication`, `replaceStrings`, manifest or hash in nix.
- D11. **Renames.**

  | Today | After |
  |---|---|
  | binary `maid-core` | `ghost-claude` |
  | placeholder `@maid-core@` | `@ghost-claude@` |
  | subcommand `maid-core map` | `ghost-claude ghost` |
  | crate and bin `build-tool` | `plain` |
  | `resources/content/`, `resources/Justfile`, `resources/tests/` | `resources/`, `plain/Justfile`, `plain/tests/` |
  | `build_tool::install_record` | `plain::record` |
  | `build_tool::{deploy, whole, stages, harness}` | `plain::{place, status, validate/install/uninstall/check, drive}` |
  | `shell_claude::config_dir::ConfigDir` | `plain::claude_config::ConfigDir` |
  | `shell_claude::transcript::project_dir_name` | `plain::claude_config::project_dir_name` |
  | `infusion::call::Ctx` | `infusion::call::Infusion` (backlog `ctx-concept-name`) |
  | agent `maid:reflector` | `ghost:reflector` |
  | `plugin/my-harness.json` | the ghost plugin's `.claude-plugin/plugin.json` |

  Hooks, MCP config, the reflect skill, rollback guards, tests, README and
  project.md follow.

## Implementation Plan

- [x] 1. Moves (D1) and paths, Justfiles, flake fileset, project.md
  Layout; the gates pass with no behaviour change.
- [x] 2. Plain's modules as its concepts; `plain::claude_config` (D2).
- [x] 3. Renames (D11): `ghost-claude`, `Infusion` (`ghost:reflector` with the plugin, step 6).
- [x] 4. `plain::stage`, `plain install --prefix`, `plain::profile`;
  the flake's plain package (D4, D5, D10).
- [x] 5. Plain uses nothing of the ghost (D3), `ghost-claude validate`
  (D6), external pieces (D3a); the adapter into the ghost's plugin (D7).
- [x] 6. The ghost: package, reuse, install, record, switch, status,
  uninstall, rollback, `ghost::*` Justfile; the flake's ghost package
  (D6, D8-D10).
- [x] 7. Tests: `rollback`, `claude_installed`, the strict check;
  project.md and README.
- [x] 8. Verify: `just ci`, `rollback`, the live tests on 2.1.319, the
  grep.

## Session Log

- 2026-11-09, planning: on Claude Code 2.1.319 in a temp
  `CLAUDE_CONFIG_DIR`, with a toy plugin installed at user scope:
  `disable --scope local` writes `enabledPlugins` false in the project's
  `.claude/settings.local.json` and `plugin list` there shows it
  disabled; `--scope project` writes `.claude/settings.json`; a project
  or local entry overrides user (local true, user false → enabled);
  without `--scope` the most specific existing entry is changed;
  uninstall leaves project and local entries behind, and `plugin list`
  then lists nothing. Today's `maid` plugin, copied with `extensions`,
  `hooks`, `mcpServers`, hooks/, claude-mcp.json, reflect and agents
  removed, passes `claude plugin validate --strict` (only the plugin
  directory's README, LICENSE and icon notes, which do not fail it).
- 2026-11-10, planning (a new session, on `ed75861`): read streams 13 and
  14. The genome brackets a bring-in with its part's persona, else the
  part's name (`infusion/src/genome.rs`), and a part is named by its
  repository's last segment, so the adapter in mAId's plugin shows today
  as `[personal, own]`. `build-tool` takes `shell-claude` for `ConfigDir`
  (seven files), `project_dir_name` (harvest) and `Claude` (whole-status's
  bring-ins only); it takes `ghost` for `contract` (validation), `time`
  and `find_on`. `Ctx` appears 42 times in 12 files. The maid plugin's
  version today hashes the hooks, which name the core's store path; after
  the split it hashes only `resources/`, so a nix and a cargo install of
  the same tree give the same version.

- 2026-11-10, dev: counts, `just ci`: 651 passed, 13 ignored (626 and 12
  at `ed75861`). The ignored, run on Claude Code 2.1.319 in temp HOMEs:
  `claude_installed` 1/1 (plain alone with no genome, the ghost over it,
  off, on, off for one launch, a generation switch and back),
  `claude_live` 6/6, `plain --test strict` 1/1; `ghost-in-shell/tests/rollback`
  56/56. `nix build .#default .#ghost` builds; the nix-built and the
  cargo-staged `maid` plugin have the same version.
- 2026-11-10, code review cycle 1: correctness FAIL (high: `--prefix` onto
  an existing dir replaced it whole, so `--prefix ~/.local` would delete
  it), security and comment-hygiene PASS WITH NOTES. Fixed: a prefix that
  is not empty or an earlier install is refused; the reuse check takes
  this checkout's version from what plain's install would build (the
  flake's package with nix), and records the live version after
  installing plain; uninstall removes a plugin and marketplace at its
  profile with no record; a `ghost-in-shell` marketplace elsewhere is
  refused, not taken over; plain's environment checked before anything
  is built; on/off read at the project root; the exec bit is hashed; a
  profile path a hook could not name plainly is refused; the rollback
  script unsets `MY_HARNESS_STATE`; three comments. Left: quoting the
  Justfiles' user-typed arguments (the root Justfile's, older than this
  stream).

- 2026-11-10, code review cycle 2: PASS WITH NOTES on every lens. Fixed
  its lows: on/off and their state both work at the project root (the
  nearest `.git`); an empty marker is refused; a foreign `ghost-in-shell`
  marketplace is refused before anything is built; one doc rewrap. Left,
  as noted: the Justfiles' user-typed arguments unquoted (older than this
  stream); no test with an untracked file under `resources/` under nix.

- 2026-11-10, dev, after both review cycles: `just ci` 651 passed, 13
  ignored; `claude_installed` 1/1, `claude_live` 6/6, `strict` 1/1 on
  2.1.319; `ghost-in-shell/tests/rollback` 57/57 (58/58 after the briefing fixes, `just ci` 653); the grep outside
  `specs/` finds none of the old names; `cargo tree -p plain` has no
  `ghost`, `infusion` or `shell-claude`.

- 2026-11-10, review briefing: its defects fixed. A hook answers again
  when its working dir is gone (the install verbs had made every command
  read it first; a test that fails on the old code); `rollback` installs
  the ghost without nix over a profile-dir plain, and checks a spec-only
  commit builds the same `#ghost` too; plain's validate passes a
  declaration the ghost refuses (a test); two stale comments and the
  modules' pointer to a project.md section that is not there. Left for
  the ringmaster, from its judgement items: a disabled `maid@maid` makes
  every `ghost install` rerun plain; `plain::install::all` and the root
  `install` recipe list plain's steps twice; without nix the record still
  names the flake pin for a pensieve found on PATH; enable entries at
  project and local scope stay after `ghost uninstall`.

- 2026-11-10, closure fixes (the ringmaster's): a disabled `maid@maid` is
  warned of and reused, never reinstalled or re-enabled, in `ghost
  install`, `ghost status` and the genome (`ghost::parts::disabled`, any
  part's plugin turned off); `just install` is one verb, `plain setup`
  (`plain::install::all`, which holds the sequence and the error policy);
  a pensieve that nix did not put in the profile is recorded by its path
  and the version it reports, else "version unknown"; `ghost uninstall`
  names the project and local settings it left. Rebased onto
  `initiative/harness-core` (`755cbcf`).

- 2026-11-10, closure: after the closure fixes and the rebase, `just ci`
  657 passed, 13 ignored; `claude_installed` 1/1 on 2.1.319;
  `ghost-in-shell/tests/rollback` 61/61. Backlog answer: this stream
  closes `ctx-concept-name` (`Ctx` is `Infusion`) and the uninstall half
  of `installed-cache-upkeep` (the item now holds only the pruning and
  the in-place stamps). It files `justfile-argument-quoting` and
  `reuse-check-untracked-resources`. `harness-source-declared`,
  `claude-config-dir-in-records`, `drop-old-part-lists`,
  `shipped-declaration-clash` and `ghost-on-other-shells` stay open.
  project.md, per touched section: Architecture (plain and the ghost's
  install, the adapter in the ghost's plugin, whole-status and ghost
  status, `ConfigDir`, `plain setup`, the disabled plugin, pensieve's
  source), Tech Stack (the crates, `sha2`, the flake with no build logic,
  the ghost's profile), Layout (`plain/` by module, `shell/claude`'s new
  modules, the tests), Testing (`claude_installed` and `strict`, the
  rollback script over both installs), Deployment (`just ghost`), Hard
  constraints (what the ghost's install may touch); Mission and Agent
  Development unchanged.

## Decision Log

- Considered one nix profile holding both packages; chose one per
  package (D5, D8) · why: the ghost must install, roll back and uninstall
  on its own · cost if wrong: two generation histories to keep in step.
- Ruling (ringmaster, planning RETURN, 2026-11-09): the user's layout:
  `resources/`, `plain/`, `ghost-in-shell/` with `shell/claude` as the
  ghost's installer over plain as a library; the flake has no build
  logic; no xtask.
- Ruling (ringmaster, 2026-11-09): reflect leaves codex and kiro; the
  adapter is `[ghost, own]` (superseded: `[personal, own]`, below); validation is a step before staging, in
  `plain` and in `shell/claude` for the ghost's own content.
- `ConfigDir` goes in `plain`, not a crate of its own (D2) · why: its two
  users are `plain` and the one crate that takes `plain` · cost if wrong:
  a third user that must not take `plain` splits it out.
- Considered refusing or warning in plain's uninstall while the ghost is
  installed; chose neither: plain knows nothing of the ghost, and `ghost
  status` says mAId's plugin is gone · cost if wrong: the person learns at
  the next `ghost status`, not at uninstall.
- 2026-11-10, amended for streams 13 and 14 (the RETURN above):
  - The reuse check reads the `maid` plugin through the adapter and
    compares versions; considered also requiring each server the plugin
    declares to be registered · chose the plugin alone · why:
    `personal-memory` is registered only when a store is set, so a check
    on it would rerun plain on every install with no store; the plugin
    carries the revision · cost if wrong: a server removed by hand is put
    back by `just install`, not `just ghost install`.
  - A record not understood counts as a failed check and plain is
    installed · why: plain's install is idempotent, so running it when
    unsure places nothing twice · cost if wrong: one needless reinstall.
  - The ghost's record is an `InstallRecord` (package `ghost`) through
    `plain::record`, replacing the planned `<state>/shells/claude.json`
    · why: one format for every installer, whole-status checks it free,
    and `<state>/shells/` is the adapter's cache · cost if wrong: the
    record gains a field the shared format lacks.
  - The adapter rides the ghost's plugin manifest; `Ctx` gains no
    `adapter` and `Ghost::with_adapter` is dropped · why: stream 14's
    ghost finds every declared unit in the live shell · cost if wrong:
    none known.
  - `Ctx` → `Infusion` · why: it is what every entry point infuses the
    ghost through (the shell, host, config, session sources); widening
    the `Shell` trait's name would clash with that trait · cost if wrong:
    one more rename.
  - The genome no longer says plain is gone; `ghost status` does (R5) ·
    why: the hooks have no checkout and the ghost reads no installer ·
    cost if wrong: the person learns at `ghost status`, not in a session.
- Rulings (user, 2026-11-10, at the Planning Review Gate):
  - The model: plain installs the AI resources into each base harness
    and is the one place that manages what is installed; the ghost sees
    them as bring-ins, reuses all of it, and installs only its own
    (adapter, reflect, reflector, hooks); you launch either. A resource or
    bring-in may point at another repo (pensieve); mAId records that it
    installs it, each package names its pinned external pieces, and a
    unit's `repository` still decides where proposals go (D3a).
  - Each installer's file is an *install record*, read only by its own
    installer; the ghost never reads either. Folder `install-records/`:
    the rename costs a few paths and no migration (D2).
  - Plain status: what plain installed, each still there, registered and
    runnable; the ghost's view of the same items is `ghost status` (D3,
    D9).
  - The adapter is `[personal, own]`; no new field (R2).
  - A revision mismatch warns and never refuses: `ghost install` reruns
    plain from the same checkout; `ghost status` and `ghost rollback`
    warn "plain at X, ghost at Y"; the genome says nothing (D8).
  - Plain uses nothing of the ghost, not the crate: the `my-harness`
    checks move to `ghost-claude validate`, run by `just ghost install`
    and `just ci`; plain copies `time` and `find_on` (D3, D6).
- 2026-11-10, dev: plain stopped using `ghost` and `shell-claude` in step 2,
  not step 5 · why: `shell-claude → plain` (for `ConfigDir`) makes plain's
  use of `shell_claude::installed` a cycle cargo refuses · so the bring-in
  lines and the not-understood lines moved to `shell_claude::status`, the
  seven bring-in conformance cases to `ghost-in-shell/tests/conformance/`,
  and the declaration checks to `shell_claude::validate` then.
- 2026-11-10, dev: `just install` runs `plain profile install`, then
  `plain install` (place, record), not one `plain install` that builds
  the profile too (D5 amended) · why: rollback and the tests place on the
  agents without rebuilding · cost if wrong: one more verb to learn.
- 2026-11-10, dev: `ghost-claude install --prefix` validates the ghost's
  plugin only; `ghost-claude validate` (run by `just ghost install` and
  `just ci`) checks `resources/` too · why: nix builds the ghost's package
  from its plugin alone, so a resource edit gives the same ghost.
- 2026-11-10, dev: plain status calls an MCP item whose registered command
  is gone "not runnable", and fails on it (the user's "runnable").
- Ruling (ringmaster, 2026-11-10, closure): a disabled `maid@maid` warns
  and is reused, never re-enabled; the root `install` recipe calls one
  plain verb; a pensieve found on PATH is recorded as found; building
  `#default` on each ghost install is accepted (cached); leftover project
  and local `enabledPlugins` are accepted, named by uninstall; unquoted
  Justfile arguments are a backlog item (`justfile-argument-quoting`).
