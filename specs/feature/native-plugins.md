# Feature: native-plugins

## Git Setup

- Branch: feat/native-plugins
- Base: initiative/installable
- Worktree: `../mAId-worktrees/native-plugins`

## Feature Brief

Part of initiative: [[installable]] (stream 2)

claude and codex get mAId through their own plugin system instead of
links into their skills dirs. mAId shows up in each one's plugin list, can
be disabled there, and each `just install` updates it. kiro and agy keep
stream 1's links.

## The picture

```
mAId profile (nix, one generation per install)
  share/maid/marketplace/              <- one marketplace, both agents' manifests
    .claude-plugin/marketplace.json
    .agents/plugins/marketplace.json
    plugins/maid/                      <- the plugin: version = content hash
      .claude-plugin/plugin.json
      .codex-plugin/plugin.json
      skills/<name>/SKILL.md
  share/maid/skills -> marketplace/plugins/maid/skills

~/.local/state/maid/marketplace/       <- real dir, mAId-owned; entries link
  .claude-plugin, .agents, plugins        into the profile path (not the store)

claude  marketplace "maid" = that dir; plugin maid@maid; reads skills live from it
codex   marketplace "maid" = that dir; plugin maid@maid; copies a version into its cache
kiro, agy   unchanged: links at share/maid/skills
```

## Handoff

- **Stage:** closed
- **Carry forward (stream 3):** after `nix profile rollback`, re-run
  the plugin install (`just resources::install-skills`): codex serves
  its cached copy of the plugin, so a rollback alone does not reach it
  (claude reads the marketplace live and sees it at once, but its
  listed version stays stale until the install runs).

### Crossings

<!-- Append one line per crossing; never edit or delete a line. -->

- planning → dev
- dev → review
- review → closed

## Requirements

- `just install` installs mAId into claude and codex as a plugin named
  `maid`: `claude plugin list` and `codex plugin list` show `maid@maid`,
  enabled, at the version this install built.
- A later `just install` (any checkout or worktree) with changed skills
  updates the plugin: the version changes and a new session sees the new
  text. With nothing changed, the version stays the same.
- A plugin the user disabled stays disabled across `just install`, and
  install says so.
- The skill links stream 1 put in claude's and codex's skills dirs are
  gone after install, including codex links left by older checkouts for
  skills no longer shipped. Links mAId did not write are kept.
- Skills that hand work to each other by name (kdevkit's judge
  `kyodakit`) still find each other. In claude and codex a skill is named
  `maid:<name>`; asking for the bare name still finds it.
- kiro and agy are unchanged, and so is the browser MCP server: it is
  registered as before, outside the plugin.
- `just status` shows, per agent, the plugin's version, enabled or
  disabled, and whether it is the current install's version.
- `just uninstall` removes the plugin and the marketplace from claude and
  codex, as well as everything stream 1 removes. User data is kept.
- No agent config mAId writes names a checkout or a nix store path.

## Test Strategy

### Unit (`just test`)

- The plugin version and manifest are read from a profile tree; a missing
  manifest is an error naming `just install`.
- Plugin state is read from each CLI's JSON listing: missing, current,
  stale, disabled; unknown output is an error, not "missing".
- What install, update and uninstall would run, for each state: a disabled
  codex plugin is not re-added; a stale claude one is updated; a missing
  one is installed after its marketplace is added.
- Old links: claude's skills link and codex's per-skill links are removed
  when they point into any mAId skills tree (profile or checkout, shipped
  skill or not); a foreign link and a user's own codex skill survive.
- `uninstall` without `--force` removes a kiro/agy link another mAId
  install left (backlog `install-link-takeover-gaps`, gap 1).
- Every agent is deployed by exactly one mechanism: plugin (claude,
  codex) or link (kiro, agy).

### Integration (real CLIs, temp HOME and CODEX_HOME, no model calls)

`cargo test -p build-tool --test plugins -- --ignored` against two synthetic profile trees,
skipped where a CLI is missing:

- install: both list `maid@maid` enabled at profile A's version; the
  recorded marketplace path is the mAId marketplace dir, not a store path.
- switch to profile B, install: both at B's version; codex's cache holds
  B's text.
- disable in both, install again: both still disabled.
- uninstall: neither lists the plugin or the marketplace; the marketplace
  dir is gone.

### isolated-verify restore (temp homes, no model calls)

