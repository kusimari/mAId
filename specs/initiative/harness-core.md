# Initiative: harness-core

- Branch: `initiative/harness-core` (cut from `main` at 0881e13)
- Status: draft for discussion. The system design below grounds the
  initiative; Goal, Experience, Macro test and Constraints are drafts for
  the user to correct. Streams are not planned yet.
- Follows initiative `my-harness` (parts, part lists, whole-status, a
  memory bring-in per part), which is in review.

## Goal

Any base harness I launch plain becomes my agent, and gets better from
use. Installing bring-ins makes them available, but only a few extension
points put anything in the model's context at session start or on every
turn. A small **harness core** makes my harness present in every session
(a preamble and a session-start digest) and lets each session teach the
next (a reflection at session close that routes what was learned to where
it belongs), on any base harness, with no wrapper.

## Framing

*Ghost in the Shell*: the base harness is the **shell**, my harness is the
**ghost** that can inhabit any shell. A shell with no ghost is a **zombie in
the shell**: it acts, often well, on instinct alone, but carries no self, no
memory of who I am, nothing learned from last time. In that film's own
Puppet Master, a ghost argues it is a living organism, and the ghost here
works like one, through five functions:

| Function | What it does | In fiction |
|---|---|---|
| **genome** (plus habits) | what I carry: nature the parts ship, nurture learned since | *The Girl with All the Gifts*: fungus (nature) plus schooling (nurture) |
| **expression** | what this turn switches on, for this outcome and this place | *iZombie*: a brain's skills, taken on for one case, then gone |
| **experience** | everything done for the outcome, as it happened | *Warm Bodies*: a life relived in vivid memory |
| **adaptation** | the experience turned into a changed genome and new habits | *Warm Bodies*, *The Girl with All the Gifts*: coming alive, a new kind |
| **sleep** | where experience and adaptation happen | *Warm Bodies*: zombies do not sleep; the first dream is coming alive |

And one warning: *Pontypool*, where infection spreads through words. That is
prompt injection, and why adaptation never turns tool or web content into a
habit without my yes.

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
| **the session bring-ins** | each one's job and where its state lives; memory is one of them (one per part), reflection is another | one line each |
| **the turn bring-ins** | a catalog only: name, what they attach for, whether each keeps its own memory and where; none is loaded | one line each |
| **the environment** | the parts, their bring-ins and the stores and tools those point at, as resolved here: what is bound, reachable and ready, and what is not | a few lines |
| **what is pending** | proposals and decisions waiting for the user | a count and a pointer |

This holds after a resume or a compaction, and every sub-session starts
with the part of it that its job needs. Everything larger (whole memories,
skill bodies, the harness map) stays behind a pointer.

### 2. On every turn: minimal, current, fitted to the work

- **Reinforced, only when it changed:** the active persona and its memory,
  the goal and next step of the work (taken from the work's own record),
  and the few rules that must never drift. Kept small (about a paragraph), never the whole session-start
  picture again.
- **Turn bring-ins attach when they fit:** which ones apply is worked out,
  never declared, from the outcome being pursued, the job being done, the
  state the work has reached, and the location or project context (coding
  with kdevkit, a review briefing, a sweep, or the agent's native way of
  doing a job no bring-in covers). While they apply, their key rules come
  first, plus the entries of their own memory and of the persona's memory
  that bear on the work. The set changes as those four change.
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
  tier first, then permanent), stores, session and turn bring-ins, and the
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
- **Parts stay independent:** a part never depends on another part. The
  harness core ships with one part; every other part carries only
  bring-ins, which work as plain standard units where the core is absent.
  A bring-in names the package it uses (two parts may use the same one).
- **Small and stable context.** The session picture is loaded once per
  session (and again only when the base harness drops it, as after a
  compaction); the per-call reinforcement is sent only when it changed.
  Whether a base harness instead needs a session-plus-call preamble on
  every call, or offers a context that is kept for every call, is an
  implementation question per base harness: use its kept context if it has
  one, repeat only if it must. Turn context is bounded; anything
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
| **carrier** | one extension point used to place one item: always-loaded context, a session-start, per-prompt or end hook, MCP instructions and tools, a skill, a sub-agent definition |
| **part** | what one repo delivers: a persona name and its bring-ins |
| **persona** | a name: who the work is for. It owns nothing itself; its memory is a bring-in that points at a store |
| **bring-in** | a unit a part installs (a skill, an MCP server, a sub-agent, a hook, a plugin), at one of two scopes: |
| - **session bring-in** | acts for the whole session: memory, reflection, status, the core's own context |
| - **turn bring-in** | attaches per turn when it fits the outcome, the job, the state reached and the location (coding, writing, notes, triage, documents, web, brought-in tools) |
| **package** | the implementation a bring-in uses (for example a memory server); named by the bring-in, configured by the part. Two parts' bring-ins may use the same package |
| **store** | a git checkout the user owns; a bring-in points at it through its config |
| **harness map** | the union of every part's declarations, generated at install: the harness's model of itself |
| **environment** | the harness map as resolved where the session runs: which config is bound, which stores and tools are reachable, which bring-ins are ready. Derived, not authored |
| **session** | what the user says it is; has sub-sessions; continued by the work's own records |
| **turn fit** | which turn bring-ins apply now, worked out from outcome, job, state and location; never declared |
| **context layers** | the session picture, the per-turn reinforcement, the turn context, and on-demand pointers |
| **selector** | picks the few memories that bear on the next turn; a cheap lookup first, a local decision model later |
| **reflection** | packer (session to bundle) -> reflector (bundle + map) -> findings -> user decisions -> apply or file -> ledger |
| **ledger** | what reflection records across sessions to tell whether the harness is getting better |

