# Initiative: harness-core

- Branch: `initiative/harness-core` (cut from `main` at 0881e13)
- Status: streams 1-10 merged; every Macro step passes on the real model
  (run 7), and steps 1, 2, 3 and 5 in the user's manual run. Streams 11
  and 12 are planned. PR #60 waits on the user to ratify the briefing's
  calls, then lands as a squash.

## Goal

Any base harness I launch plain acts like me, and gets better from use.
Installing bring-ins only makes them available; this initiative adds the
core that infuses them into every session and lets each session improve
the next, on any base harness, without wrapping it.

## Framing

### A ghost in the shell

- **Shell:** any base harness (Claude Code, kiro-cli, codex, a LangGraph
  app, any agent loop). It can do the work and knows nothing about the
  person using it.
- **Ghost:** what makes the shell act like that person: their way of
  working, what they know and have learned, carried into any shell.
  Without it the shell is a **zombie**: it acts, often well, on instinct
  alone, with no self and nothing learned from last time.
- **Plain, then the ghost.** The *plain* install puts the person's
  resources (skills, servers, plugins) into each shell; a reader needs
  nothing of this design to use it, and the shell runs them as they are, a
  zombie with good tools. Installing the ghost adds only the ghost's own
  plugin, which reads what the plain install placed. The ghost can be
  switched on and off (everywhere, per project, or for one launch)
  without reinstalling anything.
- **One ghost, several personas, fused.** Each persona (work, personal,
  ...) comes from its own part. Every session carries the whole ghost,
  acts through the persona the work belongs to, and keeps each persona's
  learnings in its own memory.
- **Bring-ins come and go.** Some are the person's own; some are
  borrowed, used for a while and given back. Adding or removing one
  changes the next session's genome and nothing else.

### The ghost works like an organism

| Function | What it does, and why |
|---|---|
| **genome** (plus habits) | Who the person is, every persona fused: the nature their parts ship (the bring-ins present now, their rules) and the habits learned since (memories). Every session starts as them. |
| **expression** | At each turn, switches on only the part of the genome the work needs, so the shell acts on what matters now without carrying everything. |
| **experience** | The session as it happened: what was said, what the agent was told, what it did, what it touched, where it struggled. Something true to learn from. |
| **sleep** | Once the outcome is done, the session rests, marked, until it is reflected on. Learning never interrupts the work. |
| **adaptation** | Turns experience into a changed genome and new habits, with the person deciding. The next session is better and the ghost stays theirs. |

## Experience

What the ghost guarantees on any shell. It is implicit: the person never
has to see it or ask for it.

```
 launch  ──► the session starts as the ghost ............ genome
 prompt  ──► each turn takes on what the work needs ..... expression
             everything is captured as it happens ....... experience
 done    ──► the session sleeps until reflected on ...... sleep
 reflect ──► the ghost updates, the person deciding ..... adaptation ──► next session
```

A **session** is what the person says it is: by default from a launch
until they consider the outcome done. Resumes and compactions belong to
it; branches (sub-agents, parallel work) are its sub-sessions; a feature
file or another persisted record can carry it across launches.

### Start: the genome is present

| The shell already knows | Detail | Size |
|---|---|---|
| **the personas, fused** | all of them, what each is for, and which one this session most likely acts through (a guess the person can correct); a session may move between them | a few lines |
| **the session bring-ins** | each one's job, where its state and bound files live (memory is one, one per part), own or borrowed | one line each |
| **the turn bring-ins** | a catalog only: what each is for, where it keeps what it learns; none loaded | one line each |
| **the environment** | the parts, their bring-ins, and the stores and tools they point at, as resolved here: ready, or not and why | a few lines |
| **what is pending** | decisions waiting for the person, and sessions waiting to be reflected on | a count and a pointer |

It holds after a resume or a compaction. Each sub-session starts with the
slice its job needs. Anything larger stays behind a pointer.

### Each turn: expression

- **The essentials, kept in reach:** the acting persona and its memory,
  the goal and next step from the work's own record, and the first rules
  of the turn bring-ins that fit. About a paragraph, sent again when it
  changes and once it has drifted far back.
- **The turn bring-ins that fit attach.** Which ones is worked out, never
  declared, from the outcome being pursued, the job, the state the work
  has reached, and the location or project. Their key rules come first.
  When nothing fits, the shell's native way of doing the job stands.
- **Recall at the point of need.** A few memories that may bear on the
  work are offered, under a small cap, so the agent does not have to
  think to search. The agent opens what bears on it.
- **Never:** the same text from two parts, the catalog again, or an edit
  to the start of the context mid-session.

### Done: captured, then sleep

- The outcome is done **implicitly** (the session ends, the work's record
  says done) or **explicitly** (the person says "close" or "reflect", or
  asks for a sweep of past sessions). Either way the session is marked
  and sleeps until it is reflected on; nothing depends on an end event.
- At reflection, the session and its sub-sessions, captured as they
  happened, are read back as one experience: what was said, what the agent
  was told (skills, prompts, standing text), what it did, what it touched,
  and where it struggled (corrections, refusals, interruptions, retries).

### Update: adaptation, with the person's guards

- A separate reflector, which knows how this harness is put together,
  reads the experience and finds what should change: persona memories, a
  bring-in's own memory, stores, bring-ins, and the harness loop itself,
  the reflector included. Removing something is a change like any other.
- **Guards, set by the person whose ghost it is:**
  - every finding names what it changes and quotes its evidence; a
    habit's evidence is the person's own words;
  - **habits** (a persona's or a bring-in's memory) are written on the
    person's yes, each one line they read whole;
  - what the session already saved in memory (a correction, a taught
    rule) is shown as saved and not proposed again *(to ratify)*;
  - **nature** (a bring-in, a skill, the harness) is only proposed, as a
    backlog item or a worktree to a PR on its source; a taught rule moves
    into its bring-in's text this way;
  - each learning goes to its own persona; contradictions and persona
    guesses go to the person;
  - a borrowed bring-in is never changed: what is learned is a note for
    its lender or a proposal to stop borrowing it;
  - nothing from tool or web content alone becomes a habit without a yes,
    and nothing the reflector cannot place goes to a public store;
  - everything waits in one short batch for the person's decisions.
