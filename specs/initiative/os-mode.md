# Initiative: os-mode

- Branch: `initiative/os-mode` (cut from `main` at 0881e13)
- Status: draft for the person's review. Planned on `main`; rebased onto
  `main` once initiative `harness-core` lands, before any build. Uses the
  inbox from initiative `inbox` where present.
- Research: session orchestration and run/binding separation (kept outside
  the repo, summarised in the design below).

- **Build gate: do not build until initiative `harness-core` (PR #60) is
  merged to `main`.** Planned in parallel with it; the build session's first
  step is to rebase this branch onto `main` and confirm harness-core's
  entry points, contract and carriers are there. Before that, only this
  spec changes.

## Builds on harness-core

The ghost in the shell: every base harness is a shell; the ghost makes it
act like me; one ghost, several personas fused; genome, expression,
experience, sleep and adaptation as entry points; bring-ins with the
contract; session and turn bring-ins; state behind config; carriers
append, transient, on demand (and launch, from initiative `inbox`). This
initiative adds to that, it changes none of it.

## Goal

In any environment I launch one session that acts like an operating system
for my work: it keeps my standing work (the inbox, routines), starts a
separate session for each job I ask for, hands it over, and lets me move
to any session and back to see what is running and how it is going, with
the sessions passing control and messages between them.

## Framing

One ghost, many shells at once. The ghost has a **home**: a session that
holds its attention and routines and knows every other session it has
started. Each job runs in **its own shell**, carrying the same ghost,
focused on one outcome, with its own experience and its own sleep. The
home and the jobs **signal** each other. Where a session runs is separate
from where I see it: the same session can be a tmux window today and a
page or a chat thread later.

| Function | What it does, and why |
|---|---|
| **home** (the OS session) | Holds the inbox and routines and the table of running sessions, so I always have one place to start from and come back to. |
| **delegation** (spawning a job) | Starts a separate session for one outcome with a handover pointer, so focused work never clutters the home and each job learns on its own. |
| **signalling** (messages between sessions) | Passes control, progress and results between the home and its jobs, so they work together without me carrying messages. |
| **binding** (where I see a session) | Attaches me to any session through a window, a page or a chat thread, separate from where it runs, so how I reach my sessions can change without changing them. |

## Experience architecture

### 1. Launching home

- Launching the OS session in any environment brings up the home: the
  inbox, the routines, and the table of sessions it knows (reconciled with
  what is actually running; nothing resumed without my yes).

### 2. Asking for a job

- I ask the home for a job; it starts a separate session with the same
  ghost, a handover that points at the work's own record (a feature spec, a
  task), a permission profile chosen for the job, and a binding (a new
  window), and tells me where it is.

### 3. Moving between sessions

- I can go to any session and back. The home shows, for each: its outcome,
  its state (running, waiting for me, done, failed), its latest progress,
  and where to reach it.

### 4. Signalling, with guards

- The home and a job exchange control and messages; a job reports progress
  and results; the home can ask it to stop.
- A message between sessions can never grant a permission or approve an
  action; a job never asks another session to do what it may not do itself.
- What a job reports is treated as untrusted by the home, like tool output.
- Each job is its own session for sleep and adaptation; the home sleeps and
  adapts on its own.

## Macro test

In a scratch HOME with the personal part installed, on Claude Code:

1. Launching OS mode shows the home with an empty session table.
2. Asking for a job starts a new session in a new tmux window, with the
   ghost's genome and the handover pointer in its start; the table lists
   it as running.
3. The job reports progress; the table shows it; asking the home to stop
   the job stops it.
4. A job asking the home to approve a tool it was denied is refused.
5. Restarting the home reconciles the table with what is running and asks
   before resuming anything.

## Constraints

- **Embedded, not wrapped.** Sessions are plain launches of the shell; the
  home uses each shell's own background run, session registry, attach and
  messaging where it has them, and tmux where it does not; no loop of our
  own.
- **Run separate from binding.** A session's run never depends on how I
  am attached to it.
- **No permission laundering.** The home never runs with permission checks
  bypassed; each job gets an explicit permission profile at launch;
  messages carry no authority.
