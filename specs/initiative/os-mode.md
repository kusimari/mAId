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
session, carrying the same ghost, is started for a piece of work. The
work may come with an outcome (an inbox item that needs doing) or only
with what it is about ("work on this code"), the outcome taking shape in
the spawn as I instruct it; either way it may run for a long time, and I
can see from os-mode whether the outcome was achieved. Spawns run
independently but stay linked; when one needs me it signals me to pay
attention. The inbox can start spawns too: an item that is
best worked in a session of its own (a ticket assigned to me) signals
os-mode to spawn one to achieve it, and that spawn either finishes and
updates the item or signals me. The
inbox session itself is one of the things I launch this way.

## Framing

The inbox is the ghost's autonomic side. This is its deliberate side: the
ghost shelling out into sessions of its own for pieces of work, and
keeping track of all of them from one place. For each spawn os-mode knows
what it is about, its outcome when there is one, who started it, its
state, whether the outcome was achieved, and when it needs me.

| Function | What it does, and why |
|---|---|
| **spawning** (shelling out for a piece of work) | Starts a new session carrying the same ghost, with what the work is about, its outcome when there is one, and a pointer to where it came from (my ask, an inbox item), so the work runs on its own, with its own experience and its own sleep. |
| **working in a spawn** | I instruct the spawn in its own session, for as long as the work takes, so os-mode never stands between me and the work. |
| **signalling** (pay attention) | A spawn that needs input signals me: it shows as waiting in os-mode and as an item in my inbox, so I learn of it wherever I am. |
| **finishing** | A spawn whose work is done says so, with its result where it came from (the inbox item it started from), and ends, so every spawn closes what it opened. |
| **spawning from the inbox** | An inbox item that needs a session of its own signals os-mode to spawn one to achieve it, under my guards, so work that arrives can start before I look. |
| **standing spawns** | Spawns that keep running rather than finish, such as the inbox session, so os-mode is how the ghost's long-running sessions are launched too. |
| **links** (information between sessions) | Open between os-mode and each spawn, and possibly between spawns, carrying state, signals, results and "stop". Links carry information, never authority. |
| **merging** (one piece of work, one spawn) | Notices when a launch is about work a spawn already has, tells me, and offers to stop the launch or merge, so duplicates are deliberate, never accidental. |
| **presence** (where I meet a spawn) | How I meet any spawn: a window, a page, a chat thread. Separate from where it runs, so how I reach spawns can change without changing them. |

"Links" rather than "channels": in the inbox, channels are the ghost's
senses of outside sources; links are between the ghost's own sessions.

## Experience architecture

What must happen, whatever the shell.

### 1. Managing my spawns

- I ask a session to manage my spawns; it shows me every spawn as it
  actually is (working, waiting for me, done, failed), what it is about,
  its outcome and whether it was achieved when there is one, who started
  it, and where to meet it; it resumes nothing without my yes.
- What started os-mode does not matter to it: today I start it; later it
  may be started for me.

### 2. Launching the inbox

- From os-mode I launch "run my inbox" as a standing spawn; os-mode keeps
  it up and offers to start it again if it stops.

### 3. Spawning and working

- I say what I want to work on; os-mode spawns a session carrying the same
  ghost, with what it is about and where it came from, and the guards for
  that work, and tells me where to meet it.
- I go to the spawn and instruct it there; it runs as long as the work
  takes. Its genome at start, its expression while it works.

### 4. Spawning from the inbox

- An inbox item best worked in a session of its own (a ticket assigned to
  me) signals os-mode, which spawns a session to achieve it, carrying the
  item's ask as the outcome, if my guards allow; otherwise the spawn waits
  in the inbox as a proposal.
- That spawn either finishes, updating the inbox item it came from, or
  signals me.

### 5. Signalling and finishing

- A spawn that needs input signals me: it shows as waiting for me in
  os-mode and raises an item in my inbox; meeting it takes me straight to
  the question.
- A spawn whose work is done reports its result where it came from and
  ends; what it learned stays in its own experience and sleep.
- os-mode can ask a spawn to stop.

### 6. Merging

- When a launch, mine or the inbox's, is about work a spawn already has
  (the same ticket, the same code), os-mode tells me which spawn and
  offers: launch anyway, stop the launch, or merge.
- Merging: the new launch's ask is sent to the running spawn; once that
  spawn acknowledges it and acts on it, the duplicate shuts itself down.
