# Feature: kdevkit — decompose the workflow, harden the gates

Status: **analysis** (pre-planning). This file is the live analysis +
interaction record; it becomes the feature spec once shaping converges.

Branch: `feat/kdevkit-decompose-and-harden`
Worktree: `maid-worktrees/kdevkit-decompose`

## Feature Brief

<!-- What the user can do that they couldn't before. -->

kdevkit today is one 1246-line always-on `SKILL.md` driving four
tiers and three phases through a single agent session. Two failure
modes follow from that shape, and this feature addresses both:

- **Drift.** A long session executing a long document diverges from
  it. The fix is decomposition: the loops (project / initiative /
  feature) and the phases *within* the feature loop (plan → agentic
  dev → human review → closure) become independently loadable
  contexts, each handed to a fresh agent.
- **Soft gates.** The Code Review Gate dispatches one generic
  reviewer. Projects differ on what matters (comment hygiene,
  security, functional-vs-OO idiom), and there is no way for a
  project to add its own reviewer. The fix is a multi-reviewer
  panel with a user-extensible registry.

A third, enabling piece: if each phase can run independently given
the right context, then a feature's sub-sessions become explicit and
orderable under a master session rather than implicit in one thread.

## Why

Three sources agree on the problem, from three directions:

1. **The repo's own backlog already names it.** Two backlog items
   (`kdevkit-refactor-shrink-always-on-context`,
   `kdevkit-dev-loop-vmodel-and-ceremony`) plus a third
   (`kdevkit-durable-facts-to-repo-not-agent-memory`) are folded in
   below. The first explicitly proposes the split this feature
   implements, and its option 3 (code-driven stage transitions) is
   the mechanism.
2. **Observed live.** Recorded in the backlogs: over-ceremony on a
   one-line change (2026-07-15); an authoring-convention violation
   scoring 90/100 because the reviewer never received the convention
   (2026-07-15); a durable fact written to agent auto-memory instead
   of the repo (2026-07-27).
3. **The ecosystem converged on the same answer.** See Research
   below.

## Research — how other tooling on raw coding agents evolved

<!-- Deep research, 2026-08-03. Sources are public docs/READMEs;
     claims attributed, capability numbers are projects' own. -->

### The one finding everything else hangs off

Every mature layer on top of a raw coding agent independently
arrived at the same two-part answer:

> **Decompose the workflow into phases, give each phase a fresh
> context, and make the handoff a file rather than a conversation.**

kdevkit has the file-based handoff (the spec tree) and the phase
*names*. What it lacks is the fresh context per phase — all three
phases run in one thread, which is precisely the drift surface.

### Anthropic — why long contexts drift (the mechanism)

Anthropic's *Effective Context Engineering for AI Agents* supplies
the mechanism behind the observed drift, and it is not a soft claim:

- **Context rot.** "As the number of tokens in the context window
  increases, the model's ability to accurately recall information
  from that context decreases." Not model-specific — "this
  characteristic emerges across all models."
- **Why.** Transformer attention is n² pairwise over n tokens, so
  attention is "stretched thin" over long inputs; training
  distributions skew short, so models "have less experience with,
  and fewer specialized parameters for, context-wide dependencies."
- **Shape of the failure.** "A performance gradient rather than a
  hard cliff" — which matches the symptom exactly: kdevkit sessions
  don't fail loudly, they quietly stop honouring rules.
- **The design principle.** Find "the smallest possible set of
  high-signal tokens that make the desired outcome likely."
- **Sub-agents as the isolation primitive.** "Specialized
  sub-agents can handle focused tasks with clean context windows"
  while the lead agent holds the plan; a sub-agent may burn "tens of
  thousands of tokens or more" exploring and return "only a
  condensed, distilled summary of its work (often 1,000-2,000
  tokens)." This "showed a substantial improvement over
  single-agent systems on complex research tasks."
- **Technique selection** maps onto our three phases neatly:
  compaction for "extensive back-and-forth" (planning), note-taking
  for "iterative development with clear milestones" (dev loop),
  multi-agent for parallel exploration (the review panel).

Consequence for us: the ~1250-line always-on file is not merely
expensive, it is *causally* implicated in the rule-drops. This
retires the open question in the shrink-context backlog about
whether the refactor is worth it.

