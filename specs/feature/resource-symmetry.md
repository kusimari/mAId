# Feature: resource-symmetry

## Git Setup

- Branch: `feat/resource-symmetry`
- Base: `main`

## Feature Brief

mAId installs and tests two kinds of AI resource — pure
**skills** (markdown) and **install+skill** resources (a skill
plus a runnable install step; `browser` is the only one today).
Across the three supported coding agents (claude, kiro, codex)
the two kinds are handled **unevenly**: skills deploy uniformly
through a data-driven registry that already covers all three
agents, but the browser MCP install and the browser functional
test cover only claude and kiro — codex is missing — and most
skill fixtures test only claude and kiro.

This feature makes every resource **symmetric across all three
agents**. After it lands: one `just resources::install` deploys
both halves of every resource (skills *and* MCP registrations)
for every agent that supports them; codex is a first-class
target for MCP registration and for every skill fixture; and
the MCP install layer is **data-driven per agent** in the same
shape as the skills registry, so adding the next agent (or the
next install+skill resource) is a table edit, not a new
hand-written function.

## Requirements

<!-- The experience layer — what the user touches and observes.
     Library/flag/path/script names live in Design. -->

### Unified install experience

- `just resources::install [kiro-agent]` deploys **both halves
  of every resource** in one command: the skills (symlinked for
  all three agents) and every install+skill resource's runnable
  install (e.g. the browser MCP registration) for each agent
  that supports it.
- Env-gated parts **graceful-skip**: on a machine with no
  graphical Chrome, no `nix`, or a missing agent CLI, the
  install prints a one-line pointer to what's missing and makes
  no changes for that part — it never half-installs or errors
  out, and the skills half still completes.
- `just resources::uninstall [kiro-agent]` reverses **both
  halves** and **preserves the user-owned allowlist** (it is
  user data, not install state).
- Both verbs stay **idempotent**: re-running install is a no-op
  that reports "ok"; re-running uninstall reports
  "not-installed".

### Codex parity

- The browser MCP registers with **codex** exactly as it does
  with claude — globally, visible to every codex session — so
  after install, codex has the browser tools.
- `just resources::verify` drives **codex against every skill
  fixture**, not just the kdevkit ones, so a skill that behaves
  correctly is one that behaves correctly on all three agents.
- `just resources::browser-functional-test codex` drives codex
  through the real browser MCP, asserting an allow-listed site
  loads and an off-list site is blocked — the same attended
  check claude and kiro already have.

### Kiro's per-agent rule is honored by the unified entry

- Because kiro partitions MCP servers per agent and mAId never
  guesses which agent to write into, the unified install
  registers the browser MCP for claude and codex (global) and
  **skips kiro's MCP with an actionable message** unless a kiro
  agent name is passed: `just resources::install <kiro-agent>`.
  The skills half is unaffected — skills always install for all
  three agents.

### Granular verbs remain

- The existing `browser-mcp-install` / `-uninstall` / `-status`
  / `-allow` verbs stay, for re-registering (or inspecting)
  just the browser MCP without a full symlink pass. Their
  observable behavior is unchanged except that they now cover
  codex too.

## Test Strategy

<!-- Success criteria mapped onto project.md's two test layers:
     `just test` (build-tool unit, tempfile-fake-HOME) and
     `just verify` (AI-tool functional .smoke fixtures). -->

### Build-tool unit (`just test`)

- The Rust build-tool is **unchanged in logic** — it stays
  pure-symlink and the existing suite (including the
  `Kind::FanOut` codex coverage) is the regression guard that
  the symlink half is untouched.
- **One new test** asserts manifest ↔ skills-dir consistency:
  every `resources/content/skills/<name>` directory has a row
  in `resources/manifest` and every manifest row names a real
  skill dir. This is the only coupling between the new manifest
  and the checkout, so it is worth a cheap guard against drift.

### Functional smoke (`just resources::verify`, user-run)

