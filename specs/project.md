# Project: mAId

## Mission

<!-- What this project exists to do, and who it serves. One
     paragraph. Change only when the goal itself changes. -->

Tool-agnostic source of truth for my agentic resources — mostly
skills — compiled into whatever AI tool I'm using (Claude Code,
Kiro, Codex, future tools). The repo is the checked-in source;
`just resources::install-skills` creates the `$HOME`-facing symlinks
each tool reads from. Every supported tool discovers skills natively at
its own skills path, so skills install as plain symlinks with no
global instruction preamble; the one non-skill resource
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
  2. **Tooling** (`resources/build-tool/`) — Rust crate
     (single-file) that does the install. Validates content
     and creates/removes/reports the `$HOME`-facing
     symlinks. Plus a small bash script
     (`resources/tests/run`) that drives the AI tools
     against the installed content. Rust where types help
     (the symlink state machine and content validator);
     bash where shelling out to other tools is the job
     (driving `claude` / `kiro-cli` / `codex`).
  3. **Verbs** (Justfile recipes that use the tooling) —
     `just resources::install-skills`, `…::uninstall-skills`,
     `…::status-skills`, `…::verify-skills`
     (single-fixture: `just resources::verify-skills-one <name>`).
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
`resources/build-tool/src/main.rs` (a slice of
`(home_subpath, source_subpath, kind, agent)` tuples). The
authoritative manifest for what gets installed where. The
`agent` tag is what lets install/uninstall/status be scoped
to one coding agent (`--agent`, surfaced as the Just verbs'
selector) or, by default, cover them all. Tool adaptation
lives here: adding a new coding-agent tool = adding its
expected `$HOME` paths as registry entries, not rewriting
content. Content stays tool-agnostic; the registry translates
it into each tool's expected layout.

