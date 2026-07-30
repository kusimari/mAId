---
name: kdevkit
description: Spec-driven dev on a repo with specs/: plan or start a feature, run the dev loop through quality/test/review gates, close one out ("ship it", "close it", "feature done", "plan this", "add to backlog"), or record a durable project fact. Four tiers (project/initiative/feature/backlog); three-phase feature branch, one squash-merge.
version: 3.7.0
tags: [spec, feature, requirements, design, kdevkit, workflow, planning, backlog, initiative, public-repo]
---

# kdevkit — spec-driven development workflow

Three nested loops, with three phases on the feature branch:

```
project loop             ← project.md invariants. Cross-feature.
  initiative (optional)  ← groups multiple feature loops (§10).
    feature loop         ← one branch, three phases, one squash-merge.
      ├─ planning phase    plan(<feature>): commits + Review Gate
      ├─ dev loop          feat/fix/...: Quality → Test → Code Review → Push → [Briefing] → Review (§7)
      └─ closure phase     close(<feature>): reconcile + squash-merge (§8)
```

The branch carries all three phases on the same PR/CR; the body is
rewritten at each phase boundary. The §8 squash-merge collapses every
phase into one commit on `main`, preserving "one logical commit per
feature."

Four surfaces:

1. **Project invariants** — `project.md`. Mission, architecture,
   tech stack, layout, testing, deployment. Timeless;
   cross-feature.
2. **Initiative specs** — one file per multi-stream initiative
   under `initiative/<name>.md`. Why + ordered streams + status
   table. Time-bound (last-stream closure archives them). See
   §10.
3. **Feature specs** — one file per feature. Requirements,
   design, test strategy, implementation plan, session +
   decision logs.
4. **Backlog** — one file per wanted-future-work item.

Auto-detects the spec tree in `specs/`, `docs/specs/`, or
`.kdevkit/` (first hit wins).

The skill reads in session-arc order: §1–§2 set up context, §3–§4
enter a feature, §5 frames the run, §6/§7/§8 are the three phases,
§9 carries the always-on cross-cutting rules.

### Multi-file shape

This skill ships as three files under
`sources/skills/kdevkit/`:

- **`SKILL.md`** (this file) — always-on. Operational rules
  that fire every session: detect, entry cues, dev loop,
  closure, cross-cutting hygiene, initiative tier mechanics.
- **`setup.md`** — deferred. Templates and schemas that fire
  on **project genesis** or schema-drift verify: project.md
  template, six-section schema, `## Agent Development`
  block, code-review setup prompt, `## Active initiatives`
  index format, verify subagent return schema.
- **`interviews.md`** — deferred. Interview scripts and file
  templates that fire on **feature / backlog / initiative
  genesis**: four short feature interviews, feature file
  template, backlog item template, initiative file template,
  initiative interview shape, stream template-fill steps.

**Future-feature placement rule.** Operational content
(fires every session) belongs in `SKILL.md`. Setup
schemas and one-shot templates (fire on project / feature /
initiative genesis) belong in `setup.md` or `interviews.md`.
This split keeps the always-on context lean while preserving
correctness; future contributors should not drop new
templates into `SKILL.md`.

## 1 · Locate the spec tree

At session start, resolve `$SPEC_ROOT` by checking
`specs/` → `docs/specs/` → `.kdevkit/` (first hit wins). If
none exists and feature work begins, create `specs/`. Never
auto-migrate an existing `.kdevkit/` tree.

Within `$SPEC_ROOT`, four recognized subdirectories:
`feature/` (in-flight + completed feature records), `backlog/`
(wanted-but-not-now items), `initiative/` (multi-stream
initiatives — see §10), and `project.md` at the root.
Detection cue for the initiative tier: `$SPEC_ROOT/initiative/`
exists.

## 2 · Load project context

If `$SPEC_ROOT/project.md` exists, read it silently at session
start, then run the **structural verify** (next subsection).

If missing/empty and feature work begins, ask one question:

> _"Briefly describe this project — purpose, tech stack, and any
> hard constraints."_

Then **inline-Read `setup.md`** and follow its template +
first-time detection prose to write `$SPEC_ROOT/project.md`.

### Structural verify (verify-as-subagent)

A small structural check runs at session start to confirm
`project.md` matches the kdevkit schema without pulling the
schema narrative into main's context. **Main runs four
lightweight checks inline**; on any drift signal it dispatches
the subagent for full canonical-schema validation against
`setup.md`.

Main's inline checks:

1. The six required `##` headings are present and in fixed
   order (Mission, Architecture, Tech Stack, Layout, Testing,
   Deployment).
2. `## Agent Development > kdevkit > code_review:` is either
   present (with at least `reviewer:` set) or entirely absent
   (in which case the §4 Code-review setup prompt fires).
3. If a `## Active initiatives` index exists, every line
   matches an `$SPEC_ROOT/initiative/*.md` on disk and every
   on-disk initiative either has an index line or is archived.
4. The `code_review:` and `review_brief:` blocks (if present)
   parse as YAML with no unknown keys.

Clean → no further action. Any drift → dispatch a **fresh-
context agent call** (the same primitive §7 Code Review Gate
uses) with these inputs: the path to `project.md`, the path to
`setup.md`, and the on-disk listing of `$SPEC_ROOT/initiative/`.
The subagent loads `setup.md` and `project.md`, runs the full
canonical-schema validation, and returns:

```
{ "status": "clean" | "drift",
  "findings": [ { "section", "issue", "suggestion" }, ... ] }
```

Main applies any accepted findings via Edit. The setup
narrative never enters main's context — only the structured
verdict.

