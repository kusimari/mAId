# How the ghost works: harness-core in pictures

The system design of the core harness, kept alongside `harness-core.md`
for future reference. It shows the system from three sides (a session,
the install, the build) and records each design decision with the
options weighed and the one chosen. project.md's "My harness" points
here for the pictures.

## The pieces, and why there are this many Rust files

There are three programs. Only the first is new.

| Program | Where | What it is | Runs when |
|---|---|---|---|
| `maid-core` | `resources/core` | **the ghost.** The five functions plus the readers they need | in every session: Claude Code's hooks, its MCP server and the reflect skill call it |
| `build-tool` | `resources/build-tool` | **the installer.** It existed before; my-harness and this initiative extended it | when you run `just install`, `just status` or `just uninstall`, from the checkout |
| `pensieve` | its own repo, pinned in `flake.nix` | **the memory server** | in every session, as an MCP server |

`maid-core` is about 6,300 lines without tests, in 24 modules. Each
module is one concept from the spec's "Technical concepts" table, so the
file list reads as the design:

```
 READ WHAT IS INSTALLED              DECIDE WHAT THIS SESSION GETS        LEARN FROM THE SESSION
 ─────────────────────               ──────────────────────────           ──────────────────────
 config.rs     where things are      entry.rs   the five entry points     sleep.rs      the "done" mark
 parts.rs      read part lists                 + rendering (923)       source.rs     where sessions live
 contract.rs   read each bring-in's  delivery.rs  "sent once" record       claude.rs     pack Claude's transcript (467)
               declaration (363)     here.rs    where am I (folder,repo)  experience.rs said/told/did/touched/friction
 map.rs        fuse every part       fit.rs     which bring-ins fit       adapt.rs      guards, batch, decide (1,423)
 environment.rs ready or not, why    select.rs  which memories to recall adapt/moves.rs saved-in-memory, the move item
                                     record.rs  the work's own record     memory.rs     talk to the memory server
                                     mcp.rs     the on-demand MCP server  ledger.rs     counts across sessions
                                                                          files.rs, time.rs  small helpers
 main.rs  the CLI that hooks, the skill and you call · lib.rs  module list
```

The three big files are where the risk is:
- **`adapt.rs`:** every one of your guards is code here, not prose.
- **`entry.rs`:** every carrier's output is shaped here.
- **`claude.rs`:** Claude Code's transcript format is undocumented, so the packer is lenient.

Tests are about as large again: unit tests inside each file, plus
`resources/core/tests/` (adaptation, live Claude Code, hooks, MCP, CLI,
paid).

`build-tool` grew by about 1,900 lines in this initiative, in five places:
- install records persona and bring-ins;
- the Claude Code marketplace path;
- `CLAUDE_CONFIG_DIR`;
- whole-status shows bring-in readiness;
- binding a skill's config.

Most of its size is from earlier work, and it installs for codex and kiro
too.

## 1. A session: what happens when you start and talk

Claude Code is not wrapped. It calls the ghost through four of its own
hooks and one MCP server. Every call runs `maid-core`, which reads what
is installed fresh each time and answers.

```
 you                 Claude Code (the shell)                 maid-core (the ghost)                     on disk
 ───                 ───────────────────────                 ─────────────────────                     ───────
 launch claude ──►   SessionStart hook ────────────────────► genome
                                                               read part lists ──────────────────────► parts dir
                                                               read each bring-in's declaration ──────► profile (skills, server.json)
                                                               fuse into the harness map
                                                               check each is ready (paths, commands)
                                                               "already sent this session?" ─────────► delivery record
                     ◄── startup picture: personas, acting persona,
                         memory + store, bring-ins, ready, pending
 (first reply already knows who you are)

 type a prompt ──►   UserPromptSubmit hook ────────────────► expression
                                                               where am I? (folder, repo)
                                                               which turn bring-ins fit? (signals)
                                                               their first rules (≤30 lines, ≤3 fit)
                                                               recall: memories that bear on it (≤3) ► memory store index
                                                               the work's record (kdevkit Handoff)
                                                               drop what was already sent ──────────► delivery record
                     ◄── only what changed (often nothing)

 Claude works,        (pensieve, the memory MCP server, is called by the model directly:
 maybe saves a        a correction or a "new rule:" is saved to your store during the session)
 memory ─────────────────────────────────────────────────────────────────────────────────────────────► memory store

 a sub-agent starts  SubagentStart hook ───────────────────► genome, sub-session slice
 /clear or compact   SessionStart (clear|compact) ─────────► genome --reset (placed again)

 "close this         reflect skill:
  session" ──►         1 sleep ─────────────────────────────► mark the session done ────────────────► state/sleep
                       2 adaptation (brief) ────────────────► pack the transcript ◄──────────────────── Claude's transcript
                                                               keep the briefed copy ─────────────────► state/briefed
                       3 maid:reflector sub-agent             (read-only: Read, Grep, Glob)
                         returns findings as JSON
                       4 adaptation (findings) ─────────────► guards: quotes in your own words?
                                                               already saved? borrowed? persona?
                                                               "saved in memory" lines, move item
                                                               one batch ───────────────────────────► state/pending
                     ◄── the batch
 "ok" ──────────►      5 adaptation (decide) ───────────────► re-check, apply:
                                                               habit  → memory server's own write ──► memory store
                                                               nature → a proposal file ────────────► state/proposals
                                                               borrowed → a note for its lender
                                                               count it ────────────────────────────► state/ledger
 /exit ─────────►    SessionEnd hook ──────────────────────► sleep (mark only, if not closed)
```

