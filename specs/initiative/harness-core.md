# Initiative: harness-core

- Branch: `initiative/harness-core` (cut from `main` at 0881e13)
- Status: draft for discussion. The architecture below grounds the design;
  Goal, Experience, Macro test and Constraints are drafts for the user to
  correct. Streams are not planned yet.
- Follows initiative `my-harness` (parts, part lists, whole-status, a
  memory server per persona), which is in review.

## Goal

Any base harness I launch plain becomes my agent, and gets better from
use. Installing bring-ins makes them available, but only a few extension
points put anything in the model's context at session start or on every
turn. A small **harness core** - a preamble, a session-start digest, a
learning loop and store sync - makes my harness present in every session
and lets each session teach the next, on any base harness, with no
wrapper.

## Experience

- A fresh session on any machine already knows who I am, which stores I
  keep, and the few memories that matter, before I say anything.
- After a long session I do nothing: later, facts it learned are in the
  right persona's memory; skill edits and new skills wait in an inbox for
  my yes; harness design changes arrive as a proposed PR.
- Facts about the machine stay on the machine; facts about me travel.
- My memory follows me to another machine with no manual sync.
- A scheduled job (for example a daily sweep) runs the same plain base
  harness with the same harness and memory.

## Macro test

Draft: in a scratch HOME with the personal part installed,

1. A fresh session's context contains the preamble and the digest (both
   memory indexes, machine facts, pending inbox count), read from the base
   harness's own startup report.
2. A session that ends after a correction is queued; the reflect job turns
   it into one memory commit in the right store, and a proposed skill edit
   lands in the inbox, not in the skill.
3. A second scratch HOME pulls the store and its first session recalls the
   memory; the machine facts of the first HOME are not there.

## Constraints

- Embedded, not wrapped: everything attaches through each base harness's
  own extension points. Where those differ (hook formats), a small
  per-harness adapter is acceptable; a launcher in front of the binary is
  not.
- Public repo: the design names no employer, internal tool, store or
  machine. Anything specific to one machine or one organisation is a value
  behind a configuration pointer.
- Parts stay independent: the personal part (this repo) and any work part
  depend on the core as a standalone package, never on each other.
- The learning loop never writes harness design (rules, hooks, settings,
  bring-in code) directly; it only proposes.
- Claude Code first; other base harnesses next.

## Design

`(new)` marks what this initiative adds. `[cfg]` marks a bring-in that needs
a configuration pointer: a value set per machine or per persona, never
written into the bring-in itself.

### Two kinds of bring-in

- **Harness bring-ins** make any base harness mine and are present in
  every session: preamble, digest, memory, learning loop, sync, status.
- **Task bring-ins** serve a kind of work and load when the work calls for
  them: coding (kdevkit, kyodakit, kreviewkit), writing (writing-style),
  notes, triage (a sweep), documents, web (browser), and whatever tools a
  part brings in from elsewhere.

Each kind comes from each part: a part carries its persona's harness
bring-ins and its own task bring-ins.

### System view (no machine, no paths)

