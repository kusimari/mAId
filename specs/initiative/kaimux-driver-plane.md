# Initiative: kaimux — driver plane

## Why

kaimux today observes coding-agent sessions and lets one human
jump between them from one terminal. Two things have changed
since that shape was designed.

**The coding agents grew their own control surfaces.** All three
tools kaimux cares about now publish machine-readable session
state, and two of them accept programmatic input and can answer
their own approval prompts. kaimux's current model — a hook
reporter plus `tmux capture-pane` — reads none of it, and its
`waiting` state is derived from a signal that also fires for a
60-second idle nudge. The dashboard's most load-bearing column is
therefore the one least likely to be right.

**One terminal stopped being enough.** The sessions run on a
desktop; the human is often not at it. The want is to read the
same orchestrator view from a chat app, switch between sessions
there, and answer an agent that is blocked — without kaimux
learning anything about chat apps, and without each new channel
forking kaimux.

Those two pressures have one answer: put the tool-specific
knowledge behind an adapter seam, put the channel-specific
knowledge behind a driver seam, and keep one tool-neutral,
channel-neutral core between them. A terminal becomes one driver
among several rather than the only way in.

This initiative delivers the kaimux half. The chat driver that
consumes it is tracked separately (see *Pairing*).

## Pairing

The corporate chat driver is developed in a separate private
repository on its own initiative, and its streams are numbered 2.x
and 3.x to interleave with this one's 1.x. Nothing in this
initiative may depend on it: the control protocol is the contract,
and the reference driver in stream 1.8 is what proves the contract
without it.

A personal-channel driver (Telegram first, WhatsApp as the
degraded case) is future work in this repo, deliberately out of
scope here.

## Architecture

### Vocabulary

Four things, named so the rest of this spec can be terse:

| Term | What it is |
|---|---|
| **kaimux session** | The long-lived, **named** observation domain. Owns the registry of coding-agent sessions inside it. This is what a driver binds to. |
| **kaimux orchestrator** | The view over one kaimux session — every coding-agent session, its state, and the verbs. Not a process: a projection. |
| **kaimux terminal** | One human at a keyboard, launching or attaching to a kaimux session and rendering the orchestrator as a TUI. |
| **coding-agent session** | One `claude` / `kiro-cli` / `codex` instance in one tmux pane. |

The orchestrator being a *projection* rather than a component is
the load-bearing idea. The kaimux session owns state; the
orchestrator is how anyone reads and acts on it; the terminal is
one consumer. That is what makes a driver a peer of the terminal
rather than a bolt-on.

### Shape

```
   ┌─────────────────────────────────────────────────────────────┐
   │                    kaimux session "work"                    │
   │              (a named scope — one observation domain)        │
   │                                                             │
   │   ┌──────────┐  ┌──────────┐  ┌──────────┐                  │
   │   │ claude   │  │ kiro     │  │ codex    │                  │
   │   │ pane %3  │  │ pane %7  │  │ pane %9  │                  │
   │   └──────────┘  └──────────┘  └──────────┘                  │
   └──────────────────────────┬──────────────────────────────────┘
                              │
                the orchestrator: one view of all of them
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
      ┌──────────────┐ ┌─────────────┐ ┌──────────────┐
      │ kaimux       │ │ chat driver │ │ future       │
      │ terminal     │ │             │ │ driver       │
      └──────────────┘ └─────────────┘ └──────────────┘
```

### Three seams

```
   ADAPTERS                CORE                    RENDERERS
 ┌────────────┐      ┌──────────────┐      ┌──────────────────┐
 │ claude     │      │              │      │ terminal (fzf)   │
 │ kiro       │─────▶│   registry   │─────▶│                  │
 │ codex      │      │   + view     │      │ control plane ──▶ drivers
 │ tmux       │◀─────│              │◀─────│                  │
 └────────────┘      └──────────────┘      └──────────────────┘
  hides which          knows about           hides which
  TOOL it is           neither end           CHANNEL it is
```

- **Adapters** hide tool variety. Each answers the same small set
  of questions, however its tool allows, and declares what it
  cannot answer.
- **The core** is the only thing both tool-neutral and
  channel-neutral. One named scope's registry, one view.
- **Renderers** hide channel variety. The terminal TUI and the
  control plane are two renderers of one view.

Neither end knows the other exists. That property is what lets a
new channel arrive without touching kaimux, and a fourth coding
agent arrive without touching any driver.

## What the tools actually offer

The adapter seam exists because no two tools agree on anything.
This table is the research result that binds the design; it was
verified against the builds installed at planning time
(`claude 2.1.251`, `codex 0.146.1`, `kiro-cli 2.20.1`).

