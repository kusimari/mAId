---
name: kdevkit
description: Spec-driven development workflow — project invariants, feature specs, backlog. Three nested loops (project → feature → agent-dev) with codified close-outs. Phase gating, Conventional Commits, Quality/Test/Push/Review gates, feature-loop squash-merge close-out. Public-repo hygiene. Auto-detects specs/, docs/specs/, or .kdevkit/.
version: 2.2.0
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

### Three loops

Work nests as three loops, named here so later sections can
refer to them:

```
project loop      ← project.md invariants. Lives across features.
  feature loop    ← one branch, one merge. Closes via §9.
    agent-dev loop ← Quality → Test → Push → Review. Closes via §8.
```

Each inner loop's terminal step is the trigger for re-entering
the outer loop's next iteration. The two close-outs (§8 Review
Gate, §9 feature close-out) automate the manual handoffs that
otherwise live between iterations.

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

### Worktree recommendation

Feature work defaults to a branch on the main checkout. Where a
project prefers per-feature isolation, the feature loop should
run in a dedicated git worktree so independent features can
proceed without contaminating each other.

At feature-start (after backlog promotion or the four-interview
setup, before the first implementation slice), check
`$SPEC_ROOT/project.md` for a worktree-preference signal:

- An `## Agent Development` → `kdevkit` block declaring
  `prefer_worktree: true|false`, **or**
- A bullet under **Hard constraints** mentioning worktrees.

If the signal is **prefer**, surface a one-line suggestion —
do not auto-run, the user may have a different parent path
convention:

```
git worktree add ../<repo>-<feature> -b <branch>
```

If `project.md` is silent on worktrees, continue branch-only
without prompting. Worktree status does **not** gate the §8
Review Gate; it only affects the §9 worktree-teardown offer.

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
the gate applies between any two. The agent-dev loop's
Push + Review-open is a single phase (Push), not chained
phases — both gates green is the implicit approval to close
the inner loop. The feature close-out (§9) is a distinct phase,
gated on an explicit human cue ("close it" / "ship it" /
"merge it" / "feature done").

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

The Quality → Test → Push → Review loop's pre-push check
(§ 8) greps the staged diff for known internal markers; in
public-repo mode, fail loud on any hit and surface the
matching lines for the user to scrub before retrying. The
Review Gate (§ 8) reruns the same grep against the prepared
PR/CR title and body string before submission — review
content is also human-visible.

## 8 · Quality → Test → Push → Review loop

Apply after any coherent unit of implementation work. Once an
implementation plan is approved, the loop runs autonomously
between gates — no per-step prompts. The four gates are the
agent-dev loop's body; the Review Gate is its terminal step.

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

Only push after both prior gates pass:

```
git push -u origin <feature-branch>
```

Before pushing, run §7's internal-marker grep against the
staged diff. In public-repo mode, fail loud on any hit and
surface matching lines for the user to scrub before retrying.

### Review Gate

Always-on terminal step of the agent-dev loop. Fires after Push
when Quality and Test both passed cleanly. Closes the inner
loop by opening (or updating) the PR / CR for this branch and
returning the URL.

**Refuse-on-fail.** If either prior gate failed — or the
Quality self-review finished below threshold with residual
issues noted — do not open a review. Surface what failed and
require an explicit "open it anyway" override.

**1 · Detect the review tool.** Resolution order, first hit
wins:

1. `git remote get-url origin` → host match. Built-in
   mapping: `github.com` → `gh`. Other public hosts (e.g.
   `gitlab.com`, `codeberg.org`) may be added in the same
   spirit as the project grows.
2. `## Agent Development` → `kdevkit` block in
   `$SPEC_ROOT/project.md` declaring the per-project review
   CLI. The block names the commands for **create**, **edit**
   (update body), **list-by-branch** (existence check), and
   **merge** (used by §9).
3. Neither hit → ask the user once for the review-CLI
   commands, and offer to write them into `project.md`'s
   `## Agent Development` → `kdevkit` block so future
   sessions skip the question.

**2 · Prepare title + body.**

- Title: same `type(scope): subject` shape as the commit (per
  §5).
