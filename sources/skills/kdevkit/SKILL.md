---
name: kdevkit
description: Spec-driven development workflow — project invariants, feature specs, backlog. Three nested loops (project → feature → agent-dev) with codified close-outs. Three-phase feature branch (planning → agent-dev → closure) — each phase commits and reviews on its own; main stays single-commit-per-feature via squash. Phase gating, Conventional Commits, Quality/Test/Push/Review gates. Public-repo hygiene. Auto-detects specs/, docs/specs/, or .kdevkit/.
version: 2.3.0
tags: [spec, feature, requirements, design, kdevkit, workflow, planning, backlog, public-repo]
---

# kdevkit — spec-driven development workflow

Three nested loops, with three phases on the feature branch:

```
project loop      ← project.md invariants. Cross-feature.
  feature loop    ← one branch, three phases, one squash-merge.
    ├─ planning phase    plan(<feature>): commits + Review Gate
    ├─ agent-dev loop    feat/fix/...: Quality → Test → Push → Review (§8)
    └─ closure phase     close(<feature>): reconcile + squash-merge (§9)
```

The branch carries all three phases on the same PR/CR; the body is
rewritten at each phase boundary. The §9 squash-merge collapses every
phase into one commit on `main`, preserving "one logical commit per
feature."

Three surfaces, one per loop scope:

1. **Project invariants** — `project.md`. Mission, architecture,
   tech stack, layout, testing, deployment.
2. **Feature specs** — one file per feature. Requirements,
   design, test strategy, implementation plan, session +
   decision logs.
3. **Backlog** — one file per wanted-future-work item.

Auto-detects the spec tree in `specs/`, `docs/specs/`, or
`.kdevkit/` (first hit wins). Each inner loop's terminal step
re-enters the outer loop; the two close-outs (§8, §9) automate
the handoffs.

## 1 · Locate the spec tree

At session start, resolve `$SPEC_ROOT` by checking
`specs/` → `docs/specs/` → `.kdevkit/` (first hit wins). If
none exists and feature work begins, create `specs/`. Never
auto-migrate an existing `.kdevkit/` tree — `git mv` is a
human decision because it can touch CI wiring.

## 2 · Load project context

If `$SPEC_ROOT/project.md` exists, read it silently at session
start.

If missing/empty and feature work begins, ask one question:

> _"Briefly describe this project — purpose, tech stack, and any
> hard constraints."_

Then write `$SPEC_ROOT/project.md` from the template below.

### `project.md` template

Six sections, fixed order. The HTML comments are prompts — keep
them in place so future sessions re-read the intent.

```markdown
# Project: <name>

## Mission

<!-- What this project exists to do, and who it serves. One
     paragraph. Change only when the goal itself changes. -->

## Architecture

<!-- The logical shape: components, responsibilities, how they
     talk. Diagrams optional; words mandatory. Not a file-layout
     listing — see Layout for that. -->

## Tech Stack

<!-- Languages, runtimes, frameworks, key libraries. Versions
     only where version matters. -->

## Layout

<!-- Directory tree with a one-line annotation per entry. What
     lives where, not why. -->

## Testing

<!-- How this project is tested: unit, integration, smoke,
     manual. Which commands run which suite. Which are
     load-bearing vs. nice-to-have. -->

## Deployment

<!-- How code reaches users. Build, release, install, symlink,
     container, whatever applies. If the project isn't deployed
     in a traditional sense, describe how it's consumed. -->
```

### First-time `project.md` detection

When creating `project.md` from scratch, fill the Testing
section by:

- Probe ecosystem markers (`package.json`, `pyproject.toml`,
  `Cargo.toml`, `go.mod`, `Makefile`, `deno.json`, etc.) for
  the toolchain. Prefer CI files (`.github/workflows/*`,
  `.gitlab-ci.yml`) verbatim — they reflect what runs.
- Confirm in one batch with the user; don't drip-feed.
- Write Testing as prose, not a structured block; §8 reads
  commands out of the prose at run time.

### Optional `## Agent Development` section

`project.md` may carry an `## Agent Development` section
organised by skill name, for per-project config (score
thresholds, retry budgets, review-tool commands, etc.).
kdevkit reads this block when present; absent, it runs from
the Testing section's prose plus its own defaults.

Recognised keys under the `kdevkit` block:

- `prefer_worktree: true|false` — feature-start worktree
  recommendation (see §3).
