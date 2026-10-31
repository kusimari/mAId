# Initiative: inbox

- Branch: `initiative/inbox` (cut from `main` at 0881e13)
- Status: draft for the person's review, at the framing and experience
  layer. The design layers (technical concepts, mapping per shell, streams)
  are written only once framing and experience are agreed.
- Research: channels, connectors, jobs and inbox UIs (kept outside the
  repo; drawn on when the design layers are written).

- **Build gate: do not build until initiative `harness-core` (PR #60) is
  merged to `main`.** Planned in parallel with it; the build session's first
  step is to rebase this branch onto `main` and confirm harness-core's
  entry points, contract and carriers are there. Before that, only this
  spec changes.

## Builds on harness-core

The ghost in the shell: every base harness is a shell; the ghost (my
harness) makes it act like me; one ghost, several personas fused, each
keeping its own memory. The ghost works through genome, expression,
experience, sleep and adaptation, as entry points reached by hooks,
framework adapters or on demand. Bring-ins are standard units with the
contract (scope, job, signals, rules, package, config, state, requires) in
their standard's slot; session bring-ins act for the whole session, turn
bring-ins attach when they fit; state lives only behind config. The core
ships with mAId; parts carry only bring-ins; no part depends on another.
This initiative adds to that, it changes none of it.

## Goal

My ghost keeps a rhythm and reflexes of its own. It senses through
whatever channels my parts bring, responds to what arrives and to the time
without being prompted each time, and gathers everything that needs me into
one inbox. I start it by asking a session to run it: "run my inbox", or
"run my rhythm and reflexes".

## Framing

So far the ghost acts only when I speak to it. An organism also has an
autonomic side: it senses, keeps a rhythm, reacts by reflex, and brings to
attention only what needs a decision. This initiative is that autonomic
side. It is about **how** sensing, rhythm, reflexes and attention work,
not about **which** senses there are: channels are the senses, and they are
brought by parts.

| Function | What it does, and why |
|---|---|
| **senses** (channels, brought by parts) | Each channel lets the ghost perceive one source (a mailbox, a calendar, a chat, a queue). Any part, personal or work, can bring one, so the ghost senses what each persona needs without this initiative knowing the channel. |
| **rhythm** (routines in time) | Runs routines on a cadence (a morning sweep, a sleep at the end of the day), so recurring work happens without me starting it each time. |
| **reflexes** (responses to what arrives) | Responds when a sense reports something new (sort it, rank it, prepare a response), involuntarily, so what arrives is handled before I look. |
| **attention** (the inbox) | Holds in one place everything that needs me, from every channel and from the ghost's own pending decisions, so I work one list instead of many. |

The four are one bring-in, the **inbox**, shipped with the core. The
session that runs it is a plain session in a base harness, carrying the
ghost: the inbox is something the ghost does, not something wrapped around
the shell.

## Experience architecture

What must happen, whatever the shell.

### 1. Starting it

- I launch a session and ask it to run my inbox (or my rhythm and
  reflexes); that session takes up the rhythm and reflexes and shows me the
  inbox.
- It knows which channels are present from the bring-ins the installed
  parts declare; a part with no channel adds none.
- Asking a running inbox to stop stops its rhythm and reflexes; the inbox
  itself stays.

### 2. Sensing

- Each channel reports what is new in its source in the same shape, so
  rhythm, reflexes and attention treat every channel alike.
- What a channel reports belongs to the persona of the part that brought
  it, and stays with that persona.
- How a channel is set up and authorised, and where what it gathers is
  kept, is the channel's own concern, outside the harness.

### 3. Rhythm and reflexes

- A routine runs on its cadence; a reflex runs when a sense reports
  something new. Either is the ghost at work: its genome at start, its
  expression while it runs.
- A routine or reflex that finishes its outcome is experience like any
  other: it sleeps, and adaptation learns from it under the same guards.
- A routine or reflex never runs twice over the same thing, and a burst of
  arrivals is handled as one.

### 4. Attention

- Every item, from any channel or from the ghost itself (decisions
  waiting, a routine that failed, a channel that needs me), has one shape:
  where it came from, the persona, who, the thread, when, what it asks, and
  its state.
- The genome's "what is pending" reads from the inbox.
- It looks and feels like an inbox: a list I scan, one key to open,
  archive, snooze or act; it is there in the shell and beside it.

### 5. Guards

- What arrives through a sense is untrusted, like tool output: it never
  becomes a habit or a rule without my yes, and never steers the ghost
  across personas.
- What the ghost may do on its own in response to an item, and what waits
  for me, is set by my guards, never by the content of what arrived.

## Macro test

In a scratch HOME with the core and two parts installed, each part
bringing one test channel:

1. Asking a session to run my inbox starts the rhythm and reflexes and
   shows an inbox listing both channels' items, each with its persona.
2. A new item on one channel triggers a reflex once; two arriving together
   trigger it once.
3. A routine on its cadence runs and its result appears in the inbox.
4. An item whose content asks the ghost to change a rule produces no rule
   change; it waits for me.
5. A fresh session's genome shows the inbox count; removing a part removes
   its channel and leaves the inbox working.

## Constraints

How this must be architected, given how the harness and its parts are
designed:

- **Embedded in a base harness.** The inbox runs inside a session of a
  base harness, carrying the ghost through the same entry points; it may be
  a session of its own. Nothing outside the shell runs the agent loop.
- **A core bring-in, channel-agnostic.** The inbox ships with the core and
  names no channel; it depends only on the channel contract.
- **Channels are bring-ins of parts.** Any part, personal or work, may
  bring channels with the bring-in contract; no part depends on another,
  and the core depends on no part.
- **Channel setup, credentials and gathered data live outside the
  harness**, in the environment or in the part's setup, reached only
  through config. How is designed later; nothing in the harness may assume
  it.
- **Personas stay separate.** An item keeps the persona of the part that
  brought its channel; memory and adaptation stay per persona.
- **Carrier-neutral.** Rhythm, reflexes and attention reach the ghost
  through the existing entry points; Claude Code first, other shells
  mapped.
- **Public repo, environment-independent:** names nothing private and
  assumes no particular machine.

## To design, once framing and experience are agreed

- The channel contract: what a channel declares and reports.
- How a running inbox keeps rhythm and reflexes inside a session, and
  whether they also run with no session open.
- Where channel setup, credentials and gathered data live, per
  environment.
- The inbox store, the item shape, and the inbox views in and beside the
  shell.
- The first channels, and which part brings each.
- Then the technical concepts, the mapping per shell, and the streams.

## Open questions

1. One inbox across personas with each item labelled (my recommendation),
   or one inbox per persona?
2. Is the inbox session the only place rhythm and reflexes run, or may
   they also run with no session open?
3. Do routines and reflexes come only from the inbox, or may parts declare
   their own?

## Decision Log

- Ruling (ringmaster, to ratify): channels are bring-ins any part brings;
  the inbox (senses, rhythm, reflexes, attention) is a core bring-in that
  names no channel and runs in a session of a base harness, which may be
  its own session started on request; channel setup and credentials live
  outside the harness, designed later · why: the person's review on PR #61
  separated the channels from the autonomic side and asked for framing and
  experience before design · cost if wrong: the channel contract reshapes
  when the design layer is written
- Superseded: the first draft's personal-persona-only channels, ingest
  service, draft-only sending and connector table; they assumed design
  that is now deferred.
