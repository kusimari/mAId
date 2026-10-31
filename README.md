# mAId

Tool-agnostic source of truth for agentic skills — compiled
into whatever AI tool happens to be in use (Claude Code, Kiro,
Codex, future tools). claude and codex install them as a plugin
with their own plugin commands; kiro discovers them at its own
skills path.

The repo has two halves:

- **`resources/`** — four layers in one directory:
  the markdown content (`resources/content/`) the AI tools
  load; the Rust tooling (`resources/build-tool/`) that
  validates, installs, and verifies it;
  the Justfile verbs (`resources::install-skills`,
  `resources::uninstall-skills`, `resources::status-skills`,
  `resources::verify-skills`) that drive the tooling; and the
  core (`resources/core/`, `maid-core`) that the plugin's hooks
  call.
- **`kaimux/`** — sibling workspace member for the kaimux
  tmux-pane orchestrator. Built via `kaimux::build`.

`just install` builds the checkout into a nix profile
(`~/.local/state/maid/profile`) and points every coding agent at
it. Nothing installed refers back to the checkout: delete it and
the install keeps working.

## How mAId works

Every coding agent you launch is a **shell**: Claude Code, codex, kiro-cli,
agy, run as the vendor ships them. A shell can do the work, but it knows
nothing about you. The **ghost** is what makes it act like you: mAId plus
every other part you install beside it (a work repo, say). Each part names
a persona and brings in skills, MCP servers and plugins. The ghost is one
person with several personas, kept apart so each persona's learnings stay
in its own memory. mAId also ships the core, `maid-core`, which fuses
every part present and reaches the shell only through the shell's own
hooks. Nothing wraps the shell.

The core gives the ghost five functions. On Claude Code:

| Function | What it does | When it fires | What you see |
|---|---|---|---|
| **genome** | who you are, all personas fused: the personas and which one this work is for, the session bring-ins and where their state lives, the turn bring-ins as a catalog, which bring-ins are ready here, and how many decisions wait for you | at session start, and again after `/clear` or a compaction; a sub-agent gets a slice | the startup report, in the agent's context from the first reply; the `genome` prompt or resource of the plugin's `my-harness` MCP server shows it, and `just status --whole` shows the same readiness |
| **expression** | switches on only what this turn needs: the first rules of up to 3 turn bring-ins that fit, up to 3 memory pointers, and the goal and next step from the work's own record | on each prompt, sending only what changed | per-turn reminders the agent gets with your prompt; when no bring-in fits and no memory matches, nothing is sent and the shell works as it would without mAId |
| **experience** | packs the session and its sub-agents from the transcript: what was said, what the agent was told, what it did, what it touched, where it struggled | when a reflection starts | nothing directly; it is what the reflector reads |
| **sleep** | marks the session as done, to learn from | when the session ends, or when you say "close this session" or "reflect" | nothing at the end; the next session's startup report counts the sessions waiting |
| **adaptation** | a read-only reflector finds what should change; the core checks every finding's evidence against the session and routes it | right after "close this session", "reflect", or "reflect on the waiting sessions" | one decision batch: numbered one-line items, each with what it changes, for which persona, and the quote it rests on. You answer "ok", "1 and 3 yes, 2 no", "4 is work", "none" or "skip" |