### Cursor — rule-scoping and an explicit size ceiling

Cursor's rules system is the most direct prior art for *how to
split an always-on document*, and it validates the seam choice:

- **Four application modes**, set by frontmatter (`description`,
  `globs`, `alwaysApply`): `Always Apply` ("Apply to every chat
  session"), `Apply Intelligently` ("When Agent decides it's
  relevant based on description"), `Apply to Specific Files` (glob
  match), `Apply Manually` (`@`-mention).
- **An explicit ceiling: "Keep rules under 500 lines."** kdevkit's
  always-on file is ~2.5× that. Cursor's remedy is exactly option 2
  of our backlog: "Split large rules into multiple, composable
  rules" and "Reference files instead of copying their contents."
- **Nested per-directory rules** stack with parents, "with more
  specific instructions taking precedence" — a precedence model
  worth borrowing for project-level reviewer overrides.
- **Anti-patterns named:** pasting whole style guides (use a
  linter), documenting commands the agent already knows,
  covering rare edge cases, restating code. Cadence advice: "Start
  simple," add a rule only after the agent repeats an error.

Note the tension to resolve, not paper over: kdevkit's prose is long
*because* the rationale is load-bearing (the shrink backlog makes
this point). Cursor's "reference files instead of copying" is the
reconciliation — rationale moves to a deferred/spec location, the
rule itself stays in the always-on file.

### GitHub spec-kit — phases as commands, with consistency gates

spec-kit is the closest structural sibling to kdevkit and the
strongest evidence that our phase decomposition is right:

- **Phase sequence as explicit commands:** `/speckit.constitution`
  (principles) → `/speckit.specify` (what + why) →
  `/speckit.plan` (stack + architecture) → `/speckit.tasks`
  (actionable list) → `/speckit.implement` (build).
- **Artifact-based handoff, not conversational.** Spec feeds plan;
  tasks are built "from your implementation plan"; implement
  executes "according to the plan." Artifacts live under `specs/` —
  the same shape as our spec tree.
- **The `constitution` is our `project.md`** — "governing
  principles and development guidelines that will guide all
  subsequent development," standing guardrails rather than a
  one-time prompt. Independent convergence on the project tier.
- **Quality/consistency commands are the interesting part**, and
  they are gates kdevkit lacks:
  - `/speckit.clarify` — resolve "underspecified areas" *before*
    planning (recommended pre-`plan`).
  - `/speckit.analyze` — "cross-artifact consistency & coverage
    analysis," positioned after `tasks`, before `implement`.
  - `/speckit.checklist` — "unit tests for English," validating
    "requirements completeness, clarity, and consistency."
  - `/speckit.converge` — assesses "the codebase against
    spec/plan/tasks and append remaining work as new tasks";
    i.e. detects spec-vs-code divergence. **This is a drift
    detector, and kdevkit has no equivalent** — our §8.1
    reconcile is a manual sweep at closure only.
- **Caveat / gap:** spec-kit does *not* assign a separate agent or
  session per phase — you "launch your coding agent" and invoke
  phases as slash commands in that one agent. Its docs cover no
  token budgeting, compaction, or session boundaries. So spec-kit
  validates our *seams* but not our *isolation*; for isolation the
  prior art is BMAD and gastown (below).

### Multi-reviewer panels — early findings

`oh-my-opencode` / oh-my-* family (opinionated harnesses over
OpenCode / Codex CLI) ships the clearest panel precedent:

- **`hyperplan`** — "5 hostile agents tear apart your plan from
  orthogonal angles" pre-implementation. Note *orthogonal angles*
  and *hostile* — both directly applicable to our panel design.
- **`security-research`** — three vulnerability hunters plus two
  PoC engineers audit in parallel, "severity graded by real
  exploitability."
- **Team Mode** — a lead agent plus "up to 8 parallel members"
  coordinated by explicit tools (`team_create`,
  `team_send_message`, `team_task_create`, `team_status`) and
  watchable in a tmux focus+grid layout. **That tmux layout is
  kaimux's territory** — see the sub-session finding below.
