# Feature: kdevkit-separate-what-from-how

## Git Setup

- Branch: `feat/kdevkit-separate-what-from-how`
- Base: `main` at `f25e48a` (post-D / post-compaction)

## Feature Brief

Update the kdevkit skill so feature interviews produce a spec
that keeps **user-facing requirements** strictly separate from
**implementation design**, with tests sitting between them as
the contract layer. Today's spec template has a flat
`## Test Strategy` section after Requirements but before
Design, and the Requirements interview's prompt
(`Problem? Who interacts? Acceptance criterion?`) doesn't push
the user away from naming internal verbs, file paths, or
library names. The result is specs that braid "what the user
types" with "what writes to `~/.claude/settings.json`" —
reviewers who only care about UX have to filter the impl out
themselves, and tests phrased against internal field names
ossify the design into the test surface.

Three changes encode the discipline in the skill so the
agent gets the framing right without the user having to
correct each spec individually:

1. **Sharpen the Requirements interview prompt** in
   `interviews.md` to explicit "user-observable only"
   framing — what the user types, what they observe.
2. **Restructure the feature file template** in
   `interviews.md` to split tests around Design:
   functional/integration tests sit immediately after
   Requirements (validate user-observable behavior); unit
   tests sit after Design (validate design primitives).
3. **Add a smell test** to SKILL.md §6 — short prose the
   agent reads before writing each Requirements bullet so
   internal names get caught and moved to Design before they
   land.

Out of scope: migrating existing specs to the new layout
(left as-is until next substantial rewrite); changing how the
four interviews collect data (the four interviews stay, just
with sharper framing on Requirements); project.md structure
(this is a feature-spec change).

## Requirements

### Three-layer framing — capability / experience / design

The encoding maps the spec's existing top sections to three
layers, paired with the project's test layers in V-model
fashion:

- **`## Feature Brief`** carries the *capability* — what the
  user can now do that they couldn't before.
- **`## Requirements`** carries the *experience* — what the
  user touches, what they observe — verified by
  **functional / integration tests**.
- **`## Design`** carries the *design* — how it's built
  (schemas, plumbing, libraries, project conventions) —
  verified by **unit tests**.

The V-model pairing is the *justification* for the smell
test below: a Requirements bullet that names internals
can't be verified by a test phrased in user-observable
terms, so it's in the wrong section. The agent doesn't have
to memorise "internals → Design"; it follows from "every
Requirement gets a functional test."

This generalises across feature types — a CLI feature, an
app feature, a skill change to kdevkit itself, a service
endpoint. The capability layer answers "what can the user
do?", the experience layer answers "how do they encounter
it?", the design layer answers "how is it built?". For a
CLI: experience = flags and output. For an app: experience
= screens and visible state. For a skill change:
experience = the cues the agent recognises and the
artefacts it produces. The four-bullet smell test (§3
below) catches drift in either direction.

### Guidance, not rigid form

`interviews.md` is best-practice scaffolding — the spec's
exact section layout adapts to the feature. A spec without
design-primitive unit tests doesn't need an empty
`### Unit Tests` heading under Design; a flat
`## Test Strategy` is fine when the test surface is small.
The strictness lives in the **gates** (Planning Review §6,
Agent-dev Review §7 looping with Code Review, Closure
Review §8), not in heading shape. The skill's job is to
make the agent ask the right questions and avoid the
internal-names-in-Requirements failure mode; the spec's
shape is judgment.

### Requirements interview prompt — experience layer

The Requirements interview in `interviews.md` currently
asks: `Problem? Who interacts? Acceptance criterion?`. New
prompt shape pushes against the failure mode the
agent-session-orchestrator spec hit (mixing "user runs
`setup --key X`" with "writes a tagged entry to
`~/.claude/settings.json`"):

> _"How does the user experience this capability — what do
> they touch, what do they observe? For a CLI, flags and
> output. For an app, screens and visible state. For a skill
> change, the cues the agent recognises and the artefacts it
> produces. Library names, internal file paths,
> function/schema names, and protocol verbs go in Design —
> not here."_

The literal wording can flex; the load-bearing parts are
(a) the explicit "what do they touch / what do they observe"
framing, (b) the three feature-type examples (CLI / app /
skill change) so the agent doesn't default to CLI-thinking,
(c) the explicit no-list (library names / internal paths /
function names / protocol verbs), (d) the "those go in
Design" pointer so the user doesn't feel like they're being
told to drop the information entirely.

### Feature file template — V-model guidance

The feature file template in `interviews.md` carries the
V-model pairing as guidance, not as a fixed heading order.
Concrete changes:

- **Visible body placeholders** for `## Feature Brief`,
  `## Requirements`, `## Test Strategy`, `## Design` —
  HTML comments alone aren't enough (they vanish from
  rendered markdown; the agent loses the typed-slot
  affordance). Each section keeps its HTML guidance comment
  AND a visible `<...>` placeholder underneath.
