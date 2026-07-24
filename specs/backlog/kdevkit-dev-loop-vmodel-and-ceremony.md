---
name: kdevkit-dev-loop-vmodel-and-ceremony
description: One kdevkit SKILL.md session that lands three interlocking §5–§7 rules — a ceremony-lane classifier (trivial/small/real-feature), test-first-per-slice on the real-feature lane (V-model extended into the dev loop), and an authoring-convention rubric passed to the Code Review Gate. Merged from three backlog items that all edit overlapping §5/§7 prose and share the lane classifier.
metadata:
  type: backlog
---

# kdevkit — right-size the dev loop: ceremony lanes + test-first + authoring-convention gate

## Why these three are one session

All three are always-on prose edits to the *same* file
(`resources/content/skills/kdevkit/SKILL.md`), all in §5–§7, and
they interlock through one mechanism — a **ceremony-lane
classifier**:

- **Rule A (ceremony lanes, §5)** introduces the classifier:
  trivial/mechanical → small-with-a-fork → real-feature. It's the
  prerequisite.
- **Rule B (test-first-per-slice, §7 Test Gate)** fires *only on
  the real-feature lane* — without A, it would make one-line
  edits pay for a red-green dance, which is the exact
  over-ceremony complaint A exists to fix.
- **Rule C (authoring-convention gate, §7 Code Review Gate)**
  also keys off the lane (a trivial edit doesn't need the full
  authoring rubric) and shares the §7 dispatch prose B touches.

Landing them separately means three sessions editing overlapping
§5/§7 prose and three rounds of the same Code Review Gate — pure
waste. **Sequence within the session: A first (prerequisite),
then B and C.** One planning phase, one PR/CR, one `SKILL.md`
version bump.

---

## Rule A · Right-size the ceremony to the change (§5, always-on)

### What

Give kdevkit an explicit, always-on rule that scales its process
weight to the size of the change, so a one-line edit does not get
a four-interview spec, R1–Rn requirements, a Test Strategy table,
and a formal Planning Review Gate.

Today the skill has the *mechanism* (`planning_phase: false` in
the `kdevkit` block, plus the backlog tier for small items) but no
*guidance on when to reach for it*. SKILL.md §5–§6 read as "always
run the three phases," so an agent applies the full planning
ceremony by default on any change — including trivial ones. The
opt-out exists but is easy to miss in the moment.

Add a short "altitude of ceremony" rule near the head of §5 that
classifies the incoming work and picks a lane:

- **Trivial / mechanical** (one-line edit, config value, change
  that inherits all behavior from existing code) — skip the four
  interviews and the Planning Review Gate; a one-line Decision Log
  entry (or just the commit message) captures any real fork. Go
  straight to the dev loop (Quality → Test → Code Review → Push).
- **Small feature with a genuine design fork** — no full spec, but
  record the fork. A backlog-style note or a Decision Log line is
  enough; the Code Review Gate still runs.
- **Real feature** (multi-file, new surface, cross-repo,
  sequential streams) — the full §6 planning phase as written.

The signal is "how much undetermined design is there," not "is
this repo kdevkit-managed." The gates that protect correctness
(Quality / Test / Code Review) stay on for every lane; only the
*planning paperwork* scales down.

### Why

- **Observed live (2026-07-15).** A one-line addition to a
  package-install array plus a small non-fatal post-install step
  produced a full 6-section feature spec, two planning commits,
  and a formal Planning Review Gate before any code. The user
  pushed back: "why did you need such an elaborate spec for such a
  simple feature?" The spec even admitted the change inherits all
  its behavior from an existing loop — the exact trivial-lane
  signal.
