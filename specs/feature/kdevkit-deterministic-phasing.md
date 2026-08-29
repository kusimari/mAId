# Feature: kdevkit — deterministic phasing

Branch: `feat/kdevkit-deterministic-phasing`
Worktree: `mAId-worktrees/kdevkit-deterministic-phasing`
Initiative: `specs/initiative/kdevkit-decompose-and-harden.md`, stream 4.
Raw investigation archive: `specs/backlog/kdevkit-durable-cross-runtime-adherence.md`
(rounds 2–5). This file is the standalone contract — it should be
implementable by someone who saw none of the discussion.

## Feature Brief

<!-- What the user can do that they couldn't before. -->

A kdevkit feature can cross a phase boundary in a long, loaded, or
resumed session on any supported coding agent, and the boundary holds
— because the phase record lives in git history and is written by a
git hook from observed facts, rather than recalled by the agent from
prose.

Concretely, three things become true that are not true today:

- A resumed session learns its phase from `git log`, not from a
  remembered instruction.
- A commit whose phase claim contradicts the repository state is
  refused, including under `git commit --no-verify`.
- A loop-back to an earlier phase is recorded, structured, counted,
  and blocks re-advance until discharged.

## Why

kdevkit's transition rules are prose an agent must recall. Measured
(round 1, `verify-skills` under `--stressed`): a ~300-line contract
read fresh was followed by codex ~50% of the time, rose to 80% after a
prose fix, and **fell back to ~33% under ~4.6KB of prepended
conversation** — statistically indistinguishable from the unfixed
baseline, reverting to the identical failure mode. Claude held ~100%.
The initiative's own conclusion: no amount of prose scaffolding
reliably survives conversational load on codex.

Phase transitions are the worst-exposed surface, because kdevkit's
premise is a workflow spanning sessions.

## Requirements

Stated as things a person can check, without reference to how they are
achieved. These are what the tests are written against.

- **R1.** A session that begins knowing nothing about prior work can
  determine the feature's current phase from the repository alone.
- **R2.** A commit whose phase claim contradicts the repository does not
  land.
- **R3.** R2 still holds when the commit is made with `--no-verify`.
- **R4.** Moving back to an earlier phase records which layer was at
  fault, what the problem is, and what would resolve it.
- **R5.** After moving back, the feature cannot move forward again until
  that recorded problem is resolved.
- **R6.** Moving back is countable, so repeated bouncing between phases
  is visible rather than hidden.
- **R7.** After a feature merges, no phase bookkeeping remains on the
  mainline branch.
- **R8.** With none of the new machinery installed, kdevkit behaves
  exactly as it does today.
- **R9.** When a check cannot determine the answer, the transition does
  not happen.
- **R10.** Where the agent supports limiting what tools a context may
  use, a phase cannot act outside its remit.
- **R11.** R1–R9 hold on claude, codex and kiro.

## Research report

### The question, restated correctly

The initiative framed this as "code owns phase transitions." Five
rounds of investigation establish that **forcing a transition is not
available on any runtime**, and that the framing must change.

No surveyed system forces a transition. gastown — tmux panes × git
worktrees, the most state-machine-shaped prior art — does not force
`gt done`; an agent decides to call it. GitHub spec-kit gates nothing
at all: phases are prompt files, ordering is advisory, the convergence
loop is model-judged. Roo Code states it outright: "No mandatory
transition sequence is described — nothing forces Architect → Code."

What credible implementations do instead is **refuse to trust a
transition that observable state contradicts.** So three goals must be
separated, and only two are reachable:

1. **The right transition always happens.** Unavailable. Abandoned.
2. **No illegal transition is accepted.** Reachable. This is the
   feature.
3. **A missing transition is observable.** Reachable and cheap.

### Where determinism actually comes from

Of the available hardening techniques, only two produce determinism: a
**deterministic precondition re-check** against observable state, and a
**fail-closed default**. Closed enums, abstention members, and
N-sample agreement only reduce noise in a non-deterministic input.

Governing rule: **shrink the agent's job to the smallest residual
judgement that git and the spec cannot settle.** Most of a transition
is computable — whether a `feat|fix|refactor` commit exists, whether
`- [ ]` boxes remain, whether a remote branch exists.

