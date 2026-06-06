# Feature: agent-session-orchestrator

## Git Setup

- Branch: `feat/agent-session-orchestrator`
- Base: `main`

## Feature Brief

A thin observation-only layer over plain tmux that lets the user
see, at a glance, every coding-agent session running across all
tmux sessions / windows / panes — with each agent's current state
(`running` / `complete` / `unknown`) and a short summary of what
that session is doing — and jump to any of them with a single
keystroke. The user keeps every normal tmux verb they already
know; the orchestrator only adds the registry, the state signal,
and a fzf-driven picker that lives in a dedicated `orchestrator`
tmux session.

The shape, deliberately minimal:

- Coding agents launch through a thin wrapper command:
  `agent-orch wrap <kind> -- <agent-cmd> [args...]`.
- **Claude hooks live user-globally.** The user runs
  `agent-orch setup` once at install; it appends our four
  lifecycle hook entries (`UserPromptSubmit` / `PreToolUse` /
  `PostToolUse` / `Stop`) to `~/.claude/settings.json`,
  tagged so `agent-orch teardown` can remove only ours later.
  Claude's normal settings precedence chain (user → project →
  project-local) runs untouched, so login state, MCP servers,
  and project settings are preserved. The hooks fire on
  **every** `claude` invocation; the `hook` subcommand
  filters by `$AGENT_ORCH_PANE` (set only by our wrapper) and
  exits silently for bare-claude invocations.
- **Kiro hooks live project-scoped.** The wrapper writes
  `<cwd>/.kiro/agents/agent-orch.json` if absent, records in
  the session that it was the creator. On `pane-exited`, if
  no other live Kiro session in the same cwd remains, the
  config is removed. First-creates, others-reuse,
  last-out-deletes (refcount-agnostic). Kiro's user-global
  path is undocumented; project-scoped is the documented
  surface.
- The wrapper itself, regardless of kind, just sets
  `AGENT_ORCH_PANE=%N`, appends a record to `sessions.json`,
  and **`execvp`s the agent**: the wrapper process is replaced
  in place by the agent. The wrapper's pid becomes the agent's
  pid (POSIX guarantee). No parent process, no signal
  forwarding, no `child.pid`/`wrapper_pid` bookkeeping.
- A single `sessions.json` file is the source of truth for
  both registration and state. Every mutation is gated by a
  POSIX advisory lock. The wrapper appends a record on launch;
  the hook subcommand updates the matching record on every
  fired event; the tmux `pane-exited` hook removes it.
- The orchestrator (`agent-orch` bare invocation) ensures a
  dedicated `orchestrator` tmux session exists, switches the
  client to it, and runs the picker loop. Picking jumps the
  client to the agent's pane via `tmux switch-client -t %ID`.
- Liveness is derived from the tmux `pane-exited` hook (registry
  cleanup) plus a `kill(pid, 0)` belt-and-suspenders sweep at
  query time.

The whole tool is one Rust binary — script-style: one
`src/main.rs` with subcommand handlers as flat top-level
functions, types declared next to their use sites, helpers right
after the handler that calls them. Compiled to
`dist/agent-orch/agent-orch`. State lives at
`${XDG_STATE_HOME:-$HOME/.local/state}/agent-orch/` (per-user
runtime data). v1 ships standalone; promoting later to a
flake-installed package or registry-deployed symlink is purely
additive.

## Requirements

### State model (v1)

Two states only:

