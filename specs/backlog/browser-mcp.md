# Backlog: browser-mcp

## What

Add a browser-control capability so an agent can open,
navigate, fill forms, and submit on websites — driving
the user's real, already-logged-in Google Chrome on a
desktop envKind.

Tool: Google's official **chrome-devtools-mcp**
(github.com/ChromeDevTools/chrome-devtools-mcp, Apache-2.0).
Launched as `npx chrome-devtools-mcp@latest --autoConnect`.
`--autoConnect` (Chrome 144+) joins the user's **live,
already-authenticated** Chrome session over the DevTools
Protocol — it does NOT spawn a fresh browser or a throwaway
profile, so existing cookies/sessions are reused with zero
re-auth. This makes public, personal-login, and
Midway-gated internal sites all work uniformly: once Chrome
holds a live session, Midway is just HTML/JS to the agent.

Two deliverables, both in mAId:

1. **Skill** — `resources/content/skills/browser/SKILL.md`.
   Pure markdown; teaches the agent the chrome-devtools-mcp
   tools (navigate_page, new_page, fill, fill_form, click,
   take_snapshot, list_network_requests, …) and a safe
   driving loop: snapshot → act → verify. NO installer
   change needed — mAId's REGISTRY already symlinks
   `resources/content/skills` into both `.claude/skills`
   and `.kiro/steering/skills`.

2. **MCP registration** — new `Justfile` verbs
   (`install-mcp` / `uninstall-mcp` / `mcp-status`) that
   register the server via each harness's own CLI:
   - Claude Code:
     `claude mcp add -s user chrome-devtools -- \
       npx chrome-devtools-mcp@latest --autoConnect`
   - Kiro: equivalent entry in
     `~/.kiro/settings/mcp.json`.
   CLI-based (not a Rust JSON-merge) keeps the build-tool
   pure-symlink. Idempotent. Skips gracefully where there
   is no GUI Chrome or no `claude`/Kiro CLI.

### Safety section (must ship inside SKILL.md)

- **Domain allowlist** before driving sensitive sites.
- **Attended use only.** Do NOT point this at Midway in
  cron/unattended runs. For autonomous/scheduled agents
  touching internal services, use a registered agent
  identity per **A5 — Auth for AI Agents**
  (w.amazon.com/bin/view/Midway/PRFAQA5-AuthforAIAgentssecuringInternalServices/),
  not a piggybacked personal session.
- **Prompt-injection blast radius.** A hostile page — or a
  malicious internal wiki/ticket — can hijack an agent
  acting with the user's full Midway authority. The
  browser exposes all page content to the MCP client;
  Google's own README warns not to browse sensitive sites
  while remote debugging is open.
- **Officially-supported alternative for Midway:** Tappi
  Browser (w.amazon.com/bin/view/TappiBrowser/), an
  Electron agent workstation built to browse
  Midway-authenticated internal tools natively. Prefer it
  for internal-tool automation; chrome-devtools-mcp is the
  DIY equivalent for the user's own attended use.

## Why

- Closes the "agent stops at the browser boundary" gap for
  the user's own desktop workflows — open a dashboard,
  read it, click through a form, submit.
- **Google-native**, matching the user's Chrome: maintained
  by the Chrome DevTools org (npm author "Google LLC"),
  ~44k stars, releasing roughly daily as of mid-2026. The
  deprecated `@modelcontextprotocol/server-puppeteer` is
  Anthropic's, not Chrome-team — chrome-devtools-mcp is the
  right pick.
- **Portable across envKinds.** Lives in mAId, which
  compiles into whatever harness is in use, so the same
  skill + registration works on `darwin-kelasa` and
  `ubuntu-mane` and any future desktop envKind — unlike a
  this-box-locked home.
- **Reuses the live login.** `--autoConnect` joins the
  running session, so no separate auth flow and no
  short-lived-token problem (that problem only afflicts the
  spawn-fresh-Chrome-from-a-profile-dir mechanism, which we
  are not using).

## Scope / non-goals

- **Desktop envKinds only** (`darwin-kelasa`,
  `ubuntu-mane`). Headless al2/al2023 has no GUI Chrome to
  attach to; verbs no-op there by design — acceptable, this
  is "just a skill + MCP server."
- Harnesses: Claude Code + Kiro.
- NOT building a cross-machine CDP tunnel (headless box →
  remote desktop Chrome). `--autoConnect` is local-only;
  the agent runs on the desktop next to Chrome. Tunnel
  topology is a possible later extension, not this item.
- NOT building an A5 registered-agent identity. This item
  is attended, personal-session use only.

## Open questions

- **`--autoConnect` prerequisite.** It needs remote
  debugging enabled once via `chrome://inspect/#remote-debugging`
  on Chrome 144+. Confirm the exact one-time desktop setup
  and document it in the skill (or the Justfile verb
  prints it).
- **Kiro registration path.** Confirm Kiro's current MCP
  config location/shape (`~/.kiro/settings/mcp.json`
  `mcpServers` map) and whether Kiro has a CLI add command
  or needs a file write. If file-write, decide whether to
  reuse the marker-tagged merge pattern from
  Gorantls-agents `items/meshclaw/item.py`.
- **Allowlist mechanism.** chrome-devtools-mcp itself — is
  domain restriction a server flag, or purely a
  skill-prompt convention? Verify before claiming the
  guardrail is enforced vs. advisory.
- **Verify hook.** mAId has `resources/tests/run` driving
  `claude --print`. Can a browser skill be smoke-tested
  without a real desktop session, or is this
  desktop-manual-only?

## How to pick it up (desktop)

1. Branch from `main` (this backlog rides on `main`; the
   `feat/kaimux` / `feat/kiro-observation-only` lines are
   unrelated and 4 behind main).
2. Promote to `specs/feature/browser-mcp.md`, fill the
   open questions against a real Chrome 144+ desktop.
3. Add the skill, then the Justfile verbs; smoke-test by
   driving a public site, then a Midway page, attended.

## Trigger to promote

- User is on a desktop envKind (darwin-kelasa /
  ubuntu-mane) with Chrome 144+ and wants the agent to
  drive a real browser end-to-end.
- A concrete first workflow exists to test against (a
  specific form/dashboard), so the skill's driving loop and
  the safety guardrails can be validated on something real.