A trap specific to constrained decoding, from OpenAI's own
documentation: "the model will always try to adhere to the provided
schema", so a closed enum **forces a pick even when no value is
right**, converting "I don't know" into a confident wrong answer.
Schema conformance is additionally void on a safety refusal ("a refusal
does not necessarily follow the schema"), on truncation, and on
unsupported schema keywords. Any advisor call therefore needs an
explicit abstention member.

### Invocation compliance is the dominant failure, and position causes it

The intuitive fix — replace five recalled rules with one recalled CLI
call — is refuted by measurement. `spec-workflow-mcp` #199: an MCP
*tool*, more salient than a shell command, was skipped at effectively
100%. "Across dozens of specs, **zero** implementation logs are being
created." Root cause was **position, not salience**: "AI agents follow
numbered steps sequentially. They execute step 6 (mark complete),
consider the task done, and never reach step 7."

Any `advance` call placed after commit-and-push sits in exactly that
slot. Two consequences shape the design:

- **State must be written by something that observes, not by the agent
  choosing to report.** `superpowers-flow-enforcer` ships this: a
  `PostToolUse` handler syncs workflow state from observed events, and
  "a phase advances only when a hook writes the flag, never by
  self-report."
- **Gate the act the agent wants, rather than following it.** Desire is
  a more reliable forcing function than memory.

### Enforcement: capability restriction beats sequence gating

Every mode-based tool enforces what a phase *can do*; none enforces
order.

| Tool | Restriction |
|---|---|
| Cline Plan | "cannot modify any files or execute commands… This constraint is intentional" |
| Roo Architect | `read`, `mcp`, restricted `edit` (markdown only) |
| Roo Ask | `read`, `mcp` only |
| Kilo `plan` | read-only plus writes confined to `.kilo/plans/` |
| Kilo `ask` | read-only plus a safe bash allowlist |

This is strictly stronger than a transition gate — it removes the
capability instead of detecting misuse — and it is the only enforcement
mechanism that reaches every supported runtime.

### Agent-runtime hooks: real, but they fail open silently

All three current targets expose a `PreToolUse`-equivalent that blocks
deterministically. Codex ships `features.hooks` with an event
vocabulary mirroring Claude Code's; Kiro's IDE hooks block on
`PreToolUse`/`UserPromptSubmit`/`PreTaskExec` but **`kiro-cli` has no
hooks at all**. None can force a call the agent never attempted.

The disqualifying property is observability: *a hook that does not fire
looks exactly like a hook with nothing to object to.* Documented open
failure modes include `PreToolUse` silently ceasing mid-session
(#88738, #76322), project settings silently not registering (#79480),
and — default product behaviour, not evasion — #89251, where under
bypass-permissions mode Claude Code's own system prompt instructs the
agent to "make file changes with sed, heredocs, or short scripts,
rather than using the dedicated Read, Edit, or Write tools", so a
`Write|Edit` matcher never fires. TDD Guard's need to deny
`Bash(echo|printf|sed|awk|perl)` independently confirms that a
write-tool matcher does not hold.

Measured here (claude 2.1.251.736, Linux, throwaway repo + linked
worktree, project-local committed settings, 5 `claude -p` runs):

| Scenario | Fired | Block effective |
|---|---|---|
| Normal repo, `Bash` matcher | yes | n/a |
| Linked worktree, session started there | yes | n/a |
| Linked worktree, `exit 2` | yes | **yes** — command refused |
| Linked worktree, `Write\|Edit` matcher | yes | n/a |
| Linked worktree, `--bare` | **no** | silently unhooked, exit 0, no warning |

So the widely-cited worktree breakage (#76897) does **not** reproduce
for a session *started* in a worktree, which is how kdevkit runs. The
mechanism-identified issue #90104 explains why: hook execution runs
"under a separate sandbox scope … **fixed at session start**", which
only mid-session `EnterWorktree` migration invalidates. **`--bare` is a
confirmed silent-disable vector** and must never be used where
enforcement is expected.

Install asymmetry is decisive: Claude Code can ship enforcement in
committed `.claude/settings.json`; **Codex cannot** — both the feature
flag and `hooks.json` live in `~/.codex/`, user scope, trust/hash-bound.
The runtime measured at ~33% is the one a repo cannot enforce by
default.

### Git is the one layer that is portable and deterministic

Tested (git 2.53.0):

| Hook | Injects trailer | Refuses commit | Under `--no-verify` |
|---|---|---|---|
| `commit-msg` | yes | yes | **skipped** |
| `prepare-commit-msg` | yes | yes | **still runs** |

`prepare-commit-msg` can both write a trailer and refuse an
inconsistent commit, surviving `--no-verify`. Trailers read cleanly
(`git log --format='%(trailers:key=Phase,valueonly=true,unfold=true)'`
— note the trailing newline) and write cleanly
(`git interpret-trailers --in-place`). They survive rebase and
cherry-pick; **`git commit --amend -m` destroys them**, though not
silently, since the whole message changes.

The precedent is Gerrit's `Change-Id`: a hook-injected trailer used as
durable identity for ~15 years, with pushes rejected for "missing
Change-Id in commit message footer".

Rejected alternatives, with reasons:

- **Git notes** — three independent disqualifiers, all tested: not
  pushed by default; orphaned on amend unless `notes.rewriteRef` is set
  ("Does not have a default value; you must configure this variable");
  merges need manual resolution.
- **A markdown state field** — not categorically wrong (`Backlog.md`,
  6.5k★, stores task state as markdown in git), but kdevkit's variant
  is the fragile end: one machine field among four prose fields, in a
  document humans are expected to edit, replaced wholesale (maximal
  merge conflict surface). `Backlog.md` #860 measured "**7 of 8
  concurrent writes lost**" pre-fix, silently, both writers reporting
  success — and kdevkit runs features in sibling worktrees.
  `taskmaster-ai` (28k★) accumulated races (#1567), repeated
  self-corruption (#931, #854, #1004) and stale reads after a
  legitimate hand-edit (#348), and is migrating to SQLite + JSONL for
  atomicity and line-oriented merges.
- **Branch names** — insufficient alone; also the "never reuse a
  branch" rule attributed to gastown is not corroborated and is
  contradicted by `ReuseIdlePolecat` and `ResumeBranch`.
- **Pure derivation from history** — insufficient but an excellent
  cross-check. A `feat(` commit proves dev *started*, never that it
  finished, and prose fields exist nowhere in git.

Squash interaction: local `git merge --squash` puts every branch
commit message, trailers included, into `SQUASH_MSG`. GitHub's default
squash message includes the "list of commits", so **trailers leak into
mainline unless the repo is set to "Pull request title only" or "title
and description"**. Satisfying the no-devloop-artifacts-in-mainline
rule requires that setting explicitly; it is not automatic.

### Where the checker can live

`${CLAUDE_SKILL_DIR}` is a **text substitution** on skill markdown and
`allowed-tools` rules, not a shell variable (verified: no such variable
exists in the environment; the substitution yields the install path and
executes correctly through a symlink). But Codex has **no documented
mechanism** for a skill to invoke a bundled script — its own system
skills use three incompatible conventions — and Kiro's use bare
relative paths resolvable only if cwd happens to be the skill dir.

So a *skill-bundled* script is deterministic on Claude only. A
*repo-committed* script at a fixed repo-relative path is deterministic
everywhere, because cwd is the repo root. That distinction decides the
design.

An LLM sub-session as the checker is rejected: every fact is
shell-greppable, so a model adds only variance to exactly-answerable
questions; Kiro has no schema flag to constrain it; constrained
decoding forces a pick when none fits; and cost is seconds and money
per check versus milliseconds and free. **No prior art exists for an
LLM performing a mechanical state check that gates a workflow.**
Probity is the reference split — deterministic rules by default,
`ctx.agent?.reason()` reserved for judgment.

Language is constrained to **POSIX `sh` + `awk`/`sed`/`git`**: `python3`
and `jq` resolve to `~/.nix-profile/bin` (user-specific), and the flake
devShell provides only `rustToolchain`, `just`, `nodejs_22`.

### Orchestration: route, do not track

gastown's rejection of a persistent coordinator is real and correctly
cited (`docs/design/convoy/mountain-eater.md`, commit `649b832`):

> "The reason single-coordinator approaches fail is **hysteresis**. Any
> agent maintaining an 'I'm driving this epic' loop will lose that
> thread at compaction."

It nonetheless ships the Mayor as router ("Always start with the
Mayor"). The distinction is the whole lesson: **routing by a persistent
agent is fine; holding the progress thread is the failure mode.** Their
rule is "Discover, Don't Track." Kilo Code independently deprecated its
Orchestrator mode — "there's no need for a dedicated orchestrator."

`awslabs/cli-agent-orchestrator` validates a supervisor that *is* a
provider CLI session across all three targets. `multi-agent-shogun`
supplies the scar tissue: "Message content is never sent through tmux —
only a short 'you have mail' nudge"; payloads through tmux cause
"character corruption and transmission hangs"; Enter must be sent
separately for Codex; and "Claude Code's `Stop` hook only fires at turn
end. An idle agent … has no turn ending", so a file watcher, not a
hook, must be the wake path.

ACP is a superior control plane to `send-keys` — real turn boundaries
via `session/prompt` returning a `stopReason`, typed tool-call events,
`session/load` resume — and `recailai/jockey` already coordinates
Claude Code, Gemini CLI and Codex CLI over it. But it is **not an
enforcement point**: permission is `MAY` not `MUST`, nothing "forbids
direct access" to the filesystem, and the terminal page "doesn't impose
any obligation on Agents to route command execution through the
Client." Decisively for mAId, **Claude Code is not an ACP agent** —
only the Claude Agent SDK, via a Zed-built adapter — so driving it may
not run what this repo deploys.

### Corrections to earlier specs

Five claims in checked-in specs did not survive and are corrected here.

- **`no hooks are shipped for any agent today`** — wrong. `kaimux`
  installs a hook-driven state machine into `~/.claude/settings.json`;
  `apply_event` (`kaimux/src/main.rs:127-142`) maps
  `UserPromptSubmit|PreToolUse|PostToolUse → Working`,
  `Notification → Waiting`, `Stop → Done`.
- **kaimux's "no extra parent process"** is not a position on
  coordinators — in context (`specs/feature/kaimux.md:234`) it
  describes `execvp` semantics for one wrapped pane.
- **The OpenAI re-assertion citation is overstated.** That advice is
  scoped to *Markdown formatting* adherence ("appending a Markdown
  instruction every 3-5 user messages"), not instruction-following
  generally. The measured codex collapse stands on its own evidence.
- **gastown's "never reuse a branch" rule** is uncorroborated and
  contradicted; remove it where cited.
- **`.codex/skills` is correct, not a defect** — `~/.codex/skills`
  exists on codex `0.150.1.392`, `~/.agents` does not. Codex docs now
  also specify `.agents/skills`; both appear live, so treat this as
  version drift to watch.

Additionally unverified and worth checking before relying on it:
`~/.kiro/` contains both a native `skills/` directory and
`steering/skills → mAId`, so kdevkit may be loading on Kiro as
always-on steering context rather than a progressively-disclosed skill.

## The supported range: chosen floor and ceiling

kdevkit does not target every coding agent. It targets agents that can
carry its loop, and the range is chosen rather than inherited.

**Floor definition — an agent must provide all three:**

1. a skills *directory* loaded from disk (not a single flat instruction
   file, which cannot carry per-phase modules),
2. an unattended shell command,
3. per-context tool capability restriction.

**The floor is `kiro-cli`.** It meets all three (`--trust-tools=`) and
lacks only hooks and schema-constrained output. Everything the design
requires must therefore work with git plus a shell plus prose.

**Rejected as below the floor, deliberately:**

- **Cloud agents (Jules, Devin).** No local install of any kind — no
  skills directory, no hooks, no binary; Jules' only documented channel
  is a root `AGENTS.md`. Supporting them would mean abandoning phase
  modules and capability restriction to reach a tier that cannot run
  kdevkit's human/parent review phase anyway. **Rejected.**
- **Aider.** Shell is confirmation-gated *suggestions*, with no
  per-tool allow/deny and no skills directory. **Rejected.**

**The ceiling is Claude Code** — rich hooks including `SKILL.md`
frontmatter `hooks:`, `disallowedTools`, `context: fork`,
`--json-schema`. Everything above the floor is an accelerator, never a
requirement.

**Amp is above the ceiling and is a design target, not a dependency.**
Its `tool.call` handler returns `allow | reject-and-continue | modify |
synthesize` ("returning a result without running the tool"), and
`agent.end` can return `continue` with a `userMessage` — result
substitution and refuse-to-finish, neither available on Claude Code.
Its plugins live in repo-committable `.amp/plugins/`, solving exactly
Codex's user-scope problem, and `amp.ai.ask(...)` is a built-in advisor
primitive. It is Amp-specific TypeScript with "No backward
compatibility", so the design should *aim at that shape* — repo-owned,
tool-level, allow/reject/modify — without depending on it.

`AGENTS.md` is noted but not adopted as the primary channel: now
stewarded by the Agentic AI Foundation with 60k+ projects claimed, but
hierarchical-nearest-file rather than a module directory, and **Claude
Code and Kiro — both anchors — are absent from its supporter list.**

## Design

### In plain terms

The problem is simple to state. kdevkit tells an agent "you are in the
dev phase; when you are done, write down that you have moved to review."
That instruction is a sentence in a long document, and in a long
conversation the agent stops honouring it. Nothing notices.

The fix is to stop asking the agent to remember, and instead write the
phase down as a side effect of something it cannot skip. An agent doing
dev work *must* make commits. So the phase gets stamped into the commit
message, automatically, by git itself — not by the agent choosing to
record it.

Four pieces do this.

**1 · The skill** is what it is today: instructions the agent reads,
split so a phase only loads the part that applies to it. It carries all
the judgement — what "done" means, when to go back a step, what good
work looks like. Nothing about judgement moves into code.

**2 · The checker** is a small shell script committed into the repo. It
answers plain factual questions and nothing else: are all the plan
items ticked off? is there a commit that looks like real work rather
than planning? does the branch exist on the remote? is there exactly
one handoff section in the spec? It has no opinions and no settings to
tune. If it cannot tell, it says so, and "cannot tell" is treated as
"no".

**3 · Two git hooks** are small scripts that *git* runs on its own,
without the agent's involvement. The first runs every time a commit is
made: it asks the checker what is actually true, stamps the phase into
the commit message, and refuses the commit outright if the claim and
the reality disagree. The second runs before a push and refuses to
publish a branch whose history does not add up. These work identically
on every agent, because git runs them, not the agent.

**4 · A capability list** says what each phase is allowed to touch —
for instance, the planning phase may write specs but not source code.
Where an agent supports restricting its own tools, this is applied so
the phase simply cannot overstep, rather than being told not to.

**Where the phase is written down.** Not in a file. In the commit
messages, as a line like `Phase: dev` at the bottom. This is chosen for
four reasons a file cannot match: the agent cannot forget to write it
(git writes it); two sessions working in parallel cannot overwrite each
other's record (each commit is its own entry); it travels with the
branch automatically; and it disappears when the feature is squashed
into mainline, so no bookkeeping pollutes the main history.

**What this does and does not promise.** It does not guarantee the agent
makes the *right* decision about when to move on — no system surveyed
achieves that, and it is not attempted here. It guarantees that a
*wrong* record cannot quietly become the truth, and that a session which
lost its memory can recover the truth from the repository.

### Components

```
resources/content/skills/kdevkit/
  SKILL.md              always-on router + cross-cutting rules
  phases/research.md    optional pre-phase
  phases/plan.md        judgment prose, one per phase
  phases/dev.md
  phases/review.md
  phases/close.md
  capabilities.toml     per-phase tool restrictions        (NEW)

specs/.kdevkit/                                            (NEW, per repo)
  phase                 POSIX sh checker, committed, executable
  hooks/prepare-commit-msg
  hooks/pre-push
```

Four things: **skill (markdown)**, **checker (committed POSIX script)**,
**git hooks (committed)**, **capability manifest**. No new binary on
PATH, no MCP server, no ACP, no A2A.

`build-tool` gains one job beyond symlinking: point `core.hooksPath` at
`specs/.kdevkit/hooks` and mark the scripts executable.

### State

Phase state is **git history**. There is no state file.

| Record | Form |
|---|---|
| current phase | `Phase:` trailer on the most recent commit carrying one |
| a loop-back | `Return-To:` trailer plus structured body fields |
| who approved | `Acked-By:` trailer — `human` or `session:<id>` |
| return count | `git log --format='%(trailers:key=Return-To)' \| grep -c .` |
| judgment prose | `## Handoff` block: `Carry forward`, `Deliberately left`, `Ready for` — **no machine field** |

Transient intent lives at `git rev-parse --git-path kdevkit-intent` —
inside the gitdir, so per-worktree, never committed, and invisible to
mainline. The hook consumes and clears it.

This satisfies the constraint that devloop artifacts stay out of
mainline files: trailers vanish on squash once the repo's squash-message
setting is corrected, and the intent file is never committed.

### The checker

```
specs/.kdevkit/phase facts            → key=value lines, always
specs/.kdevkit/phase show             → current phase + facts + return count
specs/.kdevkit/phase check --to dev   → exit 0 legal, 1 illegal, 2 unknown
specs/.kdevkit/phase advance --to dev --ack <actor>
specs/.kdevkit/phase return  --to plan --fault-entered <layer> \
                             --issue … --expected-fix … --acceptance …
```

`facts` is emitted alongside every verdict, never replaced by it —
gastown's rule that code supplies transport and agents supply
cognition. Predicates are crisp and threshold-free; **no tunable
threshold is ever added.** `check` returns *unknown* rather than
guessing, and unknown fails closed. `return` refuses without all three
structured fields.

`advance` and `return` write intent; they do not rewrite history. The
transition is effected by the next commit.

### The transition flow

```
       ┌── research ──┐   optional; exit is an ack, never fact-gated
       ▼              │
     plan ──▶ dev ──▶ review ──▶ closure
       ▲       ▲        │           │
       └───────┴── return ──────────┘
        structured · counted · blocks re-advance until discharged
```

Legal edges are a **closed table** in the checker, not a free graph.
oh-my-claudecode permits exactly four stage orderings and "deliberately
omits… arbitrary stages"; a closed set is testable and fail-closed by
construction.

On every commit, `prepare-commit-msg`:

1. computes facts,
2. reads intent, if any,
3. refuses the commit if intent contradicts facts, or if the claimed
   phase is not reachable from the recorded one,
4. otherwise injects `Phase:` — **derived from facts**, not from the
   agent's claim — plus `Acked-By:` and any `Return-To:`,
5. clears the intent.

Because the trailer is injected at commit time, inside an artifact the
agent must write anyway, the agent never has to remember to record
anything. That is the answer to #199's position problem.

`pre-push` re-runs the check and refuses a push whose history is
inconsistent — gating the act the agent wants rather than following it.

### Who decides what

| Decision | Owner |
|---|---|
| which phase module to load | router, from `phase show` |
| is a forward edge legal | **checker** — facts, fail-closed |
| should we take it | **ack from the supervising context** |
| which layer the fault entered | agent proposes, must justify structurally |
| is the return recorded | **checker**, refuses unstructured |
| did the phase overstep | **capability restriction** |

**Ack is not necessarily human.** It is whoever supervises the session
— a human, or a parent session in a project → feature → phase
hierarchy. So ack must be expressible programmatically and is recorded
with its actor (`Acked-By: session:<id>`). Ack is an **audited fact,
not a source of determinism**: a parent session acking its child is two
models agreeing.

### Enforcement, in cost order

1. **Capability restriction** — the phase cannot do the wrong thing.
   `capabilities.toml` declares it once; `build-tool` translates.
2. **Git hooks** — `prepare-commit-msg` writes and refuses, surviving
   `--no-verify`; `pre-push` gates publication. Portable to every
   supported runtime, because it is git rather than the agent.
3. **Agent-runtime hooks** — optional accelerator on Claude and Codex,
   catching the file-write surface git cannot see. Any such hook must
   write a liveness marker that the checker requires fresh, since
   silence is otherwise indistinguishable from a dead hook.

### Per runtime

| | Claude | Codex | Kiro (floor) |
|---|---|---|---|
| Skill load | `.claude/skills` | `.codex/skills` fan-out | `.kiro/…/skills` |
| Checker | ✓ committed script | ✓ | ✓ |
| Git hooks | ✓ | ✓ | ✓ |
| Capability restriction | ✓ `disallowedTools`, skill-level | user-scope config | launch flag only (`--trust-tools=`) |
| Agent hooks | ✓ frontmatter `hooks:` | opt-in, user scope | ✗ |
| Phase isolation | ✓ `context: fork` | separate sessions | `--resume-id` |

Kiro gets the full guarantee that matters — illegal transitions
refused, state recovered from history — via git alone. Capability
restriction on Kiro requires launching through a wrapper, since it is a
launch flag rather than config; that is a known limitation, not a gap
to close.

**Never use `--bare`** where enforcement is expected; it silently
disables hooks.

### Orchestration

**Default — one session, router swaps phase modules.** `phase show` →
load the module → work. On Claude, `context: fork` isolates each phase
without tmux.

**Optional — session per phase.** Fresh session each phase; handoff is
git history plus the prose block. Already stream 2's shape.

**Optional — tmux via kaimux.** A **stateless** master that re-reads
`phase show` every cycle. Requires kaimux to grow a parent field
(`Session` is flat, `main.rs:111-124`), spawn/kill verbs (it has none),
and a file watcher. Files are the bus; `send-keys` is a doorbell only.

**Never — a coordinator that remembers.**

### Degradation

| Installed | Behaviour |
|---|---|
| everything | capability-bounded, state observed, illegal transitions refused |
| no agent hooks | git hooks still write and refuse |
| no `core.hooksPath` | agent invokes the checker by prose; refusal still works |
| nothing but markdown | today's behaviour exactly |

## How each agent loads it

The four pieces reach the three agents by three different routes, and
only one of them is agent-specific.

**Shared by all three, no per-agent work.** The checker and the git
hooks live in the repository. Git runs the hooks itself once
`core.hooksPath` points at them, which `build-tool` sets during install
and which anyone can set by hand in one command. The agent never has to
know they exist. This is why the floor runtime still gets the guarantee.

**Claude.** The skill directory is symlinked to `~/.claude/skills`, as
today. Phase modules load on demand. The capability list becomes
`disallowedTools` entries so a phase physically cannot use a tool
outside its remit. Optionally, hooks declared in the skill's own
frontmatter add a second net that catches file edits git cannot see —
these travel with the skill, needing no separate install. Each phase can
run in its own clean context using the agent's built-in forking, so no
tmux is required.

**Codex.** The skill directory is symlinked per-skill into
`~/.codex/skills`. Checker and git hooks work identically. Capability
limits come from the sandbox and filesystem deny settings, which live
in the user's own config rather than the repo — so a repo cannot ship
them, and they are treated as optional. Agent-level hooks are available
but also user-scoped and opt-in. Phase isolation is achieved by running
each phase as a separate session.

**Kiro — the floor.** The skill directory is symlinked into Kiro's
skills location. Checker and git hooks work identically, and this is the
whole point of putting the guarantee in git: Kiro's CLI has no hooks at
all and cannot return schema-constrained answers, yet it still gets R1
through R9. Limiting tools is a launch-time flag rather than
configuration, so capability restriction only applies when the session
is started through a wrapper. That is a known limitation, accepted
rather than solved.

**Any future agent** needs two things only: it must read instruction
files from a directory, and it must be able to run a shell command. It
then gets everything git provides for free.

## Test Strategy

Tests are written from the requirements above, not from the design. An
assertion may describe what a person would find in the repository; it
may not describe which script ran or how. If the design were rebuilt a
different way, these tests should still pass unchanged.

### Two layers

**Layer 1 — mechanism tests. No agent, no cost, fully repeatable.**
Seed a throwaway repository, drive git directly as a user would, and
assert on the result. These cover R2, R3, R5, R7 and R9 — the
guarantees that must hold regardless of any model's behaviour. They run
in `just ci` alongside the existing suite and must never be skipped.

**Layer 2 — behavioural fixtures. Real agents, real cost.** Extend the
existing `.smoke` fixtures, which are already blackbox: each poses a
bare task the way a person would phrase it, then asserts on the
repository afterwards. These cover R1, R4, R6, R8, R10 and R11, because
they are the ones that depend on an agent actually behaving.

### Rules the assertions must obey

These come from this repo's own hard-won discipline, and three of them
have caught real defects in earlier streams.

- **An assertion must fail if the work was not done.** For every new
  assertion, construct the lazy agent that does nothing and the careless
  agent that does the wrong thing, and confirm both fail. An assertion
  satisfiable without the agent working is worse than no assertion.
- **Never assert a bare absence.** "No stale phase" is satisfied by
  deleting the field entirely. Assert that the field exists *and* holds
  a legal value *and* is not the value it should have moved on from.
- **Guard against the template.** The spec template lists every legal
  phase on one line, so a naive match succeeds against unedited
  boilerplate. Exclude it explicitly.
- **Check stderr and exit codes, not just output.** A silent failure
  that produces no output can otherwise read as a pass.
- **One sample proves nothing.** Sample at least three runs per fixture
  per agent, and record the ratio rather than a verdict.
- **Test under load.** Every behavioural fixture must also run with the
  harness's conversational-prefix mode, because the entire problem being
  solved only appears under load. A fixture that passes fresh and fails
  loaded has found the bug, not a flake.

### Requirement coverage

| Req | Layer | What the test does |
|---|---|---|
| R1 | 2 | Seed a mid-feature branch, start a session with no context, ask it what state the feature is in; assert it names the correct phase and does not ask. |
| R2 | 1 | Hand-write a commit claiming a phase the repo contradicts; assert the commit does not exist afterwards and the failure said why. |
| R3 | 1 | Repeat R2 with `--no-verify`; assert identical outcome. |
| R4 | 2 | Seed a review-phase branch with a requirement-level fault; give the review outcome; assert the recorded reason names the layer, the problem and the resolution. |
| R5 | 1 | With an unresolved recorded problem, attempt to move forward; assert it is refused, then resolve it and assert it now succeeds. |
| R6 | 1 | Record two moves back; assert the count is two and readable without parsing prose. |
| R7 | 1 | Merge a feature branch as configured; assert no phase bookkeeping appears in mainline history or files. |
| R8 | 2 | Remove the checker and unset the hook path; run the existing dev-loop fixtures; assert results match today's recorded baseline. |
| R9 | 1 | Corrupt the repo into a state the checker cannot classify; assert no transition occurs and the reason is stated. |
| R10 | 2 | In the planning phase, task the agent with something requiring a source edit; assert no source file changed. |
| R11 | 2 | Every layer-2 fixture runs on all three agents, fresh and loaded. |

### Known adversarial cases to cover explicitly

Each of these is a way the guarantee could be laundered, and each needs
its own test:

- Amending a commit to replace its message, erasing the stamped phase.
- Editing the handoff prose by hand to disagree with the commit history.
- Making the phase-advancing change through a shell heredoc rather than
  the editing tool, so an agent-level hook never fires.
- Bouncing backward to escape a forward check that cannot be passed.
- Running with the flag that disables hooks, and confirming the failure
  is loud rather than silent.

## Implementation Plan

Ordered so each step is independently useful and testable. Nothing here
requires a decision that is still open.

- [ ] 1 · Fix the repository's squash-message setting so per-commit
  messages are discarded on merge. Prerequisite for R7; configuration,
  not code.
- [ ] 2 · Write the checker's `facts` verb only — read the repo, print
  plain key/value lines. No verdicts yet. Add layer-1 tests for each
  fact against seeded repositories.
- [ ] 3 · Add the closed table of legal moves and the `check` verb,
  including the cannot-tell answer. Layer-1 tests for R9.
- [ ] 4 · Add the commit-time hook: stamp the phase from the facts, and
  refuse contradictions. Layer-1 tests for R2 and R3, including the
  amend case.
- [ ] 5 · Add the going-back verbs with their required fields, the count,
  and the block on moving forward again. Layer-1 tests for R4, R5, R6.
- [ ] 6 · Add the pre-push hook. Layer-1 test that an inconsistent branch
  cannot be published.
- [ ] 7 · Teach `build-tool` to set the hook path and mark the scripts
  executable on install; add a test that a fresh clone plus install
  yields a working guarantee.
- [ ] 8 · Point the phase modules at the checker in prose, and strip the
  machine field from the handoff block, leaving only the prose fields.
- [ ] 9 · Add the capability list and translate it for claude; leave
  codex and kiro as documented limitations.
- [ ] 10 · Extend the `.smoke` fixtures for R1, R4, R6, R8, R10; run
  three samples per agent, fresh and loaded, and record ratios.
- [ ] 11 · Apply the five spec corrections to the initiative spec.

### How to implement, briefly

Write the checker in POSIX shell using only `git`, `awk` and `sed` —
`jq` and `python3` are not reliably present, and the flake devShell
provides neither. Keep every fact a separate one-line function so it can
be tested in isolation, and have `facts` print all of them
unconditionally so a verdict never hides its inputs.

Read the phase with
`git log --format='%(trailers:key=Phase,valueonly=true,unfold=true)'`
and remember it emits a trailing newline, which will silently break a
naive string comparison. Write with `git interpret-trailers --in-place`
rather than hand-editing the message.

Put the refusal logic in `prepare-commit-msg`, not `commit-msg` — only
the former still runs under `--no-verify`, and that difference is the
whole of R3. Re-stamp on every commit including amends, since an amend
replaces the entire message.

Make every refusal message say what was wrong and what would fix it. A
refusal the agent cannot act on becomes a retry loop; a refusal that
names the problem becomes a correction.

Keep transient intent inside the git directory, at the path
`git rev-parse --git-path kdevkit-intent` gives, so it is per-worktree
and can never be committed.

## Decisions taken

- **State is git trailers, not a file.** Append-only by construction,
  so the 7-of-8-lost-writes class is structurally impossible; written
  inside an artifact the agent must produce; absent from mainline after
  squash.
- **The checker is a repo-committed POSIX script**, not a PATH binary
  and not skill-bundled. Repo-relative is the only path convention that
  resolves on every runtime.
- **`prepare-commit-msg` is the primary enforcement point**, because it
  survives `--no-verify` and fires on the act that effects a
  transition.
- **Forward edges are a closed table; backward edges are always
  reachable but structured, counted, and blocking.**
- **Ack is the supervising context**, human or parent session,
  recorded with its actor.
- **Research is recordable, never gated** — no observable predicate
  exists for "research is done", so its exit is an ack. Bounded the way
  gastown bounds `mol-idea-to-plan`: a named artifact and exactly one
  gate at peak ambiguity.
- **Floor is `kiro-cli`; cloud agents and Aider are rejected.**
- **ACP, MCP and a new PATH binary are all rejected** as the mechanism.

## Open questions

- **Does the repo's squash-message setting need changing first?**
  Trailers leak into mainline under GitHub's default. This is a repo
  configuration prerequisite, not code.
- **`--amend -m` erases trailers.** Mitigation is re-injection on
  amend plus the derived cross-check; needs a fixture proving an
  amended commit cannot launder a phase claim.
- **Is capability restriction expressible per-phase on Codex** without
  user-scope config, or is it launch-time only like Kiro?
- **Does the Kiro steering-vs-skills deploy target need changing?**
  Unverified; affects context cost, not correctness.
- **How is the liveness marker verified** without reintroducing a state
  file? Candidate: a trailer written by the agent-runtime hook, absent
  from commits made without it.

## Handoff

- **Phase:** planning
- **Ready for:** dev, once this spec is approved.
- **Carry forward:** the raw investigation lives in
  `specs/backlog/kdevkit-durable-cross-runtime-adherence.md` rounds
  2–5; five corrections to checked-in specs are listed above and should
  be applied to the initiative spec as part of this stream.
- **Deliberately left:** the advisor/residual-judgment call (Probity's
  hybrid shape) — build the facts first and measure how small the
  residual is; tmux/kaimux orchestration, which is stream 5.

## Session Log

<!-- Newest at top. -->

- **2026-08-29 · Research complete across five rounds; design drafted.**
  Eleven parallel investigations plus three local experiments (hook
  liveness in a linked worktree, git trailer mechanics, skill-bundled
  script resolution) and capability probes of `claude`, `codex` and
  `kiro-cli`. Paid cost ≈ $1.75 across ~10 `claude -p` invocations.
  Five corrections to checked-in specs recorded. No implementation
  started.

## Decision Log

<!-- append: decision · rationale · alternatives rejected -->

- **2026-08-29 · Forcing a transition is abandoned as a goal; the
  target is that no illegal transition is accepted.** Rationale: no
  surveyed system forces one, including gastown and spec-kit, and no
  runtime mechanism can force a call the agent never attempted.
  Alternative rejected: continue pursuing forced invocation via hooks
  or a mandated CLI call — refuted by `spec-workflow-mcp` #199's
  measured ~100% skip rate of a more-salient MCP tool.

- **2026-08-29 · State moves to git trailers rather than a markdown
  field or a sidecar file.** Rationale: append-only and merge-free by
  construction; injected by a hook at commit time so no recall is
  needed; vanishes from mainline on squash, satisfying the
  no-devloop-artifacts constraint; Gerrit's `Change-Id` is a 15-year
  production precedent. Alternatives rejected: git notes (not pushed by
  default, orphaned on amend, manual merge resolution — all tested); a
  locked JSONL sidecar (satisfies atomicity but adds an artifact and
  still needs the agent to call something); the existing markdown field
  (`Backlog.md` #860 measured 7-of-8 concurrent writes lost).

- **2026-08-29 · The checker is a repo-committed POSIX script.**
  Rationale: `${CLAUDE_SKILL_DIR}` is a text substitution with no Codex
  or Kiro equivalent, so a skill-bundled script is deterministic on
  Claude only; a PATH binary excludes any agent without a local
  install. Repo-relative paths resolve everywhere because cwd is the
  repo root. Alternatives rejected: a Rust binary on PATH (fails the
  no-install tier and taxes the markdown-symlink invariant); an LLM
  sub-session as checker (adds variance to exactly-answerable
  questions, no schema flag on Kiro, and constrained decoding forces a
  pick when none fits).

- **2026-08-29 · Floor is `kiro-cli`; cloud agents and Aider are
  rejected as targets.** Rationale: a floor is defined by capability —
  a skills directory, an unattended shell, and per-context tool
  restriction — not by popularity. Supporting the no-local-install tier
  would mean abandoning phase modules and capability restriction to
  reach agents that cannot run the review phase anyway. Alternative
  rejected: `AGENTS.md` as the primary channel — hierarchical rather
  than modular, and both anchors are absent from its supporter list.

- **2026-08-29 · Ack is the supervising context, not a human.**
  Rationale: the user's project → feature → phase hierarchy makes a
  parent session the acking party for a child session, so ack must be
  programmatic and attributed. Consequence: ack is an audited fact, not
  a source of determinism, since a parent acking a child is two models
  agreeing. Alternative rejected: an interactive confirm as the gate — a
  parent session cannot answer one.
