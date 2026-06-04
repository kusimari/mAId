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
- For Claude Code, the wrapper synthesizes a per-launch settings
  JSON at `$STATE_DIR/tmp/<pane>/settings.json` and passes it via
  `claude --settings <path>`, wiring lifecycle hooks
  (`UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `Stop`) to
  the orchestrator's own `hook` subcommand. **No writes to
  `~/.claude/settings.json`, ever.** The user's existing
  settings are merged in as the base.
- For Kiro CLI, the wrapper writes a project-scoped agent
  config at `<cwd>/.kiro/agents/agent-orch.json` if it does not
  already exist, recording in the session record that this
  wrapper created the file. On `pane-exited`, if no other live
  Kiro session in the same cwd remains, the cleanup removes
  it. First-creates, others-reuse, last-out-deletes —
  concurrent Kiro sessions in the same repo all share one
  config, and the working tree is left clean when they all exit.
- After writing per-launch hook config, the wrapper appends a
  record to `sessions.json` (with its own pid — which becomes
  the agent's pid post-exec) and **`execvp`s the agent**: the
  wrapper process is replaced in place by the agent. No parent
  process, no signal forwarding, no `child.pid`/`wrapper_pid`
  bookkeeping.
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

### Single binary, multiple subcommands

The orchestrator ships as one Rust binary: `agent-orch`.
Subcommands (parsed via `clap` derive):

- `agent-orch wrap <kind> [--cwd <dir>] -- <agent-cmd> [args...]`
  — the wrapper. Registers the launch, synthesizes per-launch
  hook config for the supported kinds, then `execvp`s the
  agent.
- `agent-orch hook <event-name>` — invoked by the agent's hooks.
  Reads the JSON payload from stdin, applies the event to the
  matching session record under `flock`. Always exits 0.
- `agent-orch pick` — print one selected entry's pane id to
  stdout, exit. Used inside the loop.
- `agent-orch loop` — the picker loop body that runs inside the
  orchestrator session. (Internally named `cmd_loop` because
  `loop` is a Rust keyword.)
- `agent-orch list` — print the live registry as a table.
  Useful for scripting and `doctor`.
- `agent-orch unregister <pane-id>` — remove an entry. Called
  by the tmux `pane-exited` hook. Also handles tempdir cleanup
  and Kiro orphan-config cleanup.
- `agent-orch doctor` — sanity-check tmux version, fzf, agent
  CLIs on PATH, state dir writeability, dist binary at the
  expected path. Surfaces missing deps with one actionable line
  each.
- `agent-orch` (no args) — ensure the `orchestrator` session
  exists, switch the client to it, run the picker loop in the
  orchestrator session.

### Wrapper (`agent-orch wrap`)

- Refuses to run outside tmux (no `$TMUX_PANE`) — the registry
  is pane-keyed; running outside tmux has no useful identity.
- Refuses to wrap twice in the same pane (a record for that
  `pane_id` already exists). The user can `agent-orch
  unregister %N` first if intentional.
- Supported kinds for v1:
  - `claude` — full hook injection via `claude --settings
    <path>`.
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

Synthesize a per-launch settings file at
`$STATE_DIR/tmp/<pane>/settings.json`:

1. Read the user's `~/.claude/settings.json` as the base
   (or `{}` if absent).
2. Append (do not replace) hook entries on `UserPromptSubmit`,
   `PreToolUse`, `PostToolUse`, and `Stop`, each calling
   `<dist>/agent-orch hook <event-name>`.
3. Write the merged JSON.
4. Set `AGENT_ORCH_PANE=%N` in the environment.
5. `execvp("claude", ["claude", "--settings", <path>,
   <agent-args>...])`.

Cleanup of `$STATE_DIR/tmp/<pane>/` happens in the unregister
handler when `pane-exited` fires — the wrapper process is gone
by then (replaced by the agent), so RAII-style cleanup isn't
available. Co-locating cleanup with the pane-lifecycle hook is
the right shape anyway.

#### Kiro path (per L30 feedback)

Project-scoped `.kiro/agents/agent-orch.json` injection with
refcount cleanup:

1. If `<cwd>/.kiro/agents/agent-orch.json` does not exist:
   create the directory if needed, write the file with
   `agent-orch hook <event>` wired to Kiro's documented hook
   events (`UserPromptSubmit`, `PreToolUse`, `PostToolUse`,
   `Stop`). Stamp `created_kiro_config=true` on the session
   record.
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
  agent's turn.

### Registry cleanup

- Tmux hook installed by the wrapper:
  `pane-exited → agent-orch unregister #{hook_pane}`.
- `unregister`:
  1. Under `flock`, read `sessions.json`.
  2. If this is a Kiro record, count live `kind=kiro` records
     in the same cwd (excluding this one). If zero, remove
     `<cwd>/.kiro/agents/agent-orch.json` (best-effort
     `rmdir` of parent dirs).
  3. If this record had a Claude tempdir (any record, since
     Kiro doesn't use one), `rm -rf
     $STATE_DIR/tmp/<pane_id>/`.
  4. Remove the pane's record. Write back atomically.
- Sweep at query time: when the picker reads `sessions.json`,
  it filters out records whose `pid` is no longer alive
  (`kill(pid, 0)`). Catches the case where the tmux server
  restarted while the orchestrator was down. The sweep also
  runs cleanup on the sessions it removes.

### Picker (`agent-orch pick` + `loop`)

Picker UX:

- One row per live session.
- Sorted: `running` first, then `complete`, then `unknown`.
  Within each group, most-recently-active first
  (max of `state_ts`, `last_event_ts`, `started`).
- Row format:
  `<state-glyph> <kind> <cwd-tail> · <last_prompt> [· <last_tool>]`
  with `last_tool` shown only when non-empty.
- `--preview` shows the full record (state, full prompt, last
  event, started-ago, state age).

Picker loop (runs inside the orchestrator session):

```
loop:
  while true:
    sel = `agent-orch pick`
    if sel: tmux switch-client -t sel
    else: sleep 0.2
```

`pick` invokes `fzf` via `std::process::Command`, feeds it the
formatted rows on stdin, captures the selection on stdout, prints
the pane id. fzf is a hard dependency for v1; doctor checks for
it.

Orchestrator session ensure-on-run (bare `agent-orch`):

```
if !tmux has-session -t orchestrator:
  tmux new-session -d -s orchestrator '<dist>/agent-orch loop'
tmux switch-client -t orchestrator
```

### "Back to orchestrator" UX

- Documented in the README: user adds
  `bind-key -n M-o switch-client -t orchestrator` to their
  `~/.tmux.conf`. The orchestrator never edits user dotfiles.
- The wrapper's per-pane `@agent-orch-pane` user option lets
  future features introspect pane ownership without re-reading
  the registry.

### Hard constraints

- **No deployed `$HOME` writes.** The binary is invoked from
  `<repo>/dist/agent-orch/`. Per-user runtime state at
  `${XDG_STATE_HOME:-$HOME/.local/state}/agent-orch/` is
  acceptable — runtime data has to live under `$HOME`; the
  invariant is about deployed code/config.
- **No `~/.claude/settings.json` mutations**, ever. All Claude
  hook wiring goes through `claude --settings <path>`.
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
block. Coverage:

- `Session::apply_event` — one test per event covering state
  transitions, `last_prompt` truncation, `last_tool` refresh,
  `state_ts` advance, no-op on missing record.
- `read_sessions` / `write_sessions_atomic` — round-trip,
  malformed-line skip, empty file.
- `format_row` — glyph mapping, `cwd-tail` shortening,
  empty-tool case.
- `sort_sessions` — group precedence and within-group order.
- `live_filter` — drops dead pids (callback-driven).
- `kiro_orphan_paths` — returns the cwd's `.kiro/agents/
  agent-orch.json` iff no live Kiro record shares the cwd.

### Smoke (`deno task test:smoke`) — load-bearing

`tests/functional/agent-orch/smoke` (shell-driven, drives the
compiled binary):

1. Spawn a fresh tmux server on a private socket
   (`tmux -L agent-orch-test`).
2. Run `<dist>/agent-orch wrap claude -- <stub-agent>` inside a
   tmux pane, where `<stub-agent>` is a tiny shell script that
   loops on stdin.
3. Assert: `sessions.json` has exactly one record with the
   expected `pane_id`, `kind=claude`, and a live `pid` (and
   that the recorded `pid` matches the *agent's* pid since
   `execvp` preserves it).
4. Call `<dist>/agent-orch hook UserPromptSubmit` directly with
   synthetic JSON on stdin and `AGENT_ORCH_PANE` set; assert
   `state=running` and `last_prompt` populated. Repeat for
   `PreToolUse`, `PostToolUse`, `Stop`; assert transitions.
5. **Kiro cleanup test (both orderings).** Two passes — close
   creator-first vs reuser-first — both must end with the
   `.kiro/agents/agent-orch.json` removed and zero records
   remaining.
6. Tear down the tmux server.

The smoke test does not hit a real Claude / Kiro binary. Fast
(~3s), depends only on `tmux` + `bash` + the compiled binary.

### Functional (`deno task test:functional`) — user-driven

Out of scope for v1. Manual test = launch real Claude Code +
Kiro CLI under `agent-orch wrap`, run the picker, jump in/out.

### Quality gate

`deno task fmt && deno task lint && deno task check` after every
implementation slice. `fmt` / `lint` / `check` delegate to
`cargo fmt --check`, `cargo clippy -- -D warnings`, and
`cargo check` respectively when the slice touches Rust code; the
existing Deno equivalents still fire on top-level `deno.json`
edits.

## Design

### Layout (script-style)

```
sources/agent-orch/
├── Cargo.toml
├── rust-toolchain.toml          pin: stable, minimal profile
├── src/
│   └── main.rs                  ~600 LOC — everything
├── tests/
│   └── functional/
│       ├── stub-agent           shell, ~10 lines
│       └── smoke                shell, drives the built binary
└── README.md                    install + tmux keybind notes

dist/                            (gitignored; populated by build task)
└── agent-orch/
    └── agent-orch               cargo build --release output
```

**One file by default.** `src/main.rs` carries the whole tool —
clap dispatch, the `Session` model + impls, all subcommand
handlers as flat top-level functions, helpers right after the
handler that uses them, tests at the bottom in
`#[cfg(test)] mod tests`. Split a file out only if/when one file
genuinely hurts (a soft threshold of ~1000 LOC is fine to ignore
until then). Splitting earlier is the over-engineering case the
"Rust as better bash" framing rejects.

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

### Wrapper flow (Claude case)

```
agent-orch wrap claude -- --resume my-session

  guard: $TMUX_PANE set; no existing record for that pane

  install global tmux hook (idempotent, marker-file gated)
    tmux set-hook -g pane-exited 'run-shell "<dist>/agent-orch
      unregister #{hook_pane}"'

  synthesize per-launch settings
    out_dir  = $STATE_DIR/tmp/%42
    out_path = $STATE_DIR/tmp/%42/settings.json
    base     = read_user_settings("~/.claude/settings.json").unwrap_or(json!({}))
    settings = merge_hooks(base, [
      ("UserPromptSubmit", hook_cmd("UserPromptSubmit")),
      ("PreToolUse",       hook_cmd("PreToolUse")),
      ("PostToolUse",      hook_cmd("PostToolUse")),
      ("Stop",             hook_cmd("Stop")),
    ])
    write_atomic(out_path, settings)

  register
    flock $STATE_DIR/sessions.lock
    sessions.json := append({pane_id:"%42", pid: getpid(),
      kind:"claude", cwd: current_dir(), started: now(),
      state:"unknown", state_ts: now(), ...,
      created_kiro_config: false})
    tmux set-option -p @agent-orch-pane "%42"

  execvp the agent — wrapper process is replaced in place
    setenv("AGENT_ORCH_PANE", "%42")
    execvp("claude", ["claude", "--settings", out_path,
                      "--resume", "my-session"])
    // unreachable — execvp returns only on failure
```

The wrapper's pid is preserved across `execvp` (POSIX guarantee:
the process image is replaced; the pid stays the same). So the
`pid` recorded in the registry is the live agent's pid the
moment the next instruction runs.

### Wrapper flow (Kiro case)

```
agent-orch wrap kiro -- chat

  guard, install tmux hook (same as Claude)

  Kiro project-scoped config injection
    cfg = "<cwd>/.kiro/agents/agent-orch.json"
    if !exists(cfg):
      mkdir -p "<cwd>/.kiro/agents"
      write_atomic(cfg, kiro_hook_config(<dist>/agent-orch))
      created_kiro_config = true
    else:
      created_kiro_config = false

  register (with created_kiro_config flag)
    flock ...; sessions.json := append({...})

  execvp the agent
    setenv("AGENT_ORCH_PANE", "%43")
    execvp("kiro", ["kiro", "chat"])
```

### Unregister flow

```
agent-orch unregister %43

  flock $STATE_DIR/sessions.lock
    record = find(pane_id="%43")
    if !record: exit 0  (already gone)

    if record.kind == "kiro":
      siblings = sessions.filter(s =>
        s.kind=="kiro" && s.cwd==record.cwd && s.pane_id!=record.pane_id)
      if siblings.is_empty():
        rm "<record.cwd>/.kiro/agents/agent-orch.json"
        rmdir parent dirs (best-effort)

    rm -rf "$STATE_DIR/tmp/${record.pane_id}/"

    sessions.json := remove(record); write atomically
```

### Why this shape

- **Rust, script-style — one `main.rs`.** Reads top-to-bottom
  like a shell script, with the compiler catching what bash
  can't. Types declared inline with their use sites. Helpers
  next to the handler that calls them. Tests at the bottom.
  The bash version of this is 3 small files; the Rust version
  matches that scale, not idiomatic-Rust's habit of one-file-
  per-concept. Pre-modularization is the over-engineering case.
- **`execvp`, not spawn-as-child.** `nix::unistd::execvp`
  replaces the wrapper's process image with the agent's. The
  wrapper's pid becomes the agent's pid because POSIX preserves
  the pid across `execvp`. No parent process consuming RAM, no
  signal-forwarding ladder, no `child.pid`/`wrapper_pid`
  bookkeeping. This is the design the bash prototype always
  used; reaching for Rust restores it.
- **Per-launch hook config beats user-config mutation.** Both
  Claude (`--settings <path>`) and Kiro (project-scoped
  `.kiro/agents/`) take per-launch overrides. The Kiro
  override is project-scoped, not per-launch, so concurrent
  Kiro sessions in the same repo coordinate via the refcount
  cleanup rule.
- **Single sessions.json beats split files.** Source of truth
  in one place; readers and writers serialize through one
  lock; rewrite-on-update is fine at our scale.
- **Two states (`running`/`complete`).** Per the user's brief.
  Distinguishing waiting-on-permission from idle is a small v2
  with Claude's `Notification` event.
- **Hook every event into one subcommand.** Same binary, event
  name as subcommand argument. Captures `last_prompt` from
  `UserPromptSubmit.prompt` and `last_tool` from `PreToolUse` /
  `PostToolUse` — enough for the picker summary.
- **Dedicated orchestrator session, not popup.** Persistent
  dashboard pane to extend later (live refresh, summary
  preview); clean "M-o anywhere → orchestrator" verb.
- **Cleanup co-located with `pane-exited`, not RAII.** With
  `execvp`, the wrapper process is gone before the agent
  exits; Rust's `Drop` for tempdirs would fire too early. The
  unregister handler is the right place for tempdir + Kiro
  config cleanup, and it's already running on pane death.

### Trade-offs we're accepting

- `complete` doesn't distinguish "idle" from "waiting on user".
  User reads the pane after jumping. v2 with `Notification`.
- Wrapper requires the user to launch through it. Bare `claude`
  / `kiro` invocations are invisible. README documents the
  launch verbs; shell aliases are an obvious follow-up.
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

Ordered. Each row is one coherent dev-loop iteration: small
slice → Quality → Test → Code Review → Push → Review per
kdevkit §7.

1. **Skeleton + flake addition + sessions model + cargo build.**
   - Add `rust-overlay` to `flake.nix`; expose `cargo`,
     `rustc`, `clippy`, `rustfmt`, `rust-analyzer` from a
     pinned stable channel (in the same `mkShell` as Deno).
   - Add `sources/agent-orch/Cargo.toml` (dep set above) and
     `rust-toolchain.toml` (pin: stable, minimal profile).
   - Author `src/main.rs` with: clap dispatch shell (all
     subcommands as `unimplemented!()` placeholders), the
     `Session` struct + serde derives + `impl Session` block
     (`apply_event`, `format_row`), and helpers
     `read_sessions`, `write_sessions_atomic`, `with_lock`,
     `sort_sessions`, `live_filter`, `kiro_orphan_paths`. All
     in one file.
   - `#[cfg(test)] mod tests` at the bottom covering the
     pure-logic helpers and `apply_event`.
   - Add `agent-orch:build` (`cd sources/agent-orch && cargo
     build --release && mkdir -p ../../dist/agent-orch && cp
     target/release/agent-orch ../../dist/agent-orch/`),
     `agent-orch:test` (`cd sources/agent-orch && cargo
     test`), `agent-orch:check` (`cargo check + clippy +
     fmt --check`) to top-level `deno.json`.
   - Wire `deno task test:unit`, `deno task fmt`, `deno task
     lint`, `deno task check` to also run the cargo
     equivalents when the agent-orch tree changes (top-level
     fmt/lint/check still gate Deno paths separately).
   - Risk: fresh cargo dep download. One-time.

2. **Wrapper subcommand (Claude path).**
   - Implement `cmd_wrap` in `src/main.rs` for `kind=claude`:
     `$TMUX_PANE` guard, idempotent global tmux hook install
     (under `flock` on a marker file), settings synthesis at
     `$STATE_DIR/tmp/<pane>/settings.json` (merge user base +
     our hooks via `serde_json::Value`), session record
     append, `tmux set-option -p`, `nix::unistd::execvp`.
   - Helpers `synth_claude_settings` and (small)
     `read_user_settings` declared right after `cmd_wrap`.
   - Risk: edge cases around `$TMUX_PANE` (nested tmux),
     missing `~/.claude/settings.json`, claude CLI not on
     PATH.

3. **Hook subcommand.**
   - Implement `cmd_hook`: read stdin into `serde_json::Value`,
     read `$AGENT_ORCH_PANE`, take `flock`, find record, call
     `Session::apply_event`, write atomically. Always exits 0.
   - Risk: payload field names. Mitigated by the
     research-verified field names; smoke test stubs cover
     all four event payloads.

4. **Wrapper Kiro path + unregister cleanup.**
   - `cmd_wrap` Kiro branch: project-scoped
     `.kiro/agents/agent-orch.json` write-if-absent + record
     stamp, then `execvp("kiro", ...)`.
   - `cmd_unregister`: read sessions, find record, do
     creation-flag-agnostic Kiro cleanup when no live Kiro
     records in that cwd remain, `rm -rf
     $STATE_DIR/tmp/<pane>/`, remove record, write
     atomically.
   - Smoke test step 5 (concurrent Kiro both orderings) is
     load-bearing.
   - Risk: orphan files if the wrapper crashes mid-flight.
     Doctor surfaces; acceptable.

5. **Picker + orchestrator loop.**
   - `cmd_pick`: render rows from sessions.json, run fzf via
     `std::process::Command` (write rows to stdin, read
     selection from stdout), print pane id.
   - `cmd_loop`: the orchestrator-session loop body.
   - `main` bare invocation — ensure orchestrator session
     exists, switch-client.
   - Add the `tui` feature flag (gated `cmd_pick_tui` /
     `cmd_loop_tui` placeholders behind
     `#[cfg(feature = "tui")]`) — empty stubs for v1 so the
     compile path is exercised without pulling `ratatui`.
   - Risk: fzf + tmux interaction edge cases; smoke catches
     common ones.

6. **Smoke harness.**
   - `tests/functional/agent-orch/stub-agent` — `cat`-loop
     shell script.
   - `tests/functional/agent-orch/smoke` — six-step harness
     described in Test Strategy, including both Kiro
     concurrency orderings, with an extra assert that
     `recorded_pid == agent_pid` (validates `execvp` pid
     preservation).
   - Wire into `deno task test:smoke`.
   - Risk: tmux availability on the test host; skip clearly.

7. **Doctor + README.**
   - `cmd_doctor` — tmux ≥ 3.2 (display-popup) and ≥ 1.6
     (`set-hook -g pane-exited`), `fzf`, `claude` / `kiro`
     CLIs detected, state dir writeable, dist binary at the
     expected path, orphan `.kiro/agents/agent-orch.json`
     audit.
   - `sources/agent-orch/README.md` — install (`deno task
     agent-orch:build`, run from `dist/`), `agent-orch wrap`
     examples, tmux keybind snippet, architecture diagram,
     v2 ratatui note.
   - Risk: minimal.

8. **Closure.** Per kdevkit §8. The original
   `specs/backlog/agent-session-orchestrator.md` is currently
   untracked in `main`; verify at close-time and `git rm` if
   it's been added.

### Risk notes

- **Hook payload shape drift.** Event names and payload field
  names are pinned by Claude / Kiro docs as of 2026-06; the
  hook config synth is one function and easy to update if
  either renames.
- **`execvp` failure semantics.** `execvp` returns only on
  failure (e.g., binary not found, ENOEXEC); the wrapper must
  treat the call as terminal and surface a clear error. The
  registered session record is left in place — the
  pane-exited hook handles cleanup either way.
- **Single-file growth.** `main.rs` is targeted at ~600 LOC.
  If it crosses ~1000, split out `sessions.rs` (the
  most-cohesive island: model + apply + format + sort + tests
  for those). Not before.
- **Concurrent hook fires.** Two agents firing events
  simultaneously contend on the `sessions.lock`. POSIX
  advisory locks via `fs2` serialize them; worst case is a
  few ms delay. Acceptable.
- **fzf inside tmux.** Confirmed safe; doctor warns on
  insufficient tmux version.
- **Cargo build time.** ~30-60s cold, ~1-2s incremental,
  sub-second `cargo check`. Acceptable for the dev loop.

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
- **Per-launch hook settings via `claude --settings <path>`,
  not `~/.claude/settings.json` mutation.** Verified: Claude
  Code's `--settings` flag accepts a JSON file and merges
  into the precedence chain. Wrapper writes to
  `$STATE_DIR/tmp/<pane>/settings.json`, `execvp`s the
  agent, the unregister hook cleans up on `pane-exited`.
  (No `tempfile::TempDir` because `Drop` would fire before
  `execvp` if held in scope, and after `execvp` the wrapper
  is gone — co-locating cleanup with the pane-lifecycle hook
  is the right shape.)
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
