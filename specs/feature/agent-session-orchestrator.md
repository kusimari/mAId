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

- Coding agents launch through a thin wrapper:
  `agent-wrap <kind> -- <agent-cmd> [args...]`.
- For Claude Code, the wrapper synthesizes a per-launch settings
  JSON in a tempdir and passes it via `claude --settings <path>`,
  wiring lifecycle hooks (`UserPromptSubmit`, `PreToolUse`,
  `PostToolUse`, `Stop`) to a single `state-hook` reporter
  command. **No writes to `~/.claude/settings.json`, ever.** The
  user's existing settings are merged in as the base.
- For other kinds (`kiro`, etc.) the wrapper still registers the
  launch but does not inject hooks — Kiro has no per-launch hook
  injection mechanism in its current CLI. Kiro sessions show up
  in the picker with state `unknown`; the user can jump to them
  and read the pane.
- A single `sessions.json` file (`$STATE_DIR/sessions.json`) is
  the source of truth for both registration and state. Every
  mutation is gated by `flock`. The wrapper appends a record on
  launch; the state-hook updates the matching record on every
  fired event; the tmux `pane-exited` hook removes it.
- A separate `agent-orch` script ensures an `orchestrator` tmux
  session exists, switches the client to it, and runs an fzf
  picker over `sessions.json`. Picking jumps the client to the
  agent's pane via `tmux switch-client -t %ID`.
- Liveness is derived from the tmux `pane-exited` hook (registry
  cleanup) plus a `kill -0 $pid` belt-and-suspenders sweep at
  query time.

The whole tool is a few hundred lines of bash + fzf wiring. It
runs from the repo (`<repo>/dist/agent-orch/...`) with no
`$HOME` writes; promoting it later to a flake-installed package
or a registry-deployed symlink is purely additive.

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
ever updated the session (Kiro v1, agents launched outside the
wrapper that the wrapper later discovers, etc.).

### Wrapper (`agent-wrap`)

- Invocation: `agent-wrap <kind> [--cwd <dir>] -- <agent-cmd>
  [args...]`. The `--` separates the kind label and wrapper flags
  from the agent's own argv.
- Supported kinds for v1:
  - `claude` — full hook injection.
  - `kiro` — registers only; no hook injection (state stays
    `unknown`).
- Refuses to run outside tmux (no `$TMUX_PANE`) — the registry is
  pane-keyed; running outside tmux has no useful identity to
  record.
- Registers the launch by appending one record to
  `$STATE_DIR/sessions.json` (under `flock`), with fields:
  - `pane_id` (`%N` from `$TMUX_PANE`)
  - `pid` (the wrapper's `$$`, which becomes the agent's pid
    after exec)
  - `kind`
  - `cwd`
  - `started` (unix seconds)
  - `state` (`unknown` initially)
  - `state_ts` (unix seconds, mirrors `started` initially)
  - `last_prompt` (empty initially)
  - `last_tool` (empty initially)
  - `last_event` (empty initially)
- For `claude`, synthesizes a per-launch settings file in
  `$(mktemp -d)` and starts the agent with `claude --settings
  <path> [agent-args...]`. The tempdir is removed on agent exit
  via a `trap`. The settings file:
  - Loads the user's existing `~/.claude/settings.json` as the
    base (or `{}` if absent).
  - Adds (does not replace) hook entries on
    `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, and `Stop`,
    each calling `<dist>/state-hook <event-name>` with the
    pane id passed via `AGENT_ORCH_PANE` env var inherited from
    the wrapper.
- The wrapper installs a one-time global tmux hook on first run:
  `tmux set-hook -g pane-exited 'run-shell "<dist>/agent-orch
  unregister #{hook_pane}"'`. Idempotent — gated by a marker
  file under `flock`.
- Refuses to run twice in the same pane (a record for that
  `pane_id` already exists). The user can `agent-orch
  unregister %N` first if intentional.

### State reporter (`state-hook`)

- Single bash script invoked by every Claude Code hook.
  Receives the event name (`UserPromptSubmit`, `PreToolUse`,
  `PostToolUse`, `Stop`) as `$1` and the hook's JSON payload on
  stdin.
