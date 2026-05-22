---
name: kdevkit
description: Spec-driven development workflow — project invariants, feature specs, backlog. Phase gating, Conventional Commits, Quality/Test/Push gates. Public-repo hygiene. Auto-detects specs/, docs/specs/, or .kdevkit/.
version: 2.1.0
tags: [spec, feature, requirements, design, kdevkit, workflow, planning, backlog, public-repo]
---

# kdevkit — spec-driven development workflow

Self-contained methodology with three distinct surfaces:

1. **Project invariants** — maintained, change rarely. Mission,
   architecture, tech stack, layout, testing, deployment.
2. **Feature specs** — lifecycle per feature. Requirements,
   design, test strategy, implementation plan, session +
   decision logs.
3. **Backlog** — open list of wanted future work, one file per
   item.

"kdevkit" is the moniker for this methodology. The directory
name on disk is not load-bearing — the skill auto-detects
between `specs/`, `docs/specs/`, and `.kdevkit/`.

## 1 · Locate the spec tree

At session start, resolve `$SPEC_ROOT` by looking for the
following directories in order — first hit wins:

```
specs/          ← preferred
docs/specs/
.kdevkit/       ← legacy
```

If none exists and the user begins feature work, create `specs/`.
Never auto-migrate an existing `.kdevkit/` tree — that is a
human `git mv` decision because it can touch CI wiring.

Subsequent references to `$SPEC_ROOT` below mean whichever of
the three is in use.

## 2 · Load project context

If `$SPEC_ROOT/project.md` exists, read it silently at session
start. It describes what the project is, its tech stack, and any
hard constraints.

If the file is missing or empty and the user starts feature
work, ask exactly one question:

> _"Briefly describe this project — purpose, tech stack, and any
> hard constraints."_

Then write `$SPEC_ROOT/project.md` using the six-section
template below before moving on.

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

1. **Probe ecosystem markers** to identify the toolchain:
   `pyproject.toml` / `package.json` / `Cargo.toml` / `go.mod` /
   `pom.xml` or `build.gradle*` / `Makefile` / `deno.json`. Map
   each to its conventional format / lint / type-check / test
   commands.
2. **Prefer CI as ground truth.** If `.github/workflows/*.yml`
   or `.gitlab-ci.yml` defines quality / test commands, use them
   verbatim — they reflect what actually runs.
3. **One batched confirmation with the user:** present detected
   commands and ask which test layers are load-bearing vs.
   nice-to-have. Don't drip-feed questions.
4. **Write the Testing section as prose** — describe the test
   layers and mention commands inline where natural. Do not
   invent a structured "Toolchain" block; §8 reads commands out
   of the prose at run time.

### Optional `## Agent Development` section

Some skills carry an opinionated, structured flow that needs
per-project config (score thresholds, retry budgets, named
sub-agents, etc.). When such config doesn't fit naturally into
the general sections above, `project.md` may carry an optional
`## Agent Development` section, organised by skill name. kdevkit
itself does not require this section — its loop runs from the
Testing section's prose. Add an `## Agent Development` block
only when a specific skill asks for one.

## 3 · Load feature context

When the user begins work on a feature (cue words: "let's start
/ continue / pick up <feature>", or a named branch like
`feat/user-auth`), derive the feature-file path:

```
$SPEC_ROOT/feature/<feature-name>.md
```

Lowercase the name, replace spaces with hyphens.

- **File has content** → load silently. It tells you what the
  current state is and what comes next.
- **File is missing** → before running the four-interview setup,
  check `$SPEC_ROOT/backlog/<feature-name>.md`. If a matching
  spec is already scoped there, promote it: `git mv` to
  `$SPEC_ROOT/feature/<feature-name>.md` and start from its
  existing What/Why. Only run the full interview if nothing is
  scoped in either location.

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
completing any phase (requirements, design, test strategy,
implementation of a slice), stop and wait for explicit
instruction before starting the next. Phase order is flexible;
the gate applies between any two.

