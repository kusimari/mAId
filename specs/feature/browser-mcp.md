# Feature: browser-mcp

## Git Setup

- Branch: `backlog/browser-mcp`
- Base: `main`

## Feature Brief

mAId gains a second kind of installable AI resource: a
**browser-control capability**. After installing it, the agent
can open pages, navigate, fill forms, click, and read results
in the user's real, already-running Google Chrome — driving the
live session so existing logins are reused with no separate
auth flow.

The capability ships with a **guardrail the browser enforces**:
the agent may only act on sites the user has put on an
allowlist. The allowlist is the user's own file, managed
independently of how the capability is installed, so the set of
sites the agent can touch is always under direct user control.

This is also the project's first non-skill resource. Until now
`just resources::install` only laid down skills (markdown
symlinks); this feature adds verbs that register a runnable MCP
server with each agent harness. It establishes the pattern for
"install more kinds of AI resource later."

## Requirements

<!-- The experience layer — what the user touches and observes.
     Library/flag/path/script names live in Design. -->

### Install / uninstall experience (one-shot)

- A verb under the `resources::` namespace registers the
  browser capability with the installed agent harness(es).
  After it runs, the agent has browser tools available and can
  drive Chrome end-to-end (open → read → fill → submit) against
  allow-listed sites.
- A verb removes that registration; afterward the agent no
  longer has the browser tools.
- A verb reports whether the capability is currently
  registered, per harness.
- All three verbs are **idempotent** and **skip gracefully**:
  on a machine with no graphical Chrome, or missing the runtime
  the server needs, they print a one-line pointer to what's
  missing and make no changes — they never half-register or
  error out.
- Installing prints the **one-time browser setup** the user
  must do by hand (enable the browser's remote-debugging
  permission once), since the agent cannot do it for them.
- These sit **beside** `resources::install` (which lays down
  skills) as separate verbs today, because MCP registration has
  environment prerequisites and warrants explicit opt-in. The
  intended direction is convergence: as the install model
  matures, `resources::install` can orchestrate every resource
  kind (skills + MCPs), with env-gated parts graceful-skipping.

### Allowlist experience (ongoing, user-owned)

- The agent can act **only** on sites the user has allow-listed.
  Navigation to an off-list site is **blocked by the browser**,
  not merely discouraged in agent instructions.
- The allowlist is a **plain text file the user owns**, at a
  default location or any path the user chooses. The user edits
  it directly — add a site, remove a site — with any editor.
- Changes take effect on the **next browser-control session**
  with no re-install and no involvement from the install
  tooling.
- For users who would rather not hand-edit, a verb appends a
  site pattern to the allowlist **verbatim** — mAId does not
  define or validate the pattern syntax; whatever the browser
  accepts is what the user writes, passed straight through.
  (Adding through the browser's own UI is preferred where the
  browser supports it; the verb is the fallback.)
- **Empty/absent allowlist means deny, not allow-all** — if no
  sites are listed, the capability refuses to start rather than
  exposing every logged-in site. The user observes a clear
  message telling them to add at least one site.

### Safety posture (taught by the skill)

- The agent follows a **snapshot → act → verify** driving loop
  and treats page content as untrusted input.
- **Attended use only.** The skill steers the user away from
  pointing this at credential/SSO-gated sites in unattended or
  scheduled runs, and explains the prompt-injection blast
  radius: a hostile page can try to steer an agent that is
  acting with the user's live logins.

## Test Strategy

<!-- Success criteria mapped onto project.md's two test layers:
     `just test` (build-tool unit, tempfile-fake-HOME) and
     `just verify` (AI-tool functional .smoke fixtures). -->

### Build-tool unit (`just test`)

- **No new Rust coverage expected.** This feature adds no
  registry entry and no build-tool code (see Design). The
  existing `cargo test` suite must stay green — it's the
  regression guard that the symlink/registry half is untouched.

### Functional smoke (`just verify`, user-run)

- A `.smoke` fixture asserts the **browser skill loads and
  teaches the safety posture**: a prompt asking the agent how it
  drives a browser should surface the allow-listed-sites
  guardrail, the snapshot→act→verify loop, and attended-use
  caution. Substring check for a load-bearing phrase + a judge
  check for the narrative. Cheap; no real browser.

### Desktop-manual (not automated)

- Real end-to-end driving (register → enable remote debugging →
  drive an allow-listed public site → confirm an off-list site
  is blocked) is **manual on a desktop**, documented as a
  checklist in the spec's verification. The allowlist
  deny-by-default behavior and the off-list block are verified
  here, since they need a live browser.
