# Feature: agent-session-orchestrator

## Git Setup

- Branch: `feat/agent-session-orchestrator` — design and
  initial implementation. **Not yet merge-ready** (see
  Implementation Plan → Open issues).
- Follow-up branch: `feat/agent-orch-fix` (off the same base)
  — picks up the picker-row redesign (multi-line rows with
  inline pane extract), end-to-end test hardening, real-tmux
  verification, and spec ↔ code reconciliation. Worktree at
  `/local/home/gorantls/tool-workplace/ai-workspace/mAId-agent-orch-fix`.
- Base: `main`

## Feature Brief

A **birds-eye dashboard** for every coding-agent session
running across the user's tmux server. One pane of glass
lists every wrapped agent — Claude, Kiro, anything else
launched through `agent-orch wrap` — with its current
lifecycle state, the tmux address (`session:window.pane`),
elapsed time since last status change, and a snippet of what
its pane currently shows. Pressing one key jumps the tmux
client to that pane.

The tool is observation-only: agents launch through a thin
wrapper that `execvp`s into the real CLI, hooks report
lifecycle events into a `sessions.json` registry, and a
single fzf-driven picker reads the registry plus live tmux
state to render the dashboard. No daemon, no IPC, no I/O
interception. The user keeps every normal tmux verb they
already know.

The shape of the experience, deliberately minimal:

- Launch a wrapped agent: `agent-orch wrap claude -- claude`
  inside any tmux pane.
- See every wrapped agent in one dashboard (`agent-orch` from
  any shell, or `<prefix> <KEY>` from within tmux once
  `setup --key <KEY>` has been run).
- Each dashboard row carries its status icon, tmux address,
  agent kind, cwd, elapsed time, and a 3-line excerpt of the
  pane's current content. A larger preview window on the
  right shows the focused row's last ~25 lines.
- Sorted by priority — agents needing attention float to the
  top, working agents sit in the middle, idle/stale agents
  sink to the bottom.
- Pressing `enter` jumps the tmux client to that pane; the
  picker stays alive so the user comes back to the same view
  with cursor and query preserved.
- Closing an agent's pane removes its row automatically; a
  fresh wrap appears within ~100ms.