- **Delegation is category-based, not model-based:** the agent
  picks a category (`visual-engineering`, `deep`, `quick`,
  `ultrabrain`) and "the harness picks the right model." This is
  the same role-not-product indirection `project.md` already
  mandates for skill dispatch — independent convergence on our own
  architectural rule.
- **Context isolation as a first-class budget concern:**
  "Fire 5+ specialists in parallel. Context stays lean. Results
  when ready." Plus hierarchical per-directory `AGENTS.md`
  generated by `/init-deep` so "agents read only relevant
  context," pitched as good "for both token efficiency and your
  agent's performance."

### gastown — the most complete answer to our exact problem

Gas Town (`gastownhall/gastown`, Go; `steveyegge/gastown`
redirects there) is a multi-agent workspace manager: tmux panes ×
git worktrees, state in a Dolt SQL server. It has independently
built most of what this feature reaches for, and several of its
choices are visibly scar tissue from documented incidents.

**Vocabulary.** A **Town** (`~/gt/`) is HQ; a **Rig** is one
project repo under management. The atomic unit is a **Bead** —
"Git-backed atomic work unit"; issue, task, or epic. Everything
is a bead: work items, agent identities, role definitions, mail
messages. A **Hook** is "an agent's primary work queue" — exactly
one live assignment per agent. A **Convoy** wraps related beads
as a work order. Templates go **Formula** (TOML) → `bd cook` →
**Protomolecule** (frozen) → **Molecule** (active) or **Wisp**
(ephemeral).

**#1 — they explicitly rejected a persistent master session.**
This lands directly on the brief's third point, and it is the
single most consequential finding:

> - **The epic IS the thread. The beads ARE the state.**
> - **No agent needs to remember anything.** Each check
>   discovers state fresh.
> - **Dogs bring fresh context every time.** Zero hysteresis by
>   construction.
> - **The label triggers patrol behavior. No persistent
>   coordinator needed.**

Their table lists the *rejected* design beside the shipped one:
"Dedicated coordinator agent" → "No coordinator — patrol steps +
Dogs"; "Recovery via molecule resume" → "Recovery via beads state
discovery."

Coordination is **shared database + pull model + daemon
reconciliation**. A parent holds no child handles. `gt sling
<bead> <rig>` allocates a slot, makes a worktree on a fresh
branch, starts a session, hooks the work — and the receiver
*discovers its own assignment* (`gt hook`, `gt prime`). No prompt
payload passes in-process. Completion is detected by a Go daemon
polling events every 5s and scanning for stranded beads every
30s. Parallelism comes from bead dependencies unblocking, not
imperative scheduling. `scheduler.max_polecats` caps concurrency
"to prevent API rate limit exhaustion."

**Three lifetimes, deliberately separated** — the drift answer in
its cleanest form:

| Layer | Lifecycle |
|---|---|
| **Identity** (agent bead, work history) | Permanent |
| **Sandbox** (git worktree, branch) | Per assignment |
| **Session** (context window) | Ephemeral, cycles freely |

> **Key insight:** Session cycling is **normal operation**, not
> failure. The polecat continues working—only the Claude context
> refreshes.

