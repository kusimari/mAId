# Feature: live-shell

## Git Setup

- Branch: `feat/install-manifest/work` (the stream's first name, kept)
- Base: `initiative/harness-core`

## Feature Brief

The ghost builds itself from what is installed in the shell, however it got
there: plain, another package's installer, or by hand. `ghost` says what it
needs of a shell (`Installed`); Claude Code's adapter reads it from Claude
Code's own records, cached. Each unit's declaration says whose it is: a
declared unit is its author's own, in the part its `repository` names, and
an undeclared or overlaid one is borrowed. A part's persona is the one its
memory declares. Plain keeps its own install record for its own uninstall
and status, and the ghost never reads it.

Part of initiative: [[harness-core]]

## Handoff

- **Stage:** closed
- **Ready for:** the ringmaster's squash-merge into `initiative/harness-core`
- **Carry forward:** for stream 12: rename `Ctx` to a concept word with
  its other renames (backlog `ctx-concept-name`); its ghost uninstall
  removes `<state>/shells/` (backlog `installed-cache-upkeep`); its
  `ghost.json` replaces the adapter's declaration inside maid's plugin.
- **Deliberately left:** filed as backlog `shipped-declaration-clash`,
  `installed-cache-upkeep`, `drop-old-part-lists`, `ctx-concept-name`;
  `harness-source-declared` and `claude-config-dir-in-records` stay open.

### Crossings
<!-- Append one line per crossing. Never edit or delete a line. -->
- planning → planning · RETURN · fault: design · issue: a separate part
  manifest behind a pointer made two files per package and put the ghost's
  meaning outside the install record · fix: one install manifest per
  package in one folder; tenure derived from each entry's `source.repo`
  against the package's, `since` from a new `installed` date; the ghost's
  meaning only in an opaque `extensions.my-harness` slot copied verbatim
  from one resources file; overlays relative to a recorded `root` · done
  when: one file per package with no pointer; tenure derived from source;
  the ghost meaning only in `extensions.my-harness`, copied verbatim
- planning → dev
- dev → planning · RETURN · fault: design · issue: the ghost read
  installers' records, which are plain's business and miss anything
  installed another way · fix: `ghost::installed::Installed`, implemented
  by `shell/claude` from Claude Code's own records with a cache on their
  modification times; own or borrowed by declaration; the persona from the
  memory's declared `persona`; values bound from the registration through
  each config key's `bind`; plain's record keeps no ghost block; the ghost
  reads no install record or part list · done when: the ghost reads no
  installer record; `Installed` is implemented by `shell/claude`, with the
  cache; equality holds; a hand-installed declared unit joins
- planning → dev
- dev → review
- review → dev · RETURN · fault: implementation · issue: a Claude Code
  record of another shape was read as nothing installed · fix: name each
  record not understood, with Claude Code's version, in the genome,
  `maid-core environment` and whole-status; entries stay lenient · done
  when: a test per record file passes
- dev → review
- review → closure

## Requirements

- R1. Every genome, expression and adaptation answer for maid's own
  install is the same as before, but for the ruled differences (below).
- R2. A unit installed by hand that declares itself joins the ghost in the
  part its repository names; one that declares nothing is borrowed.
- R3. A unit whose declaration names another contract, or none, is
  unreadable and named in the genome and status.
- R4. Plain's install record holds plain facts only; maid's old
  `maid-harness.json` is migrated on the next install with nothing placed
  twice; uninstall removes exactly what the record names.
- R5. `just status --whole` checks every install record (and old part
  list) as before, and reports the bring-ins of each declared part as the
  ghost finds them.
- R6. A hook on every prompt does not re-read Claude Code's records while
  their modification times are unchanged.

## Design

- D1. **`ghost::installed`**: `Installed { units(here), var(name) }`, `Unit`
  (kind, name, manifest, dir, from, enabled, registration, contents),
  `Origin` (marketplace, repo, rev, since), `Registration` (command, args,
  env). `Vec<Unit>` is one.
- D2. **`ghost::parts`**: finds the parts in the enabled units (project.md
  "What the ghost reads", rules 1-6), with what the plugins ship
  (`servers/<name>/server.json`, `overlays/<name>/`). `Part::from_json`
  is the part's JSON form, for tests only.
- D3. **`ghost::binding`**: a config key's `bind` (`arg:`, `command`,
  `env:`, `value:`, `path:`), read from the registration or the shell's
  environment.