The same answers are available on demand. The `my-harness` MCP server
(`maid-core mcp`) serves the genome as a resource and prompt and
expression as a tool, and the CLI (`maid-core genome`) gives them too.
Both go through the same entry points the hooks use.

## 2. Install: how the ghost gets onto Claude Code

`just install` does two things:
1. builds an immutable **profile**, a nix store path;
2. points each agent at it through that agent's own commands.

Nothing installed names your checkout, so you can delete the checkout
afterwards.

```
 your checkout                     nix profile (immutable, one per generation)        Claude Code's own config
 ─────────────                     ───────────────────────────────────────────         ────────────────────────
 just install
  │
  ├─ 1 validate content (every declaration reads, every rules pointer resolves)
  │
  ├─ 2 nix build ────────────────► ~/.local/state/maid/profile ─► /nix/store/…-maid
  │                                  bin/maid-core        the ghost
  │                                  bin/pensieve         the memory server
  │                                  bin/maid-browser-mcp
  │                                  share/maid/
  │                                    marketplace/plugins/maid/
  │                                      skills/            kdevkit, kyodakit, kreviewkit, writing-style,
  │                                                         notes, browser, reflect (each with my-harness metadata)
  │                                      agents/reflector.md
  │                                      hooks/claude.json  the four hooks ─┐  "@maid-core@" replaced with
  │                                      claude-mcp.json    the MCP server ─┘  this build's bin/maid-core
  │                                      .claude-plugin/plugin.json (+ extensions."my-harness": the adapter)
  │                                    servers/           personal-memory, browser server.json (declarations)
  │                                    overlays/          document-skills (declaration for a borrowed unit)
  │
  ├─ 3 build-tool install --agent claude   (every `claude` call gets CLAUDE_CONFIG_DIR exactly as resolved)
  │     claude plugin marketplace add <profile>/share/maid/marketplace ─────────────► settings.json, plugins/
  │     claude plugin install maid@maid  (hooks, MCP server, skills, agent) ─────────► plugins/
  │     vendor marketplace + document skills (borrowed) ─────────────────────────────► plugins/
  │     claude mcp add personal-memory <profile>/bin/pensieve --store <your store> ──► .claude.json
  │     env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1   (Claude's own memory off) ───────────► settings.json
  │     permissions: read the plugin dir; edit writing-style's config dir ───────────► settings.json
  │
  └─ 4 record in mAId's part list ──────────────────────────────────────────────────► ~/.local/state/my-harness/parts/maid-harness.json
        persona: personal
        bring_ins: each with unit (its manifest), source + rev, tenure (own|borrowed),
                   route (write|backlog|worktree|report), bound values (store, index, serve, ...)
        harnesses.claude: what was placed (so uninstall removes exactly that)
```

**Another part** (your work repo) installs its own bring-ins and writes
its own `<part>.json` beside mAId's. It never touches mAId's carriers.
`maid-core` reads every part list on every call, so the next session's
genome includes it with nothing re-installed. A part whose bring-ins
carry no declaration shows them as *undeclared*. They still work as plain
skills and servers; they just don't fit turns or take part in adaptation.

**Rollback** moves the profile link to the previous generation. Claude
Code holds the profile path as typed, so it follows the link with no
re-registration.

## 3. The build: what each command builds (as first built; D3 changes it)

There are three kinds of build. People mean different ones when they say
"the build".

