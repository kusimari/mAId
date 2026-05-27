---
name: kdevkit
description: Spec-driven development workflow — project invariants, feature specs, backlog. Three nested loops (project → feature → agent-dev) with codified close-outs. Phase gating, Conventional Commits, Quality/Test/Push/Review gates, feature-loop squash-merge close-out. Public-repo hygiene. Auto-detects specs/, docs/specs/, or .kdevkit/.
version: 2.2.1
tags: [spec, feature, requirements, design, kdevkit, workflow, planning, backlog, public-repo]
---

# kdevkit — spec-driven development workflow

Three nested loops:

```
project loop      ← project.md invariants. Cross-feature.
  feature loop    ← one branch, one merge. Closes via §9.
    agent-dev loop ← Quality → Test → Push → Review. Closes via §8.
```

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

## 3 · Load feature context

At feature-start (cues: "let's start / continue / pick up
<feature>", or a branch like `feat/user-auth`), do these in
order: **(1)** locate or promote the feature spec, **(2)**
check `project.md` for a worktree preference, then **(3)**
proceed under §6 phase gating.

**1 · Locate or create the feature spec.** Derive
`$SPEC_ROOT/feature/<feature-name>.md` (lowercase, hyphenated).

- File has content → load silently.
- File is missing → first check
  `$SPEC_ROOT/backlog/<feature-name>.md`. If a matching item
  is scoped there, promote it via `git mv` into `feature/`
  and start from its existing What/Why. Only run the
  four-interview setup if nothing is scoped in either
  location.

**2 · Check `project.md` for worktree preference.** Feature
work defaults to a branch on the main checkout, but some
projects prefer per-feature isolation in a dedicated git
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

**Phase gating.** Do not chain phases automatically. After
each phase (requirements, design, test strategy,
implementation of a slice), stop and wait for explicit
instruction before starting the next; phase order is flexible.
The agent-dev loop's Push + Review-open is a single phase —
both gates green is the implicit approval to close the inner
loop. The feature close-out (§9) is a distinct phase, gated on
an explicit human cue ("close it" / "ship it" / "merge it" /
"feature done").

**Assumptions within a phase.** If input is ambiguous, present
a brief plan and wait for approval. If the path is clear, act
without a plan step.

**YOLO mode.** `yolo` → drop the phase gate and assumption
plan for the rest of the session. `yolo off` → revert
immediately.

**Feature completion.** Driven by §9. The project.md
update offer lives there as a soft step.

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

Terminal step of the agent-dev loop. Fires after Push when
both prior gates passed cleanly. Opens (or updates) the PR /
CR for this branch and returns the URL.

**Refuse-on-fail.** If a prior gate failed or finished with
residual issues noted, do not open a review. Surface what
failed and require an explicit override.

**1 · Body shape.** Title: `type(scope): subject` (per §5).
Body required: **Why** (one paragraph — motivation, not file
changes; the diff is authoritative for *what*) and
**Approach** (bullets covering the actual changes). Suggested
when warranted: **Verification** (commands + results),
**Reading guide** (file order with compare-against hints),
**Pairs with** (cross-repo links). Match §5's PR rule (body =
why + approach) — don't impose more structure on small diffs.

**2 · Body grep.** Run §7's internal-marker grep against the
prepared title + body before submission. Hit → fail loud,
surface lines, abort.

**3 · Update vs. create.** Look up an existing review on this
branch. If found, update its body. Otherwise, create a new
one.

**4 · Return the URL** as the last line of inner-loop output.

## 9 · Feature close-out loop

Closes the **feature loop**. Trigger: an explicit human cue —
"feature done" / "close it" / "ship it" / "merge it". Distinct
phase from the agent-dev loop; gated separately per §6.

Drives merge + cleanups + backlog reconciliation in one
sequence — no per-step prompts in the steady-state path. The
user's role stays at "approve the review" and "give the close
cue." Specific commands resolve via the order in §8 intro
(implicit → kdevkit block → ask + persist).

**1 · Reconcile in-flight markers in the feature spec.** Sweep
`$SPEC_ROOT/feature/<feature>.md` for unchecked Implementation
Plan items, open Decision Log entries, unresolved questions.
Resolve in place or move out — to the backlog or a follow-up
feature. The merged feature spec is "done in place"; do not
move directories.

**2 · Squash merge to `main`** — one logical commit per
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

**3 · Branch cleanup.** Delete the feature branch from local
**and** remote, then prune stale remote refs. Deletion is the
declared default for merged feature branches — surface it as
one line, do not pause for permission.

**4 · Soft `project.md` verify.** Re-run §6's offer to update
`project.md` with what changed. Decline is fine; close-out
continues either way. Not a hard block.

**5 · Backlog cleanup (interactive).** List
`$SPEC_ROOT/backlog/` contents and ask:

> _"Which backlog items did this feature close out? Pick any
> that apply, or 'none'."_

`git rm` the chosen ones. Unchosen items stay.

**6 · Worktree teardown — offer-only.** If the current working
directory is a non-primary worktree, surface its path and
offer to remove it. Do not auto-remove — the worktree may
have artifacts (logs, scratch files, debug output) worth
inspecting first.

## Session Log

<!-- Newest at top. Observations during sessions; promote durable
     ones into the body above at wrap-up. -->