- Every `.smoke` fixture runs against **all three agents**
  (`tools: claude,kiro,codex`). The runner already supports
  codex; the change is per-fixture `tools:` fields. This is the
  "verify across every agent we ship" success criterion.
- Risk accepted (see Decision Log): a skill whose prose was
  tuned for claude/kiro may not yet drive codex to the same
  result. Those surface only in the user-run, API-costed verify
  — never in `just ci` — and are content-fix follow-ups, not
  blockers for this feature.

### Desktop-manual (attended, user-run)

- `just resources::browser-functional-test codex` (and the
  existing claude/kiro forms) drives the real browser MCP
  against an isolated temp allowlist, asserting an allow-listed
  site loads and an off-list site is **blocked**. Codex is
  global, so no agent name is needed. `[confirm]`-gated and
  separate from the bulk `.smoke` runner, as today.

## Design

<!-- Rationale first. -->

### Why this shape

The asymmetry has a single root cause: **skills are
data-driven, MCP install is hand-written**. Skills deploy
through the `REGISTRY` constant in
`resources/build-tool/src/main.rs` — one row per (agent-home,
source, kind), so all three agents are handled by the same
loop and adding an agent is a row. The browser MCP install, by
contrast, lives in `resources/browser/manage` as hand-written
`register_claude` / `register_kiro` functions (plus
unregister/status variants); there is simply no codex function,
and the divergent-per-agent shape is what let codex be omitted
silently.

The fix mirrors the registry's data-driven shape in the shell
layer, and adds a small declarative manifest so a single
orchestrator can deploy every resource's both halves.

**The Rust build-tool stays pure-symlink.** This is a standing
hard constraint (see `project.md` Hard constraints and the
browser-mcp feature): MCP registration is a *runnable command*
a symlink can't express. So symmetry is achieved entirely in
the shell/Just layer — the manifest, the table-driven `manage`,
and a thin orchestrator — never by folding MCP into Rust.

**The manifest does not duplicate the registry.** The registry
maps *content-dir → agent-home* and symlinks the whole skills
dir (claude, kiro) or fans out per child (codex); it never
enumerates individual skills. The manifest enumerates
*individual resources* and marks which additionally carry an
install step. These are orthogonal axes — "where does each
agent read skills from" vs. "which resources need a runnable
install beyond the symlink" — so the manifest carries **no
agent-home column**. Agent homes stay solely in the registry;
adding a fourth agent touches the registry (symlink homes) and
the `manage` per-agent table (MCP), never the manifest.

**Unifying install is safe because registration ≠ activation.**
Folding the browser MCP registration into `resources::install`
does not make the always-works skills install suddenly
dangerous: each install hook graceful-skips on missing
prerequisites, and registering the MCP server is harmless
without an allowlist — the launcher is **deny-by-default** and
refuses to start until the user adds a site. So a plain
`resources::install` on a headless box lays the symlinks and
cleanly skips the MCP step. This realizes the convergence the
browser-mcp feature named as the intended direction
("`resources::install` can orchestrate every resource kind").

### Components

1. **Manifest** — `resources/manifest`. Plain-text,
   whitespace-delimited, `#`-commented. One row per resource:

   ```
   # name           kind           install         mcp_server       launcher
   notes            skill          -               -                -
   writing-style    skill          -               -                -
   kdevkit          skill          -               -                -
   browser          install+skill  browser/manage  chrome-devtools  browser/launch
   ```

   `kind` is `skill` (the Rust symlinker fully handles it — the
   row exists for completeness and the consistency check) or
   `install+skill` (also run its `install` hook). The
   `install` / `mcp_server` / `launcher` columns are the data
   the hook needs; lifting `chrome-devtools` and the launcher
   path here removes the last hardcoded browser literals from
   `manage`, so `manage` becomes a generic MCP registrar
   parameterized by the manifest.