- `planning_phase: true|false` (default `true`) — three-phase
  feature branch (planning → agent-dev → closure) per §6/§8/§9.
  Set `false` to revert to spec-bundled-with-code review.

## 3 · Load feature context

At feature-start (cues: "let's start / continue / pick up
<feature>", or a branch like `feat/user-auth`), do all four
of these checks before the first implementation slice — they
are parallel decisions, not optional:

1. **Locate or promote the feature spec.**
2. **Check `project.md` for a worktree preference.**
3. **Proceed under §6 phase gating.**
4. **Plan to commit the spec.** When the four-interview
   setup (or backlog promotion) yields a populated
   `specs/feature/<feature>.md`, that file becomes the first
   commit on the feature branch — `plan(<feature>): initial
   spec` — and ships through the §8 Planning Review Gate
   before any code work starts. Wait for the explicit cue
   ("plan the feature" / "ready to commit the spec") before
   pushing; phase gating still applies. Skip this step only
   if the project's `kdevkit` block declares
   `planning_phase: false` (§2).

Detail for each follows.

### 1 · Locate or create the feature spec

Derive `$SPEC_ROOT/feature/<feature-name>.md` (lowercase,
hyphenated).

- File has content → load silently and **start in the
  planning phase**, not agent-dev. Even when Requirements /
  Design / Test Strategy / Implementation Plan are all
  populated, confirm with the user that the spec is ready
  as-is or whether the planning round should iterate before
  any code is written. A spec being complete on disk does not
  mean it has been *reviewed*. The planning → agent-dev cue
  (§6) is the gate.
- File is missing → first check
  `$SPEC_ROOT/backlog/<feature-name>.md`. If a matching item
  is scoped there, promote it via `git mv` into `feature/`
  and start from its existing What/Why. Only run the
  four-interview setup if nothing is scoped in either
  location.

### 2 · Check `project.md` for worktree preference

Feature work defaults to a branch on the main checkout, but
some projects prefer per-feature isolation in a dedicated git
worktree so independent features don't contaminate each
other. The signal lives in either:

- An `## Agent Development` → `kdevkit` block declaring
  `prefer_worktree: true|false`, **or**
- A bullet under **Hard constraints** mentioning worktrees.

If the signal is **prefer**, suggest creating a worktree at
feature-start (don't auto-run — parent-path conventions
vary). If silent, continue branch-only **without prompting**.
Worktree status does not gate the §8 Review Gate; it only
affects the §9 worktree-teardown offer.

### 3 · Proceed under §6 phase gating

Phase gating governs the rest of the feature loop. See §6 for
the full rule; in summary: stop after each phase
(requirements / design / test strategy / implementation
slice), wait for explicit instruction before the next.

### Feature setup — four short interviews

One interview per topic. Use existing project context to skip
questions you already have answers to.

1. **Requirements.** What problem does this solve? Who interacts
   with it? What is the acceptance criterion?
2. **Design.** Technical approach, main components, how they
   interact. Call out trade-offs.
3. **Test strategy.** What kinds of tests (unit / integration /
   smoke / manual), which are load-bearing, what are the key
   scenarios.
4. **Implementation plan.** Ordered list of tasks + risk notes.

### Feature file template

```markdown
# Feature: <name>

## Git Setup

- Branch: <branch-name>
- Base: <commit-ish or branch>

## Feature Brief

<one paragraph — what this feature is and why it is being built>

## Requirements

<bullet list>

## Design

<technical approach, components, interactions>

## Test Strategy

<validation approach, key scenarios>

## Implementation Plan

<ordered task list with risk notes>

## Session Log

<!-- append entries as work progresses: date · what was done ·
     decisions made -->

## Decision Log

<!-- append entries for each significant choice: decision ·
     rationale · alternatives rejected -->
```

## 4 · Backlog

When the user describes something they want but not now — an
idea, a frustration, a "we should eventually" — write it to
`$SPEC_ROOT/backlog/<item-name>.md`. One file per item. Do not
consolidate into a single `FIXES.md` or `TODO.md`.

### Backlog item template

```markdown
# Backlog: <item-name>

## What

<!-- One paragraph: what this is, not how. -->

## Why

<!-- Motivation — what prompted the idea. Links to the
     conversation/incident if applicable. -->

## Open questions

<!-- Things that would need to be decided before this becomes a
     feature spec. Blockers, dependencies, unknowns. -->
```

Promoting a backlog item to a feature is a `git mv` into
`$SPEC_ROOT/feature/`, then filling in Requirements / Design /
Test Strategy / Implementation Plan around the existing
What/Why.

## 5 · Git practices (always-on in spec-driven repos)

### Branches

- `<type>/<short-description>` — `feat` · `fix` · `chore` ·
  `docs` · `refactor` · `test`.

### Commits

- Conventional Commits: `type(scope): subject` — imperative
  mood, lowercase, no trailing period; subject ≤ 72 chars.
- Body (when present) explains _why_, not _what_. The diff is
  authoritative for _what_.
- **No `Co-Authored-By` trailer.** Commits are authored by the
  user; the assistant is not a co-author.
- Every commit leaves the repo in a working state. Commits are
  save points — commit whenever a coherent unit of work is done;
  don't batch to end-of-feature.
- New commits, never amends, unless the user explicitly asks.

#### Phase-typed commits on the feature branch

Two extra types encode the feature-branch phase in the subject
line. Existing types (`feat` / `fix` / `chore` / `docs` /
`refactor` / `test`) are the **agent-dev** vocabulary,
unchanged.

- **`plan(<feature>): subject`** — planning-phase commits.
  Scope is the feature name. Touches only
  `specs/feature/<feature>.md` (and rarely
  `specs/backlog/<item>.md` when promoting). No code edits.
- **`close(<feature>): subject`** — closure-phase commits.
  Reconciles in-flight markers in the feature spec, applies
  any soft `project.md` verify edit, removes resolved
  backlog items. Code stays untouched here — drift found at
  closure goes back to agent-dev.

The squash-merge at §9.6 collapses every phase into one
commit on `main`; the type encoded in commits is the
on-branch narrative for reviewers, not the on-`main` shape.

Projects whose CI enforces a closed Conventional-Commits set
substitute `docs(spec/<feature>): plan — …` and
`chore(close/<feature>): …`; the phase signal in the subject
is what matters, the literal verb is secondary. Document the
substitution in the project's `## Agent Development` →
`kdevkit` block.

### Pull Requests

- Title: same `type(scope): subject` shape.
- Body: _why_ + approach.
- **Squash merge preferred.** Keep PRs small — one concern per
  PR.
- PR-ready means: Quality Gate + Test Gate both pass locally.

### Hygiene

- No commented-out code, debug prints, temp files, secrets, or
  credentials in commits.

## 6 · Session behaviour (always-on)

**Keep the feature file current.** After each meaningful unit
of work — a decision made, a subtask completed, an open question
answered — update the feature file's Session Log / Decision Log
before starting the next unit. Do not batch.

**Phase gating.** Do not chain phases automatically. Two
gating layers stack:

- **Interview phases** (within feature setup): requirements /
  design / test strategy / implementation plan. Stop after
  each; phase order is flexible.
- **Branch phases** (the three rounds on the feature
  branch): planning / agent-dev / closure. Stop at each
  boundary. The boundaries are gated on these cues:
  - **Planning → agent-dev**: `"spec looks good"` /
    `"start build"` / `"plan approved"` / equivalent.
  - **Agent-dev → closure**: `"close it"` / `"ship it"` /
    `"merge it"` / `"feature done"` / equivalent.

Within a phase, the agent-dev loop's Push + Review-open is a
single step — both gates green is the implicit approval to
close the inner loop and continue agent-dev iterations.
Closure (§9) is its own phase, entered only on the explicit
agent-dev → closure cue.

If `## Agent Development` → `kdevkit` declares
`planning_phase: false`, skip the planning phase entirely —
spec edits ride with the first agent-dev commit per the older
convention.

**Assumptions within a phase.** If input is ambiguous, present
a brief plan and wait for approval. If the path is clear, act
without a plan step.

**YOLO mode.** `yolo` → drop the phase gate and assumption
plan for the rest of the session. `yolo off` → revert
immediately.

**Feature completion.** Driven by §9. The project.md
update offer lives there as a soft step.

### Three-phase cheat-sheet

```
planning   plan(<feature>):    spec only      → planning Review Gate
agent-dev  feat/fix/...:       implementation → agent-dev Review Gate
closure    close(<feature>):   reconcile      → closure Review Gate → squash-merge
```

One branch, one PR/CR, one squash-merge. Body rewritten at
each phase boundary. PR title rewritten to the dominant
`feat(<scope>): subject` at the Closure Review Gate so
`main`'s squash-merge subject matches §5.

## 7 · Public-repo hygiene

Skills, specs, commit text, and PR/CR bodies in a public
project must not leak internal names. Public-mode signal:

- A bullet in `$SPEC_ROOT/project.md`'s **Hard constraints**
  declaring the repo public, **or**
- `git remote` resolves to an obviously public host while
  `project.md` is silent (treat as public until told
  otherwise).

When public, NEVER write internal names into `SKILL.md` /
`agent.md` / `command.md` bodies, anything under `specs/`,
`project.md`, commit messages, PR/CR descriptions, or
top-level docs.

"Internal names" is project-specific — at minimum: internal
product / team / ticket / code-review / repo / store names
and internal emails. Where ambiguous, ask and persist the
answer to `project.md`'s Hard constraints.

If asked to capture work mentioning internal names: note it,
surface the public-repo constraint, offer to file into a
corporate spec tree the user maintains elsewhere. Do not
silently strip names and write a partial capture.

The §8 Push Gate greps the staged diff for internal markers;
the Review Gate reruns the same grep against the prepared
PR/CR title and body before submission. In public-repo mode,
either hit fails loud and surfaces the matching lines.

## 8 · Quality → Test → Push → Review loop

Apply after any coherent unit of implementation work. Once an
implementation plan is approved, the loop runs autonomously
between gates — no per-step prompts. The four gates are the
agent-dev loop's body; the Review Gate is its terminal step.

### Inputs · read commands from `project.md`

Read `$SPEC_ROOT/project.md`'s Testing section for format /
lint / type-check / test commands. If ambiguous or missing,
ask the user once and offer to update `project.md`. If the
Testing section itself is missing, fall back to §2's first-
time detection.

A `kdevkit` block under `## Agent Development` in
`project.md` overrides the defaults below (score threshold,
retry budget, review-tool commands, branch-cleanup commands,
merge command).

**Resolution of any specific command this loop needs**
(review CLI, branch-deletion verb, merge, worktree ops,
etc.) — first hit wins:

1. Agent's implicit knowledge for the host (e.g. on a public
   host the conventional CLI is well-known).
