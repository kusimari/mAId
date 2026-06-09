# Feature: agent-session-orchestrator

## Git Setup

- Branch: `feat/agent-session-orchestrator`
- Base: `main`

## Feature Brief

A thin observation-only layer over plain tmux that lets the user
see, at a glance, every coding-agent session running across all
tmux sessions / windows / panes — with each agent's current
lifecycle state and a short summary of what that session is
doing — and jump to any of them with a single keystroke. The
user keeps every normal tmux verb they already know; the
orchestrator only adds a registry, a derived state signal, and
an event-driven fzf picker that lives in a dedicated `agent-orch`
tmux session.

The shape, deliberately minimal:

- Coding agents launch through a thin wrapper:
  `agent-orch wrap <kind> -- <agent-cmd> [args...]`. The wrapper
  appends a record to `sessions.json`, sets `AGENT_ORCH_PANE=%N`,
  and **`execvp`s the agent**: the wrapper process is replaced in
  place. The wrapper's pid becomes the agent's pid (POSIX
  guarantee). No parent process, no signal forwarding, no
  child-pid bookkeeping.
- **Claude hooks live user-globally.** The user runs
  `agent-orch setup --key <KEY>` once at install; it appends our
  six lifecycle hook entries (`UserPromptSubmit` / `PreToolUse` /
  `PostToolUse` / `PostToolUseFailure` / `Notification` / `Stop`)
  to `~/.claude/settings.json`, tagged so `agent-orch teardown`
  can remove only ours later. The hooks fire on **every**
  `claude` invocation; the `hook` subcommand filters by
  `$AGENT_ORCH_PANE` (set only by our wrapper) and exits silently
  for bare-claude invocations, so the user's normal claude usage
  is unaffected.
- **Kiro hook reporting is out of scope for v1.** Kiro panes
  register and appear in the picker, but their state stays
  `cold` because we don't drive Kiro hooks. Lifecycle cleanup
  (`pane-exited` → unregister) still works, and the wrapper
  still execvp's the kiro CLI. See _Design rationale → Kiro is
  observation-only in v1_ for the why.
- A single `sessions.json` is the source of truth for both
  registration and state. Every mutation is gated by a POSIX
  advisory lock (`fd-lock`). The hook subcommand writes
  state on every fired event; the tmux `pane-exited` hook
  removes the record on pane death.
- The orchestrator session is `agent-orch` (matches the binary
  name; visible as `agent-orch:` in `tmux ls`). Bare
  `agent-orch` ensures that session exists, switches the
  client to it, and runs the picker body. Picking a row jumps
  the client to the agent's pane via `tmux switch-client -t %ID`.
- The picker is event-driven and persistent across selections.
  fzf runs once with `--listen=<sock>` and a `--preview` window
  on the right. A `notify`-watcher pushes `reload(...)` to fzf
  whenever `sessions.json` actually changes; a 1-second
  heartbeat pushes `refresh-preview` so the preview window
  stays live without disturbing the list cursor. `enter` runs
  `tmux switch-client` via `execute-silent` so fzf stays alive
  across selections — the user comes back to the same picker
  process, with cursor and query preserved.
- The right-side preview shows the last N lines of the focused
  agent's tmux pane via `tmux capture-pane`. That's what tells
  the user whether the (idle-glyph) agent is actually idle, or
  asking for a permission prompt, or showing a "Not logged in"
  error — pane content is the source of truth, the row is just
  the index.
- "Back to orchestrator" is a user-chosen prefix-table keybind:
  `agent-orch setup --key X` binds `<your-tmux-prefix> X` to
  `switch-client -t agent-orch`. Prefix-bound (not root-bound)
  so inner TUIs (claude/kiro) never compete with tmux for the
  keystroke. `teardown` self-discovers any prefix binding
  routing to `agent-orch` and removes it — no `--key` argument
  needed, no state file.

