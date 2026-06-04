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
  JSON in a tempdir and passes it via `claude --settings <path>`,
  wiring lifecycle hooks (`UserPromptSubmit`, `PreToolUse`,
  `PostToolUse`, `Stop`) to the orchestrator's own `hook`
  subcommand. **No writes to `~/.claude/settings.json`, ever.**
  The user's existing settings are merged in as the base.
- For Kiro CLI, the wrapper writes a project-scoped agent
  config at `<cwd>/.kiro/agents/agent-orch.json` if it does not
  already exist, recording in the session record that this
  wrapper created the file. On `pane-exited`, if this wrapper
  created the file AND no other live Kiro session in the same
  cwd is still using it, the cleanup removes it. First-creates,
  others-reuse, last-out-deletes — concurrent Kiro sessions in
  the same repo all share one config, and the working tree is
  left clean when they all exit.
- A single `sessions.json` file is the source of truth for both
  registration and state. Every mutation is gated by `flock`.
  The wrapper appends a record on launch; the hook subcommand
  updates the matching record on every fired event; the tmux
  `pane-exited` hook removes it.
- The orchestrator (`agent-orch` bare invocation) ensures a
  dedicated `orchestrator` tmux session exists, switches the
  client to it, and runs the picker loop. Picking jumps the
  client to the agent's pane via `tmux switch-client -t %ID`.
- Liveness is derived from the tmux `pane-exited` hook (registry
  cleanup) plus a `kill -0 $pid` belt-and-suspenders sweep at
  query time.