### The bring-in contract

A bring-in stays a plain unit of an existing standard (an Agent Skill, an
MCP server, a plugin, a sub-agent), so anyone can use it without this
harness. The contract adds only what this design needs, carried inside the
standard's own extension slot; a base harness that does not know it
ignores it.

**Declared by the bring-in** (travels with it, wherever it is vended):

| Field | What it says | Needed for |
|---|---|---|
| `scope` | session or turn | session start: which list it belongs to |
| `job` | one line: what it is for | session start: the catalog line; turn fit |
| `signals` | optional hints that it fits: outcomes, jobs, states, locations or file patterns; without them the harness uses `job` and the standard's own description | every turn: turn fit |
| `rules` | a pointer to the few lines that come first while it applies | every turn: turn context |
| `package` | the implementation it runs, if any (for example a memory server) | install: what to provide; status: ready or not |
| `config` | every key it needs set per environment or part, secrets marked, **including each place it keeps state** (its store, its local tier, its permanent tier) | the part binds values; nothing environment-specific inside the bring-in |
| `state` | none, or which config keys hold its tiers and the promotion rule (auto, propose or off) | session start (own memory or not), selector, reflection |
| `requires` | what must be present for it to be ready (a command, a base harness capability) | status: available vs ready |

**Recorded by the part at install** (differs per installation):

| Field | What it says | Needed for |
|---|---|---|
| `part`, `persona` | who installed it, for whom | session picture, routing |
| `source` | where it came from: a git project repo, a git store, a machine directory, or a vendor; pinned revision | reflection: where a change goes |
| `route` | how a change is made: write, backlog, worktree to PR, report only (defaults from the source) | reflection: the finding's route |
| `values` | the config values bound here, store locations included | everything that reads state |

**Carried in existing slots, under one namespace:** an Agent Skill's
`metadata` (flat keys); a plugin manifest's `extensions` entry; an MCP
server's `_meta`; a sub-agent or hook through the plugin that ships it.
Name, version, licence, permissions, dependencies and the base harness's
own activation are left to the standards.

**State lives only behind config.** No state location is fixed by the
design: each one is a config key the bring-in declares and the part binds,
so a bring-in, a store or an environment can change without touching the
other two.

### What forms a harness

```
 MY HARNESS
 |
 +-- part (one per repo; today a personal part and a work part)
 |    |
 |    +-- persona ............... a name: who the work is for
 |    |
 |    +-- session bring-ins ..... act for the whole session
 |    |     memory (points at a store), reflection, status, ...
 |    |
 |    +-- turn bring-ins ........ attach per turn when they fit
 |          coding, writing, notes, triage, documents, web, brought-in tools
 |
 |    every bring-in declares:  scope | job | signals | rules | package | config | state | requires
 |                                                                         |
 |                                       config holds every state location: store, local, permanent
 |    the part records:         part | persona | source | route | values
 |
 +-- harness map ............... union of all parts' declarations (generated)
 |
 +-- environment ............... the map resolved where the session runs (derived):
                                 bound config, reachable stores and tools, ready bring-ins
```

### How a session is assembled, at each point

