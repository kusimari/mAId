# Feature: resource-symmetry

## Git Setup

- Branch: `feat/resource-symmetry`
- Base: `main`

## Feature Brief

mAId installs and tests two kinds of AI resource — pure
**skills** (markdown) and **installables** (a skill plus a
runnable install step; the browser-control MCP is the only one
today). Across the three supported coding agents (claude, kiro,
codex) the two kinds are handled **unevenly**: skills deploy
through a registry that covers all three agents but with no way
to target one, while the browser MCP install and the browser
functional test cover only claude and kiro — codex is missing —
and most skill fixtures test only claude and kiro.

This feature makes both kinds **symmetric across all three
agents behind one simple, patterned CLI**. Two ideas do the
work:

1. **A `<action>-<resource-kind>` verb pattern.** Every verb
   names both what it does and which resource kind it touches —
   `install-skills` / `install-browser-mcp`,
   `uninstall-skills` / `uninstall-browser-mcp`, and so on — so
   neither kind is the implicit default. There is no bare
   `install`; the kind is always explicit.
2. **A uniform coding-agent selector** on those verbs: name an
   agent (`claude`/`kiro`/`codex`) to act on just that one, or
   omit it to act on all three.

```
just resources::install-skills       [claude|kiro|codex]              # skills
just resources::install-browser-mcp  [claude|kiro|codex] [kiro-sub]   # the installable
```

After it lands: codex is a first-class target for MCP
registration and for every skill fixture; the skills install can
be scoped to one agent or all; and the MCP install layer is
**data-driven per agent** in the same shape as the skills
registry, so adding the next agent (or the next installable) is a
table edit, not a new hand-written function.

The two resource kinds keep **separate verbs** (skills are always
safe; installables are env-gated and opt-in) but share the **same
action verbs and selector UX**, so the experience is uniform even
though the mechanisms differ. The browser MCP is the first
installable, so its `-browser-mcp` verbs name it directly; when a
second installable lands, the same `<action>-<kind>` pattern
generalizes cleanly (`install-<kind>`, …).

## Requirements

<!-- The experience layer — what the user touches and observes.
     Library/flag/path/script names live in Design. -->

### The verb pattern + coding-agent selector

- Every verb reads `<action>-<resource-kind>`: the action
  (`install` / `uninstall` / `status` / `verify`) and the kind
  (`-skills` / `-browser-mcp`) are both explicit. There is no
  bare `install` or `verify` — the kind is never implicit.
- Every verb takes an **optional coding-agent argument** —
  `claude`, `kiro`, or `codex`. Name one to act on just that
  agent; **omit it to act on all three**. The same word means
  the same thing on every verb.
  - `just resources::install-skills` — install skills for all
    three.
  - `just resources::install-skills codex` — install skills for
    codex only.
  - `just resources::install-browser-mcp` — register the browser
    MCP for all three (kiro caveat below).
  - `just resources::install-browser-mcp claude` — register it
    for claude only.
- An unrecognized selector is a clear error listing the valid
  agents; it never silently does nothing.

### Skills verbs (`*-skills`)

- `install-skills [agent]` installs skills for the selected
  agent, or all three when none is named. Always safe — no
  environment prerequisites.
- `uninstall-skills [agent]` and `status-skills [agent]` reverse
  and report for the same scope.
- Idempotent: re-running install reports "ok"; re-running
  uninstall reports "not-installed".
- **No kiro sub-agent anywhere here.** kiro *skills* are global
  (one steering path, every kiro agent sees them), so the skills
  verbs never ask which kiro agent — that concern belongs only
  to the browser MCP verbs (below).

### Installable verbs (`*-browser-mcp`)

- `install-browser-mcp [agent] [kiro-sub]` registers the
  browser-control MCP for the selected agent, or all three when
  none is named. Env-gated: on a machine with no graphical
  Chrome, no `nix`, or a missing agent CLI, it prints a one-line
  pointer and **graceful-skips** that agent — never a
  half-register or error.
- `uninstall-browser-mcp [agent] [kiro-sub]` removes the
  registration for the same scope and **preserves the
  user-owned allowlist** (user data, not install state).
  `status-browser-mcp [agent] [kiro-sub]` reports registration
  state.
- **codex reaches parity with claude:** the browser MCP
  registers with codex globally, so after install codex has the
  browser tools — the same as claude.