The whole tool is one Rust binary — script-style: a single
`src/main.rs` organized as four typeclasses (Session, Store,
Wrapper, Loop) top-to-bottom. Compiled to
`dist/agent-orch/agent-orch`. State lives at
`${XDG_STATE_HOME:-$HOME/.local/state}/agent-orch/`. v1 ships
standalone; promoting later to a flake-installed package or
registry-deployed symlink is purely additive.

## Requirements

### Lifecycle: two states + pane preview

Two-state machine driving the row glyph; a fzf preview pane
showing the agent's actual screen content tells the user the
nuance.

| State   | Glyph | Triggered by                                            |
|---------|-------|---------------------------------------------------------|
| running | `▶`   | `PreToolUse` fired without a matching `PostToolUse` yet |
| idle    | `·`   | everything else (`Stop`, `Notification`, no events ever, silence after a turn) |

That's the entire state machine. The user only cares about
"actively doing something" vs "not". Distinguishing _why_ a
session is idle (finished, waiting on permission, crashed mid-
tool, never been driven) is what the **pane preview** does.

The picker has two columns:

```
┌──────────────────────────────────────────┬──────────────────────────────┐
│ ▶ claude proj-a                          │  ⚠ Not logged in             │
│ · claude proj-b                          │    Please run /login         │
│ ▶ claude proj-c                          │                              │
│ · kiro   proj-c                          │                              │
└──────────────────────────────────────────┴──────────────────────────────┘
       picker rows                              fzf --preview
       (running first, idle next)               last N lines of focused
                                                pane via tmux capture-pane
```

