---
name: kdevkit
description: Spec-driven development workflow — locate spec tree, load project + feature context, start a session, run it through planning → dev loop → closure with phase-gating cues, ship via Conventional Commits and Quality/Test/Push/Review gates. Three-phase feature branch on a single PR/CR; main stays single-commit-per-feature via squash. Public-repo hygiene. Auto-detects specs/, docs/specs/, or .kdevkit/.
version: 2.6.0
tags: [spec, feature, requirements, design, kdevkit, workflow, planning, backlog, public-repo]
---

# kdevkit — spec-driven development workflow

Three nested loops, with three phases on the feature branch:

```
project loop      ← project.md invariants. Cross-feature.
  feature loop    ← one branch, three phases, one squash-merge.
    ├─ planning phase    plan(<feature>): commits + Review Gate
    ├─ dev loop          feat/fix/...: Quality → Test → Push → Review (§7)
    └─ closure phase     close(<feature>): reconcile + squash-merge (§8)
```

The branch carries all three phases on the same PR/CR; the body is
rewritten at each phase boundary. The §8 squash-merge collapses every
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
`.kdevkit/` (first hit wins).

The skill reads in session-arc order: §1–§2 set up context, §3–§4
enter a feature, §5 frames the run, §6/§7/§8 are the three phases,
§9 carries the always-on cross-cutting rules.

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
one batch; write Testing as prose — §7 reads commands from it
at run time.

### Optional `## Agent Development` section

`project.md` may carry an `## Agent Development` section
organised by skill. Keys under `kdevkit`:

- `prefer_worktree: true|false` — feature-start worktree
  recommendation (see §4).
- `planning_phase: true|false` (default `true`) — three-phase
  feature branch (planning → dev → closure) per §5/§6/§7/§8.
  Set `false` to skip §6 Planning and let spec edits ride
  with the first dev commit.
- `code_review:` — nested block configuring the §7 Code Review
  Gate. All keys optional; defaults below.

  ```yaml
  code_review:
    reviewer: host-native       # default; alternative: skill:<name>
                                # / mcp:<server.tool> / agent:<name>
    threshold: 70               # 0–100 score floor for Push
    authority: hard-stop        # alternative: soft
    retry_budget: 2             # fix-and-retry cycles before stop
  ```

  - **`reviewer`** — who runs the review. Prefix-tagged so the
    orchestrator knows what to dispatch:
    - `host-native` (default) — the host coding agent's built-in
      code review.
    - `skill:<name>` — a skill in the registry (bare strings
      without a prefix default to `skill:`).
    - `mcp:<server>.<tool>` — an MCP server's tool.
    - `agent:<name>` — a named project-configured agent.
  - **`threshold`** — score floor; sub-threshold loops back to
    Quality. Default `70`.
  - **`authority`** — `hard-stop` blocks Push when retries
    exhaust; `soft` allows Push with residuals appended to
    Session Log.
  - **`retry_budget`** — fix-and-retry cycles before stop.
    Default `2`. Pairs with the Test Gate's own retry budget;
    worst-case loop is `retry_budget × test_budget`.

  Omitting the block entirely triggers the §3/§4 setup UX. Once
  written (even with all defaults), the block sticks — the
  question doesn't re-fire next session.

## 3 · Load feature context

Entry cues: `"let's start / continue / pick up <feature>"`, or a
branch like `feat/user-auth`. Resolve the entry mode:

1. **Continue / pick up `<feature>`** — look for
   `$SPEC_ROOT/feature/<feature-name>.md` (work-in-progress).
   Fall back to `$SPEC_ROOT/backlog/<feature-name>.md`; promote
   with `git mv` into `feature/` and start from the existing
   What/Why.
2. **Start `<feature>`** — if neither file exists, run the four
   interviews (§6) and write the spec.

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
`$SPEC_ROOT/backlog/<item-name>.md`. One file per item; never
consolidate into a single `FIXES.md` or `TODO.md`. Closure-time
cleanup of resolved items lives in §8 step 3.

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
Strategy / Implementation Plan around the existing What/Why
(template in §6).

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
- **Other preferences load from the `kdevkit` block** —
  threshold, retry budget, review CLI, branch-cleanup, merge.
  Full resolution rule is in §7.

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

Update `Session Log` / `Decision Log` after each unit of work;
don't batch.

## 6 · Feature planning

Trigger: a populated spec lacks the user's review (§3
spec-already-drafted rule), or `<feature>` is being started
fresh.

### Four short interviews

One per topic; skip what existing project context already
answers. Order matters: tests sit immediately after requirements
so success criteria are declared before the design converges —
the dev loop (§7) then has a verifiable target, not a sketch to
validate after the fact.

1. **Requirements.** Problem? Who interacts? Acceptance
   criterion?
2. **Test strategy.** Per `project.md`'s Testing section: which
   layers fire for this change, what are the success criteria,
   what's load-bearing vs. nice-to-have? Map onto existing test
   commands; don't invent new layers.
3. **Design.** Technical approach, components, interactions,
   trade-offs.
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

## Test Strategy

<success criteria mapped onto project.md test layers>

## Design

<technical approach, components, interactions>

## Implementation Plan

<ordered task list with risk notes>

## Session Log

<!-- append: date · what was done · decisions made -->

## Decision Log

<!-- append: decision · rationale · alternatives rejected -->
```

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

Tests are part of the same iteration as the behavior change —
not a follow-up. When an implementation slice changes a behavior
the project's tests evaluate, the test update lands in the same
loop iteration, before the Push Gate. The §6 Test Strategy maps
each success criterion to a project test layer; the Test Gate
verifies them.

1. Run tests. All pass (zero failures, zero errors).
2. On failure: diagnose, fix, re-run. Default budget: **2**
   fix-and-retry cycles. If still failing, stop and report.
3. If fixes were substantial, re-run the Quality Gate.

### Push Gate

Only push after Quality + Test pass.

### Agent-dev Review Gate

Fires after Push. Apply §9 Review Gates. Phase-specific body
content: **Approach** (bullets covering the changes).

**Refuse-on-fail.** Prior gate failed or noted residual issues
→ no review. Surface failure; require explicit override.

## 8 · Closure

Closes the **feature loop**. Trigger: an explicit cue —
`"feature done"` / `"close it"` / `"ship it"` / `"merge it"`.

Steps 1–3 stage spec / docs / backlog edits as
`close(<feature>):` commits before the §8.6 squash; step 3
must be asked even when the answer is "none" — *asking is the
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
· `test`. Two extras encode feature-branch phase:

- **`plan(<feature>):`** — planning-phase. Touches only
  `specs/feature/<feature>.md` (rarely `specs/backlog/` on
  promotion). No code edits.
- **`close(<feature>):`** — closure-phase. Reconciles in-flight
  markers, applies any `project.md` verify edit, `git rm`s
  resolved backlog items. No code edits — drift goes back to
  the dev loop.

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
- **One PR/CR per branch.** Open as a normal review, not
  draft. Create on the first gate; update title + body on
  subsequent gates. Return the URL as the last line of phase
  output.
- **PR-ready** means Quality Gate + Test Gate both pass
  locally.
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
- §7 Push Gate (staged diff before push) and Review Gate
  (title + body before submission).
- §8 Closure Review Gate (title + body before submission).

In public-repo mode, any hit fails loud, surfaces lines, aborts.

### Commit hygiene

No commented-out code, debug prints, temp files, secrets, or
credentials in commits.

## Session Log

<!-- Newest at top. Observations during sessions; promote durable
     ones into the body above at wrap-up. -->