- With a "real" install in a temp HOME/CODEX_HOME, run `isolated-verify`
  with an unmatched selector (no agent calls): during the run the plugins
  point at the throwaway profile; after it, both report the real
  profile's version and marketplace, the old enabled state, and the
  throwaway marketplace dir is gone. A forced failed restore exits 4.

### Acceptance (by hand, this machine)

- `nix build` twice, with and without a skill edit: the plugin version
  changes only with the edit.
- `just install` from this worktree: both plugin lists show `maid@maid`;
  `~/.claude/skills` and mAId's `~/.codex/skills/*` links are gone; kiro's
  link remains; `just status` reports all of it.
- Paid, one run: `just resources::verify-skills-isolated smoke
  kdevkit-judgement-live --agent claude,codex`. kdevkit has to be found
  unaided inside the plugin and then load `kyodakit` by name. It is the
  only evidence for the name-handoff requirement, and within the
  initiative's single-fixture allowance. Run only after the restore test
  above passes; afterwards `just status` shows the real version.

## Design

- **One marketplace for both agents, built into the profile.** claude
  reads `.claude-plugin/marketplace.json`, codex `.agents/plugins/
  marketplace.json`; one plugin dir carries both `plugin.json`s and the
  skills, so there is one copy. `share/maid/skills` becomes a link to the
  plugin's skills, so kiro, agy and validation read the same tree.
- **The registered path is a real mAId-owned dir, not the profile.**
  Checked: codex resolves symlinks when it records a marketplace, so
  registering the profile would pin one generation's store path, which
  the 30-day wipe can collect. claude keeps the path as given. So install
  makes `<profile's parent>/marketplace/` with three links into the
  profile path, and registers that with both. It never changes between
  installs, and it follows `MAID_PROFILE`, so `isolated-verify`'s
  throwaway profile gets its own.
- **Version = content hash**, computed in the flake build over the
  plugin's files, as `1.0.0-<12 hex>`. Both agents cache by version and
  compare for inequality, not order (checked: `claude plugin update` goes
  from a newer to an older version), so a content hash gives "changes
  exactly when content does", and stream 3's rollback works without a
  counter.
- **A second `Deploy` impl, `Plugins`, as `deploy.rs` always planned.**
  It shells out to `claude plugin …` / `codex plugin …`. A `Deployment`
  routes each agent to `Plugins` or `Symlinks`; `stages.rs` keeps
  speaking `Deploy`. REGISTRY keeps only the kiro and agy rows; a
  `PLUGIN_AGENTS` list names claude and codex, with the old link
  location each must reap. Splitting `Plugins` into pure "read state"
  and "plan commands" functions plus a thin runner is what makes the
  first two unit-testable without the real CLIs.
- **Per-agent CLI facts it encodes** (all checked in a temp HOME):
  - claude: `marketplace add` is idempotent and repoints a changed path;
    `install` once, then `update` each time; `update` keeps a disabled
    plugin disabled; skills are read live from the marketplace path.
  - codex: `marketplace add` refuses a changed path, so remove, then add;
    `plugin add` installs or updates, copying into its cache, but
    re-enables a disabled plugin, so a disabled codex plugin is not
    re-added (install reports it as left disabled and not updated);
    `plugin remove` before `marketplace remove`, or the config entry is
    orphaned.
  - Both: `list --json` gives id, version and enabled.
- **Old links are reaped by the same rule install already uses**
  (`is_maids`: points into a mAId skills tree), at claude's
  `.claude/skills` and at every link under codex's `.codex/skills`.
  Scanning by rule instead of by `FanOut` expansion is what also reaps a
  skill no longer shipped (backlog gap 2). `remove` uses the same rule
  for kiro and agy (gap 1). That closes `install-link-takeover-gaps`.
- **Install repairs a codex marketplace whose dir is gone.** codex
  refuses every plugin command (`plugin list`, `marketplace list`)
  while its recorded `maid` marketplace dir is missing, which is what an
  isolated run that dies before restoring leaves. Install and uninstall
  then remove that record first. `status` also reports each agent's
  marketplace (ok, WRONG, missing), and `is_deployed` requires it to be
  this install's dir.
- **A CLI not on PATH is a skip**, as `manage` does for the browser
  server: nothing to install into.
- **`State` gains plugin outcomes** (`Stale { found, want }`, `Disabled`,
  `NoCli`), and `outcome()` / `describe()` gain their lines. A disabled
  plugin does not fail install.