```
 just ci  (development checks, from the checkout, no model calls)
   cargo fmt --check · cargo clippy -D warnings · cargo test (workspace)
     ├─ maid-core: unit tests + tests/ (adaptation drives the real pensieve; cli; hooks; mcp; shipped_fit)
     └─ build-tool: unit tests + tests/ (conformance cases, install/status with stub CLIs, config dir)

 nix build .#default  (the profile, what `just install` installs)
   ├─ maid-core      buildRustPackage from Cargo.lock  → bin/maid-core
   ├─ pensieve       its flake at the pinned commit    → bin/pensieve
   ├─ browser MCP    pinned chrome-devtools-mcp        → bin/maid-browser-mcp
   └─ the plugin     copy resources/content, write the claude/codex manifests,
                     replace @maid-core@ in hooks, MCP config and the reflect skill,
                     version = hash of the plugin's content (agents cache plugins by version)
   The profile changes only when its content does: a docs-only commit gives the same store path.

 cargo run -p build-tool  (the installer, run from the checkout through just; never installed)
   validate · install · uninstall · status --whole · record · memory · harvest
   uses maid-core as a library, so status and sessions judge readiness the same way

 ignored tests  (run by hand; `--ignored`)
   claude_live, claude_installed     real Claude Code against a loopback stub of the model API (free)
   claude_paid                       real Claude Code and a real model (paid; refuses to write under HOME)
   resources/tests/rollback          full install / rollback / uninstall in a temp HOME (free)
```

## Design decisions

### D1 · Who makes the judgement on a prompt (expression)

The prompt hook decides which turn bring-ins fit, what of them to repeat,
and which memories bear on the work. Built first as Rust alone (declared
signals scored, word overlap for recall), which is deterministic but blind
to meaning: Macro run 5 missed a saved habit on the first turn of a shell
question.

| Option | What it means | Trade |
|---|---|---|
| A. Rust alone (as first built) | signals and word overlap decide | cheap, predictable; blind to meaning |
| **B. Rust gathers, the agent judges (chosen)** | Rust keeps what is deterministic (location and file signals, caps, timing) and widens the net where meaning matters: the prefix offers short candidates ("may bear on this: no-awk-in-scripts (shell habit)") and the model already in the session decides what to open | no extra model call, inside the hook's time limit; a few prefix lines; relies on the agent reading them |
| C. A local model in the hook | the `Selector` interface already allows it | real judgement, no outside call; latency every prompt, a model to install |
| D. A hosted model from the hook | | good judgement; money and latency every prompt |

What to repeat stays the bring-in author's choice: the `rules` section it
declares. The agent may load the whole bring-in on its own; the prefix
carries only the essentials.

### D2 · How often the prefix goes

The prefix is a prompt context in front of the prompt: it reinforces the
essentials so they are not lost, whatever the agent loaded earlier.

| Option | Trade |
|---|---|
| a. Once a session, then on change (as first built) | smallest context; rules from turn 3 drift out of reach by turn 40 |
| b. Every prompt | matches the prefix model; the history fills with copies |
| **c. On change, and again once it has drifted (chosen)** | a short form of the essentials goes again when the fit changes, after a number of turns, or when the last copy is far back; Rust decides this from the delivery record |

### D3 · Build, install and wiring are three layers

Chosen by the user: everything mAId builds is cargo and Rust; nix is how
this environment is managed, not how mAId is built.

| Layer | Owner | What it does |
|---|---|---|
| **build** | `cargo build` (with a Rust step for the non-binary artefacts) | processes `resources/content` and the crates into one staged tree under `target/`: `bin/maid-core`, the plugin (skills, agent, hooks and MCP config with the core's path filled in, manifests, content-hash version), server declarations, overlays |
| **install** | a Rust install step, `PREFIX`-style | copies the staged tree into a top-level directory with the same layout; `@maid-core@` resolves to `<prefix>/bin/maid-core` |
| **wire** | build-tool | points each agent at the installed directory through its own commands (plugin marketplace, MCP registration, settings) and records the part list |
| nix, one way to run the layers | the flake | its derivation runs build and install with `PREFIX=$out`, so the directory is a store path with generations and rollback; then wiring names the profile link |

A non-nix installer runs the same build and install with its own prefix,
then the same wiring. This is the usual shape of robust projects: the
build system knows nothing of packaging, install is a `PREFIX`/`DESTDIR`
copy into a standard layout, and packagers (nix, distro packages,
Homebrew) wrap those two steps. In Rust, non-binary artefacts are
staged by a Rust step in the workspace (the "xtask" pattern) rather than
by the packager. Third-party runtimes (pensieve, the browser MCP's node
package) stay pinned dependencies the installer resolves; nix pins them
today.

## What to judge in review

- **Is `maid-core` the right size?** 24 modules map one to one to the
  spec's concepts. `entry.rs` (rendering for every carrier) and `adapt.rs`
  (every guard) could each split later. The cost now is two long files to
  read.
- **One core, many carriers.** Hooks, the MCP server, the skill and the CLI
  all reach the same functions. That is how hooked and on-demand use
  never drift apart.
- **Install is placement, not wrapping.** Everything goes through Claude
  Code's own plugin, MCP and settings commands, and everything is
  recorded, so uninstall and other parts know what is mAId's.
- **The map is derived every call.** No stored state can go stale when a
  part installs or leaves. The only state the ghost keeps is what it sent,
  what is pending, and what it learned.