- `running` — the agent is in the middle of a turn (received a
  user prompt, hasn't reached `Stop` yet).
- `complete` — the agent finished a turn (`Stop` fired). Whether
  it's idle, waiting on a permission prompt, or showing the user
  a question is **not distinguished** in v1. The picker shows
  `complete`; the user reads the pane on jump.

A third pseudo-state, `unknown`, applies when no hook fire has
ever updated the session.

### Single binary, minimal CLI

The orchestrator ships as one Rust binary: `agent-orch`. The
user-facing CLI is **five verbs**:

- `agent-orch setup` — install the machinery: write Claude
  hooks into `~/.claude/settings.json` (idempotent, tagged) and
  install the `M-o` keybind on the live tmux server. Run once
  per machine; re-run after `tmux kill-server` / reboot to
  restore the keybind.
- `agent-orch teardown` — uninstall: remove the tagged Claude
  hooks (preserving anything else in the user's settings) and
  unbind `M-o`.
- `agent-orch wrap <kind> [--cwd <dir>] -- <agent-cmd> [args...]`
  — launch an agent. Registers the pane in `sessions.json`,
  for Kiro writes the project-scoped `.kiro/agents/agent-orch.json`,
  sets `$AGENT_ORCH_PANE`, then `execvp`s the agent. Claude
  hooks are already wired by `setup`; Kiro uses the
  project-scoped config the wrapper just wrote.
- `agent-orch loop` (or `agent-orch` bare — same thing) — open
  the UX. Self-detects whether it's running inside the
  orchestrator tmux session: if not, ensures the session exists
  and `switch-client`s the user to it; if yes, runs the
  event-driven picker. fzf stays alive across selections;
  `enter` runs `tmux switch-client` via `execute-silent` so the
  picker survives the round-trip; the registry file is watched
  via `notify` and `reload(...)` is pushed to fzf over a Unix
  socket whenever a hook updates state.
- `agent-orch doctor` — verify the install: tmux ≥ 3.2, fzf ≥
  0.71.0 on PATH, agent CLIs detected, state dir writeable, the
  `M-o` keybind currently registered, no stale Claude hooks
  pointing at a missing binary, no orphan
  `.kiro/agents/agent-orch.json`.

**Three internal subcommands** — exist only because external
systems (Claude hooks, tmux pane-exited, fzf reload) shell out
to them. Hidden from `--help` via clap's `#[command(hide)]`.
Documented for completeness:

- `agent-orch hook <event-name>` — Claude/Kiro lifecycle event
  callback. Reads JSON payload from stdin. Filters first on
  `$AGENT_ORCH_PANE`; if unset, exits 0 silently (bare claude
  with `setup` installed). Otherwise applies the event to the
  matching record under `flock`. Always exits 0 even on
  internal errors — a failing hook reporter must never block
  the agent's turn.
- `agent-orch unregister <pane-id>` — tmux `pane-exited` hook
  target. Removes the record under `flock` and runs per-kind
  cleanup via the matching `Wrapper` impl (Kiro:
  refcount-agnostic project config removal; Claude: no-op).
- `agent-orch render` — fzf `reload(<cmd>)` target. Prints the
  formatted picker rows to stdout (`<pane_id>\t<row>` per
  line). Also used by `loop` for initial fzf population.

### Wrapper (`agent-orch wrap`)

- Refuses to run outside tmux (no `$TMUX_PANE`) — the registry
  is pane-keyed; running outside tmux has no useful identity.
- If a record already exists for the same pane:
  - if its recorded pid is alive → genuine double-register,
    refuse loud.
  - if dead (agent crashed, pane stayed alive as a shell, etc.)
    → run prior kind's cleanup, replace silently. Closes the
    "I just want to re-wrap in this pane" friction.
- Supported kinds for v1:
  - `claude` — assumes the user has run `agent-orch setup`.
    No per-launch hook config write. Just register + `execvp`.
  - `kiro` — project-scoped `.kiro/agents/agent-orch.json`
    injection with refcount cleanup (see below).
  - any other kind — registers and `execvp`s only; state stays
    `unknown`.
- Registers the launch by appending one record to
  `$STATE_DIR/sessions.json` (under POSIX advisory lock):
  - `pane_id` (`%N` from `$TMUX_PANE`)
  - `pid` (the wrapper's own pid via `getpid()`, which becomes
    the agent's pid post-`execvp` — same process, same pid)
  - `kind`
  - `cwd`
  - `started` (unix seconds)
  - `state` (`unknown` initially)
  - `state_ts` (mirrors `started` initially)
  - `last_prompt` (empty initially)
  - `last_tool` (empty initially)
  - `last_event` (empty initially)
  - `last_event_ts` (`0` initially)
  - `created_kiro_config` (boolean; `true` if this wrapper
    created `<cwd>/.kiro/agents/agent-orch.json`)
- Sets `tmux set-option -p @agent-orch-pane "%N"` on the
  registered pane.
- Installs a one-time global tmux hook on first run (idempotent
  via a marker file under `flock`):
  `tmux set-hook -g pane-exited 'run-shell "<dist>/agent-orch
  unregister #{hook_pane}"'`.

#### Claude path

Claude hooks live in the user's `~/.claude/settings.json`
**user-globally**, installed once via `agent-orch setup`. The
wrapper does no per-launch hook synthesis. `execvp` flow:

1. Set `AGENT_ORCH_PANE=%N` in the environment.
2. `execvp("claude", agent_argv)` — agent argv passed
   through unchanged. No `--settings` flag.

The hooks fire on **every** `claude` invocation while `setup`
is installed, including bare `claude` outside our wrapper. The
`hook` subcommand filters: if `$AGENT_ORCH_PANE` is unset, it
exits 0 silently. Bare claude → no env var → no-op. Wrapped
claude → env var set by step 1 → record updates.

##### Why top-level instead of per-launch settings file

Claude's `--settings <path>` flag is documented as additive —
"keys you set override the same keys in settings.json for this
session" — but in practice (and per the docs' under-specified
treatment of the `hooks` key), passing `--settings` displaces
the user's full settings precedence chain for that launch:
login state, project settings, MCP servers stored outside the
settings file. We hit this on the manual functional test — a
wrapped Claude session was unauthenticated even when the user
was logged in via bare claude. Top-level installation lets
Claude's normal precedence chain run untouched; we just add
hooks on top.

##### Setup / teardown

- `agent-orch setup` reads `~/.claude/settings.json`, ensures
  root + `hooks` are objects, appends our four hook entries
  (each tagged with a marker so `teardown` can find them),
  writes back.
- `agent-orch teardown` reverses: filters out tagged entries,
  prunes empty containers, writes back. If the file becomes an
  empty object, removes it.
- Both are idempotent. `setup` after `setup` is a no-op (the
  file already has our entries; we detect by tag and skip).

##### Tag and merge contract

Each hook entry we add carries a top-level `"agent-orch": true`
field. Claude ignores extra JSON fields. On teardown we filter
on this tag; user-authored entries (no tag) are preserved
verbatim.

```json
{
  "matcher": "",
  "hooks": [{ "type": "command", "command": "<dist>/agent-orch hook Stop" }],
  "agent-orch": true
}
```

##### What this trades off

- **Bare claude pays a small overhead** — every `claude`
  invocation forks the agent-orch binary on each lifecycle
  event. The binary's filter (env-var check → exit 0) runs in
  ~5ms. Across ~10–20 events per turn that's ~50–100ms total —
  noticeable but not painful.
- **Setup is install-time, not launch-time** — users run
  `agent-orch setup` once. Forgetting to run it before
  wrapping → no hooks fire → wrapped Claude sessions stay
  `unknown` state. `agent-orch doctor` will surface this in
  the follow-up.
- **Hook command embeds the binary path** — if the user moves
  the binary, the hook command points at a missing path.
  Claude logs an error but the agent's turn proceeds. Doctor
  flags this for re-`setup`.

#### Kiro path (per L30 feedback)

Project-scoped `.kiro/agents/agent-orch.json` injection with
refcount cleanup. Kiro's hooks schema mirrors Claude's nested
matcher+hooks shape (`matcher: ""` matches all):

```json
{
  "hooks": {
    "Stop": [{
      "matcher": "",
      "hooks": [{ "type": "command",
                  "command": "<dist>/agent-orch hook Stop" }]
    }]
  }
}
```

1. If `<cwd>/.kiro/agents/agent-orch.json` does not exist:
   create the directory if needed, write the file with the
   four hook entries (`UserPromptSubmit`, `PreToolUse`,
   `PostToolUse`, `Stop`). Stamp `created_kiro_config=true`
   on the session record.
2. If the file already exists: leave it alone. Stamp
   `created_kiro_config=false`. Multiple Kiro sessions in the
   same cwd share one config; the first one in created it.
3. Set `AGENT_ORCH_PANE=%N` in the env, `execvp("kiro", ...)`.
4. On `pane-exited` (handled by `agent-orch unregister`):
   regardless of whether *this* record created the file, count
   live records with `kind=kiro` and the same `cwd`; if none
   remain, `rm <cwd>/.kiro/agents/agent-orch.json` (and
   `rmdir` parent dirs best-effort if empty). Then remove the
   session record.

The cleanup is creation-flag-agnostic (any closing Kiro session
checks the live-sibling count) — a flag-only design has an
order-dependent leak when reusers outlive the creator. The
`created_kiro_config` flag is kept for `doctor`-time auditing,
not as the gate on removal.

Edge case: if the wrapper crashes between the file write and
the registry append, a stale `.kiro/agents/agent-orch.json` may
remain. `agent-orch doctor` surfaces this by checking known cwds
against live sessions.

### Hook subcommand (`agent-orch hook <event>`)

- **First, filter on `$AGENT_ORCH_PANE`.** If unset or empty,
  exit 0 silently. This is the load-bearing filter for the
  Claude top-level install: bare `claude` (with our `setup`
  installed) fires our hook command on every event, but only
  wrapped invocations have `AGENT_ORCH_PANE` set in their env.
  Bare claude → exit 0 in ~5ms. Wrapped claude → continue.
- Reads `$AGENT_ORCH_PANE` for the pane id (set by the wrapper
  before `execvp`; the env propagates into the agent and into
  the hook's subshell).
- Reads the hook's JSON payload from stdin (Claude / Kiro both
  pipe the event payload).
- Under `flock` on `sessions.json`:
  1. Read the file.
  2. Find the record matching the pane id. If no record:
     no-op exit 0 (a stale hook fire after unregistration is
     safe to ignore).
  3. Update `last_event` to the event name and `last_event_ts`
     to now.
  4. Event-specific updates:
     - `UserPromptSubmit` → `state=running`,
       `last_prompt=<prompt[:80]>` (read from the JSON's
       `prompt` field).
     - `PreToolUse` → `state=running`,
       `last_tool=<tool_name>`.
     - `PostToolUse` → leave state alone, refresh
       `last_tool=<tool_name>`.
     - `Stop` → `state=complete`.
  5. Write the file back atomically (write to `.tmp`, `rename`
     into place — readers never see partial JSON).
- Always exits 0. A failing hook subcommand must not block the
  agent's turn (esp. critical for Claude `Stop` /
  `UserPromptSubmit`, which exit-2-block).

### Registry cleanup

- Tmux hook installed by the wrapper:
  `pane-exited → agent-orch unregister #{hook_pane}`.
- `unregister`:
  1. Under `flock`, read `sessions.json`.
  2. Run the matching `Wrapper` impl's `cleanup` for the
     record's kind:
     - **Claude**: no-op (the user-global hooks installed by
       `setup` stay in place — they're shared across all
       Claude sessions on the system, not per-pane).
     - **Kiro**: count live `kind=kiro` records in the same
       cwd (excluding this one). If zero, remove
       `<cwd>/.kiro/agents/agent-orch.json` (best-effort
       `rmdir` of parent dirs).
     - **Other**: no-op.
  3. Remove the pane's record. Write back atomically.
- Sweep at query time: when the picker reads `sessions.json`,
  it filters out records whose `pid` is no longer alive
  (`kill(pid, 0)`). Catches the case where the tmux server
  restarted while the orchestrator was down. The sweep also
  runs cleanup on the sessions it removes.

### `loop` — the UX

Picker UX:

- One row per live session.
- Sorted: `running` first, then `complete`, then `unknown`.
  Within each group, most-recently-active first
  (max of `state_ts`, `last_event_ts`, `started`).
- Row format:
  `<state-glyph> <kind> <cwd-tail> · <last_prompt> [· <last_tool>]`
  with `last_tool` shown only when non-empty.
- `--preview` (fzf side panel) shows the full record (state,
  full prompt, last event, started-ago, state age).

`agent-orch loop` (or bare `agent-orch` — same thing) is the
single user-facing UX verb. It self-detects whether it's running
inside the orchestrator tmux session via `$TMUX` +
`tmux display-message -p '#{session_name}'`:

```
if running outside the orchestrator tmux session:
  if !tmux has-session -t orchestrator:
    tmux new-session -d -s orchestrator '<dist>/agent-orch loop'
  tmux switch-client -t orchestrator
  exit
# else: we're inside the orchestrator session; run the picker
spawn fzf --listen=<state>/fzf.sock \
          --with-nth=2.. --track --id-nth=1 \
          --bind 'enter:execute-silent(tmux switch-client -t {1})+clear-query' \
       < <(<self> render)              # initial population

main thread: wait on fzf child
watcher thread:
  notify-debouncer-mini watches <state>/ (parent dir, atomic-rename
  swaps inode). On debounced sessions.json change:
    write `POST / HTTP/1.1\r\nContent-Length:N\r\n\r\nreload(<self> render)`
    to <state>/fzf.sock as a UnixStream. fzf parses HTTP/1.1 over UDS.
  thread exits when the watcher's channel is dropped (on fzf
  child shutdown / Loop::body's stack unwind).
```

**fzf does not exit on selection.** `execute-silent` and
`clear-query` are *non-terminal* actions — fzf runs the
command, captures its exit code, and stays alive with the same
query, cursor, and row list. The terminal actions that *do*
exit fzf are `accept`, `accept-non-empty`, `abort`, `become(...)` —
none are in our bind. So an `enter` press fires `tmux
switch-client`, the user's tmux client lands on the agent's
pane, and fzf is untouched in the orchestrator pane behind the
scenes. Press M-o (the keybind installed by `setup`) and the
tmux client returns to the orchestrator session, with fzf
right there waiting — query cleared, cursor preserved by
content match, rows up to date with whatever reloads the
watcher pushed while the user was away.

The orchestrator pane is occupied by fzf for as long as the
user keeps the picker open. fzf actually exits only on:
- Esc / Ctrl-C (default `abort` binding)
- the orchestrator tmux session being killed
- the fzf process being killed directly

When fzf does eventually exit, the Rust main thread's
`child.wait()` returns, `Loop::body` returns, the watcher
debouncer drops, the watcher thread's channel disconnects
and the thread exits cleanly.

The recursion through `tmux new-session -d ... 'agent-orch loop'`
re-enters the same verb, which detects "now I'm inside the
orchestrator session" and falls through to the picker path. One
verb, one mental model.

`{1}` is the original column-1 (pane id) regardless of
`--with-nth` (which controls display only). `--track --id-nth=1`
keeps the cursor on the same pane id across reloads.

### "Back to orchestrator" UX

- **`agent-orch setup` installs the M-o keybind on the live
  tmux server** (`tmux bind-key -n M-o switch-client -t
  orchestrator`). Live-only — the binding persists for the
  life of the tmux server but is lost on `tmux kill-server`
  or reboot. Re-run `setup` to restore. **Persistent install
  across reboots is a follow-up.**
- Fallback path that always works: run `agent-orch` (no args)
  from any shell pane. Bare invocation does ensure-session +
  switch-client. Useful when you don't have the keybind
  installed (fresh machine, just rebooted, etc.) or when a
  shell pane is more accessible than the keybind.
- The wrapper's per-pane `@agent-orch-pane` user option lets
  future features introspect pane ownership without re-reading
  the registry.

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
  `"agent-orch": true`, leaving every other field of the
  user's settings file alone.
- **No `<cwd>/.kiro/agents/agent-orch.json` orphans.** The
  refcount cleanup contract above is the load-bearing rule.
- **Observation only.** The wrapper `execvp`s the agent; the
  hook subcommand writes a state file; neither sits in the
  agent's I/O path.
- **Public-repo hygiene** (mAId-wide). No internal product /
  team / ticket names in scripts, examples, or this spec.
- **Standalone first.** No mAId registry entry in v1. The
  install path is `dist/agent-orch/` populated by `deno task
  agent-orch:build` (which delegates to `cargo build
  --release`). Promoting to a flake / nix package or a
  registry-deployed symlink is a follow-up.

## Test Strategy

Mapped onto `project.md`'s four-layer test surface (`test:unit`,
`test:smoke`, `test:functional`, `test:all`).

### Unit (`deno task test:unit`) — load-bearing

`deno task test:unit` delegates to `cargo test`. Tests live at
the bottom of `src/main.rs` in a `#[cfg(test)] mod tests`
block. Coverage by typeclass:

- **§1 Session** — `apply_event` per-event state transitions
  (Stop → Complete; UserPromptSubmit → Running with prompt
  truncation; PreToolUse → Running with tool name;
  PostToolUse → tool name only); `format_row` glyph + cwd-tail
  + tool/no-tool variants; `cwd_tail` root-anchored edge
  cases.
