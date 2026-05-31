# Feature: kdevkit-fixture-three-phase-update

## Git Setup

- Branch: `feat/kdevkit-fixture-three-phase-update`
- Base: `main`

## Feature Brief

PR #11 (`feat(kdevkit): three-phase feature branch — plan/agent-dev/close`)
shipped substantial structural changes to the kdevkit skill: new
section numbering (§1–§9 in session-arc order), the spec-already-drafted
rule in §3, two new commit types (`plan(<feature>):` /
`close(<feature>):`), per-phase Review Gate body shapes, and a
restructured §8 close-out that stages reconciliation + project.md
verify + backlog cleanup as commits *before* the squash-merge.

Two related gaps surfaced when reviewing this CR:

1. **Fixtures didn't update with the implementation.** The judge-mode
   functional smoke fixtures under
   `tests/functional/skills/kdevkit-*.smoke` were authored against
   the old §6/§8/§9 structure. The agent-dev loop on PR #11 made
   substantial structural edits but never updated the tests that
   evaluate the change — so the live skill is now judged against an
   obsolete narrative. This is a **symptom**, not the root cause.
2. **The root cause: feature planning has tests *after* design.**
   With the current §6 interview order
   (Requirements → Design → Test Strategy → Implementation Plan),
   tests are a downstream concern that the dev loop can drift away
   from. If tests are designed *after* implementation is sketched,
   they describe what the agent built, not what success means. By
   the time the dev loop is running, tests are already the lowest-
   priority item in the spec. **Reordering** to
   Requirements → **Test Strategy** → Design → Implementation Plan
   forces tests to describe success criteria before the design
   converges, so the dev loop has a target to verify against.

This feature does both: reorder §6's interview structure (and the
feature-file template) to put tests before design, and realign the
five fixtures to the new structure.

## Requirements

### Reorder §6 interviews and feature-file template

The current §6 four-interview order — Requirements → Design → Test
Strategy → Implementation Plan — treats tests as a downstream
concern. Reorder to **Requirements → Test Strategy → Design →
Implementation Plan** so tests describe success criteria before the
design converges. The dev loop now has a verifiable target instead
of a sketch-after-the-fact validation step.

Reordering touches:

- §6 "Four short interviews" — reorder the numbered list (no prose
  rewrite of individual interview prompts).
- §6 "Feature file template" — reorder the headers
  (`## Test Strategy` moves above `## Design`).
- §6 "Planning Review Gate" body-shape spec — the **Spec summary**
  block currently lists `R / D / T / I one-liners`; update to
  `R / T / D / I` to match the new order.

The Test Strategy interview should also lean explicitly on
`project.md`'s Testing section — when the project declares which
test layers are load-bearing, the feature's tests should map onto
those layers, not invent a new structure. Tighten the interview
prompt to call this out: *"Test strategy. Per project.md's Testing
section: which layers fire for this change, what are the success
criteria, what's load-bearing vs. nice-to-have?"*

This reorder also affects every existing feature spec on disk
(`specs/feature/*.md`). They stay as-is — the rule applies to new
specs from this point forward; existing files keep their authored
shape (rev-2 compression precedent). The closure-phase reconcile
sweep doesn't rewrite section order on shipped features.

### Update existing fixtures

- **`kdevkit-feature-loop.smoke`** — prompt currently says "I'm
  starting a new feature… 'let's start feature X'". Extend to also
  cover continue / pick-up entry mode, since §3 now explicitly
  distinguishes them. `expected_narrative` must:
  - Include the entry-mode resolution (start vs. continue/pick up;
    look in `feature/` first, fall back to `backlog/` and promote).
  - Include the spec-already-drafted → §6 Planning rule (a populated
    spec on disk is not a *reviewed* spec; start in planning, not
    dev).
  - Reference §4 (Start feature session) for worktree-pref handling
    rather than §3.
  - Stop chaining phases (existing rule, retain).