Row content is intentionally minimal: `<glyph> <kind> <cwd>`.
No prompt text, no tool name, no duration counter — those
caused flicker (every reload blocks fzf's input briefly) and
duplicated information the preview already shows. The preview
window updates at 1 Hz via fzf's `refresh-preview` action,
which is non-blocking and doesn't disturb the cursor or query.

Sort order: running first (most-recently-active first within
that group), then idle (most-recently-active first). A
just-finished idle agent stays near the top so the user sees
it; a long-idle one drifts down.

### State derivation

Hook events drive `Session::apply_event`. Mapping:

- `UserPromptSubmit` → no state change. Stamps
  `last_event_ts` (used by sort and the implicit
  "recently active" hint).
- `PreToolUse` → state := running.
- `PostToolUse` / `PostToolUseFailure` → state := idle.
  Tool finished; whatever the agent does next (more
  tools, Stop, notification) is up to it.
- `Notification` → state := idle. The preview window
  shows the actual notification body so the user knows
  what the agent is asking about.
- `Stop` → state := idle.

`apply_event` always bumps `last_event` and `last_event_ts`
unconditionally — used by sort and by the preview's "ago"
hint.

### Pane preview (`agent-orch peek <pane-id>`)

A new hidden subcommand wraps `tmux capture-pane -p -t
<pane-id> -E -1 -S -<N>` to dump the last `<N>` visible lines
of the agent's pane to stdout. fzf's `--preview` invokes it
on every focused row, so the user sees the actual screen
content of whichever agent the cursor is on.

`<N>` is small (default ~10 lines) so:
- the preview window stays compact;
- ANSI/Unicode noise from the agent's TUI drawing doesn't
  overwhelm the relevant content;
- capture is fast (capture-pane on a single pane is
  microseconds).

`peek` writes raw output (no ANSI stripping in v1 — fzf's
preview window renders ANSI fine). If terminal width is too
narrow for a useful preview, the user can resize fzf's
preview ratio via the standard tmux/fzf controls.

### Single binary, three user-facing verbs + bare invocation

```
agent-orch setup [--key X]   # install Claude hooks; --key binds <prefix> X
agent-orch teardown          # remove hooks + self-discover keybind
agent-orch wrap <kind> [--cwd <dir>] -- <agent-cmd> [args...]
agent-orch                   # open the UX (self-detects in/out of agent-orch session)
```

Plus four hidden internal verbs that exist only because external
systems shell out to them: `hook` (Claude lifecycle event
callback), `unregister` (tmux `pane-exited` target), `render`
(fzf `reload(...)` target), `peek` (fzf `--preview` target —
dumps the last N lines of a pane via `tmux capture-pane`). All
carry `#[command(hide = true)]` so they don't appear in `--help`.

`agent-orch doctor` is planned but deferred to a follow-up
ticket: it'll verify tmux ≥ 3.2, fzf ≥ 0.71.0 on PATH, agent
CLIs detected, state dir writeable, the orchestrator-switch
keybind currently registered, no stale Claude hooks pointing at
a missing binary, no orphan `.kiro/agents/agent-orch.json`.

### Wrapper (`agent-orch wrap`)

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
  3. Pushes a fresh `Idle` Session record (no hooks have
     fired yet; the pane preview will show the agent's
     startup screen).
- Outside the lock:
  4. Installs the global tmux `pane-exited` hook (idempotent
     via marker file + tmux's own `set-hook -g` idempotence).
  5. Sets pane option `@agent-orch-pane` for future
     introspection.
  6. Sets `AGENT_ORCH_PANE` env var.
  7. `execvp`s the agent.

The wrapper inherits the calling shell's full env, so PATH
ordering is whatever the shell set. See _Design rationale →
PATH-resolved agent binary_ for why we don't intervene.

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

### Picker (`agent-orch` bare)

Self-detecting:
- `$TMUX` set + `tmux display-message -p '#{session_name}'` ==
  `agent-orch` → run `Loop::body` (we're inside the
  orchestrator session, tmux just spawned us).
- Otherwise → run `Loop::run` (ensure session exists,
  `switch-client -t agent-orch`).

`Loop::body` spawns fzf once with these flags:
- `--listen=<sock>` — Unix socket fzf listens on for
  HTTP/1.1 control commands.
- `--with-nth=2..` — show every column except the pane id.
- `--track --id-nth=1` — keep the highlight on the same
  pane id across reloads.
- `--preview='<self> peek {1}' --preview-window=right:50%`
  — show last N lines of the focused pane.
- `--bind 'enter:execute-silent(tmux switch-client -t {1})+clear-query'`
  — non-terminal binding. fzf stays alive across selections.

Two background threads drive updates over the socket — **but
only one of them ever fires `reload(...)`**. The flicker the
v1 design suffered came from heartbeat-driven reloads: every
`reload(...)` blocks fzf's input briefly (the prompt dims, the
cursor freezes), so at 1 Hz the picker felt alive but
unusable.

- **Watcher.** `notify-debouncer-mini` watches the store dir;
  100ms debounce. When `sessions.json` actually changes (a
  hook event landed, an unregister fired), POST
  `reload(<self> render)` to fzf. The list refresh is needed
  because rows can appear / disappear / change state.
- **Heartbeat.** Every 1 second, POST `refresh-preview` to
  fzf. This re-runs the preview command (`<self> peek
  <focused-pane>`) but does **not** touch the list — the
  cursor stays put, the query stays put, the prompt stays
  bright. The preview window updates in place so the user
  sees the agent's pane content tick forward.

Main thread blocks on the watcher channel with a 200ms
timeout, polls fzf's `try_wait` between iterations, drains
backlogs, and dispatches the right action (reload vs
refresh-preview) based on which channel sent the tick. When
fzf exits, the body returns and the orchestrator session
terminates.

### "Back to orchestrator" UX

`agent-orch setup --key <KEY>` binds `<tmux-prefix> <KEY>` to
`switch-client -t agent-orch` in the **prefix table**, not the
root table. Inner TUIs (claude/kiro) consume root-bound keys
inconsistently; prefix bindings are the standard tmux idiom
(`<prefix> c`, `<prefix> "`, `<prefix> d`) and are reliably
intercepted before any inner program sees them.

Live-only — survives until the tmux server exits. Persistence
across reboots is the user's job: bake the equivalent
`bind-key` line into `~/.tmux.conf` via home-manager, chezmoi,
yadm, ansible-pull, or whatever their dotfiles setup is.
Re-running `setup --key Y` after a prior `--key X` swaps cleanly
(old binding removed, new one installed).

`teardown` self-discovers and removes any prefix binding whose
action is `switch-client -t agent-orch`. The user doesn't need
to remember what key they bound; we don't keep state.

`$AGENT_ORCH_TMUX_SOCKET` is honored as `-L <name>` on every
tmux invocation we make, so the integration script can target a
private tmux server without touching the user's real one.

Fallback that always works: bare `agent-orch` from any shell
pane. Useful when no keybind is installed yet, or after a tmux
server restart.

### Hard constraints

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
  `"x-agent-orch-managed": true`, leaving every other field
  of the user's settings file alone.
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

### Runtime prerequisites

The compiled binary is environment-agnostic. It needs:

- `tmux` ≥ 3.2 on PATH (for `set-hook`, `set-option`,
  `bind-key`, `switch-client`).
- `fzf` ≥ 0.71.0 on PATH (`--listen=<sock>` ≥ 0.66.0,
  `--track --id-nth=N` ≥ 0.71.0).
- The wrapped agent's CLI on PATH (`claude`, `kiro-cli`, etc.)
  for the kinds the user actually wraps.
- A writable `$HOME` (for `setup` / `teardown` to edit
  `~/.claude/settings.json`) and a writable
  `${XDG_STATE_HOME:-$HOME/.local/state}/agent-orch/`.

Anything else (`flake.nix`, `nix develop`, `direnv`,
`rust-overlay`) is **build-time tooling for mAId
contributors**, not a constraint on users running the binary.

## Test Strategy

Three layers, mapped onto `project.md`'s test surface.

### Unit (`deno task agent-orch:test`) — load-bearing

Delegates to `cargo test`. Tests live at the bottom of
`src/main.rs` in `#[cfg(test)] mod tests`. Each typeclass is
exercised through its own surface against a tempdir `Store`.

Coverage:

- **§1 Session — state machine.** `apply_event` flips state
  to running on `PreToolUse`, idle on every other event.
  Sort order: running before idle; within group most-
  recently-active first. `cwd_tail` root-anchored edge
  cases. `format_row` produces `<glyph> <kind> <cwd>`.
- **§2 Store** — `read` on empty / missing / malformed file;
  `mutate` round-trips and observes prior state.
- **§3 Wrapper** — `Claude::prepare` is a pass-through;
  `Kiro::prepare` writes `.kiro/agents/agent-orch.json` first
  time + reuser leaves flag false; `Kiro::cleanup` keeps
  config while sibling alive + removes refcount-agnostically
  on close-creator-first ordering; `wrap()` refuses
  double-register on alive pid + replaces stale record on
  dead pid; default `hook` body filters on
  `$AGENT_ORCH_PANE` unset (silent no-op); `hook` updates
  state correctly via both Claude and Kiro impls (proves
  default-method inheritance).
- **§4 Loop — render** filters dead pids and sorts live ones.
  `render_to` emits one tab-separated `<pane_id>\t<row>` line
  per session. `peek` writes `tmux capture-pane` output for
  a given pane; gracefully handles missing-pane (returns
  empty) without erroring.
- **`setup` / `teardown` JSON merge** — creates settings with
  tagged entries; preserves user-existing entries and appends
  ours; idempotent (no duplicates on re-run); refreshes
  command path on re-run; rejects non-object root +
  non-array event; teardown removes only tagged entries;
  teardown removes the file when only our content remained;
  full setup → teardown round-trip restores pre-state
  byte-for-byte.

Quality gate: `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings`.

### Integration (`deno task agent-orch:integration`) — load-bearing

`tests/agent-orch/integration.sh` drives the compiled binary
against a **private tmux server** (`tmux -L agent-orch-test-$$`)
with `XDG_STATE_HOME` pointed at a tempdir. Exercises the real
tmux side effects (`set-hook`, `set-option`, `switch-client`,
`bind-key`) and real argv handling on a live process — the
half of the system unit tests can't reach.

Cases:

1. `wrap claude` registers a session and stamps the pane id.
2. `hook UserPromptSubmit` keeps state idle (only `PreToolUse`
   flips it to running).
3. `hook PreToolUse` flips state to running.
4. `hook PostToolUse` (or `Stop`) flips state back to idle.
5. `render` emits a tab-separated `<pane_id>\t<row>` per
   registered pane, in picker-sort order.
6. `unregister <pane>` removes the record.
7. **Kiro refcount-agnostic cleanup** survives close-creator-
   first ordering (the load-bearing invariant for kiro).
8. `wrap` refuses without `$TMUX_PANE`.
9. The global tmux `pane-exited` hook is registered with the
   `unregister` command after first `wrap`.
10. `setup` / `teardown` round-trips on a fresh `$HOME`
    (creates settings.json, removes it on teardown when
    only our content remained); preserves user-existing
    entries through the round-trip.
11. **Keybind round-trip on a live tmux server.** Split into
    11a (setup --key X installs `<prefix> X`), 11b (re-keying
    --key Y swaps X→Y cleanly), 11c (teardown
    self-discovers and removes the binding without --key),
    11d (setup without --key installs hooks only and leaves
    the prefix table untouched).

CI environments without tmux/jq/the dist binary skip silently
(exit 0).

### Functional (`tests/agent-orch/functional-*.sh`) — user-driven

Three scripts that drive the **user's real tmux server** with
real claude/kiro-cli CLIs:

- `functional-setup.sh <KEY>` — spawns the four-session fixture
  and installs hooks + keybind.
- `functional-test.sh` — fires real prompts at the wrapped
  agents via `tmux send-keys`, polls the registry, asserts
  state transitions reflect the actual agent activity.
- `functional-teardown.sh` — kills the four sessions, runs
  `agent-orch teardown`, clears the registry. Idempotent.

The fixture (four sessions on the live server, untouched by
integration tests):

| Session      | Layout                                                                               | What's wrapped                  |
|--------------|--------------------------------------------------------------------------------------|---------------------------------|
| `proj-a`     | 1 window, 1 pane                                                                     | claude                          |
| `proj-b`     | 2 windows. Window 2 (`code`) has horizontal split — **two claudes side-by-side**     | claude × 2 (same cwd)           |
| `proj-c`     | 1 window, vertical split — kiro top, claude bottom                                   | kiro + claude (same cwd)        |
| `agent-orch` | the orchestrator session itself; bootstrapped detached                               | the picker (fzf body)           |

Scenarios the functional test asserts:

- **Running flips on real tool execution.** Submit a prompt
  that forces claude to use a tool (e.g. "list files in
  cwd"). Wait. Assert the row's state hits running while
  the tool is in flight, then settles back to idle within
  N seconds of the tool finishing. The narrow window
  matters — a fast tool may flip running→idle inside a
  single poll cycle, so the test polls at high frequency.
- **Two agents in one window track independently.** Submit
  different prompts to proj-b's left and right claudes;
  assert both rows have their own state and `last_event_ts`
  without cross-contamination.
- **Mixed kinds in one window.** proj-c's kiro and claude
  panes are both in the registry. Claude advances normally;
  kiro's row stays idle (state-tracking is out-of-scope,
  but registration must still work).
- **Pane preview shows the agent's screen content.** Run
  `agent-orch peek <pane>` directly; assert output matches
  what's currently visible in the pane (last N lines).
- **Kiro refcount-agnostic cleanup under real `pane-exited`.**
  Close proj-c's kiro pane; assert the project-scoped Kiro
  config is removed (or kept, if claude+kiro share a cwd via
  some other ordering).
- **`<prefix> KEY` round-trips back to the orchestrator
  session.** Verified by reading `tmux list-keys -T prefix`
  output (rather than synthesizing keystrokes — too flaky).
- **Dead-pid filter drops crashed agents from the picker.**
  Kill an agent's pid directly; on next render the row is
  gone.
- **Stale-environment safety.** A pane whose parent zsh has
  `~/.local/bin` ahead of `~/.toolbox/bin` in PATH (the
  tmux-resurrect-restored case on Amazon-internal machines)
  resolves `claude` to the standalone Anthropic install,
  which can't authenticate. Functional test starts each
  session **fresh** (no resurrect), so this is documented
  as a known env-side risk in Design Rationale rather than
  asserted on. See "Stale-shell PATH risk" below.

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

## Design

### File layout

```
sources/agent-orch/
├── Cargo.toml                    deps: anyhow, clap, fd-lock, nix,
│                                       notify, notify-debouncer-mini,
│                                       serde, serde_json
└── src/main.rs                   single file, ~1900 LOC including tests

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
§1 · Session   record type + apply_event + format_row + sort
§2 · Store     state-dir owner + flock + atomic writes
§3 · Wrapper   trait + Claude / Kiro / Other impls
§4 · Loop      picker — render / render_to / run / body
CLI            clap + main dispatch
Tests          #[cfg(test)] mod tests
```

### `sessions.json` shape

One record per wrapped pane:

```json
{
  "pane_id": "%17",
  "pid": 274317,
  "kind": "claude",
  "cwd": "/tmp/proj-a",
  "started": 1780619367,

  "state": "running",                // running | idle
  "state_ts": 1780619400,

  "last_event": "PreToolUse",
  "last_event_ts": 1780619400,

  "created_kiro_config": false       // refcount cleanup signal
}
```

That's the entire schema. The picker row is `<glyph> <kind>
<cwd>` — derivable from these fields plus a glyph lookup. The
preview is generated on demand from the live tmux pane via
`tmux capture-pane`; we don't snapshot it into the registry.
All fields after `state_ts` carry `#[serde(default)]` so
legacy registries (with extra fields like `last_prompt` /
`tools_this_turn` from earlier shapes) deserialize cleanly —
extras are ignored.

### §1 · Session

```rust
struct Session { ... }                            // see shape above

impl Session {
    fn apply_event(&mut self, event: &str, now: u64);
    fn format_row(&self) -> String;               // "<glyph> <kind> <cwd>"
    fn activity(&self) -> u64;                    // for sort: max(state_ts, last_event_ts)
    fn is_running(&self) -> bool;                 // for sort precedence
}
```

`apply_event` is the entire state machine: `PreToolUse` →
`state := running`, everything else → `state := idle`. One
match arm per event; `_` falls through to a `last_event_ts`
bump only. The caller (`hook` subcommand) handles "bare
claude" filtering; `apply_event` itself is unconditional.
Payload parsing isn't needed — the state machine doesn't
read prompt / tool fields anymore.

### §2 · Store

```rust
struct Store { dir: PathBuf }

impl Store {
    fn from_env() -> Result<Self>;            // resolves XDG/HOME
    fn new(dir: PathBuf) -> Self;             // tests pass a tempdir
    fn read(&self) -> Result<Vec<Session>>;   // no lock
    fn mutate<F, T>(&self, f: F) -> Result<T> // read-modify-write under flock
        where F: FnOnce(&mut Vec<Session>) -> Result<T>;
}
```

`mutate` handles flock + atomic write internally. Every
read-modify-write site collapses to one `store.mutate(|v| ...)`
call. Atomic write = per-pid tmp + rename, no fsync (state-dir
scratch).

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

fn wrapper_for(kind: &str) -> Box<dyn Wrapper> { ... }
```

`hook` has a default trait method body that's identical for all
kinds today. A future kind whose stdin payload differs can
override; today neither Claude nor Kiro overrides.

The free `wrap()` function dispatches to the right trait impl,
mutates the store, then (when side effects are enabled) installs
the global tmux hook + tags the pane + execvp's the agent.
Per-kind branches live in the trait impls; `wrap()` itself is
kind-agnostic.

### §4 · Loop

```rust
struct Loop<'a> { store: &'a Store }

