# Feature: kaimux

<!-- Read this Session handoff block FIRST. Everything below
     it is design history brought forward from the abandoned
     feat/agent-orch-fix branch. Don't re-read 1800 lines on
     session entry — the handoff names where to start. -->

## Session handoff (next session — read this first)

You are continuing the kaimux dev/review loop. The branch
`feat/kaimux` is open as PR #24, awaiting the user's review.

### State on entry
- `feat/kaimux` branched from main (post-merge of
  `feat/resources-and-kaimux`, commit `e6b8273`).
- The agent-orch source tree is reapplied here as
  `kaimux/`, with `agent-orch` → `kaimux` rename applied
  throughout (88 src refs + tests + this spec).
- Workspace member registered. `just ci` green
  (fmt + clippy + workspace test, 54 kaimux tests
  passing). `just kaimux-build` produces a 1.6 MB
  stripped binary at `dist/kaimux`.
- PR #24 body names provenance + what's-landed vs.
  in-flight.

### What the user expects you to do next
1. **Wait for PR #24 review.** The user opened the
   review themselves; respond to comments via the kdevkit
   §7 comment-prefix convention (`[agent]:`).
2. **After PR #24 merges to main**, continue the
   in-flight kaimux design work — the items in this spec
   under `## Implementation Plan` → `### Open issues`
   (picker-row redesign with inline pane extract,
   end-to-end test hardening with real tmux, spec ↔ code
   reconciliation).
3. **Operate per kdevkit §6/§7/§8.** Each in-flight
   item is its own dev-loop iteration: implement →
   Quality + Test gates → Code Review Gate → Push →
   Agent-dev Review Gate. Closure when the user gives
   the cue.

### What you should NOT do on session entry
- Don't re-do the rename or restructure — already done.
- Don't squash-merge the agent-orch branch — already
  abandoned; the manual reapply landed instead.
- Don't propose rewriting the spec from scratch — the
  design history below is the dev memory; preserve it.

### Files to read before any code change
1. This file's Git Setup, Feature Brief,
   `## Implementation Plan` → `### Open issues` (search
   for "Open issues" or read the bottom).
2. `kaimux/src/main.rs` — single-file, organized as four
   typeclasses (Session / Store / Wrapper / Loop). The
   typeclass-redesign commits in agent-orch-fix's history
   established this shape.
3. `Cargo.toml` (workspace), `Justfile` (kaimux verbs).
4. `specs/project.md` Architecture section for the
   surrounding mAId shape.

### Backlog item to be aware of
`specs/backlog/kdevkit-functional-style-and-idiomatic-libs.md`
captures a kdevkit promotion candidate that emerged from
the `feat/resources-and-kaimux` review: design + dev should
use functional style + idiomatic libraries from the start.
Apply this lens when implementing the in-flight kaimux
items — reach for `Iterator::partition` / `serde-as-schema`
/ `try_for_each` rather than hand-rolling.

---

## Git Setup

- Branch: `feat/kaimux` — landing the agent-orch design as
  `kaimux/` workspace member on the new
  `resources/`+`kaimux/` repo shape (post merge of
  `feat/resources-and-kaimux`).
- Base: `main`
- Worktree: `/local/home/gorantls/tool-workplace/ai-workspace/mAId-kaimux`

### Provenance

This feature is the **rename + restructure landing** of the
in-flight `agent-orch` design that lived on the abandoned
branch `feat/agent-orch-fix` (40 commits, never merged). The
agent-orch source was carried over here in one commit, with
`agent-orch` renamed to `kaimux` throughout (88 source refs,
plus tests, plus spec). The full design history below is
preserved as the **dev memory** for continuing the feature —
the original 4-state lifecycle / multi-line picker / F1-F8
test plan / etc. — not as a description of what already
shipped on this branch.

### What's already in this branch (vs. what's still in flight)

- **Landed** (committed on this branch):
  - `kaimux/` workspace member (Cargo.toml, src/main.rs,
    tests/ — bash integration scripts).
  - Workspace member registered in root `Cargo.toml`.
  - `just kaimux-build` recipe in `Justfile`.
  - `cargo test -p kaimux` passes (54 unit tests).
  - This spec, ported from agent-orch-fix.
- **Still in flight** (continued in the next dev session):
  see Implementation Plan → Open issues, plus any specific
  asks the user surfaces on review of this PR.

## Feature Brief

A **birds-eye dashboard** for every coding-agent session
running across the user's tmux server. One pane of glass
lists every wrapped agent — Claude, Kiro, anything else the
user launches — with a status icon, the tmux address, the
agent kind, the working directory, elapsed time since last
activity, and a snippet of what the pane currently shows.
Pressing one key jumps the tmux client to that pane; another
peeks without leaving the dashboard.

The tool is observation-only. It doesn't manage sessions,
windows, or panes — the user keeps every normal tmux verb
they already know. kaimux only adds the inventory and the
jump table.

The shape of the experience, deliberately minimal:

- Launch a wrapped agent inside any tmux pane:
  `kaimux wrap claude -- claude`.
- See every wrapped agent in one dashboard (`kaimux` from
  any shell, or `<prefix> <KEY>` from within tmux once
  `setup --key <KEY>` has been run).
- Sorted by priority — agents needing attention float to the
  top, working agents sit in the middle, finished and idle
  agents sink to the bottom.
- Pressing `enter` jumps the tmux client to the focused
  agent's pane. The dashboard stays alive so coming back
  preserves cursor and search state.
- Closing an agent's pane removes its row automatically; a
  fresh wrap appears within about a second.

