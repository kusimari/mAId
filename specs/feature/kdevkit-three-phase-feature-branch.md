# Feature: kdevkit-three-phase-feature-branch

## Git Setup

- Branch: `feat/kdevkit-three-phase-feature-branch`
- Base: `main`

## Feature Brief

The kdevkit feature loop currently has one Review Gate that fires after the
first agent-dev push, so the spec is committed *with* the first
implementation slice. There's no chance to review the spec on its own
before code lands, and the §9 close-out edits (in-flight reconciliation,
`project.md` verify, backlog cleanup) often happen after the squash-merge,
landing as separate commits on `main` and breaking the
"one-logical-commit-per-feature" invariant.

This feature codifies a **three-phase model** on the feature branch —
**planning → agent-dev → closure** — each with its own commits + push +
Review-Gate update. Two new Conventional-Commit types — `plan(<feature>):`
and `close(<feature>):` — encode the phase in the subject line. All three
phases collapse into one squash-merge commit on `main`, preserving the
existing invariant; the feature branch itself becomes a faithful narrative
of the round-by-round work.

This spec subsumes
`specs/backlog/kdevkit-planning-and-closeout-hardening.md` — its three
hardening concerns (spec-already-drafted handling, drive-all-six-steps
discipline, bundle `project.md` updates into the squash) are absorbed into
the new rules. That backlog item is `git rm`'d at this feature's closure.

## Requirements

### Three phases on the feature branch

- **Planning phase.** Produces `plan(<feature>):` commits. Touches only
  `specs/feature/<feature>.md` (and rarely `specs/backlog/<item>.md` when
  promoting). No code edits in this phase.
- **Agent-dev phase.** Existing behavior — `feat` / `fix` / `refactor` /
  `test` / `docs` / `chore` commits, Quality → Test → Push → Review gates
  per §8.
- **Closure phase.** Produces `close(<feature>):` commits that bundle:
  (a) reconciliation of in-flight markers in the spec; (b) any
  `project.md` updates the soft-verify produces; (c) `git rm` of resolved
  backlog items. The squash-merge to `main` (existing §9.2) carries
  everything from all three phases.

### Phase-gating cues (§6 extension)

- **Planning → agent-dev**: `"spec looks good"` / `"start build"` /
  `"plan approved"` / equivalent.
- **Agent-dev → closure**: `"close it"` / `"ship it"` / `"merge it"` /
  `"feature done"` (existing §9 trigger language stays valid).
- Phase gating still says: stop after each phase, wait for explicit cue.

### Spec-already-drafted handling (§3)

When `specs/feature/<feature>.md` exists with content, the agent **starts
in the planning phase**, not agent-dev. Even if all four interview
sections are populated, the agent confirms the spec is ready as-is or
iterates on the planning round before any code is written.

Resolves backlog item 1 from
`specs/backlog/kdevkit-planning-and-closeout-hardening.md`.

### Review Gate fires per phase (§8)

The Review Gate runs at each phase boundary; the body shape changes per
phase but the gate logic (Refuse-on-fail, Update-vs-create, internal-
marker grep, return URL) is unchanged.

- **Planning Review Gate.** Title: `plan(<feature>): subject`. Body: Why +
  Spec summary (Requirements / Design / Test Strategy / Implementation
  Plan, one line each) + Open questions. PR/CR opens as a normal PR/CR
  (not draft) — the `plan(...)` title prefix and commit history convey
  the phase signal across hosts.
- **Agent-dev Review Gate.** Existing behavior. Title:
  `feat(scope): subject` etc. Body: Why + Approach.
- **Closure Review Gate.** Title is **rewritten** to the most descriptive
  `feat(<scope>): subject` from the agent-dev phase (or `fix(...)` /
  `refactor(...)` per the dominant work type). Reason: hosts that inherit
  the PR title onto the squash-merge commit (e.g. GitHub) need
  `main`'s history to read `feat(<scope>):`, not the closure mechanic.
  Body: rewrite to final shape — Why + Approach + Verification + optional
  Reading guide / Pairs with / Spec & docs touched at close-out.

### §9 close-out reordering (resolves backlog items 2 & 3)

The §9 sequence is rewritten so reconciliation, `project.md` verify, and
backlog cleanup all land **as commits on the feature branch before the
squash-merge**, not as ambient post-merge bookkeeping:

1. Reconcile in-flight markers in the feature spec.
2. Soft `project.md` verify (moved earlier — was §9.4).
3. Backlog cleanup (interactive). *Asking* is mandatory even when the
   answer is "none".
4. Commit + push the closure phase (`close(<feature>):` commits).
5. Closure Review Gate.
6. Squash-merge to `main`.
7. Branch cleanup — local + remote, default delete, no permission pause.
8. Worktree teardown — offer-only.

### Always-on by default (§2 opt-out key)

Three-phase flow is the default for every kdevkit-aware project. Projects
that prefer spec-bundled-with-code review can opt out via
`planning_phase: false` in their `## Agent Development` → `kdevkit` block.
Documented as a new optional key.

### Strict-CI substitution

Projects whose CI enforces a closed Conventional-Commits set substitute
`docs(spec/<feature>): plan — …` / `chore(close/<feature>): …`. The
phase signal is what matters; the literal verb is secondary. Documented
in the project's `## Agent Development` → `kdevkit` block.

### Out of scope

- Auto-detecting the dominant `feat(...)` subject for the Closure Review
  Gate's title rewrite — agent picks per-feature; not codified as
  metadata.
- Cross-repo "Pairs with" automation.
- Hard-blocking close-out on `project.md` updates (still soft per §6).
- Auto-running `git worktree add` / `git worktree remove` (still
  offer-only per the existing rule).

## Design