The whole tool is one self-contained Deno-compiled binary —
`dist/agent-orch/agent-orch` — with subcommands. State lives at
`${XDG_STATE_HOME:-$HOME/.local/state}/agent-orch/` (per-user
runtime data; the "no `$HOME` writes" constraint applies to
deployed code and config, not to runtime state which has to live
under `$HOME` by definition). v1 ships standalone (run from the
repo's `dist/` or copy elsewhere); promoting later to a
flake-installed package or a registry-deployed symlink is purely
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

The orchestrator ships as one Deno-compiled binary: `agent-orch`.
Subcommands:

- `agent-orch wrap <kind> [--cwd <dir>] -- <agent-cmd> [args...]`
  — the wrapper. Registers the launch, synthesizes per-launch
  hook config for the supported kinds, then `Deno.exec`s the
  agent (or for unsupported kinds, registers and exec's only).
- `agent-orch hook <event-name>` — invoked by the agent's hooks.
  Reads the JSON payload from stdin, applies the event to the
  matching session record under `flock`. Always exits 0.
- `agent-orch pick` — print one selected entry's pane id to
  stdout, exit. Used inside the loop.
- `agent-orch loop` — the picker loop body that runs inside the
  orchestrator session.
- `agent-orch list` — print the live registry as a table.
  Useful for scripting and `doctor`.
- `agent-orch unregister <pane-id>` — remove an entry. Called by
  the tmux `pane-exited` hook.
- `agent-orch doctor` — sanity-check tmux version, fzf, agent
  CLIs on PATH, state dir writeability, dist binary at the
  expected path. Surfaces missing deps with one actionable line
  each.
- `agent-orch` (no args) — ensure the `orchestrator` session
  exists, switch the client to it, run the picker loop in the
  orchestrator session.

### Wrapper (`agent-orch wrap`)

- Refuses to run outside tmux (no `$TMUX_PANE`) — the registry
  is pane-keyed; running outside tmux has no useful identity to
  record.
- Refuses to wrap twice in the same pane (a record for that
  `pane_id` already exists). The user can `agent-orch
  unregister %N` first if intentional.
- Supported kinds for v1:
  - `claude` — full hook injection via `claude --settings
    <tempfile>`.
  - `kiro` — project-scoped `.kiro/agents/agent-orch.json`
    injection with refcount cleanup (see below).
  - any other kind — registers and exec's only; state stays
    `unknown`.
- Registers the launch by appending one record to
  `$STATE_DIR/sessions.json` (under `flock`):
  - `pane_id` (`%N` from `$TMUX_PANE`)
  - `pid` (the wrapper's `pid`, which becomes the agent's pid
    after exec)
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

Synthesize a per-launch settings file in `Deno.makeTempDir()`,
load the user's existing `~/.claude/settings.json` as the base
(or `{}` if absent), append (do not replace) hook entries on
`UserPromptSubmit`, `PreToolUse`, `PostToolUse`, and `Stop`,
each calling `<dist>/agent-orch hook <event-name>`. Inherit
`AGENT_ORCH_PANE=%N` into the agent's env so the hook subcommand
can identify which session record to update. Start the agent
with `claude --settings <path> [agent-args...]`. Remove the
tempdir on agent exit via a `defer`-equivalent.

#### Kiro path (per L30 feedback)

Project-scoped `.kiro/agents/agent-orch.json` injection with
refcount cleanup:

1. If `<cwd>/.kiro/agents/agent-orch.json` does not exist:
   create the directory if needed, write the file with
   `agent-orch hook <event>` wired to the same hook events
   Claude uses (via Kiro's documented hook config schema). Stamp
   `created_kiro_config=true` on the session record.
2. If the file already exists: leave it alone. Stamp
   `created_kiro_config=false`. Multiple Kiro sessions in the
   same cwd share one config; the first one in created it.
3. Set `AGENT_ORCH_PANE=%N` in the env, exec `kiro [args...]`.
4. On `pane-exited` (handled by `agent-orch unregister`): if
   the closing record's `created_kiro_config=true`, count the
   live records with `kind=kiro` and the same `cwd`; if zero
   remain, `rm <cwd>/.kiro/agents/agent-orch.json` (and
   `rmdir` the parent dir tree if empty, ignoring failures).
   Then remove the session record.

This means: first wrapper writes, subsequent wrappers reuse,
last wrapper out cleans up. Concurrent Kiro sessions in the
same repo all share one config; the working tree is left
unchanged when they all exit. The `created_kiro_config` flag
on the record makes the cleanup decision deterministic — only
the wrapper that wrote the file is responsible for removing
it (and only when no other Kiro sessions still depend on it).

Edge case: if the closing wrapper crashes before
`pane-exited` fires (rare — tmux always fires the hook on
pane death), a stale `.kiro/agents/agent-orch.json` may
remain. `agent-orch doctor` surfaces this as a warning by
checking known cwds against live sessions.

### Hook subcommand (`agent-orch hook <event>`)

- Reads `$AGENT_ORCH_PANE` for the pane id (set by the wrapper
  when it builds the per-launch hook config; the env propagates
  into the hook's subshell).
- Reads the hook's JSON payload from stdin (Claude / Kiro both
  pipe the event payload).
- Under `flock` on `sessions.json`:
  1. Reads the file.
  2. Finds the record matching the pane id. If no record:
     no-op exit 0 (a stale hook fire after unregistration is
     safe to ignore).
  3. Updates `last_event` to the event name and `last_event_ts`
     to now.
  4. Event-specific updates:
     - `UserPromptSubmit` → `state=running`,
       `last_prompt=<prompt[:80]>` (read from the JSON's
       `prompt` field).
     - `PreToolUse` → `state=running`,
       `last_tool=<tool_name>` (from `tool_name`).
     - `PostToolUse` → leave state alone, refresh
       `last_tool=<tool_name>`.
     - `Stop` → `state=complete`.
  5. Writes the file back atomically (write to `.tmp`, `rename`
     into place — readers never see partial JSON).
- Always exits 0. A failing hook subcommand must not block the
  agent's turn.

### Registry cleanup

- Tmux hook installed by the wrapper:
  `pane-exited → agent-orch unregister #{hook_pane}`.
- `unregister`:
  1. Under `flock`, read sessions.json.
  2. If the pane's record has `created_kiro_config=true`,
     check whether any other live record has `kind=kiro` and
     the same `cwd`. If none, remove
     `<cwd>/.kiro/agents/agent-orch.json`.
  3. Remove the pane's record. Write back atomically.
- Sweep at query time: when the picker reads sessions.json, it
  filters out records whose `pid` is no longer alive
  (`kill -0 $pid` via `Deno.kill(pid, 0)` or `process.kill`).
  Catches the case where the tmux server restarted while the
  orchestrator was down. The sweep also runs a refcount-
  cleanup pass on any orphaned `created_kiro_config` records
  it removes.

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

Picker loop:

```
loop:
  while true:
    sel = `agent-orch pick`
    if sel: tmux switch-client -t sel
    else: sleep 0.2
```

`pick` invokes `fzf` directly via `Deno.Command`, feeds it the
formatted rows, captures the selection, prints the pane id.
fzf is a hard dependency for v1; doctor checks for it.

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
  acceptable — runtime data has to live under `$HOME`, the
  invariant is about deployed code/config (no symlinks installed
  by `deno task deploy`, no settings.json mutations).
- **No `~/.claude/settings.json` mutations**, ever. All Claude
  hook wiring goes through `claude --settings <tempfile>`.
- **No `<cwd>/.kiro/agents/agent-orch.json` orphans.** The
  refcount cleanup contract above is the load-bearing rule.
- **Observation only.** The wrapper exec's the agent; the hook
  subcommand writes a state file; neither sits in the agent's
  I/O path.
- **Public-repo hygiene** (mAId-wide). No internal product /
  team / ticket names in scripts, examples, or this spec.
- **Standalone first.** No mAId registry entry in v1. The
  install path is `dist/agent-orch/` populated by `deno task
  agent-orch:build`. Promoting to a flake / nix package or a
  registry-deployed symlink is a follow-up.

## Test Strategy

Mapped onto `project.md`'s four-layer test surface (`test:unit`,
`test:smoke`, `test:functional`, `test:all`).

### Unit (`deno task test:unit`) — load-bearing

`tests/agent-orch/sessions_test.ts` covers the pure-logic
module under `sources/agent-orch/src/sessions.ts`:

- `readSessions(content)` — empty / malformed-line skip / parse.
- `applyEvent(sessions, paneId, event, payload, now)` — one
  test per event covering state transitions, `last_prompt`
  truncation, `last_tool` refresh, `state_ts` advance, no-op
  on missing record.
- `removeSession(sessions, paneId)` — basic + idempotent.
- `formatRow(session)` — glyph mapping, `cwd-tail` shortening,
  empty-tool case.
- `sortRows(sessions)` — group precedence and within-group
  order.
- `liveFilter(sessions, isAlive)` — drops dead pids.
- `kiroRefcountCleanup(sessions, removingPane)` — returns the
  set of `.kiro/agents/agent-orch.json` paths that should be
  removed (only the cwds where the removing record had
  `created_kiro_config=true` AND no other live `kind=kiro`
  session shares that cwd).

### Smoke (`deno task test:smoke`) — load-bearing

`tests/functional/agent-orch/` with a harness that:

1. Spawns a fresh tmux server on a private socket
   (`tmux -L agent-orch-test`).
2. Runs the compiled binary `<dist>/agent-orch wrap claude --
   <stub-agent>` inside a tmux pane, where `<stub-agent>` is a
   tiny shell script that loops on stdin.
3. Asserts: `sessions.json` has exactly one record with the
   expected `pane_id`, `kind=claude`, and a live `pid`.
4. Calls `<dist>/agent-orch hook UserPromptSubmit` directly
   with synthetic JSON on stdin and `AGENT_ORCH_PANE` set;
   asserts `state=running` and `last_prompt` populated.
   Repeats for `PreToolUse`, `PostToolUse`, `Stop`; asserts the
   record matches the expected transitions.
5. **Kiro cleanup test.** Two passes:
   - Single-session: `wrap kiro` in a fresh tempdir →
     `.kiro/agents/agent-orch.json` exists and the record has
     `created_kiro_config=true`. Kill the stub → `pane-exited`
     fires → file removed; record gone.
   - Concurrent: `wrap kiro` twice in the same tempdir → only
     the first record has `created_kiro_config=true`; file
     exists once. Kill the second pane → file still there
     (other live record). Kill the first → file still there
     (cleanup looks for live records, finds the second-but-
     first-created one... wait — that second one had
     `created_kiro_config=false`, so it never tries to clean
     up). Re-state: the file is removed only when a record
     with `created_kiro_config=true` closes AND no live
     `kind=kiro` records remain in that cwd. Test both orderings:
     (a) close first then close second → first close leaves
     file, second close (whose flag is false) does not try to
     clean, so file leaks → BUG; (b) close second then close
     first → first close finds zero live kiro siblings,
     removes file → OK. **The order-(a) leak is a real
     concern**; see Decision Log for resolution.
6. Tears down the tmux server.

The smoke test does not hit a real Claude / Kiro binary — it
exercises the wrapper, registry, hook, and cleanup paths
against a stub agent. Fast (~3s), depends only on `tmux` +
`bash` + the compiled binary.

### Functional (`deno task test:functional`) — user-driven

Out of scope for v1. Manual test = launch real Claude Code +
Kiro CLI under `agent-orch wrap`, run the picker, jump in/out.
Promote to a fixture if regressions show up.

### Quality gate

`deno task fmt && deno task lint && deno task check` after every
implementation slice.

## Design

### Layout

```
sources/agent-orch/
├── src/
│   ├── main.ts              subcommand dispatch
│   ├── wrap.ts              wrapper (Claude + Kiro paths)
│   ├── hook.ts              hook subcommand
│   ├── pick.ts              picker (fzf invocation)
│   ├── loop.ts              orchestrator-session loop
│   ├── unregister.ts        pane-exited handler + Kiro cleanup
│   ├── doctor.ts            sanity check
│   ├── sessions.ts          parse / apply / format / sort
│   ├── tmux.ts              thin wrappers around tmux commands
│   └── paths.ts             $STATE_DIR resolution, $XDG_STATE_HOME
├── deno.json                this subdir's task surface
└── README.md                install + tmux keybind notes

tests/agent-orch/
├── sessions_test.ts
└── functional/
    ├── stub-agent
    └── smoke

dist/                         (gitignored; populated by build task)
└── agent-orch/
    └── agent-orch            single binary, deno compile output
```

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

Top-level array. Rewrite-on-update under `flock` — at ≤100
records the cost is ms-scale and we get atomic semantics.

### Wrapper flow (Claude case)

```
agent-orch wrap claude -- --resume my-session

  ↓ guard: $TMUX_PANE set, no existing record for that pane
  ↓ register
    flock $STATE_DIR/sessions.lock
    sessions.json := append({pane_id:"%42", pid:Deno.pid,
      kind:"claude", cwd:Deno.cwd(), started:now,
      state:"unknown", state_ts:now, ...,
      created_kiro_config:false})
    tmux set-option -p @agent-orch-pane "%42"

  ↓ install global tmux hook (idempotent, marker-file gated)
    tmux set-hook -g pane-exited 'run-shell "<dist>/agent-orch
      unregister #{hook_pane}"'

  ↓ synthesize per-launch settings
    tmp = Deno.makeTempDirSync()
    base = readUserSettings("~/.claude/settings.json") ?? {}
    settings = mergeHooks(base, {
      UserPromptSubmit: [hookCmd("UserPromptSubmit")],
      PreToolUse:       [hookCmd("PreToolUse")],
      PostToolUse:      [hookCmd("PostToolUse")],
      Stop:             [hookCmd("Stop")],
    })
    writeFile(`${tmp}/settings.json`, settings)
    onExit(() => Deno.removeSync(tmp, {recursive:true}))

  ↓ exec the agent (preserves pid as agent's pid)
    Deno.env.set("AGENT_ORCH_PANE", "%42")
    Deno.execvp("claude", ["--settings", `${tmp}/settings.json`,
                            "--resume", "my-session"])
```

### Wrapper flow (Kiro case)

```
agent-orch wrap kiro -- chat

  ↓ guard, register (same as Claude)

  ↓ Kiro project-scoped config injection
    cfg = "<cwd>/.kiro/agents/agent-orch.json"
    if !exists(cfg):
      mkdir -p "<cwd>/.kiro/agents"
      writeFile(cfg, kiroHookConfig(<dist>/agent-orch))
      flock $STATE_DIR/sessions.lock
      sessions.json: set this record's created_kiro_config = true

  ↓ exec the agent
    Deno.env.set("AGENT_ORCH_PANE", "%43")
    Deno.execvp("kiro", ["chat"])
```

### Unregister flow

```
agent-orch unregister %43

  flock $STATE_DIR/sessions.lock
    record = find(pane_id="%43")
    if !record: exit 0  (already gone)

    if record.created_kiro_config:
      siblings = sessions.filter(s =>
        s.kind=="kiro" && s.cwd==record.cwd && s.pane_id!=record.pane_id)
      if siblings.length == 0:
        rm "<record.cwd>/.kiro/agents/agent-orch.json"
        rmdir parent dirs (best-effort)

    sessions.json := remove(record); write atomically
```

**Concurrent-Kiro cleanup correctness.** With `siblings = live
records other than this one, sharing kind+cwd`, removing the
file happens iff (1) this record created the file, and (2) no
other Kiro session is still alive in that cwd. Two important
cases:

- A → creates, B → reuses (`created_kiro_config=false`),
  A unregisters first → A finds B as a sibling → does not
  remove → B keeps using the file → B unregisters → B's flag
  is false, so B never tries to remove → **file leaks.** This
  is the bug flagged in Test Strategy step 5(a).
- The fix: on every unregister of a Kiro session, regardless
  of whether *this* record created the file, check whether
  *no* live Kiro records remain in that cwd. If true, attempt
  to remove the file (best-effort; the file may already be
  gone if the original creator cleaned up). This makes the
  cleanup creation-flag-agnostic and order-independent. The
  flag is kept on the record so `doctor` can audit which
  sessions are responsible for which files; it is no longer
  the sole gate on removal.
- See Decision Log for the resolution and why it's the right
  call.

### Why this shape

- **All-Deno single binary.** Already in mAId's toolchain; one
  language; one test surface; `deno compile` gives a
  self-contained binary. Hot path on the hook subcommand is
  ~50ms cold start; with ~10-20 hook fires per turn, that's
  well within the sub-second guidance and far less than the
  turn itself takes. If hot-path latency ever matters, the
  hook subcommand is ~30 lines and can be replaced with a bash
  script in a small follow-up; the rest of the system stays
  Deno.
- **Per-launch hook config beats user-config mutation.** Both
  Claude (`--settings <path>`) and Kiro (project-scoped
  `.kiro/agents/`) take per-launch overrides. The Kiro
  override is project-scoped, not per-launch, so concurrent
  Kiro sessions in the same repo coordinate via the refcount
  cleanup rule.
- **Single sessions.json beats split files.** Source of truth
  in one place; readers and writers serialize through one
  flock; rewrite-on-update is fine at our scale.
- **Two states (`running`/`complete`).** Per the user's brief.
  Distinguishing waiting-on-permission from idle is a v2 with
  Claude's `Notification` event.
- **Hook every event into one subcommand.** Same binary, event
  name as subcommand argument. Captures `last_prompt` from
  `UserPromptSubmit.prompt` and `last_tool` from
  `PreToolUse.tool_name` / `PostToolUse.tool_name` — enough for
  the picker summary.
- **Dedicated orchestrator session, not popup.** Persistent
  dashboard pane to extend later; clean "M-o anywhere → orch"
  verb.

### Trade-offs we're accepting

- `complete` doesn't distinguish "idle" from "waiting on user".
  User reads the pane after jumping. v2 with `Notification`.
- Wrapper requires the user to launch through it. Bare `claude`
  / `kiro` invocations are invisible. README documents the
  launch verbs; shell aliases are an obvious follow-up.
- `dist/` is rebuilt by the user; no auto-rebuild on source
  edit. `--watch` is a follow-up.
- No multi-host support. Local-only by design.
- Kiro cleanup is best-effort: if the binary crashes between
  the file write and the flag set, a stale config could
  remain. Doctor surfaces this. Acceptable.

### What's deliberately not built

- No background daemon, no IPC socket, no proxy of agent I/O.
- No notification surface (OS toasts, status-bar icons).
- No automatic `~/.tmux.conf` edits.
- No TUI yet — the picker is fzf. Promoting to a full ratatui-
  / Cliffy-style TUI is a v2 if the picker UX feels limiting.

## Implementation Plan

Ordered. Each row is one coherent dev-loop iteration: small
slice → Quality → Test → Code Review → Push → Review per
kdevkit §7.

1. **Skeleton + sessions module + build task.**
   - `sources/agent-orch/src/sessions.ts` — `Session` type,
     `readSessions`, `applyEvent` (one branch per event), 
     `removeSession`, `formatRow`, `sortRows`, `liveFilter`,
     `kiroRefcountCleanup`.
   - `sources/agent-orch/src/paths.ts` — `$STATE_DIR`
     resolution honoring `$XDG_STATE_HOME`, default
     `$HOME/.local/state/agent-orch/`. Lock-file helper.
   - `sources/agent-orch/deno.json` — task surface for this
     subdir.
   - Top-level `deno.json` gains `agent-orch:build` (deno
     compile → `dist/agent-orch/agent-orch`),
     `agent-orch:doctor`.
   - `tests/agent-orch/sessions_test.ts` covering all functions.
   - Risk: minimal. Pure module + test + build wiring.

2. **Wrapper subcommand (Claude path).**
   - `sources/agent-orch/src/main.ts` — subcommand dispatch.
   - `sources/agent-orch/src/wrap.ts` — Claude path:
     `$TMUX_PANE` guard, sessions append, tempdir settings
     synthesis (merge user base + our hooks), `set-option -p
     @agent-orch-pane`, idempotent global tmux hook install
     (under `flock` on a marker file), `Deno.execvp` to
     claude. Tempdir cleanup via `addSignalListener` /
     `globalThis.addEventListener("unload")`.
   - `sources/agent-orch/src/tmux.ts` — thin wrappers around
     tmux commands.
   - Risk: edge cases around `$TMUX_PANE` (nested tmux),
     missing `~/.claude/settings.json`, claude CLI not on
     PATH.

3. **Hook subcommand.**
   - `sources/agent-orch/src/hook.ts` — reads stdin, reads
     `$AGENT_ORCH_PANE`, calls `applyEvent` under `flock`.
     Always exits 0.
   - Risk: payload field names. Mitigated by the
     research-verified field names; smoke test stubs cover
     all four event payloads.

4. **Wrapper Kiro path + unregister cleanup.**
   - `wrap.ts` Kiro path: project-scoped
     `.kiro/agents/agent-orch.json` write-if-absent + record
     stamp.
   - `sources/agent-orch/src/unregister.ts` — read sessions,
     find record, do creation-flag-agnostic cleanup of
     `.kiro/agents/agent-orch.json` when no live Kiro records
     in that cwd remain, remove record, write atomically.
   - Smoke test step 5 (concurrent Kiro both orderings) is the
     load-bearing test.
   - Risk: orphan files if the binary crashes mid-flight.
     Doctor surfaces; acceptable.

5. **Picker + orchestrator loop.**
   - `pick.ts` — render rows from sessions.json, run fzf,
     print selection.
   - `loop.ts` — the orchestrator-session loop body.
   - `main.ts` bare invocation — ensure orchestrator session
     exists, switch-client.
   - Risk: fzf + tmux interaction edge cases; smoke catches
     common ones.

6. **Smoke harness.**
   - `tests/functional/agent-orch/stub-agent` — `cat`-loop
     shell script.
   - `tests/functional/agent-orch/smoke` — six-step harness
     described in Test Strategy, including both Kiro
     concurrency orderings.
   - Wire into `deno task test:smoke`.
   - Risk: tmux availability on the test host; skip clearly.

7. **Doctor + README.**
   - `doctor.ts` — tmux ≥ 3.2 (display-popup) and ≥ 1.6
     (`set-hook -g pane-exited`), `fzf`, `claude` /
     `kiro` CLIs detected, state dir writeable, dist binary
     at the expected path, orphan `.kiro/agents/agent-orch.json`
     audit.
   - `sources/agent-orch/README.md` — install (run `deno task
     agent-orch:build`, run from `dist/`), the four
     `agent-orch wrap` examples, tmux keybind snippet,
     architecture diagram.
   - Risk: minimal.

8. **Closure.** Per kdevkit §8. The original
   `specs/backlog/agent-session-orchestrator.md` is currently
   untracked in `main`; verify at close-time and `git rm` if
   it's been added.

### Risk notes

- **Hook payload shape drift.** Event names and payload field
  names are pinned by Claude / Kiro docs as of 2026-06; the
  per-launch settings synth is one function and easy to update
  if either renames.
- **`pid` semantics.** `Deno.execvp` (or the Deno equivalent
  via `Deno.Command(...).spawn()` + `Deno.exit` after `wait`)
  preserves the wrapper's pid as the agent's pid, so the
  recorded `pid` matches the running agent. Verified
  implicitly by smoke (`kill -0 $pid` against the stub agent).
- **Concurrent hook fires.** Two agents firing events
  simultaneously hit the same `sessions.lock`. `flock`
  serializes them; worst case is a few ms delay. Acceptable.
- **fzf inside tmux.** Confirmed safe; doctor warns on
  insufficient tmux version.
- **Hot-path cold start.** ~50ms × ~10-20 hook fires per turn
  is within Claude's sub-second hook guidance and far less
  than the turn itself. If it matters in practice, swap
  `hook.ts` for a 30-line bash script in a follow-up.

## Session Log

<!-- date · what was done · decisions made -->

- 2026-06-04 · Initial spec drafted.
- 2026-06-04 · Plan revised after PR #18 feedback. Key changes:
  Kiro lifted to first-class via project-scoped
  `.kiro/agents/agent-orch.json` injection with refcount
  cleanup; language consolidated to all-Deno single binary
  (subcommands replace the bash trio); `$STATE_DIR` clarified
  as runtime data under `$HOME/.local/state` (not a deployed
  config write). Hot-path latency math captured under risks.

## Decision Log

- **Single Deno binary, subcommands.** mAId already runs Deno;
  no new toolchain. `deno compile` yields one self-contained
  binary in `dist/`. Subcommand structure (`wrap`, `hook`,
  `pick`, `loop`, `unregister`, `list`, `doctor`) replaces the
  bash trio (agent-wrap, agent-orch, state-hook). One language
  to test, one toolchain to build, one binary to deploy.
  Alternative (rejected): all Rust. Rejected because adding
  Rust to the flake for ~5ms hot-path savings is
  over-engineering for the current shape; the path to a real
  TUI later exists in Deno (Cliffy, Ink-equivalent, raw ANSI)
  if the picker UX needs it. Alternative (rejected): bash hot
  path + Deno warm path. Rejected because the latency win is
  small (a few hundred ms per turn) and the cost of two
  languages is real. If hot-path latency ever matters, swap
  `hook.ts` for a bash script in a small follow-up — local
  change, not architectural.
- **Kiro lifted to first-class via project-scoped
  `.kiro/agents/agent-orch.json` injection with refcount
  cleanup.** L30 feedback. Wrapper writes the file on launch
  if absent; closing wrappers remove it iff no live Kiro
  sessions remain in that cwd. Concurrent Kiro sessions in
  the same repo share one config; first-creates, last-out-
  deletes. The cleanup is creation-flag-agnostic (any closing
  Kiro session checks the live-sibling count) — the original
  flag-only design had an order-dependent leak (close-creator-
  first leaves the file when reusers remain, and the reusers'
  flag-false records never try to clean). Keeping the
  `created_kiro_config` flag on the record gives `doctor` an
  audit signal but is not the sole gate on removal.
- **`$STATE_DIR` defaults to
  `${XDG_STATE_HOME:-$HOME/.local/state}/agent-orch/`**.
  L35 feedback. The "no `$HOME` writes" invariant is about
  deployed code/config (no symlinks installed by deploy, no
  settings.json mutations) — runtime state has to live under
  `$HOME` by definition. Spec text updated to make this
  distinction explicit.
- **Per-launch hook settings via `claude --settings <tempfile>`,
  not `~/.claude/settings.json` mutation.** Verified: Claude
  Code's `--settings` flag accepts a JSON file and merges it
  into the precedence chain. Wrapper synthesizes a tempdir
  settings file, points the agent at it, removes the tempdir
  on exit.
- **Two states only: `running` / `complete`.** Plus `unknown`
  for hook-less agents. Per the user's brief: keep v1 simple.
  Distinguishing waiting via `Notification` is a small v2.
- **Hook every event into one subcommand.** Same binary, event
  name as subcommand argument. Cheap. Captures `last_prompt`
  and `last_tool` for the picker summary without any
  interpretation logic. Adding more events later is just one
  `+= [{type:"command",...}]` line in the settings synth.
- **Single `sessions.json` (top-level array), flock-and-
  rewrite.** User asked for one file as the source of truth.
  JSONL with in-place line updates is racy; rewriting the
  whole array under `flock` is simple and fast at our scale
  (≤100 sessions).
- **Dedicated `orchestrator` tmux session, not popup.** User
  selected this in the design interview. Persistent dashboard
  pane to extend later (live refresh, summary preview);
  matches "M-o anywhere → orchestrator" cleanly.
- **Stay out of `$HOME` for deployed code. v1 ships to
  `dist/`.** Lets us promote later to a flake / nix install or
  a registry entry without rewriting; keeps the install path
  one `deno compile` for now.
