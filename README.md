# mAId

Tool-agnostic source of truth for agentic skills — compiled
into whatever AI tool happens to be in use (Claude Code, Kiro,
Codex, future tools). Skills are the only deployed artefact; each
tool discovers them natively at its own skills path.

The repo has two halves:

- **`resources/`** — three layers in one directory:
  the markdown content (`resources/content/`) the AI tools
  load; the Rust + bash tooling (`resources/build-tool/`,
  `resources/tests/run`) that installs and tests it; and
  the Justfile verbs (`resources::install-skills`,
  `resources::uninstall-skills`, `resources::status-skills`,
  `resources::verify-skills`) that drive the tooling.
- **`kaimux/`** — sibling workspace member for the kaimux
  tmux-pane orchestrator. Built via `kaimux::build`.

`just resources::install-skills` populates each target's skills
destination. For the three coding agents that means symlinks from `$HOME`
into the content tree, so edits are live for the next AI session; for the
desktop app it means a copied plugin, because that app refuses a
symlinked plugin directory.

## Develop

The repo-local flake provides `cargo` and `just`:

```
direnv allow              # loads the flake on shell entry
just                      # lists every recipe
```

Without direnv: `nix develop` once per shell (or prefix
`nix develop --command` to each command). Cargo + just are
hard prerequisites; there's no bootstrap shim.

The development methodology (spec-driven, phase-gated) is
encoded in the
[`kdevkit` skill](./resources/content/skills/kdevkit/SKILL.md).
Project context: [`specs/project.md`](./specs/project.md).
Feature specs: [`specs/feature/`](./specs/feature/).

## Verbs

Three groups, namespaced by what they touch:

**`resources::*`** — operate on `$HOME` or the AI tools. Every
verb reads `<action>-<resource-kind>` and takes the uniform
target selector (`claude|kiro|codex|desktop`; omit for all four):

```
just resources::install-skills [agent]     # validate content + populate each destination
just resources::uninstall-skills [agent]   # remove install-managed destinations
just resources::status-skills [agent]      # report current state (copied ones can be STALE)
just resources::verify-skills [agent]      # drive the coding agents against installed content (costs API credits, gated)
just resources::verify-skills-one <name> [agent]   # single fixture

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
just resources::install-skills            # all four targets
just resources::install-skills codex      # or scope to one
sudo just resources::install-skills desktop   # system path; needs elevation
```

What it does:

1. Validates `resources/content/` — each `skills/<name>/SKILL.md`
   has the required frontmatter.
2. Populates each destination per the registry at the top of
   [`resources/build-tool/src/main.rs`](./resources/build-tool/src/main.rs),
   using that row's **strategy**:
   - **symlink** — `~/.claude/skills` and `~/.kiro/steering/skills`
     (whole-dir), and `~/.codex/skills` (per-skill, since codex owns that
     directory and ships its own skills there). Edits are live.
   - **copy** — the desktop app's plugin directory, as a packaged plugin
     (manifest + skills). It refuses a symlinked plugin dir, so copy is
     the only way in. Two consequences: the install survives the checkout
     being moved or deleted, and it can go **stale** when the source is
     edited — `status-skills` reports that, and re-running install
     refreshes it.

`just resources::uninstall-skills` is idempotent. Hand-written files at a
managed destination are preserved unless you pass
`--force`.

The desktop target carries **document-shaped skills only**; terminal-shaped
ones (built around a checkout or a shell session) are excluded by design,
and install names what it skipped. See `specs/project.md` for the rule.

mAId installs **skills only**. Each supported tool discovers them
natively at its own skills path (verified: claude, kiro, codex all
load skills with no extra preamble), so mAId deploys no global
instruction file. `AGENTS.md` is a repo-root convention
(per-project), not a global per-tool preamble; loading a project's
`AGENTS.md` / `project.md` is the `kdevkit` skill's work-time job.

## Browser control

`resources::install-browser-mcp` registers Google's
`chrome-devtools-mcp` server with the installed agent
harness(es), so the agent can drive your real, already-running
Chrome (open, navigate, fill, submit, read). It's the first
non-skill resource mAId installs; it's desktop-only and skips
gracefully where there's no graphical Chrome, no `nix`, or no
harness CLI. Like the skills verbs, it takes the coding-agent
selector (`claude|kiro|codex`; omit for all three).

The MCP runtime is **self-contained in mAId**: the server runs
on Node.js, which mAId provides from its own flake (the same one
`direnv allow` loads). The launcher enters that flake on each
connection, so **Node need not be on your PATH** — only `nix`,
which the repo already requires. Registering with an agent
writes to *its* config (an MCP is an out-of-process service
they call); running it stays inside mAId's environment.
`install-browser-mcp` warms the flake so the first connection is
fast; on a cold cache after a fresh checkout that warm-up (or
the first connection) may take a while as nix builds the
devShell.

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
   `just resources::install-browser-mcp kiro <kiro-sub>`. Omit it
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
  [`resources/build-tool/src/main.rs`](./resources/build-tool/src/main.rs).
- Reference shape for a new skill:
  [`resources/content/skills/kdevkit/SKILL.md`](./resources/content/skills/kdevkit/SKILL.md)
  (live siblings:
  [`notes/`](./resources/content/skills/notes/SKILL.md),
  [`writing-style/`](./resources/content/skills/writing-style/SKILL.md)).
- Full verb list: `just --list`.