2. **Table-driven MCP registrar** — refactor
   `resources/browser/manage`. Replace the hand-written
   per-agent functions with a per-agent **data table**
   mirroring the registry:

   ```
   # agent   cli        scope      style
   #   scope: global (claude, codex — one server, every session)
   #          per-agent (kiro — needs an explicit agent name)
   #   style: readd   (claude — `mcp add` errors if the name exists)
   #          replace (kiro, codex — add is add-or-replace)
   MCP_AGENTS=(
     "claude  claude    global     readd"
     "kiro    kiro-cli  per-agent  replace"
     "codex   codex     global     replace"
   )
   ```

   A generic `mcp_foreach <op> <server> <launcher> [kiro-agent]`
   holds all shared logic once — CLI-on-PATH check + graceful
   skip, per-agent skip when unnamed, message formatting. Three
   thin `case "$agent"` dispatchers (`mcp_add` / `mcp_remove` /
   `mcp_get`) issue only the irreducible CLI differences:
   claude's `-s user` + remove-then-add, kiro's
   `--name/--agent/--command` + stderr-with-ANSI `mcp list`
   status parse, codex's global idempotent `mcp add`/`remove`/
   `get`. This is strictly less hand-written per-agent code than
   today (one generic loop + three small dispatchers vs. six
   full functions), and codex joins as a global agent beside
   claude.

3. **Unified orchestrator** — `resources/install` (new,
   POSIX-ish bash), invoked by the `resources::install` verb.
   It (a) runs the Rust build-tool for the skills half of every
   resource (`cargo run -p build-tool ... install`, passing
   through `--dry-run` / `--force`), then (b) walks
   `resources/manifest` and runs each `install+skill` row's
   hook as `<install> install <launcher> [kiro-agent]`. The
   hook graceful-skips internally, so the orchestrator stays
   resource-agnostic. `resources/uninstall` is the mirror (Rust
   uninstall + `<install> uninstall [kiro-agent]` per row). The
   kiro-agent name threads through as one optional positional
   at each layer (positional because just submodule recipes
   don't bind `name=value`).

4. **Verb surface** (`resources/Justfile`):
   - `resources::install [kiro-agent]` — now calls
     `resources/install` (both halves). Still the primary verb.
   - `resources::uninstall [kiro-agent]` — calls
     `resources/uninstall` (both halves; allowlist preserved).
   - `resources::browser-mcp-install [kiro-agent]` /
     `-uninstall` / `-status` / `-allow` — unchanged surface;
     they call the now-table-driven `manage`, so they cover
     codex. `browser-mcp-allow` stays browser-specific (the
     allowlist is a browser concept).
   - `resources::browser-functional-test [claude|kiro|codex]
     [kiro-agent]` — the doc comment gains codex.

5. **Tests** —
   - Add `codex` to the `tools:` field of the six fixtures
     currently `claude,kiro` (`browser-safety`, `notes`,
     `notes-add-note`, `notes-git-commit`, `notes-vault-selector`,
     `writing-style`). The four `kdevkit-*` already include it.
   - `resources/tests/browser-functional` gains a codex branch
     in `tool_available()` and `drive()` (via `codex exec
     --sandbox workspace-write --skip-git-repo-check
     --dangerously-bypass-approvals-and-sandbox` so the browser
     MCP tools actually fire — the parallel to claude's
     `--dangerously-skip-permissions` and kiro's
     `--trust-all-tools`), plus the run loop and arg parser.
     Codex is global, so it needs no agent gate.
   - One Rust test asserting manifest ↔ skills-dir consistency.

### Notes / constraints

- Codex is a **global** MCP agent (verified: `codex mcp add
  <name> -- <cmd>` is idempotent; `codex mcp remove` exits 0
  when absent; `codex mcp get` exits 1 absent / 0 present). It
  needs no agent name, so it joins claude in the global path.
- The ASBX `codex` wrapper intercepts top-level `--help` but
  passes subcommands through; the real signatures come from
  `codex mcp help add` / `codex exec help`. This is why the
  design pins exact flags from live probing, not `--help`.
- No Rust logic change — the pure-symlink constraint holds. The
  only Rust delta is a new test.

## Implementation Plan

- [ ] **Slice 1 — table-driven `manage` + codex MCP.** Convert
  `resources/browser/manage`'s per-agent functions to the
  `MCP_AGENTS` table + `mcp_foreach` + three dispatchers; add
  the codex row; keep the browser-specific epilogue. Verify:
  shellcheck clean, `just resources::browser-mcp-status` runs,
  `just ci` green (Rust untouched).
- [ ] **Slice 2 — tests across all three agents.** Add `codex`
  to the six `tools:` fields; add the codex branch/loop/parser
  to `resources/tests/browser-functional`; update the Justfile
  doc comment. `just ci` unaffected.
- [ ] **Slice 3 — manifest + unified orchestrator.** Add
  `resources/manifest` and `resources/install` /
  `resources/uninstall`; lift the `chrome-devtools` / launcher
  literals into manifest columns passed as args to `manage`;
  point `resources::install` / `resources::uninstall` at the
  orchestrators; add the Rust consistency test. Verify:
  graceful-skip on a no-chrome/no-nix box still lays symlinks;
  `just resources::status` resolves; `just ci` green.
- [ ] **Docs.** README + `project.md` touch so the unified
  entry point and codex-everywhere are discoverable (closure
  phase bubbles the durable bits up).

- *Risk note:* codex skill-prose portability may make some
  fixtures fail in the user-run `verify` until prose is tuned;
  accepted, surfaces only in API-costed verify.
- *Risk note:* the codex sandbox mode for MCP tool use in the
  functional test needs the bypass flag; confirmed present, low
  blast radius (attended, user-run).

## Session Log

<!-- append: date · what was done · decisions made -->

- 2026-07-18 · Planning. Grounded against the repo and the live
  CLIs: confirmed the skills registry already covers all three
  agents while `manage` (browser MCP) and `browser-functional`
  cover only claude+kiro, and most `.smoke` fixtures test only
  claude+kiro. Verified codex's MCP surface (`mcp add` global +
  idempotent, `remove` exits 0 absent, `get` exits 1 absent)
  and the `codex exec --dangerously-bypass-approvals-and-sandbox`
  flag for the functional test. Wrote this spec; opening the
  Planning Review Gate.