This is intentionally narrower than
[lazyclaude](https://github.com/any-context/lazyclaude),
[claude-dashboard](https://github.com/seunggabi/claude-dashboard),
or [workmux](https://github.com/raine/workmux)'s dashboard:
kaimux doesn't manage sessions (no create / rename /
delete), doesn't route permission prompts, doesn't manage
git worktrees. It's the **inventory + jump table** layer
those tools all need first. The richer flows are tracked as
follow-ups (see Prior art and Implementation Plan).

The rest of this spec is split four ways:

- **Requirements** — what the user sees and does. Two
  surfaces: launch experience (the CLI) and runtime
  experience (the dashboard).
- **Test Strategy** — functional + integration tests
  validate those requirements end-to-end before any design
  appears. A reader can stop here and still know what the
  tool does.
- **Design** — how the requirements are met inside the
  binary: hooks, registry, picker plumbing.
- **Unit Tests** — what the design's primitives must hold,
  exercised against their own contract.

---

## Requirements — launch experience

What the user types and what they observe. Everything in
this section is from the perspective of someone reading
`kaimux --help` and trying it for the first time.

### CLI surface

```
kaimux setup [--key X] [--session NAME]   # opt in to agent tracking
kaimux teardown                           # opt out, no leftovers
kaimux wrap <kind> [--cwd <dir>] -- <agent-cmd> [args...]
kaimux [--session NAME]                   # open the dashboard
```

Three user-facing verbs plus a bare invocation. Anything
else in `--help` is internal plumbing the user shouldn't
need to know about.

### `kaimux setup [--key X] [--session NAME]`

Opts in to agent tracking on the user's machine.

- After `setup`, every wrapped Claude session reports
  lifecycle events to the dashboard. Bare-claude
  invocations (typed without going through `wrap`) are
  unaffected — the user's normal claude usage looks
  identical to before.
- With `--key X`, also binds `<tmux-prefix> X` to "switch
  to the dashboard". So if the user's tmux prefix is
  `Ctrl-b` and they ran `setup --key O`, then `Ctrl-b O`
  from any pane on any session jumps to the dashboard.
- With `--session NAME`, the dashboard is hosted in a
  tmux session named `NAME` instead of the default
  `kaimux`. Useful when the user already has a
  session named `kaimux`, or wants more than one
  dashboard scoped to different agent fleets.
- Idempotent. Re-running `setup` after a binary upgrade or
  a moved install path silently refreshes; it doesn't
  duplicate anything. Re-running `setup --key Y` after a
  prior `setup --key X` swaps the binding cleanly (`X`
  removed, `Y` added).
- Survives until tmux server exits. The user is responsible
  for re-running `setup --key X` after a `tmux
  kill-server` / reboot, or for baking the equivalent
  `bind-key` line into their `~/.tmux.conf` themselves.
  (Persistent install is a follow-up — see Implementation
  Plan.)

### `kaimux teardown`

Opts out. Reverses everything `setup` did.

- The dashboard keybind goes away regardless of which key
  was bound — the user doesn't need to remember the suffix
  they used.
- Wrapped agents that are still running keep running; they
  just stop appearing in the dashboard's status updates.
  The user decides when to close their panes.
- Idempotent. Re-running on an already-torn-down system
  is a silent no-op.
- Leaves the user's other Claude settings (permissions,
  custom hooks, MCP config, anything they wrote
  themselves) byte-for-byte untouched.

### `kaimux wrap <kind> [--cwd <dir>] -- <agent-cmd> [args...]`

Launches a coding agent under observation. The user types
`wrap` once per agent they want to appear in the dashboard.

Argument shape:

- `<kind>` — agent type. `claude` and `kiro` are first-
  class; anything else registers as "kind=other" and shows
  up in the dashboard but doesn't get lifecycle tracking.
- `--cwd <dir>` — optional working directory. Defaults to
  the current directory.
- Everything after `--` is the actual agent invocation,
  passed through verbatim. `wrap claude -- claude` runs
  the user's claude. `wrap claude -- claude --resume abc`
  runs claude in resume mode.

Behavior:

- **Refuses to run outside tmux.** Prints a clear error
  and exits non-zero. (The dashboard keys on tmux pane
  ids, so there's no useful behavior outside tmux.)
- **Refuses without an agent command after `--`.** Same
  shape as above.
- **Refuses to wrap a pane that already has a live wrapped
  agent.** Suggests `kaimux unregister` or killing the
  prior agent first.
- **Auto-replaces a stale registration.** If a previous
  `wrap` in the same pane crashed without cleaning up
  (the agent process exited but the pane stayed alive),
  the next `wrap` notices the dead registration and
  proceeds without complaint. (Stale rows also disappear
  from the dashboard automatically — see _Runtime
  experience → Liveness expectations_.)
- **Becomes the agent process.** The user's prompt
  `wrap claude -- claude` runs as if they had typed
  `claude` directly, except the resulting agent is
  observed by the dashboard. There is no extra parent
  process, no signal forwarding, no second prompt. The
  agent quits on `Ctrl-C` or `Ctrl-D` exactly the way it
  always has.

### Bare `kaimux [--session NAME]`

Opens the dashboard from wherever the user is.

- **From outside tmux** (a plain shell): creates the
  dashboard's tmux session if it doesn't exist, then
  switches the user's tmux client to it. (If the user
  isn't attached to any tmux client, prints a hint and
  asks them to `tmux attach -t <session-name>`.)
- **From inside tmux, anywhere except the dashboard
  session**: same — switches the client to the dashboard.
- **From inside the dashboard session**: this is how the
  dashboard's body actually runs. The user shouldn't need
  to think about this distinction; bare `kaimux`
  always does the right thing.
- **Always works** as a fallback. If the user forgot to
  set up the keybind, or the keybind was lost on tmux
  server restart, typing `kaimux` in any shell pane
  gets them to the dashboard.
- `--session NAME` opens the dashboard with the named
  session. Defaults to `kaimux`. If the user ran
  `setup --session NAME` to install a keybind for a
  custom name, they'd typically use bare `kaimux
  --session NAME` from outside tmux to match.

### Hidden plumbing verbs

`kaimux --help` lists only the four verbs above. Four
more verbs exist for tmux and Claude to call into
(`hook`, `unregister`, `render`, `peek`) but are
deliberately hidden — the user never types them. Tests do,
to drive transitions deterministically; that's what they're
for.

---

## Requirements — runtime experience

What the user sees and does once the dashboard is open. The
dashboard's job is to make "what's running where, doing
what" obvious without focusing each row in turn.

### Lifecycle states

Every wrapped agent is in exactly one of four states. The
state shows as a colored icon at the start of the row.

| State   | Icon | Color  | Meaning                                                                                |
| ------- | ---- | ------ | -------------------------------------------------------------------------------------- |
| waiting | `💬` | yellow | Agent is blocked on the user — permission prompt or clarification. The user must act. |
| done    | `✓`  | green  | Agent finished its turn. Output is sitting there, worth reading.                       |
| idle    | `·`  | yellow | Agent has been sitting `done` long enough to look forgotten. Probably needs attention. |
| working | `▶`  | dim    | Agent is actively running a tool. Self-managing — nothing for the user to do.          |

These are categories the user cares about — *do I need to
think about this agent right now?* The icons exist so the
answer is visible across N rows at a glance.

### Sort order

Sort by "does this agent need the user's attention?" Top of
the dashboard down to the bottom:

1. **waiting** rows — blocked on the user. Most-recently-
   asked first.
2. **done** rows — just finished a turn, output worth
   scanning. Most-recently-finished first.
3. **idle** rows — sitting `done` past the idle threshold;
   either give it a task or close it. Most-recently-
   transitioned-to-idle first.
4. **working** rows — actively making progress, doesn't
   need the user. These sit at the bottom so the user's
   eye lands on the things demanding action first.

Within each bucket, more recent activity sorts above older.

### Row layout

Each row is a small block of four lines: a header with the
identifying fields, then a 3-line excerpt of the agent's
pane. Rows are visually separated by a blank line so the
eye can scan them.

```
💬 proj-c:0.1     claude  …/repos/proj-c           45s
  Allow Bash command "rm -rf node_modules"?
  [y/n/always allow]

✓  proj-a:0.0     claude  proj-a                   2m
  > Done. The failing test in tests/state.rs:42 was caused
    by a stale fixture; updated and re-ran the suite.

·  proj-c:0.0     kiro    …/repos/proj-c           1h
  Ready. Type a message…
  > █

▶  proj-b:code.0  claude  …/work/proj-b            12s
  $ cargo build --release
     Compiling kaimux v0.1.0
     Finished `release` profile [optimized] in 12.3s
```

Header columns, left to right:

- **Status icon** — see lifecycle states above.
- **Tmux address**, formatted as `<session>:<window>.<pane>`
  — what the user would type into `tmux send-keys -t ...`.
  Stable text the user can copy.
- **Agent kind** — `claude`, `kiro`, etc.
- **Working directory** — fixed-width column (~24 chars).
  When the full path fits, shown verbatim. When it
  doesn't, truncated from the front with a leading `…/`
  so the trailing path segments — usually the
  distinguishing ones — stay visible. Examples:
  `proj-b` (fits as-is), `…/projects/kaimux-fix`
  (long path, truncated). Fixed width keeps every row's
  columns aligned so the eye scans cleanly down.
- **Elapsed time** — how long since the last activity
  (`5s`, `2m`, `1h`, `3d`). Live signal that the agent is
  making progress (or stuck).

Snippet lines are the last 3 visible lines of the agent's
pane, indented two spaces so the eye separates them from
the next row's header. Colors and ANSI styling from the
agent's output are preserved — claude's banner, build error
markers, kiro's prompt cursor all render the way the user
remembers them.

### Side preview window

Right half of the dashboard shows a deeper preview of the
focused row's pane — about 25 lines, ANSI-colored, updated
once a second. The user reads the inline 3-line snippet to
scan, and the side preview to actually understand what the
agent is doing.

### Keys

| Key      | What it does                                                                  |
| -------- | ----------------------------------------------------------------------------- |
| `j` / `k`| Move the cursor down / up.                                                    |
| `enter`  | Jump to the focused agent's tmux pane. Dashboard stays alive in the background. |
| `p`      | Peek — open the focused agent's pane in a popup overlay. Closes on `q`.       |
| `x`      | Stop tracking the focused row. (Does **not** kill the agent process.)         |
| `/`      | Filter rows by typing. Type-to-narrow.                                        |
| `esc`    | Exit the dashboard.                                                           |

A one-line cheatsheet of these keys is always visible at the
top of the dashboard.

The intent is narrow: `enter` and `p` are the user's two
ways to look at an agent (by switching to it, or without
switching to it). `x` removes a stuck/crashed entry from
the dashboard if the automatic cleanup didn't catch it. The
user kills agent processes with normal tmux verbs (`Ctrl-D`
in the pane, or `tmux kill-pane`) — kaimux does not
sit in that path.

### Liveness expectations

What the user reasonably expects to "just happen" without
re-opening the dashboard:

- **A new wrap** appears as a new row within ~1 second.
- **A wrapped pane closing** (the agent quits, the user
  runs `tmux kill-pane`, the agent crashes, the tmux
  server restarts and forgets the pane) makes its row
  disappear within ~1 second. The dashboard never lists
  rows that point at panes the user can't actually jump
  to — pressing `enter` on a row always switches the user
  to a live pane, or the row was already gone before the
  user got to it.
- **An agent transitioning** between lifecycle states
  flips the row's icon within ~1 second of the
  transition.
- **A permission prompt** — the agent asking the user
  something — flips the row to `waiting` and floats it to
  the top of the dashboard within ~1 second.
- **The side preview window** ticks live as the agent's
  pane content changes — once-per-second refresh, no
  user action required.
- **The cursor and the search query** survive every
  refresh. Pressing `enter` to jump to a pane and coming
  back lands the user on the same row they were on, with
  the query they had typed still active.

The "no stale rows" guarantee is load-bearing. The user
must never see a row, press `enter`, and have nothing
happen because the underlying pane went away — that breaks
the dashboard's promise. Three independent cleanup paths
(see _Design → Cleanup paths_) ensure stale rows drop
quickly regardless of how the pane went away (clean exit,
crash, kill, server restart).

### Observability outside the dashboard

- **`kaimux render`** dumps the dashboard's current
  data to stdout (one record per line). Useful for
  scripting, status bars, or testing. Format documented
  under Design.
- **The keybind installed by `setup --key X`** is
  surface-able via `tmux list-keys -T prefix` for users
  curious about what's bound where.
- The dashboard is hosted in a normal tmux session
  (default name `kaimux`, configurable via
  `--session`). `tmux attach -t <session-name>` works.
  `tmux ls` shows it alongside the user's other
  sessions.

---

## Hard constraints

- **No deployed `$HOME` writes from `deno task deploy`.**
  The binary is invoked from `<repo>/dist/kaimux/`.
  Per-user runtime state (the registry, lock files) lives
  under `${XDG_STATE_HOME:-$HOME/.local/state}/kaimux/`
  — runtime data has to live somewhere; the invariant is
  about *deployed code/config*, no symlinks installed by
  the registry.
- **`~/.claude/settings.json` is touched only by explicit
  user-invoked `setup` / `teardown`**, never by `wrap` or
  any other implicit path. Both verbs are tag-scoped —
  they edit only entries they themselves added, leaving
  every other field of the user's settings file alone.
- **No orphan project-scoped agent configs.** When the
  last agent of a kind closes in a given working
  directory, any project-scoped config that kind wrote is
  removed. This is the load-bearing rule even for kinds
  whose state-tracking is deferred.
- **Observation only.** The wrapper steps aside before the
  agent runs. The state-reporter writes a small file and
  exits. Nothing sits in the agent's I/O path.
- **Public-repo hygiene** (mAId-wide). No internal
  product / team / ticket names anywhere in this spec,
  scripts, or examples.
- **Standalone first.** No mAId registry entry in v1. The
  binary is a plain Rust executable at `dist/kaimux/`,
  produced by `cargo build --release`. Future packaging
  is purely additive.

---

## Prior art

Adjacent tools we sanity-checked the design against. None
solve the same problem; recording what each does well so
the spec lands at "smallest thing that fills the gap"
rather than "yet another tmux dashboard."

- **[tmux-tea](https://github.com/2KAbhishek/tmux-tea)** —
  fzf-driven tmux session picker (directories + sessions).
  No agent awareness; no live state. We borrow its
  pane-content preview pattern, the cheatsheet header
  line, and the multi-line item styling.

- **[workmux dashboard](https://github.com/raine/workmux)** —
  closest fit. tmux + git worktrees + agent status in a
  single TUI, with `🤖 working / 💬 waiting / ✅ done`
  icons, priority sort, live preview, peek-without-leaving
  (`p`), kill-key. We borrow the **four-state model +
  priority sort + elapsed time + peek key** more or less
  wholesale because workmux has proven they work; we do
  *not* take on its worktree management, PR review, diff
  staging, or input mode (yet). Workmux is the right
  model for "manage parallel agent sessions"; kaimux
  is scoped to "observe across whatever sessions tmux
  already has", so the surfaces stay disjoint.

  **Considered: fork workmux as our base instead.**
  Concrete numbers: workmux is ~447 KB across 167 `.rs`
  files with 49 deps (ratatui TUI, daemon-over-UDS,
  custom backends for tmux/kitty/wezterm/zellij, sandbox
  runner, LLM-driven naming, GitHub PR integration). Our
  binary today is ~2.2 K LOC in one file with 7 deps.
  Forking buys: sidebar, broader agent coverage (gemini /
  codex / copilot already wired), shipped live-preview /
  kill / sweep / theme. It costs:
  (a) a much wider install contract — workmux's setup
  writes config files, tmux hooks, and a Claude plugin
  marketplace entry, colliding with mAId's "no deployed
  `$HOME` writes" hard constraint;
  (b) ongoing rebase tax against an upstream whose
  philosophy is worktree-shaped — every workmux release
  we'd ask "does this assume worktrees we don't have";
  (c) a fundamentally different `wrap` model — workmux
  creates the tmux windows, ours registers whatever pane
  already exists, and that's the load-bearing UX choice
  the user explicitly wants;
  (d) Kiro is already broken upstream, so the fork
  doesn't solve our deferred Kiro slice.
  Conclusion: **borrow the patterns, keep the shape.**
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
  Significantly larger surface than kaimux wants. We
  watch its inline-approval popup pattern as the **likely
  v2 shape** for handling permission prompts from the
  dashboard without switching panes — tracked under
  Implementation Plan → Follow-up.

- **[claude-dashboard](https://github.com/seunggabi/claude-dashboard)**
  — k9s-style TUI for Claude sessions, with conversation-
  log viewer, CPU/memory monitoring, and bulk-kill-idle.
  Useful precedent for the "single pane of glass" framing
  but the implementation is heavier (~2s polling daemon,
  Go binary, custom session manager). Out of scope for
  v1.

The throughline: every adjacent tool ships *more* —
session management, worktree workflows, in-app approvals,
log parsing. kaimux deliberately ships *less* to be the
common substrate any of them could sit on top of.

---

## Runtime prerequisites

The compiled binary is environment-agnostic. It needs:

- `tmux` ≥ 3.2 on PATH (for the keybinding, hook, popup,
  and switch-client features the dashboard relies on).
- `fzf` ≥ 0.71.0 on PATH.
- The wrapped agent's CLI on PATH (`claude`, `kiro-cli`,
  etc.) for the kinds the user actually wraps.
- A writable `$HOME` (for `setup` / `teardown` to edit
  `~/.claude/settings.json`) and a writable
  `${XDG_STATE_HOME:-$HOME/.local/state}/kaimux/`.

Anything else (`flake.nix`, `nix develop`, `direnv`,
`rust-overlay`) is **build-time tooling for mAId
contributors**, not a constraint on users running the
binary.

---

## Test Strategy

Two layers validate the requirements above without depending
on any internal design choice. A reader can stop at the end
of this section and still know what the tool does, by
reading what the tests assert. A third layer (Unit) lives
after the Design section because it tests internal
primitives, not user-visible behavior.

Tests at the requirements layer **may call hidden plumbing
verbs** (`kaimux hook <event>`, `kaimux render`,
`kaimux unregister`) as a stand-in for what real Claude
or tmux would do — calling `hook` directly is how a test
deterministically simulates a permission prompt without
spinning up a real agent waiting for a real prompt. The
**assertions** stay on observable behavior: what the user
sees in the dashboard, what `kaimux render` prints to
stdout, what tmux's keybind listing reports.

**Why bash for functional + integration, not a Rust
test binary.** Considered both. Bash wins for v1: the
tests are mostly `tmux send-keys`, `tmux ls`, `cat
sessions.json | jq`, and `kaimux render` polls.
Bash speaks all of those natively; a Rust test binary
would shell out for each one anyway, with a layer of
indirection between "what to test" and "the assertion."
The cases where Rust would shine — concurrent state,
structured polling, type-safe parsing — aren't hot in
the current scenarios. Bash also lets the user run a
single script directly to debug a fixture by hand
(`functional-test.sh F2b`), which a `cargo test
--features functional` workflow forces into a
recompile cycle. Re-evaluate if the functional script
grows past ~500 lines or if cross-platform assertions
become a load-bearing concern.

### Functional (`tests/kaimux/functional-*.sh`) — user-driven

Drives the **user's real tmux server** with real
`claude` and `kiro-cli` CLIs. This is the only layer that
can catch "hooks weren't actually firing" or "the live-
update loop quietly stopped."

Three scripts:

- `functional-setup.sh <KEY>` — spawns a four-session
  fixture and runs `kaimux setup --key <KEY>`.
- `functional-test.sh` — fires real prompts at the wrapped
  agents via `tmux send-keys`, polls the dashboard, asserts
  the user-observable behaviors below.
- `functional-teardown.sh` — kills the fixture, runs
  `kaimux teardown`. Idempotent.

The fixture (four tmux sessions on the live server,
untouched by other test layers):

| Session      | Layout                                                                         | What's wrapped                  |
| ------------ | ------------------------------------------------------------------------------ | ------------------------------- |
| `proj-a`     | 1 window, 1 pane                                                               | claude                          |
| `proj-b`     | 2 windows. Window 2 (`code`) has a horizontal split — two claudes side-by-side | claude × 2 (same cwd)           |
| `proj-c`     | 1 window, vertical split — kiro top, claude bottom                             | kiro + claude (same cwd)        |
| `kaimux` | the dashboard session itself; bootstrapped detached                            | the dashboard body              |

Functional tests gate on prerequisites — missing tmux /
fzf / claude / kiro-cli on PATH skips with a clear
message rather than failing.

#### F1. Setup spawns the multi-session fixture cleanly

After `functional-setup.sh O`:

- `tmux ls` shows all four sessions.
- The dashboard (`kaimux render`) emits one item per
  wrapped pane, five total (1 + 2 + 2 + 0 — the dashboard
  session itself isn't wrapped).
- `<prefix> O` is registered as a keybind that switches
  to the dashboard. Verified by reading `tmux list-keys
  -T prefix`.

#### F2. Dummy queries propagate into the dashboard

For each wrapped Claude pane in the fixture:

- `tmux send-keys -t <pane> 'list files in cwd' Enter`,
  followed by polling the dashboard.
- While the agent runs the resulting tool, the row's
  status icon is `▶` (working).
- Once the tool completes and the turn ends, the row's
  status icon flips to `✓` (done) within ~1 second.
- After the configured idle threshold passes (default
  ~60s of further quiet), the row's status icon becomes
  `·` (idle).
- The row's snippet text changes between the pre-prompt
  sample and the post-completion sample.
- The elapsed-time field advances monotonically while
  the agent sits in the same state (`5s` → `15s` → `45s`
  → `1m`).

For Kiro: the row's snippet still updates as kiro
responds (proving the inline-extract surface is kind-
independent), but its lifecycle state may not advance
because Kiro hooks are out of scope in v1.

The narrow window on working → done matters: a fast tool
may flip working → done inside a single poll cycle, so the
test polls at high frequency (every 100ms) for up to 30s.

#### F2b. Permission prompts surface as `waiting` and sort to the top

Drive a Claude pane to a permission-prompt state (the
cleanest reproduction is a `Bash` command on a write the
user hasn't pre-allowed).

- Within ~1 second: the matching row's status icon is
  `💬` (waiting).
- The row sorts to the **top** of the dashboard.
- The row's snippet lines reflect the prompt body — the
  user can read the pending question without switching
  panes.
- After the user approves and Claude proceeds, the row's
  state flips through `▶` then `✓`, and the row drops
  down the list as priority decreases.

If the test environment can't reliably trigger a real
permission prompt, this scenario is documented as
**deferred** with an explicit log line, not a hard
failure.

#### F3. Two agents in one window track independently

In `proj-b:code` (the horizontal split with two
claudes):

- Submit different prompts to the left and right panes.
- Both rows track their own state and elapsed-time
  without cross-contamination.
- Their snippets are visibly different (each pane saw a
  different prompt).

#### F4. Mixed kinds in one window

In `proj-c` (vertical split, kiro top, claude bottom):

- Prompt the bottom (claude) — its row advances normally
  through `▶` and `✓`, and its snippet updates.
- Send a query to the top (kiro) — its lifecycle state
  may not advance (out of scope in v1), but its snippet
  does.
- Both panes appear in the dashboard throughout.

#### F5. Closing a wrapped pane removes its row

Pick a wrapped pane (e.g. `proj-a`'s sole pane).

- Capture the pre-state count of dashboard rows.
- `tmux kill-pane -t <pane>`.
- Within ~2 seconds: the dashboard no longer has a row
  for that pane id.
- Other rows are unchanged.

For Kiro specifically:

- If the killed kiro pane was the last kiro session in
  its working directory: the kiro project-scoped config
  is removed.
- If a sibling kiro session in the same directory is
  alive: the config persists.

#### F6. Wrapping a fresh agent appears in the dashboard

From a non-fixture pane (or a fresh tmux session):

- Type `kaimux wrap claude -- claude`.
- Within ~2 seconds: a new row exists for that pane id,
  with the right kind and cwd.
- The new row's snippet reflects the agent's startup
  screen (pre-first-prompt content from the pane).

#### F7. Both agentic-state AND tmux-pane content surface

The load-bearing UX assertion: the dashboard conveys two
independent signals per row. Lifecycle state comes from
hook events; pane snippet comes from live tmux state.

For one wrapped Claude pane:

- Capture the row's status icon and first snippet line.
  Both agree on the agent being inactive.
- Send a prompt that forces a tool. While the tool runs:
  - status icon is `▶` (working)
  - snippet shows the tool's in-flight output (e.g.
    `cargo build` progress) — clearly different from
    the idle snippet.
- After completion:
  - status icon is `✓` (done)
  - snippet shows the post-completion line — different
    from both the pre-prompt and the in-flight
    snippets.
- Verifying independence: stop the hook reporter
  (e.g. unset `KAIMUX_PANE` for the pane) and
  assert the snippet still updates while the icon is
  frozen at its last-known value.

#### F8. `<prefix> KEY` round-trip and dead-pid filter

- `setup --key O` followed by `tmux list-keys -T prefix`
  shows `<prefix> O` bound to switching to the
  dashboard. (We don't synthesize keystrokes — too
  flaky in shell.)
- Re-running `setup --key Q` removes the `O` binding
  and adds a `Q` binding.
- `teardown` removes the binding without needing the
  key argument.
- Killing an agent's pid directly (`kill -9 <pid>`)
  drops its row from the dashboard on the next refresh,
  even if the pane stays alive.

### Integration (`deno task kaimux:integration`) — load-bearing

`tests/kaimux/integration.sh` drives the compiled
binary against a **private tmux server** with a tempdir
for state. Faster, deterministic, doesn't cost API
credits. Uses hidden plumbing verbs (`hook`, `render`,
`unregister`) to simulate what real agents and tmux
would do.

CI environments without tmux/jq/the dist binary skip
silently (exit 0).

#### I1. `wrap claude` registers a session

After `wrap claude -- sleep 300` in a tmux pane, the
dashboard shows one row for that pane.

#### I2. Lifecycle transitions

Drive `kaimux hook <event>` for various events as a
stand-in for the agent's hook executor. Assert the
resulting dashboard row state:

- A "user submitted a prompt" event leaves the row
  inactive (no tool has fired yet).
- A "tool starting" event flips the row to `▶`
  (working).
- A "tool finished" event followed by a "turn done"
  event flips the row to `✓` (done).
- A "permission prompt" event flips the row to `💬`
  (waiting), and the row sorts to the top of the
  dashboard.
- A `done` row that has been quiet past the idle
  threshold (default 60 seconds) renders with the `·`
  (idle) icon.

The test calls `kaimux hook` directly with each
event name. It does not assert on the event names; it
asserts on the resulting dashboard icon and sort
position.

#### I3. Dashboard output reflects new wraps and
unregisters

- Wrap a second pane → `kaimux render` now emits
  two items.
- Run `kaimux unregister <P1>` → render emits one
  item, and the removed pane id is absent.

#### I4. Kiro project-scoped config cleanup

Two kiro sessions sharing a working directory: closing
the first leaves the project-scoped kiro config in
place; closing the second removes it. Holds whether
the closes happen in creator-first or last-created-
first order.

#### I5. `wrap` rejects bad invocations

- Outside tmux: refuses with a clear error.
- Empty agent argv (no command after `--`): refuses
  with a clear error.

#### I6. Pane-exited cleanup

After the first `wrap`, tmux's global `pane-exited`
hook is registered. The hook calls
`kaimux unregister` so a real pane death cleans
the dashboard automatically.

#### I7. `setup` / `teardown` round-trip

- `setup` on a fresh `$HOME` creates the user's
  Claude settings file with our entries.
- `setup` on a Claude settings file that already has
  user-authored entries preserves them; ours are
  appended, not merged into theirs.
- Re-running `setup` is idempotent — no duplicates.
- `teardown` removes only what we added; everything
  else is byte-for-byte preserved.
- `teardown` after a fresh `setup` removes the file
  entirely (no leftover scaffolding).

#### I8. Keybind round-trip

- `setup --key X` installs a keybind whose action
  switches to the dashboard.
- Re-running `setup --key Y` swaps `X → Y` cleanly.
- `teardown` removes the keybind without needing the
  suffix argument.
- `setup` without `--key` doesn't touch the prefix
  table.

#### I9. Pane content surfaces with ANSI colors

`kaimux peek <pane>` (the side preview's data
source) preserves ANSI escape sequences from the
pane. Send a colored marker into a pane via
`tmux send-keys`; assert peek output contains both
the visible text and the escapes.

#### I10. Liveness behaviors

- New wrap → dashboard updates within ~1 second.
- `unregister` → dashboard updates within ~1
  second.
- Hook event → dashboard updates within ~1 second.

(Implemented by polling `kaimux render` rather
than driving fzf directly; what fzf does with
render's output is fzf's responsibility, not ours.)

### What's deliberately not tested

- Concurrent hook fires at high rate (multiple agents
  firing events in the same millisecond — would need
  many real agents, infeasible in CI).
- Cross-platform (we don't run macOS CI; POSIX
  advisory locks behave the same on macOS in practice).
- The fzf TUI's visual rendering — multi-line item
  layout, gap spacing, ANSI color rendering. What we
  emit is asserted; what fzf draws with it is fzf's
  contract, exercised manually by the user attaching
  to the dashboard.

---

## Design

How the requirements above are met. Everything in this
section is implementation detail the user shouldn't have
to know about.

### File layout

```
sources/kaimux/
├── Cargo.toml                    deps: anyhow, clap, fd-lock, nix,
│                                       notify, notify-debouncer-mini,
│                                       serde, serde_json
└── src/main.rs                   single file, ~2200 LOC including tests

tests/kaimux/
├── integration.sh                private-server shell-driven E2E (load-bearing)
├── functional-setup.sh           live-server fixture spawn (user-driven)
├── functional-test.sh            live-server prompt-driven assertions (user-driven)
└── functional-teardown.sh        live-server fixture cleanup (idempotent)

dist/kaimux/kaimux        gitignored; the released binary
```

`src/main.rs` is organized top-to-bottom as four
typeclasses:

```
§1 · Session   record type + apply_event + format_header + sort
§2 · Store     state-dir owner + flock + atomic writes
§3 · Wrapper   trait + Claude / Kiro / Other impls; setup/teardown
§4 · Loop      picker — render / render_to / peek / run / body
CLI            clap + main dispatch
Tests          #[cfg(test)] mod tests
```

### How the lifecycle states are derived

Claude fires hook events at well-defined moments in its
turn — prompt submitted, tool starting, tool done, turn
finished, permission needed. `setup` installs a tagged
hook entry per event in `~/.claude/settings.json`; each
fired event causes Claude to invoke
`kaimux hook <event>` with a JSON payload on stdin.

The `hook` subcommand looks up the matching dashboard
record by pane id (read from `$KAIMUX_PANE`, set by
`wrap`) and applies the event:

| Hook event           | Stored state | Notes                                                  |
| -------------------- | ------------ | ------------------------------------------------------ |
| `UserPromptSubmit`   | working      | User just handed Claude a task; transitional.          |
| `PreToolUse`         | working      | Tool actively running.                                 |
| `PostToolUse`        | working      | Tool finished, but more may follow in the same turn.   |
| `PostToolUseFailure` | working      | Same — more tools may follow.                          |
| `Notification`       | waiting      | **Highest-priority signal** — needs the user.          |
| `Stop`               | done         | Turn finished.                                         |

Stored state is one of three: `Working`, `Waiting`,
`Done`. The fourth user-visible state, `idle`, is a
**render-time decay**: `done` records older than
`IDLE_THRESHOLD_SECS` (default 60) display as `idle`
without their stored state changing.

This keeps the on-disk shape stable (no background
process flipping a file every minute) and lets the
display threshold be tuned without migrations.

`apply_event` always bumps `last_event_ts` regardless of
whether the event maps to a state change — the elapsed-
time column reads from this field.

### Hook installation (`setup` / `teardown`)

`~/.claude/settings.json` follows a nested matcher-groups
schema. `setup` appends one entry per lifecycle event
(`UserPromptSubmit`, `PreToolUse`, `PostToolUse`,
`Notification`, `Stop`) under the `hooks` key, each
tagged with `"x-kaimux-managed": true` so future
`setup` / `teardown` runs find ours unambiguously.

Idempotence rules:

- Re-running `setup` rewrites the command path on tagged
  entries (handles a moved binary) but does not
  duplicate.
- `setup` errors loud on a non-object root or a
  non-array event entry — silent-drop would leave the
  user with a hookless wrapper.
- `teardown` removes only tagged entries. Empty arrays
  are pruned; an empty `hooks` object is pruned; a
  settings file reduced to `{}` is removed entirely.

Bare-claude invocations (claude run without `wrap`) hit
the same hooks but find `$KAIMUX_PANE` unset and
exit silently. The user's normal claude usage is
unaffected.

The keybind installed by `setup --key X` is
prefix-table-bound (not root-bound) so inner TUIs like
claude don't race with tmux for the keystroke. Action:
`switch-client -t kaimux`.

`teardown` self-discovers the keybind by scanning
`tmux list-keys -T prefix` for any binding whose
action is `switch-client -t kaimux`. The user
doesn't need to remember which suffix they bound.

### `wrap`: registering a pane

`wrap` runs inside the registry-mutation lock:

1. Verifies no live double-register, replaces stale.
2. Calls the kind's `prepare` (Kiro writes its
   project-scoped config; Claude is a pass-through).
3. Pushes a fresh `Done`-state record (no events have
   fired yet).

Outside the lock:

4. Installs the global tmux `pane-exited` hook
   (idempotent via marker file + tmux's own
   `set-hook -g` idempotence).
5. Sets a pane option `@kaimux-pane` for future
   introspection.
6. Sets `KAIMUX_PANE` env var.
7. `execvp`s the agent — wrapper process is replaced
   in place. The wrapper's pid becomes the agent's
   pid. No parent process, no signal forwarding.

The wrapper inherits the calling shell's full env, so
PATH ordering is whatever the shell set. (See Design
rationale → PATH-resolved agent binary for why we
don't intervene.)

### Cleanup paths

Three independent cleanup routes catch different
failure modes:

- **`pane-exited` (tmux global hook)** — installed on
  first `wrap`, fires per closing pane, calls
  `kaimux unregister #{hook_pane}`. Handles the
  normal case where the user closes the pane or the
  agent quits cleanly.
- **Liveness probe at render time** — `kill(pid, 0)`
  over every record. Drops dead-pid rows even if the
  tmux hook didn't fire (server restart, killed
  externally).