```
 1. SESSION START  (once; again only if the base harness drops it)

    harness map -------------+
    environment (resolved) --+--> SESSION PICTURE
    pending decisions -------+      personas
                                    session bring-ins + where their state lives
                                    turn bring-ins: catalog, own memory or not
                                    environment: bound, reachable, ready
                                    pending count; everything larger: a pointer

 2. EVERY TURN  (only what changed)

    the work's own record --+--> REINFORCEMENT (on change)
      goal, next step, state |      active persona + memory, goal, next step,
    active persona ----------+      never-drift rules
                                    +
    outcome, job, state, ---> TURN FIT --+--> TURN CONTEXT (while they apply)
    location                  (worked out)|     rules of the turn bring-ins in play
    turn bring-ins' job + signals --------+     + selected memories for them
    memories behind config --> SELECTOR (capped) --> RECALL: the few for this turn

 3. CLOSE / ON REQUEST / SWEEP  (after the session, outside it)

    session + sub-sessions --> PACKER --> BUNDLE
                                 said | told | did | touched | friction
    BUNDLE + harness map ----> REFLECTOR --> FINDINGS (thing, kind, evidence)
                                                 |
          contradictions, persona guesses,       v
          proposals ---------------------> USER DECIDES (one batch)
                                                 |
          route from the map:                    v
            memory .................... write, where its config points
            bring-in memory ........... write to its local tier
            bring-in / harness ........ backlog or worktree -> PR on its source
                                                 |
                                                 v
                                              LEDGER: is the harness getting better
```

## System design

How the conceptual design maps onto the base harnesses, Claude Code first.
Still environment-independent: every location is a config value bound per
environment. Rows marked *(investigate)* are settled by a test on each base
harness before they are built.

### Who ships what

| Repo | Ships |
|---|---|
| **personal part** (this repo) | the **harness core** (renderer, the three hooks, turn fit, selector, packers, reflection skill and reflector agent, decision step, ledger) and its own session and turn bring-ins |
| **work part** (another repo) | only bring-ins: its session bring-ins (its memory, pointing at its store) and its turn bring-ins, with the contract fields; its part list |
| **packages** | what bring-ins name: for example the memory server, used by both parts' memory bring-ins with different config |

The core reads every part list present, so it serves the work part's
bring-ins without either part depending on the other. Without the personal
part installed, the work part's bring-ins still work as plain units; only
the core's behaviour (session picture, turn fit, reflection) is absent.

### Entry points, carriers and on-demand use

The core is a small set of **entry points**, each carrier-neutral: input is
plain data (session id, prompt, location, the work's record), output is
blocks, each with a hash and an "as of" marker. Every way of reaching the
core calls the same entry point, so a hook and an on-demand call never
drift apart.

| Entry point | Does |
|---|---|
| `genome` | genome plus habits: what the parts ship (nature) and what was learned since (nurture), as the session picture: personas, session and turn bring-ins, environment, pending |
| `expression` | what this turn switches on: turn fit, selector, reinforcement, as the turn context for this prompt |
| `experience` | what was lived: a session, from a session source, into a bundle |
| `adaptation` | bundle + harness map -> findings -> decision batch -> route -> ledger: nurture written, nature proposed, new nurture created |
| `sleep` | marks a session as ended and unadapted; experience and adaptation happen at sleep, on request or in a sweep |

Three **carrier kinds** reach them, declared per base harness in a
**carrier profile** in the harness map:

- **append**: what the carrier adds stays in history (coding CLI hooks).
  Here a block is sent only when its hash changed.
- **transient**: added to one model call and not kept (framework
  middleware, context processors). Here the turn context can be re-sent
  whole on every call.
- **on demand**: the agent or the user calls the core: MCP tools, MCP
  resources and prompts (the picture and the turn context are always
  offered this way too), the reflection skill, a CLI. Works in any host,
  needs no hook trust and no app change, but does not guarantee it runs:
  the picture arrives after the first call and recall only when asked.

**Delivery is recorded per session id**, supplied by the adapter or the
caller, so a hook and an on-demand call do not send the same block twice.

**Session start** is the first turn on a session id with no picture yet, or
the first turn after a compaction or resume; a base harness's own start
event, where it has one, triggers it earlier. **Close never depends on an
end event:** an end event, where it exists, only records the session;
packing and reflection run on request or in a sweep.

**Turn input is neutral:** the adapter supplies location and the work's
record (a coding CLI: cwd, repo, branch, a feature spec; a framework app:
its own notion of where and what).

**Sessions come from a session source:** a base harness's transcript, a
framework's own store (checkpointer, session service), or traces
(OpenTelemetry GenAI or an observability tool), each adapter setting an
explicit session id. Traces are strongest on friction and on what the
model was told, weakest on what the user said.

**Adapters are bring-ins:** each base harness or framework gets a small
session bring-in that binds the entry points to its carriers. A framework
app needs one registration line by its owner (a plugin, middleware or
callback); where that is not possible, on-demand use still works.
MCP server instructions are not a reliable carrier outside the coding CLIs,
so the picture never depends on them.

**The decision batch** uses the host's own human-in-the-loop where it has
one (an interrupt, an approval); otherwise the findings are filed as one
backlog item for the user.

### The three hooks

```
 SESSION START  Claude Code: SessionStart hook, on startup | resume | clear | compact
   runs   core genome
   reads  every part list (the harness map); each bring-in's bound config
   does   resolve the environment (bound, reachable, ready); pull reachable stores
          without blocking; count pending decisions and unreflected sessions
   emits  the dynamic session picture (environment, likely persona, pending)
   static part of the picture: one rules file the core renders at install,
          so it is not re-sent each session
   sub-sessions: SubagentStart hook emits the slice a sub-agent's job needs

 EVERY TURN     Claude Code: UserPromptSubmit hook
   runs   core expression
   reads  the prompt; location (cwd, repo, branch); the work's own record
          (goal, next step, state reached); turn bring-ins' job + signals;
          memory indexes through their config
   does   turn fit: which turn bring-ins apply (v1 deterministic and keyword
          match, no model call); selector: top few memories (v1 lexical, one
          request-in, ranked-list-out interface so a local model can replace it)
   emits  reinforcement + turn context (rules first lines) + recall pointers,
          within the per-turn budget; on this append carrier, only blocks
          whose hash changed

 CLOSE          two halves
   Claude Code SessionEnd hook: core sleep marks the session as
          unreflected (fast, no model call); nothing depends on it firing
   reflection skill ("reflect", "close this session", or a later sweep):
          core experience -> bundle; core adaptation: reflector agent (fresh
          context: bundle + map) -> findings -> check (evidence, tool-only
          content, secrets, public-store guard) -> one decision batch -> route by the map
          (memory write where its config points; bring-in local tier;
          backlog item or worktree on the source) -> ledger append
```

| Hook | codex | kiro | agy |
|---|---|---|---|
| session start | SessionStart (hooks need trust) | agentSpawn *(investigate placement)* | SessionStart |
| every turn | UserPromptSubmit | userPromptSubmit | PreInvocation (truly per call) |
| close | session end hook *(investigate)* | stop *(investigate)* | *(investigate)* |
| static picture | user AGENTS.md block | always steering | always-on rule |

*(investigate)* per base harness: whether the picture survives compaction
and reaches sub-agents; whether a context is kept for every call so the
per-turn hook can stay silent; how much hook output is kept; hook trust.

### State through config

Every state location is a config key on the bring-in that keeps it, bound
by the part: a memory bring-in's store; a bring-in's local and permanent
tiers. The core's own state (pending decisions, ledger, unreflected
sessions, the generated map and environment) is the same: config keys on
the core's own session bring-ins. **Sync:** session start pulls each
reachable store without blocking; the memory package commits on every
write; reflection pushes at its end; a conflict becomes a pending decision.