After your yes, a habit is written: to the persona's memory through its
own memory server, or to a bring-in's local tier. A change to a skill, a
bring-in or the harness itself only becomes a proposal (a backlog item,
or a worktree to turn into a PR), or a note when its route is report
only. A bring-in you borrowed is never changed;
what is learned about it becomes a note for its lender. A persona
habit needs that persona's store: set yours with `just memory-store
<path>` (see Verbs).

Only Claude Code carries the ghost today. On codex, kiro-cli and agy the
skills work as plain skills. How each piece is built is in
[`specs/project.md`](./specs/project.md), "My harness". To add another
persona repo, follow its "For any persona part" section.

## Develop

The repo-local flake's dev shell provides `cargo`, `just` and `jq` (the
install tests need it):

```
direnv allow              # loads the flake on shell entry
just                      # lists every recipe
```

Without direnv: `nix develop` once per shell (or prefix
`nix develop --command` to each command). The dev shell is for
working on mAId; what gets installed is the flake's `default`
package (see Install).

The development methodology (spec-driven, phase-gated) is
encoded in the
[`kdevkit` skill](./resources/content/skills/kdevkit/SKILL.md).
Project context: [`specs/project.md`](./specs/project.md).
Feature specs: [`specs/feature/`](./specs/feature/).

## Verbs

**Install** (root, no namespace) - everything mAId installs, for
every coding agent or one (`claude|kiro|codex|agy`):

```
just install [agent]                # build the profile, point the agents at it (plugin or links), register your memory server with claude
just uninstall [agent]              # remove the plugin, the links, the MCP registration, and (no agent) the profile
just status [agent]                 # profile generation, plugins, links, MCP registration, the memory store
just status --whole [agent]         # what every part's list says it placed, checked on each harness
just rollback                       # every agent back on the install before this one
just verify-install                 # the end-to-end install test on this machine (free)
just verify-install-paid            # the same, also asking each agent (9 model calls)
```

**Memory** (root) - your personal store is a git checkout you own.
`just install` registers the memory server
([pensieve](https://github.com/kusimari/pensieve)) with claude as
`personal-memory`, bound to that store, and turns Claude's own automatic
memory off so saves go to the server. With no store set, install says so,
registers nothing, and, with claude on PATH, removes the memory-off value an
earlier install set (Claude's own memory is then on, unless another part
turned it off). Install never
creates the store or writes into it. Put an `about.md` at its root: what belongs in this store
and what does not, then a short profile. The server sends it to the agent
as standing instructions; pensieve's README has the format.

```
just memory-store <path>                                 # set your personal store (an existing checkout); then `just install`
just memory-harvest --to <dir> [--project <path-prefix>] # copy Claude's own memories (only those projects) into <dir>/memory/, leaving the originals
```

Three more groups, namespaced by what they touch:

**`resources::*`** — operate on `$HOME` or the AI tools. Every
verb reads `<action>-<resource-kind>` and takes the uniform
coding-agent selector (`claude|kiro|codex`; omit for all three):

```
just resources::install-profile            # validate content + build the checkout into the profile
just resources::install-skills [agent]     # install or update the claude/codex plugin; link kiro (and agy) at the profile
just resources::uninstall-skills [agent]   # remove the plugin and the links
just resources::status-skills [agent]      # report each plugin's version and each link
just resources::status-whole [agent]       # check every part's list against each harness
just resources::check-skills [agent]      # pre-install: verify each skill from the checkout (costs API credits, gated)
just resources::smoke-skills [agent]      # post-install: verify against the deployed tree (gated)
just resources::verify-skills [agent]     # both stages
just resources::verify-skills-one <name> [agent]   # single fixture, both stages
just resources::verify-skills-dry [name]  # construct + structurally check every prompt; free

just resources::install-browser-mcp [agent]               # register the browser-control MCP server (globally, for each agent)
just resources::uninstall-browser-mcp [agent]             # remove it (keeps your allowlist)
just resources::status-browser-mcp [agent]                # report registration state + allowlist size
just resources::browser-mcp-allow <pattern>               # append a site pattern to the allowlist
just resources::verify-browser-mcp [claude|kiro|codex] [kiro-agent]   # ATTENDED: drives real Chrome (run by hand)

