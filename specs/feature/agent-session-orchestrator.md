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

    /// Synthesize/ensure the per-kind hook config and return
    /// (program, argv) we hand to execvp plus any flag the
    /// session record needs to carry (today: only Kiro's
    /// "I created the file" bit).
    fn prepare(&self, ctx: &WrapCtx) -> Result<Prepared>;

    /// Per-kind cleanup on unregister. Claude removes the
    /// per-pane tmpdir; Kiro runs the refcount-agnostic
    /// `.kiro/agents/agent-orch.json` cleanup.
    fn cleanup(&self, store: &Store, removing: &Session,
               others: &[Session]) -> Result<()>;

    /// Hook handling — DEFAULT method body, identical for all
    /// kinds today. A future kind whose hook payload differs
    /// (different stdin field for the prompt, etc.) can
    /// override; both Claude and Kiro inherit this default.
    fn hook(&self, store: &Store, pane_id: &str, event: &str,
            stdin: &mut dyn Read, now: u64) -> Result<()> {
        // read payload, find matching record, apply event,
        // write back through store.mutate. One implementation,
        // lives on the trait.
    }
}

struct Claude;
struct Kiro;
struct Other(String);   // register-only — no per-kind config

struct WrapCtx<'a> {
    store: &'a Store,
    self_path: &'a Path,
    user_claude_settings: &'a Path,
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

`Claude::prepare` synthesizes the per-pane settings file under
`store.tmp_dir(pane_id)`, splices `--settings <path>` after
`argv[0]`, returns `created_kiro_config: false`.

`Kiro::prepare` writes `<cwd>/.kiro/agents/agent-orch.json` if
absent, returns `created_kiro_config: <bool>`, leaves argv
unchanged.

`Other::prepare` returns argv unchanged, `created_kiro_config:
false` — the wrapper still registers the pane (so the picker
sees it), but no per-kind hook config means the session stays
in `unknown` state for its lifetime.

`Claude::cleanup` removes `store.tmp_dir(pane_id)`.

`Kiro::cleanup` checks `others` for any live `kind=kiro &&
cwd==removing.cwd`; if none, removes the project-scoped
`.kiro/agents/agent-orch.json`. **Refcount-agnostic** —
ignores the `created_kiro_config` flag so close-creator-first
ordering still removes the file when the last reuser exits.

`Other::cleanup` is a no-op.