| | claude | codex | kiro |
|---|---|---|---|
| Live discovery | per-session registry file, includes the tmux pane address | daemon query — **no on-disk registry, no tmux address anywhere** | session inventory command + per-session lock file holding a pid |
| Native state | `busy` / `shell` / `idle` / `waiting`, with a reason string | `notLoaded` / `idle` / `active` / `systemError`, active carrying `waitingOnApproval` or `waitingOnUserInput` | `in_progress` / `waiting_on_user` / `completed` / `idle` / `failed` — **newer engine only, not the default** |
| Distinct blocked-on-human | yes | yes, and it distinguishes approval from question | yes, on the newer engine only |
| **Answer an approval** | **no** — needs keystrokes, or its own first-party channel relay | **yes**, two ways: a lifecycle-hook decision, or a response to the request it sends you | **yes**, over its agent protocol — but only for sessions we started |
| Input to a running session | yes, over a per-session socket, with asynchronous delivery receipts | yes, and it can also steer a turn already in flight | **no** — resume-only, in a new process |
| Last assistant message | end-of-turn hook payload | four ways, including a turn-complete notification | end-of-run summary field, or the transcript |
| Interrupt | no | yes | no |

Two conclusions drive the streams:

**Launch mode decides fidelity.** For codex, the default mode runs
a private in-process server: a session a human started by typing
the bare command is *invisible* — no socket, no pid file, nothing
enumerable. For kiro, a session started outside our control is
opaque beyond its existence and cwd. So `wrap` does not become
obsolete; it becomes the thing that guarantees fidelity. It only
becomes optional for claude, which publishes its registry either
way.

**claude is the odd one out on approvals.** Two of three tools can
answer an approval over a real protocol. Only claude needs a
keystroke floor. The plane must therefore express an approval
answer as a first-class verb with an honest outcome, not as
"send some text".

## Streams

Each stream is one branch, one PR, one squash-merge.

1. **named-scope** (`feat/kaimux-named-scope`) — a kaimux session
   becomes a real named scope: scope-addressed registry, `wrap`
   takes the scope, cleanup sweeps every scope.
   Prereq: none.
2. **adapter-seam-claude** (`feat/kaimux-adapter-claude`) — introduce
   the adapter contract and land the claude adapter on it: native
   discovery, native state including the reason string and the
   new background-task state, corrected lifecycle-event mapping,
   end-of-turn message as the snippet. `wrap` becomes optional
   for claude. Prereq: 1.
3. **adapter-codex** (`feat/kaimux-adapter-codex`) — codex adapter:
   ensure the shared daemon at wrap time, subscribe to status,
   send turns, answer approvals. Hook-synthesised registry as the
   daemon-free fallback. Prereq: 2.
4. **adapter-kiro** (`feat/kaimux-adapter-kiro`) — kiro adapter:
   own session creation over its agent protocol so state and
   approvals are available; honest `unknown` for sessions kaimux
   did not launch. Prereq: 2.
5. **core-extraction** (`refactor/kaimux-core-extraction`) —
   structured view model; the terminal picker becomes one
   serializer of it rather than the only shape. No behaviour
   change. Prereq: 2.
6. **control-plane-read** (`feat/kaimux-control-plane-read`) —
   `kaimux serve`: capability handshake, snapshot, peek,
   subscribe; the change-nudge and the output stream.
   Prereq: 5.
7. **control-plane-write** (`feat/kaimux-control-plane-write`) —
   send a turn and answer an approval with honest outcomes;
   terminal input mode and inline approval on the same layer.
   Prereq: 6.
8. **reference-driver** (`feat/kaimux-reference-driver`) — a
   reference driver and a protocol conformance harness, so
   "driver-agnostic" is a tested property rather than a claim.
   Prereq: 7.

Streams 1–4 each ship standalone value to the terminal user.
Stream 5 is the only one with no visible payoff; it is
deliberately sequenced *after* three adapters exist, so the view
model is designed against real variety rather than guessed at.

## Decisions taken at the initiative level

These bind every stream.

- **A kaimux session is scoped to a name, not to the tmux
  server.** Observing everything on the server is explicitly out
  of scope. Two named scopes are two independent fleets, which is
  also what gives a driver two independent orchestrator views
  with no driver-side bookkeeping.

- **Adapters declare what they cannot do; the core never
  fabricates.** A tool that cannot report state yields `unknown`
  plus pane content, never a confident glyph it has not earned. A
  send that went out as keystrokes reports an unverified outcome.
  A driver that lies about delivery is worse than one that admits
  doubt, because the human goes looking for a reply that never
  comes.

- **`wrap` is how fidelity is guaranteed, not a legacy verb.**
  Per-tool launch preparation lives behind it. Sessions started
  outside kaimux are still observed, at whatever fidelity their
  tool allows.

