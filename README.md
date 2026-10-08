# mAId

Tool-agnostic source of truth for agentic skills — compiled
into whatever AI tool happens to be in use (Claude Code, Kiro,
Codex, future tools). claude and codex install them as a plugin
with their own plugin commands; kiro discovers them at its own
skills path.

The repo has two halves:

- **`resources/`** — three layers in one directory:
  the markdown content (`resources/content/`) the AI tools
  load; the Rust tooling (`resources/build-tool/`) that
  validates, installs, and verifies it; and
  the Justfile verbs (`resources::install-skills`,
  `resources::uninstall-skills`, `resources::status-skills`,
  `resources::verify-skills`) that drive the tooling.
- **`kaimux/`** — sibling workspace member for the kaimux
  tmux-pane orchestrator. Built via `kaimux::build`.

`just install` builds the checkout into a nix profile
(`~/.local/state/maid/profile`) and points every coding agent at
it. Nothing installed refers back to the checkout: delete it and
the install keeps working.

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
just install [agent] [kiro-sub]     # build the profile, link the agents at it
just uninstall [agent] [kiro-sub]   # remove the links, the MCP registration, and (no agent) the profile
just status [agent] [kiro-sub]      # profile generation, links, MCP registration
just rollback                       # every agent back on the install before this one
just verify-install                 # the end-to-end install test on this machine (free)
just verify-install-paid            # the same, also asking each agent (9 model calls)
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
just resources::check-skills [agent]      # pre-install: verify each skill from the checkout (costs API credits, gated)
just resources::smoke-skills [agent]      # post-install: verify against the deployed tree (gated)
just resources::verify-skills [agent]     # both stages
just resources::verify-skills-one <name> [agent]   # single fixture, both stages
just resources::verify-skills-dry [name]  # construct + structurally check every prompt; free

just resources::install-browser-mcp [agent] [kiro-sub]     # register the browser-control MCP server (claude/codex global; kiro into the named sub-agent)
just resources::uninstall-browser-mcp [agent] [kiro-sub]   # remove it (keeps your allowlist)
just resources::status-browser-mcp [agent] [kiro-sub]      # report registration state + allowlist size
just resources::browser-mcp-allow <pattern>               # append a site pattern to the allowlist
just resources::verify-browser-mcp [claude|kiro|codex] [kiro-sub]   # ATTENDED: drives real Chrome (run by hand)
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
     the profile carries, through `~/.local/state/maid/marketplace`
     (a directory of links into the profile). It shows in `claude
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
extra preamble. `AGENTS.md` is a
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
3. **Kiro is per-agent.** Claude and codex expose a registered
   server to every session, so no agent is named. Kiro partitions
   MCP servers per agent and `kiro-cli chat` runs a specific agent
   — so name the sub-agent to register into:
   `just install kiro <kiro-sub>`. Omit it
   and kiro is skipped (claude and codex still install). mAId
   never guesses which of your agents to write into. Use the
   *same* sub-agent name when testing:
   `… verify-browser-mcp kiro <kiro-sub>`.

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