Skills deploy through the registry as symlinks — each supported tool
discovers them natively at its own skills path (`~/.claude/skills`,
`~/.kiro/steering/skills`, `~/.codex/skills`), verified to load with
no extra preamble. mAId installs no global instruction file:
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
  *registration* goes in each harness config (`claude mcp
  add` / `kiro-cli mcp add`); its *runtime* (Node) is mAId's
  own, supplied from the repo flake. This is why the registry
  above can't express it — registration is a runnable command,
  not a symlink — so it lives in `resources::browser-mcp-*`
  Just verbs (shell over each harness's MCP CLI), keeping the
  Rust build-tool pure-symlink.
- **Browser-enforced allowlist.** The agent may act only on
  sites in a user-owned allowlist file
  (`${XDG_CONFIG_HOME:-$HOME/.config}/maid/browser-allowlist`).
  Enforcement is at the browser (a launch flag), not in skill
  prose — a prompt-injected agent still can't leave the list.
  Deny-by-default: an empty/absent list refuses to start. A
  thin launcher (`resources/browser/launch`) re-reads the file
  and enters the flake on each (re)connect, so edits apply next
  session without restarting Chrome.

## Tech Stack

<!-- Languages, runtimes, frameworks, key libraries. Versions
     only where version matters. -->

- **Runtime:** Rust (cargo workspace) + Just for the verb
  surface. `resources/build-tool` is today's only
  workspace member; future Rust crates (e.g. `kaimux/`)
  join as additional members.
- **Isolation:** `flake.nix` + `.envrc` load the rust
  toolchain + `just` via direnv (rust-overlay). Cargo and
  just are hard prerequisites — no `./install` shim. Users
  on machines without them enter `nix develop` themselves.
  The flake also bundles `nodejs_22` — the runtime the
  browser-control MCP server (`chrome-devtools-mcp`, run via
  `npx`) needs — so that capability is self-contained in mAId:
  `nix` is its only host prerequisite, node need not be on the
  user's PATH.
- **Entrypoints:** Justfile organised as a root file with
  `mod` declarations per area, so verbs are namespaced by
  what they touch:
  - **`resources::*`** (operate on `$HOME` or the AI tools).
    Verbs follow the `<action>-<resource-kind>` pattern, each
    taking the coding-agent selector (`claude|kiro|codex`; omit
    for all three): `just resources::install-skills [agent]`,
    `…::uninstall-skills [agent]`, `…::status-skills [agent]`,
    `…::verify-skills [agent]` (drives the agents against
    installed content; costs API credits, gated behind a
    confirmation prompt), `…::verify-skills-one <name> [agent]`,
    `…::verify-skills-kind <kind> [agent]` (one kind of skill
    test across every skill — see Testing), `…::verify-skills-dry
    [name]` (construct and structurally check every prompt
    without calling an agent; free).
    Browser-control MCP adds
    `install-browser-mcp [agent] [kiro-sub]` /
    `uninstall-browser-mcp` / `status-browser-mcp` /
    `browser-mcp-allow <pattern>` (register/report the server
    + append allowlist patterns), plus `verify-browser-mcp`
    (attended, `[confirm]`-gated — drives real Chrome).
  - **`kaimux::*`** (operate on the kaimux crate):
    `just kaimux::build` (release + copy to `dist/`),
    `just kaimux::test`, `just kaimux::integration`.
  - **Workspace hygiene** at the root (no namespace —
    operates on every member): `just test`, `just fmt`,
    `just fmt-check`, `just lint`, `just check`,
    `just ci` (full gate).
  Each recipe is a one-liner over native cargo or a bash
  fixture-runner — `just --list` shows the root verbs,
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
├── flake.nix / .envrc      repo-local rust toolchain + just (direnv + rust-overlay)
├── resources/
│   ├── Justfile            `resources::*` verb surface (install/uninstall/status/verify)
│   ├── build-tool/         single-file Rust crate (install/uninstall/status)
│   │   ├── Cargo.toml      deps: clap, anyhow; dev: tempfile
│   │   └── src/main.rs     registry + content checks + symlink core + clap + tests
│   ├── content/            the deployable skills (symlinked in)
│   │   └── skills/<name>/SKILL.md   (incl. browser/ — browser-control safety posture)
│   ├── browser/            browser-control MCP (not symlinked — runnable)
│   │   ├── launch          allowlist-enforcing launcher; enters flake, execs chrome-devtools-mcp
│   │   └── manage          data-driven MCP registrar (MCP_AGENTS table: claude/codex global, kiro per-sub-agent)
│   └── tests/              bash fixture-runner (drives claude / kiro / codex against installed content)
│       ├── run             entrypoint (`just resources::verify-skills` calls this)
│       ├── browser-functional   ATTENDED test: drives real Chrome, asserts off-list blocked
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
Load-bearing — this is the §8 Test Gate default. Includes
a structural integration test (`structural_install_to_real_directory_layout`)
that runs a full install→status→uninstall round-trip in
the fake $HOME, replacing the older bash structural smoke.

**`just resources::verify-skills` — AI-tool functional tests.** Drives
the real coding agents (claude, kiro, codex) against the `.smoke`
fixtures under `resources/tests/skills/`. Three verification styles
share the harness: **substring** (the reply contains a string),
**semantic** (a judge call checks the reply's meaning), and
**behavioral** (`--- setup ---` / `--- assert ---` shell blocks — the
agent runs against a seeded test project and the assert inspects the
changes it made). Every fixture runs against each requested agent; the
verb's agent selector (surfaced to the runner as `--tools <list>`)
scopes to one, default all three, all required. Slow (minutes), costs
API credits, requires the managed symlinks already deployed (run
`just resources::install-skills` first). Gated behind a confirmation
prompt in the Justfile.

### The five kinds of skill test

A verification *style* (above) is how a test checks. A **kind** is what
question it answers. The two are orthogonal, and conflating them is how
the suite once drifted into a dozen fixtures that all answered the same
question. Kinds are two axes composed:

- **How the skill is reached.** *Explicit* — the prompt names the skill
  and the path it lives at for the agent under test, which isolates
  content: a failure means the skill is wrong, not that it failed to
  load. *Implicit* — the prompt states only the task, so the agent must
  recognise it and load the right skill unaided.
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
`kreviewkit`) can be
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
contract from the installed `SKILL.md` rather than assuming it; a
skill without one skips activation/discovery and is proven to fire by
its `enact` / `integration` artefacts instead.

The pair is diagnostic. When a skill's explicit test passes and its
implicit counterpart fails, the fault is triggering, not content — no
bisection needed. `activation` and `discovery` are **generated** by the
runner from a fixture's `skill:` field, so no fixture authors them and
none writes a skill path (the per-agent paths live only in the runner's
`skill_path`, mirroring build-tool's `REGISTRY`; hand-copying them into
prompts is what let them drift out of sync per fixture).

**What a new skill is expected to carry.** At minimum one `enact`
section per load-bearing behavior — which also produces its
`integration` run — plus one `playback` section per guardrail or
absence path that produces no artefact (a refusal, a stop-with-error, a
safety posture). `activation` and `discovery` come free. Prefer the
behavioral form for `enact` wherever the skill's correct action leaves
an inspectable change; fall back to a judge narrative only when the
output is irreducibly prose.

**Triggers belong in `description:`, never only in the body.** A skill's
`name` and `description` are the only fields an agent reads to decide
whether the skill applies; the body loads *after* it triggers. So a
"when to use this" section in the body cannot cause the skill to fire —
it is only ever read by a skill that already fired. Lead the description
with what the *user* asks for ("Use when… Typical triggers include
'ship it', 'close it out'…"), then describe the machinery. This is not
style preference: `kdevkit` kept its phase cues at lines 304/754/1004 of
a 1120-line body and silently failed to self-trigger on codex — it did
plain git work and never loaded — while `notes`, `writing-style`, and
`browser`, which all lead with the user's action, triggered fine against
60+ competing installed skills. A `discovery` failure for a skill that
loads correctly when handed its path is almost always this.

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

`resources/tests/run --dry-run` constructs every prompt and checks it
structurally **without calling an agent** — explicit prompts must carry
that agent's own skill path, implicit prompts must leak no skill name,
path, or marker, and a malformed fixture fails before any credits are
spent. Run it before any paid run. `--kind <list>` scopes a run to one
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

The §8 Test Gate uses `just test` by default. SKILL.md
prose revisions add `just resources::verify-skills` (judge mode)
as their A/B evidence. The §9 close-out can run
`just resources::status-skills` after an install to confirm
symlinks resolved.

### Functional tests are user-driven

Agentic runs (an AI assistant working through this project)
**must** stop at `just test`. `just resources::verify-skills` costs
API credits and takes minutes; whether to spend that budget
on a given change is a human call. The agent prepares the
fixture, names the exact command, and hands off — it does
not run it. The Justfile's `[confirm]` gate on `verify-skills`
provides a second line of defense.

Commands the user runs by hand:

- All fixtures: `just resources::verify-skills`
- A single fixture: `just resources::verify-skills-one <name>`
  (e.g. `just resources::verify-skills-one notes-git-commit`).

The fixture file's basename (without `.smoke`) is the
`<name>`.

Quality gate: `just fmt-check` + `just lint` + `just check`
(or the bundled `just ci`). Run after any implementation
slice.

## Deployment

<!-- How code reaches users. Build, release, install, symlink,
     container, whatever applies. If the project isn't deployed
     in a traditional sense, describe how it's consumed. -->

Not a service — consumed locally.
`just resources::install-skills` validates content and creates the
`$HOME`-facing symlinks; `just resources::uninstall-skills`
reverses them. `just resources::status-skills` reports current
managed-symlink state. `just resources::verify-skills` drives the
real AI tools against the installed content. Each takes an
optional coding-agent selector (`claude|kiro|codex`; omit for all
three). App workspace members (`kaimux/`) build via
`just kaimux::build` (a one-liner over `cargo build -p kaimux
--release` + copy into `dist/`).

The browser-control MCP deploys separately from skills
(env-gated, opt-in): `just resources::install-browser-mcp
[agent] [kiro-sub]` registers the server with each harness
(claude/codex global; kiro per named sub-agent) and prints the
one-time manual step (enable Chrome remote debugging via
`chrome://inspect`). It graceful-skips where a prereq is missing
(no GUI Chrome, no `nix`, no harness CLI) — never a
half-register. `uninstall-browser-mcp` removes the registration
but preserves the user-owned allowlist (it's user data). Skills
and the installable stay separate verbs (skills always safe;
the installable env-gated) but share the uniform
`<action>-<resource-kind>` naming and coding-agent selector, so
the experience is symmetric across resource kinds.

### Hard constraints

- **Never write into `~/.claude/skills/`, `~/.kiro/steering/skills/`,
  `~/.codex/skills/`, or any registry destination directly.** These
  paths are symlinks back into the checkout; a non-symlink file there
  breaks deploy invariants. Edit the source under
  `resources/content/` instead — the symlink exposes changes
  live. (This guardrail is mAId-project context — it protects mAId's
  own deploy invariant — which is why it lives here, not in a
  globally-installed preamble.)
- **Registry is the single source of truth** for deployment.
  Adding a new managed path = a registry change + CR, never an
  ad-hoc edit.
- **No global state mutation** on install. The rust toolchain
  and `just` come from the repo-local flake; `build-tool` is
  invoked through `cargo run -p build-tool` (wrapped by Just)
  from the checkout — no shim under `~/.local/bin`, no
  `cargo install` anywhere in the install path.
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
  - `reviewer: host-native` — use the host coding agent's built-in
    code review (Claude Code's `/code-review` skill, Kiro's
    equivalent). No project-specific reviewer skill yet; revisit
    once host-native review proves consistently weak across
    feature work in this repo.
- `review_brief:`
  - `enabled: true` — fire kdevkit's §7 Review Briefing at the
    dev → closure hand-off, so a human gets a briefing (what
    shipped, spec-vs-diff reconciliation, where to focus, what
    needs their judgement) as the PR body before giving the
    closure cue. Complements `code_review:` rather than
    replacing it: that gate serves the agent (blind,
    diff-vs-project, scored), this one serves the human
    (spec-aware, diff-vs-spec, prose).
  - `reviewer:` omitted — resolve the single installed tool
    advertising the review-briefing role. kdevkit dispatches the
    *role*, never a named product; this repo happens to ship one
    (`resources/content/skills/kreviewkit/`), and dogfoods it.