- **Bounded:** a cap on concurrent jobs, jobs do not spawn jobs (depth 1),
  a budget per job, idle jobs reaped.
- **Stable ground:** session ids, resume, hooks and tmux; preview features
  of a shell are used only behind an adapter.
- **Public repo**, **environment-independent**, **Claude Code first.**

## Conceptual design

### Technical concepts

| Concept | What it is, and why | Serves |
|---|---|---|
| **os bring-in** | a session bring-in: a skill plus a small jobs server (MCP and CLI), so the home is a capability of the ghost, not a launcher | home |
| **job record** | outcome, session id, handover pointer, permission profile, binding, state, last progress | home, delegation |
| **process table** | derived at read time from the shells' own registries and the binding layer, never a second source of truth | home |
| **handover** | a pointer to the work's own record plus the outcome, so a job starts from the record, not from the home's conversation | delegation |
| **permission profile** | the permissions a job runs with, chosen at launch | guards |
| **spawn policy** | caps, depth, budget, reaping | guards |
| **signal** | a message between sessions over the shell's own messaging, carrying no authority | signalling |
| **binding** | an attachment of a person to a session: a tmux window, a page, a chat thread | binding |
| **binding layer** | the service that presents sessions through a binding (the tmux dashboard first; a projection service later) | binding |
| **reconcile** | at home start, compare the table with what runs, and ask before resuming | home |

### How it is assembled

```
 HOME  (os bring-in in one plain session)
   inbox + routines (from initiative inbox)
   process table <-- shells' registries + binding layer
   "start a job" --> jobs server --launch--> new shell + ghost
                       job record: outcome, handover, profile, binding
 JOBS  each its own session: genome at start, expression per turn,
       sleep and adaptation on its own
 SIGNALS  home <--> job over the shell's messaging (no authority)
 BINDINGS  tmux window per session + dashboard   (first)
           projection to chat threads             (next)
           a page in a browser                    (later, after review)
```

## System design: mapping per shell

| Need | Claude Code | codex | kiro-cli |
|---|---|---|---|
| background run | its background sessions and supervisor | its app-server daemon | its serve mode *(test)* |
| session registry | its local session records | its threads | *(test)* |
| attach a person | attach, or a tmux window | the TUI's remote attach | *(test)* |
| messages between sessions | its cross-session messaging | queue a message to a thread | *(test)* |
| binding fallback | tmux | tmux | tmux |

Remote bindings offered by a shell's vendor depend on the shell's login;
where they are not available, the binding layer provides its own.

## Streams

| # | Feature | Ships | Needs | Status |
|---|---|---|---|---|
| 1 | os-bring-in | the os session bring-in, the jobs server and CLI, job records, the process table derived from Claude Code's registry and tmux | harness-core | planned |
| 2 | delegation | start a job with handover, permission profile and a tmux-window binding; spawn policy; reconcile at start | 1 | planned |
| 3 | signalling | progress and stop between home and jobs over the shell's messaging; the no-authority guard; job reports treated as untrusted | 2 | planned |
| 4 | home-dashboard | the table in the home and in a tmux dashboard pane | 2 | planned |
| 5 | inbox-at-home | the inbox and routines from initiative inbox shown and driven from the home | inbox, 1 | planned |
| 6 | fold-back | README and project.md: home, delegation, signalling and binding in the ghost's framing | 1-5 | planned |

## Open questions

1. Is a job its own session (my recommendation) or a sub-session of the
   home?
2. May inbox rules start a job on their own, or only propose one in the
   inbox?
3. Should the two tmux tools (the small dashboard in this repo and the
   projection service in a separate repo) become one binding layer?
4. Which bindings matter after tmux: chat threads, a page, a phone?
5. Default permission profile for a job, and the concurrency cap.

## Decision Log

- Ruling (ringmaster, to ratify): OS mode is a session bring-in plus a
  binding layer, not a launcher; the process table is derived from the
  shells' own registries; jobs are separate sessions with a handover
  pointer; tmux is the first binding · why: the shells now separate run
  from binding themselves, so no wrapper is needed; every OS-shaped
  gateway owns the agent loop, which this design forbids · cost if wrong:
  a daemon later, at the cost of the embedded rule