The kdevkit skill is markdown — every behavior is encoded as instructions
the agent reads when the skill loads. The change set:

1. **Frontmatter.** Bump `version: 2.2.1` → `2.3.0`. Extend
   `description` with the three-phase shape.
2. **Preamble (after the opening "Three nested loops" block).** Add a
   "phases on the feature branch" sub-block that enumerates planning /
   agent-dev / closure with their commit-type signal.
3. **§2 (Optional `## Agent Development` section).** Document the new
   `planning_phase: true|false` opt-out key (default true).
4. **§3 (Load feature context).** In "Locate or create the feature spec",
   add the spec-already-drafted handling. In the four-checks list, append
   a fourth check naming the planning-phase commit point.
5. **§5 (Git practices).** Append two bullets to the Commits list
   covering `plan(<feature>):` and `close(<feature>):`. Note the
   strict-CI substitution.
6. **§6 (Session behaviour).** Rewrite the phase-gating paragraph to name
   the three branch phases explicitly with their cues.
7. **§8 (Quality → Test → Push → Review).** Restructure the Review Gate
   subsection so it explicitly fires per phase: Planning / Agent-dev /
   Closure. Keep the Refuse-on-fail / Update-vs-create / grep / return-
   URL rules common across phases.
8. **§9 (Feature close-out loop).** Reorder steps so reconciliation /
   `project.md` verify / backlog cleanup land as commits before the
   squash-merge.
9. **(Optional) Quick-reference cheat-sheet.** One-paragraph block at the
   end of §6 summarising the three phases for skim-reading.

The change touches `sources/skills/kdevkit/SKILL.md` and this feature
spec. It removes
`specs/backlog/kdevkit-planning-and-closeout-hardening.md` at closure (the
backlog item's three concerns are absorbed). No other code or files
change.

## Test Strategy

- **Quality gate (load-bearing):** `deno task fmt` + `deno task lint` +
  `deno task check`. Skill is markdown — gate runs against the workspace;
  SKILL.md must remain a well-formed file.
- **Test gate (load-bearing):** `deno task test:unit` — the existing
  22-test suite. No consumed surface changes; should pass unchanged.
- **Smoke (optional):** `deno task test:smoke` after deploy confirms
  symlinks still resolve.
- **Functional (judge mode, user-driven):** Update the existing
  `tests/functional/skills/kdevkit-feature-loop.smoke` and
  `kdevkit-feature-closure.smoke` fixtures to assert the agent recalls
  `plan(<feature>):` / `close(<feature>):` commit conventions. Optionally
  add `kdevkit-planning-phase.smoke` exercising spec-already-drafted
  handling. Functional runs are user-driven per the project.md Testing
  rule; the agent prepares the fixture and hands off.
- **Manual verification (out-of-band):** Dogfood the new convention on
  *this* feature — the planning commit + Review Gate path is the first
  real-world exercise.

## Implementation Plan

1. Bump frontmatter version + description in
   `sources/skills/kdevkit/SKILL.md`.
2. Add the "phases on the feature branch" preamble sub-block.
3. Add the `planning_phase` opt-out key to §2's Optional `## Agent
   Development` section paragraph.
4. Edit §3: append spec-already-drafted handling to "Locate or create the
   feature spec", and append the planning-phase commit point as a fourth
   check.
5. Edit §5: append `plan(<feature>):` and `close(<feature>):` bullets to
   Commits, plus the strict-CI substitution note.
6. Edit §6: rewrite the phase-gating paragraph to name the three branch
   phases + cues. Optionally append the cheat-sheet block.
7. Edit §8: restructure the Review Gate subsection to fire per phase.
8. Edit §9: reorder the close-out steps so reconciliation /
   `project.md` verify / backlog cleanup are commits before the squash-
   merge.
9. Run Quality + Test gates locally.
10. Stage + commit per the new convention; push the feature branch; open
    the PR via the planning Review Gate (dogfood).

### Risk notes

- Skill is consumed live by every kdevkit-aware session. Misnumbered
  headings or broken markdown ripple immediately. The diff review must
  confirm §1–§9 number cleanly after edits.
- The planning-phase commit becomes the first impression of every
  feature for reviewers. The body shape (Why + Spec summary + Open
  questions) must read well at first glance — overly prescriptive
  phrasing risks turning the PR description into a re-paste of the spec
  itself.
- The "title rewrite at Closure Review Gate" rule is new; agents that
  miss it will leave `close(<feature>):` as the squash-merge subject on
  `main`, regressing §5. The rule needs to be load-bearing in §8's
  Closure Review Gate paragraph, not a sidebar.

## Session Log

<!-- newest at top -->

## Decision Log

- **Planning PR opens normal, not draft.** Considered draft mode on hosts
  that support it. Rejected: adds a draft-to-non-draft transition step at
  the planning → agent-dev boundary, breaks uniformly across hosts that
  don't have a draft concept. The `plan(<feature>):` title prefix +
  commit history convey the phase signal without needing host-specific
  state.
- **Always-on default with opt-out, not opt-in.** Considered making the
  three-phase flow opt-in via `planning_phase: true` per project.
  Rejected: would lose the always-on safety net of "spec captured in git
  before code" that's the whole point of this feature. Opt-out via
  `planning_phase: false` keeps the safety net default and gives projects
  with strong spec-bundled-with-code preferences an escape hatch.
- **Rewrite PR title at Closure Review Gate to `feat(<scope>):`, not
  `close(<feature>):`.** GitHub's squash-merge inherits the PR title onto
  the commit subject; leaving `close(<feature>):` would make `main`'s
  history read as a closure mechanic instead of the feature itself.
  Rewriting at the Closure Review Gate is one extra step but produces a
  faithful `main` history.