just resources::set-memory-store <path>                     # what `just memory-store` wraps
just resources::status-memory                               # the store, its server on claude, and Claude's own memory
just resources::harvest-memory --to <dir> [--project <p>]   # what `just memory-harvest` wraps
```

**`kaimux::*`** — operate on the kaimux crate:

```
just kaimux::build          # release build + copy to dist/kaimux
just kaimux::test           # unit tests
just kaimux::integration    # end-to-end tmux integration test
```

**Workspace hygiene** (no namespace; operates on every
member):

```
just test         # workspace unit tests (sub-second; tempfile-fake-HOME for resources, tempdir Store for kaimux)
just fmt          # rustfmt
just fmt-check    # rustfmt --check
just lint         # clippy --workspace --all-targets -- -D warnings
just check        # cargo check --workspace
just ci           # the full hygiene gate
```

## Install

```
just install          # all agents
just install codex    # or scope to one
```

What it does:

1. Validates `resources/content/` - each `skills/<name>/SKILL.md`
   has the required frontmatter. A skill may ship deferred modules
   beside it (see below) that ride along in the same directory.
2. Builds the checkout's flake into the mAId profile as a new
   generation: the skills, plus the runtimes they need (the browser
   MCP server and its node) as nix closures. Nix reads tracked files
   only, so `git add` a new file before installing.
3. Installs the skills into each agent
   ([`resources/build-tool/src/shared.rs`](./resources/build-tool/src/shared.rs)
   lists which way):
   - claude and codex get a plugin, `maid@maid`, from a marketplace
     the profile carries (codex through `~/.local/state/maid/marketplace`,
     a directory of links into the profile). It shows in `claude
     plugin list` / `codex plugin list` and can be disabled there; a
     disabled plugin stays disabled. Its version is a hash of its
     content, so an install with changed skills updates it. Skills
     are named `maid:<name>`; the bare name still finds them.
   - kiro gets one link per skill in `~/.kiro/skills`, beside your
     own kiro skills, which it leaves alone; `~/.gemini/config/skills`
     links at the profile's skills dir.
   - Links an older mAId install left in `~/.claude/skills`,
     `~/.codex/skills` or `~/.kiro/steering/skills` are removed.

   The browser MCP is registered with the profile's launcher, outside
   the plugin.

Agents name the profile path or the marketplace dir beside it, never a
checkout or a store path, so any later install - from this checkout,
another clone, or any worktree - takes over by adding a generation.
`just rollback` returns every agent to the install before it (all of
them: the profile is shared), and again to the one before that. An install after a rollback replaces the
installs rolled back from. Installs older than 30 days are removed,
except the one before the live install. If the agents cannot take an
earlier install (one from before the plugin), rollback refuses and
changes nothing. Set `MAID_PROFILE` to use
another location.

`just verify-install` proves all of this on this machine's real install:
it installs from a clone and deletes the clone, checks what each agent
lists, starts the browser server with only `HOME` set, searches what mAId
installed for checkout paths, takes over from a second clone and rolls
back, then uninstalls (keeping your allowlist and learned rules) and
reinstalls from this checkout. It leaves no earlier install to roll back
to. `just verify-install-paid` also asks each agent what it sees.

Skill edits in a checkout reach sessions at the next `just install`
(codex reads its own copy of the plugin, so only an install updates it).

**Dependencies.** mAId does not care how these got onto PATH:

| Kind | What | Provided by |
|---|---|---|
| Runtime prerequisites | the coding-agent CLIs, `git`, a graphical Chrome (browser only) | the environment |
| Runtime-provided | the skills, node, chrome-devtools-mcp | the mAId profile |
| Install and dev prerequisites | `nix` with flakes; `direnv` optional | the developer |

`just uninstall` is idempotent. Hand-written files at a managed
destination are preserved. `just install` replaces a symlink another
mAId install left (the latest install wins); any other symlink there is
reported and kept, unless you pass `--force` to
`just resources::install-skills`.

mAId installs no global instruction file. Each supported tool loads
the skills natively, as a plugin or from its skills path, with no
extra preamble. On Claude Code the ghost's context reaches each session
through the plugin's hooks, not through a file. `AGENTS.md` is a
repo-root convention (per-project), not a global per-tool preamble;
loading a project's `AGENTS.md` / `project.md` is the `kdevkit`
skill's work-time job.

## Browser control

`just install` (or `resources::install-browser-mcp` on its own)
registers Google's
`chrome-devtools-mcp` server with the installed agent
harness(es), so the agent can drive your real, already-running
Chrome (open, navigate, fill, submit, read). It's the first
non-skill resource mAId installs; it's desktop-only and skips
gracefully where there's no graphical Chrome or no harness
CLI. Like the skills verbs, it takes the coding-agent
selector (`claude|kiro|codex`; omit for all three).

The MCP runtime is **self-contained in the mAId profile**: the
server (pinned in `flake.nix`) and its Node.js are a nix closure,
and the agents are registered with the profile's
`bin/maid-browser-mcp`. Node, npx and nix need not be on the
agent's PATH, and nothing is fetched at run time. Registering with
an agent writes to *its* config (an MCP is an out-of-process
service they call); running it stays inside the profile.

Three things to know before first use:

1. **One-time browser setup.** Enable remote debugging once in
   Chrome via `chrome://inspect/#remote-debugging`, then accept
   the permission prompt the first time the agent attaches. The
   install verb prints this reminder.
2. **Allowlist (deny-by-default).** The agent may act *only* on
   sites you allow-list — the browser enforces it. The allowlist
   is your own plain-text file (default
   `~/.config/maid/browser-allowlist`, or set
   `$MAID_BROWSER_ALLOWLIST`), one pattern per line. An empty or
   absent list refuses to start rather than exposing every
   logged-in site. Edit it directly or use
   `resources::browser-mcp-allow '<pattern>'`; changes take
   effect on the next session — Chrome is not restarted.
3. **Kiro reads it from its global MCP config.** mAId adds
   `chrome-devtools` to `~/.kiro/settings/mcp.json` and touches no
   other entry there. A kiro agent sees it when it pulls in the
   global config (kiro's default agent does). If that file already
   has a `chrome-devtools` that is not mAId's, mAId leaves it alone.

The `browser` skill teaches the agent the safe driving loop and
the attended-use safety posture. `uninstall-browser-mcp` removes
the registration but leaves your allowlist in place.

## Where to look next

- Everything that gets installed:
  [`resources/content/`](./resources/content/).
- How installation is decided: the `REGISTRY` constant at
  the top of
  [`resources/build-tool/src/shared.rs`](./resources/build-tool/src/shared.rs).
- Reference shape for a new skill:
  [`resources/content/skills/kdevkit/SKILL.md`](./resources/content/skills/kdevkit/SKILL.md)
  — also the worked example of **deferred modules**: an always-on
  `SKILL.md` plus `phases/`, `tiers/`, `setup.md`, `interviews.md`
  that it reads on demand, so the file loaded every session stays
  lean as the workflow grows
  (live siblings:
  [`notes/`](./resources/content/skills/notes/SKILL.md),
  [`writing-style/`](./resources/content/skills/writing-style/SKILL.md),
  [`kreviewkit/`](./resources/content/skills/kreviewkit/SKILL.md)).
- Full verb list: `just --list`.
