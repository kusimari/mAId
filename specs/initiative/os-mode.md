# Initiative: os-mode

- Branch: `initiative/os-mode` (cut from `main` at 0881e13)
- Status: draft for the person's review, at the framing and experience
  layer. The design layers (technical concepts, mapping per shell, streams)
  are written only once framing and experience are agreed.
- Research: session orchestration and run/binding separation (kept outside
  the repo; drawn on when the design layers are written).

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
contract in their standard's slot; state lives only behind config. The
core ships with mAId; parts carry only bring-ins; no part depends on
another. Initiative `inbox` adds the autonomic side (senses, rhythm,
reflexes, attention) as a session started with "run my inbox". This
initiative adds to that, it changes none of it.

## Goal

From one session I spawn and manage my spawns. Spawning shells out: a new
session, carrying the same ghost, goes off to achieve one outcome and
comes back with it. Spawns run independently but stay linked, so they
keep me and each other informed, and raise to my inbox what needs me. The
inbox session itself is one of the things I launch this way.

## Framing

The inbox is the ghost's autonomic side. This is its deliberate side: the
ghost choosing to shell out, sending a spawn of itself after an outcome,
and keeping track of all of them from one place.

| Function | What it does, and why |
|---|---|
| **spawning** (shelling out for an outcome) | Starts a new session for one outcome, carrying the same ghost and a handover to the work, so focused work runs on its own, with its own experience and its own sleep. |
| **return** (coming back with the outcome) | A spawn that achieves its outcome reports it back and ends, so every spawn closes the loop it opened. |
| **standing spawns** (launched and kept up) | Spawns that keep running rather than return, such as the inbox session, so os-mode is how the ghost's long-running sessions are launched too. |
| **links** (information between sessions) | Open between os-mode and each spawn, and possibly between spawns, carrying progress, questions, results and "stop", so they work together without me carrying messages. Links carry information, never authority. |
| **raising** (to my attention) | A spawn that needs me says so to the inbox, so I find it with everything else that needs me, wherever I am. |
| **merging** (one outcome, one spawn) | Notices when a launch is after an outcome a spawn already pursues, tells me, and offers to stop the launch or merge, so duplicates are deliberate, never accidental. |
| **presence** (where I meet a spawn) | How I meet any spawn: a window, a page, a chat thread. Separate from where it runs, so how I reach spawns can change without changing them. |

"Links" rather than "channels": in the inbox, channels are the ghost's
senses of outside sources; links are between the ghost's own sessions.

## Experience architecture

What must happen, whatever the shell.

### 1. Managing my spawns

- I ask a session to manage my spawns; it shows me every spawn as it
  actually is (running, waiting for me, returned, failed) and resumes
  nothing without my yes.
- What started os-mode does not matter to it: today I start it; later it
  may be started for me.

### 2. Launching the inbox

- From os-mode I launch "run my inbox" as a standing spawn; os-mode keeps
  it up and offers to start it again if it stops.

### 3. Spawning

- I ask for an outcome; os-mode spawns a session carrying the same ghost,
  with a handover pointing at the work's own record (a feature spec, a
  task) and the outcome, and with the guards chosen for that work; it tells
  me where to meet it.
- The spawn starts from the record, not from os-mode's conversation: its
  genome at start, its expression while it works.

### 4. Merging

- When I launch for an outcome a spawn already pursues, os-mode tells me
  which spawn and offers: launch anyway, stop the launch, or merge.
- Merging: the new launch's ask is sent to the running spawn; once that
  spawn acknowledges it and acts on it, the duplicate shuts itself down.
- Two spawns for the same outcome remain possible when I choose it.

### 5. Links and raising

- Each spawn reports progress and results over its link; os-mode shows,
  for each: its outcome, its state, its latest progress, and where to meet
  it. os-mode can ask a spawn to stop.
- A spawn that needs me raises it to the inbox.
- On return, the spawn reports its outcome and ends; what it learned stays
  in its own experience and sleep.