- **kiro's per-agent caveat, confined to these verbs.** kiro
  partitions MCP servers per named sub-agent and mAId never
  guesses which one. So targeting kiro takes a **trailing
  sub-agent name**: `just resources::install-browser-mcp kiro
  <sub-agent>`. Targeting kiro (or all) *without* a name
  graceful-skips kiro's MCP with a message telling the user to
  name the agent; claude and codex need no such name. This is
  the one asymmetry the browser MCP can't erase (it's kiro's
  model), and it lives here rather than leaking into the general
  selector.
- `browser-mcp-allow <pattern>` (allowlist management, not an
  action-on-resource verb) keeps its name — it's browser-specific
  and orthogonal to the install lifecycle.

### Codex parity in the verify (token-spending) tests

- `verify-skills [agent]` drives the selected agent (or all
  three) against every skill fixture — not just the kdevkit ones
  — so a skill that behaves correctly is one that behaves
  correctly on all three agents. Replaces today's bare `verify`.
- `verify-browser-mcp [agent] [kiro-sub]` is the attended
  functional test: it drives the selected agent (or all three)
  through the real browser MCP, asserting an allow-listed site
  loads and an off-list site is blocked. Replaces today's
  `browser-functional-test`; codex now included (global, so no
  sub-agent needed). Both `verify-*` verbs spend tokens / drive
  real Chrome and stay `[confirm]`-gated and user-run.

## Test Strategy

<!-- Success criteria mapped onto project.md's two test layers:
     `just test` (build-tool unit, tempfile-fake-HOME) and
     `just verify` (AI-tool functional .smoke fixtures). -->

### Build-tool unit (`just test`)

- The Rust build-tool stays **pure-symlink**; the existing suite
  (including the `Kind::FanOut` codex coverage) is the
  regression guard that the symlink mechanics are untouched.
- **New tests for the agent selector** (the one behavioral
  change on the Rust side): install/uninstall/status scoped to a
  single agent touches only that agent's registry rows and
  leaves the others alone; the default (no agent) still covers
  all three; an unknown agent name is a clean error. These
  assert against the tempfile-fake `$HOME`, same as the existing
  symlink tests.

### Functional smoke (`just resources::verify-skills`, user-run)

- Every `.smoke` fixture runs against **all three agents**
  (`tools: claude,kiro,codex`). The runner already supports
  codex; the change is per-fixture `tools:` fields. This is the
  "verify across every agent we ship" success criterion.
- Risk accepted (see Decision Log): a skill whose prose was
  tuned for claude/kiro may not yet drive codex to the same
  result. Those surface only in the user-run, API-costed
  `verify-skills` — never in `just ci` — and are content-fix
  follow-ups, not blockers for this feature.

### Desktop-manual (attended, user-run)

- `just resources::verify-browser-mcp codex` (and the claude /
  kiro forms) drives the real browser MCP against an isolated
  temp allowlist, asserting an allow-listed site loads and an
  off-list site is **blocked**. Codex is global, so no sub-agent
  name is needed. `[confirm]`-gated and separate from the bulk
  `.smoke` runner, as today. Renamed from
  `browser-functional-test` to fit the `verify-<kind>` pattern.

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

The fix is two-part, both driven by the **same coding-agent
selector**: give the skills registry an agent tag so install can
be scoped, and mirror the registry's data-driven shape in the
shell layer so the MCP registrar gains codex as a table row.

**Two verbs, not one — but one selector.** Skills and the
installable stay **separate verbs**: skills are always safe and
prerequisite-free, while the installable is env-gated (needs
Chrome, `nix`, the agent CLI) and warrants explicit opt-in.
Merging them would drag the installable's environment
prerequisites — and kiro's per-sub-agent MCP concern — onto the
always-safe skills path. What makes the experience *feel*
unified is not one verb but **one selector word** (`claude` /
`kiro` / `codex`, default all) meaning the same thing on both.

**kiro's sub-agent belongs only to the installable.** kiro
*skills* install to a single global steering path — every kiro
sub-agent sees them — so the skills verb never needs a sub-agent
name. Only kiro *MCP* is per-sub-agent. Confining the sub-agent
argument to `browser-mcp-install` (as a trailing arg) keeps the
general selector clean; this is why the earlier "thread a
kiro-agent through the unified install" shape was wrong.

**Keep the installable simple until there's a second one.** The
browser MCP is the only installable today. Rather than build a
manifest + generic orchestrator now (a real cost in moving
parts, and speculative until a second installable exists), the
browser-mcp verbs stay self-contained and simple. When a second
installable arrives, generalizing them into an `installable`
surface is a clean follow-up — the per-agent table refactor here
is exactly the groundwork that makes that easy.