This is intentionally narrower than [lazyclaude](https://github.com/any-context/lazyclaude),
[claude-dashboard](https://github.com/seunggabi/claude-dashboard),
or [workmux](https://github.com/raine/workmux)'s dashboard:
agent-orch doesn't manage sessions (no create / rename /
delete), doesn't route permission prompts, doesn't manage git
worktrees. It's the **inventory + jump table** layer those
tools all need first — picking and observing across whatever
sessions tmux already has, agent-agnostic, single-binary,
fits in `~2200` LOC of Rust. The richer flows are tracked as
follow-ups (see Prior art and Implementation Plan →
Follow-up).

Everything else in this document explains how that experience
is delivered, what guarantees it carries, and how it's tested.

---

## Requirements — launch experience

These cover what the user does to install and start using the
tool, in the order they hit them.

### Single binary, three user-facing verbs + bare invocation

```
agent-orch setup [--key X]   # install Claude hooks; --key binds <prefix> X
agent-orch teardown          # remove hooks + self-discover keybind
agent-orch wrap <kind> [--cwd <dir>] -- <agent-cmd> [args...]
agent-orch                   # open the picker (self-detects in/out of agent-orch session)
```

Plus four hidden internal verbs invoked only by external
systems shelling into the binary: `hook` (Claude lifecycle event
callback), `unregister` (tmux `pane-exited` target), `render`
(fzf `reload(...)` target — emits one item per session),
`peek` (fzf `--preview` target — dumps the last N lines of a
pane via `tmux capture-pane`). All carry `#[command(hide =
true)]` so they don't appear in `--help`.

`agent-orch doctor` is planned but deferred to a follow-up
ticket; see Implementation Plan → Follow-up.

### One-time setup

`agent-orch setup [--key <KEY>]`:

1. Append our five lifecycle hook entries
   (`UserPromptSubmit`, `PreToolUse`, `PostToolUse`,
   `Notification`, `Stop`) to `~/.claude/settings.json`, each
   tagged `"x-agent-orch-managed": true` so `teardown` can
   remove only ours later. Hooks fire on **every** `claude`
   invocation; the `hook` subcommand filters by
   `$AGENT_ORCH_PANE` (set only by our wrapper) and exits
   silently for bare-claude invocations, so the user's normal
   claude usage is unaffected. **`Notification` is required**
   for the `waiting` state — without it the dashboard can't
   distinguish "needs input" from "still working" and the
   priority sort stops being useful.
2. With `--key X`, also bind `<tmux-prefix> X` to
   `switch-client -t agent-orch` in the **prefix table** (not
   the root table — inner TUIs would race for root-bound keys).
   Re-running `setup --key Y` swaps cleanly; `teardown`
   self-discovers and removes any prefix binding whose action
   is `switch-client -t agent-orch`. No `--key` argument
   needed for teardown, no state file kept.

Both verbs are idempotent: re-running `setup` rewrites command
paths in case the binary moved but doesn't duplicate;
`teardown` no-ops on an absent settings file.

The keybind is live-only (lost on `tmux kill-server`).
Persistence across reboots is the user's job — bake the
`bind-key` line into `~/.tmux.conf` via home-manager / chezmoi
/ yadm / ansible-pull / whatever drives their dotfiles.

### Wrapping an agent

`agent-orch wrap <kind> [--cwd <dir>] -- <agent-cmd> [args...]`
typed inside a tmux pane:

- Refuses to run outside tmux (no `$TMUX_PANE`) — the registry
  is keyed by pane id, so we need it.
- Refuses to run without an agent command after `--`.
- If a record exists for the pane:
  - **Alive recorded pid:** refuse loud. User runs unregister
    or kills the prior agent.
  - **Dead recorded pid:** clean it up (run the prior kind's
    `cleanup`), proceed with the new wrap. Auto-replace
    handles the case where an agent crashed and `pane-exited`
    didn't fire.
- Inside the registry-mutation lock, the wrapper:
  1. Verifies no live double-register, replaces stale.
  2. Runs the kind's `prepare` (Kiro writes its project-scoped
     config; Claude is a pass-through).
  3. Pushes a fresh `Idle` Session record (no hooks have fired
     yet; the picker's pane snippet shows the agent's startup
     screen).
- Outside the lock:
  4. Installs the global tmux `pane-exited` hook (idempotent
     via marker file + tmux's own `set-hook -g` idempotence).
  5. Sets pane option `@agent-orch-pane` for future
     introspection.
  6. Sets `AGENT_ORCH_PANE` env var.
  7. `execvp`s the agent — wrapper process is replaced in
     place. The wrapper's pid becomes the agent's pid (POSIX
     guarantee). No parent process, no signal forwarding, no
     child-pid bookkeeping.

The wrapper inherits the calling shell's full env, so PATH
ordering is whatever the shell set. See _Design rationale →
PATH-resolved agent binary_ for why we don't intervene.

### Opening the picker

Bare `agent-orch` self-detects:

- **Inside the orchestrator session** (`$TMUX` set + `tmux
  display-message #{session_name}` == `agent-orch`) → run the
  event-driven picker body. (This is the path tmux took when
  it spawned `agent-orch` as the orchestrator session's
  startup command.)
- **Anywhere else** → ensure the orchestrator session exists
  (creating it detached if not), then `switch-client -t
  agent-orch`.

Fallback that always works: bare `agent-orch` from any shell
pane. Useful when no keybind is installed yet, or after a tmux
server restart.

`$AGENT_ORCH_TMUX_SOCKET` is honored as `-L <name>` on every
tmux invocation we make, so the integration script can target a
private tmux server without touching the user's real one.

### Tearing down

`agent-orch teardown`:

- Removes our tagged hook entries from
  `~/.claude/settings.json`. Empty arrays / objects are pruned;
  the file is removed entirely if the result is `{}`.
- Self-discovers and removes any prefix-table binding whose
  action is `switch-client -t agent-orch`.
- Leaves `sessions.json` and any wrapped panes untouched —
  in-flight agents keep running; they just stop reporting
  hook events.

---

## Requirements — runtime experience

These cover what the user sees and does once the picker is
open. The picker's job is to make "what's running where, doing
what" obvious without reading any single agent's pane in full.

### Lifecycle: four states, glyphs, and pane content

A four-state machine drives the row glyph. The states match
the proven workmux/lazyclaude shape — they're the smallest
set of distinctions that lets the user **prioritize attention
across agents** at a glance:

| State   | Glyph | Color   | Triggered by                                              |
|---------|-------|---------|-----------------------------------------------------------|
| waiting | `💬`  | yellow  | `Notification` event (permission / approval prompt)       |
| working | `▶`   | green   | `PreToolUse` fired without a matching `PostToolUse` yet   |
| done    | `✓`   | dim     | `Stop` / `PostToolUse` after a `UserPromptSubmit`         |
| idle    | `·`   | gray    | `done` + idle for >60s, OR no events ever fired           |

Sort priority (top → bottom):
**waiting > working > done > idle**, ties broken by
most-recently-active first. A "needs me right now" prompt
floats to the top; a long-idle agent sinks. Same approach
workmux's dashboard takes; same reason it works.

Hook event mapping (`Session::apply_event`):

- `UserPromptSubmit` → state := working transitionally; this
  is the moment the user handed Claude a task. If no
  `PreToolUse` fires within ~3s (Claude answered from
  context with no tool calls), the next event drops it to
  `done`. Stamps `last_event_ts`.
- `PreToolUse` → state := working.
- `PostToolUse` / `PostToolUseFailure` → state := working
  if more `PreToolUse` events follow (still in the loop), or
  → state := done after `Stop`.
- `Notification` → state := waiting. **Highest priority**
  state — these are permission prompts the user must
  approve. The pane snippet shows the prompt body so the
  user can decide without switching panes (whether to
  approve from the dashboard is a follow-up — see Prior art
  → lazyclaude).
- `Stop` → state := done. After 60s without further events,
  the render-time decay logic flips done → idle (so a
  forgotten agent fades visually).

`apply_event` always bumps `last_event` and `last_event_ts`
unconditionally — used by sort and by the "elapsed" hint
(`5s ago`, `2m ago`, `1h ago`).

**Implementation note.** The done → idle decay is computed at
render time (`State::display(&self, now: u64) -> DisplayState`),
not stored in `sessions.json`. This way a session left running
in a forgotten tmux pane doesn't need any background process
to keep flipping its state file every minute; the picker just
reads `now - state_ts` and decides.

### Picker row schema

Each picker entry is a **multi-line item**: one header line
plus a 3-line snippet of the agent's pane. fzf items are
null-delimited (`--read0`); `--gap 1` renders a blank visual
line between items.

```
💬 proj-c:0.1     claude  proj-c        45s
  Allow Bash command "rm -rf node_modules"?
  [y/n/always allow]
                                              ← gap line
▶  proj-b:code.0  claude  proj-b        12s
  $ cargo build --release
     Compiling agent-orch v0.1.0
     Finished `release` profile [optimized] in 12.3s

✓  proj-a:0.0     claude  proj-a        2m
  > Done. The failing test in tests/state.rs:42 was caused
    by a stale fixture; updated and re-ran the suite.

·  proj-c:0.0     kiro    proj-c        1h
  Ready. Type a message…
  > █
```

Header line layout (tab-separated, fixed column widths so
the picker scans cleanly at any width):

```
<icon> <session>:<window>.<pane>  <kind>   <cwd-tail>  <elapsed>
```

- `<icon>` is `💬` waiting / `▶` working / `✓` done / `·`
  idle — colored via ANSI escapes so the column carries
  status at-a-glance even before the eye reaches the text.
- `<session>:<window>.<pane>` is `#S:#I.#P` from tmux —
  human-typeable. (Internally, fzf still tracks rows by tmux's
  `%N` pane-id via `--with-nth=2.. --id-nth=1`; the address is
  what the user reads.) Resolution is best-effort — if tmux
  doesn't return one (server gone, pane closed mid-render),
  the row falls back to `?:?.<pane-id>` so the row still
  appears.
- `<kind>` is `claude` / `kiro` / whatever was wrapped.
- `<cwd-tail>` is the last segment of the cwd. Disambiguates
  two agents sharing a session name across different paths.
- `<elapsed>` is `now - last_event_ts` rendered as `5s` /
  `2m` / `1h` / `3d`. Live signal that the agent is making
  progress (or stuck).

Snippet lines are the last 3 visible lines of `tmux capture-
pane -p -e -t <pane> -E -1 -S -3`. The `-e` flag preserves
ANSI escape codes so colored output (claude's banner, build
errors, etc.) renders in the picker. Each snippet line is
indented two spaces so eye-tracking distinguishes it from
the next session's header.

**No pid in the row.** Earlier draft included `pid=N`; in
practice the user never needs it from the dashboard, and it
crowded the line. Available via `agent-orch render` JSON for
scripting (follow-up).

### What `render` emits

`agent-orch render` writes one item per live session to stdout,
items separated by `NUL` (`\0`). Each item is exactly:

```
<pane_id>\t<header line>\n<snippet line 1>\n<snippet line 2>\n<snippet line 3>
```

- The leading `<pane_id>\t` is what fzf `--id-nth=1` keys on;
  `--with-nth=2..` hides it from the display.
- The four lines are joined by `\n` inside the item; `\0`
  separates items.
- An item dropped to <3 snippet lines (e.g. fresh pane, brief
  output) pads with empty lines so item heights stay uniform —
  prevents fzf from re-laying-out on every reload.

### Sort order — priority-based, like a triage queue

Sort by priority (top → bottom):

1. **waiting** (most-recently-waiting first) — these agents
   are blocking the user; they go first.
2. **working** (most-recently-active first) — actively in a
   tool call; the user might want to peek.
3. **done** (most-recently-finished first) — just completed
   a turn, output worth scanning.
4. **idle** (most-recently-active first) — long quiet, sinks
   to the bottom.

A just-finished agent (state=done, glyph=`✓`) stays near the
top of the done bucket so the user sees the result; after 60s
of further quiet it decays to idle and drifts down.

### fzf bindings and refresh model

The picker spawns fzf once with these flags:

- `--listen=<sock>` — Unix socket fzf listens on for HTTP/1.1
  control commands.
- `--read0` — items are null-delimited (allows multi-line
  items).
- `--gap=1` — one blank visual line between items.
- `--highlight-line` — highlight all lines of the focused
  item, not just the header.
- `--with-nth=2..` — hide the pane id (column 1) from
  display.
- `--track --id-nth=1` — keep the cursor on the same pane id
  across reloads.
- `--ansi` — render the ANSI escapes captured by `peek`'s
  `-e`.
- `--preview '<self> peek {1}' --preview-window=right:50%` —
  the side preview shows ~25 lines for the focused row,
  complementing the row's inline 3-line snippet.
- `--header='enter jump · p peek · x kill record · / filter · esc exit'`
  — one-line cheatsheet (tmux-tea / lazyclaude pattern).

Keybindings (intentionally narrow — every key shipped is one
the user is sure to use):

| Key      | Action                                                   |
| -------- | -------------------------------------------------------- |
| `enter`  | switch tmux client to the focused pane; picker stays up |
| `p`      | "peek" — temporarily attach to the focused pane in a tmux popup, return on `q` |
| `x`      | unregister the focused row (drops it from picker; does NOT kill the agent process) |
| `/`      | fzf's built-in filter — type to narrow rows             |
| `esc`    | exit picker (orchestrator session closes)                |

Bind shapes:

- `--bind 'enter:execute-silent(tmux switch-client -t {1})+clear-query'`
  — non-terminal binding. fzf stays alive across selections.
- `--bind 'p:execute(tmux display-popup -E "tmux attach -t {1}")'`
  — read-only peek without leaving the dashboard. Returns
  to the picker on `q` / popup close.
- `--bind 'x:execute-silent(<self> unregister {1})+reload(<self> render)'`
  — drop the wrapped record. Useful for crashed-but-stuck
  rows. Killing the agent process itself is intentionally
  *not* bound — the orchestrator is observation-only; if the
  user wants to kill a process, they jump there with
  `enter` and do it.

Deferred (tracked under Implementation Plan → Follow-up):
input-mode key (forward keystrokes to the focused pane
without switching), inline-approval of `Notification`
prompts, view-mode toggle (worktrees / sessions / agents).
These are good ideas; v1 ships with `enter` / `p` / `x` only
to keep the surface honest.

Two background threads drive updates over the listen socket:

- **Watcher.** `notify-debouncer-mini` watches the store dir;
  100ms debounce. When `sessions.json` actually changes (a
  hook event landed, an unregister fired), POST `reload(<self>
  render)` to fzf. The list refresh is needed because rows can
  appear / disappear / change state, AND because each row's
  inline snippet is regenerated from the latest pane content.
- **Heartbeat.** Every 1 second, POST `refresh-preview` to
  fzf. This re-runs the side preview command for the focused
  row but does **not** touch the list — the cursor stays put,
  the query stays put, the prompt stays bright. The row's
  inline snippet only refreshes on the next `reload` (i.e.
  the next real registry change), so the row text is
  "freshest at last event" rather than "live."

Trade-off, recorded so a future reader doesn't redo the
debate: the row snippet could be live too if heartbeat sent
`reload(...)` instead of `refresh-preview`, but `reload(...)`
blocks fzf's input briefly while it re-runs the source — at 1
Hz that produces continuous flicker. Splitting the two actions
gives us live preview window + stable list rows; the inline
snippets exist to make the at-a-glance scan informative even
without focusing a row, not to be a real-time feed.

### Self-detection of inside vs. outside

- `$TMUX` set + `tmux display-message -p '#{session_name}'` ==
  `agent-orch` → run `Loop::body` (we're inside the
  orchestrator session, tmux just spawned us).
- Otherwise → run `Loop::run` (ensure session exists,
  `switch-client -t agent-orch`).

When fzf exits (Esc / kill), the body returns and the
orchestrator session terminates.

### "Back to orchestrator" UX

`agent-orch setup --key <KEY>` binds `<tmux-prefix> <KEY>` to
`switch-client -t agent-orch` in the **prefix table**, not the
root table. Inner TUIs (claude/kiro) consume root-bound keys
inconsistently; prefix bindings are the standard tmux idiom
(`<prefix> c`, `<prefix> "`, `<prefix> d`) and are reliably
intercepted before any inner program sees them.

---

## Requirements — lifecycle behavior

### Hook subcommand (`agent-orch hook <event>`)

- Reads JSON payload from stdin (Claude's hook protocol).
- Filters first on `$AGENT_ORCH_PANE`; if unset, exits 0
  silently — handles bare-claude invocations after `setup`
  installed the user-global hooks.
- Otherwise applies `Session::apply_event` to the matching
  registry record under `flock`.
- **Always exits 0**, even on internal errors. A failing hook
  reporter must never block the agent's turn.

### Registry cleanup

- **`pane-exited` (tmux global hook)** — installed by the
  wrapper on first `wrap`, fires per closing pane, calls
  `agent-orch unregister #{hook_pane}`.
- **Liveness probe at render time** — `kill(pid, 0)` over
  every record. Drops dead-pid rows from the picker even if
  `pane-exited` didn't fire (server restart, etc.).
- **Per-kind cleanup** runs inside `unregister` via the
  matching `Wrapper` impl. Kiro removes
  `<cwd>/.kiro/agents/agent-orch.json` if no other live Kiro
  session shares the cwd (refcount-agnostic). Claude is a
  no-op.

### Kiro is observation-only in v1

Kiro panes register and appear in the picker, but their
`state` stays `idle` because we don't drive Kiro hooks. Kiro
hooks live inside agent persona JSONs
(`~/.kiro/agents/<name>.json`) using a different schema than
Claude — camelCase events, inline `{matcher?, command,
timeout_ms?}` shape, no nested `hooks` array. Wiring them
without modifying the user's chosen agent persona is a
follow-up; see Implementation Plan → Follow-up — Kiro state
tracking. Lifecycle cleanup (`pane-exited` → unregister) still
works, the wrapper still execvp's the kiro CLI, and the
picker's pane snippet still surfaces what kiro is doing
visually.

### Storage and concurrency

A single `sessions.json` is the source of truth for both
registration and state. Every mutation is gated by a POSIX
advisory lock (`fd-lock`). The hook subcommand writes state on
every fired event; the tmux `pane-exited` hook removes the
record on pane death. Atomic write = per-pid tmp + rename, no
fsync (state-dir scratch).

State lives at `${XDG_STATE_HOME:-$HOME/.local/state}/agent-
orch/`.

### `sessions.json` shape

One record per wrapped pane:

```json
{
  "pane_id": "%17",
  "pid": 274317,
  "kind": "claude",
  "cwd": "/tmp/proj-a",
  "started": 1780619367,

  "state": "running",
  "state_ts": 1780619400,

  "last_event": "PreToolUse",
  "last_event_ts": 1780619400,

  "created_kiro_config": false
}
```

The picker entry is derived from these fields plus a live
`tmux display-message`/`capture-pane` lookup at render time
(for the human-readable address and the snippet). All fields
after `state_ts` carry `#[serde(default)]` so legacy registries
deserialize cleanly — extras are ignored.

---

## Hard constraints

- **No deployed `$HOME` writes from `deno task deploy`.** The
  binary is invoked from `<repo>/dist/agent-orch/`. Per-user
  runtime state at
  `${XDG_STATE_HOME:-$HOME/.local/state}/agent-orch/` is
  acceptable — runtime data has to live under `$HOME`; the
  invariant is about *deployed code/config* (no symlinks
  installed by the registry).
- **`~/.claude/settings.json` is touched only by explicit
  user-invoked `setup` / `teardown`**, never by `wrap` or any
  other implicit path. Both verbs are idempotent and
  tag-scoped — they edit *only* entries marked
  `"x-agent-orch-managed": true`, leaving every other field of
  the user's settings file alone.
- **No `<cwd>/.kiro/agents/agent-orch.json` orphans.** The
  refcount-agnostic cleanup contract is the load-bearing rule
  even though Kiro state-tracking is out of scope for v1.
- **Observation only.** The wrapper `execvp`s the agent; the
  hook subcommand writes a state file; neither sits in the
  agent's I/O path.
- **Public-repo hygiene** (mAId-wide). No internal product /
  team / ticket names in scripts, examples, or this spec.
- **Standalone first.** No mAId registry entry in v1. The
  binary is a plain Rust executable at `dist/agent-orch/`,
  produced by `cargo build --release`. Future packaging is
  purely additive.

## Prior art

Adjacent tools we sanity-checked the design against. None
solve the same problem; recording what each does well so
the spec lands at "smallest thing that fills the gap" rather
than "yet another tmux dashboard."

- **[tmux-tea](https://github.com/2KAbhishek/tmux-tea)** —
  fzf-driven tmux session picker (directories + sessions).
  No agent awareness; no live state. We borrow its
  `tmux capture-pane -ep` preview pattern, the
  `--header` cheatsheet line, and the multi-line item
  styling.

- **[workmux dashboard](https://github.com/raine/workmux)** —
  closest fit. tmux + git worktrees + agent status in a
  single TUI, with `🤖 working / 💬 waiting / ✅ done` icons
  driven by hooks, priority-sort, live preview, peek-without-
  leaving (`p`), kill-key. We borrow the **four-state
  model + priority sort + elapsed time + peek key** more or
  less wholesale because workmux has proven they work; we
  do *not* take on its worktree management, PR review, diff
  staging, or input mode (yet). Workmux is the right model
  for "manage parallel agent sessions"; agent-orch is
  scoped to "observe across whatever sessions tmux already
  has", so the surfaces stay disjoint.

  **Considered: fork workmux as our base instead.** Concrete
  numbers: workmux is ~447 KB across 167 `.rs` files with 49
  deps (ratatui TUI, daemon-over-UDS, custom backends for
  tmux/kitty/wezterm/zellij, sandbox runner, LLM-driven
  naming, GitHub PR integration). Our binary today is
  ~2.2 K LOC in one file with 7 deps. Forking buys us:
  sidebar, broader agent coverage (gemini / codex /
  copilot already wired), and shipped live-preview / kill /
  sweep / theme. It costs us:
  (a) a much wider install contract — workmux's setup
  writes `~/.workmux.yaml`, `~/.config/workmux/`, tmux
  `window-status-format` hooks, and a Claude plugin
  marketplace entry, which collides with mAId's "no
  deployed `$HOME` writes" hard constraint;
  (b) ongoing rebase tax against an upstream whose
  philosophy is worktree-shaped — every workmux release
  we'd ask "does this assume worktrees we don't have";
  (c) a fundamentally different `wrap` model — workmux
  creates the tmux windows, ours registers whatever pane
  already exists, and that's the load-bearing UX choice
  the user explicitly wants;
  (d) Kiro is already broken upstream, so the fork doesn't
  solve our deferred Kiro slice.
  The conclusion: **borrow the patterns, keep the shape.**
  Crediting workmux for the four-state model is the right
  relationship; pulling in their daemon + ratatui + 49
  deps to replace 2.2K LOC of our own code is not. If a
  year from now we want sidebar / worktree / sandbox
  flows, the right move is to **detect-and-defer** to a
  user's existing workmux install (additive
  integration), not to fork.

- **[lazyclaude](https://github.com/any-context/lazyclaude)**
  — Claude-only TUI (lazygit-shaped). Adds permission-
  prompt overlays, scrollback browser, MCP/plugin
  management, PM/Worker multi-agent orchestration.
  Significantly larger surface than agent-orch wants. We
  watch its inline-approval popup pattern as the **likely
  v2 shape** for handling `Notification` from the
  dashboard without switching panes — tracked under
  Implementation Plan → Follow-up.

- **[claude-dashboard](https://github.com/seunggabi/claude-dashboard)**
  — k9s-style TUI for Claude sessions, with conversation-
  log viewer, CPU/memory monitoring, and bulk-kill-idle.
  Useful precedent for the "single pane of glass" framing
  but the implementation is heavier (~2s polling daemon, Go
  binary, custom session manager). The conversation-log
  viewer is interesting; we don't pursue it because
  claude's `~/.claude/projects/<cwd>/<session>.jsonl`
  format is not stable across versions and parsing it
  inside `agent-orch peek` would couple us to a moving
  target. Out of scope for v1.

The throughline: every adjacent tool ships *more* — session
management, worktree workflows, in-app approvals, log
parsing. agent-orch deliberately ships *less* to be the
common substrate any of them could sit on top of.

## Runtime prerequisites

The compiled binary is environment-agnostic. It needs:

- `tmux` ≥ 3.2 on PATH (for `set-hook`, `set-option`,
  `bind-key`, `switch-client`, `display-message`).
- `fzf` ≥ 0.71.0 on PATH (`--listen=<sock>` ≥ 0.66.0,
  `--track --id-nth=N` ≥ 0.71.0, `--read0 --gap` ≥ 0.55.0).
- The wrapped agent's CLI on PATH (`claude`, `kiro-cli`, etc.)
  for the kinds the user actually wraps.
- A writable `$HOME` (for `setup` / `teardown` to edit
  `~/.claude/settings.json`) and a writable
  `${XDG_STATE_HOME:-$HOME/.local/state}/agent-orch/`.

Anything else (`flake.nix`, `nix develop`, `direnv`,
`rust-overlay`) is **build-time tooling for mAId
contributors**, not a constraint on users running the binary.

---

## Test Strategy

Three layers, mapped onto `project.md`'s test surface. Each
layer covers what the layer below can't reach.

### Unit (`deno task agent-orch:test`) — load-bearing

Delegates to `cargo test`. Tests live at the bottom of
`src/main.rs` in `#[cfg(test)] mod tests`. Each typeclass is
exercised through its own surface against a tempdir `Store`.

#### State machine + picker row schema

- **`Session::apply_event`** —
  - `PreToolUse` → working; bumps `last_event_ts`.
  - `PostToolUse` / `PostToolUseFailure` → working (more
    tools may follow) — verified by the next `Stop` flipping
    to done.
  - `Notification` → waiting; `last_event_ts` bumped.
  - `Stop` → done; `last_event_ts` bumped.
  - `UserPromptSubmit` → working transitionally;
    `last_event_ts` bumped.
  - Unknown events bump `last_event_ts` only, no state
    change.
- **`State::display(now)` decay** — done + (now - state_ts >
  60s) → renders as idle. Pure-function; same input ⇒ same
  output.
- **Sort order** — waiting > working > done > idle; within a
  group, most-recently-active first. Stable when two records
  share a timestamp. A waiting record always outranks a
  working one even if it's older.
- **Header-line shape** — given a session record + resolved
  address + computed elapsed, the formatter emits
  `<icon>\t<addr>\t<kind>\t<cwd-tail>\t<elapsed>`. Each field
  appears, in the right order. ANSI color codes are present
  on the icon (proves `--ansi` will render the right hue).
  Address-resolution failure falls back to `?:?.<pane-id>`
  without erroring. `cwd_tail` root-anchored edge cases
  (root, empty, single segment). `format_elapsed` covers
  `<60s` (`Ns`), `<60m` (`Nm`), `<24h` (`Nh`), `≥24h`
  (`Nd`).
- **Item assembly (`render`)** — given a list of sessions and
  an injected pane-content fn (returning canned strings), the
  output is one `\0`-separated item per session, each item =
  `<pane_id>\t<header>\n<line1>\n<line2>\n<line3>`. Snippet
  shorter than 3 lines pads to 3. Snippet with embedded `\0`
  (defensive) is sanitized — stripped or replaced — so the
  null delimiter remains unambiguous.
- **`render` after state change** — registry mutated via
  `apply_event` then rendered; output reflects the new icon,
  the new elapsed time, and the re-sorted order.
- **`render` priority sort** — list of (waiting, working,
  done, idle) sessions with mixed timestamps renders in
  priority order, ties broken by recency.
- **`render` after unregister** — registry has the row
  removed; it disappears from output. Other rows untouched.
- **`render` after fresh wrap** — registry has a new record
  appended; it appears in output, sorted by activity.

#### Store + Wrapper

- **`Store::read`** — empty / missing / malformed file.
- **`Store::mutate`** — round-trips, observes prior state,
  releases lock on panic.
- **`Claude::prepare`** — pass-through; argv unchanged.
- **`Kiro::prepare`** — writes
  `<cwd>/.kiro/agents/agent-orch.json` first time, `created`
  flag false on reuse.
- **`Kiro::cleanup`** — keeps config while sibling alive;
  removes refcount-agnostically on close-creator-first
  ordering.
- **`wrap()`** — refuses double-register on alive pid; replaces
  stale record on dead pid (runs prior kind's cleanup); refuses
  empty agent argv.
- **`hook()` default body** — filters on unknown pane (silent
  no-op); applies state correctly via both Claude and Kiro
  impls (proves default-method inheritance).

#### `setup` / `teardown` JSON merge

- Creates settings with tagged entries; preserves user-existing
  entries and appends ours; idempotent (no duplicates on
  re-run); refreshes command path on re-run; rejects non-object
  root + non-array event; teardown removes only tagged entries;
  teardown removes the file when only our content remained;
  full setup → teardown round-trip restores pre-state
  byte-for-byte.

Quality gate: `cargo fmt --check` + `cargo clippy --all-targets
-- -D warnings`.

### Integration (`deno task agent-orch:integration`) — load-bearing

`tests/agent-orch/integration.sh` drives the compiled binary
against a **private tmux server** (`tmux -L
agent-orch-test-$$`) with `XDG_STATE_HOME` pointed at a
tempdir. Exercises real tmux side effects (`set-hook`,
`set-option`, `switch-client`, `bind-key`, `display-message`,
`capture-pane`) and real argv handling on a live process —
the half of the system unit tests can't reach.

Cases:

1. `wrap claude` registers a session and stamps the pane id.
2. `hook UserPromptSubmit` records the event (state stays
   working/done depending on prior).
3. `hook PreToolUse` flips state to working.
4. `hook PostToolUse` keeps state working (more tools may
   follow); `hook Stop` flips state to done.
4b. `hook Notification` flips state to waiting (highest
    priority — sorts above working in render).
4c. **State decay (done → idle).** Set a `Stop` event with
    `state_ts = now - 90`. Render → row's icon is `·`
    (idle), since 90 > 60s decay threshold. Same record
    with `state_ts = now - 30` renders as `✓` (done).
5. **`render` emits a `\0`-separated item per registered
   pane** in priority-sort order. Each item parses as
   `<pane_id>\t<header>\n<l1>\n<l2>\n<l3>`. Header carries
   the icon (with ANSI color escapes), the
   `session:window.pane` address resolved via `tmux
   display-message`, the kind, the cwd tail, and the
   elapsed-time field. Snippet lines reflect the pane's
   actual content (asserted after sending a
   `MARKER_RENDER_42` echo into the pane).
6. **`render` reflects state transitions live.** A `hook
   PreToolUse` flips the rendered icon to `▶`; a subsequent
   `hook Stop` flips it to `✓`; a subsequent `hook
   Notification` flips it to `💬` and the row sorts to the
   top. Address, elapsed-time, and snippet update each call.
7. **`render` reflects new wraps and unregisters live.** Wrap
   a second pane → render now emits two items. `unregister
   <P1>` → render emits one. Removed pane id absent.
8. `unregister <pane>` removes the record.
9. **Kiro refcount-agnostic cleanup** survives close-creator-
   first ordering.
10. `wrap` refuses without `$TMUX_PANE`.
11. The global tmux `pane-exited` hook is registered with the
    `unregister` command after first `wrap`.
12. `setup` / `teardown` round-trips on a fresh `$HOME`;
    preserves user-existing entries through the round-trip.
    `setup` installs the `Notification` event entry alongside
    the others.
13. **Keybind round-trip on a live tmux server.** Split into
    13a (setup --key X installs `<prefix> X`), 13b (re-keying
    --key Y swaps X→Y cleanly), 13c (teardown
    self-discovers and removes the binding without --key),
    13d (setup without --key installs hooks only and leaves
    the prefix table untouched).
14. **`peek` with ANSI** — send a colored marker into the
    pane, assert `peek --lines N` output contains both the
    visible text AND the escape codes (proves `-e` flag is in
    place).

CI environments without tmux/jq/the dist binary skip silently
(exit 0).

### Functional (`tests/agent-orch/functional-*.sh`) — user-driven

Three scripts that drive the **user's real tmux server** with
real claude/kiro-cli CLIs, exercising the end-to-end loop:

- `functional-setup.sh <KEY>` — spawn the four-session fixture
  + install hooks + bind `<prefix> <KEY>`.
- `functional-test.sh` — fire dummy queries at the wrapped
  agents via `tmux send-keys`, poll the registry, assert state
  transitions reflect actual agent activity, assert `render`
  output reflects the same state.
- `functional-teardown.sh` — kill the four sessions, run
  `agent-orch teardown`, clear the registry. Idempotent.

The fixture (four sessions on the live server, untouched by
integration tests):

| Session      | Layout                                                                               | What's wrapped                  |
|--------------|--------------------------------------------------------------------------------------|---------------------------------|
| `proj-a`     | 1 window, 1 pane                                                                     | claude                          |
| `proj-b`     | 2 windows. Window 2 (`code`) has horizontal split — **two claudes side-by-side**     | claude × 2 (same cwd)           |
| `proj-c`     | 1 window, vertical split — kiro top, claude bottom                                   | kiro + claude (same cwd)        |
| `agent-orch` | the orchestrator session itself; bootstrapped detached                               | the picker (fzf body)           |

Scenarios the functional test asserts. Numbered to match the
user's mental model of the loop:

#### F1. Setup spawns the multi-session fixture cleanly

After `functional-setup.sh O`:

- `tmux ls` shows all four sessions: `proj-a`, `proj-b`,
  `proj-c`, `agent-orch`.
- `~/.local/state/agent-orch/sessions.json` has 5 records (1
  + 2 + 2 + 0 — the orchestrator session itself isn't
  wrapped).
- Each record's `pane_id` resolves to a live pane via `tmux
  list-panes -a -F '#{pane_id}'`.
- `~/.claude/settings.json` carries our six tagged hook
  entries.
- `tmux list-keys -T prefix` shows `<prefix> O →
  switch-client -t agent-orch`.
- `agent-orch render` emits 5 items, each one parses as the
  documented `<pane_id>\t<header>\n<l1>\n<l2>\n<l3>` shape.

#### F2. Dummy queries propagate to sessions.json AND the picker

For each wrapped Claude pane:

- `tmux send-keys -t <pane> 'list files in cwd' Enter`,
  followed by a few seconds of polling.
- Assert the matching `sessions.json` record passes through
  state=`working` (during `Bash` tool execution) and settles
  to state=`done` within N seconds of completion. (After
  60s of further quiet, render output flips the icon to
  `·` idle — the decay layer.)
- Assert `agent-orch render` output for that row reflects
  the same transitions: `▶` icon during work, `✓` immediately
  after, `·` after the 60s decay window.
- Assert the row's snippet lines change between the
  pre-prompt sample and the post-completion sample (the pane
  content moved on).
- Assert the elapsed-time field advances monotonically while
  the agent is in `done` state (`5s` → `15s` → `45s` → `1m`).

For Kiro: the row stays at state=`done`/`idle` (Kiro hooks
are out of scope in v1, so no `Notification` / `PreToolUse`
events ever fire), but the snippet lines still update
visibly as kiro responds — proving the inline-extract path
is kind-independent.

The narrow window on working→done matters: a fast tool may
flip working→done inside a single poll cycle, so the test
polls at high frequency (every 100ms) for up to 30s.

#### F2b. Permission prompts surface as `waiting` and sort to the top

Drive a Claude pane to a state where it asks for tool
approval (the cleanest reproduction is a `Bash` command on
a write the user hasn't pre-allowed). When Claude emits the
`Notification` hook event:

- Within 1s: the matching `sessions.json` record is
  state=`waiting`, last_event=`Notification`.
- `agent-orch render` puts that row **at the top of the
  output** (priority sort), with the `💬` icon.
- The row's snippet lines reflect the prompt body — the
  user can read the pending approval question without
  switching panes.
- Once the user approves (`tmux send-keys -t <pane> '1'
  Enter`) and Claude proceeds: state flips to `working` then
  `done`, row drops down the list as priority decreases.

If the agent is unable to actually issue a Notification
event in the test environment (no allowlist behavior
available), this scenario is documented as **deferred** with
an explicit log line, not a hard failure. Same gating
philosophy as the rest of the functional layer.

#### F3. Two agents in one window track independently

In `proj-b:code` (the horizontal split with two claudes):

- Submit different prompts to the left and right panes.
- Assert both rows have their own state and `last_event_ts`
  without cross-contamination.
- Assert their snippets are visibly different (each pane saw
  a different prompt).

#### F4. Mixed kinds in one window

In `proj-c` (vertical split, kiro top, claude bottom):

- Prompt the bottom (claude) — its row advances normally
  through working → done, icon and snippet update.
- Send a query to the top (kiro) — its row's state stays
  flat (no Kiro hooks fire, so it remains at whatever its
  last hook event left it — typically `done` or never
  flipped), but its snippet updates.
- Assert both panes are present in the registry and in
  `render` output throughout.

#### F5. Closing a wrapped pane removes its row from sessions.json AND the picker

Pick a wrapped pane (e.g. `proj-a`'s sole pane).

- Capture the pre-state: count of records in `sessions.json`,
  count of items in `agent-orch render`.
- `tmux kill-pane -t <pane>`.
- Within 2s: `sessions.json` no longer has the record (the
  global `pane-exited` hook fired `agent-orch unregister`).
- Within 2s: `agent-orch render` no longer emits an item for
  that pane id.
- Other rows' state and snippet are unchanged.

If the killed pane was the sole one in its session, the tmux
session goes away too — re-running `agent-orch render` proves
the row is gone from the picker. (The picker process would
also have observed a watcher tick and reload-ed.)

For Kiro specifically, F5 also verifies:

- If the killed kiro pane was the last kiro session in its
  cwd: `<cwd>/.kiro/agents/agent-orch.json` is removed.
- If there's a sibling kiro session in the same cwd: the
  config persists.

#### F6. Wrapping a fresh agent appears in sessions.json AND the picker

From a non-fixture pane (or a fresh tmux session):

- `agent-orch wrap claude -- claude`.
- Within 2s: `sessions.json` has a new record matching the
  pane id, state=`done` (no events have fired yet — the
  initial state is `done` so the row appears in the
  middle-priority bucket rather than blocking the user's
  attention as `working` would).
- Within 2s: `agent-orch render` emits a new item for that
  pane id, with the right kind/cwd and a `✓` icon (or `·`
  if the elapsed time exceeds the 60s decay window before
  the assertion).
- The new row's snippet reflects the agent's startup screen
  (pre-first-prompt content from `tmux capture-pane`).

#### F7. Both agentic-state AND tmux-pane content surface

This is the load-bearing UX assertion: the picker conveys
two independent signals per row, one from sessions.json
(lifecycle state) and one from tmux (pane content snippet).

For one wrapped Claude pane:

- Capture `sessions.json` state and the row's first snippet
  line. Assert they agree on the agent being inactive
  (state == done or idle by decay, snippet == prompt cursor
  or similar).
- Send a prompt that forces a tool. While the tool runs:
  - state == working (from sessions.json)
  - snippet shows the tool's in-flight output (e.g. `cargo
    build` progress) — clearly different from the idle
    snippet.
- After completion:
  - state == done (from sessions.json)
  - snippet shows the post-completion line (different from
    both the pre-prompt and the in-flight snippets).
- The two signals are independent — verified by stopping
  the hook reporter (e.g. unsetting `AGENT_ORCH_PANE` for
  the pane) and asserting that the snippet still updates
  while state is frozen at its last-known value.

#### F8. `<prefix> KEY` round-trip, dead-pid filter, stale-shell PATH

- `<prefix> O` from a non-orchestrator pane lands the client
  in the `agent-orch` session. Verified by reading `tmux list-
  keys -T prefix` (rather than synthesizing keystrokes —
  too flaky).
- Kill an agent's pid directly (`kill -9 <pid>`); on next
  render the row is gone (dead-pid filter).
- Document, don't assert: tmux-resurrect-restored panes can
  resolve `claude` to whichever `claude` binary the
  resurrected shell's PATH happened to put first, which on
  some setups is not the binary the user wanted. Functional
  fixture starts each session fresh (no resurrect), so this
  is a known env-side risk rather than an assertion.
  `agent-orch doctor` (follow-up) surfaces the mismatch.

Functional tests **gate on prerequisites being available**:
tmux, fzf, claude, and kiro-cli on PATH. Missing any → skip
with a clear message. They are part of the **dev loop** when
the build environment has those CLIs available — see
`project.md`'s Testing section.

### What's deliberately not tested

- Concurrent hook fires at high rate (multiple agents firing
  events in the same millisecond — integration script
  exercises the lock under serial load; a real stress test
  would need many real agents, which is infeasible in CI).
- Cross-platform (macOS / Linux flock semantics differ
  slightly but POSIX advisory locks behave the same on both
  in practice; we don't run macOS CI).
- The fzf keybind itself firing `tmux switch-client` — the
  integration script verifies the binding is registered;
  whether tmux dispatches it correctly is tmux's
  responsibility.
- The fzf picker's visual rendering (multi-line item layout,
  `--gap` spacing, ANSI-color rendering). What `render`
  emits is asserted in unit + integration; what fzf does
  with that string is fzf's contract, exercised manually by
  the user when they `tmux attach -t agent-orch` after
  `functional-setup.sh`.

---

## Design

### File layout

```
sources/agent-orch/
├── Cargo.toml                    deps: anyhow, clap, fd-lock, nix,
│                                       notify, notify-debouncer-mini,
│                                       serde, serde_json
└── src/main.rs                   single file, ~2200 LOC including tests

tests/agent-orch/
├── integration.sh                private-server shell-driven E2E (load-bearing)
├── functional-setup.sh           live-server fixture spawn (user-driven)
├── functional-test.sh            live-server prompt-driven assertions (user-driven)
└── functional-teardown.sh        live-server fixture cleanup (idempotent)

dist/agent-orch/agent-orch        gitignored; the released binary
```

`src/main.rs` is organized top-to-bottom as four typeclasses,
matching the conceptual decomposition:

```
§1 · Session   record type + apply_event + format_header + sort
§2 · Store     state-dir owner + flock + atomic writes
§3 · Wrapper   trait + Claude / Kiro / Other impls
§4 · Loop      picker — render / render_to / peek / run / body
CLI            clap + main dispatch
Tests          #[cfg(test)] mod tests
```

### §1 · Session

```rust
#[derive(Serialize, Deserialize)]
enum State { Working, Waiting, Done }
// `Idle` exists only as a *display* state — `done` decays
// to idle at render time after 60s of quiet. Storing it in
// sessions.json would require a background process to flip
// the file once a minute; instead we compute it at render.

#[derive(Copy, Clone)]
enum DisplayState { Working, Waiting, Done, Idle }

struct Session { ... }                                     // shape above

impl Session {
    fn apply_event(&mut self, event: &str, now: u64);
    fn display_state(&self, now: u64) -> DisplayState;     // decays done → idle
    fn format_header(&self, addr: &str, now: u64) -> String;
    fn activity(&self) -> u64;                             // for sort
    fn priority(&self, now: u64) -> u8;                    // 0=waiting..3=idle
}

fn format_elapsed(secs: u64) -> String;                    // "5s", "2m", "1h", "3d"
```

`apply_event` is the entire state machine — see the table in
the Lifecycle section above. `display_state` adds the done →
idle decay (`done && now - state_ts > 60`). `priority` maps
display state to sort key.

`format_header` takes a pre-resolved address string
(`"proj-b:code.0"`) and `now` so the elapsed column can be
computed without a `SystemTime::now()` call inside the
formatter — keeps the function pure and unit-testable.

### §2 · Store

```rust
struct Store { dir: PathBuf }

impl Store {
    fn from_env() -> Result<Self>;                         // resolves XDG/HOME
    fn new(dir: PathBuf) -> Self;                          // tests pass a tempdir
    fn read(&self) -> Result<Vec<Session>>;                // no lock
    fn mutate<F, T>(&self, f: F) -> Result<T>              // read-modify-write under flock
        where F: FnOnce(&mut Vec<Session>) -> Result<T>;
}
```

`mutate` handles flock + atomic write internally. Atomic write
= per-pid tmp + rename, no fsync (state-dir scratch).

### §3 · Wrapper

```rust
trait Wrapper {
    fn kind(&self) -> &str;
    fn prepare(&self, ctx: &WrapCtx) -> Result<Prepared>;
    fn cleanup(&self, store: &Store, removing: &Session, others: &[Session]) -> Result<()>;
    fn hook(&self, store: &Store, pane_id: &str, event: &str,
            stdin: &mut dyn Read, now: u64) -> Result<()> { ... default ... }
}

struct Claude;     // prepare = passthrough; cleanup = no-op
struct Kiro;       // prepare = ensure project config; cleanup = refcount-agnostic
struct Other(String); // register-only; both no-op
```

`hook` has a default trait method body that's identical for
all kinds today. A future kind whose stdin payload differs
can override.

### §4 · Loop

```rust
struct Loop<'a> { store: &'a Store }

impl<'a> Loop<'a> {
    fn render(&self) -> Result<Vec<Item>>;                 // one Item per session
    fn render_to(&self, stdout: &mut dyn Write) -> Result<()>;
    fn peek(&self, pane_id: &str, lines: u32, stdout: &mut dyn Write) -> Result<()>;
    fn run(&self, self_path: &Path) -> Result<()>;         // outside-agent-orch path
    fn body(&self, self_path: &Path) -> Result<()>;        // inside-agent-orch picker
}

struct Item {
    pane_id: String,
    header: String,
    snippet: [String; 3],
}
```

`render` returns one `Item` per live session, sorted by
priority (waiting > working > done > idle, ties broken by
recency). For each session it shells out to `tmux display-
message` for the address and `tmux capture-pane -p -e -E -1
-S -3` for the snippet. Tmux failures fall back gracefully
(address → `?:?.<pane-id>`, snippet → empty padded lines).

`render_to` serializes each `Item` as `<pane_id>\t<header>\n
<line1>\n<line2>\n<line3>` and joins items with `\0`.

`peek` shells out to `tmux capture-pane -p -e -t <pane_id> -E
-1 -S -<lines>` for the side preview window — wider context
than the row's inline snippet.

`body` spawns fzf with `--read0 --gap=1 --highlight-line
--ansi --preview ... --header ...`, then runs two threads:
- **Watcher** posts `reload(<self> render)` to fzf when
  `sessions.json` changes (debounced 100ms).
- **Heartbeat** posts `refresh-preview` to fzf every 1s.

When fzf exits (Esc / kill), the body returns and the
agent-orch session terminates.

### CLI

```rust
enum Cmd {
    Setup    { #[arg(long)] key: Option<String> },
    Teardown,
    Wrap     { kind: String, #[arg(long)] cwd: Option<PathBuf>,
               #[arg(last = true)] agent_argv: Vec<String> },
    #[command(hide = true)] Hook        { event: String },
    #[command(hide = true)] Unregister  { pane_id: String },
    #[command(hide = true)] Render,
    #[command(hide = true)] Peek        { pane_id: String,
                                          #[arg(long, default_value_t = 25)] lines: u32 },
}

// `None` (no subcommand) → bare invocation.
//   inside agent-orch tmux session  → Loop::body
//   anywhere else                   → Loop::run
```

Note: `peek`'s default `lines` rises to 25 — it now feeds the
side preview window, which can show more context than the
3-line inline snippet.

---

## Design rationale

The non-obvious choices a future reader needs to understand
the code. Listed in roughly the order they get hit when
reading the codebase top-to-bottom.

- **Rust, single-file `src/main.rs`.** This started as a
  Deno/TypeScript prototype and pivoted to Rust because the
  wrapper needs `execvp` to replace its own process with the
  agent — the wrapper's pid must become the agent's pid so
  `kill(pid, 0)` from another shell actually targets the
  agent. Deno (and any GC'd runtime) can't do that. Rust gets
  us a 5-MB static binary, predictable signal handling, and
  honest typeclasses via traits. Single-file because the
  whole thing is ~1500 LOC; splitting into modules would add
  ceremony without buying separation.

- **Hook reporter, not a daemon.** `agent-orch hook <event>`
  is invoked directly by Claude's hook executor on every
  fire. State writes are flock-serialized inside the
  reporter; the reporter exits in <10ms. This keeps the
  observation-only invariant — there's no long-running
  background process to crash, leak fds, or get out of sync.

- **`agent-orch` is the orchestrator session name.** Matches
  the binary name. `tmux ls` shows `agent-orch:` next to the
  user's other sessions, so the role is discoverable. The
  bare invocation self-detects via `$TMUX` +
  `tmux display-message #{session_name}` whether it's already
  inside that session and routes to either bootstrap-and-
  switch (outside) or the event-driven body (inside). One
  user-facing entry point, two code paths.

- **Prefix-bound keybind, not root-bound.** The earlier
  design bound `M-o` at root. Inner TUIs (claude/kiro) eat
  Alt-letter keys inconsistently, so the keystroke would
  sometimes reach tmux and sometimes get swallowed.
  Prefix-bound (e.g. `<prefix> O`) is the standard tmux idiom
  — `<prefix> c`, `<prefix> "`, `<prefix> d` — and tmux
  intercepts every prefix key before any inner program sees
  it. The user picks the suffix at install time
  (`setup --key X`); we don't presume one.

- **Two-state lifecycle, not six.** An earlier iteration had
  six states (cold / thinking / active / waiting / idle /
  stalled) packed into the row content. Two problems showed
  up in real use: (a) the row lied — it said "thinking" when
  claude had actually crashed and printed "Not logged in"
  to the pane, because no `Stop` ever fires on crash; (b)
  cramming "what tool", "what prompt", "how long" into the
  row text meant every heartbeat had to redraw the row,
  which `reload(...)` does by blocking fzf's input — at 1
  Hz the picker felt continuously frozen. Switched to a
  binary running / idle distinction (running = `PreToolUse`
  fired without matching `PostToolUse`; idle = everything
  else) plus pane-content surfaces — first as a side preview
  window only, then (this revision) also as an inline 3-line
  snippet baked into each row. The user reads the truth from
  the pane, not from a guessed-at row text. Row content
  stays minimal so the list rarely needs to reload.

- **Multi-line rows over single-line + always-side-preview.**
  Inspired in part by the tmux-tea picker, which uses `tmux
  capture-pane -ep` in its preview. For agent-orch we need
  more than one row's worth of context at a glance — the
  user is triaging across N agents, not picking one
  directory. A 3-line inline snippet per row makes the
  at-a-glance scan informative; the side preview stays for
  deeper context on the focused row. fzf ≥ 0.55 supports
  `--read0` for null-delimited multi-line items + `--gap` for
  inter-item spacing; ours requires ≥ 0.71 anyway for
  `--track --id-nth=N`, so the dependency cost is zero.

- **Heartbeat → `refresh-preview`, not `reload`.** Both fzf
  actions update the picker, but they have very different
  cost. `reload(...)` re-runs the source command, blocks
  input while it does, and clears the prompt — at 1 Hz the
  cursor jitters and the search query feels broken. `refresh-
  preview` re-runs only the preview command for the focused
  row, doesn't touch the list, and doesn't block input. So
  the heartbeat (1 Hz, just to keep the side preview live as
  the agent works) sends `refresh-preview`; the watcher
  (rare, only on real `sessions.json` changes) sends
  `reload`. Net result: list rows are stable, side preview
  ticks live, no flicker. Inline row snippets refresh on
  next reload — fresh-at-last-event rather than streaming.

- **`tmux capture-pane -e` for ANSI.** The earlier `peek`
  used `-p` only, stripping color. Claude and kiro produce
  colored output (banners, build logs, error markers); the
  preview window felt washed out. `-p -e` preserves the
  escapes; fzf's `--ansi` flag interprets them. Same change
  applies to the inline row snippets.

- **`session:window.pane` over raw `%N` for the row header.**
  `%17` is meaningless to a human; `proj-b:code.0` is what
  the user typed when they ran `tmux send-keys -t
  proj-b:code.0`. Internally the registry still keys on
  `%N` (stable across renames), and fzf still tracks rows
  by `%N` via `--id-nth=1`; the human-readable address is
  resolved at render time via `tmux display-message -p -t
  <pane> '#S:#I.#P'` and only used for display. Resolution
  failure (server gone, pane closed) falls back to
  `?:?.<pane-id>` so the row still appears.

- **PATH-resolved agent binary.** The wrapper does
  `execvp("claude", ...)` — no path lookup, no PATH munging.
  Whichever `claude` is first in the **launching shell's**
  PATH wins. This is correct: a fresh `zsh -l` resolves
  `claude` exactly the way the user expects (their
  `.zprofile` / `.zshrc` runs and whatever shims / wrappers
  they configure land at the right precedence). Bare
  `claude` typed in that same shell would resolve to the
  same binary, so the wrapper isn't doing anything
  surprising. The flakiness mentioned below is purely about
  shells whose PATH was inherited from a stale source.

- **Stale-shell PATH risk.** Some setups have two `claude`
  binaries on PATH — for example, an auth-wrapper shim and
  an upstream standalone build. Whichever appears first in
  PATH wins. The user typically arranges their login
  shell so the wanted binary is first; the risk is that a
  shell which inherits PATH from a different source (a
  parent process, a tmux-resurrect-restored pane, an IDE
  integration's environ) ends up with a different
  ordering, so `claude` resolves to the wrong binary and
  the user sees "Not logged in" or similar. This isn't a
  wrap bug — bare `claude` from that same shell would
  resolve the same way. The right fix is in the user's
  shell setup (put the wanted directory first, or remove
  the duplicate symlink). The spec records the failure
  mode here so anyone hitting it knows where to look;
  `agent-orch doctor` (follow-up) will detect and surface
  the mismatch heuristically.

- **Claude hooks user-globally, not per-launch.** v0 wrote a
  per-launch settings file and pointed claude at it. That
  displaced claude's normal precedence chain (login state,
  MCP servers, project settings) — the wrapped claude wasn't
  authenticated even though bare claude was. Switching to
  `setup` writing tagged entries to `~/.claude/settings.json`
  preserves the precedence chain. The hooks fire on **every**
  claude invocation; `$AGENT_ORCH_PANE` filters
  bare-invocations to silent no-ops.

- **Kiro is observation-only in v1.** Kiro hooks live inside
  agent persona JSONs (`~/.kiro/agents/<name>.json`) using a
  different schema than Claude — camelCase events
  (`userPromptSubmit`, `postToolUse`), inline `{matcher?,
  command, timeout_ms?}` shape, no nested `hooks` array. We
  don't have a clean place to inject our reporter without
  modifying the user's chosen agent persona (which has cross-
  user implications). Two viable follow-ups: (a) merge our
  tagged hooks into the user's `chat.defaultAgent` persona,
  undone on teardown; (b) ship a project-scoped stub persona
  that the user opts into via `kiro --agent agent-orch`. v1
  registers Kiro panes and runs lifecycle cleanup, but
  leaves their state at Idle. The inline-snippet path still
  surfaces what kiro is doing visually — the user reads the
  snippet, not the glyph, for kiro rows. Backlog item tracks
  the right fix.

- **Event-driven picker, not a poll loop.** An earlier
  iteration re-spawned fzf every 500ms. The user lost cursor
  position, lost typed query, and the picker flickered.
  Switched to fzf's `--listen=<sock>` so the same fzf
  process accepts control commands over a Unix socket;
  `enter` is bound to `execute-silent(tmux switch-client -t
  {1})+clear-query`, which is non-terminal — fzf doesn't
  exit on selection. A watcher thread sends `reload(<self>
  render)` only when `sessions.json` actually changes; a
  heartbeat sends `refresh-preview` at 1 Hz to keep the
  side preview window live. Result: a single long-lived fzf
  process, list cursor / query / search state preserved
  across switches, no flicker.

- **Self-discovering teardown.** v0 had `agent-orch teardown
  --key X` to undo a prior `setup --key X`. Forgetting the
  key meant orphaned bindings. Switched to teardown reading
  `tmux list-keys -T prefix` and unbinding any line whose
  action is `switch-client -t agent-orch`. No state file, no
  argument, no orphan paths.

- **Functional tests on the user's real tmux server.**
  Integration tests run on a private socket — fast,
  deterministic, isolated. But they can't drive real claude
  or kiro-cli. Functional tests fill that gap: they spawn
  the four-session fixture on the user's running tmux
  server, fire real prompts via `send-keys`, and assert that
  the registry reflects what the agents actually did. They
  cost API credits and minutes per run, so they're
  user-driven (not in CI), but they're the only thing that
  catches "hooks weren't actually firing because the wrong
  `claude` binary was on PATH" or "the heartbeat thread
  quietly stopped".

---

## Implementation Plan

v1 is **incomplete — needs more work before merge.** The
bones are in place (single binary, hooks, picker, keybind,
two-state + side preview), but real use + a sanity-check
against [workmux's dashboard](https://github.com/raine/workmux)
surfaced that the UX needs more density. Open work continues
on `feat/agent-orch-fix`.

### Status — what's landed (on the parent feature branch)

- Single binary, three user-facing verbs + bare invocation.
- Claude hook reporter wired through user-global `setup` /
  `teardown` (hooks installed in `~/.claude/settings.json`,
  tagged for clean removal).
- Kiro observation-only (registers + lifecycle cleanup; state
  stays idle without hook reporting).
- Event-driven persistent picker via fzf `--listen` +
  `notify-debouncer-mini`. **Two-state lifecycle**
  (Running/Idle) + tmux pane side-preview window.
- User-specified prefix-table keybind via `setup --key X` +
  self-discovering teardown.
- Three test layers: unit tests, integration cases on a
  private tmux server, functional scripts driving the user's
  live server. All gates green (44 unit, 17 integration).

### Open issues — to address before we call v1 done

Using the shipped slice end-to-end on the user's real
workflow + comparing to adjacent tools (Prior art) showed
the surface needs more density. None of these are blockers
for the design, but together they mean v1 isn't yet "the
state we want."

- **Four-state lifecycle + priority sort.** Today's two-state
  (running/idle) is too coarse — it hides the most important
  case (waiting on permission) and doesn't help the user
  prioritize across N agents. Move to working / waiting /
  done / idle (stored as Working/Waiting/Done in JSON; idle
  is a render-time decay), with priority sort waiting >
  working > done > idle. Workmux has shipped this; the
  pattern works.
- **Picker-row redesign (this revision's main payload).**
  The current row is `<glyph> <kind> <cwd>` with all live
  signal in the side preview only. Real use showed that's
  too thin: a 6-row picker hides everything until you focus
  each row in turn. The redesign carries the four-state
  icon, `session:window.pane`, kind, cwd-tail, and elapsed
  time in the header line, plus a 3-line inline pane-content
  snippet — at-a-glance density without losing the side
  preview. Needs:
  - `State` becomes a 3-variant enum (Working/Waiting/Done);
    `DisplayState` adds Idle as a render-time decay.
  - `Session::apply_event` re-mapped to the new states;
    `Notification` is a real signal now.
  - `Session::format_header` carries icon + addr + kind +
    cwd-tail + elapsed.
  - `Loop::render` returns multi-line `Item`s sorted by
    priority; `render_to` serializes with `\0` separators.
  - `Loop::body` adds `--read0 --gap=1 --highlight-line
    --ansi` to fzf invocation, plus the `--header`
    cheatsheet line, the `p` peek-popup bind, and the `x`
    unregister bind.
  - `peek` adds `-e` to `tmux capture-pane` for ANSI.
  - Default `peek --lines` rises (10 → 25) to give the side
    preview meaningful context now that the row has its own
    snippet.
  - `setup` HOOK_EVENTS list adds `Notification` (back —
    earlier draft pruned it; the four-state model needs it).
- **Stale-shell PATH risk has only been documented, not
  detected at runtime.** The Design Rationale calls out the
  duplicate-`claude`-on-PATH ordering issue. The user's
  shell setup is the right place to fix it; the binary
  itself doesn't surface the mismatch today. `agent-orch
  doctor` (follow-up) would catch this; until that
  exists, the next user to hit a resurrect-restored pane
  with the wrong binary first on PATH has no signal.
- **Functional fixture has not been driven through the full
  user-loop yet.** The fixture-spawning script and the
  teardown script exist; the asserted-scenarios script
  (F1–F8 in Test Strategy above) needs to land in
  `functional-test.sh` and be exercised end-to-end with real
  claude/kiro-cli.
- **Spec ↔ code drift on Kiro hook integration.** Spec says
  Kiro is observation-only in v1. Code still writes a
  Claude-shape JSON to `<cwd>/.kiro/agents/agent-orch.json`,
  which Kiro logs as "invalid agent config" on every prompt.
  Either drop the write entirely (spec-true), or wire the
  right Kiro shape (close the deferred slice).

### Follow-up — Kiro state tracking

Pick the right injection point (merge into user's default
agent persona vs. ship a project-scoped stub persona),
implement the camelCase event shape, add it to the functional
test scenarios. Tracked in `specs/backlog/`.

### Follow-up — `agent-orch doctor`

Sanity-check skill. Required checks:
- tmux ≥ 3.2, fzf ≥ 0.71.0 on PATH.
- Each wrappable agent CLI detected (`claude`,
  `kiro-cli`).
- State dir writeable.
- Orchestrator-switch keybind currently registered in the
  prefix table.
- No stale Claude hook entries in `~/.claude/settings.json`
  pointing at a missing binary path.
- No orphan `<cwd>/.kiro/agents/agent-orch.json`.
- **PATH-mismatch heuristic**: for each registered pane,
  resolve the parent shell's `which claude` (and
  `which kiro-cli`) and compare to the binary the user
  marked as preferred (config option, e.g.
  `~/.config/agent-orch/preferred.toml`). Flag any pane
  where the resolved binary differs from the preferred
  one — typically because two `claude` binaries coexist on
  PATH and the resurrected shell put the wrong one first.
  This catches the tmux-resurrect-restored case (see
  "Stale-shell PATH risk" in Design Rationale) without
  changing wrap behavior.

Tracked in `specs/backlog/`.

### Follow-up — persistent tmux keybind across server restarts

Today the keybind is live-only (lost on `tmux kill-server` /
reboot). Two paths under consideration: edit the user's
tmux conf with sentinel markers; or write a sidecar
`tmux.conf` under our state dir and have the user add one
`source-file -q ...` line themselves. Path A breaks for users
whose tmux conf is on a read-only filesystem (declarative-
config managers); path B costs the user one-time manual edit
but works everywhere. Pick after the live-only version proves
out in real use.

### Follow-up — input mode + inline approval (lazyclaude-style)

Two adjacent UX wins, recorded for after the four-state
shift lands and proves out:

- **Input mode** (`i`). Forward keystrokes to the focused
  pane without leaving the dashboard — useful for typing a
  follow-up question or approving a prompt without `enter`-
  switching out. lazyclaude does this; workmux's dashboard
  does this.
- **Inline approval** when the focused row is `waiting`.
  Bind `1` / `2` / `3` to send "approve" / "approve always"
  / "reject" keystrokes into the focused pane via `tmux
  send-keys`. Same payload Claude reads from the user
  typing manually; we just shortcut it.

Both deferred because they expand the surface area beyond
"observe + jump" and require careful thought about race
conditions (what if the agent finishes mid-keystroke and
the dashboard re-sorts the row away from focus). v1 ships
without them; revisit after the four-state model is in
production for a few weeks.

### Follow-up — JSON output for scripting

`agent-orch render --json` to emit a structured array
instead of the fzf-shaped serialization. Useful for shell
integrations (status bars, notification daemons, custom
dashboards) and for tests that want to assert on individual
fields without parsing the multi-line render shape. Same
sort and decay as the human path; just a different
serializer. Cheap to add once we're sure the schema is
stable.
