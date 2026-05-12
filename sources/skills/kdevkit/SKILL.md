---
name: kdevkit
description: Spec-driven development workflow — project + feature context files, phase gating, Conventional Commits, Quality/Test/Push gates. Use when starting or continuing feature work in a repo with a .kdevkit/ directory.
version: 1.0.0
tags: [spec, feature, requirements, design, kdevkit, workflow, planning]
---

# kdevkit — spec-driven development workflow

Self-contained methodology: project context, feature files, phase gating, git practices, and a
quality → test → push loop. Inspired by the kdevkit reference implementation; this skill is the
in-session copy so sessions don't need to fetch anything at runtime.

## 1 · Load project context

If the current repo contains `.kdevkit/project.md`, read it silently at session start. It describes
what the project is, its tech stack, and any hard constraints.

If the file is missing or empty and the user starts feature work, ask exactly one question:

> _"Briefly describe this project — purpose, tech stack, and any hard constraints."_

Write the answer to `.kdevkit/project.md` before moving on.

## 2 · Load feature context

When the user begins work on a feature (cue words: "let's start / continue / pick up <feature>", or
a named branch like `feat/user-auth`), derive the feature-file path:

```
.kdevkit/feature/<feature-name>.md
```

Lowercase the name, replace spaces with hyphens.

- **File has content** → load it silently. It tells you what the current state is and what comes
  next.
- **File is missing** → before running the four-interview setup, **check
  `.kdevkit/feature-wip/<feature-name>.md`**. If a matching spec is already scoped there (a previous
  session wrote it down), promote it: `git mv` to `.kdevkit/feature/<feature-name>.md` and start
  from its Requirements/Design. Only run the full interview if nothing is scoped in either location.

### Feature setup — four short interviews

One interview per topic. Use existing project context to skip questions you already have answers to.

1. **Requirements.** What problem does this solve? Who interacts with it? What is the acceptance
   criterion?
2. **Design.** Technical approach, main components, how they interact. Call out trade-offs.
3. **Test strategy.** What kinds of tests (unit / integration / smoke / manual), which are
   load-bearing, what are the key scenarios.
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

## 3 · Git practices (always-on in spec-driven repos)

### Branches

- `<type>/<short-description>` — `feat` · `fix` · `chore` · `docs` · `refactor` · `test`.

### Commits

- Conventional Commits: `type(scope): subject` — imperative mood, lowercase, no trailing period;
  subject ≤ 72 chars.
- Body (when present) explains _why_, not _what_. The diff is authoritative for _what_.
- Every commit leaves the repo in a working state. Commits are save points — commit whenever a
  coherent unit of work is done; don't batch to end-of-feature.
- New commits, never amends, unless the user explicitly asks.

### Scope

- Stays local to this project. Never modify global git config, never write outside the project root
  from git hooks or scripts.

### Pull Requests

- Title: same `type(scope): subject` shape.
- Body: _why_ + approach. Keep PRs small — one concern per PR.
- Squash merge.
- PR-ready means: Quality Gate + Test Gate both pass locally.

### Hygiene

- No commented-out code, debug prints, temp files, secrets, or credentials in commits.

## 4 · Session behaviour (always-on)

**Keep the feature file current.** After each meaningful unit of work — a decision made, a subtask
completed, an open question answered — update the feature file's Session Log / Decision Log before
starting the next unit. Do not batch.

**Phase gating.** Do not chain phases automatically. After completing any phase (requirements,
design, test strategy, implementation of a slice), stop and wait for explicit instruction before
starting the next. Phase order is flexible; the gate applies between any two.

**Assumptions within a phase.** If a phase's input is ambiguous, present a brief plan (what you're
assuming, how you intend to proceed) and wait for approval. If the path is clear, act without a plan
step.

**YOLO mode.** If the user says `yolo`: drop the phase gate and drop the assumption plan for the
rest of the session. If the user says `yolo off`: revert immediately.

**Feature completion.** When the user indicates the feature is done:

1. Offer: _"Shall I update `.kdevkit/project.md` with what changed? This keeps future sessions
   oriented."_
2. On yes: append/revise the project file with the new patterns, constraints, or components
   introduced.

## 5 · Quality → Test → Push loop

Apply after any coherent unit of implementation work.

### Quality Gate

1. Format source files (repo-appropriate formatter).
2. Lint; fix all violations; re-run until clean.
3. Type-check (if the repo supports it); fix all errors.
4. Self-review the diff against the base branch. Score your own work 0–100 for correctness,
   security, and adherence to project conventions. Threshold: 70.
   - ≥ 70 → proceed to Test Gate.
   - < 70 → fix the highest-severity issues and re-review **once only**; if still below threshold,
     proceed and note residual issues in the Session Log.

### Test Gate

1. Run the full test suite.
2. All tests pass (zero failures, zero errors).
3. On failure: diagnose, fix, re-run. Up to **2** fix-and-retry cycles. If still failing, stop and
   report — do not push.
4. If the fixes were substantial, re-run the Quality Gate before pushing.

### Push Gate

Only push after both gates pass:

```
git push -u origin <feature-branch>
```

Opening a PR is a human decision, not part of this loop.

## 6 · Repo-specific toolchains

Document per-project commands directly in `.kdevkit/project.md` or a sibling
`.kdevkit/agent-dev-instructions.md` if the detection gets more nuanced. The skill stays
tool-agnostic; the project file carries the specifics.

## Session Log

<!-- Newest at top. Observations during sessions; promote durable
     ones into the body above at wrap-up. -->