<!-- append: decision · rationale · alternatives rejected -->

- **Single unified `resources::install`, not a separate
  `install-all`.** Considered adding a distinct `install-all`
  verb beside the skills-only `install` (keeps the always-safe
  primitive untouched). Rejected in favor of making
  `resources::install` itself the unified entry: registration ≠
  activation (deny-by-default launcher) and every hook
  graceful-skips, so unifying carries no new risk, and one
  entry point is the convergence the browser-mcp feature
  named. The granular `browser-mcp-*` verbs remain for
  MCP-only (re)installs.
- **Manifest is a checked-in table, not convention-based
  discovery.** Globbing `resources/content/skills/*` can't
  express the install half (nothing on disk says "browser also
  registers an MCP server") and can't be reviewed as one
  artefact. A checked-in manifest matches the project's
  "registry is the single source of truth" value. Kept free of
  agent-home paths so it never duplicates the Rust registry.
- **MCP layer made data-driven (table), matching the Rust
  registry's shape.** The whole asymmetry stemmed from
  hand-written per-agent functions; a per-agent table + generic
  loop is the symmetric mirror of the registry and makes the
  next agent a row, not a function.
- **Codex required for every skill fixture** (user decision).
  Accepted that some fixtures may fail on codex until prose is
  tuned; those are content follow-ups surfaced only in the
  user-run verify, never in `just ci`.
- **Rust stays pure-symlink** (standing hard constraint).
  Symmetry lives in the shell/Just layer; the only Rust delta
  is a consistency test.