- **Stale-record replacement at wrap time** — if a
  pane has a record with a dead pid, the next `wrap`
  in that pane runs the prior kind's cleanup and
  proceeds.

Per-kind cleanup runs through the matching `Wrapper`
impl. For Kiro, the project-scoped config is removed
when no other live kiro session shares the working
directory (refcount-agnostic — the creator can close
first while a reuser is alive; closing the last
holder of any kind drops the file).

### Storage and concurrency

A single `sessions.json` is the source of truth for
both registration and state. Every mutation is gated
by a POSIX advisory lock (`fd-lock`). Atomic writes
use the per-pid-tmp-then-rename pattern. State lives
at `${XDG_STATE_HOME:-$HOME/.local/state}/kaimux/`.

### `sessions.json` shape

```json
{
  "pane_id": "%17",
  "pid": 274317,
  "kind": "claude",
  "cwd": "/tmp/proj-a",
  "started": 1780619367,

  "state": "working",
  "state_ts": 1780619400,

  "last_event": "PreToolUse",
  "last_event_ts": 1780619400,

  "created_kiro_config": false
}
```

Fields after `state_ts` carry `#[serde(default)]` so
legacy registries deserialize cleanly — extras are
ignored.

### Picker plumbing (`Loop`)

`Loop::render()` returns a `Vec<Item>` where each
`Item` is `(pane_id, header, [snippet; 3])`. For each
live session it shells out to `tmux display-message`
for the human-readable address and
`tmux capture-pane -p -e -E -1 -S -3` for the
snippet. Tmux failures fall back gracefully —
unresolved address becomes `?:?.<pane-id>`, missing
snippet becomes empty padded lines.