- D4. **`ghost::contract`** gains `contract` (required, 1), `name`,
  `persona`, `repository` and a key's `bind`; a plugin's standard
  `repository` stands when its slot names none.
- D5. **`Ghost::from(&dyn Installed, here)`**; `Ghost::of(parts)` the fusion.
  `ghost::Config` keeps only `state` (and `shells()` for the adapter's
  cache).
- D6. **`infusion::Shell`**: what an entry point asks for the ghost
  (`shell.ghost(here)`); every `Installed` is one. Tests hand in their parts
  as JSON.
- D7. **`shell_claude::installed::Claude`**: Claude Code's records, read as
  2.1.319 writes them (Session Log), the derived units cached under
  `<state>/shells/claude/`, keyed on the files' and dirs' modification
  times and each directory marketplace's resolved path.
- D8. **Declarations shipped**: every skill and server `contract: 1`; the
  memory `persona: personal` and its binds; writing-style's two path binds;
  the overlay `contract: 1`; the adapter's `name: claude-adapter`; the
  plugin manifest's `repository`; servers and overlays copied into the
  plugin (`servers/chrome-devtools/` renamed from `servers/browser/` to its
  registered name).
- D9. **Plain**: `install_record.rs` (`InstallRecord`), no `extensions`;
  whole-status reports the ghost's bring-ins through `Claude`.

| File | Concept | Spec section |
|---|---|---|
| `ghost/src/installed.rs` | what the shell has installed: `Installed`, `Unit` | Concept, "What the ghost reads" |
| `ghost/src/contract.rs` | a unit's `Declaration`, read from its standard's slot | Concept, "The bring-in contract" |
| `ghost/src/parts.rs` | parts: a repo's persona and bring-ins, own or borrowed by declaration | Concept, "What forms the ghost", "What the ghost reads" |
| `ghost/src/binding.rs` | bound values, from the registration through `bind` | Concept, "The bring-in contract" (`config`) |
| `ghost/src/ghost.rs` | the `Ghost`: fusion, `Ghost::from(&dyn Installed)` | Concept, "What forms the ghost" (fusion) |
| `ghost/src/environment.rs` | the environment: ready, not ready, undeclared | Concept, "What forms the ghost" (environment) |
| `ghost/src/config.rs` | where the ghost keeps its own state | System design, "The layout" |
| `infusion/src/call.rs` | the call, and the `Shell` the ghost is in | Concept, "The five entry points" |
| `shell/claude/src/installed.rs` | Claude Code's installs, from its own records, cached | On Claude Code; System design decisions |
| `resources/build-tool/src/install_record.rs` | plain's `InstallRecord` | On Claude Code, "Installing and wiring" |
| `resources/build-tool/src/whole.rs` | plain status, every record; the ghost's bring-ins (interim) | On Claude Code, "Installing and wiring" |

## Test Strategy

- Unit tests in each file: installed (Claude's shapes, project scope,
  disabled, the cache, a directory marketplace through a moving link),
  parts (own, borrowed by overlay, by hand, disabled, a plugin's own name,
  part names), binding (each bind, path defaults), contract (contract
  refused, standard repository), ghost (fusion, secrets).
- Conformance: the bring-in cases rewritten onto Claude-shaped homes
  (states, env and state through registrations, malformed alone, optional
  tier, another contract, however installed); install records and both
  old shapes for entries.
- Before/after on a fixture install; `resources/tests/rollback` (migration,
  a hand-installed skill joining); `browser-mcp`; `claude_installed`;
  `claude_live`; `just ci`.

## Implementation Plan