### 6. Presence

- I can meet any spawn, through whichever presence is available, and come
  back to os-mode.

### 7. Guards

- A link can never grant a permission or approve an action; a spawn never
  asks another session to do what it may not do itself.
- What a spawn reports, or sends in a merge, is untrusted to its
  receiver, like tool output.
- Each spawn sleeps and adapts on its own, under the same guards.

## Macro test

In a scratch HOME with the core installed:

1. Asking a session to manage my spawns shows none.
2. Launching "run my inbox" from it starts the inbox as a standing spawn.
3. Spawning for an outcome starts a session whose genome shows the
   handover; it is listed as running, with where to meet it.
4. The spawn reports progress, raises one item that shows in the inbox,
   then returns its outcome and ends.
5. Spawning again for an outcome already running is noticed; choosing
   merge sends the ask to the running spawn, which acknowledges and acts,
   and the duplicate shuts down.
6. A spawn asking os-mode to approve something it was denied is refused.

## Constraints

How this must be architected, given how the harness and its parts are
designed:

- **Embedded in a base harness.** os-mode is a session of a base harness
  carrying the ghost; every spawn is a plain session of a base harness
  carrying the same ghost. Nothing outside the shells runs the agent loop.
- **A core bring-in.** os-mode ships with the core; it names no part, no
  channel and no particular work tracker.
- **Start-agnostic.** Nothing may assume who started os-mode or a spawn,
  so starting it without me can come later without changing how it works.
- **Spawns are independent.** A spawn keeps working if os-mode stops;
  links inform, they do not hold spawns together.
- **Links open to more than os-mode.** Designed so links between spawns
  can be added without changing spawns.
- **Running separate from presence.** A spawn's run never depends on how I
  am meeting it.
- **Shell-agnostic.** Nothing may assume a spawn runs in the same base
  harness as os-mode.
- **Links carry no authority.** Guards come from the person and the
  harness, never from a message.
- **One source of truth.** What os-mode shows comes from the spawns as
  they are; it never keeps a second account that can drift.
- **The inbox is used, not assumed.** os-mode launches and raises to the
  inbox when it is there; it still works without it.
- **Personas as in harness-core.** A spawn carries the whole fused ghost;
  what it remembers goes to the persona it belongs to.
- **Carrier-neutral, Claude Code first;** public repo, names nothing
  private; environment-independent.

## To design

- The handover: what a spawn receives and how it points at the work.
- How os-mode knows its spawns, from each shell's own records.
- How a spawn's guards are chosen at launch.
- Links: how messages travel, per shell; whether and how spawns link to
  each other.
- Same-outcome detection: how os-mode tells two launches are after the
  same outcome.
- Merge: the hand-off, the acknowledgement, and the shutdown.
- Standing spawns: keeping the inbox up.
- Presences: which first, and which later.
- Then the technical concepts, the mapping per shell, and the streams.

## Open questions

1. Links between spawns: needed, and when?
2. May a spawn spawn, or only os-mode?
3. Bounds: how many spawns at once, a budget per spawn, when idle spawns
   are ended.
4. May an inbox reflex launch a spawn on its own, or only propose one in
   the inbox?
5. Which presences matter after the first: a page, a chat thread, a
   phone?

## Decision Log

- Ruling (person): the framing is spawning and managing spawns, not a
  home; a spawn shells out, achieves an outcome and returns; os-mode
  launches the inbox; spawns raise to the inbox; same-outcome launches are
  noticed and can be stopped or merged (the ask passed on, acknowledged
  and acted on, then the duplicate shuts down) · why: the person's review
  on PR #62
- Ruling (ringmaster, to ratify): "links" for information between
  sessions, so it does not collide with the inbox's channels · cost if
  wrong: a rename in the spec
- Superseded: the first draft's tmux-first binding, depth-1 spawning,
  caps and per-shell mapping move to design or open questions; the
  "home" naming of the second draft.
