# Feature: kdevkit — cross-tool verification (stream 6)

Part of initiative: [[kdevkit-decompose-and-harden]] (stream 6)

Branch: `test/kdevkit-cross-tool`
Worktree: `maid-worktrees/kdevkit-cross-tool`

## Feature Brief

Run the whole decomposed, hardened kdevkit workflow against **kiro**
and **codex** — the two non-Claude coding agents mAId targets — to
confirm the prose decomposition (streams 1–3) and the A/B fixes hold
outside Claude Code. The initiative's mission invariant is that mAId
is tool-agnostic: a decomposition that only works on one host is a
regression regardless of how well it reads.

## Why

- **The mission invariant.** Streams 1–3 and the A/B fixes were all
  verified on claude only. Stream 6 is where "portable surface"
  stops being an assumption.
- **Ordered before streams 4–5 deliberately.** 4–5 build
  tool-specific mechanism (deterministic phasing, session
  orchestration) on top of the portable prose layer. Verifying that
  layer holds cross-tool *first* means any tool-specific assumption
  baked into 4–5's design is caught before it is expensive to
  unwind, not after.

## Requirements

- **R1.** The full fixture suite runs against kiro and codex, paid,
  both stages (check + smoke).
- **R2.** Every failure is traced to one of: a real skill defect
  (fix the skill), a fixture bug (fix the fixture), or model/judge
  noise (retest to confirm, leave as-is).
- **R3.** The three A/B fixes (consolidation-skip, backlog-filing,
  `Phase:` staleness) are confirmed holding on both kiro and codex.

## Design

The run surfaced four distinct classes, three of which needed a fix.

### A · Fixture bug — lettering-ban assert too broad (fixed the fixture)

`kdevkit-consolidate.smoke` banned any `(a)/(b)/(c)` lettering
anywhere in the spec as evidence of an unconsolidated Design. But
the fixture's own sibling assert *requires* the rejected-alternatives
rationale to be relocated into the Decision Log — and the settled
form of that, used across this repo's own Decision Logs and seeded
verbatim in `kdevkit-consolidated-resume.smoke`, is
`Alternatives rejected: (a) … (b) …`. So a compliant agent that
relocated correctly failed the ban. Hit kiro (enact) and codex
(integration) — both did the *right* thing and were punished.

Fixed by scoping the ban to exclude the Decision Log section
(`awk '/^## Decision Log/{f=1} !f'`), mirroring how `dev.md`'s own
consolidation-check scopes to Design-only. Adversarially reprobed:
a non-compliant agent that leaves lettering in Design still fails.

### B · Kiro skill defect — spurious `plan()` handoff commit (fixed the skill)

Kiro treated the dev→review Handoff rewrite as its own phase
transition and committed it as `plan(<feature>): hand off dev ->
review`. Two violations: §1's "review is the back half of the dev
loop, not a fourth phase, no entry cue of its own," and the
established convention (stated in `kdevkit-phase-boundary.smoke`'s
own comment) that a dev-phase Handoff rewrite is never a `plan()`
commit. Root cause: `dev.md`'s "Leaving dev" said to rewrite the
Handoff but never named which commit carries it, so kiro invented
a commit type to fill the gap.

Fixed by stating in "Leaving dev" that the rewrite rides the same
dev-type commit as the work it hands off (new commit per §9, never
an amend), and that inventing a `plan()` commit manufactures a
boundary the workflow does not have.

### C · Prose ambiguity — no-remote reading (fixed the skill)

With no git remote configured, codex read "after Push, rewrite the
Handoff with `Phase: review`" literally: Push can't happen without a
remote, so it held at `Phase: dev`. Claude and kiro advanced anyway.
The fixtures (none of which seed a remote) assert advancement, so
codex's reading — though defensible — was the outlier.

Fixed by stating in the Push Gate that a missing remote is an
environment gap to report, not a reason to hold at `Phase: dev`:
the gates decide readiness and they've passed, so proceed with the
Handoff rewrite as if Push had succeeded.

### D · Model/judge noise (no change — retested to PASS)