2. `## Agent Development` → `kdevkit` block in `project.md`.
3. Ask once and offer to persist the answer into the block
   so future sessions skip the question.

### Quality Gate

1. Run the format command. Apply all auto-fixes.
2. Run the lint command. Fix all violations; re-run until clean.
3. Run the type-check command (if applicable). Fix all errors.
4. Self-review the diff against the base branch. Score your own
   work 0–100 for correctness, security, and adherence to
   project conventions. Default threshold: **70**.
   - ≥ threshold → proceed to Test Gate.
   - < threshold → fix highest-severity issues and re-review
     **once only**; if still below, proceed and note residual
     issues in the Session Log.

### Test Gate

1. Run the test command.
2. All tests pass (zero failures, zero errors).
3. On failure: diagnose, fix, re-run. Default budget: **2**
   fix-and-retry cycles. If still failing, stop and report —
   do not push.
4. If the fixes were substantial, re-run the Quality Gate.

### Push Gate

Only push after Quality + Test pass. Push the feature branch
to its upstream. Before pushing, run §7's internal-marker
grep against the staged diff; in public-repo mode, hit fails
loud.

### Review Gate

Fires after Push when both prior gates passed cleanly. Opens
or updates the PR/CR for this branch and returns the URL.
Same gate runs at each branch-phase boundary (planning /
agent-dev / closure); the body shape changes per phase, the
gate logic does not.

