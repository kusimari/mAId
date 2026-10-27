# Initiative: harness-core

- Branch: `initiative/harness-core` (cut from `main` at 0881e13)
- Status: draft for discussion. The system design below grounds the
  initiative; Goal, Experience, Macro test and Constraints are drafts for
  the user to correct. Streams are not planned yet.
- Follows initiative `my-harness` (parts, part lists, whole-status, a
  memory server per persona), which is in review.

## Goal

Any base harness I launch plain becomes my agent, and gets better from
use. Installing bring-ins makes them available, but only a few extension
points put anything in the model's context at session start or on every
turn. A small **harness core** makes my harness present in every session
(a preamble and a session-start digest) and lets each session teach the
next (a reflection at session close that routes what was learned to where
it belongs), on any base harness, with no wrapper.

## Experience architecture

What must happen implicitly, at three points of a session, whatever the
base harness. This is behaviour the harness guarantees, not anything the
user sees or asks for. The conceptual design and its mapping to the
system design follow from it.

A **session** is what the user says it is: by default from a launch until
the agent is asked to close. Resumes and compactions belong to it;
branches (sub-agents, parallel work) are its sub-sessions; a feature file
or another persisted record can carry a session across launches.

### 1. At session start: the agent already knows

| It knows | Detail | Size |
|---|---|---|
| **my personas** | all of them, what each is for, and which one this session most likely serves (a guess the user can correct) | a few lines |
| **the harness bring-ins** | each one's job and where its state lives; memory is one of them (one per persona), reflection is another | one line each |
| **the task bring-ins** | a catalog only: name, when to use it, whether it keeps its own memory and where; none is loaded | one line each |
| **the environment** | which base harness, where it runs, what is reachable (stores, tools, network, auth), what it cannot do here | a few lines |
| **what is pending** | proposals and decisions waiting for the user | a count and a pointer |

This holds after a resume or a compaction, and every sub-session starts
with the part of it that its job needs. Everything larger (whole memories,
skill bodies, the harness map) stays behind a pointer.

### 2. On every agent call: minimal, current, task-aware

- **Reinforced, only when it changed:** the active persona and its store,
  the goal and next step of the work, and the few rules that must never
  drift. Kept small (about a paragraph), never the whole session-start
  picture again.
- **Task context, when a task is in play:** the task bring-in's key rules
  first, plus the entries of its own memory and of the persona's memory
  that bear on this task. Added when the task starts, dropped when it
  ends.
- **Recall at the point of need:** a memory that bears on what the agent is
  about to do reaches it then, not only if the agent thinks to search.
- **Never:** the same text from two parts, a catalog repeated on every
  call, or anything that changes the start of the context mid-session.

### 3. At close, on request, or in a sweep of sessions: reflection

- **Input:** the session with its sub-sessions: what the user said, what
  the agent was instructed (skills, prompts, standing text), what it did,
  what it touched, and where it hit friction (corrections, refusals,
  interruptions, prompts, retries).
- **The reflector** is separate from the session and knows how the harness
  is put together: a map of every part, bring-in, memory and store, with
  each one's source (a git project repo, a git store, or a machine
  directory) and how a change to it is made.
- **It learns from everything** and looks for what should change so the
  next session is better: persona memories, a bring-in's own memory (local
  tier first, then permanent), stores, harness and task bring-ins, and the
  harness loop itself, the reflector included. Removing something is a
  learning like any other.
- **Each finding** names the thing it changes, quotes its evidence, and
  goes where the map says: applied (a memory), filed as a backlog item, or
  worked in a worktree into a PR on the owning source.
- **The user decides** contradictions, guesses about which persona a
  learning belongs to, and every proposal, in one short batch.
- **It notices whether the harness is getting better:** fewer bring-in
  updates needed and the same corrections no longer repeated, more work
  right the first time; for memories, more done with less.

Later, not in this initiative: the same reflection runs as a job over
closed sessions, and other scheduled work (a daily sweep) runs the same
plain base harness with the same harness and memory.

> Constraints, Conceptual design and the mapping to the system design
> below are from the previous draft and will be reworked once this
> experience architecture is agreed.

## Macro test

Draft: in a scratch HOME with the personal part installed,

1. A fresh session's context contains the preamble and the digest (each
   persona's memory index, environment facts, pending proposals), read
   from the base harness's own startup report.
2. A session in which the user corrects the agent once and teaches a
   bring-in a rule, then closes: the reflection writes the fact to the
   right persona's memory, the rule to that bring-in's local tier, and
   files one skill-edit proposal as a backlog item; it applies nothing
   else.
3. The same reflection started by hand mid-session gives the same routing.
4. A second scratch HOME pulls the stores and its first session recalls
   the memory; the first HOME's environment facts are not there.

## Constraints

