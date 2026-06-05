# Manual functional test — agent-orch

Drives the compiled binary against **real tmux + real Claude/Kiro
agents**. Covers what the in-process unit tests and the
shell-driven integration script can't:

- The full picker UX (fzf in a real terminal).
- `tmux switch-client -t %ID` actually moving the user's view.
- The `back to orchestrator` keybind.
- Real Claude / Kiro hook payloads driving the registry through
  `Wrapper::hook`'s default method body.
- Multiple sessions with mixed window / pane layouts behaving
  the same way as single-pane sessions.

The integration script (`integration.sh`) covers correctness on a
private tmux server. This document is the human-driven story to
verify ergonomics: "open three coding-agent sessions, open the
loop, navigate around, come back."

## Pre-flight

```sh
# From the repo root, in a real terminal (NOT inside tmux):
deno task agent-orch:check
deno task agent-orch:test
deno task agent-orch:build         # produces dist/agent-orch/agent-orch
deno task agent-orch:integration   # confirms 9/9 shell cases pass
```

Have on PATH:
- `tmux` (≥ 3.2 for `display-popup`; ≥ 1.6 for `set-hook -g pane-exited`)
- `fzf`
- `claude` (Claude Code CLI)
- `kiro` (Kiro CLI)
- `jq` (only used by the integration script; useful for inspecting
  `sessions.json` during this test)

Recommended `~/.tmux.conf` keybind for the round-trip:

```tmux
# Press M-o (Alt-o) from any tmux pane to jump back to the
# orchestrator session.
bind-key -n M-o switch-client -t orchestrator
```

If you don't want to edit `~/.tmux.conf`, set it on the running
server before the test:

```sh
tmux set -g status-keys vi    # or whatever you have
tmux bind-key -n M-o switch-client -t orchestrator
```

Use the absolute path to the compiled binary throughout (or add
`<repo>/dist/agent-orch` to `$PATH` for this shell). Below it's
spelled `agent-orch` — adjust if your shell can't resolve it.

> **Heads-up.** The wrap command synthesizes per-launch Claude
> settings under `${XDG_STATE_HOME:-$HOME/.local/state}/agent-orch/tmp/<pane>/`
> and writes a project-scoped `<cwd>/.kiro/agents/agent-orch.json`
> for Kiro. The Kiro file is removed by `unregister` when the last
> kiro session in that cwd exits; if you Ctrl-C this test halfway
> through, run `agent-orch unregister %N` for any panes you abandon
> (or just delete `~/.local/state/agent-orch/sessions.json` to start
> clean).

## CLI shape (read this first)

`agent-orch wrap` takes a **kind** and then **the actual command
to run** after `--`:

```
agent-orch wrap <kind> [--cwd <dir>] -- <agent-cmd> [args...]
```

- `<kind>` selects the `Wrapper` impl (`claude`, `kiro`, or any
  string for register-only `Other`). It picks how hooks get
  injected.
- `<agent-cmd>` is the program the wrapper actually `execvp`s.
  This is what runs in the pane after the wrapper sets up the
  registry and hook config.

The same word appears twice when wrapping the canonical agents:

```sh
agent-orch wrap claude -- claude                  # plain
agent-orch wrap claude -- claude --resume foo     # with claude args
agent-orch wrap kiro   -- kiro                    # plain
agent-orch wrap kiro   -- kiro chat               # with kiro args
```

The first `claude` is *the kind* (drives hook injection); the
second `claude` is *the binary on PATH* the wrapper exec's. They
just happen to share the name. If you wrote `agent-orch wrap
claude --` with nothing after the `--`, the wrapper exits with
`agent-orch wrap needs an agent command after \`--\`` because it
has no program to exec.

## Test plan — three agent sessions + one orchestrator viewer

You'll create **four** tmux sessions on the user's running tmux
server (not the test socket the integration script uses):

| Session    | Layout               | What runs there |
|------------|----------------------|-----------------|
| `proj-a`   | 1 window, 1 pane     | Claude wrapped  |
| `proj-b`   | 2 windows; window 2 has 2 split panes — Claude in left, plain shell in right | Claude wrapped (in window 2 left pane) |
| `proj-c`   | 1 window with horizontal split — Kiro top, Claude bottom | Kiro + Claude both wrapped |
| `viewer`   | starts as a plain shell — you open the loop from here | a plain shell so you can run `agent-orch` and `M-o` from inside tmux |

The orchestrator session itself (`orchestrator`) is created
automatically by `agent-orch` on bare invocation. You do **not**
create it manually.

### Step 1 — three project sessions, mixed layouts

Open three real terminals (or three iTerm/Terminal tabs), one per
session. Run these *outside any tmux*:

**Terminal 1** — single-pane Claude session in `proj-a`:

```sh
mkdir -p /tmp/proj-a && cd /tmp/proj-a
tmux new-session -s proj-a -n work
# you're now inside proj-a; launch claude through the wrapper
agent-orch wrap claude -- claude
# … press Enter through any Claude startup; type a prompt or two
```

**Terminal 2** — two-window, two-pane layout in `proj-b`:

```sh
mkdir -p /tmp/proj-b && cd /tmp/proj-b
tmux new-session -s proj-b -n notes
# inside proj-b. create a second window for code work:
tmux new-window -n code
# split the new window horizontally (vim-style: C-b %)
tmux split-window -h
# you're now in the right pane. select the LEFT pane:
tmux select-pane -L
# launch claude in the left pane
agent-orch wrap claude -- claude
# leave the right pane as a plain shell; do whatever in it.
# the first window ('notes') is also a plain shell.
```

**Terminal 3** — vertical split, Kiro on top, Claude on bottom in `proj-c`:

```sh
mkdir -p /tmp/proj-c && cd /tmp/proj-c
tmux new-session -s proj-c -n agents
# split the window vertically (top/bottom)
tmux split-window -v
# you're now in the bottom pane. launch claude here:
agent-orch wrap claude -- claude
# return to the top pane (C-b ↑) and launch kiro there:
tmux select-pane -U
agent-orch wrap kiro -- kiro
```

After these three terminals, three (or four — `proj-c` has two)
agents should be running. Confirm from anywhere:

```sh
agent-orch list
```

You should see four rows — three Claude (one per project) and one
Kiro — with state `unknown` until you submit a prompt to each, at
which point the row's state moves to `running` and `last_prompt`
populates. Submit a quick prompt to each agent (anything — `say
hello`) so the state moves off `unknown`.

### Step 2 — the viewer session

**Terminal 4** — a plain shell where you'll launch the loop and
do navigation:

```sh
tmux new-session -s viewer
# inside viewer. you're a plain shell. now open the orchestrator:
agent-orch
```

`agent-orch` (no args) does three things:

1. Notices there's no `orchestrator` tmux session yet; creates it
   detached, running `agent-orch loop-body`.
2. `tmux switch-client -t orchestrator` — your viewer terminal's
   client now displays the orchestrator session, NOT `viewer`.
3. The orchestrator session's pane is running the loop body,
   which renders the picker.

What you should see: an `fzf` picker listing all four wrapped
agents. Each row shows `<state-glyph> <kind> <cwd-tail> · <prompt>
[· <tool>]`. Running rows (`▶`) sort first; complete (`✓`) next;
unknown (`·`) last; within each group, most-recently-active first.

### Step 3 — pick and switch

In the picker, type to filter or use arrow keys; press `Enter` on
one of the rows. `fzf` exits, the loop body reads the chosen pane
id, and runs `tmux switch-client -t %ID`. Your terminal now
displays the **agent's pane** in its **agent's session** —
exactly the pane and window the agent is in, even if the session
has multiple windows or splits.

Verify:

- For `proj-b`'s Claude — you should land on `proj-b:code`'s
  *left* pane (where you ran `agent-orch wrap claude`), not the
  right pane and not window 1.
- For `proj-c`'s Claude — you should land on the *bottom* pane.
- For `proj-c`'s Kiro — you should land on the *top* pane.