- **`isolated-verify`** finds the profile to restore from kiro's link,
  not claude's (which goes away). Its install and restore already go
  through `build-tool install`, which now repoints the plugins too. The
  restore must leave both plugins at the real profile's version, enabled
  as before, with the marketplace back on the real dir; it fails (exit 4)
  otherwise. This is tested in temp homes before any paid run.
- **Project docs that change:** Architecture (`deploy.rs` has two impls;
  how claude and codex get skills), Tech Stack (verbs unchanged),
  Hard constraints (`~/.claude/skills` and `~/.codex/skills` are no
  longer mAId-written; the marketplace dir and plugin registrations are
  owned state), Testing (the ignored real-CLI tests).

## Implementation Plan

- [x] Flake: the marketplace and plugin in the profile, version from a
      content hash, `share/maid/skills` linked to the plugin's skills.
- [x] build-tool: `Plugins` with its state reader and command planner;
      `Deployment` routing; old-link reaping and the `remove` rule; State
      and outcome lines; unit tests.
- [x] build-tool: the ignored real-CLI integration tests.
- [x] `isolated-verify`: owner from kiro's link; restore puts both
      plugins back on the real profile's version and checks it; restore
      tested in temp homes (no model calls) before the paid run.
- [x] Docs: README, project.md.
- [x] Acceptance on this machine, then the one paid smoke run.

- *Risk note:* switching this machine's claude from links to the plugin
  renames every mAId skill to `maid:<name>` mid-initiative, including in
  the ringmaster's sessions. Bare names still resolve (checked), but a
  slash command typed as `/kdevkit` becomes `/maid:kdevkit`.
- *Risk note:* claude may keep old cache versions; they are small, and
  claude reads the marketplace path anyway.

## Session Log

- 2026-10-08 · Closure. Rebased onto `initiative/installable` (9ccc0c7),
  `just ci` green. No unticked plan items. Handoff resolved: the rollback
  note stays as stream 3's carry-forward; the browser MCP outside the
  plugin is a ruling, not wanted work; the isolated-verify edge (a
  marketplace with no plugin before the run) and the custom
  `MAID_PROFILE` root name are not wanted, both hand-made states that
  are reported. project.md: Layout (serde_json, `tests/no_cli.rs`),
  Deployment (plugins), Architecture (ownership by name). Backlog:
  `install-link-takeover-gaps` closed (uninstall takes over another mAId
  install's link; codex links to skills no longer shipped are reaped by
  rule).

- 2026-10-08 · Briefing returned three defects, fixed: the real-CLI test
  also skips when `CLAUDE_CONFIG_DIR` is set (it would reach past the
  temp HOME); the install and uninstall lines for plugin states have a
  unit test (`plugin_outcome_lines_say_what_happened`); stale comments
  about codex as a link agent, and project.md's "the registry symlinks
  the skills directory", corrected. `FanOut` stays in `Kind`: no
  REGISTRY row uses it, `PLUGIN_AGENTS` uses it for codex's old links.

- 2026-10-08 · Code review, correctness lens (no named-risk paths), two
  cycles, both PASS WITH NOTES. Taken from cycle 1: old links are reaped
  only after the plugin is in, and never when the CLI is missing (new
  test `tests/no_cli.rs`, fails with the reap moved first); status
  reports an agent whose CLI refuses as unreadable instead of failing;
  restore uninstalls a plugin that was missing before; the codex repair
  fires only on codex's missing-marketplace refusal; the post-install
  check requires the wanted version and the mAId marketplace; a listing
  without `enabled` is an error; the real-CLI test covers a disabled
  codex plugin through a marketplace round trip (it stays disabled).
  Cycle 2: `isolated-verify` refuses to start while old mAId links are
  present (status lists them), and compares only what it restores;
  unreadable messages are one line. Restore cases re-run in a temp HOME:
  disabled claude kept, codex missing kept missing, old link refused
  (exit 1), forced failure exit 4.

