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
- **R4 (added mid-feature).** Any pass-rate claim rests on ≥3
  samples, not one — per the user's explicit correction after a
  single-sample "clean" result on kreviewkit turned out to be noise.
- **R5 (added mid-feature).** Before treating a prose fix as
  resolved, verify it survives conversational load (`--stressed`),
  not just a fresh single-shot read — per the user's explicit
  question about whether a skill can be "forgotten in a long
  session," which R3's claude/kiro/codex A/B fixes were never
  challenged on and which the kreviewkit fix failed.

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

### E · Codex prose-decay under session load (kreviewkit, not kdevkit — investigated, not solved here)

Sampling `kreviewkit`'s playback fixture (not the three kdevkit A/B
fixtures) N=5-6 per agent found real, reproducible flakiness on
codex — ~50%, not the ~100% a single clean sample suggested — while
claude and kiro held near-100%. The failure: codex intermittently
drops load-bearing contract points from a ~300-line prose skill,
even though the full body is loaded (confirmed: codex does not
truncate a directly-invoked skill at read time — this is a
retention/attention characteristic, not a loading bug).

Fixed the shared prose (checklist + tagged non-negotiables list +
terminal self-check — validated by both Anthropic's and OpenAI's own
skill/prompt-authoring guidance) and raised codex's *unstressed*
rate to 80%, no regression on claude/kiro. **Then stress-tested with
`--stressed`** (~4.6KB of unrelated prior conversation prepended) —
codex collapsed back to ~33%, the same pre-fix failure mode. The
fix survives a fresh single-shot read; it does not survive
conversational load. Claude held ~100% stressed.