- Embedded, not wrapped: everything attaches through each base harness's
  own extension points. Where those differ (hook formats), a small
  per-harness adapter is acceptable; a launcher in front of the binary is
  not.
- Independent of any machine or environment: the design names no host,
  path layout, OS or organisation. Where something differs per
  environment, it is a value behind a configuration pointer; how a given
  environment lays out its files is decided later and separately.
- Public repo: no employer, internal tool or store names.
- Parts stay independent: the personal part (this repo) and any work part
  depend on harness-core as a standalone package, never on each other.
- Reflection proposes harness and skill changes; it never writes them.
- Claude Code first; other base harnesses next.

## Design

`(new)` marks what this initiative adds. `[cfg]` marks a bring-in that needs
a configuration pointer: a value set per environment or per persona, never
written into the bring-in itself.

### Two kinds of bring-in

- **Harness bring-ins** make any base harness mine and are present in
  every session: preamble, digest, memory, reflection, sync, status.
- **Task bring-ins** serve a kind of work and load when the work calls for
  them: coding (kdevkit, kyodakit, kreviewkit), writing (writing-style),
  notes, triage (a sweep), documents, web (browser), and whatever tools a
  part brings in from elsewhere.

Each part carries both kinds for its persona.

### Bring-in memory

A bring-in may keep its own memory (writing-style's learned rules, a
sweep's history). It declares it, so the harness can route to it, sync it
and show it:

- **local tier**: pending learnings, kept where the bring-in runs, not yet
  trusted;
- **permanent tier**: confirmed learnings, in a persona store or the
  owning repo, so they travel;
- a **promotion rule** from local to permanent, and a **pointer** to each
  tier `[cfg]`.

### System view

```
 SURFACES  how a session starts; each starts a PLAIN base harness
   terminal now  |  jobs and channels later (same plain start)
                                       |
+--------------------------------------v-------------------------------------------+
| BASE HARNESS   vendor model + agent loop      claude-code | codex | kiro | agy     |
|                                                                                    |
|  agent loop:  session start --> turns (reason -> act/tool)* --> close              |
|                    ^                ^                             ^                |
|  extension points: |                |                             |                |
|   always-loaded context ---- preamble (new)                       |                |
|   session-start hook ------- digest (new)                         |                |
|   MCP tools + instructions -------- memory, task MCPs             |                |
|   skills / sub-agents ------------- task bring-ins, reflection    |                |
|   session-close hook ------------------------------ reflection (new)               |
|   settings / permissions / plugins (install-time plumbing)                         |
+--------------------------------------^-------------------------------------------+
                                       |  injected ONLY through extension points
+--------------------------------------+-------------------------------------------+
| MY HARNESS                                                                         |
|                                                                                    |
|  HARNESS BRING-INS: present in every session                                       |
|   preamble     who I am, which stores, how this harness learns      context (new)  |
|   digest       memory indexes + environment facts + pending         hook    (new)  |
|   memory       one memory server per persona                [cfg]   MCP            |
|   reflection   at close, or on request: route what was learned       skill+hook(new)|
|   sync         pull at start, commit on write                [cfg]  hook    (new)  |
|   status       installed vs declared (part lists, --whole)          CLI            |
|                                                                                    |
|  TASK BRING-INS: loaded on demand; each may declare its own memory                 |
|   coding  kdevkit, kyodakit, kreviewkit        writing  writing-style  [cfg mem]   |
|   notes   notes [cfg vault]                    triage   a sweep        [cfg mem]   |
|   documents, web (browser [cfg]), and tools a part brings in from elsewhere        |
+------------------------------------------------------------------------------------+
        delivered by PARTS; each independent, each carries BOTH kinds
 +----------------------------------------+   +-----------------------------------------+
 | PERSONAL PART  (this repo)             |   | WORK PART  (another repo, if any)       |
 |  harness: personal preamble,           |   |  harness: work preamble,                |
 |           personal memory              |   |           work memory                   |
 |  task:    coding, writing, notes,      |   |  task:    its own skills, tools and     |
 |           web, documents               |   |           brought-in packages           |
 +-------------------+--------------------+   +--------------------+--------------------+
                     |  both depend on standalone public packages   |
                     +-----> memory server (one instance per store) <+
                     +-----> harness-core (preamble, digest, reflection, sync) (new) <+

 STATE: each owner declares its own; the pointer says where it lives
 +------------------------+----------------------------+---------+----------------------+
 | scope                  | home                       | travels | pointer              |
 +------------------------+----------------------------+---------+----------------------+
 | me                     | personal store             | yes     | personal part  [cfg] |
 | work profile           | work store                 | yes     | work part      [cfg] |
 | project                | the repo's own files       | repo    | the repo             |
 | environment            | environment store (new)    | never   | harness-core   [cfg] |
 | bring-in memory, local | where the bring-in runs    | no      | the bring-in   [cfg] |
 | bring-in memory, perm. | persona store or its repo  | yes     | the bring-in   [cfg] |
 | proposals              | PR or backlog on the owner | repo    | the owning part      |
 +------------------------+----------------------------+---------+----------------------+

 REFLECTION (new): at session close, or when I ask
   read the session -> classify each learning -> route by owner, gated by blast radius
     fact about me / my work ........... that persona's memory (written, one commit)
     a bring-in learned something ...... that bring-in's local tier (written);
                                         promotion to permanent per its rule
     skill edit, new skill ............. PR or backlog item on the owning part (proposed)
     harness design change ............. PR or backlog item on the owning part (proposed)
   then: a short report of what it wrote and what it proposed
   later: the same reflection as a job over closed sessions
```

## Open questions

These are the ones the design cannot settle by default.

1. **What is "a session" for reflection?** Sessions get resumed days
   later, compacted, branched into sub-agents, or run in parallel on one
   piece of work. Is the unit one process, one piece of work across
   processes, or a time window? This decides when "close" fires and what
   the reflection reads.
2. **Who reflects?** The session's own agent has the full context but is
   biased toward its own mistakes; a fresh reviewer with only the
   transcript is cleaner but misses what was implicit and costs more.
   Which one, or the first proposing and the second checking?
3. **What may reflection learn from?** Transcripts hold tool output and
   fetched content as well as my words. Learning only from what I said
   is safe but misses what the agent discovered; learning from everything
   risks keeping an injected instruction forever. Where is the line?
4. **When does a learning become true?** For persona memory and for each
   bring-in's promotion from local to permanent: once said, once
   confirmed, seen twice, or survived a week? One rule for all, or each
   owner decides? And when a new learning contradicts a kept one, which
   wins?
5. **How does the harness unlearn?** Stale facts, a correction I gave
   only for one task, a lesson that was wrong. Is forgetting another
   reflection outcome, a decay, or only by hand?
6. **A session that crossed personas.** Work and personal in one sitting,
   or a personal tool used for work. Split the learnings by item, ask
   once, or file them by where the session ran? The personal store may be
   public, so the cost of a wrong guess is uneven.
7. **PR or backlog?** A proposal can be a concrete change (a PR) or an
   idea (a backlog item). What decides which, and how does a pending
   proposal reach me again (the digest, a weekly list) so it does not
   rot?
8. **How do we know it is getting better?** A signal reflection records
   (the same correction repeated, prompts that recur, skills never used)
   so that improvement is measured, not assumed.
9. **How much should the harness know about itself?** For reflection to
   propose harness design changes, the agent needs a model of the harness
   (its parts, bring-ins and owners). How much of that belongs in the
   preamble every session, versus loaded only when reflecting?

## Streams

| # | Feature | Ships | Needs | Status |
|---|---|---|---|---|

## Decision Log

- Ruling (user, 2026-10-26): reflection is triggered at session close
  and can be started by hand; automating it as a job comes later, after
  doing it by hand shows how · why: design for the trigger that exists
  today, learn the automation from use · cost if wrong: none; the job
  reuses the same reflection
- Ruling (user, 2026-10-26): learnings route by owner: facts to the
  right persona's memory; a bring-in's learnings to its own memory, local
  tier first, then permanent; skill and harness changes only as PRs or
  backlog items on the owning part · why: each owner keeps its own state
  and its own review · cost if wrong: none known
- Ruling (user, 2026-10-26): the design is independent of any machine or
  environment; file layout per environment is decided later · why: it
  will run in different kinds of environments · cost if wrong: none
- Ruling (user, 2026-10-27, PR review): the design reads as three layers
  of one architecture: the experience architecture (what must happen
  implicitly at session start, on every agent call, and at close), then
  the conceptual design, then its mapping to the system design. The
  "Experience" is implicit behaviour, not what the user sees.
- Ruling (user, 2026-10-27): answers to the open questions:
  - a session is what the user says it is: by default a launch until the
    agent is asked to close; resumes, compactions and branches (which make
    sub-sessions) belong to it; a feature file or other persisted record
    can continue one;
  - a separate reflector that knows how the harness is designed is given
    the session;
  - reflection learns from everything (my words, how the agent read them,
    the words of the skills and prompts); the aim is to find what in the
    harness, bring-ins, memories or stores to change so the next session
    is better;
  - contradictions and cross-persona guesses go to the user to decide,
    for now; automating later;
  - unlearning is learning that something should be removed;
  - a simple map of how the harness is put together lets the reflector
    know each thing's source (a git project repo, a git store, or a
    machine directory) and say "file this as a backlog item" or "work it
    in a worktree into a PR", without spelling out how;
  - getting better: fewer bring-in updates needed and higher accuracy of
    work; for memories, productivity going up;
  - the harness knows as much about itself as reflection needs.