This is the load-bearing UX assertion: tmux's `switch-client -t
%ID` jumps to the exact pane regardless of how the session is
nested.

### Step 4 — back to orchestrator

You're now sitting in an agent's pane. Press your `M-o` keybind
(or `tmux switch-client -t orchestrator` directly if you didn't
add the keybind). The terminal flips back to the orchestrator
session.

The picker re-renders against the current registry (state may
have advanced if your agents were processing — `▶` for running,
`✓` if `Stop` fired since you last looked).

Pick a different agent. Switch in. Press `M-o`. Switch in
again. The loop is the user-loop you'd actually use.

### Step 5 — verify Kiro refcount cleanup at exit

In `proj-c`, you have one Claude and one Kiro both rooted at
`/tmp/proj-c`. Kiro's first wrap created
`/tmp/proj-c/.kiro/agents/agent-orch.json`:

```sh
ls /tmp/proj-c/.kiro/agents/
# → agent-orch.json
```

Now exit just the Kiro pane (Ctrl-D it, or whatever Kiro's quit
verb is, or `tmux kill-pane` from another tmux client). The tmux
`pane-exited` hook fires `agent-orch unregister %N`. Since this
was the only `kind=kiro` session in `/tmp/proj-c`, the file
should be removed:

```sh
ls /tmp/proj-c/.kiro/agents/ 2>&1
# → No such file or directory
```

If you had two Kiro panes in `/tmp/proj-c` and exited the
*first* one (the creator), the file should still be there — the
second pane is still using it. This is the close-creator-first
ordering covered by integration case 7; manually exercising it
gives you confidence the lifecycle works under real tmux events.

### Step 6 — verify dead-pid filtering

Pick any wrapped agent. Then in another terminal:

```sh
# Find the agent's pid:
agent-orch list
# kill it directly (simulating a crash before pane-exited could fire):
kill -9 <pid>
```

The `pane-exited` hook should still fire when the pane goes
away — but if for some reason it doesn't (server restart, etc.),
the loop's `render` step filters dead pids via signal-0 probe.
Confirm by re-rendering the picker (press M-o to come back if
you're not already in orchestrator) — the dead row should be
gone.

### Step 7 — clean up

Exit each agent (Ctrl-D / quit verb). Each pane-exit fires
`unregister`, the row disappears from `agent-orch list`, the
per-pane Claude `tmp/<pane>/settings.json` directory gets
removed, the project-scoped Kiro config gets removed when its
last pane exits.

Final state:

```sh
agent-orch list
# → (no registered sessions)
ls ~/.local/state/agent-orch/tmp/ 2>&1
# → empty (or No such file or directory)
ls /tmp/proj-c/.kiro/ 2>&1
# → No such file or directory
```

Kill the orchestrator session and your three project sessions:

```sh
tmux kill-session -t orchestrator
tmux kill-session -t proj-a
tmux kill-session -t proj-b
tmux kill-session -t proj-c
tmux kill-session -t viewer
```

## What you've verified

- **Real tmux integration** — `set-hook -g pane-exited`,
  `set-option -p`, `switch-client -t %ID`, `new-session -d
  agent-orch loop-body` all work against the actual tmux server,
  not the private socket the integration script uses.
- **Real `execvp`** — the wrapper's pid is preserved as the
  agent's pid; `kill -9 <pid_from_list>` actually kills the
  agent.
- **Real Claude / Kiro hooks** — the per-launch settings file
  (Claude) and project-scoped agent config (Kiro) actually fire
  the hook subcommand on real prompts. State transitions
  (`unknown → running → complete`) reflect real agent activity,
  not synthetic JSON payloads piped via `echo`.
- **Picker UX in a real terminal** — fzf rendering, key
  bindings, sort order, preview behavior.
- **Multi-pane / multi-window targeting** — `switch-client -t
  %42` jumps to the exact pane regardless of layout depth.
- **Round-trip workflow** — picker → agent → `M-o` → picker →
  another agent → `M-o`, the actual user loop.
- **Lifecycle cleanup under real pane-exited events** — Kiro
  refcount-agnostic close ordering, Claude per-pane tmpdir
  removal, registry consistency.

## What this doesn't cover (still follow-up tickets)

- `agent-orch doctor` — not implemented yet.
- `sources/agent-orch/README.md` — not authored yet.
- Concurrent hooks at high rate (multiple agents firing events
  in the same millisecond — the integration script exercises
  the lock under serial load; a real stress test would need
  many agents).
- Cross-platform (macOS / Linux flock semantics differ slightly
  but POSIX advisory locks behave the same on both).

## Troubleshooting

- **`agent-orch wrap needs an agent command after \`--\``** —
  you typed `agent-orch wrap <kind> --` with nothing after the
  `--`. The wrapper has no program to exec. See **CLI shape**
  above: the kind is repeated as the first agent argv after `--`,
  e.g. `agent-orch wrap claude -- claude`.
- **`Error: $TMUX_PANE unset`** — you ran `agent-orch wrap`
  outside tmux. The wrapper requires `$TMUX_PANE` so it can key
  the registry by pane id.
- **`pane %N already registered`** — you wrapped twice in the
  same pane, or a previous wrap crashed without `unregister`
  firing. Run `agent-orch unregister %N` and try again.
- **fzf picker shows nothing** — either no wrapped agents, or
  all their pids are dead. Run `agent-orch list` to see the raw
  registry; if rows show but the picker is empty, the live-pid
  probe is dropping them all.
- **Tmux says `no current client`** — `switch-client -t ...`
  needs an attached client. If you're running `agent-orch` from
  a non-tmux shell to bootstrap the orchestrator, the bare
  invocation creates the orchestrator session detached and then
  switches the *current* tmux client — meaning you must be
  *inside* a tmux session (the `viewer` session in this test)
  for the switch to land.
- **Stale Kiro config** — if a wrap crashed between writing
  `<cwd>/.kiro/agents/agent-orch.json` and registering the
  session, the file may stay until manually removed. Future
  `agent-orch doctor` will surface this.