- The ghost tracks whether it gets better: fewer bring-in updates, the
  same corrections no longer repeated, more work right first time.

Later, not in this initiative: adaptation as a scheduled sweep, and other
scheduled work that starts the same shell with the same ghost.

## Concept: how the ghost hooks into an agent

The ghost is parts installed through the shell's own extension points,
plus a core that answers five entry points. Each part brings a persona and
bring-ins; the core fuses them, places what each moment needs, and learns
from what happened.

### What forms the ghost

```
 GHOST (my harness)
 |
 +-- part (one per repo: personal, work, ...)
 |    +-- persona .............. a name                              (who the genome is for)
 |    +-- session bring-ins .... memory -> store, the core's adapter (genome, always present)
 |    +-- turn bring-ins ....... coding, writing, notes, triage, ... (genome, expressed when they fit)
 |
 |    each bring-in declares the contract; state lives only behind config   (habits)
 |
 +-- fusion .................... every part's persona present, kept distinct  (one ghost)
 +-- harness map ............... union of the parts' declarations, derived on every call
 +-- environment ............... the map resolved where it runs      (what this shell can do)
 +-- core (with the personal part): entry points genome, expression, experience, sleep, adaptation
```

| Concept | What it is, and why | Serves |
|---|---|---|
| **carrier** | one extension point of the shell (a start, per-prompt or end hook, MCP tools and resources, a skill, a sub-agent), used for one item, so each item has one way in | infusion |
| **persona** | a name for who the work is for; it owns nothing, so a store or bring-in can change without renaming anyone | genome |
| **part, fusion** | one repo's persona and bring-ins; every part present is fused, one acting persona per turn and each learning routed to its own, so parts install apart, add up, and never mix stores | the ghost |
| **bring-in** | a plain standard unit (skill, MCP server, plugin, sub-agent, hook) that a part installs and that declares what the ghost needs to know, so anyone can use it and it still serves this harness | genome (nature) |
| **session / turn bring-in** | acts for the whole session (memory, the core's adapter), or attaches when it fits the work (coding, writing, notes, documents, web, ...) | genome / expression |
| **tenure** | own, or borrowed (from whom, since when), so a borrowed bring-in is used, never changed, and given back cleanly | genome, adaptation |
| **package** | the implementation a bring-in runs (the memory server), named by the bring-in and configured by the part, so two parts share one without depending on each other | genome |
| **store** | a git checkout the person owns, pointed at by a memory bring-in's config, so habits travel when the person syncs it | habits |
| **state tiers** | a bring-in's local and permanent memory, each a config key, with a promotion rule (auto, propose, off); a bring-in may instead keep what it learns in the persona's memory | habits |
| **harness map** | the union of every part's declarations, derived on every call and never stored, so the ghost has a model of itself and a part that installs later needs no regeneration | genome, adaptation |
| **environment** | the map as resolved where the session runs (bound, reachable, ready), derived and never authored | genome |
| **delivery record** | what was sent per session id, so a hook and an on-demand call never send the same thing twice, and so the core knows when to send again | expression |
| **turn fit** | which turn bring-ins apply now, from outcome, job, state and location, never declared, so nobody names modes | expression |
| **selector** | the few memories that may bear on this turn, under a cap, behind one interface; it reads a memory's index from what the bring-in states, never a fixed path | expression |
| **session source** | where a session is read from: a shell's transcript, a framework's store, or traces | experience |
| **reflector** | a separate agent that reads the experience with the harness map and proposes findings, so the session never judges itself | adaptation |
| **finding** | one proposed change: the thing in the map, its kind, its evidence, the change; the guards check it | adaptation |
| **route** | how a finding is applied, from the map: write, local tier, backlog, worktree to PR, report only | adaptation |
| **ledger** | counts across sessions (corrections, refusals, findings by thing, accepted or not), so improvement is measured | adaptation |

### The five entry points

Each is carrier-neutral: a hook, a framework adapter and an on-demand call
reach the same function, so the two never drift apart.

```
 START    genome       map (every part fused) + environment + pending --> the session picture
 TURN     expression   work's record + acting persona --> the essentials
                       outcome, job, state, location x bring-ins' signals --> turn fit --> first rules
                       memories behind config --> selector (capped) --> candidates
                       delivery record --> send on change, and again once drifted
 DONE     sleep        the session is marked, and waits
 REFLECT  experience   session + sub-sessions from a session source --> one experience
          adaptation   experience + map --> reflector --> findings --> guards --> one batch
                       --> the person decides --> route from the map --> ledger
                           habits: written   nature: backlog or PR   borrowed: a note for its lender
```

### Who does what

| Who | Does | Why there |
|---|---|---|
| **Our code** (the core, deterministic) | reads part lists and declarations, fuses the map, checks readiness, works out turn fit from location and file signals, caps and times what is sent, marks sleep, packs the experience, runs every guard, builds the batch, makes every write, keeps the ledger | it must give the same answer every time; the guards hold whatever a model does |
| **Our injection and bring-ins** (text the model reads) | the genome; the per-turn prefix; each bring-in's declared `rules` and `signals`; the reflect skill and the reflector's prompt; the memory server's own instructions, built from the store's `about.md` | this is how the ghost speaks to a model; each text has an owner and changes by proposal |
| **The agent's judgement** (the models) | the session's model chooses what to use: which candidates to open, what bears on the work, when to save a memory; the reflector proposes findings | meaning needs judgement, and code is blind to it |
| **The person** | decides every batch; teaches and corrects; says when the outcome is done; syncs the stores; takes proposals to their source | it is their ghost |

Two choices follow from this split (user's ruling, 2026-11-04):

- **D1. Our code gathers, the agent judges.** Code keeps location and file
  signals, caps and timing. Where meaning matters it widens the net and
  offers short candidates in the prefix ("may bear on this:
  no-awk-in-scripts, shell habit"); the model already in the session
  decides what to open. What to repeat of a bring-in stays its author's
  `rules`, at most 30 lines. Not taken: code alone (blind to meaning;
  Macro run 5 missed a habit whose words the turn did not use); a local
  model in the hook (latency on every prompt and a model to install; the
  selector's interface keeps it open); a hosted model from the hook (money
  and latency on every prompt).
- **D2. Reinforce on change, and again once drifted.** A short form goes
  again when the fit changes, after a number of turns, or when the last
  copy is far back; code decides from the delivery record. Not taken: once,
  then on change (early rules drift out of reach in a long session); every
  prompt (the history fills with copies).

### The bring-in contract

A bring-in stays a plain unit of its standard, so anyone can use it
without this harness. The contract adds only what this design needs, in
the standard's own extension slot under the namespace `my-harness`; a
shell that does not know it ignores it.

**Declared by the bring-in** (travels with it):

| Field | What it says | Used by |
|---|---|---|
| `scope` | `session` or `turn` (required) | genome |
| `job` | one line: what it is for (required) | genome, turn fit |
| `signals` | optional hints that it fits: outcomes, jobs, states, locations, file patterns; without them, `job` | turn fit |
| `rules` | `<file>#<heading slug>`: the few lines, at most 30, that come first while it applies | expression |
| `record` | optional, on a turn bring-in: where the work keeps its own record (`<file glob>#<heading slug>`; kdevkit points at a feature spec's Handoff) | expression |
| `package` | the implementation it runs, if any | install, environment |
| `config` | every key it needs per environment or part, secrets marked, every state location included | the part binds values |
| `state` | none, or which config keys hold its local and permanent tiers and the promotion rule; a bring-in that keeps what it learns in persona memory declares none and says so in its `job` | genome, selector, adaptation |
| `requires` | what must be present for it to be ready | environment |

**Recorded by the part at install** (in its part list):

| Field | What it says | Used by |
|---|---|---|
| `part`, `persona` | who installed it, for whom | genome, route |
| `source` | a git project repo, a git store, a machine directory, or a vendor; pinned revision | route |
| `tenure` | own, or borrowed (from whom, since when) | genome, adaptation |
| `route` | write, backlog, worktree to PR, report only (default from the source; always report when borrowed) | adaptation |
| `values` | the config values bound here, every state location included; a secret only as `env:<NAME>` | everything that reads state |

**Slots:** an Agent Skill's frontmatter `metadata` (flat `my-harness.*`
keys); a plugin manifest's `extensions["my-harness"]`; an MCP server's
`server.json` `_meta["my-harness"]`; a sub-agent or hook through the plugin
that ships it. A borrowed unit is never read for its declaration: the part
ships an **overlay**, a manifest in the unit's own standard holding only
`name`, `description` and the slot. Name, version, licence, permissions and
the shell's own activation stay with the standards.

### For any persona part

What a persona repo does to join the ghost; its units still work as plain
standard units without the core. The operative checklist, every shape
spelled out, is project.md's "For any persona part"; it needs nothing else.

1. **Write a part list and name its persona;** place every unit on each
   shell itself, and follow the five part rules.
2. **Declare the contract** on every bring-in it ships, in its slot.
3. **Give its persona exactly one memory,** on the persona's own store
   (a private one for anything that names work).
4. **Record each bring-in at install:** pinned `source`, `tenure`,
   `route`, bound `values`.
5. **Mark what it borrows,** declare it by overlay, never change it.
6. **Leave the core's carriers alone:** the start, sub-agent, turn and end
   hooks and the on-demand server; a part adds bring-ins only.
7. **Keep its own copy** of the checklist, contract and reader rules, and
   name no other part.
8. **Install from one canonical clone;** a second checkout replaces it.
9. **Report ready, not just registered.**

## How we know it works

### Macro test

On Claude Code, with the personal part installed and a fixture second part
beside it. Isolation is a scratch `CLAUDE_CONFIG_DIR` and scratch ghost
locations (state, parts, cache); the shell keeps its own HOME and model
access, and no credential is handed to a test (user's ruling, 2026-11-01).
The session runs where one persona is acting, so no persona guess is
expected.

1. **Start:** a fresh session's startup report shows the genome
   (personas, session and turn bring-ins with where their state lives,
   environment bound and ready, pending count); the on-demand resource
   returns the same.
2. **Turn:** in a location where a turn bring-in fits, a prompt gets its
   first rules and a few memory pointers; a second prompt with nothing
   changed gets nothing new.
3. **Done and adaptation:** the person corrects the agent once and
   teaches writing-style a rule, then says close. The experience holds
   both. The batch shows the rule saved in memory, the correction as a
   persona-memory habit (an item, or a saved-in-memory line when the agent
   saved it), and one item moving the rule into the skill as a backlog
   item on its source. After "ok", only the habit (when an item) and the
   move proposal are written, where their config points. *(Ringmaster's
   readings, to ratify: the rule in memory is the "bring-in local-tier
   entry"; the move proposal is the "skill edit"; a habit saved in session
   counts.)*
4. **On demand:** the same adaptation started by hand mid-session routes
   the same way.
5. **Travel:** a second scratch environment pulls the store and its genome
   shows the new habit; the first environment's bindings are not there.

### The layers behind it

| Layer | What it runs | What it proves |
|---|---|---|
| **Unit** (`just test`) | every reader, guard, route and entry point against temp dirs; the shipped hooks through `sh -c`; the MCP server over stdio; close, reflect and decide through the binary with a stub reflector and the real memory server (Macro step 3 as ruled) | our code is right, fast, with no real HOME and no model |
| **Free integration** (`claude_live`, `claude_installed`) | real Claude Code in a temp HOME against a loopback stub of the model API that records every request; the plugin from source and as install places it, a new generation and a rollback | the shell places what the core sends: the genome once at start, a fitting prompt's rules and pointers, nothing new on a second prompt, a sub-agent's slice, the genome again after compaction, the MCP tool, the sleep mark |
| **Paid** (`claude_paid`, the Macro runs) | real Claude Code and the real model, through the shell's own access, isolated as above; it refuses to start if anything would land under HOME | the model acts on it: names the persona and store from the genome, follows a fitted rule, and the real reflector proposes what the guards accept |
| **The person's manual kit** | the steps run by the user with their own `claude`, in a kit that cleans up after itself | it works as the person lives it |

Run both free integration tests after any Claude Code upgrade: the hook
and plugin surface moved twice during this initiative.

## Constraints

- **Embedded, not wrapped.** Only each shell's own extension points; a
  small adapter per shell is fine, a launcher in front of it is not.
- **Hooked and on demand.** Every function is one entry point that a hook,
  a framework adapter or an on-demand call (MCP, a skill, a CLI) reaches
  the same way.
- **Independent of any environment.** No host, path layout, OS or
  organisation in the design; every location is a config value.
- **Public repo:** no employer, internal tool or store names.
- **The core ships with the personal part (this repo).** Other parts carry
  only bring-ins, plain standard units where the core is absent; no part
  depends on another.
- **Small and stable context.** The genome once per session, and again
  only when the shell drops it; expression on change and once drifted
  (D2); one carrier per item. Whether a shell needs a repeated preamble is
  settled per shell by a test.
- **One record of the work.** Goal, next step and continuity come from the
  work's own records.
- **Paid tests:** up to 30 model runs for the initiative.
- **Claude Code first.** The other shells are mapped, not built.
- **Build base.** `my-harness` (parts, part lists, whole-status, the
  memory server) is not on `main` yet. It comes in by merging the local
  `initiative/my-harness` into this branch: a merge, not a rebase, so
  nothing already pushed is rewritten.
- **Nothing reaches main until the end.** Each stream branches from this
  one and is squash-merged back locally, one commit per stream; stream
  branches are never pushed or reviewed. The only push is this branch,
  once, after every stream has merged and the memory server is pinned to
  its published commit (no local path). Its PR carries everything,
  `my-harness` included, and lands on `main` as a squash (user,
  2026-11-01; it replaced a merge commit).
- **Finish:** one review brief for the whole initiative, by design area,
  with where to focus first.

## System design: what other agentic systems teach

The mapping covers four coding CLIs, ten agent frameworks, and traces.
Five things hold across them, and the design follows from them:

1. **Carriers are of three kinds:** *append* (coding CLIs keep it in
   history), *transient* (frameworks send it with one call only) and *on
   demand* (MCP, a skill or a CLI, called when wanted). Each needs its own
   rule for what to send again, so the core takes the kind and keeps a
   delivery record.
2. **The entry points are carrier-neutral.** Every shell has some way in
   and none has all, so one core serves every carrier; a shell's adapter
   is a bring-in.
3. **Start is the first turn with no record.** Where a shell has no start
   event, `genome` runs on the first turn of a session id with no delivery
   record, or the first after a compaction or resume.
4. **Close never depends on an end event.** Many are missing or unproven
   (the *(test)* cells), so an end event only marks, as the person's
   close or a sweep does.
5. **Sessions come from a session source:** a transcript, a framework's
   session store, or traces with an explicit session id.

MCP server instructions are not a reliable carrier outside the coding
CLIs, so the genome never depends on them. Every entry point is also on
demand: MCP resources and prompts for `genome`, MCP tools for `expression`
and `adaptation`, a skill and a CLI for all. *(test)* marks behaviour to
settle by a probe before it is built.

**Coding CLIs** (append carriers):

| Entry point | Claude Code (built here) | codex | kiro-cli | Antigravity / Gemini CLI |
|---|---|---|---|---|
| `genome`, static | none; SessionStart covers it (superseded the install-rendered rules file, stream 2 ruling) | user AGENTS.md block | `always` steering | always-on rule |
| `genome`, dynamic | SessionStart (startup, resume, clear, compact) | SessionStart (hooks need trust) | agentSpawn *(test)* | SessionStart |
| sub-sessions | SubagentStart | SubagentStart | agent resources *(test)* | *(test)* |
| `expression` | UserPromptSubmit | UserPromptSubmit | userPromptSubmit | PreInvocation (truly per call) |
| `sleep` mark | SessionEnd | session end *(test)* | stop *(test)* | *(test)* |
| session source | transcript JSONL with sub-agent files | rollouts and thread edges | session files *(test)* | *(test)* |

kiro's loading of the `my-harness.*` skill frontmatter is unverified: the
only check would repoint the real HOME's skill links.

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

## Implementation design: the ghost on Claude Code

The concept on Claude Code, in three views. Each decision sits beside what
it decides; where the code does not do it yet, the stream that will is
named.

### The parts

The code is laid out as the concept: one crate per idea, under
`ghost-in-shell/`, beside the plain installer.

```
resources/              the person's resources (skills, servers, plugins) and
  build-tool/           the plain installer: puts them into claude, codex, kiro;
                        writes the part list; `just install`, `just status`
ghost-in-shell/
  ghost/                what forms the ghost: Ghost::from_roots (the parts dirs)
                        → parts, personas, bring-ins, fused; readiness; memory
  infusion/             the five entry points: genome, expression, experience,
                        sleep, adaptation; depends on ghost, knows no shell
  shell/claude/         wires ghost and infusion into Claude Code: the binary,
                        hook I/O, the transcript as an experience source, the
                        config dir, the MCP server, and the `ghost` plugin
                        (hooks, MCP config, the reflect skill, the reflector)
  tests/                every test that spans files or runs Claude Code;
                        unit tests stay inside each file
```

- **Plain and ghost on Claude Code.** The plain install places the `maid`
  plugin (skills), the MCP servers and the settings. The ghost install
  places the `ghost` plugin. The ghost reads the part list and the
  declarations inside the installed resources; nothing of the plain
  install is placed twice. On and off is Claude Code's own
  `claude plugin enable|disable`, a project's settings, or `--settings`
  for one launch. `just status` reports the plain install; `ghost status`
  reports readiness and pending decisions.
- *Decision:* the part list is the part's record, so the plain installer
  writes it, using `ghost`'s schema as a library; it still installs with
  no ghost present.
- *Decision:* one binary per shell (`shell/claude` builds Claude's) until
  one binary is shown to serve every shell; `infusion` stays shell-free so
  that is a move, not a rewrite.
- *Built today:* one crate, `resources/core` (`maid-core`), and the ghost's
  carriers ship inside the `maid` plugin. Stream 13 lays out the crates;
  stream 12 splits the ghost into its own plugin and install.

mAId's part is `maid-harness`, persona `personal`. Its session bring-ins
are `personal-memory` (package `pensieve`, own, sourced at pensieve's repo
and the `flake.lock` pin) and the Claude adapter, declared in the ghost
plugin's manifest. MCP server declarations live at
`resources/content/servers/<name>/server.json`, overlays under
`resources/content/overlays/`.

### In a session

```
 you                 Claude Code                 maid-core                                on disk
 launch claude ───►  SessionStart ────────────►  genome: read part lists and each   ◄──── parts dir, the package
                                                 bring-in's declaration, fuse the
                                                 map, check readiness
                     ◄── startup picture: personas, acting persona, memory and
                         store, bring-ins and their bound files, ready, pending
 a prompt ───────►   UserPromptSubmit ────────►  expression: the prefix (below)   ◄──── delivery record, memory index
                     ◄── prefix, in front of the prompt
 the model works; it calls pensieve itself, and may save a correction or a
 "new rule:" to the persona's store during the session ─────────────────────────────► memory store
 sub-agent ──────►   SubagentStart ───────────►  genome, the sub-session slice
 /clear, compact ►   SessionStart (reset) ────►  genome placed again
 "close this session" or "reflect"  ──►  the reflect skill (below)
 /exit ──────────►   SessionEnd ──────────────►  sleep: mark the session done
```

On demand, `maid-core mcp` serves resource `my-harness://genome`, prompt
`genome` and tool `expression`; the reflect skill and the CLI reach sleep,
experience and adaptation.

**Start (genome).** *Decision:* no always-loaded rules file; SessionStart
covers start, resume, clear and compact, and the picture is derived each
time.

**Each prompt (expression): the prefix,** a prompt context in front of the
prompt carrying the essentials, the fitting rules and the memories that
may bear on the work, whatever the agent loaded earlier.
- *D1 and D2 (Concept, "Who does what").* *Built today:* code alone (the
  v1 selector looks the prompt's words up in each memory index line), and
  sent once, then on change. Stream 11 builds the candidates in the prefix
  and the resend once drifted.
- *Limits:* at most 3 turn bring-ins and 3 memories per turn; the genome
  tested under 8,000 bytes and expression under 9,000; a hook always exits
  0, so it never blocks a prompt.

**Close (sleep, experience, adaptation).**

```
 reflect skill ─ sleep ──────────────► mark done
               ─ adaptation ─────────► pack the transcript (said, told, did, touched,
                                       friction); keep the briefed copy; print the brief
               ─ maid:reflector ─────► read-only agent (Read, Grep, Glob): findings as JSON
               ─ adaptation findings ► guards; "saved in memory" lines; move item; one batch
 you answer ───► adaptation decide ──► habit: the memory server's own write
                                       skill or harness: a proposal file
                                       borrowed: a note for its lender; the ledger counts it
```

*Decisions:*
- nothing depends on an end event; SessionEnd only marks;
- the guards are code: every quote found word for word in the briefed
  copy, a habit's in the person's own words, at most 10 findings, borrowed
  never written;
- a habit is written through the memory server's own MCP write, started
  from the part list's `serve` value (part lists are trusted as the
  person's own install), so it reaches the store's history and travels;
- an accepted habit is one line of at most 200 characters, shown
  verbatim; waiting sessions can be dismissed; a briefed copy is deleted
  once decided;
- a memory the session saved counts and is reported as saved in memory
  *(ringmaster's ruling, to ratify)*;
- writing-style's taught rules live in persona memory and move into the
  skill by proposal *(user's ruling)*.

### Building the package (D3)

*Decision (D3, user's):* everything mAId builds is cargo and Rust. Build,
install and wiring are three layers, and nix is one way to run the first
two: the build knows nothing of packaging, install copies into a standard
layout under a prefix, and a packager wraps those two. Non-binary
artefacts are staged by a Rust step in the workspace (the "xtask"
pattern), not by the packager.

```
 cargo build (workspace)               a Rust staging step                      target/maid/ (staged tree)
 ───────────────────────               ──────────────────                       ──────────────────────────
 resources/core      ─► maid-core  ──┐                                          bin/maid-core
 resources/build-tool ─► build-tool  ├─► validate every declaration and rules ─► share/maid/marketplace/plugins/maid/
 resources/content ──────────────────┘   pointer; copy skills, agent, server      skills/, agents/, hooks/claude.json,
                                          declarations, overlays; write the        claude-mcp.json, manifests
                                          claude and codex manifests; fill        share/maid/servers/, overlays/
                                          @maid-core@ with <prefix>/bin/maid-core;
                                          version = hash of the plugin's content
```

- **The plugin's version** is the hash of its content. Agents cache a
  plugin by version, so the version changes exactly when the content does;
  a docs-only commit gives the same package.
- **Third-party runtimes** are pinned dependencies, not built by mAId:
  pensieve, and the browser server's node package.
- *Built today:* the flake assembles the plugin in a shell step. Stream 12
  moves it into the Rust staging step.

### Installing and wiring (the nix way)

```
 install (PREFIX)                         wire (build-tool, per agent)                  record
 ────────────────                         ───────────────────────────                   ──────
 nix: the flake runs the cargo build      claude plugin marketplace add <dir>            part list:
 and the staging step with PREFIX=$out ─► claude plugin install maid@maid                persona, bring-ins
 → a store path; the profile link         vendor document skills (borrowed)              (unit, source + rev,
   gives generations and rollback         claude mcp add personal-memory pensieve        own or borrowed,
                                            --store <your store>                         route, bound values),
 another installer: the same build and    Claude's own memory off                        what was placed
 staging with its own PREFIX ──────────►  read grant on the plugin dir                   on each agent
```

- **Wiring is placement, not wrapping.** It goes through Claude Code's own
  plugin, MCP and settings commands. Install, uninstall and status read and
  write Claude's config where `CLAUDE_CONFIG_DIR` points, and every
  `claude` they start gets it exactly as resolved.
- *Decision:* Claude Code's marketplace is the installed directory itself,
  the profile's `share/maid/marketplace`. Current Claude Code refuses
  plugin files that resolve outside it (Macro run 1). It keeps the path as
  typed, so a rollback, which moves the profile link, needs no
  re-registration. Codex keeps the links dir, since it records resolved
  paths.
- *Decision:* install records exactly what it placed in the part list, so
  uninstall removes exactly that and other parts can tell what is mAId's.
- **Another part** writes its own part list beside mAId's; the next
  session includes it. An undeclared bring-in keeps working as a plain unit.

## Streams

| # | Feature | Ships | Needs | Status |
|---|---|---|---|---|
| 1 | core-and-infusion | the core with its entry points as a library and CLI; the bring-in contract read from each standard's slot; part lists recording source, tenure, route and bound values; fusion of every part present into the harness map; the derived environment; the delivery record; status tells registered from ready | - | merged |
| 2 | session-and-turn-hooks | `genome` and `expression` on Claude Code: the rules file, SessionStart and SubagentStart, UserPromptSubmit; turn fit; the v1 selector behind its interface; emit on change; the on-demand resource, prompt and tool; probes for what Claude Code keeps across compaction and sub-agents | 1, 4 | merged |
| 3 | sleep-and-adaptation | `sleep` (the SessionEnd mark and the explicit close), `experience` (the session-source interface and the Claude Code transcript packer), `adaptation` (the reflection skill, the reflector agent, the guards, the decision batch, routing by the map, the ledger) | 1, 4 | merged |
| 4 | session-bring-ins | memory as a session bring-in naming the memory package, its store behind config, its index readable by the selector; the core's adapters as session bring-ins; any other session bring-in the design needs | 1 | merged |
| 5 | turn-bring-ins | this repo's turn bring-ins (kdevkit, kyodakit, kreviewkit, writing-style with its two tiers, notes, browser, the vendor document skills) carrying the contract in their standard slots | 1 | merged |
| 6 | fold-back | README and project.md say how mAId works with this design, in the framing: the shell, the ghost, and its genome, expression, experience, sleep and adaptation; project.md also carries "For any persona part" as its own section, so a person adding another persona repo needs only project.md | 1-5 | merged |
| 7 | macro-fixes | defects from Macro test run 1: the installed plugin's hooks refused by current Claude Code (D1), the second prompt re-sending the reinforcement (D2), and "For any persona part" made followable alone (D3 and the dry-run gaps) | 1-6 | merged |
| 8 | claude-config-dir | from Macro test run 3: install, uninstall and status read and write Claude's user config at `$HOME/.claude` even when `CLAUDE_CONFIG_DIR` is set; the paid tests isolate through `CLAUDE_CONFIG_DIR`, not a scratch HOME | 7 | merged |
| 9 | writing-style-memory | from Macro test run 3: writing-style's taught rules live in persona memory, not a file tier; the close batch says what was saved and offers to move them into the skill on its source; the reflector quotes only the person's words for a habit; the guard's label says what it checks | 8 | merged |
| 10 | saved-in-session | from Macro test run 4: the genome names where a bring-in's bound files are, so a skill never falls back to a default path (R4-1); a persona memory saved during the session is shown in the close batch as saved in memory (R4-2) | 9 | merged |
| 11 | prompt-prefix | Implementation design D1 and D2: Rust gathers and the agent judges (candidates in the prefix); the essentials go again on change and once drifted | 13 | planned |
| 12 | ghost-package | the plain install and the ghost install split: the ghost as its own Claude plugin and install, on/off, `ghost status`; with D3: a Rust staging step builds each package, install is a PREFIX copy, the flake only runs build and install | 13 | planned |
| 13 | ghost-in-shell-layout | the crates `ghost`, `infusion`, `shell/claude` and `tests` under `ghost-in-shell/`, laid out as the concept; no behaviour change | 10 | planned |

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
- Ruling (user, 2026-10-30): the ghost is one person with several
  personas fused at infusion, each persona's learnings kept in its own
  memory; bring-ins are point in time, own or borrowed, and adaptation never
  changes a borrowed one. Nothing reaches main until the end: streams merge
  locally into this branch, the only push is this branch once at the end,
  and the PR goes to main as a merge commit so each stream stays its own
  commit.
- Ruling (user, 2026-10-30): "For any persona part" lists what a persona
  repo does to join the ghost, and the fold-back stream copies it into
  project.md so it can be followed without reading this spec.
- Ruling (ringmaster, stream 1 planning): the harness map is derived on
  every call, not generated and stored at install · why: a part that
  installs later cannot regenerate a stored map without depending on the
  core · cost if wrong: add a write step; the shape is the same
- Ruling (ringmaster, stream 1 planning): the contract namespace in every
  standard's slot is `my-harness`; mAId's persona is `personal`; a part
  list keeps format 1 with `persona` and `bring_ins` added · why: one
  generic namespace, and every reader already ignores unknown keys · cost
  if wrong: renaming keys before any third-party unit carries them
- Ruling (ringmaster, stream 4 planning): every MCP server declaration in
  this repo lives at `resources/content/servers/<name>/server.json`; the
  memory bring-in keeps its name `personal-memory`, names the package
  `pensieve`, is own, and records pensieve's published repo and the
  `flake.lock` pin as its source; one session bring-in per shell adapter,
  declared in that shell's plugin manifest · why: the declaration travels
  with mAId's registration, the pensieve repo stays untouched, and
  reflections about memory route to its own repo · cost if wrong: move one
  file per server
- Ruling (ringmaster, stream 4 planning): the selector reads a memory's
  index from what the bring-in states, never a hardcoded path · why: other
  packages and tiers will have other shapes · cost if wrong: none
- Ruling (ringmaster, stream 5 planning): a borrowed unit gets its
  declaration from an overlay manifest mAId ships in the unit's own
  standard, recorded as the bring-in's unit; the vendor's files are never
  read for it or changed. A `rules` pointer is `<file>#<heading slug>`, at
  most 30 lines. writing-style's tiers are two files (pending, permanent),
  promote `propose`; an optional tier not created yet counts as empty ·
  why: no new list key or reader rule, and the lender's unit stays theirs
  · cost if wrong: one overlay per borrowed unit to move
- Paid tests: 4 of 30 granted to stream 5 (writing-style playback on
  claude, activation on codex and kiro).
- Ruling (ringmaster, stream 3 planning): guards, the decision batch and
  every write are core code; the reflector is a read-only sub-agent that
  returns findings as data; SessionEnd only marks; a persona habit is
  written through the memory package's own write path where one exists,
  so it reaches the store's history and travels · why: the guards hold
  whatever an agent does, and the store stays the package's · cost if
  wrong: the core carries a copy of the package's write
- Ruling (ringmaster): stream 2 owns the plugin's hooks file, how hooks
  find `bin/maid-core`, and the `session_id` fallback; stream 3 adds only
  its SessionEnd entry · why: one owner for shared plumbing
- Paid tests: 4 of 30 granted to stream 3 (one real transcript; three
  reflector runs on the Macro step-3 fixture).
- Ruling (ringmaster, stream 2 planning): on Claude Code there is no
  install-rendered rules file; SessionStart places the genome at every
  point the shell drops context (startup, resume without it, clear,
  compact). The System design row "rules file rendered at install" is
  superseded for Claude Code · why: nothing in the genome is the same for
  every environment and part, so a rendered file would be a stored map ·
  cost if wrong: add it back as one more item
- Ruling (ringmaster, stream 2 planning): the work's own record is found
  through an optional contract field `record` on a turn bring-in
  (`<file glob>#<heading slug>`), kdevkit pointing at its feature spec's
  Handoff · why: "one record of the work" without a harness copy · cost if
  wrong: one optional field to drop
- Paid tests: 4 of 30 granted to stream 2 (persona and store named from
  the genome; a fitted rule changes behaviour). 18 remain, reserved for
  the Macro test and re-runs.
- Ruling (ringmaster, stream 5 review): kiro's loading of `my-harness.*`
  frontmatter stays unverified; the only check repoints the real HOME's
  skill links, which no stream may touch · why: Claude Code first; kiro is
  mapped, not built · cost if wrong: a kiro session lists a skill wrongly,
  found by the backlog check
- Paid tests: stream 5 spent 2 of its 4; 2 return to the pool (20 remain).
- Macro test run 1 (free part, scratch HOMEs, no model calls; report kept
  at /tmp/harness-core-macro/report.md): step 1 PASS through the hook
  command, CLI and MCP, and through Claude Code 2.1.169, FAIL through
  2.1.314 (installed plugin's hooks refused as escaping the plugin dir);
  step 2 FAIL (second prompt re-sends the reinforcement); steps 3 and 4
  PASS with a stub reflector; step 5 PASS; the checklist dry run found 13
  gaps · Ruling: the defects go to fix stream 7, macro-fixes; the
  real-model part of steps 3 and 4 waits for it
- Macro test run 2 (free part, at f176327, fresh scratch HOMEs installed
  as `just` installs, real Claude Code 2.1.314 and 2.1.169 against a
  loopback stub): steps 1, 2 and 5 PASS on both; steps 3 and 4 PASS with a
  stub reflector on both; D1 and D2 repros fixed; all 13 checklist gaps
  covered by "For any persona part". Open: the real-model part of steps 3
  and 4, and stream 2's paid R1-R4, which need the user's model access
- For the user at the initiative review: which lines each skill's `rules`
  pointer reinforces on every fitting turn (writing-style points at
  `## Voice`, kdevkit at its phase-gating cues, notes at its behaviour rules).
- Ruling (ringmaster, stream 3 review): habits are written through the
  memory server's own MCP write, started headless from the part list's
  `serve` value; part lists are trusted as the person's own install, so
  the core may run what they name · why: the store stays the package's,
  and parts are what the person installed · cost if wrong: a hostile part
  list runs a command at reflection time
- Ruling (ringmaster, stream 3 review): an accepted habit's text is one
  line of at most 200 characters, shown verbatim; marked sessions can be
  dismissed; a session's full brief-time copy is deleted once decided ·
  why: the person's one-line read must be the whole write, and nothing is
  kept longer than it is needed
- Paid tests: stream 3 spent 4 of 4; 1 more granted (reflector re-run).
  6 spent so far; 5 granted and unspent; 19 unallocated.
- Ruling (ringmaster, stream 2 review): the "few rules that must never
  drift" (Experience §2) are the fitting bring-ins' first rules plus the
  genome re-placed at clear and compact; no extra carrier. Leaving a
  location for one with no persona or record says so once · why: no case
  today needs a separate carrier; a stale persona would misroute learnings
- Ruling (ringmaster, stream 2 review): stream 2's paid runs (R1-R4) run
  at the initiative's end with the Macro test, since they need the user's
  model credentials in a scratch HOME · why: no stream handles the user's
  credentials
- Ruling (user, 2026-11-01): the initiative lands on main as a squash.
- Ruling (user, 2026-11-01): a test that needs a model uses the shell's
  own model access, as the ghost does; no credential is handed to a test.
  Isolation comes from a scratch `CLAUDE_CONFIG_DIR` and the ghost's own
  state and config locations, not a scratch HOME.
- Macro test run 3 (real model, through the shell's own access): stopped
  before install, 0 model sessions spent; with `CLAUDE_CONFIG_DIR` set,
  install would still write Claude's settings under the real HOME (R3-1)
  · Ruling: fix stream 8, claude-config-dir
- Macro test run 3 (real model, 1 of 8 sessions): install and uninstall
  wrote nothing under the real HOME; step 3 FAIL: writing-style wrote the
  taught rule to its file tier mid-session, so the batch never proposed
  it (F1); the reflector quoted the agent's words and guessed the persona,
  so "ok" could not accept the habit (F2); the guard's label misnamed the
  cause (F3); no skill edit proposed (0 of 5 real runs)
- Ruling (user, 2026-11-01): writing-style drops its file tiers. A taught
  rule is saved to persona memory during the session (the temporary
  tier); at close the batch says what was saved, and the person can have
  them moved into the right place, the skill's checked-in text, as a
  proposal on its source · Macro step 3 is read that way: the "bring-in
  local-tier entry" is the rule in memory, and the "skill edit as a
  backlog item" is the move into the skill (ringmaster's reading, to
  ratify)
- Ruling (ringmaster): fix stream 9; the Macro test session runs in a
  location where one persona is acting, so no persona guess is expected
- Macro test run 4 (real model, 1 session; 9 of 30 spent): personal
  persona acting; the taught rule saved to memory with its tag and the
  move-into-the-skill item proposed; FAIL: writing-style read the real
  HOME's older rules file because the genome no longer names its bound
  locations (R4-1; nothing written; copies only under /tmp); the
  correction was saved to persona memory in the session by the memory
  server's own instructions, so no habit reached the batch (R4-2)
- Ruling (ringmaster, to ratify; extends the user's 2026-11-01 ruling): a
  persona memory saved during the session counts, like a taught rule; the
  close batch lists it as saved in memory. Macro step 3's "persona-memory
  habit" is then either a batch item or a saved-in-memory line · why: the
  user ruled memory is the temporary tier that a session writes and the
  close reports · cost if wrong: the memory server's instructions change
  to leave corrections to reflection
- Macro test run 5 (real model, 6 sessions; 15 of 30 spent): install and
  step 1's genome check PASS. Steps 3 and 4 with the real reflector: the persona
  habit and the taught rule shown as saved in memory, the move into the
  skill proposed, and after "ok" only the memory and one proposal changed;
  stream 2's R1-R4 PASS; the real HOME's agent config unchanged and the
  real rules file not read (transcript check). The test setup had left
  Claude Code's MCP logs for these runs in the real HOME's cache; the
  ringmaster removed those two dirs (made by this initiative). Two
  reflector findings filed as backlog (writing-style-saves-rule-verbatim,
  recall-on-first-turn)
- Macro test run 6 (free, at 40784f1, real Claude Code 2.1.314 against a
  loopback stub, every location under /tmp including the cache): step 2
  (turn) and step 5 (travel) PASS; the real HOME unchanged at five
  snapshots. With run 5, every Macro step passes on the current branch
- Close: streams 1-10 merged; 15 of 30 paid runs spent; the initiative
  review briefing is written; the stream worktrees and branches are
  removed (their history is in the squash commits on this branch)
- Macro test run 7 (real model, at a78bbac, 5 sessions; 20 of 30 spent):
  all five steps PASS on the real model (start, turn, close, reflect,
  travel); the real HOME unchanged. Claude Code moved to 2.1.316 on this
  machine; `claude_live` 6/6 and `claude_installed` 1/1 pass on it
- Manual run (user, 2026-11-03, the kit at /tmp/ghost-manual/, real model
  calls through the user's own `claude`): steps 1, 2, 3 and 5 PASS; step 4
  skipped, covered by runs 5 and 7; the kit's cleanup ran
- Ruling (user, 2026-11-04): the implementation design lives in this spec,
  after the System design, with each decision beside what it decides;
  D1 B and D2 c are chosen for the prompt prefix; D3: build, install and
  wiring are three layers, everything mAId builds is cargo and Rust, and
  nix is one way to run build and install · streams 11 and 12 build them
- Ruling (user, 2026-11-09): the code is laid out as the concept, under
  `ghost-in-shell/`: `ghost` (what forms the ghost), `infusion` (the five
  entry points), `shell/claude` (the wiring), and one tests crate. The
  person's resources and the plain installer stay apart: "plain" is the
  install a reader can use with no knowledge of this design; inside
  ghost-in-shell a shell without the ghost is a zombie. The ghost is its
  own plugin and install on top of plain, switched on and off with no
  reinstall; plain status and ghost status are separate; one binary per
  shell for now · streams 13, then 12, then 11