**Assumptions within a phase.** If a phase's input is ambiguous,
present a brief plan (what you're assuming, how you intend to
proceed) and wait for approval. If the path is clear, act
without a plan step.

**YOLO mode.** If the user says `yolo`: drop the phase gate and
drop the assumption plan for the rest of the session. If the
user says `yolo off`: revert immediately.

**Feature completion.** When the user indicates the feature is
done:

1. Offer: _"Shall I update `$SPEC_ROOT/project.md` with what
   changed? This keeps future sessions oriented."_
2. On yes: append/revise the project file with the new patterns,
   constraints, or components introduced.

## 7 · Public-repo hygiene

Some projects are public; some are internal. Skills,
specs, and commit text written in a public project must
not leak internal product/team/ticket/CR/repo/store
names. The canonical signal is the project's own
declaration:

- If `$SPEC_ROOT/project.md`'s **Hard constraints**
  section contains a bullet declaring the repo public,
  apply the rules below.
- If the checkout's `git remote -v` resolves to an
  obviously public host (`github.com`, `gitlab.com`,
  `codeberg.org`, etc.) and `project.md` is silent,
  treat the repo as public until told otherwise.

When in public-repo mode, NEVER write internal names
into:

- `SKILL.md` / `agent.md` / `command.md` bodies
- `specs/feature/*.md`, `specs/backlog/*.md`,
  `project.md`
- Commit messages and PR descriptions
- `README.md` and other top-level docs

What counts as an "internal name" is project-specific.
At minimum: internal product names, team identifiers,
ticket IDs, code-review URLs, internal repo names,
internal storage/store names, and internal employee
emails. Where ambiguity exists, ask the user and add
the answer to `project.md`'s Hard constraints.

When the user asks to capture work that mentions
internal names — note it; surface that the public repo
won't carry the names; offer to file the capture into
a corporate spec tree (the user maintains that tree
elsewhere) instead. Do not silently strip names and
write a partial capture either.

The Quality → Test → Push loop's pre-push check (§ 8)
greps the staged diff for known internal markers; in
public-repo mode, fail loud on any hit and surface the
matching lines for the user to scrub before retrying.

## 8 · Quality → Test → Push loop

Apply after any coherent unit of implementation work. Once an
implementation plan is approved, the loop runs autonomously
between gates — no per-step prompts.

### Inputs · read commands from `project.md`

Before running, read `$SPEC_ROOT/project.md`'s Testing section
to identify the format / lint / type-check / test commands for
this project. If a command is ambiguous or missing:

- Ask the user **once**, then offer to update `project.md` so
  the answer persists.
- If `project.md` itself is missing the Testing section, fall
  back to §2's first-time detection rubric and confirm in one
  batch.

If the project has an `## Agent Development` section with a
`kdevkit` block, prefer those values over the generic defaults
below (score threshold, retry budget).

### Quality Gate

1. Run the format command. Apply all auto-fixes.
2. Run the lint command. Fix all violations; re-run until clean.
3. Run the type-check command (if applicable). Fix all errors.
4. Self-review the diff against the base branch. Score your own
   work 0–100 for correctness, security, and adherence to
   project conventions. Default threshold: **70**.
   - ≥ threshold → proceed to Test Gate.
   - < threshold → fix the highest-severity issues and re-review
     **once only**; if still below, proceed and note residual
     issues in the Session Log.

### Test Gate

1. Run the test command identified above.
2. All tests pass (zero failures, zero errors).
3. On failure: diagnose, fix, re-run. Default budget: **2**
   fix-and-retry cycles. If still failing, stop and report — do
   not push.
4. If the fixes were substantial, re-run the Quality Gate before
   pushing.

### Push Gate

Only push after both gates pass:

```
git push -u origin <feature-branch>
```

Opening a PR is a human decision, not part of this loop.

## Session Log

<!-- Newest at top. Observations during sessions; promote durable
     ones into the body above at wrap-up. -->
