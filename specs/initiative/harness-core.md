# Initiative: harness-core

- Branch: `initiative/harness-core` (cut from `main` at 0881e13)
- Status: spec final, ready to run. The build runs in its own session from
  this spec alone.
- Builds on initiative `my-harness` (parts, part lists, whole-status, the
  memory server as a bring-in), which is not on `main` yet. See
  Constraints > Build base.

## Goal

Any base harness I launch plain acts like me, and gets better from use.
Installing bring-ins only makes them available; this initiative adds the
core that infuses them into every session and lets each session improve
the next, on any base harness, without wrapping it.

## Framing

### The core harness: a ghost in the shell

- **Every base harness is a shell**: Claude Code, kiro-cli, codex, a
  LangGraph app, any agent loop. It can do the work, but it knows nothing
  about the person using it.
- **A ghost in the shell makes it act like that person**: their way of
  working, what they know, what they have learned, carried into whichever
  shell they use.
- **Without that infusion, the base harness is a zombie in the shell**: it
  acts, often well, on instinct alone, with no self, no memory of who it
  serves, and nothing learned from last time.

### The ghost works like an organism

| Function | What it does, and why |
|---|---|
| **genome** (plus habits) | Carries who the person is: the nature their parts ship (bring-ins, rules) and the habits learned since (memories, what bring-ins have learned), so every session starts as them and not as a blank shell. |
| **expression** | Switches on, at each turn, only the part of the genome the work at hand needs, so the shell acts on what matters now without carrying everything at once. |
| **experience** | Records everything done toward the outcome as it happened (what was said, what the agent was told, what it did, what it touched, where it struggled), so there is something true to learn from. |
| **sleep** | Is the moment, once the outcome is done, when experience is gathered and adaptation runs, so learning never interrupts the work itself. |
| **adaptation** | Turns experience into a changed genome and new habits, with the person deciding, so the next session is better and the ghost stays theirs. |

## Experience architecture

What the ghost guarantees, implicitly, whatever the shell. None of it is
something the person sees or has to ask for.

A **session** is what the person says it is: by default from a launch
until they consider the outcome done. Resumes and compactions belong to
it; branches (sub-agents, parallel work) are its sub-sessions; a feature
file or another persisted record can carry it across launches.

### 1. When the harness starts: the genome is present

| The shell already knows | Detail | Size |
|---|---|---|
| **the personas** | all of them, what each is for, and which one this session most likely serves (a guess the person can correct) | a few lines |
| **the session bring-ins** | each one's job and where its state lives (memory is one, one per part) | one line each |
| **the turn bring-ins** | a catalog only: what each is for, whether it keeps its own memory and where; none loaded | one line each |
| **the environment** | the parts, their bring-ins, and the stores and tools they point at, as resolved here: bound, reachable, ready, or not | a few lines |
| **what is pending** | decisions waiting for the person | a count and a pointer |

It holds after a resume or a compaction, and each sub-session starts with
the slice its job needs. Anything larger stays behind a pointer.

### 2. When the harness takes a turn: expression

- **Reinforced only when it changed:** the active persona and its memory,
  the goal and next step taken from the work's own record, and the few
  rules that must never drift. About a paragraph.
- **The turn bring-ins that fit attach:** which ones is worked out, never
  declared, from the outcome being pursued, the job being done, the state
  the work has reached, and the location or project context. Their key
  rules come first, with the entries of their own memory and of the
  persona's memory that bear on the work. When nothing fits, the shell's
  native way of doing the job stands.
- **Recall at the point of need:** a few memories reach the turn without
  the agent having to think to search, under a small cap; selection starts
  as a cheap lookup and is built to move to a local model.
- **Never:** the same text from two parts, the catalog again, or an edit to
  the start of the context mid-session.

### 3. When the outcome is done: sleep and experience

- The outcome is done **implicitly** (the session ends, the work's record
  says done) or **explicitly** (the person says "close", "reflect", or
  asks for a sweep of past sessions). Either way the session is marked for
  sleep; nothing depends on an end event firing.