How the host translates "fresh-context agent call" is
host-specific (Claude Code's Agent tool, Kiro's equivalent,
Codex's CLI). Where unavailable, fall back to inline-Read of
`setup.md` and run the validation in main.

### Session-start read order

When the spec tree carries initiatives, the agent reads at
session start in this order: `project.md` → the **Active
initiatives** index → the current initiative (if the entry cue
references one or the current feature is auto-linked to one,
per §6) → feature(s) for the current branch. Read only the
referenced initiative(s); do not load the whole `initiative/`
tree unconditionally.

### Context layers & the AGENTS.md convention

kdevkit's context spans three layers, each with its own home —
the split the wider agent ecosystem converged on (AGENTS.md as
the operational file; a separate project-knowledge file — Kiro
steering, spec-kit constitution, memory-bank; per-feature specs):

- **Operational** → a repo-root `AGENTS.md` where one exists —
  build / test / lint commands, code style, PR/commit
  conventions. The tool-agnostic "README for agents," read by
  many coding agents beyond this skill.
- **Project-knowledge** → `project.md` — mission, architecture,
  tech rationale, constraints, and which test layer is
  load-bearing. The persistent *why* and *shape*; kdevkit's home
  tier. `project.md` is the project-knowledge layer, not an
  AGENTS.md variant.
- **Per-feature** → the feature spec — transient, alive only
  while the feature is in flight. §8 closure bubbles its durable
  content up into the two persistent layers.

**Detect, don't assume.** Read a repo-root `AGENTS.md` if
present (same first-hit-wins spirit as the spec-tree detection).
A repo whose operational context lives in `AGENTS.md` is worked
with as-is; do not force a `project.md` into being beside it, and
do not duplicate the same commands across both files.

**Never corrupt the AGENTS.md convention.** Anything written to a
repo-root `AGENTS.md` must still read as a normal, lean AGENTS.md
to any tool or human. Never write kdevkit-internal scaffold into
it — the fixed six-section headers, HTML-comment prompts,
Session/Decision logs, or the `## Active initiatives` index. That
scaffold stays in `project.md` and the spec tree. AGENTS.md holds
operational instruction, not methodology structure.

**Lean beats detailed.** Both persistent layers stay concise:
exact commands and explicit boundaries over vague prose, no
auto-generated bloat. Over-stuffed context files measurably
*degrade* agent performance and cost — the same compaction
discipline that keeps this skill's always-on context lean applies
to what the agent writes into `project.md` and `AGENTS.md`.

## 3 · Load feature context

Entry cues: `"let's start / continue / pick up <feature>"`, or a
branch like `feat/user-auth`. Initiative-tier cues:
`"start initiative <name>"`, `"show initiatives"`,
`"stream <n> for <initiative>"` — see §10 for what each does.

Resolve the entry mode for feature work:

1. **Continue / pick up `<feature>`** — look for
   `$SPEC_ROOT/feature/<feature-name>.md` (work-in-progress).
   Fall back to `$SPEC_ROOT/backlog/<feature-name>.md`; promote
   with `git mv` into `feature/` and start from the existing
   What/Why.
2. **Start `<feature>`** — if neither file exists, run the four
   interviews (§6) and write the spec.
3. **Stream `<n>` for `<initiative>`** — start a feature whose
   Git Setup names the initiative as its parent. Auto-populates
   the feature spec's `Part of initiative: [[<name>]]` link
   (§6). Otherwise behaves as a normal **start** entry.

**A spec on disk is not a reviewed spec** — when entering with a
populated `feature/<feature>.md`, start in §6 Planning (not §7
Dev). Apply §6's **Plan-commit rule** (numbered sequence): commit
→ push → open the Planning Review Gate → *then* wait for the
planning → dev cue. The single source of truth for the order
lives in §6 so it fires regardless of entry path.

In every entry mode — start, continue, or pick up — read
`project.md`'s `kdevkit.prefer_worktree` *first* and decide:
**prefer** → suggest a worktree (don't auto-run); silent →
branch-only without prompting. Then load other preferences from
the same `kdevkit` block (§4 covers this fully). The four
interviews in §6 only run when no spec is found.

### Backlog

When the user describes wanted-but-not-now work — an idea, a
frustration, a "we should eventually" — write it to
`$SPEC_ROOT/backlog/<item-name>.md` using the **backlog item
template** (inline-Read `interviews.md` for the template body
if not already in context). One file per item; never
consolidate into a single `FIXES.md` or `TODO.md`. Closure-time
cleanup of resolved items lives in §8 step 3.

Promoting backlog → feature: `git mv` into
`$SPEC_ROOT/feature/`, then fill Requirements / Design / Test
Strategy / Implementation Plan around the existing What/Why
using the feature file template (in `interviews.md`; see §6).

## 4 · Start feature session

One-time setup decisions on entry:

- **Worktree vs. branch.** Read `kdevkit.prefer_worktree`
  (§2); **prefer** → suggest a worktree at start (don't
  auto-run; parent-path conventions vary); silent →
  branch-only without prompting. Worktree status doesn't gate
  the §7 Review Gate; it only affects the §8 worktree-teardown
  offer.
- **Planning-phase opt-out.** `kdevkit.planning_phase: false`
  skips §6 entirely — spec edits ride with the first dev
  commit.
- **Code-review setup prompt.** If `kdevkit.code_review:` is
  missing from `project.md`, fire the prompt on session entry
  — firing on entry (regardless of fresh / continue / pick-up
  mode) keeps the prompt out of the dev loop, even though the
  §7 Code Review Gate is the only gate that reads the config.
  **Inline-Read `setup.md`** for the prompt's exact wording,
  the `[agent]:` heads-up note, and the sticky-write rules.
  After the user replies, sticky-write the answer to
  `project.md`'s `## Agent Development > kdevkit` block.
  `'skip'` is a valid answer (the prompt re-fires next
  session); `'default'` writes
  `code_review: { reviewer: host-native }`.
- **Other preferences load from the `kdevkit` block** — the
  full `code_review.*` block (`reviewer`, `threshold`,
  `authority`, `retry_budget`), the optional
  `review_brief.*` block (`enabled`, `reviewer` — §7 Review
  Briefing), plus review CLI, branch-cleanup, merge. Full
  resolution rules are in §7.

## 5 · Run feature session

```
planning   plan(<feature>):    spec only      → planning Review Gate (§6)
dev loop   feat/fix/...:       implementation → agent-dev Review Gate (§7)
closure    close(<feature>):   reconcile      → closure Review Gate (§8) → squash-merge
```

One branch, one PR/CR, three phases. Body rewritten at each
phase boundary; PR title rewritten to the dominant
`feat(<scope>):` subject at the Closure Review Gate so the
squash-merge commit on `main` reads as a feature ship.

### Phase-gating cues

Do not chain phases automatically. Two gating layers stack:

- **Interview phases** (within §6 Planning): requirements /
  design / test strategy / implementation plan. Stop after
  each; order is flexible.
- **Branch phases** (planning / dev / closure). Stop at each
  boundary. Each cue gates the *move* to the next phase, not the
  commits/pushes that precede the gate — those must already have
  happened. Cues:
  - **Planning → dev**: `"spec looks good"` /
    `"start build"` / `"plan approved"`. *Fires only after the
    Planning Review Gate is open — see §6 Plan-commit rule for
    the prerequisite sequence.*
  - **Dev → closure**: `"close it"` / `"ship it"` /
    `"merge it"` / `"feature done"`. *Fires only after the
    Agent-dev Review Gate is open — see §7.*

Both Review Gate greens close the inner loop; closure (§8)
requires the explicit cue.

### Operational gating

These fire during phase execution to influence boundaries
(distinct from §9's cross-cutting hygiene):

- **Assumptions within a phase.** Ambiguous → present a brief
  plan and wait for approval. Clear path → act.
- **YOLO mode.** `yolo` drops the phase gate + assumption
  plan for the session; `yolo off` reverts.

### Keep the feature file current

Update `Session Log` / `Decision Log` after each unit of
work; don't batch. When the Implementation Plan uses
checkbox shape (`- [ ]`), tick the corresponding box to
`- [x]` in the same commit that completes the slice — the
ticked spec is part of the dev commit, not a closure-time
sweep. §8.1 reconcile is the safety net for slices ticked
late or missed; the live discipline lives here.

### Initiative-stream auto-link

When this feature is a stream of an active initiative, §6
Planning auto-populates the `Part of initiative: [[<name>]]`
line in the feature spec — see §6 (and §10 for what counts as
active and how matching resolves).

## 6 · Feature planning

Trigger: a populated spec lacks the user's review (§3
spec-already-drafted rule), or `<feature>` is being started
fresh.

### Four short interviews

**Ground first.** Before opening interview 1, read
`project.md`, scan related feature specs in
`$SPEC_ROOT/feature/`, survey the corners of the
codebase the feature touches, and survey what the language /
ecosystem already offers for the problem (see "Reach for what
exists" below). The interview is calibrated
to what's there now, not the user's recollection. Findings
worth keeping land in the Session Log as work progresses;
the grounding step does not introduce a new artefact (no
`research.md`).

When entering a feature with no spec on disk (start mode), run
four short interviews in fixed order — Requirements → Test
Strategy → Design → Implementation Plan. Tests sit immediately
after requirements so success criteria are declared before the
design converges; the dev loop (§7) then has a verifiable
target, not a sketch to validate after the fact. Skip topics
existing project context already answers.

**Inline-Read `interviews.md`** for the interview-by-interview
prompt shape and the feature file template body. After the
four interviews, write the feature spec, then return here for
the Plan-commit rule.

**Answer the interviews yourself from the grounding, and write
the file.** The interviews are *your* checklist for what the spec
must cover, not a questionnaire to hand the user. Draft each
answer from `project.md`, the backlog item, and the code you just
read, then **write `$SPEC_ROOT/feature/<feature>.md` before you
ask the user anything.** The spec on disk is what the user reacts
to — a list of questions is not a reviewable artefact, and neither
is a set of interview answers in chat.

Ask only what you genuinely cannot infer, and ask it *in the
spec*: record the open question in the Session Log or inline, and
carry on. A single blocking question is warranted only when
proceeding either way would waste the work (§5's ambiguity rule);
"what should the flag be called" is not that. If the user's
request already says to do the file work, treat any urge to
open with clarifying questions as the ordering mistake the
Plan-commit rule warns about.

### Requirements smell test (always-on)

The spec's three top sections pair with the project's test
layers in V-model fashion:

- **Feature Brief** = the *capability* (what the user can
  now do).
- **Requirements** = the *experience* (what the user
  touches and observes) — verified by **functional /
  integration tests**.
- **Design** = *how it's built* (schemas, plumbing,
  libraries, project conventions) — verified by **unit
  tests**.

Functional/integration tests are pinned to Requirements so
they assert in user-observable terms; unit tests are pinned
to Design so they assert design primitives. That pairing is
the *why* behind the smell test below — a Requirements
bullet that names internals can't be verified by a test
phrased in user-observable terms, so it's in the wrong
section.

Before writing each Requirements bullet, check it against
the smell test; move violators to Design.

A Requirements bullet belongs in Design if it names any of:

- A library / framework name, or any third-party tool the
  user doesn't invoke directly.
- A file path / config key / data shape the user doesn't
  see in the surface they interact with.
- A function / class / trait / type / schema name from the
  implementation.
- An internal subcommand, hook event name, or protocol verb
  that's not part of the user-facing surface.

The discipline generalises across feature types — a CLI
feature (experience = flags and output), an app feature
(experience = screens and visible state), a skill change
(experience = the cues the agent recognises and the
artefacts it produces), a service endpoint (experience =
request shape and response).

The discipline is guidance, not rigid form. `interviews.md`'s
template and prompts are best-practice scaffolding; the
spec's exact section layout adapts to the feature. The
strictness lives in the **gates** — Planning Review (§6),
Agent-dev Review (§7, loops freely with Code Review), and
Closure Review (§8) — not in the heading shape.

### Reach for what exists (design-time, always-on)

A design move, not a coding-style preference, and the mirror
of the smell test above: the smell test keeps library names
*out* of Requirements; this puts the *right* library *into*
Design. Before deciding *how* a non-trivial piece of work is
built, **survey what the language / ecosystem already offers
and name the well-known library or idiom that already does
the job** — "load YAML with the established parser into a
typed struct," not a hand-rolled frontmatter parser; a known
filesystem-walk crate, not a hand `read_dir` recursion.

The justification is **inherited expertise** — a battle-tested
dependency encodes vetted edge cases and community practice
the agent would otherwise re-derive badly — **not** DRY.
"Shorter code" is the wrong reason; "someone already solved
this correctly" is the right one.

Guard: **well-known *and* earns its weight.** Don't pull a
new or heavy dependency for a trivial job a few honest lines
or an already-present import handle; weigh the dependency
against the hand-roll and say so when the hand-roll wins (a
lightweight direct call can beat a heavyweight dep). The rule
is language-agnostic — "the idiom *this* language / codebase
speaks," never a fixed per-language library list.

For load-bearing design choices, record the alternative
weighed in the Decision Log ("considered X; chose Y
because …"). Recommended, not mandatory for every helper.

### Initiative-stream auto-link

When the feature being started is a stream of an active
initiative (the initiative's Streams list names this feature's
branch or feature-spec basename — see §10), §6 Planning
auto-populates the `Part of initiative: [[<name>]]` line in
the feature spec, immediately after `## Feature Brief`. No
prompt; the link populates silently when the match is
unambiguous. If two or more active initiatives reference the
same name, ask one line to disambiguate.

### Plan-commit rule

The populated spec must reach the user as a reviewable artefact
before any code work begins. Order matters:

1. Finish the four interviews and write
   `$SPEC_ROOT/feature/<feature>.md`.
2. Confirm readiness with the user; iterate on the spec if
   needed.
3. **Commit** the spec as `plan(<feature>): initial spec`.
4. **Push** the feature branch.
5. **Open the Planning Review Gate** (PR/CR with the
   phase-specific body shape — see below).
6. **Then** wait for the planning → dev cue (§5).

The cue gates the *move* to dev — not the planning commit. The
commit + push + review must happen first so the user has
something concrete to react to. Reversing this order (waiting
for the cue before committing) is the most common ordering
mistake — a planning agent can read "confirm readiness" as the
exit-from-planning cue and stop there. It isn't. Steps 3–5 are
the artefact; step 6 is the gate after the artefact exists.

This rule is the single source of truth for both planning entry
paths — fresh-from-interviews and spec-on-disk (§3); §3 cites it
rather than duplicating.

Skip steps 3–6 if `planning_phase: false` (§2) — spec edits ride
with the first dev commit.

### Planning Review Gate

Fires after the `plan(<feature>):` push. Apply §9 Review
Gates. Phase-specific body content: **Spec summary**
(R / T / D / I one-liners) + **Open questions**.

## 7 · Dev loop

Apply after any coherent unit of implementation work. The
loop runs autonomously between gates — no per-step prompts.

### Write for intent (dev-time, always-on)

The dev-time mirror of §6's "Reach for what exists": that rule
finds the library at design time; this wires the code at dev
time. **Frame each function around what a caller would say it
does**, then wire the logic in the shape the language and
surrounding codebase already speak — defaulting to functional
/ fluent (chains, iterator combinators, library calls) over
hand-rolled mutable state machines **when that reads more
clearly as the intent**. **Reach first for what's already in
reach** — stdlib, an existing dependency, an already-imported
helper — over re-deriving equivalent logic, and **match the
surrounding code's conventions** rather than importing a
foreign style.

Legibility is the goal, **not dogma**: don't force a fluent
chain where a plain loop or a typed pattern-match is the
honest, clearer tool, and don't refactor working code between
equivalent forms without a readability or correctness gain.

**Comments carry intent, not history.** A comment states the
present-tense *why* a reader can't read off the code — the
non-obvious constraint, the gotcha — and stays terse. It does
not paraphrase the line below it, narrate the decision trail,
or retell the bug that led here; that history goes to the
commit / PR / Decision Log (§9 Conventional Commits draws the
same line — the commit carries *why we changed it*, the comment
carries *what it is now*). External references are a terse
pointer (`see project.md "<section>"`), not a retelling of the
source. Like the rest of this section it's a legibility default,
not a gate — the Code Review Gate may note a history-narrating
comment but doesn't hard-stop on phrasing.

### Re-pin on reactive change (always-on)

The altitude rituals in §6 (pin to `project.md`, survey what
exists, find the right owner, decide the experience before the
implementation) are gated on *phase* — they fire at planning. But
a change driven by CR/PR feedback or a verify finding, or any
mid-dev change the agent makes **reactively**, is just as much a
design decision; keying the check to *when* (planning) instead of
*what* (a design decision is being made) lets displaced design
slip through in fix-mode.

So: before writing a reactive change that **introduces, moves,
renames, or re-scopes a component, or alters a contract**, re-pin
with four quick questions —

1. **Owner.** Does `project.md` already name a layer / module /
   repo whose responsibility this falls under? Put it there, not
   at the point of failure.
2. **Altitude.** Is the fix at the right tier, or patching a
   symptom one level below where the cause lives?
3. **Reuse / idiom.** Does an existing mechanism already do this
   that the fix should extend rather than duplicate?
4. **Symmetry.** If the change adds an install / create / enable,
   is the inverse (uninstall / delete / disable) covered?

This is §6's "Reach for what exists" and the requirements smell
test re-fired on the feedback path. **Cost guard:** it's a few
lines of reasoning in the Session / Decision Log, not a phase
gate — a pure local fix (off-by-one, wrong string, missing guard)
doesn't trip it, and when the four answers are trivially "yes,
right spot" it leaves one log line and proceeds. **Scope limit:**
the check validates against the *project's own* design; it won't
surface ecosystem knowledge that lives only in external docs.

### Inputs · read commands from AGENTS.md → project.md

Resolve format / lint / type-check / test commands from the
operational layer first: a repo-root `AGENTS.md` where one exists
(§2 Context layers), then `project.md`'s Testing section, then §2
first-time detection. `project.md`'s Testing section carries the
layer semantics and which suite is load-bearing; the command
*strings* live in `AGENTS.md` when the repo keeps one, so the two
files don't duplicate them. The `kdevkit` block under `## Agent
Development` overrides defaults below (the full `code_review.*`
block — `reviewer`, `threshold`, `authority`, `retry_budget` —
the optional `review_brief.*` block, plus review CLI,
branch-cleanup, merge).

**Resolve any specific command** (review CLI, branch-delete,
merge, worktree ops) via implicit host knowledge → `kdevkit`
block → ask once and persist.

### Quality Gate

Deterministic checks only — anything subjective moves to the
**Code Review Gate** (below).

1. Run format; apply auto-fixes.
2. Run lint; fix until clean.
3. Run type-check (if applicable); fix all errors.

All three pass → Test Gate.

### Test Gate

Tests are part of the same iteration as the behavior change —
not a follow-up. When an implementation slice changes a behavior
the project's tests evaluate, the test update lands in the same
loop iteration, before the Code Review Gate. The §6 Test Strategy
maps each success criterion to a project test layer; the Test
Gate verifies them.

1. Run tests. All pass (zero failures, zero errors).
2. On failure: diagnose, fix, re-run. Default budget: **2**
   total attempts (initial run + 1 retry) — same semantics as
   the Code Review Gate's `retry_budget`. If still failing, stop
   and report.
3. If fixes were substantial, re-run the Quality Gate.

### Code Review Gate

A real code review run by a separate agent, on a green diff. The
reviewer is **not** the agent doing the implementation — it sees
a fresh context so feature-spec narrative doesn't bias the read.

**Resolve the reviewer.** Read
`kdevkit.code_review:` from `project.md` (§2). Defaults if
missing keys: `reviewer: host-native`, `threshold: 70`,
`authority: hard-stop`, `retry_budget: 2`. If the entire block
is missing, the §4 setup UX should already have prompted —
proceed with defaults if the user replied 'skip'.

**Dispatch contract.** The reviewer runs in a **fresh-context
agent call** — no feature spec, no session log, no in-progress
conversation history.

Receives:

- `project.md` (project invariants — every reviewer needs the
  architecture / hard-constraints / public-repo signal).
- The diff vs. base.
- The reviewer reference + threshold + authority + retry_budget.

Excluded:

- `feature/<feature>.md` (deliberately excluded — feature
  context is what we're trying to keep out).
- Session log / Decision log.
- Conversation history.

Reviewers that legitimately need feature context (e.g. "did the
implementation match the spec?") must ask for it themselves —
the contract default is "no feature-spec." This keeps the gate
honest about what it's reviewing: the diff against the project,
not the diff against the agent's own plan. How the host
translates "fresh-context agent call" is host-specific (Claude
Code's Agent tool, Kiro's equivalent, Codex's CLI); the
contract is portable.

**Returns.** A findings list + a 0–100 score.

**Score handling.**

- Score ≥ `threshold` → Push Gate.
- Score < `threshold` → loop back to start of Quality:
  1. Append findings (or a one-line summary, plus a reviewer
     URL where the host produces one) to the feature spec's
     Session Log so they're captured.
  2. Treat the highest-severity findings as the next
     implementation slice — apply "Re-pin on reactive change"
     (above) before writing the fix.
  3. Re-enter Quality Gate from the top.
  4. Re-run Test Gate.
  5. Re-run Code Review Gate.
  6. Repeat until score ≥ threshold or `retry_budget`
     exhausted.

Worst-case loop: `retry_budget` outer review cycles per slice
(default 2 — the count includes the first review attempt, not
retries on top of it). The Test Gate's own retry budget runs
inside each Test Gate invocation; it doesn't multiply the
review-cycle count, since Code Review only re-fires after Test
passes. After exhausting `retry_budget`, behavior splits on
**`authority`**:

- `hard-stop` (default) — refuse Push; surface findings to user;
  await explicit override.
- `soft` — allow a final Push with residuals appended to Session
  Log. Matches the older "fix once, proceed with residuals"
  softness for projects that prefer it.

### Push Gate

Only push after Quality + Test + Code Review pass (the latter
per `authority`).

### Comment-prefix convention

When the agent operates the CR/PR review surface under the
human's identity (the common case for host-driven review
CLIs that bind to the operator's account), both parties post
under the same author — the review tool threads by author,
not content, so review notes and agent replies land flat in
the timeline, indistinguishable.
The prefix gives a sequential, grep-able substitute for
threading without forcing a workflow change on either party.

The rule:

- **Every comment body the agent posts on the CR/PR starts with
  `[agent]:` on the first line**, followed by the comment
  content. No carve-outs by comment type — free-form replies,
  short status acks ("done", "fixed in `<sha>`"), and
  resolved-thread acknowledgements all get the prefix.
- The convention applies to **comment bodies only**: not the
  CR/PR description (no thread to disambiguate), not commit
  messages (already attributable via the Conventional Commits
  subject), not the diff itself.
- **Human side: prefix optional.** Bare comment bodies read as
  human. A human MAY use `[human]:` to mark a comment as a
  steer rather than a review note, but the skill does not
  require it. The prefix discipline is the agent's
  responsibility, not a symmetric convention.
- **Forward-only.** The convention applies to comments posted
  after this rule is adopted. Earlier comments on in-flight
  CRs stay un-prefixed and are read by their context — no
  backfill.
- **Travels with the actor, not the tool.** Other skills the
  agent invokes (e.g. project-specific reviewers, automated
  review-iterators) inherit the rule by being invoked by an
  agent already under it. kdevkit does not enumerate per-skill
  carve-outs.

Illustrative command shapes (tool-specific; the rule itself is
tool-agnostic):

```sh
# project-specific review CLI
<cli> reply -m '[agent]: applied fmt fix in 7a3c2f1; rerunning Test Gate'

# GitHub
gh pr comment <pr> --body '[agent]: applied fmt fix in 7a3c2f1; rerunning Test Gate'
```

The §4 setup-prompt blurb mentions this convention so a human
encountering kdevkit on a fresh project sees it at project
genesis.

### Agent-dev Review Gate

Fires after Push. Apply §9 Review Gates. Phase-specific body
content: **Approach** (bullets covering the changes).

**Refuse-on-fail.** A prior gate (Quality / Test / Code Review)
failed or noted residual issues → no review. Surface failure;
require explicit override.

### Review Briefing (dev → closure hand-off)

The §7 Code Review Gate serves the *agent*: it scores a diff
against the project, blind to the spec. This gate serves the
*human*: before they give the closure cue, they get a briefing
that plays the feature back and shows them where to look. The
two are complements — the gate checks diff-vs-project, the
briefing checks diff-vs-spec — and neither replaces the other.

**Where this runs.** This gate is **step 0 of the Agent-dev
Review Gate above**, not a separate phase: it fires after Push,
and its output is the body that gate submits. Read it as
"before opening or updating the PR/CR, get the briefing." The
Agent-dev gate's **Refuse-on-fail** rule applies unchanged — a
failed Quality / Test / Code Review gate means no briefing and
no review, since there is nothing green to brief.

**Opt-in.** Read `kdevkit.review_brief:` from `project.md`
(§2). Absent or `enabled: false` → skip this gate entirely;
kdevkit behaves as it always has. Only `enabled: true` fires
it. **Inline-Read `setup.md`** for the block's full key schema
and defaults.

**Dispatch a role, not a product.** Ask for **an independent
review-briefing tool** — do not hard-code which one. Resolve
in order:

1. `review_brief.reviewer` names it (`<ref>` grammar as
   §7's `code_review.reviewer`: `host-native` /
   `skill:<name>` / `mcp:<server>.<tool>` / `agent:<name>`).
2. Otherwise the single installed tool advertising that role.
3. Ambiguous or empty — several candidates, or none found →
   **ask once and persist** to `project.md`. Never guess, and
   never fall back to reviewing without the briefing silently:
   say the role couldn't be resolved and let the user decide.

**Contract.** The briefing tool runs in a **fresh-context
agent call** — it must not have written the code, and must not
see the implementer's conversation or session narrative.
Unlike the §7 gate it **is** given the spec, because
reconciling spec-vs-diff is the point. It gets: the feature
spec, the diff vs. base, `project.md` (+ `AGENTS.md` where
one exists), the Decision / Session logs, and the Test Gate's
report where one was produced (the captured Test Gate output,
or the Session Log entry recording it — where neither exists,
say so and let the briefing report coverage as unverified).
It reads the branch **read-only** — whole files, not just
hunks — and **changes nothing that is already on the branch**:
no edits, commits, pushes, or PR mutation. It may write only
its own briefing artefact, and only where the dispatcher names
a destination. Prefer dispatching it with read-only tools where
the host allows it.

**Returns** a human-facing briefing.

**The briefing MUST land on the PR/CR.** A briefing that stops
at the terminal has not been delivered — the review surface is
where the human reviews, so that is where the briefing belongs.
Whenever a briefing tool is used, its output becomes the PR/CR
**body**:

1. Dispatch **after Push, before this gate submits the PR/CR
   body**, so the briefing *is* that body rather than an
   overwrite of an Approach-only body a moment later.
2. The PR/CR normally already exists — §9 creates it at the
   first gate, which is §6 Planning — so the usual action is
   **rewrite the body in place** with the briefing. Where none
   exists yet, create it with the briefing as the body. Either
   way it stays one PR/CR per branch (§9); the body is
   rewritten, never appended as a comment.
3. The briefing's sections subsume §9's body shape — its
   playback carries **Why**/**Approach**, its focus map *is*
   **Reading order** — so it replaces that body rather than
   sitting beside it.
4. Apply §9's internal-marker grep to the briefing **before**
   submission: it is agent-authored prose entering a public
   surface, and it quotes freely from the spec and the diff.
5. Report the PR/CR URL to the user as the last line of the
   gate's output, and say the body is a review briefing so they
   know what they are being handed.

Where a host's review surface genuinely cannot be written to,
say so explicitly and put the briefing in front of the user
some other way — never drop it silently.

**At closure (§8.5), the briefing survives.** The Closure Review
Gate rewrites the *title* and adds **Verification**; it does
**not** replace a briefing body with an Approach-only one. Where
the closing diff moved on materially from what was briefed,
re-dispatch for a refreshed briefing rather than downgrading the
body.

The briefing informs the human's closure decision; it does
**not** gate it. `"close it"` remains the only trigger for §8,
and the briefing never carries an approve/request-changes
verdict — that is the human's call.

## 8 · Closure

Closes the **feature loop**. Trigger: an explicit cue —
`"feature done"` / `"close it"` / `"ship it"` / `"merge it"`.

The closure cycle reuses §7's **comment-prefix convention** for
any agent-authored CR/PR comments posted during reconcile or
the Closure Review Gate.

Steps 1–3 stage spec / docs / backlog edits as
`close(<feature>):` commits before the §8.6 squash; step 3
must be asked even when the answer is "none" — *asking is the
artifact*.

**1 · Reconcile in-flight markers.** Sweep
`$SPEC_ROOT/feature/<feature>.md`. Implementation Plan items
in checkbox shape: literal grep for `- [ ]` markers; tick to
`- [x]` if quietly done, or move out (backlog or follow-up
feature). Implementation Plan items in older prose-numbered
shape: read each and resolve. Then sweep open Decision Log
entries and unresolved questions the same way. The merged
spec is "done in place" — do not move directories. Stage
edits.

**2 · Persistent-layer verify (per touched section).** Closure
bubbles durable content up out of the transient feature spec into
the two persistent layers (§2 Context layers). For each
`project.md` section the feature touched — Mission, Architecture,
Tech Stack, Layout, Testing, Deployment, Hard constraints, Agent
Development — ask one targeted question: _"Did this feature change
what's documented under \<section\>?"_. Asking is mandatory;
declining the edit is fine. Stage any accepted edits.

**Operational changes go to `AGENTS.md`, not `project.md`.** If
the feature changed a build/test/lint command or another
operational fact and the repo keeps a root `AGENTS.md`, the edit
lands there (kept lean, per §2's convention) — `project.md`
Testing keeps only the layer semantics. **Binding decisions
bubble up as rationale:** a Decision Log entry that constrains
*future* features gets its *why* folded into the relevant
`project.md` section (not copied verbatim, not a standing
decisions log). Non-binding decisions stay in the feature spec's
Decision Log, archived in place with the feature.

Decide which sections were touched from the diff:

- **Tech Stack** — a dependency added/removed, or a runtime
  version moved.
- **Layout** — a top-level directory or file gained/lost,
  per project.md's tree.
- **Testing** — a test command added/removed, or a layer's
  semantics changed.
- **Deployment** — the deploy/install path or registry
  changed.
- **Architecture** — a documented moving part gained or
  lost a responsibility.
- **Mission** — meaningful shift in what the project is
  for. Rare.
- **Hard constraints** — a new invariant, or an old one
  weakened.
- **Agent Development** — a `kdevkit` (or other skill)
  block key changed, or a new skill-scoped preference
  landed.
- **AGENTS.md (operational)** — a build/test/lint command,
  code-style rule, or PR/commit convention changed, and the
  repo keeps a root `AGENTS.md`. The edit lands there, lean.

Untouched sections aren't asked about. The asking is the
artifact; the user can answer "no, project.md is fine"
for every touched section and closure proceeds.

**3 · Backlog cleanup (interactive).** List
`$SPEC_ROOT/backlog/`; ask: _"Which backlog items did this
feature close out? Pick any, or 'none'."_ `git rm` the chosen
ones; asking is mandatory even when the answer is "none".

**3.5 · Initiative Status update (auto).** If the closing
feature is a stream of an active initiative (the feature spec
carries `Part of initiative: [[<name>]]` near the top), update
the initiative's Status table row: branch, CR, status =
`shipped`, ship date, one-line learning. Stage the edit. If
this is the **last** stream (every other row in the Status
table is already `shipped`), the same staged edit also
archives the initiative spec — `git rm
$SPEC_ROOT/initiative/<name>.md` and remove the line from
`project.md`'s `## Active initiatives` index (the index is a
bullet list; the Status table is the per-initiative file). No
separate `close(<initiative>):` commit; the last stream's
`close(<feature>):` does the work. See §10 for the table
format.

**4 · Commit + push.** Staged closure edits land in one or
more `close(<feature>):` commits per §9. Push.

**5 · Closure Review Gate.** Apply §9 Review Gates. Body
rewritten to final shape; phase-specific content: **Approach**
+ **Verification** (required at close-out) + optional **Spec
& docs touched at close-out**. **Title rewritten** to the
dominant agent-dev subject (`feat(<scope>): subject` etc.) —
*not* the `close(<feature>):` subject — so the squash-merge
commit on `main` reads as a feature ship, not a closure
mechanic.

**6 · Squash merge to `main`** — one logical commit per
feature. Exceptions:

- Single-commit branch: squash and plain merge are equivalent.
- Branch with *several* logical features (rare): one squash
  merge per logical feature.
- Non-linear `main` by convention: squash still works; surface
  before going non-default.
- FF-only `main`: squash locally, then commit and push (review
  tool can't be the merger).

**7 · Branch cleanup.** Delete the feature branch local +
remote; prune stale refs. Default delete, one line, no
permission pause.

**8 · Worktree teardown — offer-only.** Non-primary worktree →
surface path and offer removal. Do not auto-remove — artifacts
may be worth inspecting.

## 9 · Cross-cutting rules (always-on)

These fire at every phase. Operational gating (YOLO,
ambiguous → plan) lives in §5 — not here.

### Conventional Commits

`type(scope): subject` — imperative mood, lowercase, no
trailing period; subject ≤ 72 chars. Body (when present)
explains *why*; the diff is authoritative for *what*.

Existing types: `feat` · `fix` · `chore` · `docs` · `refactor`
· `test`. Three extras encode tier-specific phases:

- **`plan(<feature>):`** — feature-planning-phase. Touches only
  `specs/feature/<feature>.md` (rarely `specs/backlog/` on
  promotion). No code edits.
- **`close(<feature>):`** — feature-closure-phase. Reconciles
  in-flight markers, applies any `project.md` verify edit,
  `git rm`s resolved backlog items, updates the parent
  initiative's Status table (§8.3.5) and archives it on
  last-stream close. No code edits — drift goes back to the
  dev loop.
- **`plan(<initiative>):`** — initiative-planning. Authors
  `$SPEC_ROOT/initiative/<name>.md` and adds the
  `## Active initiatives` index entry to `project.md`. No
  code edits. There is no `close(<initiative>):` type — the
  last stream's `close(<feature>):` archives the initiative
  spec (§8.3.5).

The §8.6 squash-merge collapses every phase into one commit on
`main`; the type encodes the on-branch narrative, not the
on-`main` shape. CI-restricted projects may substitute
`docs(spec/<feature>):` and `chore(close/<feature>):`; document
in the `kdevkit` block.

Branch naming: `<type>/<short-description>` — `feat` · `fix` ·
`chore` · `docs` · `refactor` · `test`.

### Author identity

**No `Co-Authored-By` trailer.** Commits are authored by the
user; the assistant is not a co-author.

### Working state

Every commit leaves the repo working. Commit per coherent unit;
don't batch to end-of-feature. New commits, never amends,
unless the user explicitly asks.

**Cross-stream rebase carve-out.** New commits, never amends,
except the cross-stream rebase covered in §10 — the only place
this rule yields.

### Review Gates

Universal CR/PR contract — applied by §6 Planning, §7
Agent-dev, and §8 Closure. Each gate adds only the
phase-specific content section + any per-gate exception.

- **Title.** `<type>(scope): subject` — Conventional Commits
  shape (above). The phase prefix (`plan(...)` / dev type /
  rewritten `feat(...)` at close) carries the phase signal
  across hosts.
- **Body.** **Why** (motivation, not file changes — the diff
  is authoritative for *what*) + *phase-specific content* +
  **Reading order** (grouped by phase: *Read for intent:* … ;
  *Read for contract:* … ; *Read for plumbing:* …).
  Optional: **Verification** (commands + results), **Pairs
  with** (cross-repo links).
  *Exception:* where §7's Review Briefing gate is enabled, the
  briefing it returns replaces this body at that gate — its
  sections already carry Why/Approach and the Reading order.
  The requirement is satisfied, not waived.
- **One PR/CR per branch.** Open as a normal review, not
  draft. Create on the first gate; update title + body on
  subsequent gates. Return the URL as the last line of phase
  output.
- **PR-ready** means Quality + Test + Code Review Gates pass
  locally (the latter per `code_review.authority`).
- **Squash merge** is the default close (§8.6). Keep PRs
  small — one concern per PR.

**Apply:** internal-marker grep on title + body before every
submission; commit hygiene (below) on every commit.

### Public-repo hygiene

Public projects must not leak internal names. Public-mode
signal: a `project.md` Hard-constraints bullet declaring the
repo public, or `git remote` on an obviously public host while
`project.md` is silent (treat as public until told otherwise).

When public, NEVER write internal names into skills / agents /
commands, anything under `specs/`, `project.md`, commit text,
or PR/CR bodies.

Internal names = product / team / ticket / CR / repo / store
names + internal emails. Ambiguous → ask, persist to Hard
constraints. Never silently strip; offer to file into a
corporate spec tree elsewhere.

**Internal-marker grep** — one source of truth, invoked from:

- §6 Planning Review Gate (title + body before submission).
- §7 Code Review Gate (diff before dispatch — the diff leaves
  the agent's controlled boundary on dispatch, so any
  internal-marker leak must be caught before, not after).
- §7 Push Gate (staged diff before push) and Review Gate
  (title + body before submission).
- §8 Closure Review Gate (title + body before submission).

In public-repo mode, any hit fails loud, surfaces lines, aborts.

### Commit hygiene

No commented-out code, debug prints, temp files, secrets, or
credentials in commits.

### Spec-discipline anti-patterns

These are the failure modes that survive the Quality / Test
/ Code Review gates because the gates check the diff, not
the diff's relationship to the plan. Fire proactively
during dev, not reactively at review.

- **No scope creep mid-dev.** The Implementation Plan items
  in the feature spec are the contract. If new work
  surfaces during dev, either add it to the Plan and
  confirm with the user, or move it to
  `$SPEC_ROOT/backlog/`. Don't silently expand the diff.
- **No unrelated refactor bundling.** One feature = one
  focused diff. Drive-by cleanups in unrelated files
  belong in their own feature or a `chore(<scope>):`
  follow-up — not bundled into the feature's commits.
- **No premature closure.** The closure cue (`"close it"`
  / `"ship it"` / `"merge it"` / `"feature done"`) is a
  hard gate. Quality + Test + Code Review passing is
  necessary but not sufficient; the explicit cue is
  required even when everything looks done.
- **No silent plan amendments.** Changes to the
  Implementation Plan after the Planning Review Gate
  opens warrant a one-liner in the Decision Log
  (rationale + what shifted). Reviewers see the
  original plan in the Planning Review commit; the
  closing diff should be reconcilable to that plan plus
  the Decision Log entries.

### Skill-file placement

Future kdevkit changes follow the multi-file split declared at
the top of this file: operational rules in `SKILL.md`;
templates and one-shot setup schemas in `setup.md` or
`interviews.md`. New always-on prose lands in `SKILL.md`; new
templates land in the appropriate deferred file with a
trigger from `SKILL.md`.

## 10 · Initiative tier

The fourth tier in kdevkit, slotted between project (timeless)
and feature (one branch). An initiative captures multi-feature
work that can't fit on one branch — the *why* plus the ordered
*streams* (each stream = one feature = one branch / CR /
squash-merge) that deliver it, plus a Status table updated by
each stream's closure commit.

Initiatives are time-bound: created when the multi-stream work
is identified, archived by the last stream's
`close(<feature>):` commit (§8.3.5).

### When to create one

Any time CR review or planning produces "this needs to land as
several CRs in order," the work is an initiative. The signal
is sequential dependency between branches, not just a large
feature. A large feature that can ship as one branch stays a
feature; only when the work has to fan out across multiple
branches in a defined order does it become an initiative.

### Initiative entry verbs

- **"start initiative `<name>`"** — write
  `$SPEC_ROOT/initiative/<name>.md` and populate
  `project.md`'s `## Active initiatives` index with a one-line
  entry. **Inline-Read `interviews.md`** for the initiative
  file template + the three short initiative interviews
  (Why → Streams → initiative-level Decisions). After the
  spec is written, commit as `plan(<initiative>): initial
  spec`, push, and open the Planning Review Gate per §6 / §9
  with phase-specific body content: **Why** + **Streams** +
  **Decisions taken at the initiative level**.
- **"show initiatives"** — list active initiatives from
  `project.md`'s index. Read-only; no commit.
- **"stream `<n>` for `<initiative>`"** — start a feature
  whose Git Setup names the initiative as its parent.
  **Inline-Read `interviews.md`** for the
  template-fill steps (which fields populate from the parent
  initiative's stream entry, which come from the four
  feature interviews). The feature spec's `Part of
  initiative: [[<name>]]` line auto-populates per §6;
  otherwise the flow is a normal §3 feature start followed
  by §6 Planning.

### Cross-stream rebase mechanics

When Stream `n+1` is in-flight and Stream `n` re-ships to
`main` after CR review:

1. From Stream `n+1`'s branch:
   `git fetch origin && git rebase origin/main`. Resolve
   conflicts in place.
2. Re-run §7 Quality + Test + Code Review Gates for the slice
   that intersects the rebased change. Threshold and
   retry-budget semantics unchanged.
3. Force-push: `git push --force-with-lease`. Only after §7
   reverifies — never push a rebased branch with stale gates.
   Plain `--force` is unsafe against concurrent pushes;
   `--force-with-lease` is the contract.
4. If the rebase substantially changed the diff (e.g. shrunk
   because Stream `n`'s changes are now in `main`), update the
   open CR/PR body so reviewers aren't reading against a stale
   summary.

This is the only place §9's "new commits, never amends" rule
yields — the sequential-stream contract requires rebasing.

### Working across repo shapes (guidance, not contract)

Tier definitions (project / initiative / feature / backlog)
are about *how* to work. *Where* the work lives is orthogonal:

- **Single-repo** (default): `$SPEC_ROOT = specs/` (or
  `docs/specs/`, `.kdevkit/`). All four tiers live here.
- **Multi-repo, per-repo specs**: each repo carries its own
  `specs/`. An initiative whose streams span repos is awkward
  — the initiative spec lives in one repo by convention; each
  cross-repo stream's feature spec lives in the repo where the
  stream's branch lives. Cross-repo references use
  fully-qualified paths or repo names.
- **Cross-repo program** (multiple repos under one umbrella):
  out of scope for kdevkit. A separate top-level "program"
  surface (in a workspace-level directory, not inside any one
  repo) is the right shape; the skill does not encode this.

The tier definitions are repo-shape agnostic; this guidance
shows how they map onto common shapes without baking
assumptions into the templates.

## Session Log

<!-- Newest at top. Observations during sessions; promote durable
     ones into the body above at wrap-up. -->