- Reads `$AGENT_ORCH_PANE` for the pane id (set by the wrapper
  when it builds the per-launch settings; the env propagates
  into the hook's subshell).
- Under `flock` on `sessions.json`:
  1. Reads the file.
  2. Finds the record matching the pane id.
  3. Updates `last_event` to the event name and `last_event_ts`
     to now.
  4. Event-specific updates:
     - `UserPromptSubmit` → `state=running`,
       `last_prompt=<prompt[:80]>` (read from the JSON's
       `prompt` field).
     - `PreToolUse` → `state=running`,
       `last_tool=<tool_name>` (from the JSON's `tool_name`
       field).
     - `PostToolUse` → leave state alone, refresh
       `last_tool=<tool_name>` so the user sees the most
       recent tool the agent touched.
     - `Stop` → `state=complete`.
  5. Writes the file back (atomically — write to `.tmp`, `mv`
     into place).
- All actions are O(N) over the session count. N is small
  (≤100 typical); rewriting the whole file on each event is
  fine and avoids JSONL line-update racy semantics.
- Always exits 0. A failing reporter must not block the agent's
  turn.

### Registry cleanup

- The global tmux hook installed by the wrapper:
  `pane-exited → agent-orch unregister #{hook_pane}`. Removes
  the pane's record from `sessions.json` under `flock`.
- Sweep at query time: when the picker reads `sessions.json`,
  it filters out records whose `pid` is no longer alive
  (`kill -0 $pid`). Catches the case where the tmux server
  restarted while the orchestrator was down.

### Picker (`agent-orch`)

Subcommands:

- `agent-orch` (no args) — ensure the `orchestrator` session
  exists, switch the client to it, run the picker loop.
- `agent-orch pick` — print one selected entry's pane id to
  stdout, exit. Used inside the loop.
- `agent-orch list` — print the live registry as a table.
  Useful for scripting and `doctor`.
- `agent-orch unregister <pane-id>` — remove an entry. Called
  by the tmux `pane-exited` hook.
- `agent-orch doctor` — sanity-check tmux version, `jq`, `fzf`,
  Claude CLI, state dir writeability. Surfaces missing deps
  with one actionable line each.

Orchestrator session ensure-on-run:

- If a tmux session named `orchestrator` exists, `switch-client
  -t orchestrator`.
- Otherwise, `tmux new-session -d -s orchestrator '<dist>/
  agent-orch loop'` then `switch-client -t orchestrator`.

Picker loop (runs inside the orchestrator session):

```sh
while :; do
  sel=$(agent-orch pick) || { sleep 0.2; continue; }
  tmux switch-client -t "$sel"
done
```

Picker UX:

- One row per registered session.
- Sorted: `running` first, then `complete`, then `unknown`.
  Within each group, most-recently-active first
  (max of `state_ts`, `last_event_ts`, `started`).
- Row format:
  `<state-glyph> <kind> <cwd-tail> · <last_prompt> · <last_tool>`
  with `last_tool` shown only when non-empty.
- `--preview` shows the full record (state, full prompt, last
  event, started-ago, age-of-state).
- Selection runs `tmux switch-client -t <pane-id>` and the loop
  iterates so when the user comes back, the picker is already
  re-rendered against the latest state.

### "Back to orchestrator" UX

- Documented in the README: user adds
  `bind-key -n M-o switch-client -t orchestrator` to their
  `~/.tmux.conf`. The orchestrator never edits user dotfiles.
- The wrapper sets `tmux set-option -p @agent-orch-pane "%N"`
  on the registered pane, so future features (e.g. a status-line
  plugin) can introspect the pane ownership without re-reading
  the registry.

### Hard constraints

- **No `$HOME` writes by deploy.** The tool runs from
  `<repo>/dist/agent-orch/`. State (`~/.local/state/agent-orch/`
  or `$XDG_STATE_HOME/agent-orch/`) is per-user runtime data,
  not deployed code, and is allowed.
- **No `~/.claude/settings.json` mutations**, ever. All hook
  wiring goes through `claude --settings <tempfile>`.
- **Observation only.** The wrapper `exec`s the agent; the
  reporter writes a state file; neither sits in the agent's I/O
  path.
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

Pure-logic Deno modules under `sources/agent-orch/lib/`:

- `sessions.ts` — read / update / write the sessions file.
  Functions: `readSessions(s) → Session[]`, `applyEvent(sessions,
  paneId, event, payload) → Session[]`, `formatRow(session) →
  string`, `sortRows(sessions) → Session[]`. The bash
  wrapper-side calls `deno run` on a small entrypoint that takes
  args + stdin and prints the new file contents — keeps the
  bash code about plumbing and the logic about state.
  - Tests: empty file, append, in-place update on hook event,
    dead-pid filter, sort precedence, format row variants
    (long prompt truncation, missing tool, unknown state).

The bash scripts themselves are tested via smoke; the Deno
modules carry the logic worth typed-test coverage.

### Smoke (`deno task test:smoke`) — load-bearing

`tests/functional/agent-orch/` with a harness that:

1. Spawns a fresh tmux server on a private socket
   (`tmux -L agent-orch-test`).
2. Runs `<dist>/agent-orch/agent-wrap claude -- <stub-agent>`
   inside a tmux pane, where `<stub-agent>` is a tiny shell
   script that loops on stdin (simulates a long-running agent).
3. Asserts: `sessions.json` has exactly one record with the
   expected `pane_id`, `kind=claude`, and a live `pid`.
4. Calls the state-hook directly with synthetic JSON payloads
   to simulate Claude firing each event in turn:
   `UserPromptSubmit` → asserts `state=running` and
   `last_prompt` populated; `PreToolUse` → asserts `last_tool`;
   `Stop` → asserts `state=complete`.
5. Kills the stub agent; asserts the `pane-exited` hook fired
   and the record is gone from `sessions.json`.
6. Tears down the tmux server.

Smoke does not hit a real Claude binary — the wrapper, registry,
hook, and cleanup are all exercised against the stub. Fast
(~2s), dependency-free (`tmux` + `bash` + `jq`).

### Functional (`deno task test:functional`) — user-driven

Out of scope for v1. Manual test = launch real Claude Code
under `agent-wrap`, run the picker, jump in/out. Promote to a
fixture if regressions show up.

### Quality gate

`deno task fmt && deno task lint && deno task check` after every
implementation slice. Bash scripts go through `shellcheck`
inside the smoke harness (warnings fail the test).

## Design

### Layout

```
sources/agent-orch/
├── bin/
│   ├── agent-wrap            bash, ~80 lines
│   ├── agent-orch            bash, fzf wiring + loop, ~120 lines
│   └── state-hook            bash, ~40 lines
├── claude/
│   └── settings.template.json   merged into per-launch settings
├── lib/
│   └── sessions.ts           deno: parse + apply event + format
└── README.md                 install + tmux keybind notes

tests/
├── sessions_test.ts
└── functional/
    └── agent-orch/
        ├── stub-agent
        └── smoke

dist/                         (gitignored; populated by build task)
└── agent-orch/
    ├── agent-wrap
    ├── agent-orch
    ├── state-hook
    └── lib/sessions.js       deno bundle output
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
    "last_event_ts": 1717459382
  },
  {
    "pane_id": "%43",
    "pid": 12389,
    "kind": "kiro",
    "cwd": "/repo/bar",
    "started": 1717459380,
    "state": "unknown",
    "state_ts": 1717459380,
    "last_prompt": "",
    "last_tool": "",
    "last_event": "",
    "last_event_ts": 0
  }
]
```

Top-level array, not JSONL — we rewrite the whole file on every
update under `flock`. With ≤100 records the cost is negligible
and we get atomic semantics for free.

### Wrapper flow (Claude case)

```
agent-wrap claude -- --resume my-session

  ↓ guard: $TMUX_PANE set
  ↓ register
    flock $STATE_DIR/sessions.lock
    sessions.json := append({pane_id:"%42", pid:$$, kind:"claude",
                              cwd:$(pwd), started:$(date +%s),
                              state:"unknown", state_ts:..., ...})
    tmux set-option -p @agent-orch-pane "%42"

  ↓ install global tmux hook (idempotent, marker-file gated)
    tmux set-hook -g pane-exited \
      'run-shell "<dist>/agent-orch unregister #{hook_pane}"'

  ↓ synthesize per-launch settings
    TMP=$(mktemp -d)
    base=$(cat ~/.claude/settings.json 2>/dev/null || echo '{}')
    SH="<dist>/state-hook"
    jq --arg sh "$SH" '
      .hooks.UserPromptSubmit += [{type:"command", command:$sh+" UserPromptSubmit"}]
    | .hooks.PreToolUse       += [{type:"command", command:$sh+" PreToolUse"}]
    | .hooks.PostToolUse      += [{type:"command", command:$sh+" PostToolUse"}]
    | .hooks.Stop             += [{type:"command", command:$sh+" Stop"}]
    ' <<<"$base" > "$TMP/settings.json"
    trap 'rm -rf "$TMP"' EXIT

  ↓ exec the agent (preserves $$ as agent's pid)
    export AGENT_ORCH_PANE="%42"
    exec claude --settings "$TMP/settings.json" --resume my-session
```

For `kiro` (and any future hook-less kind): everything except
the synth + flag — the wrapper just registers and exec's.

### State reporter flow

```
state-hook UserPromptSubmit < <stdin-json>
  pane=$AGENT_ORCH_PANE                               # "%42"
  prompt=$(jq -r '.prompt // empty' < <(cat))         # from stdin
  flock $STATE_DIR/sessions.lock
    sessions.json := update_record(pane_id=pane,
      state="running",
      last_prompt=$(echo "$prompt" | head -c 80),
      state_ts=$(date +%s),
      last_event="UserPromptSubmit",
      last_event_ts=$(date +%s))

state-hook Stop < <stdin-json>
  flock ...
    sessions.json := update_record(pane_id=$AGENT_ORCH_PANE,
      state="complete",
      state_ts=...,
      last_event="Stop",
      last_event_ts=...)
```

The Deno `lib/sessions.ts` handles parse → mutate → serialize.
The bash wrapper around it is just lock + IO.

### Picker flow

The orchestrator's tmux window runs:

```sh
while :; do
  sel=$(agent-orch pick) || { sleep 0.2; continue; }
  pane="${sel%%	*}"
  tmux switch-client -t "$pane"
done
```

`agent-orch pick`:

```sh
deno run --allow-read <dist>/lib/sessions.js render \
  --state-file "$STATE_DIR/sessions.json" \
  --filter-alive \
  | fzf --with-nth=2.. --preview 'echo {} | cut -f1 | xargs deno run --allow-read <dist>/lib/sessions.js show --state-file "$STATE_DIR/sessions.json"' \
  | cut -f1
```

`render` emits one line per live session:
`<pane-id>\t<state-glyph> <kind> <cwd-tail> · <prompt> [· <tool>]`.

`show` emits a multi-line preview for the focused row.

Liveness filter (`--filter-alive`) checks `kill -0 $pid` and
drops dead records (and removes them from `sessions.json` under
`flock` so the next pick is even cheaper).

### Why this shape

- **Hooks beat scraping** for state. They fire on the actual
  event with an authoritative payload, so we don't maintain
  prompt-string regexes that break on UI updates.
- **Per-launch settings beat user-settings mutation.** No
  side effects, easy to debug (tempdir is one inspection away),
  and matches mAId's "no global state mutation" invariant.
- **Single `sessions.json` beats split registry/state files.**
  The user wanted one source of truth that the hook reporter
  updates and the picker reads. With ≤100 sessions and `flock`,
  rewrite-on-update is plenty fast.
- **Two states (`running`/`complete`) for v1.** Distinguishing
  waiting-on-permission from idle requires either Claude's
  `Notification` event (which we can add as a thin v2) or
  pane-scrape (rejected — fragile). The user reads the pane on
  jump.
- **Hook every event into the same reporter command.** Same
  binary, event name as `$1`. Cheap. Captures `last_prompt` from
  `UserPromptSubmit`, `last_tool` from `PreToolUse` /
  `PostToolUse` — enough for the picker summary without any
  interpretation logic.
- **Dedicated orchestrator session, not popup.** User-selected
  in the design interview. Persistent dashboard pane to extend
  later (live refresh, log preview, summary panel); cleaner
  "M-o anywhere → orchestrator" verb.
- **Bash + fzf for the wrapper / picker; Deno only for
  sessions.ts.** Hook fires must be cheap forks — Deno
  cold-start (~50ms × N) on every event isn't worth it. The
  pure logic (parse, apply event, format, sort) lives in Deno
  for typed unit-test coverage.

### Trade-offs we're accepting

- Kiro registers but stays `unknown`. Kiro CLI has no
  documented per-launch hook injection (no `--settings` flag,
  no `KIRO_CONFIG_DIR`). The user can still see the Kiro
  session in the picker and jump to it; they just don't get
  state. v2 work item: figure out per-launch hook injection
  for Kiro (might require a project-scoped `.kiro/agents/`
  override or upstream contribution).
- `complete` doesn't distinguish "idle" from "waiting on user".
  The user reads the pane after jumping. Adding a `waiting`
  state (via Claude's `Notification` event) is a small v2.
- Wrapper requires the user to launch agents through it. Bare
  `claude` invocations are invisible. README documents the
  launch verbs; shell aliases are an obvious follow-up.
- `dist/` is rebuilt by the user; no auto-rebuild on source
  edit. `deno task agent-orch:build --watch` is a possible
  follow-up.
- No multi-host support. Local-only by design.

### What's deliberately not built

- No background daemon, no IPC socket, no proxy of agent I/O.
- No notification surface (OS toasts, status-bar icons). The
  picker is the surface; `M-o` is the verb.
- No automatic `~/.tmux.conf` edits.

## Implementation Plan

Ordered. Each row is one coherent dev-loop iteration: small
slice → Quality → Test → Code Review → Push → Review per
kdevkit §7.

1. **Skeleton + sessions module.**
   - Add `sources/agent-orch/lib/sessions.ts`: `Session` type,
     `readSessions`, `applyEvent`, `removeSession`, `formatRow`,
     `sortRows`, plus a small `main` entrypoint exposing
     subcommands `render` / `show` / `apply` / `unregister`.
   - Add `tests/sessions_test.ts` covering: empty file, append,
     event-applied state transitions for each event, format
     row, sort precedence, dead-pid filter.
   - Add `agent-orch:build` to `deno.json`. Stub `dist/`
     structure.
   - Risk: minimal. Pure module with typed tests.

2. **Wrapper script (Claude path).**
   - `sources/agent-orch/bin/agent-wrap`. Argument parsing,
     `$TMUX_PANE` check, sessions append (via `deno run
     lib/sessions.js apply --add`), tempdir Claude settings
     synthesis (`jq` to merge user base + our hooks),
     `set-option -p @agent-orch-pane`, idempotent tmux
     `pane-exited` hook install (under `flock` on a
     `.hook-installed` marker), `exec`.
   - `claude/settings.template.json` minimal — empty hooks
     object as the merge target.
   - `kiro` kind code path: register and `exec` only (no
     settings synth).
   - Risk: edge cases around `$TMUX_PANE` (nested tmux),
     missing `~/.claude/settings.json` (base = `{}`), `claude`
     CLI not on PATH (doctor surfaces this).

3. **State reporter.**
   - `sources/agent-orch/bin/state-hook`. Reads stdin JSON,
     reads `$AGENT_ORCH_PANE`, calls `deno run lib/sessions.js
     apply --pane $PANE --event $1` with the JSON piped in,
     under `flock`. Always exits 0.
   - Risk: hook payload field names. Mitigated by the
     research-verified field names (`prompt`, `tool_name`,
     `tool_input`); smoke test stubs cover the four event
     payloads.

4. **Orchestrator picker (`agent-orch`).**
   - Subcommand parsing: bare / `pick` / `list` / `unregister`
     / `doctor` / `loop`.
   - `pick` builds fzf input via `deno run lib/sessions.js
     render --filter-alive`, runs fzf with `--preview` of
     `show`, prints the selected `pane-id` to stdout.
   - `loop` is the dedicated-orchestrator-session loop body.
   - Bare invocation: ensure-session-exists +
     `switch-client -t orchestrator`.
   - Risk: fzf + tmux interaction edge cases (escape codes,
     resize). Smoke catches the common ones.

5. **Smoke test harness.**
   - `tests/functional/agent-orch/stub-agent` — `cat`-loop
     shell script that simulates a long-running agent.
   - `tests/functional/agent-orch/smoke` — six-step harness
     described in Test Strategy.
   - Wire into `deno task test:smoke` (existing `tests/
     functional/run --no-tools` flow picks it up).
   - Risk: tmux availability on the test host. Skip with a
     clear message if `tmux` isn't on PATH.

6. **Doctor + README.**
   - `agent-orch doctor` checks tmux ≥ 3.2 (display-popup) and
     ≥ 1.6 (`set-hook -g pane-exited`), `jq`, `fzf`, `claude`
     CLI, state dir writeability.
   - `sources/agent-orch/README.md` — install, usage (the four
     `agent-wrap` examples), tmux keybind snippet, sessions
     file shape, architecture diagram, Kiro v1 caveat.
   - Risk: minimal. Documentation slice.

7. **Closure.** Per kdevkit §8 — reconcile any in-flight
   markers in this spec, soft `project.md` verify (mention
   `agent-orch` in Layout if useful; otherwise decline),
   backlog cleanup (the original
   `specs/backlog/agent-session-orchestrator.md` is currently
   untracked in `main`, so nothing to `git rm` — verify at
   close-time).

### Risk notes

- **Hook payload shape drift.** Claude Code evolves its hook
  surface; the per-launch settings template is a thin shim
  that's easy to update if a hook event renames. Caught at
  smoke since smoke directly drives `state-hook`.
- **`pid` semantics.** Recording `$$` only works if the
  wrapper truly `exec`s. The wrapper's `exec claude ...` line
  is small and unit-tested implicitly via smoke (the assertion
  that `kill -0 $pid` succeeds against the stub agent).
- **Concurrent hook fires.** Two agents firing `Stop`
  simultaneously hit the same `sessions.lock`. `flock`
  serializes them; worst case is a handful of milliseconds
  delay per event. Acceptable.
- **fzf inside tmux.** Confirmed safe; doctor warns on
  insufficient tmux version.

## Session Log

<!-- date · what was done · decisions made -->

- 2026-06-04 · Initial spec drafted. v1 narrowed to Claude Code
  hooks; Kiro registers as `unknown`. State model simplified
  to `running` / `complete` / `unknown`. Single `sessions.json`
  as both registry and state. Decisions captured in Decision
  Log below.

## Decision Log

- **Per-launch hook settings via `claude --settings <tempfile>`,
  not `~/.claude/settings.json` mutation.** Verified: Claude
  Code's `--settings` flag accepts a JSON file and merges it
  into the precedence chain. Wrapper synthesizes a tempdir
  settings file, points the agent at it, removes the tempdir
  on exit. Side-effect-free. Alternative (rejected): wrapper
  installs hooks into `~/.claude/settings.json` on first run.
  Rejected because it persists across non-orchestrated
  launches and creates an install/uninstall lifecycle.
- **v1 = Claude Code only for hook injection.** Verified Kiro
  CLI has no documented per-launch hook override (no
  `--settings`, no `KIRO_CONFIG_DIR`, no `--hooks-file`).
  Hooks live in project-scoped `.kiro/agents/<name>.json`.
  v1 ships Claude with full hooks; Kiro registers via the
  wrapper but state stays `unknown`. v2: figure out Kiro
  per-launch injection. The registry shape doesn't change
  when Kiro becomes hook-able.
- **Two states only: `running` / `complete`.** Plus `unknown`
  for hook-less agents. Per the user's brief: keep v1 simple,
  don't try to distinguish "waiting on permission" from
  "idle". The user reads the pane on jump. Distinguishing
  waiting via Claude's `Notification` event is a small,
  optional v2.
- **Hook every event into one reporter command.** The state
  hook is `state-hook <event-name>` reading the JSON payload
  on stdin. Same binary, event name as `$1`. Captures
  `last_prompt` (from `UserPromptSubmit.prompt`) and
  `last_tool` (from `PreToolUse.tool_name`,
  `PostToolUse.tool_name`) — enough for the picker summary
  without any interpretation logic. Adding more events later
  is just a `+= [{type:"command", ...}]` line in the settings
  synth.
- **Single `sessions.json` (top-level array), flock-and-
  rewrite.** The user asked for one file as the source of
  truth that hooks update and the picker reads. JSONL with
  in-place line updates is racy; rewriting the whole array
  under `flock` is simple and fast at our scale (≤100
  sessions). Per-pane state files (the original sketch) are
  rejected for the consolidation win.
- **Dedicated `orchestrator` tmux session, not popup.** User
  selected this in the design interview. Gives a persistent
  dashboard pane that the orchestrator can extend later
  (live refresh, summary preview); matches "M-o anywhere →
  orchestrator" cleanly.
- **Bash + fzf for the wrapper / picker; Deno only for
  `sessions.ts`.** Hook fires and pane-exited callbacks must
  be cheap forks — Deno cold-start isn't worth it on every
  event. Pure logic (parse, apply, format, sort) lives in
  Deno for typed unit tests. The bash side is plumbing.
- **Stay out of `$HOME`. v1 ships to `dist/`, not as a
  registry-deployed symlink.** Lets us promote later to a
  flake / nix install or a registry entry without rewriting;
  keeps the install path one bash script + one rebuild for
  now.