- Launcher allowlist-argument construction (reads the file →
  one block flag per listed site, refuses on empty) is simple
  enough to eyeball; a shell-level check is nice-to-have, not
  load-bearing.

## Design

<!-- Rationale first. -->

### Why this shape

The capability is **Google's official `chrome-devtools-mcp`**
(Apache-2.0), launched with `--autoConnect` so it joins the
user's already-running browser over the DevTools Protocol
rather than spawning a throwaway profile — that's what makes
existing logins "just work" with no re-auth. It's the
Chrome-team-maintained server (the older
`@modelcontextprotocol/server-puppeteer` is deprecated), so it
tracks the browser closely and is the durable pick.

**The guardrail must live at the browser connection, not in the
skill.** A skill is advice; nothing forces an agent to honor an
allowlist written in prose, and a prompt-injected agent will
ignore it. `chrome-devtools-mcp` exposes `--allowedUrlPattern`
(an enforced launch flag), so the browser itself refuses
off-list navigation. The skill still teaches the safe loop, but
as guidance layered on top of hard enforcement — defense in
depth, not the only line.

**The allowlist is decoupled from install** so the user owns it
outright. Baking patterns into the registration would mean
re-running install to change the allowlist and would put the
trust policy inside tooling the user doesn't routinely touch.
Instead the registration points at a thin launcher that reads
the user's allowlist file at each launch; editing the file is
all it takes to change what the agent may touch.

**Tooling is environment-provided.** Per project convention,
mAId reuses the runtime the environment already supplies (the
JS runtime that runs the server comes from the user's
environment, e.g. a version manager) and does not install it.
mAId-owned scripts stay dependency-free (POSIX shell); should a
mAId script ever need a tool the environment lacks, it would
come through the repo flake — not a global install — but this
feature needs nothing beyond shell + the environment's runtime.

**No build-tool / registry change.** The skill half rides the
existing symlink registry (zero code). The registration half is
a runnable command, which a symlink can't express, so it lives
in `resources/Justfile` module verbs that shell out to each
harness's own MCP CLI — keeping the Rust build-tool pure-symlink,
as the project's hard constraints require.

**Pattern syntax is the browser's, not mAId's.** The allowlist
file holds whatever pattern strings the browser's enforced-allow
flag accepts; mAId reads lines and passes them through unaltered
— no validation, no bare-domain expansion. This keeps mAId
decoupled from the browser's pattern grammar as it evolves.

### Components

1. **Skill** — `resources/content/skills/browser/SKILL.md`.
   Pure markdown. Teaches the server's tools (navigate, new
   page, fill, fill form, click, snapshot, list network
   requests, …), the snapshot→act→verify loop, the allowlist
   guardrail, and the attended-use safety posture. Rides the
   existing registry symlinks into `.claude/skills` and
   `.kiro/steering/skills` — no installer change.

2. **Launcher** — a small POSIX-shell script shipped in the
   repo (e.g. `resources/browser/launch`). At launch it:
   resolves the allowlist file (default
   `${XDG_CONFIG_HOME:-$HOME/.config}/maid/browser-allowlist`,
   overridable by an env var); reads non-blank, non-`#` lines as
   site patterns; **refuses to start with a clear message if the
   list is empty/absent** (deny-by-default); otherwise builds one
   enforced-allow flag per pattern and execs the server via the
   environment's runtime with `--autoConnect`. The MCP
   registration points here (absolute path in the checkout, same
   as the symlinks point back into the checkout).

3. **`resources::` module verbs** (shell, no Rust; added to
   `resources/Justfile`, invoked as `just resources::<verb>`):
   - `install-mcp` — prereq-detect (graphical Chrome present;
     runtime present; harness CLI present); register the server
     with `claude mcp add` and/or `kiro-cli mcp add` pointing at
     the launcher; print the one-time remote-debugging setup
     reminder. Idempotent; graceful per-prereq skip.
   - `uninstall-mcp` — remove the registration via each harness
     CLI. Does **not** touch the user's allowlist file.
   - `mcp-status` — report registration state per harness.
   - `browser-allow <pattern>` — append a pattern line to the
     default allowlist file **verbatim** (the hand-edit
     fallback). Creates the file if absent.

   Final verb names are confirmed at dev time against the
   module's existing naming; the set above is the contract.

### Notes / constraints

- The enforced-allow flag requires a recent enough Chrome; the
  user's desktop is on a version that supports it. Where a
  desktop is older, the launcher's deny-by-default still holds
  (it refuses on empty list) and the skill guidance still
  applies, but enforced blocking needs the supported browser.