impl<'a> Loop<'a> {
    fn render(&self) -> Result<Vec<(String, String)>>;     // (pane_id, row)
    fn render_to(&self, stdout: &mut dyn Write) -> Result<()>;
    fn peek(&self, pane_id: &str, lines: u32, stdout: &mut dyn Write) -> Result<()>;
    fn run(&self, self_path: &Path) -> Result<()>;          // outside-agent-orch path
    fn body(&self, self_path: &Path) -> Result<()>;         // inside-agent-orch picker
}
```

`render` returns one row per live session, sorted (running
first; within group most-recently-active first); rows are
plain `<glyph> <kind> <cwd>`.

`peek` shells out to `tmux capture-pane -p -t <pane_id> -E -1
-S -<lines>` and writes to stdout. The hidden `agent-orch
peek` subcommand wraps this for fzf's `--preview`.

`body` spawns fzf with `--preview`, then runs two threads:
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
                                          #[arg(long, default_value_t = 10)] lines: u32 },
}

// `None` (no subcommand) → bare invocation.
//   inside agent-orch tmux session  → Loop::body
//   anywhere else                   → Loop::run
```

## Design rationale

The non-obvious choices a future reader needs to understand the
code. Listed in roughly the order they get hit when reading the
codebase top-to-bottom.

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
  else) plus a fzf preview window showing the agent's
  actual pane content via `tmux capture-pane`. The user
  reads the truth from the pane, not from a guessed-at row
  text. Row content stays minimal so the list rarely needs
  to reload.