**Refuse-on-fail.** If a prior gate failed or finished with
residual issues noted, do not open a review. Surface what
failed and require an explicit override.

**1 · Body shape — per phase.** Title is `type(scope):
subject` (per §5).

- **Planning Review Gate** — fires after a `plan(<feature>):`
  commit is pushed. Title: `plan(<feature>): subject`. Body:
  **Why** (motivation) + **Spec summary** (Requirements /
  Design / Test Strategy / Implementation Plan, one line
  each) + **Open questions** (anything the planning round
  needs reviewer input on). PR/CR opens as a normal review,
  not draft — the `plan(...)` title prefix carries the phase
  signal across hosts that don't have a draft concept.
- **Agent-dev Review Gate** — fires after each agent-dev
  push. Title: `feat(scope): subject` etc. Body: **Why**
  (motivation, not file changes; the diff is authoritative
  for *what*) + **Approach** (bullets covering the actual
  changes). Suggested when warranted: **Verification**
  (commands + results), **Reading guide** (file order with
  compare-against hints), **Pairs with** (cross-repo links).
  Match §5's PR rule — don't impose more structure on small
  diffs.
- **Closure Review Gate** — fires after the `close(<feature>):`
  commits are pushed. **Title is rewritten** to the dominant
  agent-dev subject (`feat(<scope>): subject` /
  `fix(<scope>): subject` / `refactor(<scope>): subject`),
  *not* the `close(<feature>):` subject. Reason: hosts that
  inherit the PR title onto the squash-merge commit (e.g.
  GitHub) need `main`'s history to read as a feature
  ship, not a closure mechanic. Body: rewrite to final shape
  — **Why** + **Approach** + **Verification** + optionally
  **Reading guide** / **Pairs with** / **Spec & docs touched
  at close-out**.