- **`kdevkit-dev-loop.smoke`** — prompt unchanged ("a coherent
  implementation slice is finished"). `expected_narrative` must:
  - Reference the four gates (Quality / Test / Push / Review)
    with current thresholds (70 score, retry budget 2).
  - Replace the "five-section body shape" guard with the
    agent-dev-phase body shape (Why + Approach required;
    Verification / Reading guide / Pairs with optional).
  - Drop the "five-section" prohibition wording from the wrong-answer
    list.
  - Keep the §9 internal-marker grep reference (one source of truth).
- **`kdevkit-feature-closure.smoke`** — prompt extends slightly
  ("the PR is approved and I say 'ship it'"). `expected_narrative`
  must reorder behaviors to the new §8 sequence:
  1. Reconcile in-flight markers in the spec.
  2. Soft project.md verify.
  3. Backlog cleanup — *interactive*, *asking is mandatory* even
     when the answer is "none".
  4. Stage all three as `close(<feature>):` commits **before** the
     squash-merge.
  5. Closure Review Gate — body rewritten to final shape; **PR
     title rewritten to dominant `feat(<scope>):` subject** so the
     squash-merge commit on `main` reads as a feature ship.
  6. Squash merge to main.
  7. Branch cleanup — local + remote, default delete, no permission
     pause.
  8. Worktree teardown — offer-only on non-primary worktrees.
- **`kdevkit.smoke`** (load-only substring) — unchanged; the
  question "what directory does kdevkit look for first" is still
  answered by `specs/`.

### New fixture

- **`kdevkit-feature-planning.smoke`** — judge fixture exercising
  the planning-phase entry behavior (the single biggest new rule):
  - Prompt: "I just promoted a backlog item to a feature spec; the
    spec is fully drafted. What do you do next?"
  - `expected_narrative`:
    - Recognises the populated-spec-on-disk → §6 Planning rule:
      a populated spec is not a *reviewed* spec.
    - Confirms readiness with the user or iterates on the spec.
    - Commits the spec as `plan(<feature>): initial spec` (commit
      type required; subject phrasing flexible).
    - Pushes the feature branch.
    - Runs the **Planning Review Gate** with a body containing
      **Why** + **Spec summary** (R / D / T / I one-liners) +
      **Open questions**.
    - Opens as a normal review (not draft).
    - Waits for the explicit planning → dev cue (e.g. "spec looks
      good" / "start build" / "plan approved") before any code work.
  - Tools: `claude,kiro` (same matrix as the other fixtures).

## Design

The fixtures are plain-text files with the shape:

```
prompt: <one line>
expect_substr: [kdevkit] applies
expected_narrative: <one paragraph>
tools: claude,kiro
```

Editing them is markdown-style prose — no executable code. The change
set is four file edits + one new file.

The harness (`tests/functional/run`) is unchanged. The structural
smoke (`deno task test:smoke`) remains a load-check; the judge mode
(`deno task test:functional`) is user-driven and produces the actual
A/B evidence.

## Test Strategy

Per `specs/project.md`'s Testing section, this project has four
layers: `test:unit` (load-bearing, default §7 Test Gate),
`test:smoke` (load-bearing, post-deploy structural check),
`test:functional` (user-driven judge mode), and `test:all`
(chained). Success criteria for this feature map onto each layer:

- **Success criterion 1 — SKILL.md still loads cleanly.**
  - `deno task fmt` clean (markdown not formatter-managed, but
    workspace stays well-formed).
  - `deno task lint` clean.
  - `deno task check maid/main.ts` clean.
  - `deno task test:unit` 22/22 — fixture and SKILL.md changes don't
    consume a surface the unit suite covers.
- **Success criterion 2 — fixtures are well-formed.**
  - `deno task test:smoke` — the harness parses every `.smoke` file
    and exits non-zero on a malformed one. Confirms the new
    `kdevkit-feature-planning.smoke` parses + the four edited
    fixtures still parse + the kdevkit skill symlink resolves.
- **Success criterion 3 — judge agrees the live skill matches the
  fixtures' new narratives.** User-driven; runs:
  - `./tests/functional/run kdevkit-feature-planning` (new fixture).
  - `./tests/functional/run kdevkit-feature-loop` (covers entry-mode
    + populated-spec → §6 Planning).
  - `./tests/functional/run kdevkit-dev-loop`.
  - `./tests/functional/run kdevkit-feature-closure`.
  - `./tests/functional/run kdevkit` (load-only substring; should
    PASS unchanged).
  - The judge's verdict is the only signal that the SKILL.md
    rewrite + fixture rewrite actually align.

The agent prepares the fixtures and surfaces the exact run command;
whether to spend the API budget on the judge is a human call (per
`specs/project.md`'s "agentic runs must stop at `test:smoke`" rule).

**A/B baseline:** Capture A (current SKILL.md + current fixtures)
before the agent-dev edits, B (updated SKILL.md + updated fixtures)
after. Same methodology as the May 2026 fixture-update pass. The
agent records the run commands; the user runs them.

## Implementation Plan

All eight steps shipped in two `feat(kdevkit):` commits on this
branch:

- `ab9790c feat(kdevkit): reorder §6 interviews + realign smoke
  fixtures` — steps 1–6.
- `5e80246 feat(kdevkit): tighten §3 entry-flow ordering +
  judge-mode prompt nudges` — judge-mode dev-loop fix-up after
  the user authorized agent-run functional tests for this session.

1. **Edit SKILL.md** — reorder §6 four-interview list to
   Requirements → Test Strategy → Design → Implementation Plan;
   reorder the feature-file template's section headers to match;
   tighten the Test Strategy interview prompt to lean on
   `project.md`'s Testing section; update the Planning Review Gate
   body-shape line from `R / D / T / I` to `R / T / D / I`. Bump
   frontmatter version `2.4.0` → `2.5.0`. ✓
2. Update `tests/functional/skills/kdevkit-feature-loop.smoke` —
   extend prompt and rewrite `expected_narrative` per Requirements. ✓
3. Update `tests/functional/skills/kdevkit-dev-loop.smoke` —
   rewrite `expected_narrative` per Requirements. ✓
4. Update `tests/functional/skills/kdevkit-feature-closure.smoke` —
   rewrite `expected_narrative` reordered to the new §8 sequence,
   adding the title-rewrite rule. ✓
5. Add `tests/functional/skills/kdevkit-feature-planning.smoke` —
   new judge fixture per Requirements. ✓
6. Run `deno task fmt && lint && check && test:unit && test:smoke`. ✓
7. Surface the exact `./tests/functional/run <name>` commands so
   the user can capture the post-edit B baseline. ✓ — followed by
   actual judge-mode runs on user authorization (5e80246).
8. Stage + commit per the three-phase convention (this spec ships as
   `plan(...)` first, then a single `feat(kdevkit): …` for the
   SKILL.md + fixtures edits, then `close(...)` at the end). ✓
   — landed as two `feat(kdevkit):` commits because the dev loop
   surfaced real recall bugs in the SKILL.md ordering prose; both
   fixes ride into the squash-merge.

### Risk notes

- Judge fixtures are sensitive to phrasing. The rev-2 compression
  pass recorded a lesson: write `expected_narrative` to enumerate
  *behaviors that change agent action if dropped*, not surface
  phrasings. New / updated narratives here need to follow that rule
  — flag literal verbs as flexible, flag commit types and the
  title-rewrite rule as required.
- Tool divergence (claude vs. kiro) historically affected §3
  worktree-preference recall and §9 squash-merge non-default-host
  exceptions. Worktree-pref is now in §4 — re-running the judge will
  reveal whether the section move improves or regresses kiro recall.
- Adding a fifth fixture inflates judge-mode runtime (one fixture ≈
  one tool call per tool ≈ one judge call per tool). Cost grows
  linearly; user judges per-feature cost on every revision.

## Session Log

<!-- newest at top -->

- 2026-05-31 · **Feature closed via §8 close-out.** Shipped as
  PR #12. Four commits across the branch (planning + behavior +
  dev-loop fix-up + closure):
  - `0317935 plan(kdevkit-fixture-three-phase-update): initial spec`
  - `dec5330 plan(...): widen scope — reorder §6 interviews`
  - `ab9790c feat(kdevkit): reorder §6 interviews + realign smoke
    fixtures`
  - `5e80246 feat(kdevkit): tighten §3 entry-flow ordering +
    judge-mode prompt nudges`
  - (plus this close-out commit)

  User authorized agent-run functional tests for this session,
  overriding `specs/project.md`'s "agentic runs must stop at
  test:smoke" rule. Three rounds of agent-driven dev-loop
  fix-and-retry: ~30 judge invocations across both tools surfaced
  two real recall misses (kiro placing plan-commit after the user
  cue; both tools occasionally eliding the worktree project.md
  preference under length pressure) and two over-strict fixture
  patterns (R/T/D/I letter-form enumeration; literal
  `kdevkit.prefer_worktree` key-name requirement). Final 3x full
  confirmation: 15/15 fixture × tool × check cells PASS, 100%
  stable.

  Closure round dogfooded the new §8 sequence: reconciliation
  (this entry), soft project.md verify (no edits — feature
  touched only SKILL.md + fixtures), backlog cleanup (asked, no
  matches: existing items `maid-as-flake-package` and
  `writing-style-mcp-server` are unrelated to fixture
  realignment). Closure Review Gate rewrites PR title from
  `plan(...)` to `feat(kdevkit): reorder §6 interviews + realign
  smoke fixtures` per §5.

## Decision Log

- **2026-05-31 · Reorder §6 interviews:
  Requirements → Test Strategy → Design → Implementation Plan.**
  The current order treats tests as a downstream concern that the
  dev loop can drift away from. Reviewer feedback on PR #12: "our
  agent-dev loop in the previous feature session did not update
  tests as part of its loop. Otherwise we would not have needed
  this CR at all." Symptom: fixture drift on PR #11. Root cause:
  tests came after design in §6, so the dev loop had no verifiable
  success target — fixtures were treated as a separate maintenance
  task that the loop didn't own. Putting tests immediately after
  requirements forces the spec to declare success criteria *before*
  the design converges, so the dev loop's Quality + Test Gates have
  something specific to verify against. Tightening the Test Strategy
  prompt to lean on `project.md`'s Testing section also forces the
  interview to map onto the project's existing layers
  (`test:unit`/`test:smoke`/`test:functional`) rather than invent a
  new structure per feature.

- **2026-05-31 · Tests are part of the agent-dev loop, not a
  follow-up.** Implicit corollary of the reorder: when an
  implementation slice changes a behavior the fixtures evaluate, the
  fixture update lands in the *same* dev-loop iteration as the
  behavior change. The Test Gate (§7) currently runs `test:unit` by
  default; for skill-prose changes, the `test:smoke` parse-check is
  load-bearing too, and the user-driven `test:functional` is the
  *behavior signal*. The agent's responsibility on a skill-prose
  change: prepare the updated fixtures, confirm they parse via
  `test:smoke`, surface the `test:functional` command for the user
  to run. PR #11 should have done this in the same loop instead of
  shipping fixture drift to PR #12.

- **2026-05-31 · Existing feature specs are not retroactively
  reordered.** New feature specs from this point forward use
  R → T → D → I; existing files in `specs/feature/` keep their
  authored shape. Rationale: rewriting shipped specs would be a
  no-behavior-change churn pass that dilutes git history and risks
  link-rot in commit messages / PR bodies that reference specific
  sections. Same precedent as the rev-2 compression pass — apply
  forward, not backward.

- **2026-05-31 · Add a fifth fixture for planning-phase entry.**
  Considered folding planning coverage into the existing
  `kdevkit-feature-loop` fixture's `expected_narrative`. Rejected:
  the spec-already-drafted-→-Planning rule is the single biggest
  behavior the rev-2/rev-3 changes added, and the planning-phase
  commit + Review Gate body shape are independent rules. Folding
  would force a long narrative that the judge can't enforce
  precisely. One fixture per phase keeps the signal-to-noise high.

- **2026-05-31 · Closure-fixture narrative covers title-rewrite rule.**
  The Closure Review Gate's PR-title rewrite (from `close(<feature>):`
  to the dominant `feat(<scope>):` subject) is a load-bearing rule —
  without it, the squash-merge commit on `main` reads as a closure
  mechanic instead of a feature ship, regressing §5. Adding to the
  fixture so an agent that misses the rule fails the judge.

- **2026-05-31 · Drop the "five-section body shape" prohibition from
  the dev-loop wrong-answers list.** Old `expected_narrative` flagged
  "treating the body shape as fixed-five-sections" as wrong because
  the original §8 body shape was Why + Approach + optional. The
  new §7 keeps that structure, so the prohibition is still
  technically valid — but it's now a false-flag risk: an agent that
  *correctly* names Why + Approach + Verification + Reading guide +
  Pairs with (the optional union) gets penalised. Dropping the
  prohibition keeps the fixture from over-fitting.