- **Heartbeat → `refresh-preview`, not `reload`.** Both
  fzf actions update the picker, but they have very
  different cost. `reload(...)` re-runs the source command,
  blocks input while it does, and clears the prompt — at
  1 Hz the cursor jitters and the search query feels
  broken. `refresh-preview` re-runs only the preview command
  for the focused row, doesn't touch the list, and doesn't
  block input. So the heartbeat (1 Hz, just to keep the
  preview live as the agent works) sends `refresh-preview`;
  the watcher (rare, only on real `sessions.json` changes)
  sends `reload`. Net result: list rows are stable, preview
  ticks live, no flicker.

- **Pane preview as the truth signal.** The state column is
  a glyph at-a-glance signal; the preview is what the user
  reads to act. `agent-orch peek <pane-id>` wraps `tmux
  capture-pane -p -t <pane> -E -1 -S -<N>` and dumps the
  last N visible lines to stdout. fzf shows them in the
  `--preview` window. When claude is "Not logged in", you
  see the message. When kiro is asking for permission, you
  see the prompt. When an agent is between turns, you see
  the prompt cursor. No row schema can capture that nuance
  reliably; pane content always can.

- **PATH-resolved agent binary.** The wrapper does
  `execvp("claude", ...)` — no path lookup, no PATH munging.
  Whichever `claude` is first in the **launching shell's**
  PATH wins. This is correct: a fresh `zsh -l` resolves
  `claude` exactly the way the user expects (their
  `.zprofile` / `.zshrc` runs and the toolbox shim or
  whatever else they have lands at the right precedence).
  Bare `claude` typed in that same shell would resolve to
  the same binary, so the wrapper isn't doing anything
  surprising. The flakiness mentioned below is purely about
  shells whose PATH was inherited from a stale source.

