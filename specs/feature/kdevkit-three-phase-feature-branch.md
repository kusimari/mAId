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

All ten items shipped via `2d46ef9 feat(kdevkit): three-phase feature
branch — plan/agent-dev/close`. Two follow-up revisions landed on the
same branch under reviewer feedback:

- `89b93de refactor(kdevkit): compress SKILL.md (628 → 473 lines, ~25%)`
  — per-rule audit + cuts of narrative rationale, worked examples,
  host mappings.
- `529702b refactor(kdevkit): restructure SKILL.md to session-arc outline`
  — §1–§9 reordered to read top-to-bottom in the order an agent acts:
  locate → load project → load feature → start session → run session
  (planning / dev / closure) → cross-cutting rules (always-on).

### Original ten-step plan

1. Bump frontmatter version + description in
   `sources/skills/kdevkit/SKILL.md`. ✓
2. Add the "phases on the feature branch" preamble sub-block. ✓
3. Add the `planning_phase` opt-out key to §2's Optional `## Agent
   Development` section paragraph. ✓
4. Edit §3: append spec-already-drafted handling to "Locate or create the
   feature spec", and append the planning-phase commit point as a fourth
   check. ✓
5. Edit §5: append `plan(<feature>):` and `close(<feature>):` bullets to
   Commits, plus the strict-CI substitution note. ✓
6. Edit §6: rewrite the phase-gating paragraph to name the three branch
   phases + cues. Optionally append the cheat-sheet block. ✓
7. Edit §8: restructure the Review Gate subsection to fire per phase. ✓
8. Edit §9: reorder the close-out steps so reconciliation /
   `project.md` verify / backlog cleanup are commits before the squash-
   merge. ✓
9. Run Quality + Test gates locally. ✓
10. Stage + commit per the new convention; push the feature branch; open
    the PR via the planning Review Gate (dogfood). ✓

### Risk notes (resolved)

- **Skill is consumed live by every kdevkit-aware session.** Section
  numbering audit ran after every edit; §1–§9 stayed clean across all
  three commits.
- **Planning-phase commit as first impression.** The PR #11 planning
  body (`49184a2`) used Why + Spec summary + Open questions; reviewer
  engaged with the Open questions block, which is the intended use.
- **Title rewrite at Closure Review Gate.** Encoded in §8 step 5 of
  the new SKILL.md and applied at this very close-out.

### Out-of-band manual verification (deferred)

Functional smoke fixtures (`tests/functional/skills/kdevkit-*.smoke`)
are user-driven per `specs/project.md`'s Testing rule. The
restructure changed section names; judge fixtures'
`expected_narrative` may need re-tune. Flagged in the rev-3 PR body.

## Session Log

<!-- newest at top -->

- 2026-05-31 · **Feature closed via §8 close-out.** Shipped as PR #11.
  Four commits across the branch (planning + behavior + compression +
  restructure):
  - `49184a2 plan(kdevkit-three-phase-feature-branch): initial spec`
  - `2d46ef9 feat(kdevkit): three-phase feature branch — plan/agent-dev/close`
  - `89b93de refactor(kdevkit): compress SKILL.md (628 → 473 lines, ~25%)`
  - `529702b refactor(kdevkit): restructure SKILL.md to session-arc outline`
  - (plus this close-out commit)

  Closure round dogfooded the new §8 sequence: reconciliation +
  project.md verify (no edits needed) + backlog cleanup
  (`kdevkit-planning-and-closeout-hardening.md` `git rm`'d as
  resolved by this feature). Closure Review Gate rewrote PR title
  from `plan(...)` to `feat(kdevkit): three-phase feature branch —
  plan/agent-dev/close` per §5 so the squash-merge commit on `main`
  reads as a feature ship.

- 2026-05-31 · **Rev-3 session-arc restructure on PR #11.** Reviewer
  follow-up after the rev-2 compression: proposed reorganising the
  skill from "reference manual organized by surface" (project / spec /
  backlog / git practices / session / hygiene / dev / closure) to a
  session-arc outline (locate → load project → load feature → start
  session → run session → cross-cutting). Critique landed four
  refinements before implementing: (1) operational gating (YOLO,
  ambiguous → plan) stays in §5, not §9, since these fire during
  phase execution rather than as cross-cutting hygiene; (2) each
  phase section ends with an explicit `Apply:` pointer naming the §9
  rules that fire there, protecting agent recall when a rule and its
  application site are in different sections; (3) §3 states the
  spec-already-drafted rule + its consequence (→ §6 Planning, not §7
  Dev) in one sentence so the rule survives an agent reading §3
  alone; (4) §9 header is labelled `Cross-cutting rules (always-on)`
  so the agent reads it as fire-everywhere, not fire-once. Result:
  473 → 505 lines. Net delta is small growth (~7%) — the structure
  win comes from the reordering, not from cuts. All load-bearing
  tokens preserved; cross-references resolve cleanly. Bumped version
  2.3.0 → 2.4.0.

- 2026-05-30 · **Compression pass on PR #11.** Reviewer feedback on
  `2d46ef9` asked for aggressive compression — the skill is loaded on
  every kdevkit-aware session and verbose prose erodes the always-on
  signal. Reused the per-rule audit methodology from the May 2026 pass
  (recorded in
  `specs/feature/kdevkit-feature-and-agent-dev-loops.md` Decision Log):
  Explore-agent classified passages into rule_only /
  rule_plus_load_bearing_rationale / rule_plus_narrative_rationale /
  worked_example / host_specific_mapping / template; cut policy dropped
  the latter three categories. Applied 31 edits across §1–§9. Result:
  628 → 473 lines (~25% cut). All load-bearing tokens still present
  (verified by grep): `plan(<feature>)`, `close(<feature>)`,
  `planning_phase`, `prefer_worktree`, threshold **70**, retry budget
  **2**, `Conventional Commits`, `Co-Authored-By`, "asking is the
  artifact". Section numbering §1–§9 intact. Quality + Test gates
  green; functional smokes deferred to user-driven judge runs.

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
- **2026-05-31 · Session-arc outline (rev 3).** Reviewer's outline
  reorders the skill so reading top-to-bottom matches the order an
  agent acts: locate → load project → load feature → start session →
  run (planning / dev / closure) → cross-cutting rules. The reference-
  manual layout we had before forced agents to assemble the
  three-phase model from §6 (session behaviour), §8 (dev loop), and
  §9 (closure) — three sections that are siblings of the same thing.
  Refinements applied before implementation: (1) operational gating
  (YOLO, ambiguous → plan) stays in §5 (Run feature session), not
  §9 (Cross-cutting), since these fire *during* execution; (2) §6 /
  §7 / §8 each end with an explicit `Apply: §9 grep + commit
  hygiene` pointer so the cross-reference is one hop, not assembled;
  (3) the spec-already-drafted rule lives in §3 with its consequence
  ("→ §6 Planning, not §7 Dev") in the same sentence; (4) §9 header
  carries the `(always-on)` label so the agent reads it as
  fire-everywhere. Single-digit unique numbering (the reviewer's
  draft outline had two §5s).
- **2026-05-30 · Second compression pass — methodology reuse.** Same
  per-rule audit + cut classifier as the May 2026 pass. Skill is
  always-on context for long feature sessions; verbose prose erodes
  recall. Cuts targeted: `project.md` and backlog template HTML
  comments (intent prompts to a human author, not load-bearing rules);
  duplicated phase-gating prose between §3 and §6; verbose intros to
  numbered lists in §8 and §9; the §6.3 cheat-sheet's restatement
  paragraph; the §8 command-resolution numbered list (folded to one
  sentence). Kept verbatim: every protected rule (§5 commit types
  incl. `plan`/`close`, §8 score 70 / retry 2, §9 numbered step
  sequence, the feature-file template the agent fills as a prompt,
  §7's grep rule). Behavior verification is judge-mode user-driven —
  fixtures unchanged, surfaced for the user to run before merge.