- Body — light shape, two required sections:
  - **Why** — one paragraph. The motivation, not a list of
    file changes. The diff is authoritative for *what*.
  - **Approach** — bullet list covering the actual changes.
- Suggested optional sections, when warranted by diff size or
  reviewer cold-read cost:
  - **Verification** — commands run + results.
  - **Reading guide** — numbered file order with "compare
    against X" hints.
  - **Pairs with** — cross-repo links when applicable.

This matches §5's existing PR rule (body: _why_ + approach)
without imposing new structure on small diffs.

**3 · Body grep.** Run §7's internal-marker grep against the
prepared title + body string before submission. Hit → fail
loud, surface matching lines, abort.

**4 · Update vs. create.** Look up an existing review for the
branch using the detected tool's list-by-branch verb (e.g.
`gh pr list --head <branch> --state open`).

- Existing review found → update its body using the **edit**
  verb (`gh pr edit <n> --body-file <tmp>`).
- No existing review → use the **create** verb
  (`gh pr create --title "<title>" --body-file <tmp>`).

**5 · Return the URL** as the last line of inner-loop output.
The agent-dev loop is now closed; the next phase is either
another agent-dev iteration on the same feature or the §9
feature close-out.

## 9 · Feature close-out loop

Closes the **feature loop**. Trigger: an explicit human cue —
"feature done" / "close it" / "ship it" / "merge it" / "land
it" / equivalent. Distinct phase from the agent-dev loop;
gated separately per §6.

The close-out drives the merge, the cleanups, and the
backlog reconciliation in one sequence — no per-step prompts
in the steady-state path. The user's role stays at "approve
the review" and "give the close cue."

**1 · Reconcile in-flight markers in the feature spec.**
Before merging, sweep `$SPEC_ROOT/feature/<feature>.md` for
unchecked Implementation Plan items, open Decision Log
entries, or unresolved questions. Either resolve them in
place (so the spec is a faithful record of what shipped) or
move them out — into the backlog, or into a new follow-up
feature. The merged feature spec is "done in place"; it does
not move directories.

**2 · Squash merge.** Default: **squash merge** to `main` —
one logical commit per feature on the main history.

- Single-commit branch → squash and plain merge produce the
  same result; either is fine.
- Multiple commits → squash. If the branch contains work for
  *several* logical features (rare), break into multiple
  squash merges, one per logical feature.
- Repo whose `main` is itself a non-linear history (merge
  commits as the norm) → squash still works, but surface the
  choice to the user before going non-default.
- Repo enforcing fast-forward only on `main` → use the local
  `git merge --squash <feat>` + `git commit` + `git push`
  pattern, since the review tool can't be the merger.

**3 · Branch cleanup — local + remote, no per-step prompts.**
Deletion is the declared default for merged feature branches.

- `gh pr merge` path: pass `--delete-branch`.
- Local-merge path: `git branch -D <feat>` +
  `git push origin --delete <feat>` + `git fetch --prune`.

Surface the deletion as one line of output. Do not pause for
permission — the close cue is the approval.

**4 · Soft `project.md` verify.** Re-run §6's existing offer:
_"Shall I update `project.md` with what changed? This keeps
future sessions oriented."_ User decline is fine; the
close-out continues either way. The aim is a prompt at the
moment when the human knows whether invariants drifted, not
a hard block.

**5 · Backlog cleanup (interactive).** List the contents of
`$SPEC_ROOT/backlog/` and ask the user which items this
feature resolves:

> _"Which backlog items did this feature close out? Pick any
> that apply, or 'none'."_

`git rm` the chosen ones. Items the user does not pick stay
in the backlog. Feature names drift, so this is a one-prompt
interactive step rather than a frontmatter pointer that
would go stale.

**6 · Worktree teardown — offer-only.** If `git worktree
list` shows the current working directory as a non-primary
worktree, surface the path and offer:

```
git worktree remove <path>
```

Do not auto-remove. The worktree may have generated artifacts
(logs, scratch files, debug output) the user wants to inspect
before teardown.

This closes the feature loop. The next phase belongs to the
project loop: pick the next feature spec, or promote a backlog
item, or step away.

## Session Log

<!-- Newest at top. Observations during sessions; promote durable
     ones into the body above at wrap-up. -->