- **Stale-shell PATH risk.** On Amazon-internal machines,
  Claude Code's toolbox installer creates two
  `claude` binaries: `~/.toolbox/bin/claude` (Bedrock-auth
  shim, the one users want) and `~/.local/bin/claude`
  (Anthropic-native standalone, used by IDE integrations).
  The user's `~/.zprofile` typically prepends `~/.local/bin`
  to PATH. Whether the toolbox shim wins depends on whether
  the toolbox installer's PATH-prepend ran during the
  shell's startup, OR the shell inherited PATH from a
  parent that already had `.toolbox/bin` promoted. tmux-
  resurrect saves panes and restores them by spawning new
  shells that **inherit the tmux server's environ at
  restore time** — if that environ has `.local/bin` ahead
  of `.toolbox/bin`, every restored pane's `claude` lookup
  hits the standalone (no Bedrock auth). This isn't a wrap
  bug; bare `claude` from such a pane would resolve the
  same way. The right fix is in env-workplace (rearrange
  `.zprofile` to put `.toolbox/bin` ahead of `.local/bin`,
  or stop creating the `.local/bin/claude` symlink). The
  spec records it here so anyone hitting "Not logged in"
  on a wrapped pane knows where to look. `agent-orch
  doctor` (follow-up) will detect and surface the
  mismatch.

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
  (`userPromptSubmit`, `postToolUse`), inline `{matcher?, command,
  timeout_ms?}` shape, no nested `hooks` array. We don't have
  a clean place to inject our reporter without modifying the
  user's chosen agent persona (which has cross-user
  implications). Two viable follow-ups: (a) merge our tagged
  hooks into the user's `chat.defaultAgent` persona, undone
  on teardown; (b) ship a project-scoped stub persona that
  the user opts into via `kiro --agent agent-orch`. v1
  registers Kiro panes and runs lifecycle cleanup, but
  leaves their state at Cold. Backlog item tracks the right
  fix.