The `hook` default method body lives on the trait (typeclass
shape). Both Claude and Kiro inherit it; the per-kind variation
is in *injection* (which file the hook command lands in), which
is already on `prepare`. The hook subcommand body becomes a
one-line dispatch through `wrapper_for(kind).hook(...)`.

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
    /// client. The loop body itself runs in the spawned
    /// orchestrator session via `agent-orch loop-body`.
    fn run(&self, self_path: &Path) -> Result<()>;

    /// Picker loop body — `pick` + `tmux switch-client` → repeat.
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
                ctx   := WrapCtx { store, self_path, user_claude_settings,
                                   pane_id, cwd, agent_argv }

  prepared = w.prepare(&ctx)?
    // claude: synth tmpdir/settings.json, splice --settings into argv
    // kiro:   ensure <cwd>/.kiro/agents/agent-orch.json, set created flag
    // other:  argv unchanged, no flag

  store.mutate(|sessions| {
    ensure no record for ctx.pane_id;
    sessions.push(Session::new(ctx, w.kind(), prepared.created_kiro_config));
    Ok(())
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
  *handling* is identical across kinds (read stdin → flock →
  apply event → write back). The hook *injection* differs
  (Claude tempfile vs Kiro project file) and is already on the
  trait via `prepare`. So `hook` lives as a default method
  body — neither Claude nor Kiro override it today, but a
  future kind can if its payload shape differs.
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
  cleanup rule on `Wrapper::cleanup`.
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
- **Cleanup co-located with `pane-exited`, not RAII.** With
  `execvp`, the wrapper process is gone before the agent
  exits; Rust's `Drop` for tempdirs would fire too early. The
  unregister handler is the right place for tempdir + Kiro
  config cleanup, and it's already running on pane death.
  `Wrapper::cleanup` is where each kind's per-pane cleanup
  lives.

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

The functional surface (wrap, hook, list, pick, unregister,
loop-body) shipped under the original four-section design
across commits `0e0281c` … `847a8cd`. **The remaining work
is one slice: a pure refactor to the typeclass shape above,
preserving every behavior.**

### Slice — typeclass redesign

One commit, one Code Review Gate, one push.

1. **Path constants.** Replace `sessions_path`, `lock_path`,
   `tmp_dir_for_pane`, `hook_install_marker` with three `const
   &str`s used inline at call sites. Drop the helper functions.
2. **`impl Session`.** Move `format_row`, `state_group`,
   `activity` onto the existing `Session` impl. Drop the free
   functions. `apply_event` already lives there.
3. **`Store` type.** Promote `read_sessions` /
   `write_sessions` / `with_lock` to private members of a new
   `struct Store { dir: PathBuf }`. Public surface: `from_env`,
   `new`, `read`, `mutate(|v| ...)`, `dir`, `tmp_dir(pane_id)`,
   `hook_marker`. Every caller using the lock dance collapses
   to one `store.mutate(...)` call.
4. **`Wrapper` trait + `Claude` / `Kiro` / `Other` impls.**
   - `kind() -> &str`
   - `prepare(ctx) -> Result<Prepared>` — moves
     `synth_claude_settings` + `merge_claude_hooks` (Claude),
     `ensure_kiro_config` + `build_kiro_config` (Kiro), argv
     splicing into the impls. `build_agent_argv` is inlined
     into `Claude::prepare` since it's the only caller now.
   - `cleanup(store, removing, others)` — Claude removes
     `store.tmp_dir(pane_id)`; Kiro runs the refcount-agnostic
     `.kiro/agents/agent-orch.json` cleanup; Other is a no-op.
   - `hook(store, pane_id, event, stdin, now)` — default
     method body, the existing `hook` function moved onto the
     trait. Neither impl overrides today.
   - `wrapper_for(kind) -> Box<dyn Wrapper>` does the dispatch.
5. **`Loop` type.** `struct Loop<'a> { store: &'a Store }` with
   `render`, `pick`, `run`, `body` methods. The free functions
   `render_rows`, `pick`, `run_loop`, `loop_body` move into
   `impl Loop`.
6. **Rewrite `wrap` and `unregister`.** The CLI dispatch in
   `main` now reads:
   ```
   match cmd {
       Wrap{..}     => wrap(&*wrapper_for(&kind), &ctx, side_effects),
       Hook{event}  => wrapper_for(&kind_of_pane(&store, &pane)).hook(...),
       Pick         => Loop::new(&store).pick(&mut io::stdout()),
       List         => /* unchanged */,
       Unregister{} => unregister(&store, &pane_id),
       LoopBody     => Loop::new(&store).body(),
       None         => Loop::new(&store).run(&self_path),
   }
   ```
   `wrap` body collapses from ~77 lines to ~25; `unregister`
   from ~34 to ~15. The kind-specific branches in these
   functions disappear into the trait impls.
7. **Tests.** Reorganize the existing 22 tests to call the new
   entrypoints (`Store::mutate`, `Loop::pick`,
   `Wrapper::prepare`/`cleanup`/`hook`). Each existing
   behavior assertion lands on the new surface unchanged. No
   new test cases — this is a refactor.
8. **Code Review Gate** (kdevkit §7).
9. **Push.**
10. **Closure** (kdevkit §8) — Decision Log entry, soft verify
    `project.md`. Spec already mentions the integration script
    and `dist/`; nothing to add.

### Out of scope for this slice

- New functionality: doctor, README, ratatui TUI, waiting-state
  remain follow-ups. They were deferred in the original ship
  and stay deferred.
- Behavioral changes: this is a refactor. CLI surface, on-disk
  shape, test count and intent all stay the same.
- Adding more kinds. `Other` is the catch-all; the trait shape
  makes Codex / Aider a one-impl-block addition when needed,
  but not in this slice.

### Risk notes

- **Trait dispatch overhead.** `Box<dyn Wrapper>` adds one
  vtable indirection per call. Hot path is `hook` (one call
  per agent event); the indirection is sub-microsecond and far
  below the flock + JSON round-trip already in the path.
- **Hook subcommand needs the pane's kind.** The CLI's `hook
  <event>` subcommand only carries `$AGENT_ORCH_PANE`; to
  dispatch through the right `Wrapper` impl, it does one extra
  unlocked `Store::read()` to find the matching record's kind.
  That read is eventually-consistent which is fine — and if
  the pane isn't registered (stale fire after unregister), the
  hook no-ops as it does today.
- **Renaming churn in tests.** 22 tests reorganize. Each
  test's assertion intent is preserved; only the call shape
  changes (`hook(state, pane, ...)` →
  `wrapper_for(kind).hook(&store, pane, ...)`). Risk: easy to
  introduce a typo in the rename and have a test still pass
  for the wrong reason. Mitigation: run the integration
  script after each round; its 9 cases drive the binary
  end-to-end and won't accept a regression.
- **`Loop` lifetime.** `Loop<'a> { store: &'a Store }` borrows
  the store for the picker lifetime. `body()` runs forever in
  the orchestrator session; that's fine because `main` builds
  the store, hands a borrow to `Loop::new`, and runs the loop
  body — the store stays alive until the orchestrator session
  exits.

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
  describe the typeclass shape; the work is one refactor slice
  preserving every shipped behavior. Awaiting planning → dev
  cue before any code changes.

## What v1 ships and what defers

**Shipped:** `wrap claude`, `wrap kiro`, `hook <event>`, `list`,
`unregister`, bare invocation (ensure orchestrator session +
switch-client), `loop-body`. End-to-end exercised by
`tests/agent-orch/integration.sh` (9 cases) over a private tmux
server. 22 in-process unit / behavior tests in `src/main.rs`.

**Deferred to follow-up tickets:**
- `agent-orch doctor` — sanity-check (tmux version, fzf,
  agent CLIs, state-dir writeability, kiro orphan audit).
  Mentioned in spec; not implemented in this PR.
- `sources/agent-orch/README.md` — install instructions, tmux
  keybind snippet, examples. Not authored in this PR.
- v2 TUI port to `ratatui` (replaces fzf). Out of scope for v1.
- v2 distinction of `waiting-on-permission` vs `complete`
  (Claude `Notification` event). One additional hook entry.

The `tests/functional/agent-orch/smoke` reference earlier in
this spec is superseded by `tests/agent-orch/integration.sh`,
which serves the same role and is wired through
`deno task agent-orch:integration`.

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