**2 · Body grep.** Run §7's internal-marker grep against the
prepared title + body before submission. Hit → fail loud,
surface lines, abort. Fires at every phase — the planning
Review Gate is the *first* place it runs for a given
feature, catching internal names early before any
implementation work.

**3 · Update vs. create.** One PR/CR per branch. On the first
phase boundary (typically planning), create. On subsequent
boundaries, update — rewrite the body to the phase shape
above.

**4 · Return the URL** as the last line of phase output.

## 9 · Feature close-out loop

Closes the **feature loop**. Trigger: an explicit human cue —
"feature done" / "close it" / "ship it" / "merge it". Distinct
phase from the agent-dev loop; gated separately per §6.

Drives merge + cleanups + backlog reconciliation in one
sequence — no per-step prompts in the steady-state path. The
user's role stays at "approve the review" and "give the close
cue." Specific commands resolve via the order in §8 intro
(implicit → kdevkit block → ask + persist).

Steps 1–3 are spec / docs / backlog edits that get **staged
together** and land as `close(<feature>):` commits on the
feature branch *before* the squash-merge. Asking step 3 is
mandatory even when the answer is "none" — *asking is the
artifact*. This guarantees `project.md` updates and backlog
cleanup ride into the squash-merge as part of the one-logical-
commit-per-feature on `main`, instead of trailing as separate
post-merge commits.

**1 · Reconcile in-flight markers in the feature spec.** Sweep
`$SPEC_ROOT/feature/<feature>.md` for unchecked Implementation
Plan items, open Decision Log entries, unresolved questions.
Resolve in place or move out — to the backlog or a follow-up
feature. The merged feature spec is "done in place"; do not
move directories. Stage the spec edits.

**2 · Soft `project.md` verify.** Re-run §6's offer to update
`project.md` with what changed. Decline is fine; close-out
continues either way. Not a hard block. Stage any edits the
user accepts.

**3 · Backlog cleanup (interactive).** List
`$SPEC_ROOT/backlog/` contents and ask:

> _"Which backlog items did this feature close out? Pick any
> that apply, or 'none'."_

`git rm` the chosen ones. Unchosen items stay. Asking is
mandatory even when the answer is "none".

**4 · Commit + push the closure phase.** All staged closure
edits land in one or more `close(<feature>):` commits per §5.
Push to upstream.

**5 · Closure Review Gate.** Run §8's Review Gate in closure
mode — body rewritten to final shape, PR/CR title rewritten
to the dominant agent-dev subject (`feat(<scope>):` etc.) so
the squash-merge commit on `main` reads as a feature ship.
Internal-marker grep fires.

**6 · Squash merge to `main`** — one logical commit per
feature on the main history. Exceptions:

- Single-commit branch: plain merge or squash produce the same
  result; either works.
- Branch carrying *several* logical features (rare): break
  into multiple squash merges, one per logical feature.
- Repo whose `main` is non-linear by convention: squash still
  works, but surface the choice before going non-default.
- Repo enforcing fast-forward only on `main`: do the squash
  locally, then commit and push (the review tool can't be the
  merger).

**7 · Branch cleanup.** Delete the feature branch from local
**and** remote, then prune stale remote refs. Deletion is the
declared default for merged feature branches — surface it as
one line, do not pause for permission.

**8 · Worktree teardown — offer-only.** If the current working
directory is a non-primary worktree, surface its path and
offer to remove it. Do not auto-remove — the worktree may
have artifacts (logs, scratch files, debug output) worth
inspecting first.

## Session Log

<!-- Newest at top. Observations during sessions; promote durable
     ones into the body above at wrap-up. -->