Conflating the three is named the original sin: "Early designs
treated polecats as monolithic. This caused recurring issues."
Their word for drift is **hysteresis**, and "zero hysteresis by
construction" is a stated goal. Context pressure is a
first-class signal (`"context_usage": 0.73` in the worker status
schema; "GT can use this to trigger compaction/handoff *before
the agent degrades*").

Their gate-reviewer briefing states the isolation contract more
sharply than kdevkit's own §7 does — arrived at independently:

> "You are a gate polecat. … **You have no memory of the
> implementation** — everything you need is in this description
> and in the code on the current branch."

**#2 — `gt handoff` is a machine-collected handoff record.** The
closest analogue to the phase handoff we need, and the key detail
is that the payload is *assembled by code*, not authored by the
agent, with hard truncation caps: `## Workspace State` (branch,
≤10 modified files, ≤5 untracked, stash count, unpushed count,
last 5 commits), `## Hooked Work`, `## Inbox` (10 lines),
`## Ready Work` (10 lines), `## In Progress` (5 lines), plus the
user's free text. There is an `enforceHandoffCooldown()`, so
cycling isn't unlimited.

Context is injected at session start by `gt prime` in fixed order
(session metadata → role template + formula checklist → operator
directives → context file → handoff content). Notably: *"No
per-directory CLAUDE.md or AGENTS.md is created… Full context is
injected by `gt prime` via SessionStart hook."* A cheaper
post-compaction path reminds the agent of just the completion
verb, with the failure it prevents recorded in a comment —
without it "polecats finish implementation and sit at the prompt
forever."

**#3 — the multi-reviewer panel exists, and its shape is
directly copyable.** A `type = "convoy"` formula fans out to N
reviewer legs, one worker each, plus a synthesis step depending
on all of them. `code-review.formula.toml` ships **ten legs**:
`correctness`, `performance`, `security`, `elegance`,
`resilience`, `style`, `smells`, `wiring`
("installed-but-not-wired gaps"), `commit-discipline`,
`test-quality` ("test meaningfulness, not just coverage"). A
plan-altitude sibling has five: `completeness`, `sequencing`,
`risk`, `scope-creep`, `testability`.

```toml
review_only = true   # legs are analysis-only — no code commits

[[legs]]
id = "completeness"
title = "Plan Completeness"
focus = "Are all requirements covered? What's missing?"

[output]
directory = ".plan-reviews/{{.review_id}}"
leg_pattern = "{{.leg.id}}.md"
synthesis  = "plan-review.md"

[synthesis]
depends_on = ["completeness", "sequencing", "risk", …]
```

Each leg shares a base prompt with `{{.leg.focus}}` injected and
a **mandated output contract**: `## Verdict` (`PASS / PASS WITH
NOTES / FAIL` + one-sentence rationale), `## Must Fix (blocks
implementation)`, `## Should Fix`, `## Observations`. Aggregation
elsewhere (`/review`) is **highest-severity-wins mapped to a
letter grade**: A (nothing) and B (MINOR only) pass; C (MAJOR)
and D (CRITICAL) fail; F unreviewable. That answers our
aggregation question with a simpler rule than weighted scoring.

**#4 — user extension without forking, via layered overrides.**
Roles are TOML (`role`, `scope`, `nudge`, `prompt_template`,
`[session]`, `[env]`, `[health]`). Users can't add new *roles*
(the seven are compiled in) but can override behavior two ways.
The motivating incident is recorded verbatim, and it is exactly
the "each project has its own guidelines" problem in the brief:

> "Multiple crew members autonomously posted `gh pr review`
> comments on GitHub during PR review tasks. The formula says
> 'post to GitHub,' and there was no way for the operator to say
> 'actually, in this rig, report back instead.'"

- **Role directives** — plain markdown at
  `~/gt/directives/<role>.md` (town) and
  `<rig>/directives/<role>.md` (rig), injected after the role
  template under the marker *"Rig Policy — overrides formula
  instructions where they conflict."* Town + rig concatenate,
  rig last, so rig wins.
- **Formula overlays** — `~/gt/formula-overlays/<formula>.toml`,
  per-step surgical edits, "CSS-like":
  `[[step-overrides]] step_id = … mode = "replace|append|skip"`.
  Rig-level *fully replaces* town-level rather than merging,
  deliberately: "This prevents conflicting step modifications
  from merging unpredictably." `skip` has real DAG semantics
  (dependents inherit the skipped step's `needs`), and
  `gt doctor` validates overlay step IDs against the current
  formula, auto-fixing stale references.

Best available model for "let someone add their own reviewer": a
**project-wins layered override** over shipped defaults, plus a
validator that catches stale references.

**#5 — a gate with a self-re-blocking retry loop.** A gate bead
is created blocked-by all implementation beads, so it becomes
runnable only when implementation is done. Clean pass → it closes
itself. Any finding → it does *not* close: it files one fix bead
per issue (mandated `## Context / ## Issue / ## Location /
## Expected Fix / ## Acceptance Criteria`), adds itself as
depending on each fix, dispatches them, and **exits without
waiting**. The stranded scan re-dispatches the gate to a *fresh*
worker once unblocked, which re-runs every review step from the
top. Role separation is enforced by instruction — *"Do NOT
attempt to fix issues yourself. Your role is gate review, not
implementation"* — and the bias is stated: *"err on the side of
filing a fix bead. False positives waste less time than missed
defects."*

**Other patterns worth stealing.** Branch names as load-bearing
metadata (the bead id is parsed out of them, hence an emphatic
never-reuse rule). A Bors-style batch-then-bisect merge queue
where *nobody merges themselves*. Per-rig machine gates as JSON
with a `phase: "post-squash"` field and `retry_flaky_tests`.
`gt seance` — querying a *previous* session to recover its
decisions. Escalate-never-block. A `rule-of-five` formula
encoding "LLM agents produce best work through 4–5 iterative
refinements" (draft → correctness → clarity → edge cases →
excellence). And an anti-pattern catalogue reading as scar
tissue: "THE IDLE POLECAT HERESY" (agent finishes and waits for
approval — "**There is no approval step**"), spawn storms from
missing explicit closes, and a wisp-economics heuristic — *"If
you would curse losing the progress after a crash, set `pour =
true`."*

**Caveat to note, not adopt.** gastown's shipped polecat role
TOML sets `start_command = "exec claude
--dangerously-skip-permissions"`. That is their autonomy posture,
not a recommendation for us: mAId's safety floor (`project.md`,
kdevkit §7) runs the other way, and any decomposition we ship
should make dispatched agents *narrower* in authority, not
broader. Flagged because their orchestration patterns are
attractive and this rides along with them.