- **§2 Store** — `read` on empty / missing / malformed file;
  `mutate` round-trips and observes prior state; concurrent
  serialization via flock.
- **§3 Wrapper** — `Claude::prepare` is a pass-through (argv
  unchanged, no created flag, no per-pane file written);
  `Kiro::prepare` writes `.kiro/agents/agent-orch.json` first
  time + reuser leaves flag false; `Kiro::cleanup` keeps
  config while sibling alive + removes refcount-agnostically
  on close-creator-first ordering; `wrap()` refuses
  double-register on alive pid + replaces stale record on
  dead pid; default `hook` body filters on `$AGENT_ORCH_PANE`
  unset (silent no-op); `hook` updates state correctly via
  both Claude and Kiro impls (proves default-method
  inheritance).
- **§4 Loop** — `render` filters dead pids via signal-0 probe
  and sorts.
- **`setup` / `teardown`** — appends tagged hook entries to
  `~/.claude/settings.json` (or its tempdir equivalent under
  `$HOME` override); idempotent on re-run; preserves
  user-existing entries verbatim; `teardown` removes only
  tagged entries; `teardown` removes the file when our
  entries were the only content.

### Smoke / integration (`deno task agent-orch:integration`) — load-bearing

`tests/agent-orch/integration.sh` (shell-driven, drives the
compiled binary against a private tmux server). Each case sets
`HOME` and `XDG_STATE_HOME` to a tempdir so the test never
touches the user's real settings.

1. Spawn a fresh tmux server on a private socket
   (`tmux -L agent-orch-test`).
2. Run `<dist>/agent-orch wrap claude -- <stub-agent>` inside a
   tmux pane, where `<stub-agent>` is a tiny shell script that
   loops on stdin. Assert `sessions.json` has the expected
   record + `execvp`-preserved pid.
3. Drive each hook event directly with synthetic JSON on stdin
   and `AGENT_ORCH_PANE` set; assert state transitions.
4. List shows the live row; unregister removes it.
5. **Kiro refcount.** Two Kiro sessions in the same cwd. Close
   creator first → config remains (sibling alive). Close
   reuser → config removed (last out, refcount-agnostic).
6. **Wrap refuses outside tmux** (`$TMUX_PANE` unset).
7. **Dead-pid filtering.** Spawn an agent that exits quickly,
   call `unregister` (simulating tmux pane-exited), assert
   `list` returns "(no registered sessions)".
8. **`setup` / `teardown` round-trip.** A pre-existing
   `~/.claude/settings.json` (in the test's `$HOME` tempdir)
   is preserved verbatim through a `setup` + `teardown` cycle.
   With pre-content: tagged entries land alongside; `teardown`
   leaves the original entries intact. Without pre-content:
   `setup` creates the file; `teardown` removes it (empty
   container pruning).
9. Tear down the tmux server.

The smoke test does not hit a real Claude / Kiro binary. Fast
(~3s), depends only on `tmux` + `bash` + the compiled binary.

### Functional (`deno task test:functional`) — user-driven

The full manual functional test plan lives at
`tests/agent-orch/manual-functional.md`. It walks through:

- `agent-orch setup` once before launching agents.
- Three project tmux sessions with mixed window/pane layouts
  (`proj-a` single-pane Claude; `proj-b` two-window with
  split; `proj-c` Kiro+Claude split).
- A fourth `viewer` session to bootstrap the orchestrator.
- Pick → switch-client → M-o round-trip across multiple
  agents.
- Lifecycle cleanup verification (Kiro refcount on
  close-creator-first, dead-pid filtering, full
  `agent-orch teardown` rolling back the install).

### Quality gate

`deno task fmt && deno task lint && deno task check` after every
implementation slice. `fmt` / `lint` / `check` delegate to
`cargo fmt --check`, `cargo clippy -- -D warnings`, and
`cargo check` respectively when the slice touches Rust code; the
existing Deno equivalents still fire on top-level `deno.json`
edits.

## Design

> **Revision note (2026-06-05).** The original design used four
> commented section-headers (`§1 Session`, `§2 Wrapper`,
> `§3 Hook`, `§4 Loop`) over loose free functions. This revision
> promotes them to four real types — typeclasses, in the user's
> terminology — based on PR #18 review feedback (L28, L120, L257
> on commit `847a8cd`). Same behavior, same CLI, same integration
> tests; the change collapses the kind-specific code (today
> scattered as `kind == "claude"` / `kind == "kiro"` branches in
> 5 sites) into two named blocks (`impl Wrapper for Claude` and
> `impl Wrapper for Kiro`) and the lock-and-mutate dance into
> one `Store::mutate` call per call site.

### Layout (script-style)

```
sources/agent-orch/
├── Cargo.toml
├── rust-toolchain.toml          pin: stable, minimal profile
├── src/
│   └── main.rs                  the whole tool, typeclasses top-to-bottom
└── README.md                    install + tmux keybind notes (deferred)

tests/agent-orch/
└── integration.sh               shell-driven E2E: 9 cases on a private tmux server

dist/                            (gitignored; populated by build task)
└── agent-orch/
    └── agent-orch               cargo build --release output
```

**One file by default.** `src/main.rs` carries the whole tool —
the four typeclasses (each with its own `impl` block), the CLI
+ `main`, and `#[cfg(test)] mod tests` at the bottom. Split a
file out only if/when one file genuinely hurts (~1000 LOC is
fine to ignore until then). Splitting earlier is the
over-engineering case the "Rust as better bash" framing rejects.

### `Cargo.toml` dep set

```toml
[dependencies]
clap        = { version = "4", features = ["derive"] }
serde       = { version = "1", features = ["derive"] }
serde_json  = "1"
anyhow      = "1"
nix         = { version = "0.29", features = ["signal", "process"] }
fs2         = "0.4"   # POSIX advisory file locking, no `flock(1)` shell-out

[dev-dependencies]
tempfile    = "3"
```

Six runtime deps, one dev dep. No async runtime, no `thiserror`,
no `tracing` — `anyhow` carries error context with `?` and
that's all v1 needs. `nix::unistd::execvp` for the wrapper's
process-image replacement; `nix::sys::signal::kill` (with
signal `0`) for liveness probes.

### `sessions.json` shape

```json
[
  {
    "pane_id": "%42",
    "pid": 12345,
    "kind": "claude",
    "cwd": "/repo/foo",
    "started": 1717459200,
    "state": "running",
    "state_ts": 1717459380,
    "last_prompt": "fix the broken test in registry_test.ts",
    "last_tool": "Bash",
    "last_event": "PreToolUse",
    "last_event_ts": 1717459382,
    "created_kiro_config": false
  },
  {
    "pane_id": "%43",
    "pid": 12389,
    "kind": "kiro",
    "cwd": "/repo/bar",
    "started": 1717459380,
    "state": "running",
    "state_ts": 1717459381,
    "last_prompt": "explain this function",
    "last_tool": "",
    "last_event": "UserPromptSubmit",
    "last_event_ts": 1717459381,
    "created_kiro_config": true
  }
]
```

Top-level array. Rewrite-on-update under POSIX advisory lock.
At ≤100 records the cost is sub-millisecond and we get atomic
semantics for free.

### Path constants (replacing the trivial path-builder helpers)

The spec previously called for five free functions
(`sessions_path`, `lock_path`, `tmp_dir_for_pane`,
`hook_install_marker`, etc.) that each did one
`state.join(...)`. They become:

```rust
const SESSIONS_FILE: &str = "sessions.json";
const LOCK_FILE: &str = "sessions.lock";
const HOOK_MARKER: &str = ".tmux-hook-installed";
```

Used inline as `state.join(SESSIONS_FILE)` /
`state.join(LOCK_FILE)` / `state.join("tmp").join(pane_id)` /
`state.join(HOOK_MARKER)`. `state_dir()` stays a function (env-
var logic + fail path).

### §1 · `Session` — the record type and operations on it

```rust
struct Session {
    pane_id: String,
    pid: i32,
    kind: String,
    cwd: String,
    started: u64,
    state: State,
    state_ts: u64,
    last_prompt: String,
    last_tool: String,
    last_event: String,
    last_event_ts: u64,
    created_kiro_config: bool,
}

impl Session {
    fn apply_event(&mut self, event: &str, prompt: Option<&str>,
                   tool: Option<&str>, now: u64);
    fn format_row(&self) -> String;
    fn activity(&self) -> u64;       // max(state_ts, last_event_ts, started)
    fn state_group(&self) -> u8;     // 0/1/2 for sort precedence
}
```

`sort_sessions` and `live_only` stay as free functions over
slices — they operate on collections, not single records. A
newtype wrapper just to attach two methods is the
over-abstraction the user pushed back against.

### §2 · `Store` — owns the state-dir, hides the lock

```rust
struct Store { dir: PathBuf }

impl Store {
    fn from_env() -> Result<Self>;            // resolves XDG/HOME
    fn new(dir: PathBuf) -> Self;             // tests pass a tempdir

    /// Eventually-consistent read. No lock — readers tolerate
    /// stale data; the registry rebuilds on the next mutate.
    fn read(&self) -> Result<Vec<Session>>;

    /// Read-modify-write under flock. The closure mutates the
    /// vec in place; Store handles lock + read + atomic write.
    fn mutate<F>(&self, f: F) -> Result<()>
    where F: FnOnce(&mut Vec<Session>) -> Result<()>;

    fn dir(&self) -> &Path;
    fn tmp_dir(&self, pane_id: &str) -> PathBuf;
    fn hook_marker(&self) -> PathBuf;
}
```

Every caller currently doing
```
with_lock(state, || {
    let mut v = read_sessions(state)?;
    ...mutate v...;
    write_sessions(state, &v)
})
```
becomes
```
store.mutate(|v| { ...mutate v...; Ok(()) })
```

Three lines collapse to one. The atomic-write mechanics
(per-pid tmp + `rename`) live as private helpers the `Store`
impl uses.

### §3 · `Wrapper` trait — typeclass with `Claude` and `Kiro` impls