- **Smell-test reference inside `## Requirements`'s HTML
  comment** so the discipline reaches the agent at
  template-fill time, not just from SKILL.md §6.
- **Optional Launch/Runtime split** mentioned in the HTML
  comment, not pre-baked as H3 headings — the agent uses
  judgment per feature.
- **Test Strategy guidance** notes the V-model default
  (functional/integration verifies Requirements; unit
  tests verify Design) and explicitly says: group cases
  under H3 subheadings when the test surface warrants;
  keep flat otherwise.
- **Design rationale leads the section.** HTML comment
  nudges "lead with rationale: why this shape, what was
  considered and rejected." No separate sub-heading.

What's NOT in the template: prescribed `### Functional /
Integration` / `### Unit Tests` H3 headings. Existing
specs in this repo all carry unit-test discussion inside
`## Test Strategy` (not under `## Design`); forcing the
new heading order would contradict the corpus and create
noise on specs whose unit-test story is "existing tests
stay green." The V-model framing in SKILL.md §6 + the
HTML guidance in the template carry the discipline; the
heading layout is judgment.

Sections NOT changing structurally: `## Git Setup`,
`## Implementation Plan`, `## Session Log`, `## Decision
Log` — all stay verbatim. The optional `Part of
initiative: [[<name>]]` line stays where it is
(immediately after `## Feature Brief`, per §6).

### Smell test in SKILL.md §6

SKILL.md §6 (the "Four short interviews" subsection) gains a
short smell-test subsection the agent reads before writing
each Requirements bullet. Constraint: lives in always-on
SKILL.md, not buried in `interviews.md` — the discipline
fires every time the agent writes a Requirements section,
and `interviews.md` is only inline-Read on fresh-feature
start.

The subsection has three parts:

1. **The three-layer framing** — Feature Brief = capability,
   Requirements = experience, Design = how-it's-built.
2. **The V-model pairing** — Requirements is verified by
   functional/integration tests; Design is verified by unit
   tests. This is the *justification* for the smell test:
   internals can't be verified by user-observable
   functional tests, so they belong in Design.
3. **The four-bullet smell test** — categories that mean
   "this belongs in Design, not Requirements":

   - A library / framework name, or any third-party tool the
     user doesn't invoke directly.
   - A file path / config key / data shape the user doesn't
     see in the UI or surface they interact with.
   - A function / class / trait / type / schema name from
     the implementation.
   - An internal subcommand, hook event name, or protocol
     verb that's not part of the user-facing surface.

A closing one-liner names the discipline as guidance, not
rigid form: the spec layout adapts to the feature; the
strictness lives in the §6 / §7 / §8 gates.

### Composition with existing kdevkit shape

- **§3 backlog promotion** — when a backlog item's "What"
  describes user-observable behavior already, the spec can
  inherit it; when it mixes what/how (most cases), the
  promotion path applies the smell test as part of the
  Requirements fill.
- **§6 four interviews** — order unchanged (Requirements →
  Test Strategy → Design → Implementation Plan). The Test
  Strategy interview's output gets written in two places:
  Functional/Integration cases land between Requirements and
  Design; Unit Tests land under Design. The agent does the
  splitting at write-time, not interview-time.
- **§8 closure** — no change. Closure reconciles in-flight
  markers and runs §8.3 backlog cleanup; this feature's own
  closure removes the `kdevkit-separate-what-from-how`
  backlog item (already `git mv`'d into `feature/` at branch
  start, so it lives at `specs/feature/`, not `backlog/`,
  and §8.3 surfaces "none" or whichever items did close).

### Frontmatter / version

- SKILL.md `version` bumps from 3.1.0 to 3.2.0 — minor
  signpost. The four interviews and spec template are part
  of the kdevkit *contract*, and this is a contract change
  (not a no-behavior compaction). 3.2.0 is honest.