**The Rust build-tool stays pure-symlink.** Standing hard
constraint (`project.md` Hard constraints, the browser-mcp
feature): MCP registration is a runnable command a symlink can't
express, so it stays in the shell/Just layer. The agent selector
does **not** violate this — it only *filters which registry rows
get symlinked*; the mechanic is still "create/remove a symlink."

### Components

1. **Agent-scoped skills install** — the Rust build-tool gains
   an **agent tag per `REGISTRY` row** and an optional
   `--agent <name>` filter on `install` / `uninstall` / `status`.
   With no filter, every row is processed (today's behavior).
   With a filter, only that agent's rows are. The tag is the
   third dimension the registry was missing:

   ```
   //                 home_subpath              source            kind      agent
   (".claude/skills",             "…/skills", Kind::Link),   // claude
   (".kiro/steering/skills",      "…/skills", Kind::Link),   // kiro
   (".codex/skills",              "…/skills", Kind::FanOut),  // codex
   ```

   An unknown `--agent` value exits non-zero with the valid
   list. Still pure-symlink — the filter only selects rows.

2. **Table-driven MCP registrar** — refactor
   `resources/browser/manage`. Replace the hand-written
   per-agent functions with a per-agent **data table** mirroring
   the registry, plus an **agent filter** so
   `browser-mcp-install <agent>` scopes to one:

   ```
   # agent   cli        scope      style
   #   scope: global (claude, codex — one server, every session)
   #          per-agent (kiro — needs an explicit sub-agent name)
   #   style: readd   (claude — `mcp add` errors if the name exists)
   #          replace (kiro, codex — add is add-or-replace)
   MCP_AGENTS=(
     "claude  claude    global     readd"
     "kiro    kiro-cli  per-agent  replace"
     "codex   codex     global     replace"
   )
   ```

   A generic `mcp_foreach <op> [agent-filter] [kiro-subagent]`
   holds all shared logic once — the row filter, CLI-on-PATH
   check + graceful skip, kiro's skip-when-unnamed, message
   formatting. Three thin `case "$agent"` dispatchers
   (`mcp_add` / `mcp_remove` / `mcp_get`) issue only the
   irreducible CLI differences: claude's `-s user` +
   remove-then-add, kiro's `--name/--agent/--command` +
   stderr-with-ANSI `mcp list` status parse, codex's global
   idempotent `mcp add`/`remove`/`get`. Strictly less
   hand-written per-agent code than today (one generic loop +
   three small dispatchers vs. six full functions), and codex
   joins as a global agent beside claude. `chrome-devtools` and
   the launcher path stay in `manage` (it's the only
   installable; no manifest needed yet).

3. **Verb surface** (`resources/Justfile`) — the
   `<action>-<kind>` pattern with the uniform selector. The
   current bare `install` / `uninstall` / `status` / `verify` and
   `browser-functional-test` are **renamed** (no bare action
   verbs survive):
   - `resources::install-skills [agent]` — `cargo run -p
     build-tool ... install` with `--agent <agent>` when given.
     Keeps `--dry-run` / `--force` passthrough.
   - `resources::uninstall-skills [agent]` /
     `resources::status-skills [agent]` — skills uninstall /
     status, same scoping.
   - `resources::install-browser-mcp [agent] [kiro-subagent]` /
     `uninstall-browser-mcp [agent] [kiro-subagent]` /
     `status-browser-mcp [agent] [kiro-subagent]` — call the
     table-driven `manage` with the agent filter; codex covered.
     Targeting kiro needs the trailing sub-agent name or kiro
     graceful-skips.
   - `resources::browser-mcp-allow <pattern>` — unchanged
     (allowlist management, browser-specific).
   - `resources::verify-skills [agent]` — the `.smoke` runner
     (renamed from `verify`); `[confirm]`-gated.
   - `resources::verify-browser-mcp [claude|kiro|codex]
     [kiro-subagent]` — the attended browser drive (renamed from
     `browser-functional-test`); `[confirm]`-gated; codex added.
   - `resources::verify-skills-one <name> [agent]` — single
     skill fixture (renamed from `verify-one` for
     pattern-consistency; confirmed at plan approval).

4. **Tests** —
   - Add `codex` to the `tools:` field of the six fixtures
     currently `claude,kiro` (`browser-safety`, `notes`,
     `notes-add-note`, `notes-git-commit`, `notes-vault-selector`,
     `writing-style`). The four `kdevkit-*` already include it.
   - `resources/tests/browser-functional` (the script behind
     `verify-browser-mcp`) gains a codex branch in
     `tool_available()` and `drive()` (via `codex exec
     --sandbox workspace-write --skip-git-repo-check
     --dangerously-bypass-approvals-and-sandbox` so the browser
     MCP tools actually fire — the parallel to claude's
     `--dangerously-skip-permissions` and kiro's
     `--trust-all-tools`), plus the run loop and arg parser.
     Codex is global, so no sub-agent gate.
   - Rust tests for the agent selector (see Test Strategy).

### Notes / constraints

- Codex is a **global** MCP agent (verified: `codex mcp add
  <name> -- <cmd>` is idempotent; `codex mcp remove` exits 0
  when absent; `codex mcp get` exits 1 absent / 0 present). It
  needs no sub-agent name, so it joins claude in the global path.
- The ASBX `codex` wrapper intercepts top-level `--help` but
  passes subcommands through; the real signatures come from
  `codex mcp help add` / `codex exec help`. This is why the
  design pins exact flags from live probing, not `--help`.
- The only Rust behavioral delta is the agent filter (still
  pure-symlink); everything else is shell/Just.

## Implementation Plan

- [x] **Slice 1 — table-driven `manage` + codex MCP + agent
  filter.** Convert `resources/browser/manage`'s per-agent
  functions to the `MCP_AGENTS` table + `mcp_foreach` + three
  dispatchers; add the codex row; add the optional agent filter
  + trailing kiro sub-agent arg; keep the browser-specific
  epilogue. Verify: shellcheck clean,
  `just resources::status-browser-mcp` and
  `… status-browser-mcp codex` both run, `just ci` green (Rust
  untouched).
- [ ] **Slice 2 — agent selector on the skills install.** Add
  the agent tag to `REGISTRY` and the `--agent` filter to
  `install` / `uninstall` / `status`; thread the optional
  selector through the renamed `resources::install-skills` /
  `uninstall-skills` / `status-skills` Just recipes. Add the
  Rust selector tests. Verify: `just resources::install-skills
  codex` links only codex, `just resources::install-skills`
  links all three, unknown agent errors; `just ci` green.
- [ ] **Slice 3 — verb rename + tests across all three agents.**
  Apply the `<action>-<kind>` rename across `resources/Justfile`
  (`verify` → `verify-skills`, `browser-functional-test` →
  `verify-browser-mcp`, `browser-mcp-install/-uninstall/-status`
  keep their kind suffix reordered to `install-browser-mcp` etc.);
  add `codex` to the six `tools:` fields; add the codex
  branch/loop/parser to `resources/tests/browser-functional`;
  update the Justfile doc comments. `just ci` unaffected.
- [ ] **Docs.** README + `project.md` touch so the verb pattern,
  the agent selector, and codex-everywhere are discoverable
  (closure phase bubbles the durable bits up).

- *Risk note:* the verb rename is a breaking CLI change — sweep
  README, `project.md`, and any spec/backlog reference to the old
  `resources::install` / `verify` / `browser-functional-test`
  names so nothing points at a dead verb.
- *Risk note:* codex skill-prose portability may make some
  fixtures fail in the user-run `verify-skills` until prose is
  tuned; accepted, surfaces only in API-costed verify.
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
  flag for the functional test. Wrote the first spec; opened the
  Planning Review Gate (PR #31).
- 2026-07-21 · Planning-review revision. On review, the earlier
  "single unified `resources::install` that threads a
  `[kiro-agent]` through both halves" was called out as
  cluttered. Reframed the whole feature around a **uniform
  coding-agent selector** (`claude`/`kiro`/`codex`, default all)
  on two **separate** verbs (skills vs. installable). Dropped the
  manifest + unified orchestrator (scaffolding for the abandoned
  single-entry design) — the browser MCP is the only installable,
  so its verb stays simple until a second one justifies
  generalizing. Confined kiro's per-sub-agent MCP concern to the
  browser-mcp verb (kiro skills are global, so it never belonged
  on the skills verb). Rust gains an agent filter (still
  pure-symlink) instead of a manifest-consistency test.
- 2026-07-21 · Planning-review round 2 (PR #31 comment). User
  asked for a symmetric `<action>-<resource-kind>` verb pattern
  so neither kind is the implicit default: `install-skills` /
  `install-browser-mcp`, and likewise for uninstall, status, and
  the two verify (token-spending) tests. Renamed the whole verb
  surface accordingly — no bare `install` / `verify` /
  `browser-functional-test` survive. At approval the user also
  confirmed `verify-one` → `verify-skills-one`. Plan approved;
  entering the dev loop.
- 2026-07-21 · Slice 1 done. Refactored `resources/browser/manage`
  from hand-written `register_claude`/`register_kiro` (+ variants)
  to a data-driven `MCP_AGENTS` table + `mcp_foreach` loop + three
  leaf dispatchers (`mcp_add`/`mcp_remove`/`mcp_get`); added the
  codex row (global) and an agent filter + validate. Renamed the
  coupled Justfile recipes to `install-/uninstall-/status-browser-mcp`
  with the `[agent] [kiro_sub]` arg order. Verified: status
  all/codex/kiro-skip/unknown-error paths, chrome graceful-skip,
  codex add→get→remove round-trip, shellcheck clean, `just ci`
  green (53 tests, Rust untouched). Code Review Gate (fresh-context
  agent, host-native): **78/100, PASS** (threshold 70). Confirmed
  the `set -e`/`readd` line is correct and no claude/kiro behavior
  regresses. Applied the in-diff findings: refreshed the stale
  in-file usage/design header, gated codex's "removed" message on a
  prior registration check (codex `mcp remove` exits 0 even when
  absent), fixed the two graceful-skip verb strings, column-aligned
  output, and de-SC2015'd the remove-then-add line. The doc-sweep
  findings (README / browser SKILL.md / project.md / the
  functional-test comment still name old verbs) are deferred to
  Slice 3 + the Docs task, which own the rename sweep.

<!-- append: decision · rationale · alternatives rejected -->

- **`<action>-<resource-kind>` verb pattern** (review steer,
  PR #31). Every verb names both the action and the kind
  (`install-skills`, `install-browser-mcp`, `verify-skills`,
  `verify-browser-mcp`, …); no bare `install` / `verify`. Chosen
  over keeping `install` as the implicit skills default (with the
  installable prefixed `browser-mcp-*`) because the asymmetric
  naming was exactly what made one kind feel primary and the
  other bolted-on. The patterned names make the two kinds read as
  peers and scale to a third kind by the same rule. Cost: a
  breaking CLI rename (swept in Slice 3).
- **Uniform agent selector, two separate verbs** (review steer,
  supersedes the first-draft "single unified install"). The
  organizing principle is one selector word — `claude`/`kiro`/
  `codex`, omit for all — meaning the same thing on both
  `resources::install-skills` and
  `resources::install-browser-mcp`. Rejected
  merging them into one verb: skills are always safe and
  prerequisite-free, the installable is env-gated and opt-in, and
  merging drags the installable's prerequisites plus kiro's
  per-sub-agent MCP concern onto the always-safe skills path. The
  uniform *selector* delivers the "feels like one experience"
  goal without coupling the mechanisms.
- **kiro sub-agent confined to the installable verb.** kiro
  *skills* install to one global steering path (every kiro
  sub-agent sees them), so the skills verb never needs a
  sub-agent name; only kiro *MCP* is per-sub-agent. The trailing
  `[kiro-subagent]` arg lives only on `browser-mcp-install`.
  Rejected threading a `kiro-agent` through the general
  interface — that was the clutter the review flagged.
- **No manifest / orchestrator yet — keep the installable
  simple.** Considered a declarative `resources/manifest` + a
  generic orchestrator so one entry point deploys every resource
  kind. Rejected as speculative: there is exactly one installable
  today, so the manifest would enumerate a single row and the
  orchestrator would loop once. The per-agent table refactor in
  `manage` is the real groundwork; generalizing to an
  `installable` surface is a clean follow-up when a second one
  lands.
- **MCP layer made data-driven (table), matching the Rust
  registry's shape.** The whole asymmetry stemmed from
  hand-written per-agent functions; a per-agent table + generic
  loop is the symmetric mirror of the registry and makes the
  next agent a row, not a function.
- **Codex required for every skill fixture** (user decision).
  Accepted that some fixtures may fail on codex until prose is
  tuned; those are content follow-ups surfaced only in the
  user-run verify, never in `just ci`.
- **Rust stays pure-symlink** (standing hard constraint). The
  agent selector only filters which registry rows are symlinked;
  the mechanic is unchanged.