### BMAD — the strongest statement of our thesis, and its reversal

BMAD matters for two opposite reasons: its **v4** line states the
fresh-context-per-phase rule more explicitly than any other
source, and its **v6** line *reversed* it. Both are evidence.

**v4 — the verbatim rule** (`bmad-core/data/bmad-kb.md`):

> **CRITICAL CONTEXT MANAGEMENT**:
> - **Context windows matter!** Always use fresh, clean context
>   windows
> - **Model selection matters!** Use most powerful thinking model
>   for SM story creation
> - **ALWAYS start new chat between SM, Dev, and QA work**

The loop is `SM (New Chat)` → human approves → `Dev (New Chat)`
→ `QA (New Chat)` → human verifies → repeat, "Only 1 story in
progress at a time." Cadence is **per story**, not per session:
"instruct the agent to compact the conversation and start a new
conversation with the compacted conversation as the initial
message. Do this often, **preferably after each story is
implemented**."

Two *distinct* rationales are given, and separating them matters
for us:

> - **Context Optimization**: **Clean chats = better AI
>   performance**
> - **Role Clarity**: **Agents don't context-switch = higher
>   quality**

The second is not a context-length claim — it is **role bleed**,
and it does not go away as context windows grow. Our four phases
are role transitions (planner → implementer → reviewee →
closer), so this argument applies to us independently of token
budgets. Notably the rule relaxes where the tool provides
isolation natively: "**Roo Code**: Switch modes within the same
conversation" — i.e. the mandate is a *proxy* for isolation, not
the goal itself.

**The self-contained handoff artifact.** v4's story file is the
handoff bus, and the design intent is stated flatly in
`templates/story-tmpl.yaml`:

> Put enough information in this section so that the dev agent
> should **NEVER need to read the architecture documents**

Reinforced in the Dev agent itself: "**Story has ALL info you
will need** … **NEVER load PRD/architecture/other docs files**
unless explicitly directed." The README calls this
"**Context-Engineered Development**" and claims it "eliminates
both planning inconsistency and context loss."

Three mechanisms make that a handoff rather than telephone, and
**all three are load-bearing** — drop one and it degrades:

1. **Extract-and-cite, never invent.** "This section MUST
   contain ONLY information extracted from architecture
   documents. **NEVER invent or assume technical details.**"
   Every detail carries `[Source: architecture/{file}.md#
   {section}]`, and absent guidance must be stated explicitly as
   "No specific guidance found in architecture docs."