```rust
trait Wrapper {
    fn kind(&self) -> &str;

    /// Per-kind preparation at wrap time. Returns (program, argv)
    /// for execvp + any flag the session record needs (today:
    /// only Kiro's "I created the project config" bit).
    /// Claude's `prepare` is a no-op — its hooks live globally
    /// via `setup`, not per-launch. Kiro's writes the project-
    /// scoped agent config if absent.
    fn prepare(&self, ctx: &WrapCtx) -> Result<Prepared>;

    /// Per-kind cleanup on unregister. Kiro runs the refcount-
    /// agnostic `.kiro/agents/agent-orch.json` removal when no
    /// live Kiro session shares the cwd. Claude is a no-op —
    /// its global hooks are removed only by `teardown`.
    fn cleanup(&self, store: &Store, removing: &Session,
               others: &[Session]) -> Result<()>;

    /// Hook handling — DEFAULT method body, identical for all
    /// kinds today. Filters on `$AGENT_ORCH_PANE` (set only by
    /// our wrapper) and exits silently if unset — that's how
    /// bare-claude (with our `setup` installed) becomes a no-op.
    /// A future kind whose stdin payload differs can override.
    fn hook(&self, store: &Store, pane_id: &str, event: &str,
            stdin: &mut dyn Read, now: u64) -> Result<()> {
        // if AGENT_ORCH_PANE unset → return Ok silently;
        // else read payload, find matching record, apply event,
        // write back through store.mutate.
    }
}

struct Claude;
struct Kiro;
struct Other(String);   // register-only — no per-kind config

struct WrapCtx<'a> {
    store: &'a Store,
    self_path: &'a Path,
    pane_id: &'a str,
    cwd: &'a Path,
    agent_argv: &'a [String],
}

struct Prepared {
    program: String,
    argv: Vec<String>,
    created_kiro_config: bool,
}

fn wrapper_for(kind: &str) -> Box<dyn Wrapper> {
    match kind {
        "claude" => Box::new(Claude),
        "kiro"   => Box::new(Kiro),
        _        => Box::new(Other(kind.to_string())),
    }
}
```

`Claude::prepare` is a **no-op** (returns argv unchanged,
`created_kiro_config: false`). Claude's hooks live in
`~/.claude/settings.json` user-globally via `agent-orch setup`,
not per-launch — so wrap has nothing per-kind to do beyond
register + execvp. Kept on the trait (rather than special-
casing `Claude` out of the dispatch) so the symmetry stays:
every kind has a `prepare` and `cleanup`, even when they're
trivial.

`Kiro::prepare` writes `<cwd>/.kiro/agents/agent-orch.json` if
absent, returns `created_kiro_config: <bool>`, leaves argv
unchanged.

`Other::prepare` returns argv unchanged, `created_kiro_config:
false` — the wrapper still registers the pane (so the picker
sees it), but no per-kind hook config means the session stays
in `unknown` state for its lifetime.

`Claude::cleanup` is a **no-op**. Nothing per-pane to clean —
the `setup` install lives at the user level and is removed
explicitly by `teardown`.

`Kiro::cleanup` checks `others` for any live `kind=kiro &&
cwd==removing.cwd`; if none, removes the project-scoped
`.kiro/agents/agent-orch.json`. **Refcount-agnostic** —
ignores the `created_kiro_config` flag so close-creator-first
ordering still removes the file when the last reuser exits.

`Other::cleanup` is a no-op.

The `hook` default method body lives on the trait (typeclass
shape). All three impls inherit it. Body shape:

```rust
fn hook(&self, store, pane_id, event, stdin, now) -> Result<()> {
    // Filter: bare claude with `setup` installed has no
    // AGENT_ORCH_PANE — skip silently. The CLI dispatch
    // layer also checks; the trait check is a belt-and-
    // suspenders for direct calls.
    if std::env::var("AGENT_ORCH_PANE").ok().filter(|s| !s.is_empty()).is_none() {
        return Ok(());
    }
    // ... read stdin, find record by pane_id, apply event,
    //     write back through store.mutate
}
```

The hook subcommand dispatch in `main` looks up the kind from
the registry (unlocked read; fails soft if registry is
unreadable) and calls `wrapper_for(kind).hook(...)`. If no
record is found, dispatches through `Other` whose default
hook body just no-ops.

### §4 · `Loop` — self-contained picker

```rust
struct Loop<'a> { store: &'a Store }

impl<'a> Loop<'a> {
    fn new(store: &'a Store) -> Self;

    /// Read sessions, filter live pids, sort, format rows.
    /// Returns (pane_id, formatted_row) pairs.
    fn render(&self) -> Result<Vec<(String, String)>>;

    /// Render rows, run fzf, write the chosen pane id to stdout.
    fn pick(&self, stdout: &mut dyn Write) -> Result<()>;

    /// Ensure the orchestrator tmux session exists, switch
    /// client. The picker body itself runs inside the spawned
    /// orchestrator session via the same `agent-orch loop` verb
    /// (it self-detects via `$TMUX` and falls through to body()).
    fn run(&self, self_path: &Path) -> Result<()>;

    /// Picker body — event-driven persistent fzf with
    /// `--listen=<sock>`, watcher pushes `reload(...)` over
    /// the socket on each `sessions.json` change. fzf stays
    /// alive across selections (`execute-silent` is
    /// non-terminal); only Esc / Ctrl-C / pane death exits it.
    /// Returns when the fzf child exits.
    fn body(&self) -> Result<()>;
}
```

The free functions `render_rows`, `pick`, `run_loop`,
`loop_body` move into `impl Loop`. CLI dispatch becomes
`Loop::new(&store).pick(...)` etc.

### Wrapper flow (typeclass-shaped)

```
agent-orch wrap <kind> -- <argv>

  Cli::parse → store := Store::from_env()
                w     := wrapper_for(kind)
                ctx   := WrapCtx { store, self_path,
                                   pane_id, cwd, agent_argv }

  store.mutate(|sessions| {
    if existing record for pane_id and its pid is alive: refuse loud.
    if existing record but pid is dead: replace silently
        (run prior kind's cleanup, drop the record).
    let prepared = w.prepare(ctx)?;
      // claude: no-op (returns argv unchanged)
      // kiro:   ensure <cwd>/.kiro/agents/agent-orch.json, set created flag
      // other:  argv unchanged, no flag
    sessions.push(Session::new(ctx, w.kind(), prepared.created_kiro_config));
    Ok(prepared)
  })?
    // The Kiro race (close-during-register) is closed here:
    // ensure_kiro_config and the registry write share one
    // critical section, so unregister can never observe the gap.

  install global tmux pane-exited hook (idempotent via marker)
  tmux set-option -p @agent-orch-pane <pane_id>
  setenv("AGENT_ORCH_PANE", pane_id)
  exec_agent(prepared.program, prepared.argv)   // execvp; never returns on success
```

The wrapper's pid is preserved across `execvp` (POSIX
guarantee), so the `pid` recorded in the registry is the live
agent's pid the moment the next instruction runs.

### Unregister flow (typeclass-shaped)

```
agent-orch unregister <pane>

  store.mutate(|sessions| {
    let removing = sessions.remove(pane_id)?;        // None → no-op
    wrapper_for(&removing.kind).cleanup(store, &removing, sessions)?;
    Ok(())
  })
```

The whole per-kind branching disappears into `Wrapper::cleanup`.

### Decoupling: `wrap` and `loop` are independent

The wrapper and the orchestrator loop never speak directly.
They communicate only through `sessions.json` on disk:

- **`wrap` is useful on its own.** It registers the launch,
  installs the global tmux `pane-exited` hook (idempotent —
  marker-file gated), synthesizes per-launch hook config, then
  `execvp`s the agent. Nothing in this path waits for, signals
  to, or assumes the existence of a running loop. Hooks fire
  and update `sessions.json` whether or not anyone is watching.
- **`loop` is just a viewer.** It reads `sessions.json` and
  drives `fzf`. Starting it surfaces whatever's already
  accumulated; stopping it leaves the registry untouched.