```
 SURFACES  how a session starts; each starts a PLAIN base harness
   terminal  |  scheduled job (timer -> non-interactive run)  (new)  |  channels (later)
                                       |
+--------------------------------------v-------------------------------------------+
| BASE HARNESS   vendor model + agent loop      claude-code | codex | kiro | agy     |
|                                                                                    |
|  agent loop:  session start --> turns (reason -> act/tool)* --> compact --> end    |
|                    ^                ^                                       ^      |
|  extension points: |                |                                       |      |
|   always-loaded context ---- preamble (new)                                 |      |
|   session-start hook ------- digest (new)                                   |      |
|   MCP tools + instructions -------- memory, task MCPs                       |      |
|   skills / sub-agents ------------- task bring-ins (loaded on demand)       |      |
|   session-end hook ---------------------------------------- capture (new) --+      |
|   settings / permissions / plugins (install-time plumbing)                         |
+--------------------------------------^-------------------------------------------+
                                       |  injected ONLY through extension points
+--------------------------------------+-------------------------------------------+
| MY HARNESS                                                                         |
|                                                                                    |
|  HARNESS BRING-INS: present in every session                                       |
|   preamble       who I am, which stores, how this harness learns    context (new)  |
|   digest         memory indexes + machine facts + pending inbox     hook    (new)  |
|   memory         one memory server per persona              [cfg]   MCP            |
|   learning loop  capture -> reflect -> curate                [cfg]  hook+job (new) |
|   sync           pull at start, commit on write, push in job [cfg]  hook+job (new) |
|   status         installed vs declared (part lists, --whole)        CLI            |
|                                                                                    |
|  TASK BRING-INS: loaded on demand                                                  |
|   coding  kdevkit, kyodakit, kreviewkit         writing  writing-style [cfg]       |
|   notes   notes [cfg vault]                     triage   a sweep [cfg vault]       |
|   documents, web (browser [cfg]), and tools a part brings in from elsewhere        |
+------------------------------------------------------------------------------------+
        delivered by PARTS; each independent, each carries BOTH kinds
 +----------------------------------------+   +-----------------------------------------+
 | PERSONAL PART  (this repo)             |   | WORK PART  (another repo, if any)       |
 |  harness: personal preamble,           |   |  harness: work preamble,                |
 |           personal memory, its jobs    |   |           work memory, its jobs         |
 |  task:    coding, writing, notes,      |   |  task:    its own skills, tools and     |
 |           web, documents               |   |           brought-in packages           |
 +-------------------+--------------------+   +--------------------+--------------------+
                     |  both depend on standalone public packages   |
                     +-----> memory server (one instance per store) <+
                     +-----> harness-core (preamble, digest, loop, sync)  (new) <+

 STATE: each bring-in declares its own; the pointer says where it lives
 +-------------------+--------------------------+---------+-----------------------------+
 | scope             | home                     | synced  | pointer                     |
 +-------------------+--------------------------+---------+-----------------------------+
 | me                | personal store           | git     | personal part config  [cfg] |
 | work profile      | work store               | git     | work part config      [cfg] |
 | project           | the repo's own files     | repo    | the repo                    |
 | machine           | machine store (new)      | never   | harness-core config   [cfg] |
 | inbox (proposals) | each persona store (new) | git     | harness-core config   [cfg] |
 | task state        | per task bring-in        | varies  | its own config        [cfg] |
 +-------------------+--------------------------+---------+-----------------------------+

 LEARNING LOOP (new), gated by blast radius
   capture  session-end hook queues the session (fast, no model call)
   reflect  job reviews queued sessions once idle ->
              fact about me / my work ........ applied to that persona's memory, one commit
              profile change, contradiction,
              skill edit, new skill .......... inbox, applied on my approval
              harness design (rules, hooks,
              settings, bring-in code) ....... proposal only: a PR to the owning part
   curate   weekly: dedupe, archive what is unused, sync
```

### Binding view: one machine (a template)

Everything machine-specific is a value here; the system view never names
it. Paths follow XDG; `<...>` is a value the user or the provisioning
picks.

```
 MACHINE  <host>: OS, HOME (possibly behind a symlink), how the base harness
          authenticates (decides whether vendor-hosted schedules exist; local
          jobs always work)

 PROVISIONED BY  the user's environment tooling: clones the stores and the
                 part repos, then runs each part's own install

 SOURCES  canonical clones on their main branch; worktrees are for development
          and never do a real install
   <personal part>  ----- install --+
   <work part>      ----- install --+--> INSTALLED below
   memory server, harness-core: pinned at a commit by each part

 INSTALLED  (machine-local, rebuilt from sources)
   each part's own install location under $XDG_STATE_HOME / $XDG_DATA_HOME
   part lists          $XDG_STATE_HOME/my-harness/parts/<part>.json

 BASE HARNESS CONFIG WRITTEN  (shared files; each part owns only its entries)
   each base harness's user settings, MCP registration, plugin registry,
   always-loaded context dir (preamble (new): one file per part)

 STORES  (git checkouts the user owns; portable)
   <personal store>   may be public: never anything that names work
                      about.md  memory/  inbox/ (new)  [the user's other files]
   <work store>       private
                      about.md  memory/  inbox/ (new)  [the user's other files]

 CONFIG POINTERS  (machine-local values)
   personal store path, work store path        set by each part's install
   task state, e.g. writing-style rules        $XDG_CONFIG_HOME/<part>/<skill>/
   notes / triage vault                        a named vault per persona
   machine store (new)                         $XDG_STATE_HOME/my-harness/machine/<host>/
   session queue (new)                         $XDG_STATE_HOME/my-harness/queue/
   jobs (new)                                  user timers running the plain base harness
                                               with explicit plugin and MCP config

 NATIVE MEMORY  (vendor's own, per machine; harvested once, then retired by the user)
```

### What the views surface

1. Task state that is knowledge about me (learned writing rules) should
   live in a persona store through its pointer, not only on one machine.
2. Each vault pointer must be set per persona; an unset pointer falls back
   to a default the user may not expect.
3. The preamble shares the always-loaded context dir with whatever else
   writes there: one file per part, owned like any other entry.
4. A real install comes from one canonical clone per part; installing from
   a second checkout replaces the first and drops whatever it lacked.

## Open questions

1. Is harness-core a standalone package that both parts pin (like the
   memory server), or does it live in this repo with any work part
   mirroring it?
2. Are the review gates right: facts auto-applied, skills and profile via
   an inbox, harness design only as a PR?
3. Should the digest be one hook output for all personas, or one per part?
4. Which job runs first: the reflect job, or a daily sweep-style job?

## Streams

| # | Feature | Ships | Needs | Status |
|---|---|---|---|---|

## Decision Log