### What changes in work already built

- **memory:** becomes a session bring-in per part that names the memory
  package and points at its store through config; the shared memory
  guidance moves once into the core's rules file, so two parts' memories
  stop repeating it; the package exposes its index for the selector.
- **parts:** their bring-ins gain the contract fields; their part lists
  gain source, route and bound values, and a ready check from `requires`.
- **install:** a real install comes from one canonical source per part;
  installing from a second checkout replaces the first.

## Streams

| # | Feature | Ships | Needs | Status |
|---|---|---|---|---|
| last | readme-and-project | README and project.md say how mAId works with this design, using the framing (ghost in the shell; genome, expression, experience, adaptation, sleep) | every other stream | planned |

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
- Ruling (user, 2026-10-29): a persona is a name and owns no store; a
  memory bring-in points at a store through config. Bring-ins split by
  scope: session bring-ins (memory, reflection, ...) and turn bring-ins
  (attach when they fit outcome, job, state and location). A bring-in
  names the package it uses instead of a shared-packages layer. The
  environment is derived: the parts and bring-ins as resolved where the
  session runs. Every state location is config on the bring-in.
- Ruling (ringmaster, to ratify): the harness core ships with the personal
  part (this repo); the work part carries only bring-ins · why: the user's
  split; no third package to publish, and the core reads every part list,
  so no part depends on another · cost if wrong: the core moves to its own
  package later, unchanged in shape
- Ruling (user, 2026-10-30): the harness core ships with the personal part
  (this repo); ratified.
- Ruling (ringmaster, from research F on non-coding frameworks): the core
  is carrier-neutral entry points (picture, turn, close-record, pack,
  reflect) that hooks, framework adapters and on-demand calls all reach;
  carriers are append, transient or on demand per a carrier profile;
  session start is the first turn without a picture; close never depends
  on an end event; sessions come from a session source (transcript,
  framework store, traces); adapters are bring-ins · why: keeps both hooked
  and on-demand use open and lets non-coding frameworks host the harness ·
  cost if wrong: a little more indirection than calling hooks directly
- Ruling (user, 2026-10-30): framing is Ghost in the Shell (base harness =
  shell, my harness = ghost; without it, a zombie in the shell); the entry
  points are named for how the ghost works as an organism: genome (plus
  habits), expression, experience, adaptation, sleep; the initiative keeps
  its name; the closing stream updates the README and project.md to tell
  how mAId works with this design, using the framing.