- **The user's mental model.** Launch agents through `wrap`
  whenever convenient; open `loop` (via bare `agent-orch` or
  the user's tmux keybind) only when you want to see the
  dashboard. The two halves can run in either order, neither
  blocks the other, and the registry survives across loop
  restarts (a fresh `loop` invocation just opens onto the
  current state).
- **One small consequence for v1.** `agent-orch unregister
  <pane-id>` is callable directly (it's the tmux hook target);
  it doesn't go through the loop either. So pane death cleans
  up the registry whether the loop is up or down.

This decoupling is load-bearing for the "throw `wrap` at
agents now, look at the dashboard later" workflow that
motivates the feature.

### Why this shape

- **Four typeclasses, one file.** `Session`, `Store`,
  `Wrapper`, `Loop` are the user's mental model. Promoting them
  to actual types (struct/trait + impl blocks) makes the file
  read as four ergonomic surfaces top-to-bottom, each with its
  own contract. The bash version of this is 3 small files; the
  Rust version is one file with four named blocks. Pre-
  modularization (one file per typeclass) is the over-
  engineering case the "Rust as better bash" framing rejects.
- **Trait carries its weight.** Today there are exactly two
  real branches (Claude vs Kiro) and the variation pattern is
  identical: synth-or-ensure config → contribute to argv →
  cleanup on exit. A trait with three methods (`prepare`,
  `cleanup`, default `hook`) captures it. If a third kind
  (Codex, Aider) ever lands, it slots in as one more `impl`.
  Without the trait, the kind-specific code is 5 scattered
  branches; with it, two named blocks.
- **`hook` as a typeclass default method.** The hook
  *handling* is identical across kinds (filter on
  `$AGENT_ORCH_PANE` → read stdin → flock → apply event →
  write back). The hook *injection* differs (Claude:
  user-global via `setup`; Kiro: project-scoped via wrap).
  So `hook` lives as a default method body shared by every
  impl; injection happens elsewhere (in `Wrapper::prepare`
  for Kiro, in the `setup` subcommand for Claude). A future
  kind whose payload shape differs can override.
- **`execvp`, not spawn-as-child.** `nix::unistd::execvp`
  replaces the wrapper's process image with the agent's. The
  wrapper's pid becomes the agent's pid because POSIX preserves
  the pid across `execvp`. No parent process consuming RAM, no
  signal-forwarding ladder, no `child.pid`/`wrapper_pid`
  bookkeeping. This is the design the bash prototype always
  used; reaching for Rust restores it.
- **Each kind gets the simplest documented surface.** Claude's
  `--settings <path>` is documented as additive but in practice
  displaces parts of the precedence chain (login state, MCP
  servers stored outside `settings.json`); we landed on
  *user-global tagged install* via `agent-orch setup`, with the
  hook subcommand filtering by `$AGENT_ORCH_PANE`. Kiro's
  user-global path is undocumented; we landed on
  *project-scoped per-cwd* via `<cwd>/.kiro/agents/`, with
  refcount-agnostic cleanup. The asymmetry is honest — each
  kind uses its most reliable documented surface. The
  user-facing CLI (`wrap` / `hook` / `pick` / `unregister`) is
  the same shape for both.
- **`Store::mutate` collapses the lock dance.** Every
  read-modify-write site went from 3 lines (with_lock + read +
  write) to 1 (`store.mutate(|v| ...)`). The lock is hidden
  inside `Store`; callers can't forget to take it.
- **Single sessions.json beats split files.** Source of truth
  in one place; readers and writers serialize through one
  lock; rewrite-on-update is fine at our scale.
- **Two states (`running`/`complete`).** Per the user's brief.
  Distinguishing waiting-on-permission from idle is a small v2
  with Claude's `Notification` event.
- **Dedicated orchestrator session, not popup.** Persistent
  dashboard pane to extend later (live refresh, summary
  preview); clean "M-o anywhere → orchestrator" verb. `Loop`
  hides this — a future popup-overlay variant slots in as a
  second `Loop` impl without touching `wrap` / `hook` /
  `unregister`.
- **Per-pane cleanup co-located with `pane-exited`, not RAII.**
  With `execvp`, the wrapper process is gone before the agent
  exits; Rust's `Drop` would fire too early. `Wrapper::cleanup`
  runs in the unregister handler — for Kiro, that's the
  refcount check + project config removal. For Claude it's a
  no-op (the user-global hooks installed by `setup` outlive any
  individual pane and are removed only by `teardown`).

### Trade-offs we're accepting

- `complete` doesn't distinguish "idle" from "waiting on user".
  User reads the pane after jumping. v2 with `Notification`.
- Wrapper requires the user to launch through it. Bare `claude`
  / `kiro` invocations are invisible to the registry. README
  documents the launch verbs; shell aliases are an obvious
  follow-up.
- **Claude users must run `agent-orch setup` once.** Forgetting
  → wrapped Claude sessions stay forever-`unknown` in the
  picker (no hooks fire → no state updates). The picker shows
  this clearly; doctor (deferred) will flag it explicitly.
- **Bare `claude` invocations pay a small overhead per event.**
  ~5ms env-var check + exit per fire, ~10–20 fires per turn =
  ~50–100ms total. Visible if measured; not painful.
- **`setup`'s recorded binary path can go stale** if the user
  moves the binary or rebuilds to a different path. Re-running
  `setup` refreshes the path (idempotence detects the stale
  entry by tag, rewrites the command). Documented as a doctor
  follow-up.
- `dist/` is rebuilt by the user; no auto-rebuild on source
  edit. Cargo's incremental builds make `cargo build` ~1-2s
  on changes; `cargo check` is sub-second.
- Cold cargo build adds ~30-60s to a fresh checkout. Tolerable;
  flake-cached after the first build.
- Single-file growth: at ~1000 LOC `main.rs` will start hurting
  to navigate. We'll split out the largest cohesive piece (the
  `Session` model + apply + sort, into `sessions.rs`) when that
  threshold hits, not before.
- No multi-host support. Local-only by design.
- Kiro cleanup is best-effort: if the wrapper crashes between
  the file write and the registry append, a stale config
  remains. Doctor surfaces this. Acceptable.

### What's deliberately not built

- No background daemon, no IPC socket, no proxy of agent I/O.
- No notification surface (OS toasts, status-bar icons).
- No automatic `~/.tmux.conf` edits.
- No TUI yet — the picker is fzf. The intended v2 TUI path is
  **`ratatui`** (the de-facto standard for Rust terminal UIs:
  Sparkline / Chart / Tabs / Table widgets out of the box,
  excellent incremental rendering, no compat-layer wobble).
  Promoting the picker to a live dashboard (sortable columns,
  refresh-on-state-change, summary preview side panel) is a
  feature-flagged port of `cmd_pick` and `cmd_loop` — the
  `Session` model and everything else stays unchanged. Add the
  flag from day one (`#[cfg(feature = "tui")]`) so the v2
  cutover is a flag flip, not a code reshape.

## Implementation Plan

The functional surface (wrap, hook, list, pick, unregister,
loop-body) and the typeclass refactor (`Session`, `Store`,
`Wrapper` trait + Claude/Kiro/Other impls, `Loop`) shipped
across commits `0e0281c` … `463eccc`, plus two correctness
fixes (hook schema + stale-record auto-replace) in `db902ad`.
Setup/teardown for Claude user-global hook install shipped
in `b416c50`.

**The remaining work is one slice: prune the CLI to five
user-facing verbs, convert `loop` from a poll loop to event-
driven persistent fzf, and add a live-server-only tmux keybind
to `setup`.**

### Slice — minimal CLI + event-driven `loop` + tmux keybind in `setup`

One commit, one Code Review Gate, one push.

#### Event-driven loop-body

Replace the current poll-shaped `Loop::body`:

```
while true:
    pid = fzf < (render rows)
    tmux switch-client -t pid
```

With a **persistent fzf** that survives across selections and
re-renders on registry changes:

```
spawn fzf --listen=<state>/fzf.sock \
          --with-nth=2.. --track --id-nth=1 \
          --bind 'enter:execute-silent(tmux switch-client -t {1})+clear-query' \
       < <(<self> render)        # initial population

watch <state>/ via notify (parent dir, atomic-rename swaps inode)
on debounced change:
    push `reload(<self> render)` to fzf.sock as HTTP/1.1 over UnixStream
wait for fzf to exit (Ctrl-C, ^D, etc.)
```

Two new subcommands wire this in:

- `agent-orch render` — prints the formatted picker rows to
  stdout. Same logic as `Loop::render` produces today, but
  exposed as its own subcommand so fzf's `reload(<cmd>)`
  invocation can call it.
- `loop-body` body changes from poll to event-driven; the
  startup is: ensure socket-path is fresh (rm if present),
  spawn fzf, poll-existence the `.sock` file (~10–50ms typical
  bind window), start the watcher, push the first reload (so
  the picker shows live state even without a session change).

**fzf flags (load-bearing):**
- `--listen=<sock>` — Unix socket path ending in `.sock`. fzf
  speaks HTTP/1.1 over it; POST body is raw `reload(...)`
  action text.
- `--with-nth=2..` — display only columns 2.. (the formatted
  row); column 1 (pane id) stays in the placeholder substrate.
- `--track --id-nth=1` — fzf 0.71.0+; cursor follows the row
  whose column-1 value matches across reloads. Plain `--track`
  is index-based and doesn't survive reload.
- `--bind 'enter:execute-silent(tmux switch-client -t {1})+clear-query'`
  — `execute-silent` runs `tmux switch-client` without exiting
  fzf; `+clear-query` clears the typed search; `{1}` is the
  raw column-1 value (pane id).

**Reload-push transport:** hand-rolled HTTP/1.1 over
`UnixStream::connect`. fzf parses the same protocol over UDS
as it does over TCP. ~30 lines of Rust, std-only, no HTTP
client crate.

**Watcher:** `notify` 8.x + `notify-debouncer-mini` 0.7.
Watch the *parent dir* of `sessions.json` (atomic-rename
swaps the inode; per-file watch goes stale). Debounce window
~150ms. Cross-platform via `recommended_watcher`.

**Architecture (no async):** main thread spawns fzf as child;
background thread runs the watcher; `mpsc::channel` glues
them. Main thread `wait`s on the fzf child; the watcher
thread on each debounced event opens a fresh `UnixStream`,
writes the reload POST, drops the connection.

**Lifecycle.** fzf is the loop. Selections fire
`execute-silent(tmux switch-client ...)` — non-terminal — so
fzf stays alive across an arbitrary number of picks. fzf
exits only when the user hits Esc / Ctrl-C, kills the
orchestrator tmux session, or kills the fzf process directly.
On exit, `child.wait()` returns, `Loop::body` returns, the
debouncer drops, and the watcher thread's channel disconnects
cleanly.

#### tmux keybind in `setup` (live-only)

`agent-orch setup` already writes Claude hooks. Add: run
`tmux bind-key -n M-o switch-client -t orchestrator` against
the live tmux server so the user gets the round-trip
keybind immediately. **Live-only.** Persistence across
`tmux kill-server` / reboot is deferred — running `setup`
again restores the binding. No edits to the user's tmux
conf in this slice.

`agent-orch teardown` symmetrically runs
`tmux unbind-key -n M-o`.

Both calls swallow errors with a one-line warning if the
live-server install fails (no tmux running, wrong socket,
etc.). The Claude-side install is independent and proceeds.

#### Why event-driven, not poll

Today's poll loop has two real problems the user named:

1. **Stale view between selections.** If a hook fires while
   fzf is up, the user sees the old state until they Esc
   and re-pick. With persistent fzf + reload, state lands
   in <100ms.
2. **Lost query and cursor on every round-trip.** Restarting
   fzf drops what the user typed and where the cursor was.
   `--track --id-nth=1` + `execute-silent` keeps both.

The cost is real — two new deps (`notify`,
`notify-debouncer-mini`) — but the UX win is load-bearing
for "open the orchestrator and leave it open."

#### Implementation steps

1. **Prune the CLI surface.**
   - Drop `Cmd::Pick` and `cmd_pick` (no longer called by
     anything once `loop` is event-driven).
   - Drop `Cmd::List` and `cmd_list` (debugging-only verb;
     `cat sessions.json | jq` covers the use case).
   - Mark `Cmd::Hook`, `Cmd::Unregister`, `Cmd::Render`,
     `Cmd::LoopBody` with `#[command(hide = true)]` so they
     don't appear in `--help` (still callable; hooks/tmux/fzf
     shell out to them).
   - Rename `Cmd::LoopBody` → `Cmd::Loop` (user-visible).
     `Loop::run` and `Loop::body` collapse to one function;
     the verb's body self-detects whether we're inside the
     orchestrator session via `$TMUX` + `tmux display-message
     -p '#{session_name}'`. Outside → ensure-and-switch.
     Inside → run the event-driven picker.
2. Add `notify = "8"` + `notify-debouncer-mini = "0.7"` to
   `Cargo.toml`.
3. Add `Cmd::Render` to the CLI enum (hidden); implement
   `cmd_render` that runs the same row-formatting as today's
   `Loop::render` against `Store::from_env` and prints the
   rows to stdout (one `<pane_id>\t<row>` per line).