2. **Per-section write ACLs.** Every story section declares
   `owner` and `editors` — the SM owns Acceptance Criteria, dev
   may only tick boxes in Tasks, `dev-agent-record` is dev-only,
   `qa-results` is QA-only. Enforced by hard constraints in each
   agent file ("you are ONLY authorized to update the 'QA
   Results' section").
3. **Halt on ambiguity.** The dev halts on "Ambiguous after
   story check" and on "**3 failures** attempting to implement or
   fix something repeatedly" — a concrete loop-breaker
   threshold, and the escape hatch that keeps self-containment
   honest instead of licensing guesswork.

The loop also closes *backwards* through artifacts, not
conversation: the SM's first act on the next story is reading the
previous story's Dev/QA record ("Implementation deviations…
Challenges encountered and lessons learned"). That is what makes
the fresh-chat rule affordable.

**Lean-agent discipline** is v4's principle #1 ("Dev Agents Must
Be Lean"): "**Save context for code** — every line counts";
"Small files, loaded on demand"; "prefer multiple small tasks
over one large branching task… **This keeps context overhead
minimal**." Its `devLoadAlwaysFiles` is the always-on budget,
with a strikingly good anti-bloat heuristic: "As your project
grows and the code starts to build consistent patterns,
**coding standards should be reduced to include only the
standards the agent still needs enforced.** The agent will look
at surrounding code… to infer the coding standards."

**v6 — the reversal, and why it doesn't refute us.** v6 collapsed
the multi-agent relay into one long-running agent, and the new
constraints run *opposite* to v4's advice:

> `Absolutely DO NOT stop because of "milestones", "significant
> progress", or "session boundaries". Continue in a single
> execution`

Rationale: "**Human attention is the bottleneck.** `bmad-build`
rebalances that tradeoff. It trusts the model to run unsupervised
for longer stretches, **but only after the workflow has created a
strong enough boundary to make that safe.**"

Read carefully, that last clause is not a repudiation of
isolation — it says a *strong approved spec boundary* is what
buys long autonomous runs. And v6 keeps isolation where it
matters most: "**Context-free subagents are a cornerstone of the
review design.**" So the idea migrated from *manual fresh chats*
to *spawned clean subagents* — which is the mature form, and
precisely the primitive kdevkit already has.

Also gone in v6: **document sharding is removed** ("The
`bmad-shard-doc` and `bmad-index-docs` utilities have been
removed"), replaced by `compile-epic-context.md`, which
synthesizes one briefing at a hard budget of **"800–1500 tokens
total."** That is a shift from *mechanically split the doc* to
*synthesize a briefing* — relevant to our open question about
where the handoff record lives, and it favours synthesis over
sharding.

Deterministic state replaced conversational continuity too:
`sprint-status.yaml` is "the single tracking artifact the whole
dev cycle reads and writes," and its parsing/ordering/merging is
done by a **Python script, not inference**, because "Parsing epic
files, deriving story keys, ordering entries, merging with an
existing status file, and counting statuses are **not judgment
calls — so they aren't done by inference**." Merge rule:
"advanced statuses are preserved, never downgraded." That is a
direct, independent endorsement of our backlog's option 3
(code-driven transitions), and it draws the line in the right
place: **code owns the mechanical part, prose owns the
judgement.**

**v6's context theory — the sharpest anti-bloat argument found.**
`docs/explanation/project-context-theory.md` opens on "an
uncomfortable finding: **most documentation written *for* AI
agents makes them worse**," and its three claims bear directly on
our ~1250-line file:

> 1. **LLM-generated context documents measurably degrade agent
>    performance** … a paraphrase is worse than the code: it
>    drops detail, **it drifts the moment the code changes**, and
>    the agent trusts it instead of looking.
> 2. **Every line of always-loaded context is paid for in every
>    session.** A 2,000-line context file is not "thorough" — it
>    is a tax on every future task.
> 3. **Wrong context is worse than no context.** An agent with no
>    documentation explores and finds the truth. **An agent with a
>    stale document confidently follows it off a cliff.**

The operational test — **the pruning test** — is the method the
shrink backlog asked for and couldn't name: *"would removing this
line change agent behavior?* If an agent would do the right thing
anyway — because the code shows it, or because it is the
ecosystem default — the line is noise." Audits must "end with the
context smaller or equal, **never larger**." Entries carry trust
status (`verified` | `generated`) so inference is "**never
laundered into fact**." Framing: "**Context is a liability to be
re-earned.**"

Their review-scope rule is also worth taking, given we are about
to multiply reviewers: agentic reviews "derail the current change
by surfacing unrelated issues and turning every run into an ad
hoc cleanup project," so incidental findings get deferred to a
`deferred-work.md` — "It is usually better to misjudge some
findings than to flood the human with thousands of low-value
review comments. The system is optimizing for **signal quality,
not exhaustive recall.**"

And a failure-layer diagnosis rule that maps onto our four
phases: "If the implementation is wrong because the intent was
wrong, patching the code is the wrong fix. If the code is wrong
because the spec was weak, patching the diff is also the wrong
fix." Regenerate from the layer where the failure entered.

**Caveats worth recording honestly.** BMAD's context claims are
asserted, never benchmarked — v4's "clean chats = better AI
performance" and v6's "measurably degrade" cite no study or
numbers. And v6 provides **no migration note** explaining why the
mandatory fresh-chat rule was dropped; v6 also silently abandoned
v4's separation of duties (the SM persona — "You are NOT allowed
to implement stories or modify code EVER!" — is gone, with
planning and review triggers folded onto the implementing agent,
undefended in the docs). So BMAD is strong evidence for the
*mechanisms* and weak evidence for the *magnitudes*; Anthropic's
context-rot material is the better citation for the latter.

## Analysis — mapping findings onto kdevkit

### Finding 1 · The seams we already have are the right ones

The decomposition proposed in the brief is not a new invention; it
is the seam every peer tool uses. Two independent confirmations:

- spec-kit's command sequence (`specify` → `plan` → `tasks` →
  `implement`) sits almost exactly on kdevkit's planning-phase
  interviews and dev loop.
