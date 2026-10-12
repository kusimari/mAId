# Project: mAId

## Mission

<!-- What this project exists to do, and who it serves. One
     paragraph. Change only when the goal itself changes. -->

Tool-agnostic source of truth for my agentic resources — mostly
skills — compiled into whatever AI tool I'm using (Claude Code,
Kiro, Codex, future tools). The repo is the checked-in source;
`just install` builds it into a nix profile in user space and links
each tool at that profile, so what is installed never depends on the
checkout. claude and codex install the skills as a plugin through
their own plugin commands; kiro (and agy) discover them natively at
their own skills path, by symlink. Either way there is no global
instruction preamble; the one non-skill resource
(browser-control MCP) registers as a runnable server via Just verbs
(see Architecture). One canonical set of resources, many consumer
surfaces. Apps that ship binaries (today: future `kaimux/`, the
agent-orch successor) live as
sibling workspace members with their own native cargo verbs.

## Architecture

<!-- The logical shape: components, responsibilities, how they
     talk. Diagrams optional; words mandatory. Not a file-layout
     listing — see Layout for that. -->

Two halves at the top level:

- **`resources/`** — three layers, working together:
  1. **Content** (`resources/content/skills/<name>/SKILL.md`) —
     the skill definitions the AI tools load. Skills are the only
     deployed artefact; each tool auto-discovers them at its own
     skills path.

     A skill may ship **deferred modules** beside its `SKILL.md` —
     sibling files, or files under subdirectories — that the
     always-on `SKILL.md` inline-Reads when a stated trigger
     fires. `SKILL.md` stays the only validated and discovered
     file; modules ride along because a skill is installed as a whole
     *directory* (copied into the plugin, or linked), so adding one
     needs no build-tool change.
     This is how an always-on file stays lean as a workflow grows:
     new phase-specific rules land in a module, not in the file
     loaded every session. `kdevkit` is the worked example
     (`phases/`, `tiers/`, plus `setup.md` / `interviews.md`).

     Skills are mostly independent, but one may **dispatch another
     by role** rather than by name: the caller declares the role it
     needs, the fillers advertise that they fill it, and install (or
     a project's own context file) binds them. `kdevkit` reaching for
     an "independent review-briefing tool" — which `kreviewkit`
     fills — is the first instance; its "developer-judgement" role, which
`kyodakit` fills, is the second. Two rules keep that from becoming
     coupling: **the caller names a role, or at most a default filler
     that project config overrides** (kdevkit's judge is `kyodakit`
     unless `project.md` sets `judgement:`), so a different filler can
     be swapped in without editing the caller; and **the filler owns its own invocation contract** (what it
     needs, how it must be run), which the caller consults rather
     than defines. A caller that dispatches another skill also owes
     it a **safety floor** — limits the dispatched skill cannot widen
     by asking, since a role it fills may be resolved from content it
     doesn't control.
  2. **Tooling** (`resources/build-tool/`) — Rust crate
     organised as the pipeline it performs: validate content,
     check each skill in isolation, install into each agent (plugin
     or `$HOME` symlinks), then smoke-test what is deployed. One file per
     category — `main.rs` (the clap shim), `shared.rs` (agents,
     registry, roots), `harness.rs` (driving an agent and scoring
     the reply), `deploy.rs` (how a skill reaches an agent), `stages.rs`
     (the pipeline: content → check → install → smoke). Dependencies
     run one direction: stages → {harness, deploy} → shared.

     **`deploy.rs` hands over to the agent where it can.** The pipeline
     declares *what* it wants deployed; `deploy.rs` alone knows *how*,
     behind a `Deploy` trait (`install` / `uninstall` / `status` /
     `is_deployed`, each returning a `Report`, never a path, and
     `placed`, what the part list records). `Plugins`
     drives claude's and codex's own plugin CLIs; `Symlinks` (the
     $HOME-layout knowledge, `Link`/`FanOut` registry expansion,
     `--force` semantics) covers the agents with no such command (kiro,
     agy). `Deployment` routes each agent to one of them, then runs
     `BringIns` (the vendor marketplaces, below), so nothing in
     `stages.rs` knows which. `check` needs no deployment at all — it receives a
     `NoDeploy` target and runs with no `$HOME`, which is what makes
     "check needs no install" a structural fact rather than a
     convention the prompt merely honours. The only bash left in the
     whole pipeline is what is genuinely shell — a fixture's
     `--- setup ---` / `--- assert ---` blocks, executed by the runner.
  3. **Verbs** (Justfile recipes that use the tooling) —
     `just resources::install-skills`, `…::uninstall-skills`,
     `…::status-skills`, `…::check-skills`, `…::smoke-skills`,
     `…::verify-skills` (both stages; single-fixture:
     `just resources::verify-skills-one <name>`).
     Every verb follows the `<action>-<resource-kind>` pattern and
     takes an optional coding-agent selector (`claude|kiro|codex`;
     omit for all three). These are how a human or another tool
     consumes the
     tooling.
- **`kaimux/`** — tmux-pane orchestrator for coding-agent
  sessions. Single-binary Rust crate (workspace member).
  Wraps `claude` / `kiro-cli` calls so each running agent
  registers itself as a tracked tmux pane; a top-level
  dashboard pane shows the inventory, status, and a
  one-key jump to any of them. State lives in
  `$XDG_STATE_HOME/kaimux/sessions.json`. No symlinking;
  built by `just kaimux::build` to `dist/kaimux` and
  invoked directly (typically via a tmux keybind that
  `kaimux setup` installs into the user's tmux config).

Both halves are members of one cargo workspace at the
root, so `cargo build --workspace` covers everything.

**Registry** lives inline at the top of
`resources/build-tool/src/shared.rs`: `REGISTRY`, a slice of
`(home_subpath, source_subpath, kind, agent)` tuples for the linked
agents, and `PLUGIN_AGENTS` for the plugin ones (with where each
one's pre-plugin links lived, which install reaps), and `BRING_INS` for
the vendor marketplaces mAId registers with the plugin ones. The
authoritative manifest for what gets installed where. The
`agent` tag is what lets install/uninstall/status be scoped
to one coding agent (`--agent`, surfaced as the Just verbs'
selector) or, by default, cover them all. Tool adaptation
lives here: adding a new coding-agent tool = adding its
expected `$HOME` paths as registry entries, not rewriting
content. Content stays tool-agnostic; the registry translates
it into each tool's expected layout.

**Install = a profile, then links.** `just install` builds the
checkout's flake `default` package into the mAId profile
(`${XDG_STATE_HOME:-~/.local/state}/maid/profile`, set in
`resources/Justfile`) as a new generation: the skills under
`share/maid/skills`, and the runtimes resources need as nix closures
under `bin/`, and the plugin marketplace under `share/maid/marketplace`
(the skills live in its plugin; `share/maid/skills` links there). Every
link and MCP registration names the *profile* path, never a store path
or a checkout, so the checkout can be deleted, and an install from any
other checkout or worktree takes over by adding a generation without
touching agent config.

**Plugins (claude, codex).** Both agents are pointed at
`<profile's dir>/marketplace`, a real directory of links into the
profile path: codex records a marketplace by its resolved path, so
registering the profile itself would pin one generation's store path.
The plugin's version is a hash of its content, computed in the flake
build; both CLIs cache by version and only compare for change, so an
install updates exactly when content changed (claude reads the
marketplace live; codex reads its cached copy, so install must run
`codex plugin add`). A disabled plugin stays disabled: `codex plugin
add` would re-enable it, so install leaves a disabled codex plugin
alone. Skills are `maid:<name>` there; a bare name still resolves.
mAId's marketplace and plugin are recognised by name (`maid`,
`maid@maid`), as its links are by path shape. Install
takes over a link only when it points at a mAId skills tree (a
profile's, or a checkout's from before the profile); any other
symlink at a managed path is kept and reported, since mAId never
touches agent config it did not write. Install and uninstall run
their other steps past such a location, then exit non-zero. Nix GC roots keep each
generation alive; an install that adds a generation wipes those older
than 30 days, and nix keeps the newest one older than that, so the
install before the live one survives (an unchanged install adds none and
skips the wipe, which would otherwise keep only the live one). **Rollback** is `nix profile rollback` plus the plugin install
(codex serves its cached copy until `codex plugin add` runs). It refuses
a generation without the skills, the plugin manifest or the browser
launcher, before switching; it does not switch back on an install
failure, since install also exits non-zero for a link it left alone.
nix rolls back to the next lower generation number, so install drops
the generations above the live one (the ones a rollback returned from),
after its build succeeds; otherwise a rollback after rollback-then-install would land on
them rather than on the install before.

**Skill reads (claude).** Claude loads a plugin of a `directory`
marketplace live from the marketplace path, so it reports each mAId skill's
base dir as `<marketplace>/plugins/maid/skills/<skill>`, and a Read of a
module there asks for permission in manual mode. A plugin cannot grant
itself reads (a plugin's settings take no permissions; a skill's
`allowed-tools` lasts one turn), and a `Read` allow rule on that path does
not match, since Claude also checks the file it resolves to, in the store.
So install adds `<marketplace>/plugins/maid` (the profile-side path, never a
store path) to `permissions.additionalDirectories` in
`~/.claude/settings.json`: Claude then reads anything under it without
asking. This widens Claude's working directories only to a read-only tree,
so the edit half of a working directory grants nothing there. It is one
element of a list the user and other parts also write: mAId owns the element
its part list records, and adopts an identical one it finds (rule 2),
removing either on uninstall; any other element is left alone, and a recorded value not of
that shape is not mAId's. claude not on PATH, or no plugin in the profile,
writes nothing; a value that is not a list is left and recorded skipped.

**Bring-ins (claude, codex).** A vendor's plugin marketplace mAId
brings in is a `BRING_INS` row in the registry: the name both CLIs list
it under, its public git source, a commit, the plugins to install from
it, and the commands its skills run. Install registers it and installs
the plugins through each CLI, after mAId's own plugin, so a failure there
(no network) leaves mAId's skills in and exits non-zero. Nothing of the
vendor's is copied into the profile. Neither CLI records who added a
marketplace, so an item is mAId's only when mAId's part list names it
as placed or the run added it; a marketplace or plugin the user had
already is kept, recorded skipped "not mAId's", and never removed. A run
that cannot list them (no CLI, or a listing that cannot be read) keeps
what the part list names as mAId's placed.
Uninstall removes mAId's plugins, then mAId's marketplace unless a plugin
not mAId's is installed from it (removing a marketplace uninstalls every
plugin from it). codex is pinned to the commit (`--ref`); claude's CLI
takes only a branch or tag, so claude holds the commit current when it
first registered it, and third-party marketplaces do not auto-update.
The row's commit applies to fresh installs only: a marketplace already
registered is not re-pinned when it changes. A part list that cannot be
read stops install and uninstall (ownership is unknown); `status` reports
it and carries on. kiro and agy have no plugin marketplace, so the part list
records them skipped. The first row is the vendor's document skills
(`xlsx`, `pptx`, `docx`, `pdf`).

Skills reach kiro (and agy) through the registry as symlinks at their
own skills path, verified to load with no extra preamble. kiro reads
`~/.kiro/skills/*/SKILL.md`, a dir that also holds the user's own
skills, so it gets one link per skill (`FanOut`); kiro's steering dir
is context, not skills, and the link an older install left there is
reaped (`LEGACY_LINKS`). In a FanOut dir mAId owns only links into a
profile's skills, since it never put a checkout link there: those it
repoints, and removes when the profile no longer ships the skill.
Anything else there is the user's, kept even with `--force`, and
reported as kept without failing the run, so a user's skill named
like a mAId one wins in kiro. mAId installs no global instruction file:
`AGENTS.md` is a repo-root convention (per-project, alongside
README.md), not a global per-tool preamble, and "load the project's
AGENTS.md / project.md" is kdevkit's work-time instruction rather
than something deployed here.

**Browser-control MCP** is the one resource that isn't a skill — the
first *runnable* one (skills are markdown symlinks; this is a live
server). It lets the agent drive the user's already-running Chrome
over the DevTools Protocol, so existing logins are reused with no
re-auth. Its shape differs from skills in two ways:

- **Registration vs. runtime split.** A skill lives entirely
  in the harness config as a symlink. An MCP server is an
  out-of-process service the harness *calls*, so only its
  *registration* goes in each harness's global config (`claude mcp
  add` / `codex mcp add`, and for kiro an edit of
  `~/.kiro/settings/mcp.json`); its *runtime* (the pinned
  chrome-devtools-mcp and its Node) is a closure in the mAId
  profile. This is why the registry
  above can't express it — registration is a runnable command,
  not a symlink — so it lives in `resources::browser-mcp-*`
  Just verbs (shell over each harness's MCP CLI), keeping the
  Rust build-tool pure-symlink.
- **Kiro's registration is a direct edit.** `kiro-cli mcp add`
  needs a login and writes every other server back through its own
  struct, dropping keys it does not know, so `manage` sets mAId's
  entry in kiro's global config with `jq` and leaves every other key
  as it is. The entry is mAId's only while its `command` is the
  profile's `bin/maid-browser-mcp`: install and uninstall leave any
  other `chrome-devtools` alone (install records it as skipped, "not
  mAId's"). The write is atomic (a temp file beside the resolved
  target, with its mode, renamed over it), and uninstall needs no
  `kiro-cli`. Kiro agents get it by pulling the global config in; mAId
  writes into no kiro agent file.
- **Browser-enforced allowlist.** The agent may act only on
  sites in a user-owned allowlist file
  (`${XDG_CONFIG_HOME:-$HOME/.config}/maid/browser-allowlist`).
  Enforcement is at the browser (a launch flag), not in skill
  prose — a prompt-injected agent still can't leave the list.
  Deny-by-default: an empty/absent list refuses to start. A
  thin launcher (`resources/browser/launch`, installed as the
  profile's `bin/maid-browser-mcp`) re-reads the file on each
  (re)connect, so edits apply next session without restarting
  Chrome.

### My harness

The terms mAId uses for what it installs:

- **Agent**: the whole entity a session runs as: base harness, my
  harness and my memory.
- **Base harness**: a model plus a vendor coding agent (`claude`,
  `codex`, `kiro-cli`, `agy`), run as the vendor ships it.
- **My harness**: what parts add on top of the base harnesses
  (skills, MCP servers, sub-agents, later more), through each base
  harness's own extension points. It has no driver loop of its own.
- **Part**: what one repo installs into the base harnesses. mAId's
  part is `maid-harness`. Other parts may sit beside it; my harness is
  the union.

The rules every part follows, so that parts add up and never fight:

1. No wrapper. The base harness binaries run as the vendor ships them:
   no wrapper scripts, aliases or `PATH` shims.
2. A part owns only what it wrote in each extension point, and
   removes only that. A setting's value (for a list, one element) that is
   exactly what the part would write is adopted: the part records it with
   `adopted`, reports it `already present, adopted`, and owns and removes
   it as if it had written it. Before adopting or writing a value it does
   not already hold, it reads the other part lists: when any of format 1
   records that setting with that value and no `skipped`, the value is that
   part's, so the part neither adopts nor writes it and records `skipped:
   "another part's"`. Two lists never both claim one value. Any other value
   it did not write is another's.
3. A setting that holds one value is set only when it is empty or
   already this part's.
4. Each part writes a part list (below) on install and removes it on
   uninstall.
5. Setup is idempotent and assumes nothing about other parts.

**The part list.** Each part keeps one file,
`${XDG_STATE_HOME:-$HOME/.local/state}/my-harness/parts/<part>.json`,
that says what it placed on each base harness. A part writes only its
own file, and may keep a temporary dotfile beside it while it writes. A
whole-status reads every `*.json` file in the dir that is not a dotfile.

```json
{
  "format": 1,
  "part": "maid-harness",
  "harnesses": {
    "claude": [
      { "kind": "marketplace", "name": "maid" },
      { "kind": "plugin", "name": "maid@maid" },
      { "kind": "mcp", "name": "chrome-devtools", "skipped": "no graphical Chrome" }
    ],
    "kiro-cli": [
      { "kind": "skill", "name": "notes" },
      { "kind": "mcp", "name": "chrome-devtools" }
    ]
  }
}
```

- `format`: 1. A reader refuses a format it does not know, and ignores
  keys it does not know.
- `part`: the part's name, the same as the file name.
- `harnesses`: keyed by the base harness's command (`claude`, `codex`,
  `kiro-cli`, `agy`), one list of entries each.
- `kind`: `skill`, `mcp`, `agent`, `plugin`, `marketplace` or
  `setting`. `name`: what the harness's own listing calls it; for a
  `setting`, its key, or a dotted path to a nested one
  (`permissions.additionalDirectories`); a top-level key that holds the
  dots itself wins (rule 14).
- `scope`, optional: where inside the harness, for a part that places
  something in one of several places. mAId places everything in one
  place per harness and writes none.
- `skipped`, optional: install meant to place this and did not, and
  why. A reader reports such an entry and does not check it for
  presence.
- `value`, optional: for a `setting`, the value the part set, so a later
  install can tell its own value from another part's. For a setting that
  holds a list, the entry is one element of it and `value` is that element.
  Writers write a string; a reader checks no other type (rule 14).
- `adopted`, optional: `true` when the part found this exact value already
  there and took it over; it owns it as if it had written it.
- An entry without `skipped` is placed: the harness lists it, in
  whatever state (a plugin the user disabled is still placed).

The list says what is there after install, read back from each step,
plus what install skipped. Uninstall leaves only what it could not
remove, and deletes the file when nothing is left. In mAId, `part.rs`
is the one writer: install and uninstall record what `Deploy::placed`
reads back, and the browser-MCP script records its server through
`build-tool record`.

**Whole-status.** `just status --whole [agent]` reads every part's list
(`whole.rs`) and reports each entry, per harness and part, in one of six
states: present, present (disabled) (the listing says the user turned it
off), missing, skipped (with the part's reason), not checkable, or
unreadable. Presence is read from the harness's own listing or config:

| Harness | skill | agent | mcp | plugin | marketplace | setting |
|---|---|---|---|---|---|---|
| `claude` | `~/.claude/skills/<n>/SKILL.md` | `~/.claude/agents/*.md` frontmatter `name` | `~/.claude.json` `mcpServers` (scope `user`) | `claude plugin list --json`, `user` scope only | `claude plugin marketplace list --json` | `~/.claude/settings.json` key (rule 14) |
| `codex` | `~/.codex/skills/<n>/SKILL.md` | - | `codex mcp list --json` | `codex plugin list --json` | `codex plugin marketplace list --json` | - |
| `kiro-cli` | `~/.kiro/skills/<n>/SKILL.md` | `~/.kiro/agents/*.json` `name` (or file name) | the agent named by `scope`, its `mcpServers`; no scope: `~/.kiro/settings/mcp.json` | - | - | `~/.kiro/settings/cli.json` key (rule 14) |
| `agy` | `~/.gemini/config/skills/<n>/SKILL.md` | - | - | - | - | - |

claude's MCP is read from its config because `claude mcp list` and `get`
start each server. agy is checked by path only. A kiro MCP's sub-agent
comes from each entry's `scope`, so `--whole` takes no sub-agent argument.

**Reader rules.** Any whole-status reader, in any repo or language, follows
these, and the conformance set below checks them. A rule is what changes a
state or the exit code. The report's wording, layout, order and reasons
are free.

1. **Text.** Every file and every CLI's stdout is UTF-8, strictly. A
   byte-order mark is not stripped, so a JSON file that starts with one is
   not JSON. stderr is decoded leniently and only ever feeds a reason. In a
   JSON object with a repeated key, the last one counts.
2. **Which lists.** Every entry in the parts dir named `<stem>.json`, with
   a stem that is not empty and does not start with `.`. No parts dir: no
   lists, which passes. A parts dir that exists but cannot be listed
   (permissions, not a dir) aborts: no report, exit 1. That is the only
   abort. Every other failure stays with its list or its entry.
3. **Refused lists.** A list is refused, reported by file name, and fails
   the run when: it cannot be read (permissions, a dir), is not UTF-8,
   JSON or an object; `format` is not a JSON number whose value, read as a
   double, is 1 (`1.0` passes, `true` and `"1"` do not); `part` is not a string equal to the
   stem, exactly; `harnesses` is not an object of arrays; an entry is not
   an object; `kind` or `name` is not a string; or `scope` or `skipped` is
   neither a string nor null. Other keys are not read, and `value` and
   `adopted` are taken whatever their type (rule 14 reads a setting's
   `value`; one that is not a string makes that entry not checkable). No entry of a refused list is checked; the other
   lists still are.
4. **Absent and present.** A field that is null is absent. A field that is
   a string is present, whatever it holds: `"skipped": ""` is skipped, and
   `"scope": ""` is a scope.
5. **Each entry**, in this order: with `skipped`, skipped, and nothing is
   read. A harness key or kind not exactly one in the table: not
   checkable. A `-` in the table: not checkable. Then its scope (rule 6),
   then its source.
6. **Scope.** Absent: the harness's one place. A claude plugin or MCP:
   `"user"` is that place too, and any other scope is not checkable. A
   kiro MCP: the scope names the kiro agent (`""` included) whose
   `mcpServers` should hold it (no such agent: missing); with no scope,
   `~/.kiro/settings/mcp.json`.
   Any other kind with a scope: not checkable.
7. **Names** compare exactly, case included, with the names a source
   holds, and are never joined into a path: a skill `a/b` or `.` is
   missing.
8. **CLIs.** A CLI is on PATH when an absolute PATH dir, in order, holds
   a regular file of that name with any execute bit. Empty and relative
   entries are skipped, never read as the current dir: status runs what
   it finds, and a `claude` planted in a checked-out repo must not run. Not on PATH: not checkable. It
   runs with the reader's HOME. If it fails to start, exits non-zero, or
   its stdout is not one JSON value (whitespace around it is fine, empty
   is not) or not the listing's shape, every entry it would answer is
   unreadable. With exit 0, stderr is ignored: stderr never decides a
   state. Shapes: claude's `plugin list` is an array, items named by `id`,
   counting only `"scope": "user"`; claude's marketplaces an array by
   `name`; codex's `plugin list` an object whose `installed` array names by
   `pluginId`; codex's marketplaces an object whose `marketplaces` array
   names by `name`; codex's `mcp list` an array by `name`. An item that is
   not an object or has no string name is ignored. An item with
   `"enabled": false` (the JSON value; not `0`, not `"false"`) is present
   (disabled).
9. **Config files** (`~/.claude.json`, `~/.claude/settings.json`,
   `~/.kiro/settings/*.json`). Absent: nothing listed. Only "does not
   exist" is absent, here and for a dir in rule 10; any other failure, a
   parent that is not a dir included, is unreadable. Cannot be read, not
   UTF-8, not JSON, or not an object: unreadable. A key counts whatever its
   value, null included (a setting's key is found as in rule 14). A member read for names (`mcpServers`) that is
   absent or null lists nothing; of any other non-object type, unreadable.
10. **Dirs.** An absent skills or agents dir lists nothing. One that
    exists but cannot be listed (permissions, not a dir): every entry read
    from it is unreadable. A skill is present when `<dir>/<name>/SKILL.md`
    is a regular file after links; when that cannot be confirmed (absent,
    not a file, its own dir without permission), missing.
11. **Agent files** are every regular file, after links, named `<stem>.md`
    (claude) or `<stem>.json` (kiro): stem not empty, dotfiles included,
    extension exact. One that cannot be read or is not UTF-8 is unreadable
    under its stem, which is the name an entry matches it by.
    - kiro: a JSON object, named by its `name` when that is a string, else
      by its stem. Not JSON or not an object: unreadable under its stem.
      Its `mcpServers` reads as in rule 9.
    - claude: lines split on LF, a trailing CR dropped. A first line that
      starts with a byte-order mark: unreadable under its stem.
      Frontmatter is the lines between a first line exactly `---` and the
      next line exactly `---`. The name is on the first of those lines
      that starts `name:` at column 0: the rest of that line, cut at its
      first ` #`, trimmed of spaces and tabs, then one pair of matching surrounding quotes
      (`"` or `'`) removed. No frontmatter, or no `name:` line: the file
      names nothing.
12. **One name, several sources** (listing items, agent files): the best
    state wins, so order never matters: present, then present (disabled),
    then unreadable, then missing. A kiro MCP whose agent name is in two
    files is present if either lists it, else unreadable if one could not
    be read or its `mcpServers` is not an object.
13. **Exit.** 0, unless an entry is missing or unreadable or a list was
    refused: then 1. An abort exits 1; a bad invocation 2. With one
    harness selected, only its entries are checked and reported, and
    refused lists still are reported and fail the run.
14. **Settings.** A setting's `name` is first a key of the config file's
    top-level object equal to the whole name, dots included; only when
    there is no such key, a dotted path split at every `.`, each part a key
    of the object one level down (`permissions.additionalDirectories`). A
    key with dots below the top level is never found as one key. Neither
    found: missing, as is a path through a value that is not an object. A `value` compares only
    as a string: one that is neither null nor a string makes the entry not
    checkable, decided before the file is read. With a string `value`, the
    entry is present only when the setting, or when the setting is an
    array one element of it, is a JSON string equal to `value` code point
    for code point (escapes decoded, no normalisation), else missing; a
    setting or element that is not a string never matches. With no `value`
    or a null one, a found key is present. `adopted` changes no state.

**Conformance set.** `resources/build-tool/tests/conformance/<case>/`, plain
data any reader's tests can copy (`tests/conformance.rs` runs `whole.rs`
against it):

- `parts/`: the parts dir (no `parts/`: no parts dir; a file: not a dir).
- `home/`: HOME. Placeholder files keep dirs git would drop; a
  `.gitattributes` of `* -text` keeps every byte as written.
- `case.json`: `about`; optional `harness`, the one selected; optional
  `clis`, per CLI, per exact argument string, `stdout` or `stdout_file`,
  `stderr` or `stderr_file` (paths under the case), and `exit`. A CLI not
  named is not on PATH; the PATH holds only the stubs' dir, which is also
  the run's working dir, and arguments not listed exit 3. Optional `path`:
  the PATH entries to use instead, as written. Optional `modes`, paths under the run dir (`parts/…`,
  `home/…`, `bin/<cli>`) to octal modes, set before the run. Where they
  do not bite (as root) the case cannot run; mAId's test then fails
  unless `CONFORMANCE_ALLOW_SKIPS=1` is set. `expect`:
  `exit`, `aborted` (when the report aborts), `entries` (`harness`,
  `part`, `kind`, `name`, `scope` when the entry has one, and `state`, as
  above) and `refused` (file names), compared in any order.

A reader of part lists keeps its own lenient structs beside the writer's
strict ones (`whole.rs` beside `part.rs`), so a kind or harness it does not
know is one entry it cannot check, not a refused file.

**Memory.** Memory is served by MCP servers, one per persona, each bound
to one store: a git checkout the user owns. Each part registers its own
server on its own store; parts share no store config. The server is
`pensieve` (its own public repo, pinned as a flake input, in the profile
as `bin/pensieve`): it owns the store format (Claude Code's auto-memory
format under `<store>/memory/`), its tools, and its instructions, which it
builds from the store's `about.md` (what belongs here and what does not,
then a short profile). That file is the user's; no part writes it or
anything else in a store. Syncing a store (pull, commit, push) is the
user's.

mAId's server is `personal-memory` on the personal store, whose path is in
`${XDG_CONFIG_HOME:-$HOME/.config}/maid/memory-store` (`just memory-store
<path>` sets it). With none set, install reports "no personal store set"
and registers nothing. Install
registers it on claude with `claude mcp add -s user personal-memory --
<profile>/bin/pensieve --store <path>` and records it as `mcp`. The entry
is mAId's while its command is the profile's `bin/pensieve`; install
re-registers it when the store path changes and leaves any other
same-named entry, or one another part's list records as placed, alone. A
store that is not there is reported and skipped, never created, and mAId's
earlier entry is removed. An unreadable `~/.claude.json` fails install.

**Claude's own automatic memory is turned off** by a part whose server is
registered, so saves go to a server and not to Claude's memory dir: user
settings `env.CLAUDE_CODE_DISABLE_AUTO_MEMORY = "1"` (Claude applies
user-settings `env` at startup). A part sets it only while its own server is
registered after install. Under rules 2 and 3 it is set when empty, adopted
when identical and no other part's list records it, otherwise left and
recorded skipped. A part removes its own value when it uninstalls, or when
it installs with claude on PATH and no server of its own; another part's
value it leaves alone. A part with a server that follows these rules then
takes the value on at its next install; until then Claude's own memory is
on. The `autoMemoryDirectory` value an
older mAId set is removed when it is still the recorded value.

`just memory-harvest --to <dir> [--project <path-prefix>]` copies Claude's
per-project memories (and, with no `--project`, codex's `memories/`
registry) into `<dir>/memory/`, never changing the originals. `<dir>` must
exist. A prefix matches the way Claude names project dirs (every character
that is not a letter or digit as `-`), so it may take a sibling; each
project taken is printed. A rerun changes nothing, and a same-name
different memory lands as `<stem>-N.md`, reported.

codex, kiro-cli and agy reach the same servers by registration, later.

## Tech Stack

<!-- Languages, runtimes, frameworks, key libraries. Versions
     only where version matters. -->

- **Runtime:** Rust (cargo workspace) + Just for the verb
  surface. `resources/build-tool` is today's only
  workspace member; future Rust crates (e.g. `kaimux/`)
  join as additional members.
- **Isolation:** one flake, two outputs. `devShells.default`
  (loaded by `.envrc` via direnv) is the dev closure: the rust
  toolchain (rust-overlay), `just`, and `jq` (the install tests read
  the agents' JSON, and the browser MCP install edits kiro's global
  config with it). `packages.default` is
  the installable: the skills plus `chrome-devtools-mcp`
  (packaged from its npm tarball, which has no dependencies)
  wrapped with `nodejs_22`, and `pensieve`, the memory server, from its
  own flake (a pinned input).
- **Dependencies.** mAId does not care how a prerequisite got
  onto PATH (host package, home-manager, nix profile), and does
  not check for it; a missing one fails in the step that needs it.

  | Kind | What | Provided by |
  |---|---|---|
  | Runtime prerequisites | coding-agent CLIs, `git`, a graphical Chrome (browser only) | the environment |
  | Runtime-provided | skills, node, chrome-devtools-mcp, pensieve | the mAId profile |
  | What the vendor document skills run | `python` with openpyxl, pandas, pypdf, pdfplumber, reportlab, pypdfium2, pdf2image, Pillow, lxml, defusedxml, python-pptx, markitdown; `node` with the `docx` and `pptxgenjs` packages; LibreOffice (`soffice`), `pandoc`, `markitdown`, `qpdf`, poppler (`pdftotext`, `pdftoppm`, `pdfimages`), ImageMagick (`magick`) | the environment; `just status` names each missing command |
  | Install and dev prerequisites | `nix` with flakes; `direnv` optional | the developer |
- **Entrypoints:** Justfile organised as a root file with
  `mod` declarations per area, so verbs are namespaced by
  what they touch:
  - **`resources::*`** (operate on `$HOME` or the AI tools).
    Verbs follow the `<action>-<resource-kind>` pattern, each
    taking the coding-agent selector (`claude|kiro|codex`; omit
    for all three): `just resources::install-profile` (validate +
    build the profile, no selector), `just resources::install-skills
    [agent]` (install the claude/codex plugin, link the rest at
    the profile),
    `…::uninstall-skills [agent]`, `…::status-skills [agent]`,
    `…::check-skills [agent]` (pre-install: reads each skill from
    the checkout, so no deploy is needed), `…::smoke-skills
    [agent]` (post-install: the agent must find the skill among
    everything deployed), `…::verify-skills [agent]` (both
    stages). All three cost API credits and are gated behind a
    confirmation prompt. `…::verify-skills-one <name> [agent]`,
    `…::verify-skills-kind <kind> [agent]` (one kind of skill
    test across every skill — see Testing), `…::verify-skills-dry
    [name]` (construct and structurally check every prompt
    without calling an agent; free).
    Browser-control MCP adds
    `install-browser-mcp [agent]` /
    `uninstall-browser-mcp` / `status-browser-mcp` /
    `browser-mcp-allow <pattern>` (register/report the server
    + append allowlist patterns), plus `verify-browser-mcp`
    (attended, `[confirm]`-gated — drives real Chrome).
  - **`kaimux::*`** (operate on the kaimux crate):
    `just kaimux::build` (release + copy to `dist/`),
    `just kaimux::test`, `just kaimux::integration`.
  - **Memory** at the root: `just memory-store <path>`, `just
    memory-harvest --to <dir> [--project <path-prefix>]`; `just status`
    reports the store, its server and Claude's own memory. Each wraps a
    `resources::*-memory*` verb over `build-tool memory`.
  - **Install** at the root: `just install [agent]`
    (profile, then skills and browser MCP), `just uninstall`,
    `just status` (`--whole`: every part's list against each
    harness), `just
    rollback`, and the Macro test `just
    verify-install` / `just verify-install-paid` (both `[confirm]`-gated).
    Later installables (kaimux) join here.
  - **Workspace hygiene** at the root (no namespace —
    operates on every member): `just test`, `just fmt`,
    `just fmt-check`, `just lint`, `just check`,
    `just ci` (full gate).
  Each recipe is a thin wrapper over native cargo (the
  kind-routing verb adds a shebang body) — `just --list` shows the root verbs,
  `just --list <module>` drills into a module. There is
  no installed binary on `$PATH`; the build-tool is
  invoked through `cargo run -p build-tool` from the
  checkout (wrapped by Just).

## Layout

<!-- Directory tree with a one-line annotation per entry. What
     lives where, not why. -->

```
mAId/
├── Cargo.toml              workspace root: members = ["resources/build-tool"]
├── Cargo.lock              committed (binary-workspace policy)
├── Justfile                root verb surface (workspace hygiene + `mod resources` / `mod kaimux`)
├── rust-toolchain.toml     stable + clippy + rustfmt
├── flake.nix / .envrc      dev shell (rust toolchain, just, jq) and the installable package
├── resources/
│   ├── Justfile            `resources::*` verb surface (install/uninstall/status/verify)
│   ├── build-tool/         Rust crate — the pipeline (check/install/uninstall/status/smoke)
│   │   ├── Cargo.toml      deps: clap, anyhow, gray_matter, serde, serde_json, tempfile
│   │   ├── src/main.rs     the shim: clap surface + dispatch, in pipeline order
│   │   ├── src/lib.rs      module wiring + the pipeline doc comment
│   │   ├── src/shared.rs   Agent, REGISTRY, roots — what every stage speaks
│   │   ├── src/harness.rs  fixtures, the five kinds, prompts, invocation, verdicts
│   │   ├── src/deploy.rs   the Deploy trait: Plugins (claude, codex), Symlinks (kiro, agy), BringIns
│   │   ├── src/part.rs     mAId's part list: the format and its one writer
│   │   ├── src/whole.rs    whole-status: every part's list, checked against each harness
│   │   ├── src/memory.rs   the personal store, personal-memory on claude, Claude's memory off
│   │   ├── src/harvest.rs  copying each harness's own memory into a store dir
│   │   ├── src/settings.rs Claude's user settings file, kept key for key
│   │   ├── src/reads.rs    Claude's additionalDirectories element for mAId's skill files
│   │   ├── src/stages.rs   content → check → install → smoke
│   │   ├── tests/integration.rs  cross-stage tests against the real repo
│   │   ├── tests/plugins.rs  ignored: Plugins and BringIns against the real claude/codex CLIs in a temp HOME
│   │   ├── tests/no_cli.rs   install with no agent CLI on PATH keeps the old links
│   │   ├── tests/stops_on_list.rs  an unreadable part list stops install and uninstall, through the binary
│   │   ├── tests/memory.rs   the memory verbs through the binary, temp HOME
│   │   ├── tests/memory_install.rs  install's claude memory step, a stub claude, its part list
│   │   ├── tests/conformance.rs  whole.rs against the conformance set
│   │   ├── tests/conformance/  the whole-status conformance set, one dir per case
│   │   └── tests/reads_install.rs   install's claude skill-reads step beside memory's
│   ├── content/            the deployable skills (copied into the profile)
│   │   ├── skills/<name>/SKILL.md   (incl. browser/ — browser-control safety posture)
│   │   ├── skills/kdevkit/  SKILL.md core + phases/, tiers/, setup.md, interviews.md
│   │   └── skills/kyodakit/ SKILL.md: developer judgement, any language
│   ├── browser/            browser-control MCP (not symlinked — runnable)
│   │   ├── launch          allowlist-enforcing launcher; the flake wraps it as maid-browser-mcp
│   │   └── manage          data-driven MCP registrar (MCP_AGENTS table; every agent global, kiro by editing its config)
│   └── tests/              fixtures + the attended browser test (the runner itself is in build-tool)
│       ├── browser-functional   ATTENDED test: drives real Chrome, asserts off-list blocked
│       ├── browser-mcp          free: kiro's browser MCP registration, temp HOME, stub CLIs
│       ├── rollback             free: `just rollback` and install's generation cleanup, temp HOME
│       ├── verify-install       the installable Macro test, on the real install (`just verify-install`)
│       ├── conversational-stream.txt  the --stressed prefix
│       └── skills/<name>.smoke   fixtures: skill + playback/enact sections (runner owns the five kinds)
├── kaimux/                 tmux-pane orchestrator for coding-agent sessions
│   ├── Justfile            `kaimux::*` verb surface (build/test/integration)
│   ├── Cargo.toml          deps: clap, anyhow, fd-lock, nix, notify, serde, serde_json
│   ├── src/main.rs         single-file, typeclass-shaped (Session/Store/Wrapper/Loop)
│   └── tests/              bash integration tests against real tmux
├── dist/                   gitignored — built binaries land here
├── target/                 gitignored — cargo's build dir
└── specs/
    ├── project.md          this file
    ├── initiative/         initiatives (kept as the record after close)
    ├── feature/            in-flight + completed feature records
    └── backlog/            per-item files for wanted future work
```

## Testing

<!-- How this project is tested: unit, integration, smoke,
     manual. Which commands run which suite. Which are
     load-bearing vs. nice-to-have. -->

Two test layers, each scoped to what they verify.

**`just test` — workspace unit tests.** Rust unit tests
covering the content validator and the symlink state
machine against a `tempfile`-fake `$HOME`, plus the kaimux
crate's 53 unit tests against a tempdir `Store`. Fast
(sub-second). No real `$HOME` side effects, no API credits.
Load-bearing — this is the kdevkit Test Gate default. `deploy.rs` carries
the structural coverage: a full install→status→uninstall round-trip in
a fake `$HOME`, `--force` semantics per state, and deployment
detection — the tests most sensitive to a symlink-layout regression —
and, for `Plugins`, reading each CLI's listing, the commands each
plugin state leads to, and reaping the links an older install left.
`whole.rs` checks each (harness, kind) against a temp HOME and stub CLIs
on a PATH it is handed, never the real ones, and `tests/conformance.rs`
runs it, and the binary for the exit code, against every case of the
conformance set (see "My harness").

`cargo test -p build-tool --test plugins -- --ignored` drives the real
claude and codex CLIs in a temp HOME (install, update, a disabled
plugin, a codex marketplace whose dir is gone, uninstall), and the
bring-ins: install and a no-op reinstall, uninstall keeping a marketplace
another plugin uses, the user's own marketplace kept and not claimed, and
an unreachable source failing the run. Free (no model calls) but needs
both CLIs and network (it clones the vendor repo), so it is out of `just
test`; run it after any change to `Plugins` or `BringIns`.

`nix develop -c resources/tests/browser-mcp` drives
`resources/browser/manage` for kiro in a temp HOME with stub `kiro-cli`
and Chrome (free, seconds): other servers and keys in kiro's global
config kept, reruns byte-identical, a same-named entry that is not
mAId's left alone, a config that is not JSON left alone, a linked
config kept a link with its mode through an atomic rename, a link
cycle refused, uninstall with no `kiro-cli`, the part list's entries
and whole-status reading them, and kiro-cli never run. Run it after
any change to `manage`.

`nix develop -c resources/tests/rollback` drives `just rollback` and
install's generation cleanup against the real claude and codex CLIs in a
temp HOME and a temp profile (free, under a minute): rollback, an
install whose build fails keeping the generations it would drop,
rollback after rollback-then-install, an earlier generation the agents
cannot take, no earlier generation, the 30-day cleanup keeping the
previous install (also after an unchanged install), rollback past a
link mAId did not write, the part list install leaves (placed,
skipped, the vendor marketplace, gone after uninstall), install and
uninstall with no claude or codex on PATH keeping the bring-ins mAId's
(a later uninstall still removes them), Claude's
plugin-dir grant (there after install, gone after uninstall), and `just
status --whole` finding what mAId placed, then a second part's list beside
it.
It needs network for the vendor marketplace.
Run it after any change to the profile recipes or the part list.

**`just verify-install` is the installable Macro test**
(`resources/tests/verify-install`), run on the real install: install
from a clone that is then deleted, what each agent's own CLI lists, the
browser server under `env -i HOME=…`, a search for checkout paths, a
second clone's takeover and rollback, uninstall keeping user data, and a
reinstall from the checkout it ran from (a trap guarantees that, and
removes any placeholder user-data file it wrote). Free, under a minute;
`just verify-install-paid` adds 9 model calls asking each agent for its
skills and for a marker line. It ends with no earlier install to roll
back to, since uninstall removes the profile.

One test deliberately breaks the fake-`$HOME` pattern:
`shipped_content_validates` points the validator at the **real**
`resources/content/`. Everything else here builds a synthetic tree, so
without it the suite can be green while the content on disk is
uninstallable — which is exactly what happened when two descriptions
carried an unquoted `: ` (see "Writing a skill"). It reads only, so it
keeps the no-side-effects property.

**`just resources::check-skills` / `…::smoke-skills` — AI-tool
functional tests, on opposite sides of install.** Drives
the real coding agents (claude, kiro, codex) against the `.smoke`
fixtures under `resources/tests/skills/`. How a test is scored is a
property of the fixture (`harness::Assertion`), not of the kind:
**Reply** (a literal substring, a judged narrative, or both — checked
together) for a skill with no inspectable side effect, or **Behavioral**
(`--- setup ---` / `--- assert ---` shell blocks — the agent runs
against a seeded scratch workdir and the assert inspects what it did)
wherever an artefact exists. Every fixture runs against each requested
agent; the verb's `--agent` selector (a comma list on the verification
verbs) scopes to one or more, default all three, all required. Slow
(minutes) and costs API credits; gated behind a confirmation prompt in
the Justfile.

**Only `smoke` needs the skills installed** (it reads the installed
copy, so run `just install` after an edit). The reach axis is the
install boundary: an explicit prompt carries the skill's own text
inline, so `check` needs no install at all — which is what makes a
content change provable *before* it is made live for every session on
the machine. An implicit prompt makes the agent find the skill unaided,
competing against every other installed skill for a capped, shared
description listing, so `smoke` requires `just install` to have run
(it says so and stops, rather than failing every test obscurely), and
`smoke-skills`, `verify-skills` and `verify-skills-one` refuse a profile
that is not this checkout's build. A failure therefore
localises: pre-install means the content is wrong; post-install with
check passing means deployment or description-budget competition.

### The five kinds of skill test

A verification *style* (above) is how a test checks. A **kind** is what
question it answers. The two are orthogonal, and conflating them is how
the suite once drifted into a dozen fixtures that all answered the same
question. Kinds are two axes composed:

- **How the skill is reached.** *Explicit* — the prompt carries the
  skill's text inline, which isolates content: a failure means the skill
  is wrong, not that it failed to load, and nothing needs deploying.
  *Implicit* — the prompt states only the task, so the agent must
  recognise it and load the right skill unaided from what is installed.
  Discovery borrows a fixture's enact task, so it runs in that fixture's
  seeded dir, read-only, where the files the task names exist.
- **What is verified.** The skill *plays back* the contract it was
  designed for (recites its rules), or *enacts* it (does the thing).

| Kind | Reach | Verifies | Question it answers |
|---|---|---|---|
| `activation` | explicit | announces | Is the skill reachable and parseable, and does it load when named? |
| `discovery` | implicit | announces | Does the skill's own `description:` trigger from a bare task? |
| `playback` | explicit | recites | Does it state its own rules correctly? |
| `enact` | explicit | performs | Loaded, does the agent actually do the thing? |
| `integration` | implicit | performs | End to end — does it fire *and* do the thing? |

`playback` + implicit is deliberately empty: reciting rules is not a
task a user phrases implicitly, so the cell has no natural test.

**`activation` and `discovery` depend on a self-announce contract.** A
skill that declares `You begin every response … with the literal line
[<skill>] applies` (today: `browser`, `notes`, `writing-style`,
`kreviewkit`, `kyodakit`) can be
checked at the reply level, because the marker is text the agent can
only know from the file. The announce line is there for the reader, not
for ceremony — it attributes a reply to a written contract rather than
to the model's own judgement, which is what lets you calibrate trust,
tell "the rules are wrong" apart from "the skill never fired", and
notice a broken install instead of silently getting plausible
non-skill output. A **workflow** skill earns that differently:
`kdevkit`'s evidence is the artefacts it leaves (phase-prefixed
commits, a feature spec, gate-shaped PR bodies), so a per-turn stamp
across a long session would be noise. The runner therefore reads the
contract from the skill itself rather than assuming it; a
skill without one skips activation/discovery and is proven to fire by
its `enact` / `integration` artefacts instead. The contract is read from
the **checkout** copy, which is the same file the deployed symlink
resolves to, so both stages agree on whether a marker is promised.

The pair is diagnostic. When a skill's explicit test passes and its
implicit counterpart fails, the fault is triggering, not content — no
bisection needed. `activation` and `discovery` are **generated** by the
runner from a fixture's `skill:` field, so no fixture authors them. No
prompt carries a path at all now: an explicit prompt carries the skill's
text, and the deployed roots stay derived from `REGISTRY` for the
install verbs alone — hand-copying them into prompts is what let them
drift out of sync per fixture.

**What a new skill is expected to carry.** At minimum one `enact`
section per load-bearing behavior — which also produces its
`integration` run — plus one `playback` section per guardrail or
absence path that produces no artefact (a refusal, a stop-with-error, a
safety posture). `activation` and `discovery` come free. Prefer the
behavioral form for `enact` wherever the skill's correct action leaves
an inspectable change; fall back to a judge narrative only when the
output is irreducibly prose.

**A skill's own loading is a load-bearing behavior too.** Where a skill
defers content to modules (see Architecture), "reads the right module
at the right moment" is a behavior that fails *silently* — the agent
proceeds from memory and the output looks plausible. It therefore earns
a fixture like any other: a `playback` that the right module is named
per stage, and a behavioral `enact` that drives a stage transition and
asserts the next stage's discipline shows up in the artefacts
(`kdevkit-module-load`, `kdevkit-phase-boundary`,
`kdevkit-handoff-resume`, `kdevkit-consolidate`, `kdevkit-closure`,
`kdevkit-code-review-panel`, `kdevkit-judgement-load`). A wrong trigger is
invisible to the deterministic gates, so this layer is the only one
that can catch it.

The same applies to **state carried across a boundary**: a stale or
missing handoff record fails silently too, and the agent proceeds
plausibly from the wrong starting point. Assert on the artefact the
boundary should have left, and pair every negative check with a
positive one — "the block no longer says planning" passes if the
block was deleted, which is not the behaviour wanted.

The record's **append-only** half needs its own trap check. A
count-based assertion ("there are crossings") passes when the list
was *replaced* rather than appended to, which destroys the history
the list exists for. Seed a crossing and assert it survives — the
positive check for an append-only structure is that the *earlier*
entry is still there, not that some entry is.

### Writing a skill

A skill has to survive two things. Design for both.

**It has to be found.** An agent sees only `name` + `description`
before deciding whether a skill applies — never the body — and that
listing is capped and shared across every installed skill. Over
budget, descriptions get shortened. So the description carries the
triggers, in the words a user would actually type, in a line or two;
the announce line is named there too. A "when to use this" section in
the body cannot fire a skill, because only an already-fired skill
reads the body.

**It also has to parse.** Frontmatter is YAML, so a `: ` anywhere
inside a `description:` value reads as a nested mapping and the whole
skill fails validation — `just install` then refuses to deploy *any*
skill, not just the broken one. Descriptions naturally attract it
(`specs/: plan or start`, `briefing tool: read-only`). **Single-quote
the value** whenever it contains a colon; the values already use `"`
freely, so `'…'` avoids escaping. `shipped_content_validates` in
build-tool is the test that catches this — every other validator test
builds a synthetic `TempDir` tree, so before it existed the suite could
be green while the real content was uninstallable.

**It has to hold.** Loading a skill buys the start of adherence, not
all of it. On a long prompt, with a large skill, the rule that slips
is the one about order — do X before Y. So the rules that must not
break go at the top, ranked, saying what they outrank ("rule 0; beats
anything else about how the reply starts"), and each imperative sits
where the agent acts on it rather than sections earlier. Precedence is
what holds; rewording is not.

**It has to outlast its agents.** Write each instruction as *what*
must be done, in terms true for any coding agent. Then, where it
helps, say *how* for each current agent, labelled by agent, using its
CLI or tools as they are today (checked, with the version where it
matters). A new agent, or an upgrade, then changes only the "how"
lines, and the rule an agent must keep never names one host's tool as
the rule itself.

Both failure modes are silent — the skill does the work and omits the
contract, or never loads and the agent improvises a plausible answer.
That is why `discovery` and `integration` exist as test kinds, and why
they assert an artefact or a self-emergent marker rather than trusting
the reply.

Two habits the suite depends on: trigger and marker behavior is
probabilistic, so sample 3–5 runs and record the ratio rather than
trusting one pass; and when a test fails, check the answer against
`SKILL.md` before concluding the agent misbehaved — a fixture that
contradicts the skill it tests fails honest work.

A fixture therefore carries only what is specific to it — never a path,
never a load-the-skill preamble:

```
skill: <skill-name>          which skill is under test (required)
tools: claude,kiro,codex     agents to run (default claude)
--- playback ---             optional; explicit recitation test
task: <the question>
expect: <narrative the judge scores against>
--- enact ---                optional; drives enact + integration
task: <the imperative task, phrased as a user would>
expect: <narrative>          for prose skills with no artefact
--- setup --- / --- assert --- optional; seed and inspect a workdir
```

`--dry-run` on either stage constructs every prompt and checks it
structurally **without calling an agent** — explicit prompts must carry
the skill's own text, implicit prompts must leak no skill name, path, or
marker, and a malformed fixture fails without stopping the run before any
credits are spent. Run it before any paid run. `--kind <list>` scopes a run to one
kind (`just resources::verify-skills-kind <kind>`).

**Prefer behavioral where an artefact exists.** When a skill's
correct behavior produces an inspectable change (a file written, a
commit made), the fixture should be behavioral (setup/assert) and
run tri-tool — a recitation probe only proves the agent *says* the
right thing. Reserve substring/semantic for genuinely non-artefact
behavior: error or absence paths (a refused action, a
stop-with-error) where a compliant agent writes nothing. A
behavioral assert must fail a no-op agent (pair a presence check
with the absence check) or it proves nothing.

**Discount what you know; don't threshold what you don't.** When an
assert has to decide whether the agent produced *prose* — a why, a
summary, an explanation — a length threshold is the wrong tool in both
directions, and tuning it converges on nothing. Measured: two lines
rejected a compliant one-line answer; fifteen words rejected an
eight-word one; four words was satisfied by a coding agent's own
attribution footer, which is five. Filter the boilerplate you can
*name* — blank lines, git trailers, attribution and URL lines, all
line-initial by convention — and then require that something survives.
That check has no threshold to be wrong about.

**Two checks needing the same set share one definition.** Two asserts
both wanted "list markers an agent might write in front of a line" and
each carried its own enumeration; one commit edited both lines and added
different markers to each, reopening a hole the earlier fix had closed.
Where two checks depend on the same notion, define it once in the
fixture (a shell function is enough) — an enumeration duplicated is an
enumeration that will drift, and it drifts in the commit that edits it.

**A fixture proving itself is not the same as two fixtures being
comparable.** The vacuity check above — a behavioral assert must fail a
no-op agent — is per-fixture, and it cannot tell you that two fixtures
demand the same work. A/Bing two designs against each other needs both
arms audited for equal demand: one arm requiring a commit while the other
was satisfiable by uncommitted edits produced a clean-looking result that
measured the difference in demand, not the difference in design.

**A skill's pre-install stage carries only `SKILL.md`.** Prose added to a
module is invisible there no matter how well written, so a fixture testing
module content must run post-install. A module-prose fix that "fails" at
the pre-install stage is telling you about the stage, not the fix.

**Adding a second mechanism to a skill means auditing every place the
first one is *instructed*, not just where it is *described*.** Describing
a new way to do something while other files still instruct the old way
unconditionally produces two rules an agent cannot both obey — and a
compliant agent then does both, which reads as drift. The description is
the tempting place to look and the wrong one.

kdevkit's Test Gate uses `just test` by default. SKILL.md
prose revisions add `just resources::check-skills` (judge mode)
as their A/B evidence — the pre-install stage is the one that
isolates content, so it is the honest A/B. kdevkit's close-out can run
`just status` after an install to confirm the profile generation and
that every link resolves.

### Functional tests are user-driven

Agentic runs (an AI assistant working through this project)
**must** stop at `just test`. The `check-skills` / `smoke-skills` /
`verify-skills` verbs cost API credits and take minutes; whether to spend that budget
on a given change is a human call. The agent prepares the
fixture, names the exact command, and hands off — it does
not run it. The Justfile's `[confirm]` gate on each of those verbs
provides a second line of defense. `verify-skills-dry` is free and
un-gated — it constructs and structurally checks every prompt without
calling an agent, so an agentic run may use it.

Commands the user runs by hand:

- Pre-install only (no deploy needed): `just resources::check-skills`
- Post-install only: `just resources::smoke-skills`
- Both stages: `just resources::verify-skills`
- A single fixture, both stages: `just resources::verify-skills-one <name>`
  (e.g. `just resources::verify-skills-one notes-git-commit`).

The fixture file's basename (without `.smoke`) is the
`<name>`.

**Prefer the isolated verb for any paid run.** `just
resources::verify-skills-isolated <check|smoke|verify> [name] [flags]`
runs the same tests from a throwaway clone of the committed branch: it
builds the clone into a throwaway profile, points every agent's skill
links at it, runs, puts the links back on whichever profile owned them,
and exits 3 if a test changed
the clone or this checkout (4 if the links could not be put back). It
needs an existing `just install`, the profile it restores the links to. Tests have escaped their scratch dir into
the real checkout before (see
`specs/backlog/test-runner-workdir-containment.md`); in the clone,
that damage is contained and reported. Commit first: uncommitted edits
are not in the clone.

Flags any check/smoke/verify run takes:

- `--repeat N` — each test N times, with a `TALLY <test>: k/N` line.
  Agent runs are noisy; use 3 before trusting a difference.
- `--control` — beside each enact test, the same task and check
  without the skill ("do not load or follow any skill"). The gap is
  what the skill adds. Meaningful at the check stage.
- `--drift` — the skill, then ~4,000 words of unrelated conversation,
  then the task: rules read early and acted on later.
- `--stressed` — the same conversation before the skill (retention
  when the skill is still the latest thing read).

### Testing the developer-judgement slot

kdevkit reads a senior-developer judge at session start and with every
phase module, and tells every dispatched agent to load it. The judge is
`kyodakit` unless `project.md` sets `judgement:`. Three things are tested,
each at three levels:

| | A. kdevkit still works | B. the slot works, for any judge | C. kyodakit is a good judge |
|---|---|---|---|
| Unit — `just test` | existing build-tool tests | `shipped_judgement_role_contract`: kdevkit names `kyodakit` as the default and `judgement:` as the override; the default ships; `judgement:` is documented | — |
| Integration — `check` | every `kdevkit-*` fixture | `kdevkit-judgement-slot` (a stub judge via `judgement: path:`, leaving `JUDGED.md`), `kdevkit-judgement-load`, `kdevkit-judgement-live` (the default judge), `kdevkit-judgement-named` (judge set in `project.md`) | `kyodakit*` fixtures, with `--control` for the gap |
| Production smoke — `smoke` | the same, skills found unaided | the same | the same |

The full replay, cheapest first:

```
just test                                   # unit, free
just resources::verify-skills-dry           # every prompt, free
just resources::verify-skills-isolated verify 'kdevkit-*'           # A
just resources::verify-skills-isolated verify 'kdevkit-judgement-*' --repeat 3   # B
just resources::verify-skills-isolated check 'kyodakit*' --repeat 3 --control   # C
just resources::verify-skills-isolated check 'kdevkit-judgement-*' --drift      # B under drift
just resources::verify-skills-isolated check 'kyodakit*' --drift                # C under drift
```

**Trimming kyodakit.** Keep the judge minimal so it holds in long
sessions. Trim with kdevkit's "Tuning what an agent uses" loop, scored by
`resources/tests/kyodakit-trim-score` (~20 paid runs): the feedback test on
claude and codex (the one test kyodakit clearly changes) and the all-rules
recital on every agent, 3 repeats each and once under `--drift`. Cuts
change wording only and never remove a rule, since a rule no test can
fail would otherwise be "trimmed" away at no cost to the score. A shorter
file at an equal score is kept. Run the full judgement replay above once on
the final version.

A fixture selector ending in `*` matches by prefix (`'kdevkit-*'`
is every kdevkit fixture); without it, it names one fixture. Tuning kyodakit against these results follows
kdevkit's "Tuning what an agent uses" (`phases/dev.md`): one change per
run, fixtures fixed, keep only what raises the tally.

Quality gate: `just fmt-check` + `just lint` + `just check`
(or the bundled `just ci`). Run after any implementation
slice.

## Deployment

<!-- How code reaches users. Build, release, install, symlink,
     container, whatever applies. If the project isn't deployed
     in a traditional sense, describe how it's consumed. -->

Not a service — consumed locally.
`just install` validates content, builds the mAId profile, installs
the claude/codex plugin and links kiro and agy at it, registers the
vendor marketplaces it brings in (Architecture, "Bring-ins"), points
`personal-memory` on the personal store and turns Claude's own automatic
memory off (Architecture, "Memory"), lets Claude read mAId's skill files (Architecture, "Skill
reads"), and writes mAId's part list (see Architecture, "My harness");
`just uninstall` reverses it, removing the list, and removes the profile;
`just status` reports the generation, each plugin and
each link, and `just status --whole` checks every part's list against
each harness; `just rollback` returns every agent to the previous install
(no selector: the profile is shared). `just resources::verify-skills` drives the
real AI tools against the installed content. Each takes an
optional coding-agent selector (`claude|kiro|codex`; omit for all
three). App workspace members (`kaimux/`) build via
`just kaimux::build` (a one-liner over `cargo build -p kaimux
--release` + copy into `dist/`).

The browser-control MCP is registered by `just install` too
(env-gated): `just resources::install-browser-mcp [agent]`, which
it calls, registers the server globally with each harness (kiro in
its global MCP config) and prints the
one-time manual step (enable Chrome remote debugging via
`chrome://inspect`). It graceful-skips where a prereq is missing
(no GUI Chrome, no harness CLI) — never a
half-register. `uninstall-browser-mcp` removes the registration
but preserves the user-owned allowlist (it's user data). Skills
and the installable stay separate verbs (skills always safe;
the installable env-gated) but share the uniform
`<action>-<resource-kind>` naming and coding-agent selector, so
the experience is symmetric across resource kinds.

### Hard constraints

- **Never write into mAId's links under `~/.kiro/skills/`, any registry
  destination, or the plugin's copies under claude's and codex's
  plugin dirs directly.** They are symlinks into the mAId profile (a
  read-only nix store path) or the agent's own cache. Edit the source
  under `resources/content/` and `just install`. (This
  guardrail is mAId-project context — it protects mAId's own deploy
  invariant — which is why it lives here, not in a globally-installed
  preamble.)
- **Registry is the single source of truth** for deployment.
  Adding a new managed path = a registry change + CR, never an
  ad-hoc edit.
- **No global state mutation** on install beyond what mAId
  owns: the mAId profile, its marketplace dir, the registry's links,
  the `maid@maid` plugin and `maid` marketplace registrations, the
  bring-in marketplaces and plugins it added, the MCP registrations, its
  elements of Claude's settings (`env.CLAUDE_CODE_DISABLE_AUTO_MEMORY`, its
  `additionalDirectories` element), and its own part list. Never the user's default
  nix profile (a home-manager may own it). The rust toolchain and `just` come
  from the dev shell; `build-tool` is invoked through `cargo run
  -p build-tool` (wrapped by Just) from the checkout — no shim
  under `~/.local/bin`, no `cargo install` anywhere in the
  install path.
- **Installed means checkout-free.** Nothing installed may name a
  checkout path, and no runtime fetch (no `npx …@latest`): a
  runtime is a pinned closure in the profile.
- **No changes to the user's env-workplace** from this
  repo. mAId stays a pure-content workspace; bootstrap
  drivers belong on the env side.
- **Public repo — no internal references in any
  artefact.** Skills, specs, commit messages, PR
  descriptions, and project docs must not name internal
  products, teams, tickets, code reviews, repos, or
  stores. Use generic placeholders or hobbyist-flavoured
  examples. When asked to capture work that mentions
  internal names, route it to a corporate spec tree
  rather than letting names land here. The `kdevkit`
  skill encodes this rule for every project; this bullet
  declares mAId as a public repo so the rule fires.

## Agent Development

<!-- Skill-scoped preferences. Each subsection is a skill name. -->

### kdevkit

- `code_review:`
  - `lenses:` — the shipped panel (`correctness`, `security`,
    `comment-hygiene`), dispatched as the host coding agent's
    native review with the three-perspective prompt (no
    project-specific reviewer skill). Opted in here as this
    repo's own dogfood of the panel this project builds —
    revisit if it proves noisier than the single-lens shape it
    replaces.
  - `fail_on: high` (default) — no per-project override needed
    yet.
- `review_brief:`
  - `enabled: true` — dev-loop completion generates a review
    briefing and uses it as the PR body. Complements
    `code_review:` rather than replacing it: that gate scores the
    diff against the project blind to the spec; the briefing is
    generated with the spec in hand.
  - `generator:` omitted — resolve the single installed tool
    advertising the review-briefing role. kdevkit dispatches the
    *role*, never a named product; this repo happens to ship one
    (`resources/content/skills/kreviewkit/`), and dogfoods it.
    What the briefing contains and how the generator runs are
    that generator's contract, not kdevkit's.