- At sleep, the session and its sub-sessions become one experience: what
  was said, what the agent was told (skills, prompts, standing text), what
  it did, what it touched, and where it struggled (corrections, refusals,
  interruptions, prompts, retries).

### 4. How the ghost updates: adaptation, with the person's guards

- A separate reflector, which knows how this harness is put together,
  reads the experience and finds what should change in persona memories, a
  bring-in's own memory, stores, session and turn bring-ins, and the
  harness loop itself, the reflector included. Removing something is a
  change like any other.
- **Guards, set by the person whose ghost it is:**
  - every finding names what it changes and quotes its evidence;
  - **habits** (persona memory, a bring-in's local tier) are written;
  - **nature** (a bring-in, a skill, the harness) is only proposed, as a
    backlog item or a worktree to turn into a PR on its source;
  - contradictions and guesses about which persona a learning belongs to
    go to the person;
  - nothing from tool or web content alone becomes a habit without the
    person's yes, and nothing the reflector cannot place goes to a public
    store;
  - everything waits in one short batch for the person's decisions.
- The ghost tracks whether it is getting better: fewer bring-in updates
  needed, the same corrections no longer repeated, more work right first
  time.

Later, not in this initiative: adaptation as a scheduled sweep, and other
scheduled work that starts the same shell with the same ghost.

## Macro test

In a scratch HOME with the personal part installed and a fixture second
part beside it, on Claude Code:

1. **Start:** a fresh session's startup report shows the genome
   (personas, session and turn bring-ins with where their state lives,
   environment bound and ready, pending count); the on-demand resource
   returns the same.
2. **Turn:** in a location where a turn bring-in fits, a prompt gets its
   first rules and a few memory pointers; a second prompt with nothing
   changed gets nothing new.
3. **Done and adaptation:** a session where the person corrects the agent
   once and teaches a bring-in a rule, then says close: the experience holds
   both; one decision batch proposes a persona-memory habit, a bring-in
   local-tier entry and one skill edit as a backlog item; after "ok" the
   habit and the entry are written where their config points and nothing
   else changes.
4. **On demand:** the same adaptation started by hand mid-session routes the
   same way.
5. **Travel:** a second scratch HOME pulls the stores and its genome shows
   the new habit; the first HOME's environment is not there.

## Constraints

- **Embedded, not wrapped.** The ghost attaches only through each shell's
  own extension points; a small adapter per shell is fine, a launcher in
  front of it is not.
- **Hooked and on demand.** Every function is one entry point that a hook,
  a framework adapter or an on-demand call (MCP tool, resource or prompt,
  a skill, a CLI) reaches the same way.
- **Independent of any environment.** No host, path layout, OS or
  organisation in the design; every location is a config value bound per
  environment.
- **Public repo:** no employer, internal tool or store names.
- **The core ships with the personal part (this repo).** Any other part
  carries only bring-ins, which work as plain standard units where the core
  is absent; no part depends on another.
- **Small and stable context.** The genome is loaded once per session (and
  again only when the shell drops it); expression sends only what changed
  on carriers that keep history; each item has exactly one carrier.
  Whether a shell needs a repeated preamble or keeps a context for every
  call is settled per shell by a test.
- **One record of the work.** Goal, next step and session continuity come
  from the work's own records.
- **Paid tests:** up to 30 model runs for the initiative.
- **Claude Code first;** the other shells in the System design are mapped,
  not built, in this initiative.
- **Build base.** The build needs initiative `my-harness`. Until it is on
  `main`, the build stacks this branch on the local `initiative/my-harness`
  and does not push code (that branch still pins the memory server by a
  local path); once `my-harness` is on `main`, it rebases onto `main` and
  pushes as usual. The spec itself is on the remote now.
- **Finish:** one review brief for the whole initiative, by design area,
  not stream by stream, with where to focus first.

## Conceptual design

### Technical concepts

The framing names what the ghost is; these are what make it work. Each is
stated with what it is, why it exists, and the framing concept it serves.

| Concept | What it is, and why | Serves |
|---|---|---|
| **base harness** | a vendor's agent loop, reached only through its extension points, because wrapping it would tie the ghost to one shell | the shell |
| **extension point / carrier** | one extension point used to place one item (always-loaded context, a start, per-prompt or end hook, MCP tools and resources, a skill, a sub-agent definition), so each item has exactly one way in | infusion |
| **carrier kind** | append (kept in history), transient (sent with one call only) or on demand (called when wanted), because each needs a different rule for what to send again | infusion |
| **part** | what one repo delivers: a persona name and its bring-ins, so personal and work can be installed apart and add up | the ghost |
| **persona** | a name for who the work is for; it owns nothing, so a store or a bring-in can change without renaming anyone | genome |
| **bring-in** | a standard unit (a skill, an MCP server, a plugin, a sub-agent, a hook) that a part installs and that declares what the ghost needs to know about it, so it can be vended to anyone and still serve this harness | genome (nature) |
| **session bring-in** | a bring-in that acts for the whole session (memory, status, the core's adapters), because some of the genome must be present from the start | genome |
| **turn bring-in** | a bring-in that attaches when it fits the work (coding, writing, notes, triage, documents, web, brought-in tools), because the rest of the genome should be expressed only when needed | expression |
| **package** | the implementation a bring-in uses (for example the memory server), named by the bring-in and configured by the part, so two parts can share one without depending on each other | genome |
| **store** | a git checkout the person owns, pointed at by a bring-in's config, so habits travel between environments | habits |
| **state tiers** | a bring-in's local (pending) and permanent memory, each a config key, with a promotion rule, so a learning is tried before it is trusted | habits, adaptation |
| **harness map** | the union of every part's declarations, generated at install, so the ghost has a model of itself to reason over | genome, adaptation |
| **environment** | the harness map as resolved where the session runs (bound, reachable, ready), derived and never authored, so the ghost knows what this shell can actually do | genome |
| **entry points** | `genome`, `expression`, `experience`, `sleep`, `adaptation`: carrier-neutral functions every carrier calls, so hooked and on-demand use never drift apart | all five functions |
| **delivery record** | what was sent per session id, so a hook and an on-demand call do not send the same thing twice | expression |
| **turn fit** | which turn bring-ins apply now, worked out from outcome, job, state and location, never declared, so nobody has to name modes | expression |
| **selector** | the few memories that bear on this turn, under a cap, behind one interface, so recall can move from a lookup to a local model | expression |
| **session source** | where a session can be read from: a shell's transcript, a framework's own session store, or traces, so experience works across shells | experience |
| **bundle** | one session and its sub-sessions as said, told, did, touched and friction, with large outputs as pointers, so the reflector reads what happened and not a raw log | experience |
| **reflector** | a separate agent that reads a bundle with the harness map and proposes findings, so the session never judges itself | adaptation |
| **finding** | one proposed change: the thing in the map, its kind, its evidence, the change | adaptation |
| **guards** | the evidence check, the tool-content and public-store checks, and the person's decision batch, so the ghost stays the person's | adaptation |
| **route** | how a finding is applied: write, local tier, backlog, worktree to PR, report only, taken from the map | adaptation |
| **ledger** | counts kept across sessions (corrections, refusals, findings by thing, accepted or not), so improvement is measured | adaptation |

### The bring-in contract

A bring-in stays a plain unit of its standard, so anyone can use it without
this harness. The contract adds only what this design needs, carried in the
standard's own extension slot under one namespace; a shell that does not
know it ignores it.

**Declared by the bring-in** (travels with it):

| Field | What it says | Used by |
|---|---|---|
| `scope` | session or turn | genome |
| `job` | one line: what it is for | genome, turn fit |
| `signals` | optional hints that it fits: outcomes, jobs, states, locations or file patterns; without them, `job` and the standard's own description | turn fit |
| `rules` | a pointer to the few lines that come first while it applies | expression |
| `package` | the implementation it runs, if any | install, environment |
| `config` | every key it needs per environment or part, secrets marked, every state location included | the part binds values |
| `state` | none, or which config keys hold its local and permanent tiers, and the promotion rule (auto, propose, off) | genome, selector, adaptation |
| `requires` | what must be present for it to be ready | environment |

**Recorded by the part at install** (in its part list):

| Field | What it says | Used by |
|---|---|---|
| `part`, `persona` | who installed it, for whom | genome, route |
| `source` | a git project repo, a git store, a machine directory, or a vendor; pinned revision | route |
| `route` | write, backlog, worktree to PR, report only (defaults from the source) | adaptation |
| `values` | the config values bound here, every state location included | everything that reads state |

**Slots:** an Agent Skill's `metadata` (flat keys); a plugin manifest's
`extensions` entry; an MCP server's `_meta`; a sub-agent or hook through the
plugin that ships it. Name, version, licence, permissions and the shell's
own activation stay with the standards.

### What forms a harness

```
 GHOST (my harness)
 |
 +-- part (one per repo: personal, work, ...)
 |    +-- persona .............. a name                              (who the genome is for)
 |    +-- session bring-ins .... memory -> store, status, adapters   (genome, always present)
 |    +-- turn bring-ins ....... coding, writing, notes, triage, ... (genome, expressed when they fit)
 |
 |    each bring-in declares: scope | job | signals | rules | package | config | state | requires
 |    each part records:      part | persona | source | route | values
 |    state lives only behind config: store, local tier -> permanent tier   (habits)
 |
 +-- harness map ............... union of the parts' declarations    (the ghost's model of itself)
 +-- environment ............... the map resolved where it runs      (what this shell can do)
 +-- core (with the personal part): entry points genome, expression, experience, sleep, adaptation
```

### How a session is assembled

```
 1. START  ->  genome
    harness map + environment + pending decisions
      --> session picture: personas, session bring-ins and their state,
          turn bring-in catalog, environment, pending count

 2. TURN  ->  expression
    the work's record (goal, next step, state) + active persona --> reinforcement, on change
    outcome + job + state + location + turn bring-ins' job and signals
      --> turn fit --> rules of the bring-ins that fit
    memories behind config --> selector (capped) --> recall for this turn

 3. OUTCOME DONE (implicit or explicit)  ->  sleep, then experience
    session + sub-sessions, from a session source --> bundle
      (said | told | did | touched | friction)

 4. UPDATE  ->  adaptation, with guards
    bundle + harness map --> reflector --> findings (thing, kind, evidence)
      --> checks --> the person decides (one batch)
      --> route from the map:
            habits:  persona memory written; bring-in local tier written
            nature:  bring-in / skill / harness -> backlog or worktree to PR
      --> ledger
```

## System design: mapping per base harness

Each entry point reaches each shell through the carrier named; *(test)*
marks behaviour settled by a probe before it is built. All five entry
points are also on demand everywhere: MCP tools for `expression` and
`adaptation`, MCP resources and prompts for `genome`, a skill and a CLI
for all.

**Coding CLIs** (append carriers):

| Entry point | Claude Code (built here) | codex | kiro-cli | Antigravity / Gemini CLI |
|---|---|---|---|---|
| `genome`, static | rules file without path scope, rendered at install | user AGENTS.md block | `always` steering | always-on rule |
| `genome`, dynamic | SessionStart (startup, resume, clear, compact) | SessionStart (hooks need trust) | agentSpawn *(test)* | SessionStart |
| sub-sessions | SubagentStart | SubagentStart | agent resources *(test)* | *(test)* |
| `expression` | UserPromptSubmit | UserPromptSubmit | userPromptSubmit | PreInvocation (truly per call) |
| `sleep` mark | SessionEnd | session end *(test)* | stop *(test)* | *(test)* |
| session source | transcript JSONL with sub-agent files | rollouts and thread edges | session files *(test)* | *(test)* |

**Agent frameworks** (transient carriers; one registration line by the
app's owner, otherwise on demand only):

| Framework | `genome` | `expression` | `sleep` mark | session source |
|---|---|---|---|---|
| pi | `session_start` | `before_agent_start` or `context` | `session_shutdown` | session JSONL |
| LangGraph / LangChain | first turn on a thread with no mark | `pre_model_hook` `llm_input_messages` / `wrap_model_call` | thread marked in state; sweep | checkpointer |
| OpenAI Agents SDK | dynamic instructions | `call_model_input_filter` | `on_agent_end` | Session store |
| Google ADK | first `before_run_callback` in a session | `before_model_callback` | `after_run_callback`, `close()` | SessionService |
| Microsoft Agent Framework | context provider `before_run` or `agent_startup` | chat middleware | `after_run`, `agent_shutdown` | its store |
| Strands | `AgentInitializedEvent` | `BeforeInvocationEvent` | `AfterInvocationEvent` | SessionManager |
| Pydantic AI | capability instructions | `before_model_request` | `after_run` | app history |
| Mastra | `processInput` | `processInputStep` / `processLLMRequest` | `processOutputResult` | memory threads |
| CrewAI | first `PRE_MODEL_CALL` | `PRE_MODEL_CALL` | after task *(test)* | its memory |
| Letta (server) | a memory block | on demand only | sweep | message API |
| traces (OpenTelemetry, observability tools) | - | - | - | traces with an explicit session id |

Where a shell has no start event, `genome` runs on the first turn of a
session id with no delivery record yet, or the first after a compaction or
resume. MCP server instructions are not a reliable carrier outside the
coding CLIs, so the genome never depends on them.

## Streams

| # | Feature | Ships | Needs | Status |
|---|---|---|---|---|
| 1 | core-and-infusion | the core with its entry points as a library and CLI; the bring-in contract read from each standard's slot; part lists recording source, route and bound values; the harness map and the derived environment; the delivery record; status tells registered from ready | - | planned |
| 2 | session-and-turn-hooks | `genome` and `expression` on Claude Code: the rules file, SessionStart and SubagentStart, UserPromptSubmit; turn fit; the v1 selector behind its interface; emit on change; the on-demand resource, prompt and tool; probes for what Claude Code keeps across compaction and sub-agents | 1, 4 | planned |
| 3 | sleep-and-adaptation | `sleep` (the SessionEnd mark and the explicit close), `experience` (the session-source interface and the Claude Code transcript packer), `adaptation` (the reflection skill, the reflector agent, the guards, the decision batch, routing by the map, the ledger) | 1, 4 | planned |
| 4 | session-bring-ins | memory as a session bring-in naming the memory package, its store behind config, its index readable by the selector; the core's adapters as session bring-ins; any other session bring-in the design needs | 1 | planned |
| 5 | turn-bring-ins | this repo's turn bring-ins (kdevkit, kyodakit, kreviewkit, writing-style with its two tiers, notes, browser, the vendor document skills) carrying the contract in their standard slots | 1 | planned |
| 6 | fold-back | README and project.md say how mAId works with this design, in the framing: the shell, the ghost, and its genome, expression, experience, sleep and adaptation | 1-5 | planned |

The work part's own bring-ins take the contract in their own repo, outside
this initiative.

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
- Ruling (ringmaster, to ratify): eight streams, each shipping something
  the Macro test can see; stream 0 lands initiative my-harness first, since
  this design builds on its parts, part lists and memory server; the work
  part's own bring-ins gain the contract fields in its own repo, outside
  this initiative · why: fewest features in dependency order, genome before
  expression, experience before adaptation · cost if wrong: re-cut streams
- Ruling (user, 2026-10-30): the spec is restructured as Framing (the core
  harness as a ghost in the shell; the ghost modelled as an organism, each
  function with what and why, no fiction), Experience architecture (start,
  turn, outcome done implicitly or explicitly, the ghost's update with the
  person's guards), Conceptual design (technical concepts with what, why
  and the framing concept each serves; contract; composition; assembly),
  System design (mapping per base harness only), and six streams (core and
  infusion, session and turn hooks, sleep and adaptation, session
  bring-ins, turn bring-ins, fold-back). Earlier rulings below that name
  "harness/task bring-ins", "mode", "picture/turn/reflect" or "close" are
  superseded by these terms. The build runs in a separate session; the
  finish is one initiative-level review brief by design area.