- Frontmatter `description` updated only if it mentions
  "four interviews" or "feature spec" framing in a way that
  the new prompt invalidates; otherwise unchanged.

### Public-repo hygiene

The smell test paragraph and the interview re-prompt examples
(in `interviews.md`) must use generic / hobbyist-flavoured
illustrations — no internal product, team, ticket, CR, or
repo names. The §9 internal-marker grep applies to the diff
on push.

## Test Strategy

### Functional / Integration

The contract change is "what the agent writes when running
the four interviews on a fresh feature." The functional
signal is a judge-mode smoke fixture that prompts an agent
to start a fresh feature and asks how it'd phrase the
Requirements interview prompt + how Requirements pairs with
the project's tests.

**New fixture: `kdevkit-requirements-user-facing.smoke`** —
judge mode (`prompt:` + `expected_narrative:`). The prompt
asks: _"I want to start a new feature called `task-snooze`.
What do you ask in the Requirements interview, and how do
you write the spec when I answer? Cover the framing of the
Requirements prompt, how Requirements pairs with the
project's tests, and where library / file-path / function
names belong."_

The `expected_narrative` covers four behaviors:

1. **Three-layer framing.** Capability (Feature Brief) /
   experience (Requirements) / design (Design). The agent
   names all three and assigns each to its section.
2. **Requirements interview framing — experience layer.**
   The agent asks how the user *experiences* the
   capability — what they touch, what they observe. At
   least one feature-type example (CLI / app / skill change
   / service) shows the agent isn't defaulting to
   CLI-thinking. Library names, internal file paths,
   function/schema names, and protocol verbs explicitly go
   in Design.
3. **Smell test before each Requirements bullet.** Library
   names, internal file paths, function/class/schema names,
   internal protocol verbs / hook events all move to
   Design.
4. **V-model pairing of Requirements and Design with the
   project's tests.** Functional / integration tests verify
   Requirements (in user-observable terms); unit tests
   verify Design (primitives). Framed as guidance / default,
   not rigid form — spec layout adapts to the feature; the
   strictness lives in the §6 / §7 / §8 gates.

Wrong answers: producing a spec where Requirements names
libraries / file paths / function names; collapsing
capability and experience into one section; defaulting to
CLI examples without acknowledging app / skill-change /
service shapes; describing the spec layout as a strict
heading order rather than guidance with V-model pairing as
the default; placing functional tests after Design where
they end up phrased against internal record-shape
assertions; skipping the smell test entirely.

### Existing functional smokes — regression net

Existing fixtures stay green:
`kdevkit.smoke`, `kdevkit-feature-loop.smoke`,
`kdevkit-feature-planning.smoke`,
`kdevkit-feature-closure.smoke`, `kdevkit-dev-loop.smoke`,
`kdevkit-review-gate.smoke`,
`kdevkit-review-config-setup.smoke`,
`kdevkit-initiative-recognition.smoke`,
`kdevkit-stream-closure.smoke`,
`kdevkit-cross-stream-rebase.smoke`,
`kdevkit-closure-after-long-session.smoke`. Audit each
`expected_narrative` for assertions tied to the OLD section
order (flat `## Test Strategy` between Requirements and
Design). Patch any that mention the section-name shape; none
should mention it (existing fixtures focus on the four
interviews + plan-commit + dev-loop + closure mechanics, not
template structure).

### Unit tests

`deno task test:unit` (default §7 Test Gate). 22 unit tests
must remain green. The deploy logic and schema validator
are unchanged — only SKILL.md and `interviews.md` prose
edits land. No new unit test required (the new fixture is
the contract signal, not a unit boundary).

### Smoke tests

`deno task test:smoke` after `deno task deploy` to confirm
the kdevkit symlinks still resolve. Cheap; runs as part of
the §7 Test Gate's regression sweep.

### Functional tests are user-driven (per project.md)

Per `project.md`'s "Functional tests are user-driven" rule,
the agent does not run `deno task test:functional`
autonomously. The agent prepares the fixture, names the
exact command (`./tests/functional/run
kdevkit-requirements-user-facing` for a single run, or
`deno task test:functional` for the full surface), and hands
off. The user runs it.