- **The core is daemonless; only `serve` is long-lived.**
  Lifecycle reporters stay short-lived processes that write state
  and exit. `kaimux serve` is a *reader* of that state plus the
  adapters, and is opt-in. Nothing sits in a coding agent's I/O
  path — the observation-only invariant is preserved, not traded
  away for the driver.

- **kaimux ships data; drivers render.** No channel-specific
  shapes in kaimux. This is enforced by this repo's public-repo
  constraint, which forbids naming the corporate gateway at all —
  the constraint is doing design work here, not just compliance.

- **Capabilities are negotiated, and defaults are the poorest
  channel.** A driver declares message size, how many choices it
  can present, whether it can edit a message, whether it has
  threads. Undeclared means unsupported, so a driver that forgets
  degrades instead of over-promising.

- **Two notification kinds, and the split is deliberate.** State
  change is a *nudge* — small, coalescible, never replayed; the
  driver refetches a snapshot. Session output is a *sequenced,
  resumable* stream, and only for subscribed sessions. The
  orchestrator view runs on the nudge, which makes rate-limit
  coalescing correct by construction rather than a later patch.

- **An approval answer is a verb with an identity, never text.**
  It carries the request id the tool issued and a decision. It is
  shaped to match the first-party relay one of the tools already
  ships, so drivers stay reusable outside kaimux and no adapter
  ever parses a terminal to discover a pending prompt.

- **Exactly one code path may answer a given approval, and every
  timeout and cancellation path denies.** Answering twice is
  worse than never answering.

- **Local-socket transport, filesystem permissions as
  authentication.** No listening port. This matches what two of
  the three coding agents already do for their own control
  sockets.

- **Reading a tool's private on-disk or on-wire shapes is
  allowed, but always version-gated with a declared fallback.**
  Several of these formats are explicitly internal to their tool.
  An adapter that cannot confirm a version it understands
  degrades to `unknown` rather than guessing.

## Test strategy

Per `project.md`'s Testing section, with one addition: this
initiative has a class of behaviour that cannot be automated, and
naming it is part of the plan rather than an afterthought.

### Automated

| Layer | What it proves |
|---|---|
| Unit | state machine, scope partitioning, view model, serializers, capability degradation. Load-bearing; sub-second. |
| Adapter contract | one shared suite run against every adapter with a **fake tool** behind it — a stub registry file, a stub protocol server, a stub CLI. Catches an adapter claiming a state it cannot know. |
| Protocol conformance | the reference driver plus a recorded event script; asserts a driver's rendering decisions. This is what makes driver-agnosticism testable. |
| Integration | real tmux on a private server, tempdir state, hidden verbs to force transitions. No credits. |

### Attended — needs a human at a keyboard

These cost API credits and minutes, and cannot be honestly faked.

| # | What | Why a human |
|---|---|---|
| H1 | Drive a **real approval prompt** on each tool; confirm the row flips to blocked with the right reason | needs a real model deciding to ask |
| H2 | **Answer** it from the terminal, then from a driver; confirm the agent proceeds | no stub can prove the agent unblocked |
| H3 | Four-plus sessions across all three tools concurrently; no cross-contamination, correct triage order | concurrency across real agents |
| H4 | Kill `serve`, kill a pane, restart a tool mid-turn; confirm recovery and no stale rows | crash-recovery paths |
| H5 | Rate-limit behaviour under a chatty session; confirm coalescing holds | emergent under real load |
| H6 | Read a deliberately degraded channel's rendering with human eyes | "is this usable" is a judgement call |

**H1 and H2 are the tests that decide whether this initiative is
real.** Everything else is scaffolding around them. Both are
already known-unrun on the existing functional fixture, which is
why they are called out at initiative level rather than left to a
stream.

One open question for H1: for one tool it is unverified whether
approval events reach its persisted transcript during an
interactive turn, since every locally available transcript was
produced in a non-interactive mode that never prompts. If
transcript-tailing becomes a fallback, that needs a live check.

## Status

| Stream | Branch | PR | Status | Shipped | Learnings |
|---|---|---|---|---|---|
| 1 | `feat/kaimux-named-scope` | — | planning | — | — |
| 2 | `feat/kaimux-adapter-claude` | — | planning | — | — |
| 3 | `feat/kaimux-adapter-codex` | — | planning | — | — |
| 4 | `feat/kaimux-adapter-kiro` | — | planning | — | — |
| 5 | `refactor/kaimux-core-extraction` | — | planning | — | — |
| 6 | `feat/kaimux-control-plane-read` | — | planning | — | — |
| 7 | `feat/kaimux-control-plane-write` | — | planning | — | — |
| 8 | `feat/kaimux-reference-driver` | — | planning | — | — |