Four first-pass failures reproduced as PASS on immediate retest
with no code change, confirming judge/model variance rather than
defects: `kdevkit-handoff-resume playback` (kiro — judge misread a
compliant dev→review→closure Handoff description as "inventing a
fourth phase"), `kdevkit-dev-loop playback` (codex), and
`kdevkit-closure enact` (codex — asked once about a missing
`project.md`, did not on retest). A fresh-context reproduction of
the handoff-resume answer against the actual skill text confirmed
the compliant answer matches what the skill prescribes, ruling out
a real defect.

### Not a finding — known harness leak

A `notes` fixture committed a stray insight file into this
worktree's real checkout (HEAD moved). This is the pre-existing,
already-tracked `test-runner-workdir-containment` gap — unrelated
to kdevkit. Cleaned up with `git reset --hard` to the merge commit.

## Test Strategy

The test is the run itself: 18 fixtures × 2 agents × 2 stages, paid.

| Fixture / kind | First pass | After fix | Class |
|---|---|---|---|
| consolidate enact (kiro) | fail | pass | A — fixture |
| consolidate integration (codex) | fail | pass | A — fixture |
| consolidated-resume enact (kiro) | fail | pass | B — skill |
| handoff-resume enact (codex) | fail | pass | C — skill |
| handoff-resume integration (codex) | fail | pass | C — skill |
| handoff-resume playback (kiro) | fail | pass (retest) | D — noise |
| dev-loop playback (codex) | fail | pass (retest) | D — noise |
| closure enact (codex) | fail | pass (retest) | D — noise |
| browser-safety playback (kiro) | fail | — | non-kdevkit, out of scope |
| all other kdevkit fixtures | pass | pass | — |

## Implementation Plan

- [x] 1 · Run full paid suite against kiro + codex, both stages.
- [x] 2 · Trace every failure to skill-defect / fixture-bug / noise.
- [x] 3 · Fix A (consolidate.smoke Decision-Log scoping);
      adversarially reprobe.
- [x] 4 · Fix B (dev.md handoff-commit type); reverify on kiro.
- [x] 5 · Fix C (dev.md no-remote Push reading); reverify on codex.
- [x] 6 · Retest the noise failures to PASS; confirm no code needed.
- [ ] 7 · Quality + Test gates; commit; push; Review Gate.
- [ ] 8 · Review Briefing; Agent-dev Review Gate.
- [ ] 9 · Closure: reconcile, initiative Status update (last stream →
      archive the initiative spec), backlog check, squash-merge.

## Handoff

- **Phase:** dev — all fixes landed and reverified on the affected
  agent/kind combinations; ready for gates and review.
- **Ready for:** Quality + Test gates, then Review Briefing and the
  Agent-dev Review Gate.
- **Carry forward:** the browser-safety playback failure on kiro is
  a non-kdevkit skill, out of this feature's scope — do not fold it
  into closure here; it belongs to whatever owns the browser skill.
  Fixes B and C both live in `dev.md`'s Push Gate / Leaving-dev
  section, so a reviewer should read those two together.
- **Deliberately left:** the known harness-containment leak
  (`test-runner-workdir-containment`) recurred and was cleaned up
  manually; not fixed here — it is a test-runner concern, already
  filed, not a kdevkit-prose concern.

## Session Log

- **2026-08-25 · Full paid suite run against kiro + codex**
  (18 fixtures × 2 agents × 2 stages). First pass: 82 PASS, 4 SKIP,
  8 kdevkit-related failures + 1 non-kdevkit (browser) + 2 harness
  leaks. Traced each: 1 fixture bug (A, two occurrences), 2 real
  skill defects (B, C), 3 noise (D), 1 out-of-scope, 1 known leak.
- **2026-08-25 · Fixes + reverify.** Fixed A/B/C; adversarially
  reprobed A against a non-compliant agent; reverified each fix on
  the agent/kind that caught it (consolidate enact+integration,
  consolidated-resume enact, handoff-resume enact+integration all
  PASS). Retested the three noise failures to PASS with no change.

## Decision Log

- **2026-08-25 · Fix C toward "advance Phase anyway," not "hold at
  dev."** Rationale: Push is a mechanical follow-on once gates are
  green; a missing remote is an environment gap the agent should
  flag, not a reason to leave the Handoff stale. Also matches what
  claude and kiro already did and what every fixture asserts
  (none seed a remote). Alternative rejected: keep the strict
  reading and update the fixtures to assert `Phase: dev` on
  no-remote — rejected because it would make the Handoff lie about
  readiness the gates already established.
- **2026-08-25 · Fix A in the fixture, not the skill.** Rationale:
  the skill behaviour was correct; the assert was wrong to ban a
  form the skill's own consolidation rule produces. Scoped the ban
  to Design-only rather than loosening it entirely, so it still
  catches a genuinely unconsolidated Design.