`Loop::render_to(stdout)` serializes each `Item` as

```
<pane_id>\t<header>\n<snippet line 1>\n<snippet line 2>\n<snippet line 3>
```

with `NUL` (`\0`) separating items. fzf reads this
via `--read0`. The leading `<pane_id>\t` is what fzf
keys on (`--id-nth=1`); `--with-nth=2..` hides it
from display.

Header line (tab-separated columns):

```
<icon>\t<addr>\t<kind>\t<cwd-tail>\t<elapsed>
```

Icon carries ANSI color escapes so `--ansi` renders
the right hue. Elapsed is computed in
`format_elapsed(now - last_event_ts)`.

Sort order is implemented by `Session::priority(now)`
returning `0..3` (waiting=0, working=1, done=2,
idle=3), ties broken by `Session::activity()`.

`Loop::peek(pane_id, lines, stdout)` shells out to
`tmux capture-pane -p -e -t <pane_id> -E -1 -S -<lines>`.
Default `lines` is 25, large enough for the side
preview window.

`Loop::body(self_path)` is the long-running fzf
process that drives the dashboard. fzf is spawned
with:

- `--listen=<sock>` — Unix socket fzf accepts
  HTTP/1.1 control commands on.
- `--read0 --gap=1 --highlight-line --ansi` — multi-
  line items, blank gap between, focused-row
  highlight, color rendering.
