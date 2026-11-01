# Initiative: os-mode (home)

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
reflexes, attention) as a session I start with "run my inbox". This
initiative adds to that, it changes none of it.

## Goal

My ghost can be in many shells at once and keep them together. I start one
session with "run my home"; from it I ask for work, each piece of work gets
a shell of its own carrying the same ghost, and I can go to any of them and
come back home to see what is running and how it is going, with the shells
keeping each other informed.

## Framing

The inbox is the ghost's autonomic side. This is its deliberate side: one
ghost choosing to put itself into several shells, each focused on one
outcome, and coordinating them from one place. The name `os-mode` stays for
the branch; the capability is the **home**, so that "run my home" sits
beside "run my inbox".

| Function | What it does, and why |
|---|---|
| **home** (where the ghost returns) | One session that knows every shell the ghost is in, and the standing ones it keeps up (the inbox), so I always have one place to start from and come back to. |
| **budding** (a shell for one outcome) | The ghost buds a new shell for one piece of work, carrying the same ghost and a handover to the work's own record, so focused work never clutters the home and each bud has its own experience and its own sleep. |
| **nerves** (impulses between shells) | Carry progress, results, "I need you" and "stop" between the home and its buds, so they work together without me carrying messages. Nerves carry information, never authority. |
| **presence** (where I meet a shell) | How I meet any shell: a window, a page, a chat thread. Separate from where the shell runs, so how I reach my shells can change without changing them. |

Why these names: a bud is the same organism grown out for a purpose, which
is what a job session is (same genome, its own experience); nerves pass
signals and nothing else; presence is about meeting, not running.

## Experience architecture

What must happen, whatever the shell.

### 1. Starting home

- I launch a session and ask it to run my home; it shows me the shells it
  knows, as they actually are (running, waiting for me, done, failed), and
  resumes nothing without my yes.
- What started the home does not matter to it: today I start it; later it
  may be started for me.
- The home can keep standing shells up, such as the inbox session, and
  start them again if they stop.

### 2. Budding

- I ask the home for a piece of work; it buds a shell carrying the same
  ghost, with a handover pointing at the work's own record (a feature spec,
  a task) and the outcome, and with the guards chosen for that work; it
  tells me where to meet it.
- The bud starts from the record, not from the home's conversation: its
  genome at start, its expression while it works.

### 3. Presence

- I can meet any shell, through whichever presence is available, and come
  back home.
- The home shows, for each bud: its outcome, its state, its latest
  progress, and where to meet it.

### 4. Nerves

- A bud reports progress and results to the home; when it needs me, that
  reaches my attention (an inbox item) as well as the home.
- The home can ask a bud to stop.

### 5. Guards

- An impulse can never grant a permission or approve an action; a bud
  never asks another shell to do what it may not do itself.
- What a bud reports is untrusted to the home, like tool output.
- Each bud sleeps and adapts on its own, under the same guards; the home
  sleeps and adapts on its own.

## Macro test

In a scratch HOME with the core installed:

1. Asking a session to run my home shows an empty table of shells.
2. Asking for a piece of work buds a new shell whose genome shows the
   handover; the table lists it as running, with where to meet it.
3. The bud reports progress; the home shows it; asking the home to stop
   the bud stops it.
4. A bud asking the home to approve something it was denied is refused.
5. Restarting the home shows the shells as they are and asks before
   resuming anything; the inbox session, if kept as standing, is started
   again.

## Constraints

How this must be architected, given how the harness and its parts are
designed:

- **Embedded in a base harness.** The home is a session of a base harness
  carrying the ghost; every bud is a plain session of a base harness
  carrying the same ghost. Nothing outside the shells runs the agent loop.
- **A core bring-in.** The home ships with the core; it names no part, no
  channel and no particular work tracker.
- **Start-agnostic.** Nothing may assume who started the home or a bud,
  so starting the home without me (at login, or with no harness started
  yet) can come later without changing how it works.
- **Running separate from presence.** A shell's run never depends on how I
  am meeting it; presences can be added without changing the shells.
- **Shell-agnostic.** Nothing may assume a bud runs in the same base
  harness as the home.
- **Nerves carry no authority.** Guards come from the person and the
  harness, never from an impulse.
- **One source of truth.** What the home shows comes from the shells as
  they are; it never keeps a second account that can drift.
- **Personas as in harness-core.** A bud carries the whole fused ghost;
  what it remembers goes to the persona it belongs to.
- **Carrier-neutral, Claude Code first;** public repo, names nothing
  private; environment-independent.

## To design

- The handover: what a bud receives and how it points at the work's record.
- How the home knows the shells, from each shell's own records.
- How a bud's guards are chosen at budding.
- The nerves: how impulses travel, per shell.
- The presences: which first, and which later.
- Standing shells: how the home keeps the inbox session up.
- Then the technical concepts, the mapping per shell, and the streams.

## Open questions

1. May a bud bud (a bud starting its own shells), or only the home?
2. Bounds: how many buds at once, a budget per bud, when idle buds are
   ended.
3. May a reflex from the inbox bud a shell on its own, or only propose
   one in the inbox?
4. Which presences matter after the first: a page, a chat thread, a
   phone?

## Decision Log

- Ruling (ringmaster, to ratify): the capability is named **home**, with
  budding, nerves and presence; the branch keeps `os-mode` · why: "run my
  home" pairs with "run my inbox", and the organism names say what each
  part does · cost if wrong: renaming in the spec only
- Ruling (ringmaster, to ratify): framing and experience first, as for the
  inbox; the first draft's tmux-first presence, depth-1 spawning, caps and
  per-shell mapping move to design or open questions · why: they assumed
  design · cost if wrong: none, they return at the design layer
