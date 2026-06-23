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
  (the next time the agent's browser server connects) with no
  re-install and no involvement from the install tooling. The
  user's Chrome — and every login in it — **stays running and
  untouched** across an allowlist edit; only the lightweight
  background browser server re-reads the file. Edits do not
  apply to a connection already in progress; *truly live,
  mid-session* allowlist changes are a deferred enhancement
  (see Decision Log).
- For users who would rather not hand-edit, a verb appends a
  site pattern to the allowlist **verbatim** — mAId does not
  define or validate the pattern syntax; whatever the browser
  accepts is what the user writes, passed straight through. The
  file is the source of truth; managing the allowlist through a
  browser-native surface is a deferred enhancement (the browser
  exposes no per-site allowlist UI to build on today).
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

**MCP runtime is self-contained in mAId's flake.** Skills can't
be self-contained — claude/kiro require them in their own
configs, so we register there. An MCP server is different: it's
an out-of-process service the harness *calls*, so while its
*registration* lives in the harness config, its *runtime* can
and should be mAId's own. The server runs on Node.js; mAId
provides it from the repo flake (the same `nix develop` /
`direnv` shell that already supplies cargo + just), rather than
depending on a user-PATH Node. Because claude/kiro launch the
server from outside mAId's direnv, the launcher re-execs itself
through `nix develop path:<repo>` to enter the flake — so Node
need not be on the user's PATH; only `nix` (the repo's standing
prerequisite). This also makes the capability real-end-to-end
testable from the repo: `direnv allow` brings the same `npx`
into reach. mAId-owned scripts otherwise stay dependency-free
shell. (Sibling-repo precedent: the internal-resources repo
supplies its runtime via a version manager where the
environment lacks it; mAId's equivalent is the flake.)

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
   repo (e.g. `resources/browser/launch`). The MCP registration
   points the server-launch command at this script (absolute
   path in the checkout, same as the symlinks point back into
   the checkout), so it runs **every time the agent's browser
   server (re)connects** — that's what makes an allowlist edit
   take effect on reconnect without touching Chrome. Each run
   it: first **enters mAId's flake** if the Node runtime isn't
   already present — re-execing itself through `nix develop
   path:<repo>` (repo root derived from the script's own
   resolved location), guarded against re-entry — so the
   server runs on mAId's bundled Node, not a user-PATH one;
   then resolves the allowlist file (default
   `${XDG_CONFIG_HOME:-$HOME/.config}/maid/browser-allowlist`,
   overridable by an env var); reads non-blank, non-`#` lines as
   site patterns; **refuses to start with a clear message if the
   list is empty/absent** (deny-by-default); otherwise builds one
   enforced-allow flag per pattern and execs the server with
   `--autoConnect` (which attaches to the already-running
   Chrome — the server process is cheap to restart; the browser
   is not restarted at all).

3. **`resources::` module verbs** (shell, no Rust; added to
   `resources/Justfile`, invoked as `just resources::<verb>`).
   Named with a common `browser-mcp-` prefix so they group and
   sit together in `just --list`:
   - `browser-mcp-install` — prereq-detect (graphical Chrome
     present; runtime present; harness CLI present); register
     the server with `claude mcp add` and/or `kiro-cli mcp add`
     pointing at the launcher; print the one-time
     remote-debugging setup reminder. Idempotent; graceful
     per-prereq skip.
   - `browser-mcp-uninstall` — remove the registration via each
     harness CLI. Does **not** touch the user's allowlist file.
   - `browser-mcp-status` — report registration state per
     harness.
   - `browser-mcp-allow <pattern>` — append a pattern line to
     the default allowlist file **verbatim** (the hand-edit
     fallback). Creates the file if absent.

### Notes / constraints

- The enforced-allow flag requires a recent enough Chrome; the
  user's desktop is on a version that supports it. Where a
  desktop is older, the launcher's deny-by-default still holds
  (it refuses on empty list) and the skill guidance still
  applies, but enforced blocking needs the supported browser.
- `browser-mcp-uninstall` is deliberately allowlist-preserving:
  the allowlist is user data, not install state.

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
- [ ] **Module verbs.** Add `browser-mcp-install` /
  `browser-mcp-uninstall` / `browser-mcp-status` /
  `browser-mcp-allow` to `resources/Justfile` (the `resources::`
  module), with prereq detection and graceful skip, calling the
  harness MCP CLIs.
- [ ] **Smoke fixture.** Add
  `resources/tests/skills/browser-*.smoke` (substring + judge)
  asserting the skill loads and teaches the safety posture.
- [ ] **Docs.** README/Justfile-help touch so the new verbs and
  the one-time browser setup are discoverable.

- *Risk note:* enforced-allow flag is browser-version-gated;
  deny-by-default in the launcher is the portable floor.
- *Risk note:* registration stores an absolute checkout path;
  moving the checkout requires a re-run of `browser-mcp-install`
  (same property the symlink registry already has).
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
- 2026-06-22 · Planning Review Gate (PR #29) — 3 review
  comments resolved: (1) verbs renamed to a common
  `browser-mcp-` prefix so they group; (2)+(3) "dynamic / no
  browser reload" + "browser-UI-as-default" — grounded that
  `--allowedUrlPattern` is launch-only and chrome://inspect is
  connection-level consent (no per-site UI). Adopted
  reconnect-rereads-file (Chrome never reloads) as the shipping
  behavior; true mid-session-live and browser-native allowlist
  management recorded as deferred enhancements.
- 2026-06-22 · Dev loop — built all 5 slices: skill, launcher,
  `browser-mcp-*` verbs (+ `manage` script), smoke fixture, docs.
  Functionally tested launcher deny-by-default + pattern parsing
  with a stubbed runtime, and the full install/status/uninstall
  lifecycle against the real `claude` CLI (registers, idempotent,
  clean uninstall, allowlist preserved); kiro skip is auth-gated
  (logged out → graceful skip with reason). Quality+Test green
  (`just ci`, shellcheck, 53 Rust tests). Code Review Gate
  (fresh-context general-purpose agent, host-native): **score
  88, PASS**, no blockers/majors. Applied 2 nits (fatal launcher
  msg → stderr; npx recheck comment); verified the status-exit
  finding was a non-issue (`claude mcp get` / `kiro mcp status
  --name` both return 1 on absence).
- 2026-06-22 · Post-dev steer: make the **MCP runtime
  self-contained**. Added `pkgs.nodejs_22` to `flake.nix`;
  rewrote the launcher to enter mAId's flake via `nix develop
  path:<repo>` (repo root derived from its own resolved path,
  re-entry-guarded) so the server runs on bundled Node, not a
  user-PATH one. `manage` install prereq changed npx→nix.
  **Real end-to-end now verified:** from a clean, npx-less env
  (as claude/kiro invoke it) the launcher entered the flake and
  actually started chrome-devtools-mcp (real startup banner);
  deny-by-default and no-nix error paths still hold through the
  re-exec. Updated README + spec rationale.

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
  `resources::browser-mcp-install` (env-gated) beside
  `resources::install` (skills, always works); `resources::
  install` can absorb all resource kinds once the model matures.
- **Verb naming: `browser-mcp-` prefix** (PR #29). Chosen over
  `install-mcp` / `mcp-status` so the capability's verbs group
  and sit together in `just --list` rather than scattering
  across the module's alphabetical listing.
- **Pattern pass-through.** `browser-mcp-allow` and the launcher
  pass allowlist lines to the browser verbatim; mAId defines no
  pattern syntax, staying decoupled from the browser's grammar.
- **Generic/public framing.** mAId is a generic public repo; the
  browser is treated as a black box ("Chrome, with guardrails").
  No product/team/internal names in spec, skill, or commits —
  the safety intent (don't loose an agent on credential-gated
  sites unattended) is kept, phrased generically.
- **MCP runtime self-contained via mAId's flake** (revised
  post-dev, on user steer). The earlier "reuse the environment's
  JS runtime, don't install it" call was reversed: mAId should be
  self-contained where it can. Skills can't (claude/kiro own
  their config), but an MCP's *runtime* can — it's an
  out-of-process service. So Node is bundled in the repo flake
  (`pkgs.nodejs_22`) and the launcher enters that flake via `nix
  develop path:<repo>` on each connect. Rejected alternative:
  user-PATH Node (e.g. a version manager) — works but isn't
  self-contained and made real e2e tests depend on the host. Net
  effect: Node off the user's PATH is fine; `nix` is the only
  prerequisite, and `direnv allow` in the repo brings the same
  `npx` for testing. Cross-environment coexistence with the
  sibling internal repo is still a deliberate follow-up.
- **Allowlist freshness: reconnect-rereads, not mid-session
  live** (PR #29). Considered a custom enforcement layer that
  checks each navigation against the live file so edits apply
  instantly; rejected for this feature — `--allowedUrlPattern`
  is launch-only and the tool exposes no runtime allowlist API,
  so live updates require building that layer. Chosen: the
  launcher re-reads the file each time the browser server
  (re)connects, so an edit applies on reconnect while Chrome and
  its logins stay up. Genuinely-live updates = deferred
  enhancement.
- **Browser-native allowlist management = deferred** (PR #29).
  Explored making the browser's own UI the default management
  surface; rejected for now — chrome://inspect is
  connection-level consent only, with no per-site allowlist UI
  to build on. The user-owned file stays the source of truth;
  a browser-native surface is a later abstraction.
- **`@latest` server pin kept** (code review nit). The launcher
  runs `npx chrome-devtools-mcp@latest` so it tracks the
  Chrome-team releases, as the spec chose. Noted risk: a future
  rename of the enforced-allow flag could silently break
  enforcement. Pinning a major is a possible later hardening;
  not changed now to avoid a silent deviation from the plan.
