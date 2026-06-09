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
  fzf runs once with `--listen=<sock>`; a `notify`-watcher and
  a 1-second heartbeat thread push `reload(...)` over the UDS
  whenever `sessions.json` changes or a timer needs to tick.
  `enter` runs `tmux switch-client` via `execute-silent` so fzf
  stays alive across selections — the user comes back to the
  same picker process, with cursor and query preserved.
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

### Lifecycle states (v1)

Six states, distinguished at the **row content** level — not
just glyphs. Active rows show what's running; idle rows
summarize what just finished.

| State    | Glyph | Triggered by                                 | Row content                                           |
|----------|-------|----------------------------------------------|-------------------------------------------------------|
| Cold     | `·`   | wrap registered, no hooks fired yet          | `<kind> <cwd> · —`                                    |
| Thinking | `◉`   | `UserPromptSubmit` or `PostToolUse` fired    | `<kind> <cwd> · "<prompt>" · thinking · 1:02`         |
| Active   | `▶`   | `PreToolUse` fired, no `PostToolUse` yet     | `<kind> <cwd> · Bash(cargo test) · 0:04`              |
| Waiting  | `⚠`   | `Notification` fired                         | `<kind> <cwd> · waiting · 0:23`                       |
| Idle     | `·`   | `Stop` fired                                 | `<kind> <cwd> · done in 2m31s · 6 tools · 7m ago`     |
| Stalled  | `⊘`   | render-time: Active + no event in `>90s`     | `<kind> <cwd> · stalled at <Tool> · 2m04s silent`     |

`Stalled` is render-time decoration — never written to the
registry, recovers as soon as the next hook event arrives.
Catches the cases where a tool is genuinely long-running, the
agent crashed mid-tool, or the hook reporter died.

Sort order in the picker: Active > Thinking > Waiting > Idle >
Stalled > Cold. Within group, most-recently-active first.

### State derivation

Hook events drive the state machine in §1 `apply_event`.
Mapping:

- `UserPromptSubmit` → Thinking. Records `prompt_started_at`,
  resets `tool_started_at`, resets `tools_this_turn`, captures
  prompt (truncated to 80 chars).
- `PreToolUse` → Active. Records `tool_started_at`, captures
  tool name + per-tool input preview.
- `PostToolUse` → Thinking. Bumps `tools_this_turn`.
- `PostToolUseFailure` → Thinking. Same as PostToolUse —
  failed tool still bumps the count.
- `Notification` → Waiting. Sticky until next hook event
  clears it. Fires for permission prompts, idle nudges, etc.
- `Stop` → Idle. Records `last_turn_duration` =
  `Stop_ts - prompt_started_at`.

Per-tool input preview is whitelisted by tool name to keep the
row predictable: Bash → `command`, Edit/Write/Read → `file_path`,
Grep/Glob → `pattern`, Agent/Task → `description`, WebFetch/Search
→ `url` or `query`. Unknown tools render just the tool name. All
previews truncate to 40 chars.

### Single binary, three user-facing verbs + bare invocation

```
agent-orch setup [--key X]   # install Claude hooks; --key binds <prefix> X
agent-orch teardown          # remove hooks + self-discover keybind
agent-orch wrap <kind> [--cwd <dir>] -- <agent-cmd> [args...]
agent-orch                   # open the UX (self-detects in/out of agent-orch session)
```