- `--with-nth=2..` — hide pane id column.
- `--track --id-nth=1` — keep cursor on the same
  pane id across reloads.
- `--preview '<self> peek {1}' --preview-window=right:50%`
  — side preview window, focused row's last 25
  lines.
- `--header='enter jump · p peek · x kill record · / filter · esc exit'`
  — cheatsheet line.
- `--bind 'enter:execute-silent(tmux switch-client -t {1})+clear-query'`
  — non-terminal binding; fzf survives the jump.
- `--bind 'p:execute(tmux display-popup -E "tmux attach -t {1}")'`
  — peek-into-popup, returns on `q`.
- `--bind 'x:execute-silent(<self> unregister {1})+reload(<self> render)'`
  — drop the focused row from the dashboard.

Two background threads drive updates over the listen
socket:

- **Watcher** (`notify-debouncer-mini` on the state
  dir, 100ms debounce). When `sessions.json` actually
  changes, posts `reload(<self> render)` to fzf.
- **Heartbeat** (1 Hz). Posts `refresh-preview` to
  fzf — re-runs the preview command for the focused
  row but doesn't touch the list. Splitting these
  two actions is what kills the flicker the v0
  design suffered (see Design rationale → Heartbeat
  → refresh-preview, not reload).

When fzf exits (Esc / kill), the body returns and
the dashboard session terminates.

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
//   inside kaimux tmux session  → Loop::body
//   anywhere else                   → Loop::run
```

### §1 · Session API

```rust
#[derive(Serialize, Deserialize)]
enum State { Working, Waiting, Done }
// Idle is a *display* state — `done` decays to idle at
// render time after IDLE_THRESHOLD_SECS. Storing it
// would require a background process.

