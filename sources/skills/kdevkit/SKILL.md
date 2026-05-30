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
auto-migrate an existing `.kdevkit/` tree.

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

<!-- Purpose + who it serves. One paragraph. -->

## Architecture

<!-- Logical shape: components + responsibilities. Words mandatory. -->

## Tech Stack

<!-- Languages, runtimes, frameworks. Versions where they matter. -->

## Layout

<!-- Directory tree, one-line annotation per entry. -->

## Testing

<!-- Test layers + commands; load-bearing vs. nice-to-have. -->

## Deployment

<!-- Build / release / install path, or how it's consumed. -->
```

### First-time `project.md` detection

When creating `project.md`, probe ecosystem markers
(`package.json`, `pyproject.toml`, `Cargo.toml`, `go.mod`,
`Makefile`, `deno.json`) and CI files (`.github/workflows/*`,
`.gitlab-ci.yml` — verbatim) for the toolchain; confirm in
one batch; write Testing as prose — §8 reads commands from it
at run time.

### Optional `## Agent Development` section

`project.md` may carry an `## Agent Development` section
organised by skill. Keys under `kdevkit`:

- `prefer_worktree: true|false` — feature-start worktree
  recommendation (see §3).
- `planning_phase: true|false` (default `true`) — three-phase
  feature branch (planning → agent-dev → closure) per §6/§8/§9.
  Set `false` to revert to spec-bundled-with-code review.

## 3 · Load feature context

At feature-start (cues: "let's start / continue / pick up
<feature>", or a branch like `feat/user-auth`), do all four
checks before the first implementation slice:

1. **Locate or promote the feature spec.**
2. **Check `project.md` for a worktree preference.**
3. **Proceed under §6 phase gating.**
4. **Plan to commit the spec.** Commit the populated spec as
   `plan(<feature>): initial spec` and ship through the §8
   Planning Review Gate before any code work; skip if
   `planning_phase: false` (§2).

### 1 · Locate or create the feature spec

Derive `$SPEC_ROOT/feature/<feature-name>.md` (lowercase,
hyphenated).

- File has content → load silently and **start in the
  planning phase**, not agent-dev. A populated spec is not a
  *reviewed* spec; confirm readiness or iterate. The planning
  → agent-dev cue (§6) is the gate.
- File is missing → check `$SPEC_ROOT/backlog/<feature-name>.md`.
  Match → promote via `git mv` into `feature/` and start from
  existing What/Why. Otherwise run the four-interview setup.

### 2 · Check `project.md` for worktree preference

Default: branch on main checkout. Signal for per-feature
worktree isolation: `kdevkit` block `prefer_worktree:
true|false`, or a Hard-constraints bullet mentioning worktrees.
**prefer** → suggest a worktree at feature-start (don't
auto-run); silent → branch-only without prompting. Worktree
status doesn't gate §8; only affects §9.8 teardown offer.

### 3 · Proceed under §6 phase gating

Stop after each interview phase (requirements / design / test
strategy / implementation plan); see §6 for the full rule.

### Feature setup — four short interviews

One per topic; skip what existing project context already
answers.

1. **Requirements.** Problem? Who interacts? Acceptance
   criterion?
2. **Design.** Technical approach, components, interactions,
   trade-offs.
3. **Test strategy.** Test kinds (unit / integration / smoke /
   manual), which are load-bearing, key scenarios.
4. **Implementation plan.** Ordered tasks + risk notes.

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

<!-- append: date · what was done · decisions made -->

## Decision Log

<!-- append: decision · rationale · alternatives rejected -->
```

## 4 · Backlog

When the user describes wanted-but-not-now work — an idea, a
frustration, a "we should eventually" — write it to
`$SPEC_ROOT/backlog/<item-name>.md`. One file per item; never
consolidate into a single `FIXES.md` or `TODO.md`.

### Backlog item template

```markdown
# Backlog: <item-name>

## What

<!-- One paragraph; what, not how. -->

## Why

<!-- Motivation; link the conversation/incident. -->

## Open questions

<!-- Blockers, dependencies, unknowns. -->

```

Promoting backlog → feature: `git mv` into
`$SPEC_ROOT/feature/`, then fill Requirements / Design / Test
Strategy / Implementation Plan around the existing What/Why.

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
- Every commit leaves the repo working. Commit per coherent
  unit; don't batch to end-of-feature.
- New commits, never amends, unless the user explicitly asks.

#### Phase-typed commits on the feature branch

Existing types (`feat` / `fix` / `chore` / `docs` / `refactor`
/ `test`) are the **agent-dev** vocabulary. Two extras encode
phase:

- **`plan(<feature>):`** — planning-phase. Touches only
  `specs/feature/<feature>.md` (rarely `specs/backlog/` on
  promotion). No code edits.
- **`close(<feature>):`** — closure-phase. Reconciles in-flight
  markers, applies any `project.md` verify edit, `git rm`s
  resolved backlog items. No code edits — drift goes back to
  agent-dev.

The §9.6 squash-merge collapses every phase into one commit on
`main`. CI-restricted projects may substitute
`docs(spec/<feature>):` and `chore(close/<feature>):`; document
in the `kdevkit` block.

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

**Keep the feature file current.** Update Session Log /
Decision Log after each unit of work; don't batch.

**Phase gating.** Do not chain phases automatically. Two
gating layers stack:

- **Interview phases** (feature setup): requirements / design
  / test strategy / implementation plan. Stop after each;
  order is flexible.
- **Branch phases** (planning / agent-dev / closure). Stop at
  each boundary. Cues:
  - Planning → agent-dev: `"spec looks good"` /
    `"start build"` / `"plan approved"`.
  - Agent-dev → closure: `"close it"` / `"ship it"` /
    `"merge it"` / `"feature done"`.

Both Review Gate greens close the inner loop; closure (§9)
requires the explicit cue.

**Assumptions within a phase.** Ambiguous → present a brief
plan and wait for approval. Clear path → act.

**YOLO mode.** `yolo` drops the phase gate + assumption plan
for the session; `yolo off` reverts.

**Feature completion.** Driven by §9.

### Three-phase cheat-sheet

```
planning   plan(<feature>):    spec only      → planning Review Gate
agent-dev  feat/fix/...:       implementation → agent-dev Review Gate
closure    close(<feature>):   reconcile      → closure Review Gate → squash-merge
```

## 7 · Public-repo hygiene

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

The §8 Push Gate greps the staged diff; the Review Gate reruns
it against the prepared title + body. Hit → fail loud, surface
lines, abort.

## 8 · Quality → Test → Push → Review loop

Apply after any coherent unit of implementation work. The
loop runs autonomously between gates — no per-step prompts.

### Inputs · read commands from `project.md`

Read `project.md`'s Testing section for format / lint /
type-check / test commands; missing → fall back to §2
first-time detection. The `kdevkit` block under `## Agent
Development` overrides defaults below (threshold, retry
budget, review CLI, branch-cleanup, merge).

**Resolve any specific command** (review CLI, branch-delete,
merge, worktree ops) via implicit host knowledge → `kdevkit`
block → ask once and persist.

### Quality Gate

1. Run format; apply auto-fixes.
2. Run lint; fix until clean.
3. Run type-check (if applicable); fix all errors.
4. Self-review the diff vs. base, score 0–100 (correctness,
   security, conventions). Default threshold: **70**.
   - ≥ threshold → Test Gate.
   - < threshold → fix highest-severity, re-review **once
     only**; if still below, proceed and note residual issues
     in the Session Log.

### Test Gate

1. Run tests. All pass (zero failures, zero errors).
2. On failure: diagnose, fix, re-run. Default budget: **2**
   fix-and-retry cycles. If still failing, stop and report.
3. If fixes were substantial, re-run the Quality Gate.

### Push Gate

Only push after Quality + Test pass. Run §7's internal-marker
grep against the staged diff first; hit fails loud.

### Review Gate

Fires after Push at each branch-phase boundary (planning /
agent-dev / closure). Opens or updates the PR/CR for this
branch and returns the URL. Body shape changes per phase; gate
logic does not.

**Refuse-on-fail.** Prior gate failed or noted residual issues
→ no review. Surface failure; require explicit override.

**1 · Body shape — per phase.** Title is `type(scope): subject`
(per §5).

- **Planning** — `plan(<feature>):` push. Body: **Why** +
  **Spec summary** (R / D / T / I one-liners) + **Open
  questions**. Normal review, not draft.
- **Agent-dev** — `feat/fix/...` push. Body: **Why** +
  **Approach**. Optional: **Verification**, **Reading guide**,
  **Pairs with**.
- **Closure** — `close(<feature>):` push. **Title rewritten**
  to the dominant agent-dev subject (`feat(<scope>):` etc.) so
  the squash-merge commit on `main` reads as a feature ship.
  Body: **Why** + **Approach** + **Verification** + optional
  **Reading guide** / **Pairs with** / **Spec & docs touched**.

**2 · Body grep.** Run §7's grep against title + body before
submission. Hit → fail loud, surface lines, abort. Planning
Review Gate is the first place it runs per feature — catches
internal names before implementation.

**3 · Update vs. create.** One PR/CR per branch. Create on the
first phase; update body on subsequent phases.

**4 · Return the URL** as the last line of phase output.

## 9 · Feature close-out loop

Closes the **feature loop**. Trigger: an explicit cue —
`"feature done"` / `"close it"` / `"ship it"` / `"merge it"`.

Steps 1–3 stage spec / docs / backlog edits as
`close(<feature>):` commits before the §9.6 squash; step 3 must
be asked even when the answer is "none" — *asking is the
artifact*.

**1 · Reconcile in-flight markers.** Sweep
`$SPEC_ROOT/feature/<feature>.md` for unchecked Implementation
Plan items, open Decision Log entries, unresolved questions.
Resolve in place or move out (backlog or follow-up feature).
The merged spec is "done in place" — do not move directories.
Stage edits.

**2 · Soft `project.md` verify.** Offer to update `project.md`
with what changed. Decline is fine; not a hard block. Stage
accepted edits.

**3 · Backlog cleanup (interactive).** List
`$SPEC_ROOT/backlog/`; ask: _"Which backlog items did this
feature close out? Pick any, or 'none'."_ `git rm` the chosen
ones; asking is mandatory even when the answer is "none".

**4 · Commit + push.** Staged closure edits land in one or
more `close(<feature>):` commits per §5. Push.

**5 · Closure Review Gate.** Run §8's Review Gate — body
rewritten to final shape, title rewritten to the dominant
agent-dev subject. Grep fires.

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

## Session Log

<!-- Newest at top. Observations during sessions; promote durable
     ones into the body above at wrap-up. -->