Plus three hidden internal verbs that exist only because external
systems shell out to them: `hook` (Claude lifecycle event
callback), `unregister` (tmux `pane-exited` target), `render`
(fzf `reload(...)` target). All three carry `#[command(hide = true)]`
so they don't appear in `--help`.

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
  3. Pushes a `Cold` Session record.
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
- `--track --id-nth=1` — keep the highlight on the same pane
  id across reloads (so a state change to the focused row
  doesn't lose the cursor).
- `--bind 'enter:execute-silent(tmux switch-client -t {1})+clear-query'`
  — non-terminal binding. fzf stays alive across selections.

Two background threads feed reload commands over the socket:
- **Watcher.** `notify-debouncer-mini` watches the store dir;
  100ms debounce; sends `()` over an mpsc channel when
  `sessions.json` changes.
- **Heartbeat.** Sends `()` every 1 second so timer columns
  advance even with no hook traffic, and `effective_state`
  demotes Active → Stalled live.

Main thread blocks on the channel with a 200ms timeout, polls
the fzf child's `try_wait` between iterations, and POSTs
`reload(<self> render)` to the socket whenever a tick arrives
(coalesces backlogs to one reload per drain). When fzf exits,
the body returns and the orchestrator session terminates.

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

- **§1 Session — state machine.** `apply_event` walks the
  full lifecycle (cold → thinking → active → thinking → idle).
  `Notification` → Waiting. `PostToolUseFailure` counts as a
  completed tool. Prompt truncation. `format_row` per-state
  content shape. `effective_state` demotion of Active →
  Stalled past `STALL_AFTER_SECS`. Sort order across all six
  states. `cwd_tail` root-anchored edge cases.
  `duration_short` and `ago` formatting across thresholds.
  `tool_input_preview` per-tool whitelist.
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
  per session.
- **`setup` / `teardown` JSON merge** — creates settings with
  tagged entries (six events); preserves user-existing
  entries and appends ours; idempotent (no duplicates on
  re-run); refreshes command path on re-run; rejects
  non-object root + non-array event; teardown removes only
  tagged entries; teardown removes the file when only our
  content remained; full setup → teardown round-trip
  restores pre-state byte-for-byte.

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
2. `hook UserPromptSubmit` flips state to thinking and stores
   prompt.
3. `hook PreToolUse` flips state to active and stores tool
   name + input preview.
4. `hook Stop` flips state to idle.
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

- **State transitions reflect real agent activity.** Submit a
  prompt to proj-a's claude via `send-keys`, wait, assert the
  registry row moves through Thinking and (if a tool fires)
  Active before reaching Idle.
- **Two agents in one window track independently.** Submit
  different prompts to proj-b's left and right claudes;
  assert both rows have their own state, prompt, tool, and
  duration without cross-contamination.
- **Mixed kinds in one window.** proj-c's kiro and claude
  panes are both in the registry. Claude advances normally;
  kiro's row stays cold (state-tracking is out-of-scope, but
  registration must still work).
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

  "state": "active",              // cold|thinking|active|waiting|idle|stalled
  "state_ts": 1780619400,

  "last_prompt": "fix the failing test",
  "last_event": "PreToolUse",
  "last_event_ts": 1780619400,

  "prompt_started_at": 1780619395,  // 0 outside an active turn
  "tool_started_at":   1780619400,  // 0 between tools
  "last_tool_name":    "Bash",
  "last_tool_preview": "cargo test",

  "tools_this_turn":     2,
  "last_turn_duration":  151,       // seconds

  "created_kiro_config": false      // refcount cleanup signal
}
```

All fields after `state_ts` carry `#[serde(default)]` so legacy
registries deserialize without a migration shim. `Stalled` is
never persisted — it's render-time-only, derived by
`effective_state(now)` from `last_event_ts`.

### §1 · Session

```rust
struct Session { ... }                            // see shape above

impl Session {
    fn apply_event(&mut self, event: &str, payload: &serde_json::Value, now: u64);
    fn format_row(&self, now: u64) -> String;     // shape varies by state
    fn effective_state(&self, now: u64) -> State; // demote Active→Stalled
    fn activity(&self) -> u64;                    // for sort
    fn state_group(&self, now: u64) -> u8;        // sort precedence
}
```