#[derive(Copy, Clone)]
enum DisplayState { Working, Waiting, Done, Idle }

struct Session { ... }  // shape above

impl Session {
    fn apply_event(&mut self, event: &str, now: u64);
    fn display_state(&self, now: u64) -> DisplayState;     // decay applied
    fn format_header(&self, addr: &str, now: u64) -> String;
    fn activity(&self) -> u64;                             // sort tiebreaker
    fn priority(&self, now: u64) -> u8;                    // 0=waiting, 1=done, 2=idle, 3=working
}

fn format_elapsed(secs: u64) -> String;   // "5s", "2m", "1h", "3d"
```

### §2 · Store API

```rust
struct Store { dir: PathBuf }

impl Store {
    fn from_env() -> Result<Self>;
    fn new(dir: PathBuf) -> Self;
    fn read(&self) -> Result<Vec<Session>>;
    fn mutate<F, T>(&self, f: F) -> Result<T>
        where F: FnOnce(&mut Vec<Session>) -> Result<T>;
}
```

`mutate` handles flock + atomic write internally.

### §3 · Wrapper API

```rust
trait Wrapper {
    fn kind(&self) -> &str;
    fn prepare(&self, ctx: &WrapCtx) -> Result<Prepared>;
    fn cleanup(&self, store: &Store, removing: &Session,
               others: &[Session]) -> Result<()>;
    fn hook(&self, store: &Store, pane_id: &str, event: &str,
            stdin: &mut dyn Read, now: u64) -> Result<()> { ... default ... }
}

struct Claude;     // prepare = passthrough; cleanup = no-op
struct Kiro;       // prepare = ensure project config; cleanup = refcount-agnostic
struct Other(String); // register-only; both no-op
```

The default `hook` body is identical for all kinds today.
A future kind whose stdin payload differs can override.

### §4 · Loop API

```rust
struct Loop<'a> { store: &'a Store }

impl<'a> Loop<'a> {
    fn render(&self) -> Result<Vec<Item>>;
    fn render_to(&self, stdout: &mut dyn Write) -> Result<()>;
    fn peek(&self, pane_id: &str, lines: u32, stdout: &mut dyn Write) -> Result<()>;
    fn run(&self, self_path: &Path) -> Result<()>;
    fn body(&self, self_path: &Path) -> Result<()>;
}

struct Item {
    pane_id: String,
    header: String,
    snippet: [String; 3],
}
```

---

## Unit Tests

Validate the Design's primitives. Live at the bottom of
`src/main.rs` in `#[cfg(test)] mod tests`. Run via
`deno task kaimux:test` (delegates to `cargo test`).
~100ms total, load-bearing.

These complement the Functional and Integration tests
above: those validate user-visible behavior; these
validate the contracts the implementation depends on
internally.

### State machine

- `apply_event` for each documented event (working from
  PreToolUse, waiting from Notification, done from Stop,
  etc.) flips the stored state correctly.
- Unknown events bump `last_event_ts` only — no state
  change.
- `display_state(now)` decays `Done` to `Idle` after
  `IDLE_THRESHOLD_SECS`; same record at `now -
  state_ts < threshold` renders as `Done`.

### Sort and display

- `priority(now)`: waiting (0) before done (1) before
  idle (2) before working (3). Sort by attention-needed,
  not by busy-ness. A waiting record always outranks any
  other state regardless of timestamp; a done record
  outranks an idle one (more recent attention-worthy
  output); idle outranks working (working agents are
  self-managing). Decay applies — a `done` record past
  the idle threshold is treated as `idle` for priority
  computation.
- `format_elapsed(secs)` covers all four buckets:
  `<60s` → `Ns`, `<60m` → `Nm`, `<24h` → `Nh`, `≥24h`
  → `Nd`.
- `format_header(addr, now)` emits the documented
  tab-separated columns with the right ANSI color on
  the icon. Address-resolution failure (mocked by
  passing `?:?.<pane-id>`) renders without erroring.
- `cwd_tail` root-anchored edge cases (`/`, ``,
  single segment).

### Render serialization

- Given a list of sessions and an injected pane-content
  fn (returning canned strings), `render_to` emits one
  `\0`-separated item per session, each item parses as
  `<pane_id>\t<header>\n<l1>\n<l2>\n<l3>`.
- A snippet shorter than 3 lines pads to 3.
- A snippet containing `\0` (defensive) is sanitized so
  the null delimiter remains unambiguous.
- After applying an event, a re-render reflects the new
  icon, the new elapsed time, and the re-sorted order.
- After `unregister`, the row disappears from
  `render_to` output; other rows are untouched.
- After a fresh `wrap`, the new row appears in
  `render_to` output, sorted by priority.

### Store

- `read` on missing / empty / malformed file behaves
  per contract (empty / empty / loud error).
- `mutate` round-trips, observes prior state, releases
  the lock on a panic inside the closure.

### Wrapper trait impls

- `Claude::prepare` is a pass-through; argv unchanged.
- `Kiro::prepare` writes the project-scoped config the
  first time, leaves the `created` flag false on
  reuse.