- **Event-driven picker, not a poll loop.** An earlier
  iteration re-spawned fzf every 500ms. The user lost
  cursor position, lost typed query, and the picker
  flickered. Switched to fzf's `--listen=<sock>` so the
  same fzf process accepts control commands over a Unix
  socket; `enter` is bound to `execute-silent(tmux
  switch-client -t {1})+clear-query`, which is
  non-terminal — fzf doesn't exit on selection. A watcher
  thread sends `reload(<self> render)` only when
  `sessions.json` actually changes; a heartbeat sends
  `refresh-preview` at 1 Hz to keep the preview window
  live. Result: a single long-lived fzf process, list
  cursor / query / search state preserved across
  switches, no flicker.

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
  catches "hooks weren't actually firing on the toolbox
  shim" or "the heartbeat thread quietly stopped".

## Implementation Plan

v1 lands the bones (binary, hooks, picker shell). The
loop-UI redesign (two-state + pane preview) and the
stale-shell PATH risk are the next slice.

### Shipped — v1 bones

- Single binary, three user-facing verbs + bare invocation.
- Claude hook reporter wired through user-global `setup` /
  `teardown` (hooks installed in `~/.claude/settings.json`,
  tagged for clean removal).
- Kiro observation-only (registers + lifecycle cleanup; state
  stays idle without hook reporting).