- Two spawns for the same work remain possible when I choose it.

### 7. Presence

- I can meet any spawn, through whichever presence is available, and come
  back to os-mode.

### 8. Guards

- A link can never grant a permission or approve an action; a spawn never
  asks another session to do what it may not do itself.
- What a spawn reports, or sends in a merge, is untrusted to its
  receiver, like tool output.
- What the inbox may spawn on its own, and what it only proposes, is set
  by my guards, never by the content that arrived.
- Each spawn sleeps and adapts on its own, under the same guards.

## Macro test

In a scratch HOME with the core installed and one test channel:

1. Asking a session to manage my spawns shows none.
2. Launching "run my inbox" from it starts the inbox as a standing spawn.
3. Asking to work on some code spawns a session, listed as working with
   what it is about and where to meet it; instructing it there works.
4. The spawn signals that it needs input: os-mode shows it waiting and the
   inbox has an item; meeting it shows the question.
5. A test item arriving on the channel that needs its own session
   signals os-mode, which spawns one with the item's ask as its outcome;
   the spawn finishes, os-mode shows the outcome achieved, and the item
   is updated.
6. Launching again for the same work is noticed; choosing merge sends the
   ask to the running spawn, which acknowledges and acts, and the
   duplicate shuts down.
7. A spawn asking os-mode to approve something it was denied is refused.

## Constraints

How this must be architected, given how the harness and its parts are
designed:

- **Embedded in a base harness.** os-mode is a session of a base harness
  carrying the ghost; every spawn is a plain session of a base harness
  carrying the same ghost, which I can work in directly. Nothing outside
  the shells runs the agent loop.
- **A core bring-in.** os-mode ships with the core; it names no part, no
  channel and no particular work tracker.
- **Outcome optional, never required.** A spawn may start with an outcome
  or without one; os-mode tracks an outcome and whether it was achieved
  when there is one, and works the same when there is none.
- **Start-agnostic.** Nothing may assume who started os-mode or a spawn
  (me, the inbox, later something else).
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
- **The inbox is used, not assumed.** os-mode launches, raises to and
  spawns from the inbox when it is there; it still works without it.
- **Personas as in harness-core.** A spawn carries the whole fused ghost;
  what it remembers goes to the persona it belongs to.
- **Carrier-neutral, Claude Code first;** public repo, names nothing
  private; environment-independent.

## To design

- What a spawn starts with: what it is about and where it came from.
- How os-mode knows its spawns and their state, from each shell's own
  records.
- How a spawn's guards are chosen at launch, by me or by the inbox.
- Signalling: how a spawn says it needs me, and how that reaches the
  inbox.
- Links: how messages travel, per shell; whether and how spawns link to
  each other.
- Same-work detection: how os-mode tells two launches are about the same
  work.
- Merge: the hand-off, the acknowledgement, and the shutdown.
- Standing spawns: keeping the inbox up.
- Presences: which first, and which later.
- Then the technical concepts, the mapping per shell, and the streams.

## Open questions

1. Links between spawns: needed, and when?
2. May a spawn spawn, or only os-mode and the inbox?
3. Bounds: how many spawns at once, a budget per spawn, when idle spawns
   are ended.
4. Which presences matter after the first: a page, a chat thread, a
   phone?

## Decision Log

- Ruling (person): the framing is spawning and managing spawns, not a
  home; a spawn shells out, achieves an outcome and returns; os-mode
  launches the inbox; spawns raise to the inbox; same-outcome launches are
  noticed and can be stopped or merged (the ask passed on, acknowledged
  and acted on, then the duplicate shuts down) · why: the person's review
  on PR #62
- Ruling (person): a spawn is started for a piece of work, not an
  outcome; I instruct it in its own session and it may run long; it
  signals me (in os-mode and the inbox) when it needs input; the inbox's
  reflexes may spawn sessions for work they find, which finish into the
  inbox or signal me; os-mode may know a spawn's outcome and whether it was
  achieved, and an inbox item signals os-mode to spawn for it · why: the
  person's second and third reviews on PR #62 · cost if
  wrong: none, os-mode only knows less
- Ruling (ringmaster, to ratify): "links" for information between
  sessions, so it does not collide with the inbox's channels · cost if
  wrong: a rename in the spec
- Superseded: the first draft's tmux-first binding, depth-1 spawning,
  caps and per-shell mapping move to design or open questions; the
  "home" naming of the second draft.