- `Kiro::cleanup` keeps the config while a sibling is
  alive; removes it refcount-agnostically when the
  last holder closes (regardless of close order).
- `wrap()` refuses double-register on a live pid;
  replaces a stale record on a dead pid (and runs the
  prior kind's cleanup); refuses empty agent argv.
- `hook` default body filters on unknown pane (silent
  no-op); applies state correctly via both Claude and
  Kiro impls (proves default-method inheritance).

### `setup` / `teardown` JSON merge

- Creating settings: tagged entries appear; user-
  authored entries are preserved alongside, not
  merged into ours.
- Idempotent: re-running `setup` doesn't duplicate.
- Path-refresh on re-run: command path on tagged
  entries is rewritten to the new self path.
- Loud rejection: non-object root → error;
  non-array event entry → error.
- `teardown` removes only tagged entries; user
  content survives byte-for-byte.
- `teardown` removes the file when only our content
  remained; preserves the file when user content
  remained.
- Full `setup → teardown` round-trip restores the
  pre-state byte-for-byte.

Quality gate: `cargo fmt --check` +
`cargo clippy --all-targets -- -D warnings`.

---

## Design rationale

The non-obvious choices a future reader needs to
understand the code. Listed in roughly the order they
get hit when reading top-to-bottom.

- **Rust, single-file `src/main.rs`.** This started as
  a Deno/TypeScript prototype and pivoted to Rust
  because the wrapper needs `execvp` to replace its
  own process with the agent — the wrapper's pid must
  become the agent's pid so `kill(pid, 0)` from
  another shell actually targets the agent. Deno (and
  any GC'd runtime) can't do that. Rust gets us a
  5-MB static binary, predictable signal handling,
  and honest typeclasses via traits. Single-file
  because the whole thing is ~2200 LOC; splitting
  into modules would add ceremony without buying
  separation.

- **Hook reporter, not a daemon.**
  `kaimux hook <event>` is invoked directly by
  Claude's hook executor on every fire. State
  writes are flock-serialized inside the reporter;
  the reporter exits in <10ms. This keeps the
  observation-only invariant — there's no long-
  running background process to crash, leak fds,
  or get out of sync.

- **`kaimux` is the dashboard session name.**
  Matches the binary name. `tmux ls` shows
  `kaimux:` next to the user's other sessions,
  so the role is discoverable. The bare invocation
  self-detects via `$TMUX` + `tmux display-message
  #{session_name}` whether it's already inside that
  session and routes to either bootstrap-and-switch
  (outside) or the event-driven body (inside). One
  user-facing entry point, two code paths.

- **Prefix-bound keybind, not root-bound.** The
  earlier design bound `M-o` at root. Inner TUIs
  (claude/kiro) eat Alt-letter keys inconsistently,
  so the keystroke would sometimes reach tmux and
  sometimes get swallowed. Prefix-bound (e.g.
  `<prefix> O`) is the standard tmux idiom and tmux
  intercepts every prefix key before any inner
  program sees it. The user picks the suffix at
  install time (`setup --key X`); we don't presume
  one.

- **Four-state lifecycle, not two.** An earlier
  iteration had two states (running / idle) packed
  into the row content. Real use showed two
  problems: (a) the state column lied — it said
  "running" when claude had crashed and printed
  "Not logged in" to the pane; (b) the two-state
  model couldn't surface the most important case,
  permission prompts blocking the user. Switched
  to working / waiting / done, with idle as a
  render-time decay. Sort by attention-needed
  (waiting → done → idle → working) so things the
  user must act on float to the top and self-
  managing working agents sink. Same four states
  workmux ships, same reason they work; sort
  order tuned to "do I need to think about this
  agent?" rather than "is this agent active?".

- **`idle` as render-time decay, not stored.**
  Storing four states would require a background
  process to flip `done` → `idle` after the
  threshold passes. We don't have a daemon; we
  don't want one. Computing it at render time
  from `now - state_ts` is the same UX without
  the moving parts.

- **Multi-line rows over single-line + always-side-
  preview.** Inspired by tmux-tea's preview pattern
  and workmux's row density. For kaimux we need
  more than one row's worth of context at a glance —
  the user is triaging across N agents, not picking
  one directory. A 3-line inline snippet per row
  makes the at-a-glance scan informative; the side
  preview stays for deeper context on the focused
  row. fzf ≥ 0.55 supports `--read0` for null-
  delimited multi-line items + `--gap` for inter-
  item spacing; ours requires ≥ 0.71 anyway for
  `--track --id-nth=N`, so the dependency cost is
  zero.

- **Heartbeat → `refresh-preview`, not `reload`.**
  Both fzf actions update the picker, but they
  have very different cost. `reload(...)` re-runs
  the source command, blocks input while it does,
  and clears the prompt — at 1 Hz the cursor
  jitters and the search query feels broken.
  `refresh-preview` re-runs only the preview
  command for the focused row, doesn't touch the
  list, doesn't block input. So the heartbeat
  (1 Hz, just to keep the side preview live)
  sends `refresh-preview`; the watcher (rare,
  only on real `sessions.json` changes) sends
  `reload`. Net result: list rows are stable,
  side preview ticks live, no flicker.

- **`tmux capture-pane -e` for ANSI.** The earlier
  `peek` used `-p` only, stripping color. Claude
  and kiro produce colored output; the preview
  felt washed out. `-p -e` preserves the escapes;
  fzf's `--ansi` flag interprets them. Same
  change applies to the inline row snippets.

- **`session:window.pane` over raw `%N` for the
  row header.** `%17` is meaningless to a human;
  `proj-b:code.0` is what the user typed when
  they ran `tmux send-keys -t proj-b:code.0`.
  Internally the registry still keys on `%N`
  (stable across renames), and fzf still tracks
  rows by `%N` via `--id-nth=1`; the human-
  readable address is resolved at render time
  via `tmux display-message -p -t <pane>
  '#S:#I.#P'` and only used for display.
  Resolution failure (server gone, pane closed)
  falls back to `?:?.<pane-id>` so the row
  still appears.

- **PATH-resolved agent binary.** The wrapper
  does `execvp("claude", ...)` — no path
  lookup, no PATH munging. Whichever `claude`
  is first in the **launching shell's** PATH
  wins. This is correct: a fresh `zsh -l`
  resolves `claude` exactly the way the user
  expects (their `.zprofile` / `.zshrc` runs
  and whatever shims / wrappers they configure
  land at the right precedence). Bare
  `claude` typed in that same shell would
  resolve to the same binary, so the wrapper
  isn't doing anything surprising.

- **Stale-shell PATH risk.** Some setups have
  two `claude` binaries on PATH — for example,
  an auth-wrapper shim and an upstream
  standalone build. Whichever appears first in
  PATH wins. The user typically arranges their
  login shell so the wanted binary is first;
  the risk is that a shell which inherits PATH
  from a different source (a parent process,
  a tmux-resurrect-restored pane, an IDE
  integration's environ) ends up with a
  different ordering. This isn't a wrap bug —
  bare `claude` from that same shell would
  resolve the same way. The right fix is in
  the user's shell setup. The spec records
  the failure mode here so anyone hitting it
  knows where to look; `kaimux doctor`
  (follow-up) will detect and surface the
  mismatch heuristically.

- **Claude hooks user-globally, not per-launch.**
  v0 wrote a per-launch settings file and
  pointed claude at it. That displaced
  claude's normal precedence chain (login
  state, MCP servers, project settings) — the
  wrapped claude wasn't authenticated even
  though bare claude was. Switching to
  `setup` writing tagged entries to
  `~/.claude/settings.json` preserves the
  precedence chain. The hooks fire on
  **every** claude invocation;
  `$KAIMUX_PANE` filters bare-invocations
  to silent no-ops.

- **Kiro is observation-only in v1.** Kiro
  hooks live inside agent persona JSONs
  using a different schema than Claude —
  camelCase events, inline shape, no nested
  `hooks` array. Wiring them without
  modifying the user's chosen agent persona
  is a follow-up. The inline-snippet path
  still surfaces what kiro is doing
  visually — the user reads the snippet,
  not the icon, for kiro rows.

- **Event-driven picker, not a poll loop.**
  An earlier iteration re-spawned fzf every
  500ms. The user lost cursor position, lost
  typed query, and the picker flickered.
  Switched to fzf's `--listen=<sock>` so the
  same fzf process accepts control commands
  over a Unix socket; `enter` is bound to
  `execute-silent(tmux switch-client -t {1})
  +clear-query`, which is non-terminal — fzf
  doesn't exit on selection. Result: a
  single long-lived fzf process, list cursor
  / query / search state preserved across
  switches, no flicker.

- **Self-discovering teardown.** v0 had
  `kaimux teardown --key X` to undo a
  prior `setup --key X`. Forgetting the key
  meant orphaned bindings. Switched to
  teardown reading `tmux list-keys -T prefix`
  and unbinding any line whose action
  matches the dashboard. No state file, no
  argument, no orphan paths.

- **Functional tests on the user's real
  tmux server.** Integration tests run on a
  private socket — fast, deterministic,
  isolated. But they can't drive real
  claude or kiro-cli. Functional tests fill
  that gap: they spawn the four-session
  fixture on the user's running tmux
  server, fire real prompts via
  `send-keys`, and assert on the resulting
  user-visible behavior. They cost API
  credits and minutes per run, so they're
  user-driven (not in CI), but they're
  the only thing that catches "hooks
  weren't actually firing because the
  wrong `claude` binary was on PATH" or
  "the heartbeat thread quietly stopped".

---

## Implementation Plan

v1 is **incomplete — needs more work before merge.**
The bones are in place (single binary, hooks, picker,
keybind, lifecycle + side preview), but real use plus
a sanity-check against
[workmux's dashboard](https://github.com/raine/workmux)
surfaced that the UX needs more density and the
lifecycle needs more states. Open work continues on
`feat/kaimux-fix`.

### Status — what's landed (on the parent feature branch)

- Single binary, three user-facing verbs + bare
  invocation.
- Claude hook reporter wired through user-global
  `setup` / `teardown` (hooks installed in
  `~/.claude/settings.json`, tagged for clean
  removal).
- Kiro observation-only (registers + lifecycle
  cleanup; lifecycle state stays inactive without
  hook reporting).
- Event-driven persistent picker via fzf `--listen`
  + filesystem watcher. **Two-state lifecycle**
  (Running/Idle) + tmux pane side-preview window.
- User-specified prefix-table keybind via
  `setup --key X` + self-discovering teardown.
- Three test layers: unit tests, integration cases
  on a private tmux server, functional scripts
  driving the user's live server. All gates green
  (44 unit, 17 integration).

### Open issues — to address before we call v1 done

Using the shipped slice end-to-end + comparing to
adjacent tools (Prior art) showed the surface needs
more density. None of these are blockers for the
design, but together they mean v1 isn't yet "the
state we want."

- **Four-state lifecycle + priority sort.** Today's
  two-state (running/idle) is too coarse — it hides
  the most important case (waiting on permission)
  and doesn't help the user prioritize across N
  agents. Move to working / waiting / done / idle
  (stored as Working/Waiting/Done in JSON; idle is
  a render-time decay), with priority sort
  waiting > done > idle > working — sort by
  attention-needed, so the things blocking the user
  are at the top and self-managing working agents
  sit at the bottom.
- **Picker-row redesign (this revision's main
  payload).** The current row is `<glyph> <kind>
  <cwd>` with all live signal in the side preview
  only. Real use showed that's too thin: a 6-row
  picker hides everything until the user focuses
  each row in turn. The redesign carries the
  four-state icon, `session:window.pane`, kind,
  cwd-tail, and elapsed time in the header line,
  plus a 3-line inline pane-content snippet — at-
  a-glance density without losing the side
  preview. Needs:
  - `State` becomes a 3-variant enum;
    `DisplayState` adds Idle as a render-time
    decay.
  - `Session::apply_event` re-mapped to the new
    states; `Notification` becomes a real
    signal.
  - `Session::format_header` carries icon + addr
    + kind + cwd-tail + elapsed.
  - `Loop::render` returns multi-line `Item`s
    sorted by priority; `render_to` serializes
    with `\0` separators.
  - `Loop::body` adds `--read0 --gap=1
    --highlight-line --ansi` to fzf invocation,
    plus the cheatsheet header line, the
    peek-popup bind, and the unregister bind.
  - `peek` adds `-e` to `tmux capture-pane` for
    ANSI.
  - Default `peek --lines` rises (10 → 25).
  - `setup` HOOK_EVENTS list adds `Notification`.
  - `setup` and bare invocation accept
    `--session NAME` (default `kaimux`).
- **Stale-shell PATH risk has only been
  documented, not detected at runtime.** `agent-
  orch doctor` (follow-up) would catch this.
- **Functional fixture has not been driven through
  the full user-loop yet.** The fixture-spawning
  script and the teardown script exist; the
  asserted-scenarios script (F1–F8 in Test
  Strategy above) needs to land in
  `functional-test.sh` and be exercised end-to-
  end with real claude/kiro-cli.
- **Spec ↔ code drift on Kiro hook integration.**
  Spec says Kiro is observation-only in v1. Code
  still writes a Claude-shape JSON to
  `<cwd>/.kiro/agents/kaimux.json`, which
  Kiro logs as "invalid agent config" on every
  prompt. Either drop the write entirely (spec-
  true), or wire the right Kiro shape (close the
  deferred slice).

### Follow-up — Kiro state tracking

Pick the right injection point (merge into user's
default agent persona vs. ship a project-scoped stub
persona), implement the camelCase event shape, add
it to the functional test scenarios. Tracked in
`specs/backlog/`.

### Follow-up — `kaimux doctor`

Sanity-check skill. Required checks:
- tmux ≥ 3.2, fzf ≥ 0.71.0 on PATH.
- Each wrappable agent CLI detected.
- State dir writeable.
- Dashboard-switch keybind currently registered.
- No stale Claude hook entries pointing at a
  missing binary path.
- No orphan project-scoped agent configs.
- **PATH-mismatch heuristic**: for each registered
  pane, resolve the parent shell's `which claude`
  and compare to the binary the user marked as
  preferred (config option). Flag any pane where
  the resolved binary differs.

Tracked in `specs/backlog/`.

### Follow-up — persistent tmux keybind across server restarts

Today the keybind is live-only (lost on
`tmux kill-server` / reboot). Two paths under
consideration: edit the user's tmux conf with
sentinel markers; or write a sidecar `tmux.conf`
under our state dir and have the user add one
`source-file -q ...` line themselves. Pick after
the live-only version proves out in real use.

### Follow-up — input mode + inline approval (lazyclaude-style)

Two adjacent UX wins, recorded for after the
four-state shift lands and proves out:

- **Input mode** (`i`). Forward keystrokes to
  the focused pane without leaving the
  dashboard.
- **Inline approval** when the focused row is
  `waiting`. Bind `1` / `2` / `3` to send
  approve / approve-always / reject keystrokes
  into the focused pane.

Both deferred because they expand the surface
area beyond "observe + jump" and require careful
thought about race conditions. v1 ships without
them.

### Follow-up — JSON output for scripting

`kaimux render --json` to emit a structured
array instead of the fzf-shaped serialization.
Useful for shell integrations (status bars,
notification daemons, custom dashboards) and for
tests that want to assert on individual fields
without parsing the multi-line render shape. Same
sort and decay as the human path; just a
different serializer.

### Follow-up — configurable snippet height

Surface height of the inline pane snippet (today
fixed at 3 lines) as runtime-tunable. Two shapes
under consideration:

- A picker keybind (`+` / `-`) that ratchets the
  snippet up or down live, persisting in memory
  for the session. Best for the "I have many
  agents and want to see less per row" case.
- A config-file value or `--snippet-lines N` flag
  on the bare invocation, sticky across sessions.

Likely both eventually. Defer to v2 — v1 ships
with the fixed 3-line height to land the broader
shape first.

### Follow-up — event-driven side-preview refresh

Today the side preview window refreshes once per
second on a heartbeat. That works but is wasteful
when nothing's changing in the focused pane.
Two leads, neither cheap:

- Pipe wrapped pane output through `tmux pipe-pane`
  to a per-pane file, watch the file with
  filesystem events. Cost: one fd + on-disk
  scratch per wrapped pane.
- Cheap-poll using `tmux display-message
  '#{history_size}'` per refresh tick and skip
  the actual capture-pane call when the value
  hasn't changed.

The 1Hz heartbeat is fine for v1. Pick this up if
heartbeat-driven re-renders show up in profiling
or if users complain about CPU usage.