- The repo's own **fixtures already split this way** —
  `kdevkit-planning.smoke`, `kdevkit-dev-loop.smoke`,
  `kdevkit-closure.smoke`, `kdevkit-agents-md.smoke`. The test
  suite is pre-shaped for the refactor's A/B evidence, which
  answers the shrink backlog's hardest open question ("how is the
  refactor verified?").

The brief's phase list refines the current three-phase model by
splitting dev: **plan → agentic dev → human code review →
closure**. That fourth phase is real and currently under-modelled —
§7's Review Briefing + Agent-dev Review Gate is where a human
enters and iterates, and it is the one phase whose loop-backs are
driven by an external party rather than by the agent's own gates.

### Finding 2 · Isolation is the missing half, not the split

kdevkit already defers content (`setup.md`, `interviews.md`) and
already dispatches fresh-context sub-agents in three places (the
§2 structural verify, the §7 Code Review Gate, the §7 briefing
generator). The primitive exists and is proven in-repo.

What is missing is applying it to the *phases* rather than to
individual checks. Every peer that solved drift did so by making
the phase boundary a context boundary.

### Finding 3 · Reviewer plurality has a precedent in this very repo

The kreviewkit feature already established the extension mechanism
this feature needs for pluggable reviewers, and `project.md`
codifies it as an architectural rule:

> the caller never names a specific skill … and the filler owns
> its own invocation contract … A caller that dispatches another
> skill also owes it a **safety floor** — limits the dispatched
> skill cannot widen by asking.

So "let someone add their own reviewer" should be **role
advertisement + a reviewer list in the `kdevkit` block**, not a new
bespoke plugin system. `code_review.reviewer` already accepts a
`<ref>` grammar (`host-native`, `skill:<name>`,
`mcp:<server>.<tool>`, `agent:<name>`) — the change is
**singular → plural**, plus verdict aggregation.

Prior decisions to respect rather than re-litigate (from
`kdevkit-code-review-gate.md`, 2026-06-01):

- Default reviewer stays `host-native`; no mAId-shipped generic
  reviewer was wanted *at that time* — but the rationale was "we're
  unsure what a good generic reviewer looks like," with an explicit
  "revisit if host-native proves consistently weak." **The
  2026-07-15 miss is that evidence**, so the revisit is now
  licensed. The multi-reviewer skill the brief asks for is the
  revisit.
- Gate placement (between Test and Push) and the full loop-back
  (Quality → Test → Review) stay.
- Excluding the feature spec from reviewer context stays — it is
  the point of the gate. The Rule C fix (pass the *authoring
  conventions*) is compatible: conventions are not feature context.
- Single-knob `authority` (hard-stop / soft) shipped deliberately
  over richer per-category semantics. A panel reopens this,
  because per-reviewer severity is exactly the "critical-category
  override" that was deferred to backlog.

