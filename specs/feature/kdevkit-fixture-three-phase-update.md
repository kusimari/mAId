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

The judge-mode functional smoke fixtures under
`tests/functional/skills/kdevkit-*.smoke` were authored against the
old §6/§8/§9 structure and pre-three-phase semantics. They no longer
match the live skill's behavior contract — running them today against
the merged skill will judge it against an obsolete narrative.

This feature realigns the four existing fixtures to the new structure
and adds a fifth fixture for the planning-phase entry behavior.

## Requirements

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

- **Quality gate (load-bearing):** `deno task fmt` + `deno task lint`
  + `deno task check`. Fixture files are plain-text, so the gates run
  against the maid/ + tests/ trees only and confirm nothing else
  broke.
- **Test gate (load-bearing):** `deno task test:unit` — the existing
  22-test suite. Should pass unchanged; fixtures aren't a consumed
  surface.
- **Smoke (load-bearing):** `deno task test:smoke` — confirms the
  fixtures are well-formed (the harness parses every `.smoke` file
  and exits non-zero on a malformed one) and that the kdevkit symlink
  still resolves.
- **Functional (judge mode, user-driven):** the *behavior* signal for
  this feature is the user running `deno task test:functional` (or a
  single fixture via `./tests/functional/run <name>`) against the
  updated kdevkit skill. The agent prepares the fixtures and hands
  off; whether to spend the API budget is a human call.
- **A/B baseline:** none formally captured. The previous
  fixture-update pass (May 2026) ran A (pre-edit) before B
  (post-edit) for each fixture; this pass can do the same when the
  user runs the judge.

## Implementation Plan

1. Update `tests/functional/skills/kdevkit-feature-loop.smoke` —
   extend prompt and rewrite `expected_narrative` per Requirements.
2. Update `tests/functional/skills/kdevkit-dev-loop.smoke` —
   rewrite `expected_narrative` per Requirements.
3. Update `tests/functional/skills/kdevkit-feature-closure.smoke` —
   rewrite `expected_narrative` reordered to the new §8 sequence,
   adding the title-rewrite rule.
4. Add `tests/functional/skills/kdevkit-feature-planning.smoke` —
   new judge fixture per Requirements.
5. Run `deno task fmt && lint && check && test:unit && test:smoke`.
6. Stage + commit per the three-phase convention; push the feature
   branch; open the PR via the Planning Review Gate (this spec
   itself ships as `plan(<feature>): initial spec` first, dogfooding
   the rule).

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

## Decision Log

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