- Event-driven persistent picker via fzf `--listen` +
  `notify-debouncer-mini`.
- User-specified prefix-table keybind via `setup --key X` +
  self-discovering teardown.
- Three test layers: unit tests, integration cases on a
  private tmux server, functional scripts driving the user's
  live server.

### Slice — loop UI redesign

Replace the multi-state lifecycle with the two-state +
pane-preview shape described in Requirements. Concretely:

- Trim `Session` to the lean shape: drop
  `last_prompt` / `last_tool_name` / `last_tool_preview` /
  `prompt_started_at` / `tool_started_at` / `tools_this_turn` /
  `last_turn_duration` / `effective_state` / `Stalled`. State
  enum becomes `Running | Idle`.
- `apply_event`: `PreToolUse` → running; everything else →
  idle. No payload parsing; just bump `last_event_ts`.
- `format_row` returns `<glyph> <kind> <cwd>` only.
- New hidden `peek <pane-id> [--lines N]` subcommand wrapping
  `tmux capture-pane`. Default N = 10.
- `Loop::body` invokes fzf with `--preview='<self> peek {1}'
  --preview-window=right:50%`. Watcher thread continues to
  send `reload(...)` on sessions.json change; heartbeat
  thread switches from `reload` to `refresh-preview` at 1 Hz.
- Drop the `setup` install of `Notification` and
  `PostToolUseFailure` hooks (no longer drive distinct
  states; the preview surfaces those situations directly).
- Update unit + integration tests to the new shape.
- Add functional test scenario: `agent-orch peek <pane>`
  output matches captured pane content.

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
  `which kiro-cli`) and compare to the toolbox-managed
  binary the user expects. Flag any pane where the resolved
  binary is the standalone `~/.local/bin/claude` while the
  toolbox shim exists at `~/.toolbox/bin/claude`. This
  catches the tmux-resurrect-restored case (see "Stale-
  shell PATH risk" in Design Rationale) without changing
  wrap behavior.

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