This feature does **not** carry a per-feature override (D
did, because D's whole point was contract preservation and
the agent needed in-loop signal). Here the change is
small-surface and the user-driven cadence is fine — the
agent's smell test won't ossify into the wrong shape between
the test-prep step and the user-run step.

### Quality gate

`deno task fmt && deno task lint && deno task check` after
the SKILL.md / `interviews.md` edit slice. Both files are
markdown — `fmt` may rewrap; `lint` and `check` are no-ops
for `.md`.

## Design

### Diff shape

- **`sources/skills/kdevkit/SKILL.md`** — adds the §6 smell
  test paragraph + four bullets immediately after the "Four
  short interviews" subsection's existing prose. Bumps
  `version` from `3.1.0` to `3.2.0`. Roughly +20 lines net.
- **`sources/skills/kdevkit/interviews.md`** — rewrites the
  Requirements interview prompt; rewrites the feature file
  template body to encode the new section ordering. Roughly
  +20 lines net (the new ordering adds H3 sub-headings under
  Requirements and Test Strategy and Design).
- **`tests/functional/skills/kdevkit-requirements-user-facing.smoke`** —
  new judge-mode fixture, ~25 lines (prompt + expected_narrative).

No other files. No `setup.md` change (template-shape change
fires on feature genesis only — the `interviews.md` home).
No `project.md` edits (the rule is project-agnostic; lives
in the skill).

### Why the smell test is in SKILL.md, not `interviews.md`

`interviews.md` is inline-Read only on fresh-feature start.
The smell test fires every time the agent writes a
Requirements section — including during continue / pick-up
flows where `interviews.md` may not be in context. SKILL.md
§6 is always-on; placing the smell test there makes it
unconditionally available. The four bullets are short enough
that the always-on context cost is negligible. This honors
the SKILL.md "Future-feature placement rule" (operational,
fires every session → SKILL.md).

### Why the four interviews stay (just sharper framing)

The four-interview shape (Requirements → Test Strategy →
Design → Implementation Plan) is the load-bearing contract
that downstream §6 / §7 / §8 mechanics rely on. Changing the
interview *count* or *order* would ripple into the
plan-commit rule, the dev-loop test gate, the closure
reconcile step. Sharpening the prompt and restructuring the
*output* (where the spec writes Test Strategy material) keeps
all that machinery intact. The agent does the
"functional/integration before Design, unit tests after
Design" splitting at write-time, not interview-time.

### Why split Requirements into Launch + Runtime is optional