### Finding 4 · Sub-session ordering is a kaimux gap, not just prose

The brief's third point (sub-sessions ordered under a master
session) has a concrete blocker: **`kaimux`'s `Session` struct is
flat.** It is keyed by `pane_id` with no parent/child field, and
the kaimux spec explicitly decided "There is no extra parent
process, no signal forwarding" for the wrap path.

So making feature sub-sessions visible as children of a master
session is a kaimux feature (a parent/lineage field + dashboard
grouping), not only a kdevkit prose change. The oh-my-* Team Mode
tmux focus+grid layout is the precedent for what that dashboard
looks like. This is a strong candidate for a **separate stream**.

## Folded-in backlogs

| Backlog | Disposition |
|---|---|
| `kdevkit-refactor-shrink-always-on-context` | **Core of this feature.** Its option 2 (split by tier + stage) is the drift fix; option 3 (code-driven transitions) is the sub-session mechanism; option 1 (compress prose) applies per-file after the split. |
| `kdevkit-dev-loop-vmodel-and-ceremony` | **Folds in.** Rule A (ceremony lanes) gates how much of a phase context loads — it becomes the dispatcher's routing input. Rule C (authoring-convention rubric to the reviewer) is subsumed by the reviewer panel: the rubric becomes one panel member. Rule B (test-first per slice) rides in the dev-phase file. |
| `kdevkit-durable-facts-to-repo-not-agent-memory` | **Folds in** — a cross-cutting rule that must survive decomposition, so it lands in whatever stays always-on. Decomposition raises its stakes: more agents = more places to leak a fact into private memory. |
| `kreviewkit-playback-layer-unverified` | Adjacent; note only. The human-review phase is this feature's territory, so verify coverage there should be checked. |

## Open questions

<!-- To resolve during shaping, before the plan commits. -->

1. **What stays always-on after the split?** The shrink backlog
   flags §9's Conventional Commits, public-repo hygiene, and Review
   Gates as plausibly irreducible. Cursor's 500-line ceiling is a
   useful target for the residual.
2. **Prose-driven or code-driven phase transitions?** Backlog
   option 2 vs 3. Code-driven fixes the growth curve and removes
   read-order bugs, but risks the "skills are plain markdown
   symlinks, no runtime" deploy invariant, and must degrade
   gracefully when a user drives kdevkit without the wrapper.
3. **Where does the handoff record live?** BMAD's answer is a
   self-contained story file. Ours might be the feature spec
   itself, a per-phase section of it, or a new artefact — noting
   §6 explicitly refused to introduce a `research.md`, so the bias
   is against new artefacts.
4. **Panel aggregation.** How several reviewer verdicts become one
   pass/fail: min score, weighted, or per-reviewer authority with
   security hard-stopping regardless. Reopens the deferred
   critical-category override.
5. **Panel cost.** N reviewers per slice multiplied by
   `retry_budget` is a real token bill. Does the ceremony lane
   (Rule A) also scale the panel size?
6. **Does the review panel ship as one skill or several?** The
   brief says "a skill which uses multiple reviewers" — so one
   dispatching skill with several internal lenses, plus a
   project-extension point.
7. **Is the kaimux lineage work in scope**, or a sibling stream
   under a shared initiative?
8. **How is the whole thing A/B'd?** The four `kdevkit-*.smoke`
   fixtures tri-tool, before and after. Budget for it up front —
   the shrink backlog's rule: "a refactor that can't be A/B'd
   shouldn't ship."

## Session Log

<!-- Newest at top. -->

- **2026-08-03 · Analysis session opened.** Worktree +
  branch created. Read `SKILL.md` (1246 lines), `project.md`,
  and four related backlogs. Deep research on spec-kit, Cursor
  rules, Anthropic context engineering, and the oh-my-* harness
  family; gastown / BMAD / reviewer-extension research in
  flight. Recorded findings 1–4 above.

## Decision Log

<!-- append: decision · rationale · alternatives rejected -->

- **2026-08-03 · Analysis captured in the feature file from the
  start, not in chat.** Rationale: user asked for it explicitly,
  and it matches §6's "the spec on disk is what the user reacts
  to." No separate research artefact (§6 refuses `research.md`).