This is kept as a real, documented improvement (better for
claude/kiro unconditionally, better for codex in the common
fresh-session case) — but it is explicitly NOT claimed as solving
cross-tool robustness under load, and it changed the initiative's
own D-open-1 decision: recorded as direct evidence that the
code-vs-prose boundary should resolve toward code owning phase
transitions, not further prose iteration, for anything that must
survive a long or resumed session.

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
- [x] 7 · Quality + Test gates (fmt-check, lint, 53 kaimux + build-tool
      unit tests, all green); commit; push; Review Gate (PR #44).
- [x] 8 · Review Briefing (PR #44 body, clean — no defects, two
      judgement questions); one tightening edit applied per the
      briefing (Leaving-dev no-remote wording made self-consistent).
- [x] 9 · Closure: reconcile, initiative Status update (stream 6
      shipped; streams 4–5 still open, so the initiative is NOT
      archived), backlog check, squash-merge.
- [x] 10 · Sampled kreviewkit's playback fixture N=5-6 per agent
      (per user request, before trusting a single-sample pass rate).
      Found ~50% adherence failure on codex specifically. Fixed the
      skill prose (checklist + tags + terminal self-check); raised
      codex to 80% unstressed. **Then stress-tested with
      `--stressed`** (per user's explicit ask, before declaring the
      fix good) — codex collapsed back to ~33% under conversational
      load, the same failure mode as before the fix. Claude held
      ~100% stressed; kiro showed a minor, inconclusive dip.
- [x] 11 · Recorded the stress-test finding against D-open-1 in the
      initiative spec: no amount of prose scaffolding reliably
      survives conversational load on codex — direct evidence for
      "code owns phase transitions," not better prose, being the
      right resolution. Structural-mechanism research is next.

## Handoff

- **Phase:** review — PR #44 open, merge held per explicit user
  instruction pending the codex finding.
- **Ready for:** merge, now that the codex finding has been
  investigated, recorded against D-open-1, and the prose fix is kept
  (net positive for claude/kiro, and codex's *unstressed* rate) with
  its stressed-load limitation explicitly documented rather than
  quietly assumed fixed.
- **Carry forward:** the kreviewkit prose fix does not solve
  cross-tool robustness under conversational load — only structural
  mechanism (streams 4–5, per D-open-1) can plausibly do that for
  codex. Do not read "shipped" on this feature as "codex problem
  solved" — it is "claude/kiro-side decomposition confirmed, codex
  gap characterized and evidenced, not closed." **Spot-checked, not
  fully verified:** ran one stressed sample each of
  `kdevkit-consolidate`/codex+kiro and `kdevkit-handoff-resume`/
  codex+kiro (R5's check was otherwise kreviewkit-only). 3/4 passed;
  stressed `kdevkit-consolidate`/kiro produced a *different* failure
  shape than kreviewkit's — not a dropped rule, but kiro becoming
  over-cautious and stopping to ask two clarifying questions (about
  the missing Planning Review Gate / no remote) instead of
  proceeding, when it correctly proceeded unstressed. **One sample is
  not a rate** — do not cite this as a percentage. A proper N-sample
  stress pass on the three A/B fixtures, across all three agents, is
  real, undone work for whoever picks up streams 4–5.
- **Deliberately left:** the `browser-safety` playback failure on
  kiro (non-kdevkit, belongs to the browser skill's owner) and the
  known `test-runner-workdir-containment` harness leak (test-runner
  concern, already filed) — both out of scope by design, neither a
  kdevkit-prose defect. Building any mechanical N-sample/lint
  tooling to catch this class of regression automatically was
  considered and explicitly deferred — the user redirected to
  "understand and fix it properly first," not "build infrastructure
  to keep re-detecting it."

## Session Log

- **2026-08-27 · Stress test overturns the "fixed" conclusion.** User
  asked, before applying the fix broadly: "find if a skill is
  forgotten in a long session — if true, the checklist might not
  help." Used the existing (never-before-exercised) `--stressed` flag
  — prepends `resources/tests/conversational-stream.txt` (~4.6KB of
  unrelated prior conversation) to the prompt. Result: codex's
  kreviewkit-playback rate fell from the unstressed 80% (post-fix) to
  ~33% under stress (2/6), reverting to the identical pre-fix failure
  mode (frames the contract as caller-defined, not the skill's own
  binding requirement). Claude ~100% stressed (3/3); kiro a minor,
  inconclusive dip (2/3, one sample). Conclusion: the prose fix is a
  real, worthwhile improvement for the common case (short/fresh
  sessions, and unconditionally better for claude/kiro) but does not
  make kreviewkit's contract durable against a loaded session on
  codex — and by extension, no purely-prose fix plausibly will.
  Recorded against D-open-1 in the initiative spec; this is direct
  evidence for resolving that decision toward code-owned phase
  transitions (streams 4–5) rather than continued prose iteration.
- **2026-08-26 · Sampling (N=5-6) finds real codex flakiness.** User
  declined to close `kreviewkit-playback-layer-unverified` on a single
  clean sample per surface, citing the backlog item's own "one sample
  proves nothing" rule. Ran 5-6 samples of kreviewkit + kdevkit-dev-loop
  playback on kiro/codex: kreviewkit-playback/codex was 3/6 (~50%),
  not the 1/1 the original run showed. Traced the failure to specific
  dropped contract points (V-model coverage read, the "would fixing it
  make it disappear" defect test, §3's bucket labels) — confirmed via
  a fresh-context reproduction that these ARE fully present in
  kreviewkit's SKILL.md, ruling out a content gap. Researched why
  (external: OpenAI's own GPT-5 guide documents instruction decay over
  long prose and recommends re-assertion + structured/tagged
  scaffolding; this is a genuine GPT-5-family trait, not a Codex CLI
  loading bug — confirmed skills are loaded in full, not truncated, at
  invocation time) and how (internal: codex's per-skill symlink
  deployment shape would make a codex-only override file cheap later,
  if ever needed — claude/kiro's whole-directory symlink would not).
  Applied the shared-file fix (non-negotiables checklist near the top,
  named tags around the three dropped points, terminal self-check) —
  raised codex to 80% unstressed with no regression on claude/kiro.
  User then asked to verify this under session load before trusting
  it (see next entry, chronologically first — dated 2026-08-27).
- **2026-08-25 · Full paid suite run against kiro + codex**
  (18 fixtures × 2 agents × 2 stages). First pass: 82 PASS, 4 SKIP,
  8 kdevkit-related failures + 1 non-kdevkit (browser) + 2 harness
  leaks. Traced each: 1 fixture bug (A, two occurrences), 2 real
  skill defects (B, C), 3 noise (D), 1 out-of-scope, 1 known leak.
- **2026-08-25 · Closure.** User confirmed both briefing judgement
  questions: keep fix C's "advance + name the skipped Push in Carry
  forward" over the stricter hold-at-dev reading, and merge on the
  single documented paid run (stream 6 is itself the verification
  stream; a second run is diminishing returns for two-line prose
  edits). NOT the last stream — streams 4–5 remain open (blocked on
  D-open-1) — so the initiative spec is updated, not archived.
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