`apply_event` is the entire state machine. One match arm per
event; `_` falls through to a bump of `last_event` + `last_event_ts`
only. Caller (the `hook` subcommand) handles "bare claude"
filtering; `apply_event` itself is unconditional.

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
    fn run(&self, self_path: &Path) -> Result<()>;          // outside-orchestrator path
    fn body(&self, self_path: &Path) -> Result<()>;         // inside-orchestrator picker
}
```

`render` captures `now` once so all rows in one snapshot share a
time reference (otherwise "5s ago" / "0:04" readings drift
across rows in a slow render).

`body` spawns fzf + watcher + heartbeat threads as described in
Requirements → Picker. When fzf exits (Esc / kill), the body
returns and the orchestrator session terminates.

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

- **Six-state lifecycle, not three.** v0 had `running` /
  `complete` / `unknown`. That couldn't tell you whether an
  agent was actively running a tool, thinking between tools,
  waiting on user input, or done minutes ago. The picker
  rendered the same `▶` glyph and the same prompt-only row
  in all of those cases. Six states each pull their own
  information density: Active shows the in-flight tool with a
  preview; Thinking shows the prompt with a hint; Waiting
  flags user intervention; Idle summarizes the completed
  turn (duration + tool count + age); Stalled is render-time
  decoration when an Active row has been silent for >90s;
  Cold is the fresh-wrap state.

- **Heartbeat thread re-renders at 1Hz.** Without it, timer
  columns (`0:04`, `7m ago`) would only advance when a real
  hook event fired the watcher. Stalled demotion would
  also lag — Active rows would look fine until the next
  unrelated registry mutation triggered a render. The
  heartbeat piggybacks on the existing watcher mpsc channel;
  at 1Hz the cost is one re-render per second, which is
  trivially cheap.

- **PATH-resolved agent binary.** The wrapper does
  `execvp("claude", ...)` — no path lookup, no PATH munging.
  Whichever `claude` is first in the **launching shell's**
  PATH wins. This is correct: the wrapper has no way to know
  which `claude` the user wants, and the launching shell's
  setup is exactly the policy the user already chose. On
  Amazon-internal machines that have both `~/.toolbox/bin/claude`
  (the Bedrock-auth shim) and `~/.local/bin/claude` (Claude
  Code's native install — created by the toolbox shim's own
  `post-install` hook), PATH ordering decides which one
  runs. Login shells go through `.zprofile` / `.profile`;
  interactive shells layer additional PATH prepending on
  top. This is why `functional-setup.sh` launches the wrap
  via `tmux send-keys` into a fresh `zsh -l` session — by
  the time it types, the login shell's full PATH setup is
  complete and `~/.toolbox/bin` is in front.

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

- **Event-driven picker, not a poll loop.** v0 re-spawned
  fzf every 500ms. The user lost cursor position, lost typed
  query, and the orchestrator pane flickered. Switched to
  fzf's `--listen=<sock>` + `notify-debouncer-mini` watching
  `sessions.json`'s parent dir + 1-second heartbeat. fzf
  stays alive across selections; `enter` is bound to
  `execute-silent(tmux switch-client -t {1})+clear-query`,
  which is non-terminal — fzf doesn't exit. The watcher
  pushes `reload(<self> render)` over hand-rolled HTTP/1.1
  to fzf's UDS whenever sessions.json changes. The picker
  feels live: state advances as agents work, you switch
  back and the cursor is where you left it, no flicker.

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

v1 ships in the single slice that's now landed. Two follow-up
slices are tracked separately.

### Shipped — v1

- Single binary, three user-facing verbs + bare invocation.
- Six-state lifecycle with per-state row content and
  heartbeat-driven re-renders.
- Claude hook reporter (six events) wired through user-global
  `setup` / `teardown`.
- Kiro observation-only (registers + lifecycle cleanup; state
  stays cold).
- Event-driven persistent picker via fzf `--listen` +
  `notify-debouncer-mini`.
- User-specified prefix-table keybind via `setup --key X` +
  self-discovering teardown.
- Three test layers: 50+ unit tests, 11+ integration cases
  on a private tmux server, three functional scripts driving
  the user's live server.

### Follow-up — Kiro state tracking

Pick the right injection point (merge into user's default
agent persona vs. ship a project-scoped stub persona),
implement the camelCase event shape, add it to the functional
test scenarios. Tracked in `specs/backlog/`.

### Follow-up — `agent-orch doctor`

Sanity-check skill: tmux ≥ 3.2, fzf ≥ 0.71.0 on PATH; agent
CLIs detected; state dir writeable; orchestrator-switch
keybind currently registered; no stale Claude hooks pointing
at a missing binary; no orphan `<cwd>/.kiro/agents/agent-orch.json`.
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