4. Rewrite `Loop::body` (the picker-side branch of
   `Cmd::Loop`):
   - Build the socket path (`store.dir().join("fzf.sock")`)
     and remove any existing file at that path.
   - Spawn fzf with the flags above; pipe `agent-orch render`
     output to its stdin for initial population.
   - Poll-existence the `.sock` file with a short retry loop
     (50ms × 20 attempts).
   - Spawn a background thread running `notify-debouncer-mini`
     watching `store.dir()` non-recursively. On each
     debounced event whose path matches `sessions.json`,
     write a reload POST to the socket.
   - Wait for the fzf child to exit. On exit, drop the
     watcher (channel close → thread exits).
5. Update `Loop::run` (the outside-orchestrator branch) — no
   change to its existing behavior; it still ensures the
   `orchestrator` session and `switch-client`s. The recursion
   target updates from `<self> loop-body` to `<self> loop`
   (same verb; self-detects on re-entry).
6. Update `cmd_setup` to also run
   `tmux bind-key -n M-o switch-client -t orchestrator` after
   the Claude-side install. Soft-fail on tmux-not-running
   with a single-line warning to stderr.
7. Update `cmd_teardown` symmetrically: `tmux unbind-key -n M-o`.
8. Tests:
   - Drop tests that exercise the pick/list verbs as separate
     entry points. Keep the rendering-related coverage by
     calling `cmd_render`.
   - Unit-test `cmd_render` produces the expected rows for a
     seeded store.
   - Unit-test the reload-message construction (build the
     `POST / HTTP/1.1\r\n...\r\n\r\nreload(...)` string from a
     given socket path + render command, no network IO).
   - The full event-driven loop is hard to unit-test (needs
     a real fzf binary + a real socket); integration is the
     load-bearing test.
9. Integration:
   - Drop case 5 ("list emits the live row") and case 6 still
     exists for unregister behavior — fold the registry-
     accumulated-state assertion into a direct `jq` read of
     sessions.json instead of going through the dropped `list`
     verb.
   - New case 11 verifies setup/teardown register and
     unregister the M-o keybind on the live tmux server:
     ```
     env HOME=$SETUP_HOME $BIN setup
     T show-options -g | grep "M-o" || fail
     env HOME=$SETUP_HOME $BIN teardown
     T show-options -g | grep "M-o" && fail
     ```
   - The full event-driven loop body is exercised by the
     manual functional test (real fzf, real tmux, real
     round-trip).
10. Manual functional plan update: the "open the orchestrator,
    pick, M-o back, pick again" loop now has the explicit
    verification "your typed query and cursor position survive
    the round-trip; state updates from hooks land in the
    picker without you having to do anything."
11. Code Review Gate.
12. Push.
13. Closure: Decision Log entry capturing the CLI prune, the
    live-only tmux-keybind decision, and the event-driven
    loop design.

### Out of scope for this slice

- **Persistent tmux keybind across reboots.** The user's
  `~/.tmux.conf` may be nix-managed and read-only. Live-only
  install ships first to validate the UX; a persistent
  install (sidecar conf + one user-added source-line) lands
  as a follow-up if reboots become annoying.
- `agent-orch doctor` — still deferred. Will gate fzf
  version (≥ 0.71.0 for `--id-nth`, ≥ 0.66.0 for
  `--listen=<sock>`), warn on missing live keybind, audit
  stale Claude hooks.
- ratatui v2.

### Old slice (shipped) — `agent-orch setup` / `teardown` for Claude

One commit, one Code Review Gate, one push.

1. **CLI additions.**
   - Add `Cmd::Setup` and `Cmd::Teardown` to the clap enum.
2. **`agent-orch setup`.**
   - Resolve `~/.claude/settings.json` (use `$HOME` per the
     research). Create the parent dir if absent.
   - Read existing JSON; ensure root is an object (bail loud
     on a non-object root, same shape as the merge logic
     already in the codebase).
   - Ensure `hooks` is an object; for each of the four events
     (`UserPromptSubmit` / `PreToolUse` / `PostToolUse` /
     `Stop`):
     - Ensure the event's array exists.
     - Append `{matcher: "", hooks: [{type: "command",
       command: "<self> hook <ev>"}], "agent-orch": true}` if
       no entry tagged `"agent-orch": true` already exists for
       that event (idempotent).
   - Write back atomically.
3. **`agent-orch teardown`.**
   - Read `~/.claude/settings.json` (no-op if absent).
   - For each of the four events, filter out entries with
     `"agent-orch": true`. If the event's array becomes empty,
     remove the event key. If `hooks` becomes empty, remove
     the `hooks` key.
   - If the resulting JSON is `{}`, remove the file. Otherwise
     write back atomically.
4. **`Wrapper::prepare` for Claude.**
   - Demote to a no-op: returns `Prepared { program:
     argv[0].clone(), argv: argv.to_vec(), created_kiro_config:
     false }`. Drop `synth_claude_settings`, `merge_claude_hooks`,
     `build_agent_argv` from the wrapper hot path (move
     `merge_claude_hooks` into the `setup` subcommand; it's
     the only caller now).
5. **`Wrapper::cleanup` for Claude.**
   - Demote to a no-op (`Ok(())`).
6. **Drop per-pane tmpdir machinery.**
   - `Store::tmp_dir` and `tests/agent-orch/integration.sh`
     case-6's "tmp dir not removed" check go away (they were
     Claude-specific).
   - `WrapCtx` loses its `user_claude_settings: &Path` field
     (no longer read at wrap time).
7. **Filter on `$AGENT_ORCH_PANE` in the `hook` subcommand.**
   - The default trait method body and / or the CLI dispatch
     check the env var first; if unset, return `Ok(())` silently.
     Bare claude → no env var → no-op exit code 0. Wrapped
     claude → env var set → existing logic runs.
8. **Tests.** Update the existing test surface:
   - Remove `claude_prepare_*` tests that asserted the synth
     file shape (no longer applicable). Replace with
     `setup_appends_tagged_entries`,
     `setup_idempotent_doesnt_duplicate`,
     `setup_preserves_user_existing_entries`,
     `teardown_removes_only_tagged_entries`,
     `teardown_removes_empty_file`.
   - Remove `claude_cleanup_removes_per_pane_tmpdir` — Claude
     cleanup is now a no-op.
   - Remove `unregister_claude_removes_per_pane_tmpdir` — same.
   - Add a `hook_filters_when_pane_env_unset` test that calls
     `Wrapper::hook` (or the dispatch wrapper) without the
     env var and asserts a clean Ok with no registry mutation.
   - Kiro tests stay unchanged.
9. **Integration script update.**
   - Drop case 6's "tmp dir not removed" assertion.
   - Add case 10 (or fold into existing): `agent-orch setup`
     writes hooks; `agent-orch teardown` removes them; the
     resulting `~/.claude/settings.json` is exactly what was
     there before (or absent if it was absent and we never
     wrote anything else). Use `XDG_STATE_HOME`-equivalent
     isolation — set `HOME` to a tempdir for the test so we
     don't touch the user's real settings.
10. **Manual functional plan update.**
    - Rewrite the pre-flight section: user runs `agent-orch
      setup` once before the test plan begins; `agent-orch
      teardown` after the test plan ends.
    - Drop the "exec claude --settings <path>" mention in the
      Claude section; remove the per-pane tmpdir verification
      step.