- [x] 1. Before: the fixture install and its outputs, at the base commit.
- [x] 2. `ghost`: installed, contract fields, parts, binding, `Ghost::from`.
- [x] 3. `infusion::Shell`; `shell/claude`'s `Claude` with its cache.
- [x] 4. Declarations and the flake; plain's record without `extensions`.
- [x] 5. Whole-status through the ghost; conformance cases.
- [x] 6. Tests crate on Claude-shaped homes; rollback; live tests.
- [x] 7. Docs.
- [x] 8. Verify: counts, `just ci`, rollback, live tests, before/after.

## Session Log

- 2026-11-09, planning: today the plain installer writes `persona` and
  `bring_ins` into `maid-harness.json`, reading skill declarations to bind
  writing-style's two optional keys; whole-status reads the part lists,
  and the ghost for bring-ins. Baseline at c53fcaa: 605 passed, 12 ignored.
- 2026-11-10, dev (install manifests, held then returned): plain's record,
  the migration and whole-status over both folders were built and pass;
  the ghost's reading of them was dropped on the RETURN above.
- 2026-11-10, Claude Code 2.1.319 in a temp `CLAUDE_CONFIG_DIR`, after a
  real plugin install at user and project scope, `mcp add` at user, local
  and project scope, a git marketplace and a disable at local scope:
  `plugins/installed_plugins.json` is `{"version": 2, "plugins": {"<id>":
  [{"scope", "installPath" (a copy under plugins/cache/<market>/<name>/<version>),
  "version", "installedAt", "lastUpdated", "gitCommitSha" (git sources),
  "projectPath" (project scope)}]}}`; `plugins/known_marketplaces.json` is
  `{"<market>": {"source": {"source": "directory", "path"} | {"source": "git",
  "url"}, "installLocation", "lastUpdated"}}`; `enabledPlugins` is written
  true in the scope's settings (`settings.json`, the project's
  `.claude/settings.json`, `.claude/settings.local.json`), and a plugin no
  settings name is listed disabled; user MCP servers are `.claude.json`'s
  `mcpServers`, local ones its `projects["<dir>"].mcpServers` (with
  `enabledMcpjsonServers` and `disabledMcpjsonServers`), project ones the
  project's `.mcp.json`, each `{"type": "stdio", "command", "args",
  "env"}`. A vendor plugin from a git marketplace may have no
  `.claude-plugin/plugin.json` of its own.

- 2026-11-10, dev: counts at the end, `just ci`: 620 passed, 12 ignored
  (605 and 12 at c53fcaa): build-tool 371, ghost 35, infusion 102,
  shell-claude 27, ghost-in-shell-tests 32, kaimux 53, less the ignored.
  The ignored, run: `claude_live` 6/6 and `claude_installed` 1/1 on Claude
  Code 2.1.319; `resources/tests/rollback` 37/37 (migration, a skill by
  hand joining); `resources/tests/browser-mcp` all pass. Before/after on
  the fixture install (temp HOME, the real claude and codex CLIs, no model;
  before at c53fcaa, after at HEAD): the genome (whole, from a folder,
  sub-session), expression for three prompts, `environment --json`, sleep,
  the adaptation brief and batch are equal but for two differences: the
  part name (`maid-harness` is `mAId`), and one MCP server that this
  machine's `claude` registers in every fresh config, now a borrowed
  `local` unit (a personas entry `no persona (local, nothing declared)`, a
  turn bring-in line, one more in the environment count and its
  undeclared line). "borrowed from" is unchanged: a borrowed unit's lender
  is its marketplace.
- 2026-11-10, code review: cycle 1 FAIL (security, high: literal MCP env
  values cached in a default-mode file); fixed with the cycle's other
  findings. Cycle 2 PASS WITH NOTES; its four medium notes fixed (a repo's
  committed settings cannot approve its own `.mcp.json` servers, a literal
  env value binds nothing, the cache has a format and its dir is tightened,
  the memory spawn's trust comment). Leaves below.

- 2026-11-10, after review: a record of Claude Code's that is not of the
  shape read is named (`<file>: shape not understood (Claude Code
  <version>)`), one test per record file; a proposal names the plugin's
  version and its install date, per the ruling. Rebased onto `d4771f1`.
  Gates again: `just ci` 626 passed, 12 ignored; `claude_live` 6/6,
  `claude_installed` 1/1 on 2.1.319; rollback 37/37; browser-mcp passes;
  the before/after unchanged.
- 2026-11-10, closure: backlog answer: this stream closes
  `record-syncs-servers-by-declaration` (the browser sync is gone; plain
  names each entry's files by name, and the ghost finds a server's
  declaration in the plugin that ships it) and
  `memory-store-record-follows-registration` (the store is bound from the
  registration itself, so the genome and status follow what Claude Code
  runs). It files `ctx-concept-name`, `installed-cache-upkeep`,
  `shipped-declaration-clash` and `drop-old-part-lists`.
  `harness-source-declared` stays open: adaptation still finds the
  harness's source by the name `claude-adapter`, now a declared name.
  project.md, per touched section: Architecture ("My harness": the
  install record, the old part lists, what the ghost reads, the contract
  fields, the reader rules, the conformance set), Layout (the new files),
  Testing (rollback and browser-mcp), Deployment (the install record),
  "For any persona part" rewritten; Tech Stack, Mission, Hard constraints
  and Agent Development unchanged (no dependency, no invariant, no kdevkit
  key).

## Decision Log

- Ruling (user, planning RETURN, 2026-11-09): one install manifest per
  package, the ghost's meaning in an opaque slot. Superseded by the next.
- Ruling (user, 2026-11-11, "the ghost builds itself from what is in the
  environment"): the ghost reads what the shell has installed through its
  adapter, never an installer's record; stream rescoped as `live-shell`.
- Rulings (ringmaster, review): no git stamp, the plugin's content-hash
  `version` stands as the revision and a proposal carries it with the
  install date; the rename's history is not migrated (no ghost on `main`
  yet); a record of another shape is named; a self-declared `repository`
  is accepted with adaptation at the user's scope; MCP args in the cache
  at `0600` accepted, env values kept out; `contract` required; `Ctx`
  renamed in stream 12.
- A plugin ships the declarations of the MCP servers it registers and of
  the units it borrows, at `servers/<name>/` and `overlays/<name>/` · why:
  Claude Code's MCP registration holds a command, not a manifest, and an
  overlay must be found by the shell's view alone · cost if wrong: a slot
  field naming them instead.
- A part's name is its repository's last segment · why: the repository is
  what the units declare and where proposals go · cost if wrong: a
  declared part name later.
- Only parts that declare are reported in whole-status · why: the work
  package's undeclared units would otherwise each add an "undeclared" line
  to a status that reported none before · cost if wrong: one filter.
- Declared parts come first in the personas line · why: the machine's
  `claude` registers its own MCP server in every fresh config, and the
  genome's first persona should be one that declares.
- Infusion's tests hand in parts as JSON through a test `Shell`, and the
  tests crate installs units into Claude-shaped homes · why: keeps the
  hundred infusion tests about infusion, while the cross-crate tests run
  the real reader.
- Left from review, each with why: adaptation builds the ghost at the
  user's scope (`here` none) · why: then nothing a project ships can reach
  the `serve` command it runs (project.md says so). Two plugins shipping a
  declaration for one name: the first in plugin-id order wins, no clash ·
  why: no case has two today; a backlog item if one appears. The cache
  stamps a directory marketplace's root, not each skill dir inside it ·
  why: a profile changes by moving its link, which is stamped; a working
  tree edited in place is a developer's case. A part named by a look-alike
  repository joins that part · why: the units' sources stay accurate, so a
  proposal still goes to the declaring repo; keying parts on the full URL
  is a later change. Cache files are not pruned or removed on uninstall,
  and a registration's args are kept in them · why: they hold what
  `.claude.json` holds, at the same user's reach (`0600`); stream 12's
  ghost uninstall removes `<state>/shells`.