Some features are all-launch (a one-shot install command),
some are all-runtime (a watcher, a dashboard), some are both
(setup + ongoing behavior, like the
agent-session-orchestrator). Forcing the split on every spec
adds noise to the simple cases. Recommending it when the
feature has both modes captures the load-bearing benefit
(separating "what user types once" from "what user sees over
time") without ossifying.

### Why functional tests sit BEFORE Design in the template

The template's section order shapes the writing order. When
Functional / Integration is between Requirements and Design,
the agent writing "what cases test that the user observes
the right thing?" only has the user-observable surface in
context. When Functional / Integration is after Design, the
internal field names are right there in the agent's
context — the path of least resistance is to assert on them.
The template ordering removes that path.

### Trade-offs considered

- **Inline the smell test in `interviews.md` vs. SKILL.md.**
  SKILL.md chosen. Trade-off: slightly more always-on
  context (one paragraph + four bullets ≈ ~120 tokens). Win:
  the discipline fires unconditionally. Inline-Read of
  `interviews.md` is conditional on fresh-feature entry; the
  smell test must fire on continue / pick-up too.
- **Hard constraints / Prior art / Runtime prerequisites as
  required vs. optional.** Optional chosen. The backlog
  proposal listed them in the template; making them
  *required* would force every feature to fill them even
  when irrelevant (most features have no hard constraints
  beyond `project.md`'s; many have no prior art). Optional
  preserves the recommendation without ossifying.
- **Version bump 3.1.0 → 3.2.0 vs. stay at 3.1.0.** Bump
  chosen. The four interviews and spec template are part of
  kdevkit's contract; this changes both. 3.1.0 was a no-
  behavior-change compaction; 3.2.0 is a behavior change.
  Honest.
- **One judge fixture vs. two.** One chosen. The fixture
  covers re-prompt + smell test + section-order +
  user-observable test phrasing in a single
  `expected_narrative`. Two fixtures (one for re-prompt,
  one for template structure) would over-isolate; the
  failure mode "agent writes a spec with library names in
  Requirements" exposes both at once. If a real failure
  exposes a granularity gap, add a narrower fixture then.
- **Migrate existing specs vs. leave as-is.** Leave as-is
  (per backlog out-of-scope). Specs are time-bound to their
  feature; rewriting old specs to the new layout is churn
  without payoff. Future rewrites adopt the new layout
  organically.

## Implementation Plan

One slice. Mechanical edits to two skill files plus one new
fixture.

1. **Edit `interviews.md`'s Requirements interview prompt.**
   Replace `Problem? Who interacts? Acceptance criterion?`
   with the user-observable framing prompt (per Requirements
   §1 above). Preserve the surrounding numbered-list shape
   (1. Requirements / 2. Test Strategy / 3. Design / 4.
   Implementation Plan).
2. **Edit `interviews.md`'s feature file template body.**
   Restructure the template's headings to encode the new
   ordering: `## Requirements` (with optional `### Launch
   experience` / `### Runtime experience` subheadings) →
   `## Test Strategy` (with `### Functional / Integration`
   subheading) → `## Design` (with `### Unit Tests`
   subheading) → `## Implementation Plan`. The Design
   section's template comment nudges the agent to lead with
   rationale ("why this shape — what was considered and
   rejected — comes first; the rest is the design itself").
   Preserve the `Part of initiative:` HTML comment,
   `## Session Log`, and `## Decision Log`.
3. **Edit SKILL.md §6 "Four short interviews" subsection.**
   Add the smell-test paragraph + four bullets immediately
   after the existing "Tests sit immediately after
   requirements..." prose. Wording per Requirements §3
   above.
4. **Bump SKILL.md frontmatter `version`** from `3.1.0` to
   `3.2.0`. Frontmatter `description` left unchanged unless
   review surfaces an inaccuracy.
5. **Run Quality Gate.** `deno task fmt && deno task lint &&
   deno task check`. Markdown changes only — fmt may
   reformat; lint/check should be no-ops.
6. **Run Test Gate.** `deno task test:unit` (22 tests
   green). `deno task test:smoke` after `deno task deploy`
   confirms symlinks resolve.
7. **Add fixture
   `tests/functional/skills/kdevkit-requirements-user-facing.smoke`.**
   `prompt:` + `expected_narrative:` shape per Test Strategy
   §1 above. The prompt uses the public-repo-safe example
   `task-snooze`.
8. **Audit existing kdevkit functional smokes.** Re-read
   each fixture's `expected_narrative`; flag any that
   reference the OLD flat-Test-Strategy section ordering.
   Expected: zero — existing fixtures focus on mechanics, not
   template shape. If any do reference it, patch the wording
   inline.
9. **Hand off the functional smokes to the user.** Per
   `project.md`'s user-driven rule, the agent names the
   commands (`./tests/functional/run
   kdevkit-requirements-user-facing` for the new fixture;
   `deno task test:functional` for the full surface) and
   waits for the user's report. No agent-side run.
10. **Run Code Review Gate.** Per
    `code_review.reviewer: host-native`, threshold 70,
    hard-stop, retry-budget 2. Reviewer sees `project.md`
    plus the diff (SKILL.md + `interviews.md` + new
    fixture); no feature spec.
11. **Push.** Open Agent-dev Review Gate per §7 / §9.
    Body: Approach (the three changes) + Reading order
    (SKILL.md §6 first as intent; `interviews.md` body as
    contract; the new fixture as plumbing).
12. **Closure.** §8.1 reconcile (this spec's Implementation
    Plan items checked off in-place); §8.2 soft project.md
    verify (likely no change — project.md doesn't touch
    feature template structure); §8.3 backlog cleanup —
    `kdevkit-separate-what-from-how.md` was promoted from
    backlog to feature at branch start, so it doesn't appear
    in the closure-time backlog list; expected answer
    "none". §8.3.5 N/A (this feature isn't part of an
    initiative). §8.4 commit + push closure edits if any.
    §8.5 Closure Review Gate (title rewritten to
    `feat(kdevkit): separate what from how in feature
    interviews` or similar). §8.6 squash-merge.

Risk notes:

- **Wording drift on the smell test.** The four bullets
  must enumerate categories ("library name", "internal
  file path", "function/trait name", "internal protocol
  verb"), not specific examples — the latter would date
  fast. Reviewer should flag if any bullet feels narrow.
- **Spec-template ossification.** Making Hard constraints /
  Prior art / Runtime prerequisites *commented-out
  placeholders* (rather than required H2 headings) keeps
  them opt-in. If reviewer or downstream sessions complain
  the placeholders are too prescriptive, they can be
  dropped from the template entirely with no behavior
  loss.
- **Existing fixtures regress.** Audit step 8 expects zero
  hits. If a fixture *does* assert the old flat-Test-
  Strategy ordering, patching it is a one-line edit; not
  expected to be load-bearing.
- **The new fixture's judge unreliability.** Judge-mode
  fixtures evaluate via a second LLM call; flaky judges
  can fail valid agent answers. The `expected_narrative`
  is intentionally broad enough to admit phrasing variance
  ("the smell test or equivalent") while still failing on
  the load-bearing wrong answers. If the fixture flakes,
  tighten the narrative, don't loosen it.
- **Backwards compatibility with old specs.** Out of scope
  per backlog. Existing specs follow the flat Test Strategy
  shape and remain valid; the new template is forward-only.
  No migration script, no validator that fails on the old
  shape.

## Session Log

<!-- append: date · what was done · decisions made -->

- 2026-06-10 · backlog → feature promotion · §6 four
  interviews completed autonomously per backlog's
  fully-fleshed What/Why/Acceptance criteria: Requirements
  (interview re-prompt framing, template restructure with
  tests split around Design, smell test in always-on
  SKILL.md, optional template additions, version bump
  3.1→3.2, public-repo hygiene); Test Strategy (one new
  judge fixture covering re-prompt + smell test + section
  order + user-observable test phrasing; existing 11
  fixtures audited, expected zero regressions; functional
  smokes user-driven per project.md, no D-style override);
  Design (smell test in SKILL.md not interviews.md to fire
  unconditionally; four interviews stay; functional tests
  before Design in template; Launch/Runtime split is
  optional; Hard constraints / Prior art / Runtime prereqs
  as commented-out placeholders); Implementation Plan (12
  ordered steps).

- 2026-06-10 · v2 reframe (V-model + scaffolding-not-rigid)
  · code review surfaced four findings: (1) `## Design`
  has no body slot before `### Unit Tests`, (2) HTML-comment
  guidance lost the typed-slot affordance, (3) `### Unit
  Tests` under Design contradicts every existing spec in the
  corpus, (4) the introducing spec didn't follow its own
  template — putting unit tests under Test Strategy, like
  the existing 6 specs. User reframed: the discipline IS
  V-model (Requirements↔functional, Design↔unit) but
  `interviews.md` is *guidance, not rigid form* — strictness
  lives in the §6 / §7 / §8 gates. Course correction: drop
  the prescribed `### Functional / Integration` and
  `### Unit Tests` H3s from the template; restore visible
  `<...>` body placeholders alongside HTML guidance comments;
  recast SKILL.md §6 smell-test prose so the V-model pairing
  is the *why* behind the four-bullet test, not just heading
  prescription; reword the new fixture's `expected_narrative`
  to assert V-model framing + scaffolding-not-rigid stance,
  not strict heading order.

## Decision Log

<!-- append: decision · rationale · alternatives rejected -->

- **Smell test lands in SKILL.md §6, not `interviews.md`.**
  Rationale: the discipline fires every time the agent
  writes a Requirements section, including continue /
  pick-up flows where `interviews.md` isn't inline-Read.
  SKILL.md is always-on; one paragraph + four bullets is
  cheap context. Honors the future-feature placement rule
  (operational → SKILL.md). Alternative rejected:
  `interviews.md`-only — fails to fire on non-fresh-start
  paths.
- **Four interviews stay (Requirements → Test Strategy →
  Design → Implementation Plan); only the prompt and the
  spec-write structure change.** Rationale: the
  interview-count contract feeds §6 plan-commit, §7 test
  gate, §8 closure. Restructuring the *output* preserves
  all downstream mechanics. Alternative rejected: split
  Test Strategy into two interviews (functional / unit) —
  ripples into plan-commit + closure; not justified by the
  marginal authoring clarity gain.
- **Tests split around Design at write-time, not
  interview-time.** Rationale: the agent collects the full
  test-strategy answer in one interview pass, then writes
  functional/integration cases between Requirements and
  Design and unit tests under Design. Splitting at
  interview-time would mean asking the user about tests
  twice. Alternative rejected: two interview phases for
  tests — over-prescriptive for a small clarification.
- **Launch / Runtime split is optional.** Rationale: many
  features are all-launch or all-runtime; forcing the
  split adds noise. Recommending it when the feature has
  both modes captures the value without ossifying.
  Alternative rejected: required Launch + Runtime headings
  on every spec — adds noise to simple features.
- **Hard constraints / Prior art / Runtime prerequisites
  as commented-out placeholders.** Rationale: the backlog
  proposed them as additions; making them required would
  force every spec to fill irrelevant sections. Optional
  placeholders nudge the agent without ossifying. If
  reviewer flags them as too prescriptive, drop entirely —
  no behavior loss. Alternative rejected: required H2
  headings — most features don't need them; adds noise.
- **One judge fixture, not two.** Rationale: re-prompt +
  smell test + template ordering + user-observable test
  phrasing are tightly coupled — the failure mode "spec
  has library names in Requirements" exposes all four at
  once. Over-isolation is what you do *after* a real
  failure exposes a gap. Alternative rejected: one
  fixture per behavior — premature granularity; harder to
  maintain.
- **Version bump 3.1.0 → 3.2.0 (signpost minor).**
  Rationale: this is a contract change — the four
  interviews' output structure changes. 3.1.0 was a
  no-behavior compaction; 3.2.0 is honest about the
  visible-to-spec-authors change. Alternative rejected:
  stay at 3.1.0 — would conflate compaction with this
  feature's contract change.
- **Functional smokes are user-driven (no D-style
  override).** Rationale: the change is small-surface,
  one new fixture; the user-driven cadence is fine. The
  agent's smell test won't ossify between test-prep and
  user-run. D needed in-loop functional runs because D
  was specifically about contract preservation under
  compaction; this feature is a forward contract change
  with low judge-flake risk. Alternative rejected:
  per-feature override running `test:functional` in §7 —
  unnecessary cost; not load-bearing.
- **Existing specs not migrated (per backlog
  out-of-scope).** Rationale: existing specs follow the
  flat Test Strategy shape and remain valid; rewriting
  them is churn without payoff. The new template is
  forward-only. Alternative rejected: bulk-rewrite all
  completed feature specs — invalidates git history
  context; no behavior gain.
- **`interviews.md` is guidance, not rigid form.**
  Rationale: per user direction after code review surfaced
  the contradiction between prescribing `### Unit Tests`
  under Design and the established repo convention
  (every existing spec keeps unit tests inside `## Test
  Strategy`). The skill's enforcement primitive is the
  gate trio (§6 Planning Review → §7 Agent-dev Review
  looping with Code Review → §8 Closure Review), not
  heading order. The four interviews and template
  scaffold the right questions; situation decides the
  spec layout. Alternative rejected: prescribe rigid H3
  ordering (`### Functional / Integration` before Design;
  `### Unit Tests` under Design) — would force every
  spec into a shape that doesn't fit specs without
  design-primitive unit tests, and contradict the corpus.
- **V-model pairing is the *why* behind the smell test.**
  Rationale: the original draft framed the smell test as
  prescriptive ("internals belong in Design"). After the
  review, the framing recasts: a Requirements bullet that
  names internals can't be verified by a functional test
  in user-observable terms, so it's in the wrong section.
  The pairing (Requirements↔functional, Design↔unit) is
  the justification; the four-bullet test is the
  shorthand. This makes the discipline derivable, not
  memorised. Alternative rejected: keep the smell test
  free-standing — works, but the agent loses the link to
  *why* and applies it brittly.
- **Restore visible body placeholders alongside HTML
  guidance comments.** Rationale: the v1 template
  collapsed all body affordances into HTML comments,
  which vanish from rendered markdown. An agent reading
  the template literally would see `## Requirements` →
  invisible comment → `## Test Strategy` and have no
  cursor for where to write. Putting `<bullet list — what
  the user observes>` under each section's HTML comment
  restores the typed-slot affordance without losing the
  guidance. Alternative rejected: HTML comments only —
  loses the visible cursor; brittle.