11. **Code Review Gate** (kdevkit §7).
12. **Push.**
13. **Closure** (kdevkit §8) — Decision Log entry recording the
    pivot. Soft verify `project.md` — likely no change
    (Layout's `tests/agent-orch/integration.sh` still applies).

### Out of scope for this slice

- `agent-orch doctor` — still deferred. The new failure mode
  ("setup'd hooks point at a now-missing binary") will be
  covered when doctor lands.
- Kiro's user-global path. Kiro's `~/.kiro/settings/cli.json`
  is undocumented; we keep Kiro project-scoped per the
  Decision Log entry below.
- A `migrate` verb that detects pre-pivot per-pane settings
  files in `$STATE_DIR/tmp/` and removes them. The previous
  unregister path already does this; on first wrap after the
  pivot lands, the user's pane is fresh.

### Risk notes

- **Forgetting to run `setup` before wrapping.** The wrapper
  doesn't know whether `setup` ran. Wrapped Claude sessions
  silently fail to update state — the registry shows them
  forever-`unknown`. Two mitigations: (a) the failure mode is
  visible (the picker shows them as `unknown`); (b) the
  manual test plan and README open with `agent-orch setup`.
  Doctor will catch it explicitly when it lands.
- **Bare claude pays a forking cost on every event.** ~5ms ×
  ~10–20 events per turn = ~50–100ms total. Visible if you're
  measuring; not painful in practice. Cost is the env-var
  check + immediate exit.
- **Tagged-entry detection.** The cleanup's filter
  (`"agent-orch": true`) relies on Claude not stripping
  unknown JSON fields when reading settings. Per the docs,
  Claude is permissive about extra fields. If a future Claude
  release tightens this and rewrites the user's settings.json,
  our tag is lost and `teardown` becomes a no-op — leaving
  hooks behind. Doctor will surface this; manual recovery is
  one `jq` filter.
- **Re-run race.** If a user runs `setup` while a Claude
  session is already running, the hooks land in
  `~/.claude/settings.json` mid-session. Claude reloads
  settings on next read; we don't promise this fires for
  in-flight sessions. Acceptable.
- **Hook command embeds the binary path.** If the user moves
  the binary or rebuilds to a different `dist/` location,
  `setup`'s recorded path goes stale. They re-run `setup`;
  the idempotence check sees the existing entry's command
  doesn't match the new path and… today, no-ops (since
  there's *some* tagged entry). We'd need `setup` to refresh
  the command if it's stale. Worth coding, small cost. This
  goes in the slice.
- **Claude's hook schema drift.** We discovered the nested
  matcher+hooks shape mid-implementation. If Claude changes
  the schema again, both `setup` and `teardown` need updates.
  Easy to fix; integration test catches it.

## Session Log

<!-- date · what was done · decisions made -->

- 2026-06-04 · Initial spec drafted.
- 2026-06-04 · Plan revised after PR #18 feedback. Kiro lifted
  to first-class via project-scoped `.kiro/agents/` injection
  with refcount cleanup; language consolidated to all-Deno
  single binary; `$STATE_DIR` clarified as runtime data.
- 2026-06-04 · Verified Deno↔Node portability. Spec adjusted
  to spawn-as-child + `runtime.ts` shim (since neither
  runtime exposes `execvp`).
- 2026-06-04 · Pivoted to **Rust, script-style**. The
  spawn-as-child workaround was forced by Deno; Rust's
  `nix::unistd::execvp` restores the bash prototype's design
  (wrapper replaces itself with the agent, pid preserved).
  Layout collapsed from 9 files to 1 `src/main.rs` —
  one-file-by-default is the operative discipline. v2 TUI
  path moved from Ink to `ratatui`. Drops the `runtime.ts`
  shim (Rust *is* the portable target). Adds `rust-overlay`
  to `flake.nix`; cargo wrappers thread through `deno task`
  to keep `project.md`'s "every dev verb is `deno task`"
  invariant.
- 2026-06-04 · PR #18 L486 review. Made the `wrap`/`loop`
  decoupling explicit — `wrap` does not depend on the loop
  running; the loop is a viewer that can be opened anytime
  onto the live registry. Added a Design subsection and
  clarifying comment in the wrapper flow pseudo-code.
- 2026-06-05 · Implementation landed via four progressive slices
  (skeleton → wrap claude → review-driven simplifications) and a
  final consolidation (`bc211fd`) that compressed the design to
  the four-section shape the user named: §1 Session, §2
  Wrapper, §3 Hook, §4 Loop — but as commented headers over
  loose free functions, not as actual types. The consolidation
  closed two correctness bugs caught at code review (kiro
  register/unregister race; parallel-test env-var pollution).
- 2026-06-05 · **Re-opened Planning Review Gate** after PR #18
  review on commit `847a8cd`. User asked to promote the four
  sections from comment-headers to four real typeclasses
  (`Session`, `Store`, `Wrapper` trait + Claude/Kiro impls,
  `Loop`), drop the trivial path-builder helpers in favor of
  `const &str` + inline `.join`, and put the kind-agnostic
  `hook` handler on the `Wrapper` trait as a default method
  body. Rewrote the Design and Implementation Plan sections to
  describe the typeclass shape; the work was one refactor slice
  preserving every shipped behavior. Landed in `463eccc`.
- 2026-06-05 · Manual functional test caught two correctness
  bugs in `463eccc`: (1) Claude rejected our synthesized
  settings.json because we were producing the flat hooks shape
  instead of the nested `matcher`+`hooks` array Claude requires;
  (2) re-wrapping in a pane whose previous agent had exited
  refused loud instead of replacing the stale record. Both
  fixed in `db902ad`: schema corrected; `wrap` now probes
  existing-record liveness and replaces if the recorded pid is
  dead, refuses only when alive.
- 2026-06-05 · Manual functional test caught a third issue:
  `agent-orch wrap claude -- claude` launched an
  unauthenticated Claude even though bare `claude` was logged
  in. Root cause: `--settings <path>` displaces parts of
  Claude's settings precedence chain — our synthesized file
  dropped login state, project settings, and MCP servers stored
  outside `settings.json`. **Re-opened Planning Review Gate**
  for a design pivot. Researched alternatives:
    - `--hooks-file`: doesn't exist.
    - `CLAUDE_CONFIG_DIR`: not in current docs.
    - Project-scoped `<cwd>/.claude/settings.json` with merged
      entries: works, but gits up every project tree and adds
      refcount cleanup logic identical to Kiro's.
    - **Top-level `~/.claude/settings.json` install via
      `agent-orch setup` / `teardown`, with the hook subcommand
      filtering on `$AGENT_ORCH_PANE`** — chosen. Bare claude
      gets a 5ms env-check + exit 0 per event; wrapped claude
      runs the existing logic. Kiro stays project-scoped (its
      user-global path is undocumented). Landed in `b416c50`.
- 2026-06-06 · **Re-opened Planning Review Gate** for an
  event-driven `loop-body` and a tmux keybind in `setup`.
  Today's `loop-body` is a poll loop that re-spawns fzf on
  every selection; user feedback was that state changes from
  hooks don't reach the picker until the next pick, and the
  typed query / cursor are lost on every round-trip. Researched
  the fzf `--listen` / `notify` combination and confirmed the
  documented primitives: fzf 0.66.0+ supports `--listen=<sock>`
  with HTTP-over-Unix-socket reload pushes; 0.71.0+ adds
  `--track --id-nth=N` for content-match cursor preservation
  across reload; `notify` 8.x with `notify-debouncer-mini` 0.7
  collapses atomic-rename's create+modify+rename burst into one
  logical event. Architecture: spawn fzf as child + watcher
  thread + `mpsc::channel`; on debounced change, write a
  hand-rolled HTTP/1.1 POST to the socket. No async runtime
  needed. Two new deps. Live-only tmux keybind installed by
  `setup` (run `tmux bind-key -n M-o ...` against the live
  server); persistence across reboots deferred.
- 2026-06-06 · **CLI prune** added to the same slice. User
  asked to keep agent-orch minimal: setup, teardown, wrap,
  loop, doctor — five user-facing verbs. The previous spec had
  grown to nine. Pruned `pick` and `list`, hid `hook`,
  `unregister`, `render`, `loop-body` from `--help`. Collapsed
  `loop-body` and the bare-invocation path into one `loop` verb
  that self-detects via `$TMUX`. Awaiting planning → dev cue
  before code changes.

## What v1 ships and what defers

**v1 user-facing CLI (target shape, all five verbs):**

```
agent-orch setup       # install Claude hooks + tmux keybind
agent-orch teardown    # uninstall
agent-orch wrap <kind> -- <agent-cmd> [args...]
agent-orch loop        # open the UX (or `agent-orch` bare)
agent-orch doctor      # verify
```

Plus three hidden internal verbs that exist only because external
systems shell out to them: `hook` (Claude/Kiro), `unregister`
(tmux `pane-exited`), `render` (fzf `reload`).

**Shipped state going into the current slice:**

- `wrap claude / kiro / <other>` — registers the launch and
  `execvp`s the agent. Kiro writes the project-scoped
  `<cwd>/.kiro/agents/agent-orch.json` with refcount cleanup.
- `hook <event>` — filters on `$AGENT_ORCH_PANE`, applies the
  event to the matching record under flock. Always exits 0.
- `unregister <pane-id>` — tmux `pane-exited` target; per-kind
  cleanup via `Wrapper::cleanup`.
- `setup` / `teardown` — Claude side only. Tagged install in
  `~/.claude/settings.json`; tag-scoped removal.
- `agent-orch` (no args) — ensure orchestrator session +
  switch-client.
- Currently *also* shipped but to be **removed** in the next
  slice as user-facing verbs: `pick`, `list`, `loop-body`. None
  of them are part of the target five-verb surface.

End-to-end exercised by `tests/agent-orch/integration.sh` (10
cases on a private tmux server) plus 39 in-process unit +
behavior tests in `src/main.rs`.

**Pending the current planning slice:**

- **Prune the CLI** to the five user-facing verbs. Drop
  `pick` and `list` entirely. Hide `hook`, `unregister`,
  `render`, and the internal-recursion target via
  `#[command(hide = true)]`. Collapse `loop-body` and the
  bare-invocation path into one `loop` verb that
  self-detects whether it's running inside the orchestrator
  session.
- **Event-driven `loop`.** Spawn fzf with `--listen`, watch
  `sessions.json` parent dir via `notify`, push `reload(...)`
  on change. Persistent picker survives selections; cursor
  position and typed query survive round-trips via
  `--track --id-nth=1`.
- **`agent-orch render`** (hidden) — prints the formatted
  picker rows for fzf's reload to consume.
- **Live-only tmux keybind in `setup` / `teardown`** —
  `tmux bind-key -n M-o switch-client -t orchestrator` /
  `tmux unbind-key -n M-o`. Persistence across reboots
  deferred.

**Deferred to follow-up tickets:**

- **Persistent tmux keybind** across `tmux kill-server` /
  reboot. Two paths under consideration: (a) edit the user's
  tmux conf with sentinel markers (won't work for nix-managed
  read-only confs); (b) write a sidecar `tmux.conf` under our
  state dir and have the user add one `source-file -q ...`
  line to their conf. Pick after the live-only version proves
  out in real use.
- `agent-orch doctor` — sanity-check (tmux version ≥ 3.2;
  fzf version ≥ 0.71.0 for `--id-nth`; agent CLIs;
  state-dir writeability; kiro orphan audit; **stale
  `~/.claude/settings.json` hooks pointing at a missing
  binary**; **was `setup` ever run on this host?**;
  **is the M-o keybind currently registered on the live tmux
  server?**).
- `sources/agent-orch/README.md` — install instructions,
  tmux keybind snippet, `setup`/`teardown` notes.
- v2 TUI port to `ratatui` (replaces fzf).
- v2 distinction of `waiting-on-permission` vs `complete`
  (Claude `Notification` event). One additional hook entry
  via `setup`.

## Decision Log

- **Rust, script-style — one `src/main.rs`.** Replaces the
  earlier all-Deno call. Reasons (in order of weight):
  1. `nix::unistd::execvp` exists in Rust. The wrapper can
     replace itself with the agent in-place — the bash
     prototype's original design. Deno (and Node) have no
     `execvp`; both forced a spawn-as-child workaround with
     parent-survives-agent costs (extra process, signal
     forwarding, `child.pid`/`wrapper_pid` bookkeeping). Rust
     deletes that complexity.
  2. **TUI growth path.** `ratatui` is the de-facto Rust
     terminal-UI standard — better than Ink (which targets
     Node and runs on Deno only via the compat layer). The
     dashboard v2 is a clean port of `cmd_pick`/`cmd_loop`,
     `Session` model unchanged.
  3. **Learning.** Building a real CLI tool is the fastest
     way to learn Rust beyond docs. Script-style keeps the
     learning curve gentle: types declared inline, helpers
     next to handlers, no early module-system fights.
  4. Hot-path latency drops from Deno's ~50ms × ~10-20
     fires/turn to ~1ms × ~10-20 — below the noise floor.
     Stops being a worry.
- **One file by default.** `src/main.rs` carries everything:
  clap dispatch, `Session` struct + impl, all subcommand
  handlers as flat top-level functions, helpers right after
  the handler that uses them, tests at the bottom. The bash
  prototype is 3 files for ~300 LOC; the Rust version targets
  ~600 LOC in 1 file. Splitting earlier is the
  over-engineering case "Rust as better bash" rejects.
  Concrete soft threshold: at ~1000 LOC, split out
  `sessions.rs` (the most-cohesive island). Until then, one
  file. The discipline forces honest assessment of "does this
  really need to be its own module?" — most answers are no.
- **Types declared inline with their use sites.** Small
  structs used only inside one function are declared inside
  that function. Module-level types (`Session`) sit at the
  top of the file with their `impl` block immediately after.
  Helpers used only by one subcommand handler sit right after
  that handler. Helpers used by multiple handlers go in a
  small "shared helpers" section between the model and the
  handlers.
- **Kiro lifted to first-class via project-scoped
  `.kiro/agents/agent-orch.json` injection with refcount
  cleanup.** L30 feedback. Cleanup is creation-flag-agnostic
  (any closing Kiro session checks the live-sibling count) —
  a flag-only design has an order-dependent leak when reusers
  outlive the creator. Flag is kept on the record for
  `doctor`-time auditing.
- **`$STATE_DIR` defaults to
  `${XDG_STATE_HOME:-$HOME/.local/state}/agent-orch/`**. L35
  feedback. The "no `$HOME` writes" invariant is about
  deployed code/config (no symlinks installed by deploy, no
  settings.json mutations) — runtime state has to live under
  `$HOME` by definition.
- **~~Per-launch hook settings via `claude --settings
  <path>`~~** *(superseded 2026-06-05 by "Claude top-level +
  filter; Kiro project-scoped" entry below).* The original
  call was: wrapper writes a per-pane settings file under
  `$STATE_DIR/tmp/<pane>/settings.json`, passes it as
  `--settings`, cleans up on `pane-exited`. Manual functional
  test surfaced that `--settings` displaces parts of Claude's
  precedence chain (login state, MCP servers, project
  settings) — the docs describe additive merge but the `hooks`
  key behavior is under-specified. We pivoted to top-level
  install + env-var filter; the per-pane tempdir machinery
  was removed.
- **Two states only: `running` / `complete`.** Plus `unknown`
  for hook-less agents. Per the user's brief: keep v1 simple.
  Distinguishing waiting via `Notification` is a small v2.
- **Hook every event into one subcommand.** Same binary,
  event name as subcommand argument. Captures `last_prompt`
  and `last_tool` for the picker summary without
  interpretation logic.
- **Single `sessions.json` (top-level array), flock-and-
  rewrite.** User asked for one file as the source of truth.
  Rewriting the whole array under POSIX advisory lock is
  simple and fast at our scale (≤100 sessions).
- **Dedicated `orchestrator` tmux session, not popup.** User
  selected this in the design interview. Persistent dashboard
  pane to extend later; matches "M-o anywhere → orchestrator"
  cleanly.
- **`ratatui` as the v2 TUI path** (replaces Ink). Better
  widget set, better incremental rendering, no compat-layer
  wobble, idiomatic in Rust. Feature-flagged from v1
  (`#[cfg(feature = "tui")]`) so the v2 cutover is a flag
  flip, not a code reshape.
- **No portability shim (`runtime.ts` dropped).** Rust *is*
  the portable target. The Deno→Node hygiene that justified
  the shim doesn't apply.
- **Stay out of `$HOME` for deployed code. v1 ships to
  `dist/`.** Lets us promote later to a flake / nix install
  or a registry entry without rewriting; keeps the install
  path one `cargo build --release` for now.
- **`wrap` and `loop` are independent.** Per PR #18 L486
  feedback. `wrap` installs the tmux hook, registers, and
  `execvp`s the agent without depending on the orchestrator
  loop being up. Hooks update `sessions.json` regardless.
  `loop` is a viewer that opens onto whatever's already
  accumulated. The user can throw `wrap` at agents now and
  open the dashboard later. Spec updated to make this
  explicit in both the wrapper flow pseudo-code and a new
  "Decoupling" subsection under Design.
- **Four typeclasses, not four section-headers.** Per PR #18
  review on commit `847a8cd` (L28, L120, L257). The original
  consolidation organized the file into four commented
  sections — `§1 Session`, `§2 Wrapper`, `§3 Hook`, `§4 Loop`
  — over loose free functions. The user's review reframed
  these as the right *types*, not just the right *layout*:
  `Session` (struct + impl for per-record ops), `Store` (the
  read/write/lock typeclass), `Wrapper` (a trait with `Claude`
  and `Kiro` impls), and `Loop` (a struct with the picker
  methods). The trait carries a default `hook` method body
  shared by both impls today (typeclass shape — Rust's
  default-method default-impl carries identical handling
  across kinds without copy-paste; a future kind whose
  payload differs can override). Net effect: the kind-specific
  code goes from 5 scattered `kind == "claude" / "kiro"`
  branches to two named `impl Wrapper for ... { }` blocks; the
  lock dance goes from 3 lines × 4 callers to one
  `store.mutate(...)` per caller; the trivial path-builder
  helpers (`sessions_path`, `lock_path`, `tmp_dir_for_pane`,
  `hook_install_marker`) collapse to three `const &str`s used
  inline. Same behavior, same CLI, same integration tests
  pass unchanged.
- **Trait carries its weight here.** The Rust-as-better-bash
  test for whether a trait is justified: there must be ≥2
  real branches today, and the variation pattern must be
  identical across them. Both are true: Claude and Kiro both
  do *synth-or-ensure config → contribute to argv → cleanup
  on exit*, just with different file targets. A trait of three
  methods (`prepare`, `cleanup`, default `hook`) captures it.
  If a third kind (Codex, Aider) ever lands, it slots in as
  one more `impl` block. Without the trait, kind-specific code
  scatters across `wrap`, `unregister`, `build_agent_argv`.
  With it, two named blocks per kind.
- **Claude top-level + filter; Kiro project-scoped.** Per
  PR #18 manual-test feedback. `--settings <path>` displaced
  Claude's settings precedence chain in practice (login state,
  project settings, MCP servers stored outside settings.json
  all dropped). Researched options:
  - `--hooks-file`: doesn't exist.
  - `CLAUDE_CONFIG_DIR`: undocumented.
  - Project-scoped `<cwd>/.claude/settings.json`: works
    additively, but adds the same refcount + tag + merge
    machinery the Kiro path has, in every project tree.
  - **Top-level `~/.claude/settings.json` via
    `agent-orch setup`** (chosen): hooks land at user level
    once; the binary's `hook` subcommand filters by
    `$AGENT_ORCH_PANE` and exits silently for bare-claude
    (non-wrapped) invocations. Bare claude pays ~5ms × ~10–20
    events/turn = ~50–100ms total per turn — visible if
    measured, not painful in practice. Wrap simplifies
    dramatically: Claude::prepare and Claude::cleanup become
    no-ops; the per-pane settings tempdir disappears entirely.
  Kiro stays project-scoped. Its user-global path
  (`~/.kiro/settings/cli.json`) is filesystem-discovered, not
  documented; banking the design on undocumented internals is
  a maintenance bomb. Kiro's `<cwd>/.kiro/agents/` is
  documented and stable.

  The asymmetry is in the impl, not the user-facing CLI —
  `wrap` / `hook` / `unregister` look the same to the user
  for both kinds. Claude users do `agent-orch setup` once at
  install; Kiro users don't need a setup verb because the
  project-scoped config is born/cleaned per cwd by
  wrap/unregister. Each kind gets the simplest mechanism that
  works reliably for *that* kind given *its* documented surface.

- **CLI pruned to five user-facing verbs.** The CLI had grown
  to nine subcommands (`wrap`, `hook`, `pick`, `list`,
  `unregister`, `loop-body`, `render`, `setup`, `teardown`)
  plus the bare-invocation path. Several were either dead
  (`pick`: no longer called once `loop` is event-driven),
  debug-only (`list`: `cat sessions.json | jq` covers the use
  case), or implementation-recursion targets that don't
  belong in `--help` (`hook`, `unregister`, `render`,
  `loop-body`). Pruned to:

      setup | teardown | wrap | loop | doctor

  Plus three hidden internal verbs (`hook`, `unregister`,
  `render`) that stay callable because Claude/Kiro/tmux/fzf
  shell out to them, but don't appear in `--help` via
  clap's `#[command(hide = true)]`. `loop-body` collapses
  into `loop` (self-detects via `$TMUX` whether it's running
  inside the orchestrator session). The bare invocation
  (`agent-orch` with no args) is an alias for `agent-orch
  loop`. One mental model, one verb per concept.

- **Event-driven `loop`** (poll-loop replacement).
  Today's loop spawns fzf, gets a selection, switches client,
  re-spawns fzf — every iteration. State updates from hooks
  during a pick don't reach the picker until the next pick;
  typed query and cursor are lost on every round-trip.
  Replaced with a persistent fzf + `notify`-driven reload:
  - `fzf --listen=<state>/fzf.sock` opens an HTTP-over-UDS
    control plane.
  - `--bind 'enter:execute-silent(tmux switch-client -t {1})+clear-query'`
    runs the switch *without* exiting fzf; the picker stays
    on screen across selections.
  - `--track --id-nth=1` (fzf 0.71.0+) keeps the cursor on
    the same row by content match across reloads.
  - `notify` + `notify-debouncer-mini` watches the parent dir
    of `sessions.json`; on debounced change, the watcher
    thread POSTs `reload(<self> render)` to the socket as
    hand-rolled HTTP/1.1.
  - `agent-orch render` is a new subcommand that prints the
    formatted picker rows so fzf's reload command can shell
    out to it.

  Two new deps (`notify`, `notify-debouncer-mini`). No async
  runtime needed; main thread waits on the fzf child, watcher
  thread runs via `mpsc::channel`. Drop the old poll body.

  Trade-off accepted: fzf < 0.71.0 silently degrades cursor
  preservation; fzf < 0.66.0 doesn't support
  `--listen=<sock>`. Doctor (deferred) gates versions; for
  now a clear error message at startup if the socket fails
  to bind.

- **Live-only tmux keybind in `setup` / `teardown`.** Run
  `tmux bind-key -n M-o switch-client -t orchestrator` at
  setup time and `tmux unbind-key -n M-o` at teardown. The
  binding persists for the life of the tmux server; survives
  `tmux source-file` and explicit detach/attach but **not**
  `tmux kill-server` or reboot. The user's tmux conf is not
  edited — the next slice will tackle persistence (sidecar
  conf + one user-added source-line). Live-only ships first
  because (a) implementation is ~10 lines, (b) it validates
  the UX assumption that M-o is the right back-to-orchestrator
  verb before we invest in conf-file edits, and (c) on this
  user's nix-managed setup the user's tmux conf may be
  read-only — we don't yet know which persistence pattern
  fits best for that case. Re-running `agent-orch setup`
  after a reboot restores the binding; doctor (deferred)
  will flag "keybind not currently registered" so the user
  knows when to do that.
