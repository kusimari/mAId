# Feature: kdevkit-retrospection-adopts

## Git Setup

- Branch: `feat/kdevkit-retrospection-adopts`
- Base: `main` at `e28fcff` (post v3.2 V-model + scaffolding-not-rigid)

## Feature Brief

After surveying the popular AI-coding-skill ecosystem
(Karpathy / vibe coding, GitHub spec-kit, BMAD-METHOD, Cline
Plan/Act + Memory Bank, Aider Architect, Agent OS, the
.cursorrules corpus, Anthropic's official skills repo), four
patterns recur across multiple frameworks that kdevkit
either lacks or under-specifies. This feature adopts those
four patterns into kdevkit so the workflow stays current
with the field on the parts that compose, while preserving
the parts that are deliberate kdevkit-specific divergences
(host-agnostic over tool-locked Plan/Act mode-switching;
single-file feature spec over multi-artefact tree;
slice-level review over per-edit auto-test).

The four adoptions:

1. **Codebase grounding before the four interviews.** A
   one-liner in §6 that names "read project.md, related
   specs, and the corners of the codebase the feature
   touches before opening the first interview" — the
   discovery step every popular framework has explicit
   (spec-kit `research.md`, Cline read-only Plan mode,
   Agent OS Discover Standards), formalised here as a
   single cue rather than a separate artefact.
2. **Harder project.md verify at closure.** Today §8.2 is
   "soft" — offer to update, decline is fine. With four
   features merged since the last project.md edit, the
   project.md drifts. Promote §8.2 from "offer" to "ask
   one targeted question per Mission/Architecture/Tech
   Stack/Layout/Testing/Deployment dimension that the
   feature touched"; decline is still fine, but asking is
   mandatory.
3. **`- [ ]` checkbox syntax in Implementation Plan.**
   Markdown task-list shape (`- [ ] step` ticked to
   `- [x]`) replaces the prose-numbered list in the
   feature template. Closure-time §8.1 reconcile becomes
   a literal grep for `- [ ]`. Cheap, additive, lifts
   readability for humans glancing at the spec.
4. **Spec-discipline anti-patterns subsection in §9.**
   Four bullets covering the kdevkit-specific failure
   modes the .cursorrules corpus catches generically: no
   scope creep mid-dev (Plan items are the contract; new
   work goes to Plan or backlog); no unrelated refactor
   bundling (one feature = one focused diff); no
   premature closure (the cue is a hard gate); no silent
   plan amendments (changes to Plan items get a
   one-liner in Decision Log).

Out of scope: per-edit auto-commit/auto-test (Aider's
shape; wrong granularity for spec-driven slice review);
multi-file spec trees (spec-kit's shape; collapses
review surface for no readability win); hard tool-level
plan/act mode (Cline's shape; not portable across hosts);
auto-extracted standards (Agent OS's shape; project.md
is hand-curated by design). The skill stays single-file-
spec, host-agnostic, and human-gated.

## Requirements

### Codebase grounding cue (Pattern 2 from retrospection)

A one-line cue lands in SKILL.md §6's "Four short
interviews" subsection, immediately before the existing
"When entering a feature with no spec on disk..." prose:

> _"Before the first interview, ground in current state:
> read `project.md`, scan related feature specs in
> `specs/feature/`, and survey the corners of the
> codebase the feature touches. The interview is
> calibrated to what's there now, not the user's
> recollection."_

The cue fires on fresh-feature start (the four
interviews context). It does NOT fire on continue /
pick-up — the spec is already grounded in the prior
session's read. The cue does NOT introduce a new
artefact (no `research.md`); findings worth keeping
land in the Session Log per existing convention.

The phrasing must explicitly cover three sources:
project.md, related feature specs, the codebase. Naming
all three pre-empts the failure mode where the agent
reads project.md only and skips the codebase scan.

### Harder project.md verify at closure (Pattern 3)

SKILL.md §8.2 currently reads "Soft `project.md`
verify. Offer to update `project.md` with what changed.
Decline is fine; not a hard block. Stage accepted
edits." Promote to a structured ask:

> _"For each `project.md` section the feature touched
> — Mission, Architecture, Tech Stack, Layout, Testing,
> Deployment, Hard constraints, Agent Development —
> ask one targeted question: 'Did this feature change
> what's documented under <section>?'. Stage accepted
> edits. Decline is fine; asking is mandatory."_

The agent decides which sections the feature *touched*
based on the diff:

- Tech Stack — touched if a dependency was added/removed
  or a runtime version moved.
- Layout — touched if a top-level directory or major
  file was added/removed (per the project.md tree).
- Testing — touched if a `deno task test:*` was added,
  removed, or its semantics changed.
- Deployment — touched if `maid/registry.ts` or the
  install path changed.
- Architecture — touched if a moving part (Sources /
  Registry / CLI) gained or lost a responsibility.
- Mission — rarely touched; ask only if the change
  meaningfully shifts what the project is for.
- Hard constraints — touched if a new invariant was
  introduced or an old one weakened.
- Agent Development — touched if a `kdevkit` block key
  changed or a new skill-scoped preference landed.

For each touched section, the agent surfaces a brief
question ("This feature added a `test:functional --stressed`
mode. Update Testing's description?") and stages the edit
on confirmation. Untouched sections aren't asked about.

The strictness lift is mandatory-asking, not
mandatory-editing. The user can answer "no, project.md
is fine" for every section; the closure proceeds. The
ask is the artifact (same shape as §8.3 backlog
cleanup, where asking is mandatory even when "none").

### Implementation Plan checkbox syntax (Pattern 5)

Update `interviews.md`'s feature file template so the
`## Implementation Plan` body placeholder uses Markdown
task-list syntax:

```
## Implementation Plan

<ordered checklist; one slice per item>

- [ ] <slice 1>
- [ ] <slice 2>
- [ ] <slice 3>
```

Convention: the agent ticks `- [ ]` to `- [x]` in the
same commit that completes the slice. Mid-slice work
stays unchecked. Closure-time §8.1 reconcile becomes a
literal grep for unchecked boxes:

> _"Sweep the spec for `- [ ]` markers in
> Implementation Plan. Resolve in place (tick to
> `- [x]` if quietly done) or move out (backlog or
> follow-up feature)."_

The cue replaces the existing §8.1 prose
("Sweep ... for unchecked Implementation Plan items,
open Decision Log entries, unresolved questions") with
a more mechanical version that names the marker
explicitly. Decision Log + unresolved questions
sweeps stay verbatim — not all in-flight markers are
checkboxes.

Existing specs (the corpus we audited at v3.2 closure)
do NOT get migrated. The new template applies
forward-only. The §8.1 reconcile cue tolerates both
shapes — `- [ ]` markers AND prose-numbered lists —
because old specs are still valid.

### Spec-discipline anti-patterns subsection (Pattern 7)

A short subsection lands in SKILL.md §9
(cross-cutting rules), positioned after "Commit
hygiene" and before "Skill-file placement". Title:
**Spec-discipline anti-patterns**. Four bullets:

1. **No scope creep mid-dev.** The Implementation
   Plan items in the feature spec are the contract.
   If new work surfaces during dev, either add it to
   the Plan and confirm with the user, or move it to
   `specs/backlog/`. Don't silently expand the diff.
2. **No unrelated refactor bundling.** One feature =
   one focused diff. Drive-by cleanups in unrelated
   files belong in their own feature or a
   `chore(<scope>):` follow-up — not bundled into the
   feature's commits.
3. **No premature closure.** The closure cue
   ("close it" / "ship it" / "merge it" / "feature
   done") is a hard gate. Quality + Test + Code
   Review passing is necessary but not sufficient;
   the explicit cue is required even when everything
   looks done.
4. **No silent plan amendments.** Changes to the
   Implementation Plan after the Planning Review Gate
   open warrant a one-liner in the Decision Log
   (rationale + what shifted). Reviewers reading the
   PR/CR see the original plan in the Planning Review
   commit; the closing diff should be reconcilable.

The subsection's framing line: _"These are the
spec-discipline failure modes that survive the
Quality / Test / Code Review gates because the gates
check the diff, not the diff's relationship to the
plan. Fire proactively during dev, not reactively at
review."_

The discipline is operational (fires every dev
slice), so SKILL.md is the right home — not
`interviews.md` or `setup.md`.

### Frontmatter / version

- SKILL.md `version` bumps from `3.2.0` to `3.3.0` —
  contract additions to the §6 interview cue, §8.2
  closure verify, §9 cross-cutting rules. Minor
  bump; no breaking change.
- Frontmatter `description` left unchanged unless
  review surfaces an inaccuracy.

### Public-repo hygiene

All cue text and anti-pattern bullets must use
generic / hobbyist-flavoured illustrations — no
internal product, team, ticket, or repo names. The
§9 internal-marker grep applies to the diff on push.

### Composition with existing kdevkit shape

- **§3 / §4 / §5 / §7 / §10**: untouched. The
  retrospection adoptions land in §6 (cue), §8 (verify),
  §9 (anti-patterns), and `interviews.md` (template
  checkbox).
- **§6 four interviews**: order unchanged. The
  grounding cue fires before interview 1, not as a
  fifth interview.
- **§8.1 reconcile**: cue rewritten to be more
  mechanical (grep `- [ ]`); the rest of the §8 cycle
  stays verbatim except for §8.2.
- **§8.2 verify**: lifted from "offer" to "ask per
  touched section."
- **§9 cross-cutting**: gains the anti-patterns
  subsection.
- **`interviews.md` Implementation Plan template**:
  switches to checkbox shape.

## Test Strategy

V-model pairing: Requirements verified by
functional/integration; Design verified by unit tests.

### Functional / Integration

The contract changes are observable through the
agent's behavior in fresh-feature, closure, and
mid-dev situations. Two judge-mode fixtures cover
the four adoptions:

1. **`kdevkit-codebase-grounding.smoke`** —
   covers Pattern 2. Prompt: _"I want to start a new
   feature called `metric-export` in this kdevkit-aware
   project. Walk me through what you do before opening
   the Requirements interview."_
   `expected_narrative` covers: (a) reads project.md
   first; (b) scans related feature specs in
   `specs/feature/`; (c) surveys the corners of the
   codebase the feature touches (e.g. greps for the
   relevant subsystem, reads the touched files); (d)
   uses what's grounded in current state to calibrate
   the four interviews — does NOT skip directly to
   asking the user about requirements. Wrong answers:
   asking the four interviews without any code/spec
   read; reading project.md only and skipping the
   codebase scan; creating a separate `research.md`
   artefact (kdevkit doesn't introduce one).

2. **`kdevkit-closure-verify-and-anti-patterns.smoke`** —
   covers Patterns 3 + 7 in one fixture. Prompt:
   _"Closing a feature `add-prom-export` that added a
   new `deno task test:metrics` test layer and a new
   directory `maid/metrics/`. Walk me through closure.
   Then: during the dev loop on this feature, I asked
   you to also fix an unrelated lint warning in
   `maid/deploy.ts` — what would you have said?"_
   `expected_narrative` covers:
   (a) §8.1 reconcile sweeps `- [ ]` markers in
       Implementation Plan;
   (b) §8.2 verify asks per-touched-section —
       at minimum Testing (new test layer) and Layout
       (new directory) are surfaced; Tech Stack /
       Architecture / Mission / Deployment / Hard
       constraints / Agent Development are surveyed
       and skipped if untouched, not asked-about
       blindly;
   (c) §8.3 backlog cleanup ask;
   (d) The unrelated-refactor question gets the
       anti-pattern response: "no unrelated refactor
       bundling — file as a follow-up `chore` or own
       feature, not in this diff";
   (e) Either covers anti-pattern 1 (scope creep) or
       anti-pattern 3 (premature closure) when
       describing how the agent would have handled
       a "while you're at it, also add..." mid-dev
       request.
   Wrong answers: skipping the per-section verify ask;
   reading "decline is fine" as "skip the question"
   (mandatory ask is the lift); accepting the unrelated
   refactor; treating Quality+Test+CR-pass as sufficient
   for closure without the cue.

The checkbox-syntax change (Pattern 5) doesn't get
its own fixture — it's a template-shape change
visible in the spec the agent writes. The grounding
fixture and the closure fixture both prompt
spec-writing; if the agent emits Implementation Plan
items as `- [ ]` checkboxes, the change is exercised
implicitly. If it emits prose-numbered lists, the
fixture's `expected_narrative` doesn't fail (old
shape stays valid forward-only); but the spec
template will guide future fresh-feature starts to
the new shape.

### Existing functional smokes — regression net

All 12 existing kdevkit smokes stay green:
`kdevkit.smoke`, `kdevkit-feature-loop.smoke`,
`kdevkit-feature-planning.smoke`,
`kdevkit-feature-closure.smoke`,
`kdevkit-dev-loop.smoke`, `kdevkit-review-gate.smoke`,
`kdevkit-review-config-setup.smoke`,
`kdevkit-initiative-recognition.smoke`,
`kdevkit-stream-closure.smoke`,
`kdevkit-cross-stream-rebase.smoke`,
`kdevkit-closure-after-long-session.smoke`,
`kdevkit-requirements-user-facing.smoke`. Audit each
`expected_narrative` for assertions that conflict
with the new behavior:

- `kdevkit-feature-closure.smoke` and
  `kdevkit-closure-after-long-session.smoke` describe
  §8.2 as "soft / offer." If they hard-assert
  "decline is fine, no further question," patch to
  align with the per-section ask. If they describe
  the soft behavior in passing without asserting the
  shape, leave alone.
- All others should be unaffected — they cover
  entry, planning, dev loop, review gate, initiative
  mechanics, requirements framing.

### Unit tests

`deno task test:unit` (default §7 Test Gate). 23
unit tests must remain green. The deploy logic and
schema validator are unchanged — only SKILL.md and
`interviews.md` prose edits land plus two new
`.smoke` fixtures (no frontmatter; the v3.1
schema test pinned this).

### Smoke tests

`deno task test:smoke` after `deno task deploy
--force` (worktree redirect) confirms the kdevkit
symlinks resolve. Cheap; runs as part of the §7
Test Gate's regression sweep.

### Functional tests at closure

Per `project.md`'s "Functional tests are
user-driven" rule, the agent does not run
`deno task test:functional` autonomously by
default. **Per-feature override applies**: the
agent runs the two new fixtures + the two existing
fixtures most likely to regress
(`kdevkit-feature-closure`,
`kdevkit-closure-after-long-session`) at closure
time after deploying the worktree symlinks, then
restores the symlinks to the primary checkout.
Same shape as v3.2's closure: lift the rule for
this feature because the contract change is the
agent's behavior, and only the smokes verify it.

The override extends the §7 Test Gate's
`retry_budget`: the four-fixture run counts as one
Test Gate invocation per cycle; if any fail, the
cycle fix-and-retries up to default budget of 2
cycles before stopping and reporting per §7.

### Quality gate

`deno task fmt && deno task lint && deno task
check` after the SKILL.md / `interviews.md` edit
slice. Both files are markdown — `fmt` may rewrap;
`lint` and `check` are no-ops for `.md`.

## Design

### Why these four, not the eight

The retrospection surfaced eight recurring patterns
across the popular tools. Four were adopted; four
were not, and the *not* is as load-bearing as the
*yes* for understanding kdevkit's identity:

- **Multi-file spec trees (Pattern 1) — not
  adopted.** spec-kit's split into spec.md + plan.md +
  tasks.md + research.md + data-model.md + contracts/
  is an artifact of its command-pipeline shape (each
  slash command writes its own file). kdevkit's
  review surface is the PR/CR diff; one coherent
  spec under review beats six artefacts to
  cross-reference. The compaction (D / v3.1) showed
  that splitting the *skill* into SKILL.md + setup.md
  + interviews.md helps when the load is asymmetric
  (interviews.md only fires on fresh start). Feature
  specs aren't asymmetric — every section is read at
  every review.
- **Hard plan/act mode-switching (Pattern 4) — not
  adopted.** Cline enforces it via tool capabilities
  (Plan mode is read-only by host gating). kdevkit
  is host-agnostic; tool-level capability gating
  isn't portable. The Plan-commit rule (commit + push
  + Planning Review Gate before any dev) is the
  kdevkit equivalent: makes the plan a reviewable
  artefact before code, which is the *goal* of mode-
  switching. Hard-mode is Cline's local optimum, not
  a universal pattern. **Per user direction, the
  solve is to write the skill so agents follow it
  by reading the prose**, not to chase tool-level
  capability gating.
- **Auto-commit-per-edit + auto-test-per-edit
  (Pattern 6) — not adopted.** Aider's
  per-edit-tested loop is the right shape for solo-
  CLI pair programming where each edit is an atomic
  intent. kdevkit's slice = unit of *intent*, not
  unit of *edit* — slice-level testing is the
  cognitive granularity the human reviews against.
  Per-edit auto-commit clutters the squash-merge
  history (single commit on main is a kdevkit
  invariant).
- **Auto-extracted standards (Agent OS) — not
  adopted.** kdevkit's project.md is hand-curated.
  Auto-extraction has its own failure modes (extracts
  dead code; captures patterns the project is moving
  away from). The §2 structural verify is the
  current safeguard against drift; Pattern 3
  (closure-time per-section ask) is the new
  proactive safeguard.

The four adoptions are the patterns that compose
with kdevkit's existing shape (single-file spec,
host-agnostic, slice-review, human-gated phases)
without breaking it.

### Diff shape

- **`sources/skills/kdevkit/SKILL.md`** —
  adds (a) the §6 codebase-grounding cue (~5
  lines), (b) the §8.2 per-section verify lift
  (~15 lines, replacing the current 3-line "soft
  verify"), (c) the §8.1 reconcile rewrite to name
  `- [ ]` explicitly (~3 lines), (d) the §9
  spec-discipline anti-patterns subsection (~25
  lines), (e) version bump 3.2.0 → 3.3.0. Net
  +45 lines.
- **`sources/skills/kdevkit/interviews.md`** —
  rewrites the `## Implementation Plan` template
  body to use checkbox syntax (~5 line change).
- **`tests/functional/skills/kdevkit-codebase-grounding.smoke`** —
  new judge fixture (~15 lines).
- **`tests/functional/skills/kdevkit-closure-verify-and-anti-patterns.smoke`** —
  new judge fixture (~25 lines).

No other files. No `setup.md` edits — none of the
four adoptions affect project genesis. No
`project.md` edits — the rule changes are in the
skill, not in project conventions.

### Why grounding lives in §6, not §3

§3 ("Load feature context") covers the entry-mode
resolution: continue / pick-up / start, plus the
backlog-promotion path. The grounding cue belongs
to the *fresh-start* path specifically — continue /
pick-up sessions are already grounded in the prior
session's read. §6 is the fresh-start home: "Four
short interviews" fires on start mode. The cue is
prefix-sized prose, not a new step.

### Why §8.2 lifts to "ask," not "auto-edit"

The user retains the call. Auto-editing project.md
on every closure introduces a ratcheting drift —
small additions accumulate, the section schema
slowly changes shape, the structural verify (§2)
flags it. Mandatory-asking with user-confirmed
staging keeps project.md hand-curated while
closing the today-decline-is-fine-and-nobody-
remembers-to-update gap.

### Why anti-patterns live in §9, not in CLAUDE.md

The user's `~/.claude/CLAUDE.md` carries
project-agnostic anti-patterns ("don't add
features beyond the task; default to no
comments"). Those fire universally. The four
anti-patterns this feature adds are
*spec-discipline-specific* — they only make
sense when the agent is following kdevkit's
plan→dev→close shape. CLAUDE.md is the wrong
home; §9 (cross-cutting kdevkit rules) is the
right one.

### Trade-offs considered

- **One closure-verify ask per touched section
  vs. one ask per dimension.** Per touched
  section chosen — silent skipping of untouched
  sections keeps the closure tractable. Per-
  dimension blanket ask would require the agent
  to ask 8 questions on every closure even when
  only Testing changed. Alternative rejected:
  ask all 8 dimensions every time — too noisy.
- **Checkbox markers vs. struck-through prose
  numbers.** Checkboxes chosen — Markdown native,
  renders in every viewer (GitHub, Obsidian,
  Code Browser), greppable as a literal `- [ ]`
  string. Struck-through prose
  (`~~1. step done~~`) renders inconsistently.
- **Anti-pattern bullet count: 4 vs. 6 vs. 8.**
  Four chosen — captures the highest-frequency
  failure modes (scope creep, refactor
  bundling, premature closure, silent plan
  amendments) without diluting into noise. The
  test cases in the literature each map to one
  of the four; adding more bullets risks
  defining anti-patterns nobody hits in
  practice.
- **Cue length in §6 grounding: one sentence
  vs. paragraph.** One sentence chosen — the
  cue compresses to a single line in the read
  flow. Longer prose risks pulling the agent
  into a "research phase" mindset that
  multiplies the work; the cue should be
  light.
- **Version bump 3.2.0 → 3.3.0 vs. 3.2.x
  patch.** Minor bump chosen — three contract
  additions (§6 cue, §8.2 lift, §9 subsection)
  + one template change. Patch (3.2.1) would
  imply no behavior change; honesty matters.
  Alternative rejected: 4.0.0 — would signal a
  breaking change that doesn't exist.

## Implementation Plan

<!-- ordered checklist; one slice per item -->

- [ ] Edit `interviews.md`'s feature template — switch
      `## Implementation Plan` body placeholder from
      prose-numbered to `- [ ]` checkbox shape per
      Requirements §3 above.
- [ ] Edit SKILL.md §6 — add the one-sentence
      codebase-grounding cue immediately before the
      "When entering a feature with no spec on disk..."
      paragraph. Wording per Requirements §1 above.
- [ ] Edit SKILL.md §8.1 — rewrite the reconcile cue
      to name `- [ ]` markers explicitly. Decision Log
      and unresolved-questions sweeps stay verbatim.
- [ ] Edit SKILL.md §8.2 — lift from "soft / offer"
      to per-touched-section structured ask. Wording
      per Requirements §2 above. The asking is
      mandatory; staging accepted edits stays.
- [ ] Edit SKILL.md §9 — add the
      "Spec-discipline anti-patterns" subsection
      (4 bullets) between "Commit hygiene" and
      "Skill-file placement". Wording per
      Requirements §4 above.
- [ ] Bump SKILL.md frontmatter `version` from
      `3.2.0` to `3.3.0`. Frontmatter `description`
      left unchanged unless review surfaces an
      inaccuracy.
- [ ] Run Quality Gate — `deno task fmt && deno
      task lint && deno task check`. Markdown
      changes only; fmt may reformat; lint/check
      no-ops for `.md`.
- [ ] Run Test Gate — `deno task test:unit` (23
      tests stay green); `deno task test:smoke`
      after worktree-redirect deploy.
- [ ] Add fixture
      `tests/functional/skills/kdevkit-codebase-grounding.smoke` —
      `prompt:` + `expected_narrative:` per Test
      Strategy §1 above. Public-repo-safe example:
      feature name `metric-export`.
- [ ] Add fixture
      `tests/functional/skills/kdevkit-closure-verify-and-anti-patterns.smoke` —
      `prompt:` + `expected_narrative:` per Test
      Strategy §2 above. Public-repo-safe example:
      feature name `add-prom-export`, file
      `maid/deploy.ts`.
- [ ] Audit two existing closure-fixtures
      (`kdevkit-feature-closure`,
      `kdevkit-closure-after-long-session`) for
      assertions that conflict with the §8.2 lift.
      Patch any "soft / offer" hard-assertions to
      align with per-section ask.
- [ ] Run §7 Test Gate with functional smokes
      (per-feature override). Deploy worktree:
      `deno task deploy --force`. Run the two new
      fixtures + the two regression fixtures.
      Read judge feedback; patch SKILL.md /
      interviews.md if needed within
      `retry_budget`. Restore symlinks to
      primary checkout when done:
      `cd ../mAId && deno task deploy --force`.
- [ ] Run Code Review Gate — per
      `code_review.reviewer: host-native`,
      threshold 70, hard-stop, retry-budget 2.
      Reviewer sees `project.md` + diff
      (SKILL.md + interviews.md + 2 new
      fixtures); no feature spec.
- [ ] Push. Open Agent-dev Review Gate per §7 /
      §9. Body: Approach (the four adoptions) +
      Reading order (SKILL.md §6 / §8 / §9 as
      intent; interviews.md as contract; new
      fixtures as plumbing).
- [ ] Closure — §8.1 reconcile (this spec's
      checkboxes ticked); §8.2 per-section verify
      ask (this feature touches Agent Development
      / kdevkit block by version-bumping the
      skill — the only project.md surface
      affected; other dimensions skipped); §8.3
      backlog cleanup ask; §8.3.5 N/A
      (not part of an initiative); §8.4 commit
      + push closure edits; §8.5 Closure Review
      Gate (title rewritten to feat); §8.6
      squash-merge; §8.7 branch cleanup; §8.8
      worktree teardown — offer.

Risk notes:

- **Wording drift on the §8.2 dimension list.**
  The eight project.md sections (Mission /
  Architecture / Tech Stack / Layout / Testing /
  Deployment / Hard constraints / Agent
  Development) are kdevkit-canonical. If
  project.md grows new sections in the future,
  the §8.2 cue needs to read from project.md's
  actual section list, not a hardcoded eight.
  v3.3 takes the hardcoded path for simplicity;
  flag for a future feature if project.md
  schema drifts.
- **Anti-pattern bullets and CLAUDE.md
  collisions.** The user's CLAUDE.md says "don't
  add features / refactor / introduce
  abstractions beyond what the task requires."
  Anti-pattern 1 (scope creep) overlaps. The
  kdevkit anti-pattern is more specific — it
  references the *Implementation Plan* as the
  contract — so it's complementary, not
  duplicative. Reviewer should flag if any
  bullet is verbatim CLAUDE.md prose.
- **§6 grounding cue triggering on too-many
  paths.** The cue is scoped to fresh-start
  (start mode) only — continue / pick-up
  sessions are already grounded. If the cue's
  wording is loose enough to fire on
  continue mode, it duplicates work. The
  template language ("Before the first
  interview...") naturally scopes to fresh-
  start because continue mode doesn't run
  interviews. Test for this in the grounding
  fixture's wrong-answers list.
- **Closure fixture dual-coverage risk.**
  `kdevkit-closure-verify-and-anti-patterns.smoke`
  covers two patterns in one
  `expected_narrative`. If one pattern's
  assertion is loose, the judge may pass
  the other and fail the loose one without
  signaling which. Mitigation: both patterns'
  assertions in the narrative are concrete
  ("at minimum Testing and Layout surfaced";
  "the unrelated-refactor question gets the
  anti-pattern response"). If judge flake
  surfaces, split into two fixtures.

## Session Log

<!-- append: date · what was done · decisions made -->

- 2026-06-10 · feature spec authored · §6 four
  interviews completed autonomously per the
  retrospection thread's already-confirmed scope
  (Patterns 2/3/5/7 adopt; 1/4/6/8 deliberately
  rejected): Requirements (codebase-grounding cue
  in §6 fresh-start, per-section closure verify
  ask in §8.2, checkbox Implementation Plan in
  template, four-bullet anti-patterns subsection
  in §9, version 3.2→3.3); Test Strategy (two new
  judge fixtures + audit two existing closure
  fixtures + per-feature functional-smoke
  override at closure); Design (rationale for
  why-these-four-not-eight; diff shape; placement
  rationale for grounding/§9 anti-patterns; trade-
  offs on section vs. dimension ask, checkbox vs.
  strikethrough, bullet count, cue length, version
  bump); Implementation Plan (15 ordered slices
  with per-step risk notes).

## Decision Log

<!-- append: decision · rationale · alternatives rejected -->

- **Adopt four patterns (2 / 3 / 5 / 7); reject
  four (1 / 4 / 6 / 8).** Rationale: per
  retrospection thread's analysis. Adopted
  patterns compose with kdevkit's existing shape
  (single-file spec, host-agnostic,
  human-gated). Rejected patterns require
  shape changes (multi-file specs;
  tool-capability gating; per-edit commit
  granularity; auto-extracted standards) that
  break kdevkit invariants for marginal
  gains. Alternative considered: adopt all
  eight to match the popular ecosystem fully —
  rejected; would dilute kdevkit's identity
  and cost maintainability.
- **§6 grounding cue, not a `research.md`
  artefact.** Rationale: the read-the-codebase
  step is real and worth codifying, but adding
  a new artefact compounds review-surface
  cost. The Session Log is the existing home
  for "what I learned scanning"; the cue
  enforces the read without a new file.
  Alternative rejected: spec-kit-style
  `research.md` — adds a sixth file to read
  for every review; review fatigue.
- **§8.2 promoted to mandatory-ask, not
  mandatory-edit.** Rationale: project.md is
  hand-curated by design — auto-edits drift
  the schema. Mandatory-asking closes the
  "decline is fine and everyone forgets to
  update" gap without forcing edits the user
  doesn't want. Alternative rejected:
  auto-edit project.md when the agent detects
  a touched section — too invasive.
- **§8.2 ask scoped to *touched* sections,
  not all eight every time.** Rationale: per
  closure tractability. Asking 8 questions
  every time generates user fatigue; touched-
  only keeps the lift productive. The agent
  decides "touched" from the diff. Alternative
  rejected: blanket-ask all eight on every
  closure — noisy.
- **Checkbox markers (`- [ ]`) over
  strike-through prose.** Rationale: Markdown
  native, renders consistently across viewers
  (GitHub PR, Obsidian, Code Browser),
  greppable as a literal string for §8.1
  reconcile. Strike-through has rendering
  inconsistencies. Alternative rejected:
  strike-through prose numbers — viewer
  inconsistency.
- **Anti-patterns subsection lives in §9, not
  CLAUDE.md, not in `interviews.md`.**
  Rationale: anti-patterns are
  spec-discipline-specific (reference the
  Implementation Plan as a contract; scoped
  to plan→dev→close shape). CLAUDE.md is
  project-agnostic; `interviews.md` is
  fresh-start-only. §9 cross-cutting fires
  every dev slice — the right home.
  Alternative rejected: scatter anti-patterns
  across §6 / §7 / §8 — fragments the
  discipline; harder to discover.
- **Four anti-pattern bullets, not six or
  eight.** Rationale: scope-creep / refactor-
  bundling / premature-closure / silent-plan-
  amendments are the highest-frequency
  failure modes. Adding more risks defining
  anti-patterns nobody hits. Alternative
  rejected: six or eight bullets — dilutes
  the signal.
- **Per-feature functional-smoke override
  at closure.** Rationale: same shape as
  v3.2's closure — the contract change is
  the agent's behavior; only smokes verify
  it. Pre-closure user-driven would lengthen
  the feedback loop. Alternative rejected:
  globally lift project.md's "user-driven"
  rule — over-broad; most features don't
  need agent-side functional runs.
- **Version bump 3.2.0 → 3.3.0
  (signpost minor).** Rationale: three
  contract additions + one template change.
  Patch (3.2.1) would imply no behavior
  change. Alternative rejected: 4.0.0 — no
  breaking change; would mislead.
- **Existing specs not migrated to
  checkbox shape.** Rationale: forward-only
  per v3.2 precedent. The §8.1 reconcile
  cue tolerates both shapes (`- [ ]` and
  prose numbered) so old specs stay valid.
  Alternative rejected: bulk-rewrite all
  feature specs' Implementation Plans —
  churn without payoff.
- **Eight project.md dimensions hardcoded
  in §8.2 cue.** Rationale: kdevkit's
  project.md schema is canonical (six +
  Hard constraints + Agent Development).
  v3.3 takes the hardcoded path for
  simplicity. If project.md schema drifts,
  a future feature can refactor §8.2 to
  read sections dynamically. Alternative
  rejected: dynamic per-section read — over-
  engineered for v3.3's purpose.
