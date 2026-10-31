# Initiative: inbox

- Branch: `initiative/inbox` (cut from `main` at 0881e13)
- Status: draft for the person's review. Planned on `main`; rebased onto
  `main` once initiative `harness-core` lands, before any build.
- Research: channels, connectors, jobs and inbox UIs (kept outside the
  repo, summarised in the design below).

- **Build gate: do not build until initiative `harness-core` (PR #60) is
  merged to `main`.** Planned in parallel with it; the build session's first
  step is to rebase this branch onto `main` and confirm harness-core's
  entry points, contract and carriers are there. Before that, only this
  spec changes.

## Builds on harness-core

The ghost in the shell: every base harness is a shell; the ghost (my
harness) makes it act like me; one ghost, several personas fused. The ghost
works through genome, expression, experience, sleep and adaptation, as
entry points reached by hooks, framework adapters or on demand. Bring-ins
are standard units with the contract (scope, job, signals, rules, package,
config, state, requires) in their standard's slot; session bring-ins act
for the whole session, turn bring-ins attach when they fit; state lives
only behind config; carriers are append, transient or on demand. This
initiative adds to that, it changes none of it.

## Goal

The personal persona of my ghost perceives my world and keeps a routine:
it gathers what arrives in my accounts (mail, calendar, messages) and what
the harness itself is waiting on, into one inbox, on a schedule or when
something arrives, without me starting a session and without wrapping the
shell.

## Framing

The ghost so far only knows the world when I tell it. An organism also
senses and keeps a rhythm:

| Function | What it does, and why |
|---|---|
| **perception** (channels) | Lets the ghost sense my accounts (mail, calendar, messaging), so it can notice what needs me without being told. |
| **rhythm** (jobs) | Runs the ghost on its own, on a schedule or when something arrives, so routine work (a sweep, sleep) happens without me starting a session. |
| **attention** (the inbox) | Holds, in one place, everything that needs me (from every channel and from the harness's own pending decisions), so I have one list to work through instead of many. |

Perception never acts on what it senses by itself: what arrives is
untrusted, and anything sent out on my behalf needs me.

## Experience architecture

What must happen implicitly, whatever the shell.

### 1. Perceiving: messages arrive

- Each connected channel's new messages are gathered outside any session,
  so a channel works whether or not a session is open.
- Senders are gated before any model sees their text: known senders pass;
  unknown ones are held for me.
- What is gathered stays in the environment where it was gathered, never in
  a git store.

### 2. Rhythm: jobs run

- A job runs on a schedule (a sweep every morning) or on a trigger (a new
  message in a swept channel), as a plain launch of a shell that carries
  the same ghost.
- Only one copy of a job runs at a time; a missed run catches up once.
- A job that finishes its outcome is a session done: it sleeps like any
  other.

### 3. Attention: one inbox

- Every item, from any channel or from the harness (decisions waiting, a
  job that failed, auth about to expire), has the same shape: where it came
  from, who, the thread, when, what it asks, its state.
- The sweep reads the channels, groups and ranks what needs me, and drafts
  what it can; it never sends.
- The genome's "what is pending" reads from this inbox.
- It feels like an inbox: a list, one key to open, archive, snooze, act; it
  is there in the shell and beside it.

### 4. Acting, with guards

- The ghost drafts; sending happens only in a session I am in, after my yes.
- Nothing from a message becomes a habit without my yes; adaptation treats
  channel content like tool content.
- A channel's credentials never enter a store, a log or the context.

## Macro test

In a scratch HOME with the personal part installed and two test channels
(a local mail account and a test chat bot):

1. Ingest gathers a new mail and a new chat message into the inbox, with
   no session open; an unknown sender's message is held.
2. A scheduled sweep job runs, ranks the items, drafts one reply, and the
   inbox shows the draft as waiting for me; nothing is sent.
3. A triggered sweep runs once when a message arrives, and only once when
   two arrive together.
4. In an attended session, I accept the draft and it is sent; the inbox
   item closes.
5. The genome of a fresh session shows the inbox count.

## Constraints

- **Embedded, not wrapped.** Ingest is a small service outside any session;
  jobs are plain shell launches through the shell's own CLI; no daemon runs
  the agent loop.
- **The personal persona only.** Channels connect personal accounts. They
  run in an environment the person designates for the personal persona;
  the design assumes no particular machine and never a work one.
- **Credentials and mirrored data stay in the environment**, behind config,
  never in git; the inbox store is machine-local.
- **Draft by default; send only attended**, after the person's yes.
- **Unofficial connectors are opt-in**, with their risk stated (a
  messaging app that forbids automated clients can suspend the account).
- **Jobs carry the ghost explicitly** (plugins, MCP and settings named on
  the command line), so a change in a shell's defaults cannot drop it.
- **Public repo:** no employer, internal tool or private store names.
- **Claude Code first;** other shells mapped.

## Conceptual design

### Technical concepts

| Concept | What it is, and why | Serves |
|---|---|---|
| **channel** | a turn bring-in (its read, search and draft tools) plus an ingest service, so reading happens in sessions and gathering happens outside them | perception |
| **ingest service** | a small process per channel that fetches new messages into the mirror and the inbox, no model involved, so channels work without a session | perception |
| **sender gate** | the allowlist each channel applies before any text reaches a model, so a stranger's message cannot steer the ghost | perception |
| **cursor** | where ingest last read, per channel, so nothing is missed or read twice | perception |
| **mirror** | the machine-local copy of what was gathered, so jobs and sessions read without asking the account again | perception |
| **job** | a named run of a shell with the ghost, an outcome, a schedule or trigger, and a permission profile | rhythm |
| **launch** (carrier kind) | the core starts a shell through its CLI (a timer or a path watch), so the ghost arrives through the usual hooks | rhythm |
| **trigger** | a schedule, a file or path change, or an ingest event, debounced, so a burst starts one job | rhythm |
| **inbox store** | machine-local list of items, one schema across sources, so attention has one place | attention |
| **item** | source, sender, thread, time, ask, priority, state (new, seen, waiting, snoozed, done), and a draft if any | attention |
| **decision item** | adaptation's decision batch as one inbox item, so the person's decisions live with everything else | attention, adaptation |
| **inbox CLI and TUI** | the same store read as a command, a status-line count, a skill, and a full-screen list in a pane | attention |
| **attended send** | a send tool usable only in a session the person is in, after their yes | guards |

### Contract additions

A **channel** bring-in also declares: its `ingest` command, its `cursor`
and `mirror` config keys, its sender `gate` config key, and that its send
tool is `attended` only. A **job** is declared by the part that wants it:
`outcome`, `schedule` or `trigger`, `permissions`, `budget`. The core
renders jobs into the environment's scheduler at install.

### How it is assembled

```
 CHANNELS (outside any session)
   ingest per channel --gate--> mirror (local) --> inbox items
                                                      ^
 HARNESS  decisions, failed jobs, auth expiring -------+
                                                      |
 JOBS  timer / trigger --launch--> shell + ghost (genome, expression)
         sweep: read mirror -> group, rank, draft --> inbox
         sleep: experience + adaptation --> decision item --> inbox
                                                      |
 ATTENTION  inbox CLI | status-line count | /inbox skill | TUI pane
                                                      |
 ACT  attended session: open item -> accept draft -> attended send
```

## System design: mapping per shell

| Need | Claude Code | codex | kiro-cli | frameworks |
|---|---|---|---|---|
| job launch | a user timer or path unit running the CLI non-interactively with plugins, MCP and settings named | `codex exec` | non-interactive chat | the app's own scheduler |
| live arrival in an open session | a plugin monitor (interactive only); channels where the shell's own login allows | *(test)* | *(test)* | middleware reading the inbox |
| per-session loop | `/loop` and session cron (short-lived) | - | - | - |
| inbox in the shell | status-line count; `/inbox` skill | skill | skill | tool |

Connectors (choose per channel; research notes in the design review):

| Channel | Local-first option | Notes |
|---|---|---|
| mail (Google, others) | a CLI or MCP server over the provider's API or IMAP/JMAP | OAuth app in testing mode expires tokens weekly |
| calendar | the same provider tools | |
| chat with a bot API | the official bot API | supported, no ban risk |
| encrypted messenger | a linked-device CLI | copyleft licence; must stay current |
| a messenger without an official client API | linked-device libraries | opt-in only: automated clients can be banned |
| messages on one desktop OS | that OS only | not portable |

## Streams

| # | Feature | Ships | Needs | Status |
|---|---|---|---|---|
| 1 | inbox-store | the item schema, the machine-local store, the inbox CLI, the genome's pending count reading it | harness-core | planned |
| 2 | jobs | the launch carrier, job declarations, the core rendering timers and path triggers, one-at-a-time and catch-up, the sweep job as the first job | 1 | planned |
| 3 | channel-contract | the channel bring-in contract, the ingest service shape, sender gate, cursor, mirror, attended send | 1 | planned |
| 4 | first-channels | mail and calendar for the personal persona, then a bot-API chat | 3 | planned |
| 5 | inbox-tui | status-line count, `/inbox` skill, the full-screen list in a pane beside the shell | 1 | planned |
| 6 | fold-back | README and project.md: perception, rhythm and attention in the ghost's framing | 1-5 | planned |

## Open questions

1. Which environment runs the personal persona's channels and jobs (and
   does it have a login that allows the shell's own channels)?
2. Which channels first: mail and calendar, then which messenger?
3. May the ghost ever send without an attended session (for example to
   decline a meeting), or only draft?
4. One inbox across personas, or the personal persona only?
5. Sweep cadence and quiet hours; how long the mirror keeps messages.

## Decision Log

- Ruling (ringmaster, to ratify): channels are turn bring-ins plus an
  ingest service outside sessions; jobs are a new carrier kind, launch;
  the inbox is machine-local; draft by default, send attended · why:
  every personal-agent gateway that does channels owns the agent loop,
  which this design forbids; this split keeps the shell plain · cost if
  wrong: a gateway later, at the cost of the embedded rule
