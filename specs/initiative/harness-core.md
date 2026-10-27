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
  the goal and next step of the work (taken from the work's own record),
  and the few rules that must never drift. Kept small (about a paragraph), never the whole session-start
  picture again.
- **Mode context, when the work is focused:** the mode is not declared by
  anyone; it is what the harness works out from the outcome being pursued,
  the location or project the agent is in, and the bring-ins that apply
  there (coding with kdevkit, a review briefing, a sweep, or the agent's
  native way of doing a job no bring-in covers). While it holds, the key
  rules of the bring-ins in play come first, plus the entries of their own
  memory and of the persona's memory that bear on it. It changes as the
  outcome, the location or the bring-ins in play change.
- **The goal and next step come from the work itself** (a feature spec, a
  task list, the base harness's own plan); the harness does not keep a
  second copy.
- **Recall at the point of need:** a few memories that bear on what the
  agent is about to do reach it then, not only if the agent thinks to
  search. A small cap keeps the cost low; the selection starts as a cheap
  lookup and moves, over time, to a local decision model so it needs no
  outside call.
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

- **Embedded, not wrapped.** Everything attaches through each base
  harness's own extension points. A small per-harness adapter is fine
  where those differ (hook formats); a launcher in front of the binary is
  not.
- **Independent of any environment.** The design names no host, path
  layout, OS or organisation. What differs per environment is a value
  behind a configuration pointer; each environment's file layout is
  decided later and separately.
- **Public repo:** no employer, internal tool or store names.
- **Parts stay independent:** each part depends on standalone packages
  (the memory server, harness-core), never on another part.
- **Small and stable context.** The session picture is loaded once per
  session (and again only when the base harness drops it, as after a
  compaction); the per-call reinforcement is sent only when it changed.
  Whether a base harness instead needs a session-plus-call preamble on
  every call, or offers a context that is kept for every call, is an
  implementation question per base harness: use its kept context if it has
  one, repeat only if it must. Mode context is bounded per mode; anything
  larger stays behind a pointer. Nothing rewrites the start of the context
  mid-session, and each item has exactly one carrier, so no two parts
  inject the same text.
- **One record of the work.** Goal, next step and session continuity come
  from the work's own records; the harness keeps no parallel copy.
- **Reflection writes memories, proposes everything else.** It applies
  facts to the right memory and a bring-in's learnings to that bring-in's
  local tier; skill, bring-in and harness changes are only filed (backlog
  or worktree to PR) on the owning source. Contradictions, persona guesses
  and proposals wait for the user.
- **Evidence or nothing.** Every finding quotes the session; content that
  came only from tool or web output cannot become a memory without the
  user's yes; a learning the reflector cannot place in a persona goes to
  the private store or to the user, never the public one.
- **Recall without stepping out, eventually.** Selection may start with an
  outside model call under a small cap, but its interface must let a
  local decision model replace it.
- **Claude Code first;** other base harnesses next.

## Conceptual design

### Concepts

| Concept | What it is |
|---|---|
| **base harness** | a vendor's agent loop; reached only through its extension points |
| **carrier** | one extension point used to place one item: always-loaded context, a session-start or per-prompt hook, MCP instructions and tools, a skill, a sub-agent definition |
| **part** | what one repo delivers for one persona: harness bring-ins and task bring-ins |
| **bring-in** | a unit a part installs (a skill, an MCP server, a sub-agent, a hook, a plugin). It declares what the harness needs to know about it (the bring-in contract, below); the part records where it came from |
| **persona** | who the work is for; owns a store and the memory in it |
| **store** | a git checkout the user owns; holds persona memory, permanent bring-in memory, and the user's own files |
| **harness map** | the union of every part's declarations: the harness's model of itself, generated at install |
| **session** | what the user says it is; has sub-sessions; continued by the work's own records |
| **mode** | derived, never declared: the outcome being pursued, the location or project context, and the bring-ins that apply there. The harness resolves it; no bring-in names a mode. Selects the mode context |
| **context layers** | the session picture, the per-call reinforcement, the mode context, and the on-demand pointers |
| **selector** | picks the few memories that bear on the next call; a cheap lookup first, a local decision model later |
| **reflection** | packer (session to bundle) -> reflector (bundle + map) -> findings -> user decisions -> apply or file -> ledger |
| **ledger** | what reflection records across sessions to tell whether the harness is getting better |

### The bring-in contract

A bring-in stays a plain unit of an existing standard (an Agent Skill, an
MCP server, a plugin, a sub-agent), so anyone can use it without this
harness. The contract adds only what this design needs, carried inside
the standard's own extension slot, and a base harness that does not know
it simply ignores it.

**Declared by the bring-in** (travels with it, wherever it is vended):

| Field | What it says | Needed for |
|---|---|---|
| `kind` | harness or task | session start: which list it belongs to |
| `job` | one line: what it is for | session start: the catalog line |
| `signals` | optional hints that it applies: locations or file patterns, outcomes it serves; without them the harness uses `job` and the standard's own description | every call: resolving which bring-ins apply |
| `rules` | a pointer to the few lines that come first while it is in play | every call: mode context |
| `state` | none, or its tiers: local (pending) and permanent, each named by a config key, with a promotion rule (auto, propose or off) | session start (own memory or not), selector (where to look), reflection (where learnings go) |
| `config` | the keys it needs set per environment or persona, secrets marked | the part binds them to values; nothing environment-specific inside the bring-in |
| `requires` | what must be present for it to be ready (a command, a base harness capability) | session start and status: available vs ready |

**Recorded by the part at install** (not part of the bring-in; it differs
per harness that uses it):

| Field | What it says | Needed for |
|---|---|---|
| `part`, `persona` | who installed it, for whom | session picture, routing |
| `source` | where it came from: a git project repo, a git store, a machine directory, or a vendor; pinned revision | reflection: where a change goes |
| `route` | how a change is made: write, backlog, worktree to PR, report only (defaults from the source) | reflection: the finding's route |
| `values` | the config values bound for this environment and persona | session start, selector |

**Carried in existing slots, under one namespace:** an Agent Skill's
`metadata` (flat keys); a plugin manifest's `extensions` entry; an MCP
server's `_meta`; a sub-agent or hook through the plugin that ships it.
What the part records goes into its part list, and the harness map is the
union of both.

**Left to the standards:** name, version, licence, permissions,
dependencies and activation the base harness does itself. The contract
does not repeat them.

### What forms a harness

```
 MY HARNESS
 |
 +-- part (one per persona: personal, work, ...)
 |    |
 |    +-- persona ............ who the work is for
 |    |     +-- store ........ git checkout: persona memory, permanent bring-in memory
 |    |
 |    +-- harness bring-ins .. present in every session
 |    |     memory, reflection, status, and the context layers' sources
 |    |
 |    +-- task bring-ins ..... present when they apply, or on demand
 |          coding, writing, notes, triage, documents, web, brought-in tools
 |
 |    every bring-in declares:  kind | job | signals | rules | state | config | requires
 |    the part records:         part | persona | source | route | values
 |    state:                    local tier --promote--> permanent tier
 |
 +-- harness map ............ union of all parts' declarations (generated):
 |                            the harness's model of itself
 |
 +-- shared packages ........ memory server, harness-core (no part owns them)
 |
 +-- environment ............ where this session runs; reachable stores, tools
```

### How a session is assembled, at each point

```
 1. SESSION START  (once; again only if the base harness drops it)

    harness map ----------------+
      every bring-in's job,     |
      when, state               |
    personas + stores ----------+--> SESSION PICTURE
    environment probe ----------+      personas (and the likely one)
    pending decisions ----------+      harness bring-ins + where their state lives
                                       task bring-ins: catalog, own memory or not
                                       environment, pending count
                                       everything larger: a pointer

 2. EVERY AGENT CALL  (only what changed)

    the work's own record ------+--> REINFORCEMENT (on change)
      goal, next step           |      active persona + store, goal, next step,
    active persona -------------+      never-drift rules
    invariants -----------------+
                                       +
    outcome (from the work) ---+
    location / project --------+--> MODE (resolved) --+--> MODE CONTEXT (while it holds)
    bring-ins that apply ------+   no one declares it  |     rules of the bring-ins in play
      (their job + signals)                            |     + selected memories for it
    stores + bring-in memory ------+
      --> SELECTOR (capped) -------+--> RECALL: the few memories for this call

 3. CLOSE / ON REQUEST / SWEEP  (after the session, outside it)

    session + sub-sessions --> PACKER --> BUNDLE
                                 said | told | did | touched | friction
    BUNDLE + harness map ----> REFLECTOR --> FINDINGS (thing, kind, evidence)
                                                 |
          contradictions, persona guesses,       v
          proposals ---------------------> USER DECIDES (one batch)
                                                 |
          route from the map:                    v
            persona memory .............. write
            bring-in memory ............. write to its local tier
            skill / bring-in / harness .. backlog or worktree -> PR on its source
                                                 |
                                                 v
                                              LEDGER: is the harness getting better
```

### Mapping to the system design

Next: which carrier each layer and component uses in each base harness,
where each concept's state lives through its pointers, and which package
ships what (harness-core, the memory server, each part). Grounded in the
research on carriers and their survival across compaction and
sub-sessions; drafted after this conceptual design is agreed.

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
- Ruling (user, 2026-10-27): experience architecture agreed. The goal and
  next step come from the work's own records, not a harness copy. A task
  is a focused mode: where the agent is and the job it is doing, guided by
  a bring-in or done natively. Recall at the point of need, with a small
  cap: start with a cheap lookup, move to a local decision model so it
  needs no outside call (a context-engineering concern).
- Ruling (user, 2026-10-27): the session picture is loaded once and the
  per-call reinforcement is sent on change. Whether a base harness needs a
  repeated preamble, or keeps a context for every call, is an
  implementation investigation per base harness.
- Ruling (user, 2026-10-27): the bring-in contract carries only what this
  design needs, not a copy of other systems' manifests, and rides inside
  existing standards' extension slots, so a bring-in developed here can be
  vended to anyone without this harness.
- Ruling (user, 2026-10-27): a mode is derived, not declared: the outcome
  being pursued, the location or project context, and the bring-ins that
  apply there. Bring-ins declare optional signals, not modes. The rest of
  the conceptual design is agreed.