- **The one useful artifact was tiny.** A single real design fork
  (run the tool's first-run setup? make it fatal?) deserved one
  Decision Log line, not a section apiece for
  requirements/tests/design.
- **The opt-out is under-advertised.** `planning_phase: false` is
  documented as a project-wide setting; nothing tells the agent to
  reach for the lighter lane *per-change*.
- **Correctness gates were never the problem.** The complaint is
  planning paperwork, not review/tests. The rule scales *planning*
  and explicitly keeps Quality/Test/Code Review on for every lane.

### Open questions

- **Where the rule lives.** Best fit: a new always-on subsection
  at the head of §5. Alternatively fold into §5's "Phase-gating
  cues." One source of truth — avoid scattering.
- **How to classify without a new interview.** The classification
  must not itself become ceremony. Likely a 3-line heuristic the
  agent self-applies silently, surfacing only its choice ("this is
  a one-line change; skipping the planning phase — say so if you
  want the full spec").
- **Per-change vs. per-project.** Per-change heuristic always on;
  suggest setting `planning_phase: false` when the agent notices a
  pattern of trivial changes. Probably both.
- **Interaction with the backlog tier.** For "small feature with a
  fork," define the minimum viable record so it doesn't drift back
  into a full spec.
- **Default bias.** When genuinely unsure, lean light and *offer*
  the full spec rather than defaulting to full and making the user
  ask for less.

---

## Rule B · Extend the V-model into the dev loop — test-first per slice (§7 Test Gate)

### What

The planning phase (§6) runs the V's downstroke: Requirements →
**Test Strategy** → Design → Implementation Plan — success
criteria declared before the design converges. The dev loop (§7)
does *not* carry the same ordering: its Test Gate says tests land
"in the same iteration, not a follow-up" but leaves within-slice
ordering open, so the agent tends to write source then tests that
pass against it.

Proposal: on the **real-feature lane** (Rule A), for each slice
whose success criterion the §6 Test Strategy maps to a test layer
— **write the test first, confirm it fails for the right reason,
then write the source until it passes.**

### Evaluation (deep research — the recommendation)

**Adopt "test-first-per-slice," not dogmatic red-green-refactor
TDD.** The distinction is the whole answer.

- **The load-bearing benefit is "confirmed-red."** A test written
  *after* the source often asserts the code's accidental behavior,
  passes on the first run, and a vacuous assertion is
  indistinguishable from a real one. Writing it first and watching
  it go red is the guard. This is the honest dev-time analogue of
  the plan's "declare success criteria before the design
  converges."
- **The classic TDD benefits don't transfer here.** Interface
  discovery and emergent design are real for a human coding blind;
  in this workflow §6 already designed the interface ("Reach for
  what exists") and decomposed the plan. Forcing test-first as an
  *interface-discovery* tool re-litigates a §6 decision; emergent
  mid-dev design is what §9's "no scope creep / no silent plan
  amendments" pushes against. The transferable core is
  **criteria-first + confirmed-red**, not the ceremony.
- **The user's objection bounds the design.** "This makes us write
  tests without the source." In compiled, statically-typed code
  (this repo is Rust) a test against a not-yet-existing
  symbol yields a *compile error*, not a clean assertion-failure
  red — literal TDD fights the toolchain. Realistic shape: stub
  the signature so it compiles and the assertion runs *red*, write
  the test, watch it fail for the asserted reason, then fill the
  body. Test-first in spirit without pretending the source doesn't
  exist.
- **It must itself be right-sized.** A one-line change / prose skill
  edit / config value has no meaningful red to confirm — mandating
  test-first there is the process-cargo Rule A sheds. Real-feature
  lane only. **This is why B cannot ship without A.**

### Recommended rule (§7 Test Gate)

On the real-feature lane, for each slice whose success criterion
the §6 Test Strategy maps to a layer (Requirements →
functional/integration; Design → unit):

1. Write the test from the mapped success criterion, against a
   stubbed signature where the language needs one to compile.
2. Run it; **confirm it fails**, and for the asserted reason (not
   a compile/setup error masquerading as a failure).
3. Write source until it passes.
4. Downstream Quality → Test → Code Review ordering unchanged.

Keep it a **legibility/discipline default, not a hard gate** —
same altitude as "Write for intent" / "Reach for what exists." The
Code Review Gate (Rule C) may note a slice whose test was
obviously retrofitted (passes trivially, asserts nothing);
phrasing/ordering is not a hard-stop. Trivial lane: skipped.

### Why

- **Symmetry the skill already half-commits to.** §6 pins
  functional/integration tests to Requirements and unit tests to
  Design (V-model pairing, SKILL.md ~§373). The plan runs the
  downstroke; the dev loop doesn't run the upstroke in the same
  order. Closing that is conceptually clean.
- **Guards a real failure mode.** Retrofitted tests that assert
  incidental behavior pass both the Test Gate and the Code Review
  Gate silently — same class of gap as the authoring miss Rule C
  targets. Confirmed-red is the cheap guard.

### Open questions

- **Where the rule lives.** §7 Test Gate (it already owns "tests
  land in the same iteration"). Confirm it reads as one more §7
  default, not a new phase.
- **Compiled-language carve-out wording.** One sentence that names
  the "stub-to-compile, then red" shape without turning a
  language-agnostic skill into a Rust-specific one.
- **Gate vs. default.** Confirmed as a default. Revisit only if
  retrofitted-test misses survive review in practice.
- **Behavioral fixture.** Ordering isn't an artefact — a
  `kdevkit-dev-loop` fixture likely stays judge-mode (as the
  gate-ordering reasoning already does), not setup/assert.

---

## Rule C · Code Review Gate enforces always-on authoring conventions (§7)

### What

Extend the §7 Code Review Gate **dispatch contract** so the
reviewer receives the skill's own always-on *authoring*
conventions, not just `project.md` + the diff. Today those
conventions ("Comments carry intent, not history", "Write for
intent", "Reach for what exists") live in `SKILL.md` and are given
to *no* gate — so nothing but the author's in-the-moment memory
enforces them.

The contract at SKILL.md (§7 "Dispatch contract" / "Receives:" /
"Excluded:") currently passes the reviewer `project.md`, the diff
vs. base, and reviewer reference + threshold + authority +
retry_budget — and deliberately excludes the feature spec,
session/decision logs, and history. That exclusion is correct (it
keeps the review honest about diff-vs-project). But it means the
reviewer also never sees the always-on authoring rules, which are
in neither `project.md` nor the diff.

Proposed change: add a fourth item to "Receives:" — a small,
fixed extract of the always-on **authoring** conventions (the ones
judging *how code and comments are written*, distinct from
design/planning rules):

- "Comments carry intent, not history" (§7 Write for intent).
- "Write for intent" (frame functions around caller intent; reach
  first for what's in reach; match surrounding style).
- "Reach for what exists" (§6, design-time) insofar as it's
  checkable from the diff.

Keep the feature-spec / logs / history exclusions exactly as they
are — this adds only the skill's own authoring rubric, not feature
context. Gate off the lane (Rule A): a trivial-lane change doesn't
need the full rubric.

### Why

- **Observed live (2026-07-15).** In a kdevkit session the agent
  wrote comments that narrated the code ("keeps set -e happy" next
  to `|| true`; an `if`-guard comment restating the guard),
  violating "Comments carry intent, not history" — *even though
  the skill was loaded and the rule was in context.* The Code
  Review Gate scored 90/100 without flagging it, because the
  reviewer's dispatch never included that rule. The user caught it
  manually.
- **The convention was orphaned from every gate.** Quality Gate is
  deterministic-only; Test Gate is behavioral; Code Review is the
  *only* gate that can catch a subjective authoring miss — and
  it's dispatched without the document defining those conventions.
  So an always-on authoring rule is enforceable *only* by the
  author remembering it mid-write: the weakest possible
  enforcement, and the exact failure mode observed.
- **Right-sized, not ceremony.** Reuses the existing gate and adds
  one input, rather than a new "verify all standing instructions"
  phase. The broad-checklist version is explicitly *not* wanted.

### Open questions

- **Which rules qualify as "authoring" vs. "planning."** The
  reviewer should get rules that judge the *diff as written*
  (comments, function shape, reuse-in-diff), not planning-phase
  rules (four interviews, requirements smell test) with no diff to
  check against. Needs a bright line so the extract stays small.
- **How to pass it without bloating the dispatch.** Options: (a) a
  short inlined rubric string; (b) a pointer to a specific
  `SKILL.md` section the reviewer inline-Reads; (c) a dedicated
  deferred `review-rubric.md`. Lean toward (a) or (b) for a lean
  reviewer context.
- **Score interaction.** Hard-stop (< threshold) or advisory?
  Comment phrasing is lower severity than a correctness bug —
  likely reported but weighted so it doesn't alone sink the score.
  Define the weighting.
- **Author-side pre-commit self-check?** Considered and rejected
  as the *primary* fix (relies on author memory — the failure mode
  itself). Could be a lightweight secondary. Decide.
- **Interaction with `code-review` skill / host-native reviewer.**
  If the reviewer is a separate skill or host-native tool, confirm
  the extra input threads through its interface, not just the
  in-skill dispatch description.

---

## Trigger to promote

- Any one of the three fires its own trigger (over-ceremony on a
  small change; a retrofitted assertion-free test surviving both
  gates; another authoring-convention miss surviving Code Review).
- A batch of kdevkit SKILL.md edits is scheduled anyway — this
  whole cluster should ride together.

## Note on editing the skill

`resources/content/skills/kdevkit/SKILL.md` is the source behind
the managed skills symlink — edit it here in the repo, not under
`~/.claude/skills/kdevkit/`. Changes land in the next session. All
three rules are always-on operational content (§5/§7), so they
belong in `SKILL.md`, not the deferred `setup.md` / `interviews.md`,
per the skill's own multi-file placement rule.