- `uninstall-mcp` is deliberately allowlist-preserving: the
  allowlist is user data, not install state.

## Implementation Plan

- [ ] **Skill.** Write `resources/content/skills/browser/SKILL.md`
  — generic/public wording, the tool list, the
  snapshot→act→verify loop, the allowlist guardrail, and the
  attended-use safety posture. (Rides existing registry; verify
  with `just status` that the symlink resolves it.)
- [ ] **Launcher.** Add the POSIX-shell launcher: resolve
  allowlist path (default + env override), parse patterns,
  deny-by-default on empty, build enforced-allow flags, exec the
  server via the environment runtime with `--autoConnect`.
- [ ] **Module verbs.** Add `install-mcp` / `uninstall-mcp` /
  `mcp-status` / `browser-allow` to `resources/Justfile` (the
  `resources::` module), with prereq detection and graceful
  skip, calling the harness MCP CLIs.
- [ ] **Smoke fixture.** Add
  `resources/tests/skills/browser-*.smoke` (substring + judge)
  asserting the skill loads and teaches the safety posture.
- [ ] **Docs.** README/Justfile-help touch so the new verbs and
  the one-time browser setup are discoverable.

- *Risk note:* enforced-allow flag is browser-version-gated;
  deny-by-default in the launcher is the portable floor.
- *Risk note:* registration stores an absolute checkout path;
  moving the checkout requires a re-`install-mcp` (same
  property the symlink registry already has).
- *Risk note:* coexistence of mAId's environment with the
  sibling internal-resources repo's environment is **out of
  scope** — explicitly a follow-up after this works.

## Session Log

<!-- append: date · what was done · decisions made -->

- 2026-06-22 · Promoted from backlog; rebased branch onto
  `main` (`0eefca5`). Grounded against the real environment:
  Chrome supports the enforced-allow flag; `claude mcp add` and
  `kiro-cli mcp add` both exist (Kiro needs no JSON-merge); the
  JS runtime is environment-provided (version-manager node, not
  on the build PATH). Generalized all wording to public/generic
  per decision below.
- 2026-06-22 · Resolved 3 planning questions: (1) allowlist
  default path under `~/.config/maid/`; (2) verbs live in the
  `resources::` Just module beside `resources::install`,
  modeled separate now with convergence intended; (3) allowlist
  patterns pass through verbatim — mAId owns no pattern grammar.
  Noted the rebase swapped the flat Justfile for `mod resources`
  / `mod kaimux`; verbs target `resources/Justfile`.

## Decision Log

<!-- append: decision · rationale · alternatives rejected -->

- **Guardrail enforced at the browser, not the skill.**
  Considered skill-prose-only allowlisting; rejected — a skill
  cannot bind an agent (or a prompt-injected one) to an
  allowlist. The browser's enforced-allow flag is the only hard
  guarantee; the skill is defense-in-depth on top.
- **Allowlist is user-owned and decoupled from install.**
  Considered baking patterns into the MCP registration;
  rejected — that would require re-installing to change trust
  policy and hide the policy inside tooling. A launcher that
  reads the user's file each launch keeps the user in control;
  edits take effect next session.
- **Deny-by-default on empty allowlist.** An empty/absent list
  refuses to start rather than allowing every logged-in site —
  fail safe, given the connection inherits all live logins.
- **No build-tool/registry change.** Skill rides the existing
  symlink registry; registration is a runnable command
  expressed as `resources::` Just-module verbs over each
  harness's MCP CLI, keeping the Rust build-tool pure-symlink
  per hard constraints.
- **Verbs separate now, converge later.** Modeled as distinct
  `resources::install-mcp` (env-gated) beside
  `resources::install` (skills, always works); `resources::
  install` can absorb all resource kinds once the model matures.
- **Pattern pass-through.** `browser-allow` and the launcher
  pass allowlist lines to the browser verbatim; mAId defines no
  pattern syntax, staying decoupled from the browser's grammar.
- **Generic/public framing.** mAId is a generic public repo; the
  browser is treated as a black box ("Chrome, with guardrails").
  No product/team/internal names in spec, skill, or commits —
  the safety intent (don't loose an agent on credential-gated
  sites unattended) is kept, phrased generically.
- **Environment-provided tooling.** mAId reuses the
  environment's JS runtime and does not install it; mAId-owned
  scripts stay dependency-free shell. Cross-environment
  coexistence with the sibling internal repo is a deliberate
  follow-up, not this feature.