- 2026-10-08 · Dev.
  - Flake: the plugin version is `1.0.0-5b903597cad1` at HEAD; one
    appended line in a skill gave `1.0.0-fe36545eb7ff`, reverting gave the
    first back. `claude plugin validate` passes on the built marketplace.
  - `just ci` green (187 build-tool unit tests + kaimux + 3 integration).
    The new tests fail with their fix removed: uninstall's mAId-link rule,
    leaving a disabled codex plugin alone, the old-link match.
  - Real-CLI test (`--test plugins -- --ignored`) passes in about 10s; it
    failed with codex's disabled plugin re-added (codex came back
    enabled), and with the marketplace repair removed.
  - Restore, temp HOME, no model calls: a dirty-tree install
    (`1.0.0-b89f295baf7d`) with claude's plugin disabled; `isolated-verify
    smoke no-such-fixture` moved claude to the clone's `1.0.0-5b90...`
    during the run (its cache shows both) and restored `b89f`, still
    disabled, codex `b89f`, both marketplaces on the real dir; exit 2.
    With the real root's `plugins` entry swapped for a real dir it exits
    4 and prints before/now. That run left codex on the deleted throwaway
    dir, where codex refuses every plugin command; `just install` could
    not recover. Fixed (install removes that record first), then
    `just install` recovered it. `just uninstall` left the allowlist and
    learned-rules files.
  - This machine: `just install` (generation 8) removed `~/.claude/skills`
    and the six `~/.codex/skills/*` links, kept `.system`; both plugin
    lists show `maid@maid 1.0.0-5b903597cad1`. Status showed codex's
    marketplace WRONG: this machine's HOME is a link to another path and
    codex records the resolved one. Fixed by comparing resolved paths; a
    second install then left codex's config untouched. No agent config
    mAId wrote names a checkout or a store path (grep).
  - `isolated-verify smoke no-such-fixture` on this machine: restored,
    exit 2.
  - Paid, one run: `verify-skills-isolated smoke kdevkit-judgement-live
    --agent claude,codex`: integration 2/2 pass (claude, codex), agents
    restored, status current after. The pass is the fixture's artefact
    check (every print fixed, which kdevkit alone tends not to do); the
    run keeps no trace of which skill files were read.

- 2026-10-08 · Planning. Spiked both CLIs in a temp HOME and CODEX_HOME
  with a two-skill marketplace (`alpha`, which says to load `beta`).
  - One marketplace dir with both agents' manifests and one plugin dir
    with both `plugin.json`s: both CLIs install it (`claude plugin
    validate` passes, warnings only).
  - claude: `marketplace list --json` keeps the symlinked path as given.
    A session lists `maid:alpha`, `maid:beta`; `Skill` with bare `beta`
    resolved to `maid:beta`, loaded from the marketplace path. After
    switching the marketplace symlink to a v2 dir without `update`, a
    session read v2 text while `plugin list` still showed v1: claude
    reads a directory plugin live from the marketplace. `update` moved
    v1→v2 and back v2→v1. Disable, then `update`: still disabled.
    `marketplace add` of the same name at another path repoints it.
  - codex: `marketplace add` of a symlink recorded the resolved target.
    A real dir whose entries link to the symlink path is recorded as
    given. `plugin add` after a switch installed v2 into the cache and
    dropped v1. A session lists `maid:alpha`, `maid:beta` from the cache
    and followed alpha into beta. `plugin add` re-enabled a plugin set
    `enabled = false`. `marketplace add` at another path is refused;
    `marketplace remove` alone leaves `[plugins."maid@maid"]` behind.
  - Fixtures carry no skill paths; only `isolated-verify` reads
    `~/.claude/skills` (to find the profile to restore).
  - Not checked: the codex TUI's own disable control (codex's switch is
    `enabled` under `[plugins."maid@maid"]`, which `plugin list` reports).

## Decision Log


- A real marketplace dir beside the profile, registered with both agents
  · codex resolves a symlinked root to a store path; considered
  registering the profile path for claude only, rejected for one shape
  across both.
- Version from a content hash, not a counter or the generation number ·
  both CLIs only compare for inequality, and a hash is stable for an
  unchanged install and needs no state.
- mAId's marketplace and plugin are recognised by name (`maid`,
  `maid@maid`), as `is_maids` recognises links by path shape · a fork
  using the same names counts as mAId's.
- Install repairs a codex marketplace whose dir is gone by removing the
  record · found when a forced failed restore left codex unusable; the
  CLI offers no other way back.
- `Plugins` in Rust behind `Deploy`, not shell beside `manage` · smoke's
  `is_deployed` and status need it, and `deploy.rs` names this as the
  intended second impl.
- Ruling (ringmaster): the browser MCP server stays out of the plugin ·
  its registration is Chrome-gated and kiro needs the current route; a
  bundled server would start and fail on machines without Chrome · cost:
  disabling the plugin leaves the browser server registered.
- Ruling (ringmaster): the `maid:<name>` rename in claude and codex is
  accepted · bare names still resolve.
