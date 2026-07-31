# Feature: kreviewkit

## Git Setup

- Branch: `feat/kreviewkit` (pre-created worktree).
- Base: **`main`** (rebased 2026-07-31). Originally stacked on
  `feat/fixtures-discovery-vs-content-split` (`588b06c`) at user
  direction; that branch squash-merged to `main` as PR #34 and was
  deleted, which auto-closed PR #35 (closed, not merged — no
  kreviewkit work reached `main`). The 12 kreviewkit commits were
  rebased `--onto main`, dropping the 15 base-branch commits already
  in `main` under the squash SHA. The stacked-base risk note called
  this; §10's cross-stream rebase mechanics is the rule that applied.

## Feature Brief

<!-- The capability layer — what can the user now do that they
     couldn't before? -->

A new standalone skill, **`kreviewkit`**, that produces a
**human-consumable review briefing** for a completed change. A
fresh, independent, **read-only** reviewer is given project context,
the feature **spec**, the diff, and (where available) the test run
report — plus read access to the branch under review — and writes a
report that plays the feature back,
reconciles what the spec called for against what was built, and
points the human at what most needs their judgement. The briefing
**is** the pull-request / code-review body.

**Independent and read-only** is the load-bearing property. The
reviewer is a fresh agent that never wrote the code, works from what
it is given plus **read-only** access to the branch under review, and
**cannot write anything** — no edits, no commits, no pushes, no
network calls, no test execution. It reads; it reports. What it must
not have is *insight it wasn't given*: no implementer conversation
history, no session narrative, nothing that lets the change justify
itself to its own reviewer.

Read-only access to the **wider branch** is deliberate, not a
loophole. A reviewer that can only see diff hunks cannot do the job:
four added lines inside a fifty-line method are only judgeable by
reading the method, and "does this belong here / does it duplicate
something / does it fit the architecture" are all questions about
code the diff never touched. Google's review guide is explicit that
reviewers should open the whole file, and zoom out to the system, for
exactly this reason (see References). So the reviewer may read the
branch; it may not mutate it, and it may not import the implementer's
reasoning.

It fills the role of an **independent review-briefing tool**.
`kdevkit` asks for that *role* at the dev → closure handoff — it
never names kreviewkit — and kreviewkit declares it fills the role.
Install, or a project's own context file, binds the two. So
kreviewkit stands fully alone: hand it a spec and a diff and it
writes the briefing, no kdevkit required.

It is the **complement** to kdevkit's §7 Code Review Gate, not a
duplicate of it. That gate is a *blind, machine-facing* reviewer,
deliberately denied the feature spec so it judges the diff against
the project's invariants without bias, and it emits a pass/fail
score. kreviewkit is *spec-aware and human-facing*: it is given the
spec on purpose, because reconciling spec-vs-diff and replaying the
design decisions is the whole point, and it emits prose a human
reads. Between them they close both axes kdevkit §9 names — the gate
checks *diff-vs-project*; kreviewkit checks *diff-vs-spec* and hands
the human a map.

### Audience: the human about to review — not the author, not a gate

The briefing is written **for the human reviewer, before they
review**. That audience is what separates kreviewkit from everything
in References, and the distinction has to survive into the skill's
prose or it will drift back into being another automated reviewer:

| | Audience | Speech act | Success looks like |
|---|---|---|---|
| **Automated reviewer** (Sourcery, CodeRabbit, kdevkit §7 gate) | the **author** | "fix this" — findings, severity, blocking status | defects caught before a human looks |
| **Reviewer guidance** (Google eng-practices) | the **reviewer** | "look for this" — a standing checklist to apply | reviewers who review well, on any change |
| **kreviewkit** | the **reviewer**, about this change | "here is what shipped, and where your attention is worth most" | a human who reviews *faster and better* than reading the diff cold |

Consequences that bind the design:

- **It orients; it does not adjudicate.** Its job is to hand over
  understanding and a focus order, not a verdict. Where it has a
  concern it says "this is worth your judgement, and here's why" —
  not "this is a defect, fix it." Adjudicating is kdevkit's §7 gate's
  job, and it already exists.
- **The human stays the reviewer.** The briefing must never read as
  "already reviewed, nothing to see." It makes the human's review
  cheaper, never optional — a briefing that induces rubber-stamping is
  a failure even if every statement in it is true.
- **It is written to be read once, in order, then set aside.** Not a
  standing checklist (Google's guide is that, and is referenced rather
  than restated) and not a findings database (the §7 gate is that).
- **Author-facing artefacts are borrowed with care.** Conventional
  Comments' labels encode *author obligation* (`blocking` = "you must
  fix before merge"). Re-pointed at a reviewer they mean *"how much of
  your attention this deserves"*. The skill uses the grammar for its
  legibility but restates the audience, so `issue (blocking)` reads as
  "don't approve without resolving this", not as an instruction to the
  author.

### What "spec" means here

The **spec** is the feature's full statement, not merely a task list:

- **What the feature is** — the capability and the experience.
- **How it should be tested** when done — the success criteria and
  which test layers verify them.
- **The details and design**, considered against the overall
  project's architecture and constraints.
- **The implementation plan**.

Under kdevkit that is `$SPEC_ROOT/feature/<feature>.md`. Standalone,
it is whatever plays that part — a linked issue, a design doc, a
pasted intent. The briefing reconciles against whatever spec it was
handed, and says plainly when it was handed a thin one.

## Requirements

<!-- The experience layer — the cues the agent recognises and the
     artefacts it produces. Skill "experience" = triggers + output,
     not internal dispatch mechanics (those are Design). -->

### Trigger experience

- **Explicit invocation is the primary path.** The user (or a
  calling workflow) names the skill / the role: *"use kreviewkit"*,
  *"run the review briefing"*, *"brief this for review"*. Explicit
  naming is preferred because it stays unambiguous once several
  review-ish tools are installed side by side.
- **Implicit pickup is the fallback, and must work.** When no other
  skill covers this kind of work, a bare task must reach kreviewkit
  unaided: *"review what was done"*, *"prep this for review"*,
  *"summarise this change for a reviewer"*, *"open the PR for this"*.
  A user who installed exactly one review-briefing tool should not
  have to know its name. The `description:` therefore leads with
  those user-phrased triggers (per project.md's "triggers belong in
  `description:`" rule) **and** names the role, so both paths land.
- **Works with or without kdevkit.** Given a spec and a diff it
  writes the briefing. When no real spec exists, it says so and
  reconstructs intent from commits / PR description rather than
  silently pretending it was handed one.
- **Announces in the response, not in the artefact.** The chat reply
  that uses this skill opens with the literal line `[kreviewkit]
  applies` (the self-announce contract that makes
  activation/discovery testable — see project.md Testing). The
  **briefing artefact stays clean** — no marker line leaks into the
  PR/CR body.

### The briefing (the artefact a human reads)

The output is one report with four sections, in this order. It is
written as the PR/CR body.

**It must not restate its own inputs.** The reviewer already has the
spec and the diff in the review; re-describing either is padding that
buries the parts only the briefing can supply. The briefing's value is
the **delta between intent and artefact** — so no spec recap, no
file-by-file diff recitation, and citations of the spec only where the
diff meets it, diverges from it, or leaves it unaddressed.

1. **Playback — the shape of what landed.** Orientation only: enough
   for a reviewer to hold the change in their head before opening a
   file — what it does and where its risk concentrates. Then the
   load-bearing design decisions, but **only those the diff reveals**
   (a decision the code embodies, an alternative weighed, a choice a
   reviewer would otherwise reverse-engineer). A decision the spec
   states plainly and the diff merely implements is left to the spec;
   restating it is reading, not briefing.
2. **Spec ↔ diff reconciliation.** What the spec called for vs. what
   the diff delivers — unmet requirements, scope that crept in,
   silent amendments to the implementation plan, unrelated changes
   bundled in (the §9 spec-discipline anti-patterns, surfaced *after
   the fact*). Includes a **V-model coverage read**: do the
   functional/integration tests map onto the requirements, and the
   unit tests onto the design primitives, that the spec declared?
   Where a test run report was supplied, coverage is read against
   what actually *ran and passed*, not merely what exists in the
   diff; where it was not, the briefing says coverage is unverified
   rather than implying it was checked.
3. **Where to focus.** A risk-ranked, *why*-annotated reading map
   using kdevkit's existing vocabulary — *Read for intent / Read for
   contract / Read for plumbing* — that tells the reviewer where to
   spend attention and where they can skim. Sequence or flow diagrams
   appear **only where control flow is non-trivial**, never as
   decoration.
4. **Needs your judgement.** *Only* the calls a human must ratify —
   design trade-offs with defensible answers either way, risks to
   accept or reject, cost/benefit spends that are the human's to make.
   Labelled `question` / `suggestion` / `nitpick` / `praise`.

- **Defects route back to the loop, not into the briefing.** A bug, a
  stale reference, a contradiction, an unmet requirement, a wrong
  assertion, a missing test — none of these are judgement calls. The
  briefing fires *at dev-loop completion*, so a defect means the loop
  is not actually complete: it belongs in the agent session as the next
  slice, not on the review surface. The generator reports defects to
  its caller on a **separate channel** from the briefing, and the
  caller re-runs the briefing on the fixed work. **The published
  briefing describes finished work, not its own loose ends.** Dividing
  test: *would fixing it make the finding disappear?* Yes → defect,
  route it back. No, because it is a choice → judgement, publish it.
  (This also means section 4 is legitimately sometimes empty; that is
  distinct from a shallow review, so a briefing that found nothing at
  all on a non-trivial diff must say so explicitly.)

- **Independent, read-only.** The briefing is written by a fresh
  reviewer that is **not** the agent that produced the code. It may
  **read** the branch under review — whole files, neighbouring code,
  tests, history — and it may **write nothing**: no edits, commits,
  pushes, network calls, or test runs. Independence is a property of
  *who* reviews and of *not inheriting the implementer's reasoning*,
  not of starving the reviewer of code context.
- **Honest, not celebratory.** The briefing surfaces gaps, risks, and
  unmet spec items plainly; it is a reviewer's aid, not a marketing
  summary. A briefing that finds nothing to focus on for a
  non-trivial diff is itself a smell.
- **Says what it was not given.** A missing or thin spec, an absent
  test report, or a diff it could not interpret are stated plainly in
  the briefing rather than papered over. Reading the branch resolves
  *code* questions; it does not manufacture an intent the spec never
  stated — that gap is a finding.

### Role-based integration experience (no hard coupling)

- **The calling workflow asks for a role, not a product.** kdevkit
  says "dispatch an independent review-briefing tool to produce a
  briefing for human consumption"; it must not name kreviewkit. Any
  tool that fills the role and honours the reviewer contract (given
  inputs + read-only, no write authority) can serve.
- **kreviewkit advertises the role.** Its `description:` declares it
  is an independent review-briefing tool, which is what lets a
  calling workflow (or a bare user task) find it.
- **Binding happens by install or by project context.** Installing
  exactly one review-briefing tool is sufficient — it gets picked up.
  Where a project wants to be explicit, or has several installed, its
  own context file (`project.md`'s `## Agent Development` block under
  kdevkit, or an `AGENTS.md`-equivalent when kdevkit isn't in play)
  names which tool fills the role.
- **Dispatched at the dev → closure handoff.** When kdevkit is
  driving: after the §7 Agent-dev Review Gate (push done) and before
  the §8 closure cue, so the human has a real briefing to read before
  saying "close it".
- **Opt-in, non-disruptive.** Additive and configurable. A repo that
  does not enable it sees kdevkit behave exactly as today.
- **Maps onto kdevkit's §9 body contract.** The four sections subsume
  the §9 body shape (Why + phase content + Reading order): Playback
  carries the Why/Approach, Where-to-focus *is* the Reading order, and
  the two new sections enrich it. No conflicting body shapes.

## Test Strategy

<!-- Per project.md's two-layer surface + the five kinds of skill
     test. Agentic runs stop at `just test`; functional (API-cost)
     is user-driven. -->

### Unit (`just test`, load-bearing — §7 Test Gate default)

- The build-tool content validator must accept the new
  `resources/content/skills/kreviewkit/SKILL.md` (valid frontmatter:
  `name`, `description`, `version`). Existing validator/symlink tests
  stay green. No new Rust logic expected — a new skill is data,
  covered by the existing validator path; the role is advertised in
  `description:`/`tags:`, so no frontmatter schema change. Confirm,
  don't add overhead.

### Smoke / structural (after install)

- `just resources::status-skills` confirms the `kreviewkit` symlink
  resolves through the registry to all three tools. The registry
  symlinks the whole `resources/content/skills/` dir, so a new folder
  is covered without a registry edit — **verify the codex `FanOut`
  path picks up the new dir** (the one place a per-skill fan-out could
  miss it).

### Functional (judge mode; user-driven per project convention)

Fixtures under `resources/tests/skills/`. `activation` + `discovery`
come free from the self-announce contract + `description:`.

| Behaviour claim | Fixture | Kind |
|---|---|---|
| Loaded when named, announces `[kreviewkit] applies` | generated | activation |
| Bare "review what was done" task reaches it unaided (implicit fallback) | generated | discovery |
| States its own contract: four sections, read-only-but-spec-aware, briefing = PR/CR body | `kreviewkit.smoke` `--- playback ---` | playback |
| Knows its audience: briefs a human about to review; orients rather than adjudicates; never a verdict | `kreviewkit.smoke` `--- playback ---` | playback |
| States the isolation model correctly: reads the wider branch, writes nothing, no implementer history | `kreviewkit.smoke` `--- playback ---` | playback |
| Given a spec + diff, produces the four-section briefing with a risk-ranked reading map | `kreviewkit.smoke` `--- enact ---` | enact |
| Reads beyond the diff — cites context from an unchanged file when the diff alone is misleading | `kreviewkit.smoke` `--- enact ---` (seeded) | enact |
| Leaves the branch untouched — no edits/commits after a briefing run | `kreviewkit.smoke` `--- assert ---` | enact |
| Catches spec↔diff drift — scope creep / unmet requirement / missing test coverage surfaces in section 2 | `kreviewkit.smoke` `--- enact ---` (seeded drift) | enact |
| kdevkit dispatches *a role*, not kreviewkit by name, at dev→closure | `kdevkit-dev-loop.smoke` (extended) | functional |

- The **read-only** property is the cheap, behaviorally-checkable one:
  a `--- setup ---`/`--- assert ---` fixture snapshots the worktree
  (`git status --porcelain` + rev) before and after a briefing run and
  asserts nothing changed. Pair it with a presence check (the briefing
  file *was* produced) so a no-op agent can't pass it.
- The **reads-wider-context** property gets the mirror fixture: seed a
  repo where the diff looks fine in isolation but is wrong given an
  unchanged neighbouring file (e.g. it duplicates an existing helper).
  A compliant briefing names the duplication; a diff-only reviewer
  cannot.
- **Wrong-answer cues** inline in each `expect:` narrative: acting as
  the implementer instead of an independent reviewer; *modifying* the
  branch (fixing what it found) instead of reporting; claiming it may
  not read unchanged files; running the test suite instead of reading
  the report; rubber-stamping (no focus items on a non-trivial diff);
  leaking `[kreviewkit] applies` into the briefing artefact; a flat
  file list instead of a risk-ranked intent/contract/plumbing map;
  treating the spec as ground truth rather than a claim to reconcile
  against the diff; kdevkit naming kreviewkit directly instead of
  asking for the role.
- `tools: claude,kiro` for the judge fixtures (cross-tool evidence
  for a new skill), `claude` default elsewhere.
- Prefer behavioral (`--- setup ---`/`--- assert ---`) where the
  briefing lands as an inspectable file; fall back to judge narrative
  for the irreducibly-prose independence/tone claims.

### Dogfood run (real usage — complements the fixtures)

The fixtures prove the contract in the small; the dogfood run proves
the skill is *actually useful to a human*, which is the only success
criterion that matters and the one no fixture can assert. **This
feature's own PR body is produced by kreviewkit.**

- **Setup:** once the skill is authored and installed, dispatch it
  independently (fresh agent, read-only) on this very branch —
  inputs: `specs/project.md`, `specs/feature/kreviewkit.md` (the
  spec), the diff vs. base, and the `just test` report.
- **The artefact:** the returned briefing replaces the PR body at the
  Agent-dev Review Gate. The user reads it *as their review briefing*
  for this feature.
- **What it tests that fixtures can't:** whether the four sections
  actually orient a human faster than reading the diff cold, whether
  the focus map points at the right things, whether section 4 asks
  for judgement on the genuinely contestable calls (role-resolution
  discovery, read-only enforcement per host), and whether the prose
  reads as a briefing rather than a verdict.
- **Pass condition is the user's judgement, not a score:** the user
  reports whether the briefing made reviewing this change easier. A
  briefing that reads well but tells the user nothing they didn't
  know is a fail; so is one that reads as "already reviewed."
- **Honesty check built in:** the dogfooded briefing must be produced
  by a genuine independent dispatch, not hand-authored by the
  implementing session. If the dispatch can't be made to work, that
  is a finding to surface, not something to paper over by writing the
  briefing by hand.

Findings from the dogfood run feed back as dev-loop slices before
closure.

Quality gate: `just fmt-check` + `just lint` + `just check` (or
`just ci`) after each slice.

## Design

<!-- The "how" layer. Rationale first. -->

### Why a standalone skill, not a kdevkit section

The briefing is useful anywhere a spec and a diff exist — reviewing a
colleague's PR, a change made outside kdevkit, a hotfix. Folding it
into kdevkit's §8 would couple it to the closure phase and kill
standalone use. A separate skill (the `browser` / `notes` precedent)
keeps it reusable and keeps kdevkit's always-on context lean. The
`k*kit` name signals the "pairs with kdevkit, stands alone"
relationship.

### Role indirection, not a named dependency

kdevkit must not name kreviewkit. It declares a **role** —
*independent review-briefing tool* — and dispatches to whatever fills
it, exactly as its existing `code_review.reviewer` block dispatches
to a reviewer reference rather than a hardcoded reviewer. Resolution
order:

1. The project's own context names the tool (`review_brief.generator`
   in `project.md`'s `## Agent Development > kdevkit`, or an
   `AGENTS.md`-equivalent when kdevkit isn't in play).
2. Otherwise, the single installed tool advertising the role is used
   — which is what makes the "I installed one, it just works" path
   in Requirements true.
3. Several installed and none named → ask once and persist (kdevkit's
   standard "resolve, then persist" move).

This keeps the two skills independently shippable and lets a project
swap in a different briefing tool without touching kdevkit.

### The reviewer contract — given inputs + read-only branch

kreviewkit reuses kdevkit's **fresh-context agent call** primitive
(the same one §2 verify and the §7 gate use), with two differences:
the inputs are inverted (it *is* given the spec), and the reviewer is
**read-only**.

Two separate axes, which the first draft of this spec wrongly
conflated into "no filesystem":

- **Context** — the reviewer needs *more* than the diff. Read access
  to the branch is required, not merely tolerated.
- **Authority** — the reviewer needs *none*. It may not change
  anything, anywhere.

**Given to it** (packaged by the caller, passed as content):

- ✅ **Project context** — `project.md` and a repo-root `AGENTS.md`
  where one exists; whatever equivalent plays that part when kdevkit
  isn't in use.
- ✅ **The feature spec** — the full statement (capability, test
  expectations, design-in-project-context, implementation plan). This
  is what §7's gate withholds and kreviewkit needs.
- ✅ **The diff vs. base**, plus the base ref so it can orient.
- ✅ **Decision / Session logs** where the spec carries them (the
  "alternatives weighed" that Playback replays).
- ⚪ **Test run report, where available** (optional). Lets section 2
  read coverage against what actually ran and passed rather than
  merely what exists in the diff. Absent → the briefing states
  coverage is unverified. Optional rather than required because not
  every invocation has one (a standalone PR review often won't), and
  requiring it would block the standalone path.

**Allowed to reach, read-only** — the reviewer may:

- ✅ **Read any file on the branch under review**, not just the
  changed hunks: the whole file around a change, callers and
  callees, sibling modules, existing tests, neighbouring conventions.
- ✅ **Read git history / blame** on the branch to see whether a
  pattern is established or newly introduced.

**Denied** — the reviewer may not:

- ❌ **Write anything.** No file edits, no commits, no pushes, no
  branch or PR mutation. It returns prose; the caller acts on it.
- ❌ **Execute the build or test suite.** It reads a test report if
  given one; it does not run or "fix" anything. (Read-only also means
  no side effects, and a reviewer that can run arbitrary commands is
  not read-only.)
- ❌ **Reach the network.** No fetching issues, CI results, or
  external state mid-review. (Distinct from the *authoring* step:
  reference material is fetched once and inlined into the skill — see
  References.)
- ❌ **See the implementer's conversation history or session
  narrative.** This is the real isolation requirement: the change
  must not get to justify itself to its own reviewer.

Rationale for the correction: a diff-only reviewer cannot answer the
questions that matter most — does this belong here, does it duplicate
something, does it fit the architecture, is this four-line addition
sitting inside a method that should have been split. Those are all
questions about code the diff never shows. Google's review guide says
so directly ("Review tools show only a few lines around each edit, so
open the whole file when needed", and zoom out to the whole system);
starving the reviewer to achieve "isolation" would trade review
quality for a purity that was never the point. The property actually
worth enforcing is **no write authority and no inherited
justification**, which is cheap to enforce and doesn't degrade the
read.

**Enforcement, in order of preference** — and the guiding rule is
*take the mechanism the host makes cheap*, not the most theoretically
airtight one:

1. **Host-level read-only tool restriction** where the host supports
   it (dispatch the reviewer with read/search tools only, no
   write/execute). Cheapest and strongest; preferred.
2. **A read-only checkout** (e.g. a detached worktree the reviewer is
   pointed at) where a host can't scope tools but can scope paths.
3. **Prose prohibition** as the floor, stated unconditionally so a
   compliant agent honours it on any host.

Deliberately *not* required: sandboxes, container isolation, or a
bespoke read-only filesystem layer. Those are the "lot of hoops" that
would sink the feature; the dev-phase slice picks whichever of the
three above the hosts actually make easy, and the spec commits to the
*property* (no writes, no inherited context) rather than to a
mechanism. If a host offers none of the three, the briefing still
carries value — record the weaker guarantee in the Session Log rather
than blocking.

The contract is described abstractly in the skill prose (portable
across Claude Code / Kiro / Codex), matching how §7 stays
host-agnostic.

### Briefing generation

The four sections map to sources:

- **Playback** ← diff + Decision Log (decisions & alternatives) +
  the spec's capability/experience statement.
- **Reconciliation** ← spec requirements + implementation plan vs.
  diff; §9 anti-pattern checklist applied retrospectively; V-model
  coverage = declared test expectations cross-checked against test
  changes in the diff and, where supplied, the test run report.
- **Where to focus** ← risk read of the diff *in the context of the
  surrounding code the reviewer read*, bucketed into kdevkit's
  Read-for-intent / -contract / -plumbing groups; diagrams gated on
  non-trivial control flow.
- **Needs your judgement** ← residue: contestable decisions, gate
  blind spots, high-risk surfaces, and anything left unverifiable.

**Don't re-derive established review dimensions — reference them.**
The skill points at Google's engineering-practices reviewer guide for
*what to look at* (design, functionality, complexity, tests, naming,
comments, style, consistency, documentation) and for the
broad-then-main-parts-then-rest navigation order, rather than
inventing a parallel checklist. Its own additions are the parts that
guide doesn't cover: the spec↔diff reconciliation and the
briefing-as-PR/CR-body output. Section 4 uses **Conventional
Comments** label grammar (`issue` / `question` / `suggestion` /
`nitpick` / `praise` + `(blocking)` / `(non-blocking)`) so severity
and expectation are explicit and greppable instead of ad-hoc.

### References (fetch-and-inline, keep the link)

Prior art exists; the skill should stand on it rather than reinvent
it. Policy: **inline the distilled guidance so the skill works
offline, and keep the source URL beside it** so a later run can
re-fetch and update. No caching machinery, no build-time fetch step —
the URL is the provenance marker for a human or agent refreshing the
content later.

**Read these sources with the audience gap in mind.** Most were built
for a *different* audience than ours (see "Audience" above): the
automated-reviewer tools address the **author** with findings, and
Google's guide addresses a **reviewer doing the review** with a
standing checklist. kreviewkit addresses a **human about to review a
specific change**. So they are mined selectively — for what to draw
the human's attention *to* and how to order it — and their
author-facing framing (verdicts, blocking status, fix-this
imperatives) is deliberately **not** carried over. The `Taken as`
column below states which side of that line each source sits on.

| Source | Taken as | What we take from it |
|---|---|---|
| [Google eng-practices — What to look for in a code review](https://google.github.io/eng-practices/review/reviewer/looking-for.html) | reviewer-facing ✅ closest fit | The review dimensions (design, functionality, complexity, tests, naming, comments, style, consistency, docs) as the *attention agenda* the briefing points into; "open the whole file / zoom out to the system" — the citation behind read-only-but-wider-than-the-diff. |
| [Google eng-practices — Navigating a CL in review](https://google.github.io/eng-practices/review/reviewer/navigate.html) | reviewer-facing ✅ closest fit | Broad → main parts → the rest. Independent corroboration of *Read for intent / contract / plumbing*; "surface serious design concerns immediately" maps onto putting section 4 in front of the human early. |
| [Conventional Comments](https://conventionalcomments.org/) | author-facing ⚠️ re-pointed | Label + decoration grammar for section 4, **restated for a reviewer**: labels signal how much of the human's attention an item deserves, not an obligation on the author. |
| [Sourcery — anatomy of a review / Reviewer's Guide](https://docs.sourcery.ai/reviews/anatomy-of-a-review/) | automated-reviewer ⚠️ artefact shape only | The artefact shape: overview + file-level why-map ("where to focus") + diagrams only for non-trivial control flow; why/risk belongs in the PR *description*. Its inline findings-and-fixes layer is **not** our model. |
| [CodeRabbit — code review overview](https://docs.coderabbit.ai/guides/code-review-overview) | automated-reviewer ⚠️ artefact shape only | The walkthrough idea: orientation before opening a file, a review-effort/risk signal, narrative kept separate from localized findings. Its severity-ranked author-facing comments are **not** our model. |
| [GitHub — about pull request reviews](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/reviewing-changes-in-pull-requests/about-pull-request-reviews) | platform mechanics | Review states (comment / approve / request changes) and body-vs-line-comment split, for the output-binding step. Note kreviewkit writes the **body** and does not submit a review state — the human owns approve/request-changes. |

Each inlined block in `SKILL.md` carries a terse
`see <url>` pointer — per project.md's comment convention, a pointer,
not a retelling.

### Output binding — the briefing is the PR/CR body

The reviewer *returns* the briefing as prose; the **caller** publishes
it as the PR/CR description — the reviewer has no write authority and
no network, by design. Rationale (Sourcery/CodeRabbit prior art, see
References): the durable why/risk
framing belongs at the top of the review where it stays visible as
the conversation grows. Single artefact, no separate file to drift
(chosen over a `review-brief.md` + link, and over Sourcery's
description/comment split — both add a second artefact to keep in
sync for no gain at this scale).

### File / surface

- **New:** `resources/content/skills/kreviewkit/SKILL.md` — the
  skill. Single-file to start (browser/notes shape), ~150–250 lines.
- **New:** `resources/tests/skills/kreviewkit.smoke` — fixtures.
- **Edit:** `resources/content/skills/kdevkit/SKILL.md` — a minimal,
  additive **role-based** dispatch hook at the §7→§8 handoff, gated
  on config. Names the role, never kreviewkit. Kept small; kdevkit is
  critical.
- **Edit:** `specs/project.md` — document the `review_brief:` block
  under `## Agent Development`, and (closure-time) any
  Layout/Testing bump for the new skill.
- **No registry edit** — the whole skills dir is symlinked; a new
  folder is covered. (Verify codex `FanOut`.)

### Config shape (project.md `## Agent Development > kdevkit`)

```yaml
review_brief:
  enabled: true            # default: false — opt-in, non-disruptive
  generator: <ref>         # optional; omit to auto-resolve the
                           # single installed review-briefing tool
```

`<ref>` reuses the grammar kdevkit's `code_review` block already
defines (`host-native` / `skill:<name>` / `mcp:<server>.<tool>` /
`agent:<name>`), so there is one reference grammar, not two.
Omitting `generator:` is the common case: resolution falls to the
installed-role lookup above.

**No keys for inputs, isolation, or section shape** — those belong to
the generator's own declared contract, which kdevkit consults rather
than specifies. That is what lets a project configure a briefing tool
with an entirely different input contract without touching kdevkit.

## Implementation Plan

<!-- One slice per item. Three-phase per kdevkit. -->

### Planning phase

- [x] Land the spec as `plan(kreviewkit): initial spec`; push; open
  the Planning Review Gate (PR #35).
- [x] Revise per Planning Review Gate feedback round 1: spec
  terminology, bundle inputs + optional test report,
  explicit-primary / implicit-fallback triggers, reviewer isolation,
  role indirection.
- [x] Revise per feedback round 2: **read-only-not-sealed** isolation
  model (reads the wider branch, writes nothing), enforcement by
  whatever mechanism the host makes cheap, and a **References**
  section that stands on existing review guidance instead of
  re-deriving it.
- [x] Revise per feedback round 3: **audience framing** — the
  briefing serves a *human about to review*, which most references
  were not built for; plus the dogfood run as a real-usage test.
  Planning → dev cue given ("go ahead and build").

### Dev phase

- [x] **Probe the read-only enforcement options.** Mechanism 1
  (host-level tool restriction) is cheap on Claude Code — native
  `--allowedTools` / `--disallowedTools` / `--tools` flags plus a
  `tools:` field in agent definitions; no sandboxing needed. Prose
  remains the portable floor for hosts without it. Recorded in the
  Session Log.
- [x] **kreviewkit SKILL.md.** Authored: description (user-phrased
  triggers first, role advertised), self-announce contract, the
  four-section briefing contract, the read-only reviewer contract,
  audience framing, standalone-vs-kdevkit modes,
  briefing-as-PR/CR-body binding, and inlined reference guidance with
  `see <url>` pointers.
- [x] **Confirmed no registry/validator change needed.** The
  validator walks every skill dir generically and checks frontmatter
  (`name` + `description` non-empty) — `kreviewkit` passes. The codex
  `FanOut` **does** enumerate the new dir: `status-skills` reported
  `.codex/skills/kreviewkit missing`, proving discovery from the
  content dir with no registry edit. 97 unit tests green.
- [x] **kdevkit role-based dispatch hook.** Added §7 "Review Briefing
  (dev → closure hand-off)": opt-in via `review_brief:`, dispatches
  the *role* with three-step resolution, contract (spec included,
  read-only, fresh context), publishes as PR/CR body, explicitly
  non-gating. Wired into §2 verify + §4/§7 preference loading.
  Version 3.6.0 → 3.7.0. Verified: kdevkit never names kreviewkit.
- [x] **project.md docs.** Documented `review_brief:` under
  `## Agent Development > kdevkit` with mAId's own dogfood setting
  (`enabled: true`, `reviewer:` omitted → role resolution).
- [x] **Fixtures.** `kreviewkit.smoke` written: `playback` covers
  audience + four sections + read-only isolation; `enact` is
  behavioral, seeding a repo where the diff looks fine alone but
  duplicates an unchanged `src/util.py` helper (only a
  wider-branch reader catches it), declares a requirement the diff
  doesn't deliver (trimming) and unit tests never added. Asserts:
  briefing produced (no-op fails), worktree + commit count unchanged
  (read-only), no marker leak into the artefact, duplication named,
  reconciliation gap named, focus map present, no verdict.
  `kdevkit-dev-loop.smoke` extended for role-based dispatch with
  wrong-answer cues. `--dry-run`: all pass, `kreviewkit`
  activation/discovery generate correctly for claude + kiro.
- [x] **Dogfood run — ran four times, each on the then-current state,
  each briefing published as the PR body.** Findings folded back as
  dev-loop slices every round rather than shipped as caveats: round 1
  found the write-authority contradiction and the diff-context leak in
  the wider-branch assert; round 2 (a blind §7 Code Review Gate,
  independent of the briefings, scoring 58/100) found the `setup.md`
  schema gap and a three-way write-authority disagreement; round 3
  found the `no test` substring false positive; round 4 found the
  partially-applied `generator:` rename and the §7 section-ordering
  mismatch. The dogfood earned its place — **none of these were caught
  by the gates, the 97 unit tests, or `--dry-run`.**
- [x] **Quality + Test + Code Review + Push.** `fmt-check` + `lint` +
  `check` + `test` (97) + `--dry-run` green after every slice;
  behavioral `enact` run green 6×; PR #36 open against `main` with a
  kreviewkit briefing as its body. (PR #35 was auto-closed unmerged
  when its base squash-merged — see the rebase Session Log entry.)

### Closure phase

- [x] Reconcile markers; soft `project.md` verify; backlog cleanup ask;
  `close(...)` commits; Closure Review Gate; squash-merge; branch
  cleanup.

### Risk notes

- *Stacked base.* **Resolved 2026-07-31** — the base landed first
  (squash-merged as PR #34, branch deleted, which auto-closed PR #35),
  and this branch was rebased `--onto main` per §10's cross-stream
  rebase mechanics. The risk fired exactly as written; the mitigation
  was the one named. Now based on `main`, so it no longer applies.
- *Read-only enforcement is only as good as the host.* Where a host
  can't restrict tools or scope a checkout, it rests on prose
  compliance. The worktree-unchanged fixture is the check; treat a
  failure there as a real finding, not a fixture bug. Explicitly **do
  not** build sandboxing/container machinery to close the gap —
  that's the hoop-jumping this feature is avoiding.
- *"Read the branch" invites scope drift.* A reviewer free to read
  anything can wander into unrelated code and produce a sprawling
  briefing. Mitigation: the four-section shape and the risk-ranked
  focus map keep the *output* scoped even when the *reading* is broad;
  watch for briefings that review the repo instead of the change.
- *Reference rot.* Inlined guidance drifts from its source. Mitigation
  is the retained URL, not machinery — a later run re-fetches. Accept
  that the inlined copy may lag; it is a distillation, not a mirror.
- *Editing kdevkit.* The dispatch hook touches a critical,
  well-tested skill. Keep it additive and behind the `enabled: false`
  default; re-read `kdevkit-dev-loop.smoke` after the edit so the
  existing narrative doesn't go stale.
- *Role resolution ambiguity.* "Single installed tool advertising the
  role" needs a real discovery story per host. If it can't be made
  reliable, fall back to requiring `reviewer:` when more than one is
  installed — don't guess silently.
- *Overlap with §7 naming.* "Code Review Gate" (§7, blind, scored)
  vs. kreviewkit (spec-aware briefing) must stay clearly distinct in
  prose and fixtures — the same watch-item the code-review-gate
  feature flagged for "Review Gates" vs "Code Review Gate".
- *Prior-art alignment.* Section shape deliberately tracks Sourcery's
  Reviewer's Guide (file-level why-map + verification path) and
  CodeRabbit's walkthrough (orientation-before-you-open-a-file,
  effort/risk signal). Diverges in the explicit spec↔diff
  reconciliation, which those tools lack because they have no spec.

## Session Log

<!-- append: date · what was done · decisions made -->

- **2026-07-31** · **Closure.** Reconciled three unchecked plan items —
  all genuinely done (the dogfood ran four rounds, gates green after
  each slice, closure in progress). **Persistent-layer verify:** three
  `project.md` sections were touched. Testing's announce-contract list
  and Agent Development's `review_brief:` block were already updated
  in-flight; **Architecture** was the one durable gap and gained a note
  on **role dispatch between skills** — the new architectural
  relationship this feature introduces, with the two rules that keep it
  from becoming coupling (caller never names a filler; filler owns its
  invocation contract) plus the safety-floor obligation. That is the
  binding constraint on *future* features, so it belongs in the
  persistent layer rather than only in this feature's spec. Layout
  needed no edit (its tree is generic over `skills/<name>/`).
  **Backlog cleanup:** asked; answer is **none** — no item is resolved.
  The two test-runner items this feature brushed against
  (`test-runner-workdir-containment`, `test-runner-sandbox-asymmetry`)
  stay open, since it added a `.gitignore` stopgap and explicitly did
  not fix the containment gap; `pr-review-tui-across-hosts` is adjacent
  but distinct (it wants a terminal review UI, not a briefing). Filed
  one new item: `kdevkit-refactor-shrink-always-on-context`, prompted by
  this feature adding ~90 lines to an already-1250-line always-on file
  despite deliberately deferring everything it could.

- **2026-07-31** · **Three corrections from reading the published PR.**
  (1) **Defect/judgement split.** The user asked why "needs your
  judgement" was in the PR at all, since such items imply the dev loop
  is incomplete — and if it is, why brief. Correct, and a design error:
  section 4 was conflating defects (fix them; the loop isn't done) with
  trade-offs (ratify them; no fix removes them). Every item across four
  briefings had been the first kind. Section 4 is now judgement-only,
  defects return on a separate channel, and kdevkit routes them as the
  next slice. (2) **Safety floor** added to kdevkit per user direction —
  no write authority, no implementer history, no credentials, no
  unattended network/shell, binding regardless of what a generator's
  contract asks for, so a malicious or misconfigured briefer can't
  widen its own authority. (3) **Fixture rigour re-aimed.** The
  missing-tests grep had been patched three rounds and stayed
  defeatable; root cause was matching *vocabulary* rather than
  *evidence*, so it now requires a coverage phrase **plus** a
  seed-unique token. The behavioral run then **failed** — correctly:
  the seeded change is all defects, so the new rule made the agent route
  them back and leave section 4 to trade-offs, while the assert still
  demanded labelled items. Rather than guess at wording a third time, I
  ran the agent against a reproduced seed and **read the actual
  briefing**: it had produced two genuine `**question —**` trade-offs
  (each noting it survives any fix) and an explicit note that the three
  defects went back to the dispatcher. The em-dash label shape was what
  my pattern missed. Loosened that one assert deliberately and verified
  all ten against the real output.

- **2026-07-31** · **Fourth briefing round; caught a
  partially-applied rename.** The post-rebase briefing found that the
  `reviewer:` → `generator:` rename had landed in `setup.md`, §7, and
  `project.md` but **not** in kdevkit's §4 preference-loading list —
  which pointed an agent at a key `setup.md`'s own schema check would
  reject as unknown. Verified and fixed, along with two spec-side
  instances (Design's resolution order, the Config shape block) that
  documented a key which no longer exists. Also fixed: the spec's
  Requirements §1 still demanded the capability-restating Playback the
  refactor had just forbidden — a silent amendment by the spec's own §9
  definition, now reconciled. Resolved the Output-rule-0 vs Publishing
  tension (announce line vs "no preamble") by making the
  verbatim-publish case an explicit exception that wins. And **moved
  `### Review Briefing` physically before `### Agent-dev Review Gate`**:
  the prose said "step 0 of the gate above" while sitting after it, so
  layout now matches execution order (Push → Briefing → Review Gate)
  instead of relying on prose to correct the reading order — this
  project's own guidance is that ordering rules are what slip first.
  Gates + behavioral run green after each change.

- **2026-07-31** · **Rebased onto `main`; PR re-opened.** The stacked
  base landed while this branch was in flight: PR #34 squash-merged
  `feat/fixtures-discovery-vs-content-split` into `main` and deleting
  that branch **auto-closed PR #35** (closed, not merged). Measured
  both recovery options before acting rather than assuming: leaving it
  alone showed **27 commits / 27 files / +2987** in a PR against `main`
  (a stale merge-base re-presenting 15 commits `main` already had under
  the squash SHA); merging `main` in gave 15 files and **9 conflicted
  files**, several in files this branch never touched plus an add/add on
  another feature's spec; rebasing `--onto main` gave **12 commits / 9
  files / +1576** and only **2 conflicts**. Rebase won on the axis that
  mattered — "the PR shows only what we changed" — and on conflict count,
  because dropping the already-merged commits stops git reconciling
  against re-written history. Both conflicts were prose in files #34 also
  edited (`kdevkit/SKILL.md` frontmatter, `kdevkit-dev-loop.smoke`
  narrative); resolved by **combining both intents** — `main`'s shortened
  description and its two judge-narrative improvements (the
  narrower-re-run precision clause and the "an answer more precise than
  this narrative is correct" instruction) kept alongside this feature's
  version bump and briefing clause, rather than picking a side. All gates
  re-run on the rebased tree (97 unit tests, `--dry-run`, and the
  behavioral `enact` run) since a rebase invalidates prior green.
  **Process note:** an earlier attempt to "dry-run" the rebase actually
  mutated the branch and left it mid-conflict — a rebase is not a dry
  run. Aborted, restored to match origin, then measured with
  `git merge-tree` and a throwaway branch instead.

- **2026-07-31** · **Contract inverted, duplication removed** — two
  corrections from reading the published briefing.
  **(1) The briefing was restating the spec.** If the spec is in the
  review anyway, re-elaborating it is padding that buries the parts only
  the briefing can supply. Added a "Don't restate the inputs" rule: the
  value is the **delta between intent and artefact**, so no spec recap,
  no file-by-file diff recitation. Playback narrowed from "what shipped
  (capability + behaviour + decisions)" to *orientation to shape and
  risk* plus only those design decisions **the diff reveals** — a
  decision the spec states and the diff merely implements is reading,
  not briefing. Two new fixture asserts, since the rule was otherwise
  untested prose: the briefing must not reproduce the spec's own section
  headings, and must stay under 200 lines for a 6-line diff.
  **(2) The contract belonged in the wrong skill.** kdevkit was
  specifying what the briefing tool receives, that it runs fresh-context
  and read-only, and that the output is human-facing — all of which is
  the *generator's* business. Inverted: kdevkit now says only that
  dev-loop completion produces a review whose briefing comes from a
  configured generator, that it **consults the generator's declared
  contract** for inputs and invocation and honours it, and how to use
  the result on the PR/CR. kreviewkit gained an explicit **Invocation
  contract** section (inputs required/wanted, how to run, what comes
  back) written as *its own* requirement for a caller to read. Config
  key `reviewer:` → `generator:` to match. Two wins, as the user framed
  it: another briefer with a different input contract can be configured
  without touching kdevkit, and kdevkit stays simple — kdevkit's section
  dropped from ~110 to 65 lines (−40%) while kreviewkit, which is loaded
  only when a briefing is actually wanted, carries the detail.
  Behavioral test re-run: PASS.

- **2026-07-30** · **Three independent review rounds now applied; third
  behavioral run PASS.** A blind §7 Code Review Gate scored **58/100**
  and its findings partly *contradicted* the dogfood briefing — which is
  itself the useful result: the two reviewers have different blind spots,
  so running both was worth it. Verified each high finding before fixing.
  Fixed: **(H1)** `setup.md` never learned `review_brief:`, so the verify
  subagent would have reported drift on the very block this feature added
  — the key schema now lives in `setup.md` (per kdevkit's own skill-file
  placement rule) with SKILL.md pointing at it, and the subagent's
  validation list gained a rule stating that an absent block is *not*
  drift. **(H2)** the wider-branch assert accepted `util.py`, which
  appears in SPEC.md and project.md — both *handed* to the reviewer — so
  it was passable without opening an unchanged file; the seed no longer
  names the file or function anywhere in the handed inputs and the grep
  is `\bslugify\b` alone. **(H3/H4)** kdevkit said the tool "writes
  nothing" while kreviewkit granted a briefing-artefact carve-out, and
  kreviewkit *also* contradicted itself 130 lines later ("you have no
  write authority"); all three now say the same thing. **(M7)** the
  announce marker could have leaked into a published PR body — the skill
  now returns the artefact with no announce line and Output rule 0
  carries the carve-out. **(M9/L2)** the no-verdict assert banned the
  bare word "approved", failing a briefing that says "don't approve
  without resolving this"; now matched as a standalone verdict line.
  **(M10)** `rev-list --count` missed `commit --amend` — reproduced it,
  now a recorded `.base-sha` comparison. Also: Output rule 0 adopted per
  the base branch's convention, description cut 634 → 361 chars (base
  range 172–344), `code-review` tag dropped and an explicit "not a
  scoring gate" line added (a project could otherwise have wired this as
  `code_review.reviewer`, which needs a score it never returns), codex
  added to the fixture's tools, §8-step-5 closure interaction specified,
  and the dev-loop diagram now shows the briefing step.

  **Then a third briefing on the fixed state found one more real defect**
  — the assert I had "tightened" still matched `no test` *inside* "no
  test report", which is boilerplate the skill **guarantees** whenever no
  report is supplied, and the enact run supplies none. Reproduced,
  narrowed, re-verified both directions. Same defect class as H2, one
  round later: **a grep is only as good as what the reviewer was already
  handed.** Behavioral test re-run after each round — three PASSes, the
  last against the strictest assertions.

  **Reversal worth recording:** the blind gate's L1 flagged the three
  `see <url>` pointers as inconsistent with the skill's own no-network
  rule, and I removed them — which silently contradicted the user's
  explicit instruction to keep the reference so a later run can update
  it, and deleted the spec's only mitigation for its own *Reference rot*
  risk. Restored as `src: <host/path>` — provenance preserved, not
  phrased as a fetch. **Lesson: a reviewer finding is not automatically
  right, and a fix that satisfies a reviewer while violating a standing
  user instruction is a regression.**

- **2026-07-30** · **Behavioral test executed — PASS.** Ran the real
  `enact` fixture (not `--dry-run`): `resources/tests/run kreviewkit
  --kind enact --tools claude`. Scoped deliberately to one fixture, one
  kind, one tool — which also avoids the unsafe generated `discovery`
  path (see the harness finding below). A live agent, given the seeded
  scratch repo, satisfied all nine tightened assertions: it read the
  unchanged `src/util.py` and named the duplication (proving
  wider-branch reading), caught the spec's untrimmed-input requirement
  and the absent unit tests, emitted a grouped
  intent/contract/plumbing focus map, left the worktree and commit
  count untouched (read-only), leaked no announce marker into the
  artefact, issued no verdict, and produced labelled judgement items.
  **Containment:** the run needed the skill readable at
  `$HOME/.claude/skills/kreviewkit`, so that one symlink was
  *temporarily* re-pointed at this worktree and **restored
  immediately** after (verified byte-identical to the saved original);
  the primary checkout was never written to and both checkouts are
  clean. Also made the PR/CR publish step **mandatory** in kdevkit —
  a briefing that stops at the terminal has not been delivered.

- **2026-07-30** · Applied the dogfood briefing's findings. Two
  **blocking** defects it found, both verified before fixing rather
  than taken on faith: (1) the skill forbade writing *anything* while
  the fixture required it to write `BRIEFING.md` — a compliant agent
  would have refused and failed the test; fixed by scoping the
  prohibition to "nothing already on the branch" and allowing the
  briefing artefact itself. (2) The generated `discovery` test reuses
  the first `enact` task with **no scratch workdir** —
  `assert_response` takes no workdir parameter at all, so
  `tool_invoke` runs in the runner's cwd (the checkout) with
  permissions skipped; my file-writing task would have been aimed at
  the repo. Pre-existing harness gap (`test-runner-workdir-containment`,
  `test-runner-sandbox-asymmetry`), but this fixture newly pointed it
  at a write task: task rephrased and `/BRIEFING.md` gitignored to
  contain an escape. Sharpest catch: the reads-beyond-the-diff
  assertion grepped for `util`, which appeared as a **diff context
  line** (`from src.util import slugify`) — reproduced and confirmed,
  so a diff-only reviewer could have passed the very assertion meant
  to catch it; removed the import from the seed and narrowed the grep
  to `slugify|util\.py`, tokens absent from the diff. Also added the
  anti-rubber-stamp assertion the spec demanded three times and tested
  nowhere, fixed §9's body contract to acknowledge the new gate,
  moved dispatch to before PR-open, made role resolution ask rather
  than proceed silently on zero-or-many, and cleared three
  doc-staleness items. **Deferred, flagged:** `just test`'s 97 tests
  never read the real `resources/content/` (all use `TempDir`), so
  "green" means "nothing regressed", not "kreviewkit validates" —
  pre-existing coverage shape, backlog item at closure.

- **2026-07-30** · Dev phase. Authored `kreviewkit/SKILL.md` (v1.0.0)
  and the kdevkit §7 Review Briefing hook (v3.6.0 → 3.7.0).
  **Enforcement probe finding:** mechanism 1 (host-level read-only
  tool restriction) is cheap on Claude Code — native
  `--allowedTools` / `--disallowedTools` / `--tools` flags plus a
  `tools:` field in agent definitions — so no sandboxing or
  read-only-checkout machinery is needed there; prose stays the
  portable floor for hosts lacking it. **Registry finding:** confirmed
  no registry or validator change is needed — the validator walks
  skill dirs generically, and `status-skills` reporting
  `.codex/skills/kreviewkit missing` proved the codex `FanOut`
  enumerates the new dir on its own. Gates: `fmt-check` + `lint` +
  `check` + `test` green (97 unit tests); fixture `--dry-run` all
  pass. Note: did **not** re-point the live `$HOME` symlinks at this
  worktree — `status-skills` shows them bound to the primary checkout,
  which is correct and left alone.

- **2026-07-30** · Round-3 revision. User flagged that the References
  were mostly built for a *different audience* — automated reviewers
  addressing the **author**, or standing guidance for **any**
  reviewer — whereas kreviewkit briefs *this human about this change*.
  Added an **Audience** subsection (three-way table + four binding
  consequences: orient don't adjudicate; the human stays the reviewer;
  never submit a review state; read-once-then-set-aside) and a
  `Taken as` column on the References table marking each source
  reviewer-facing vs. author-facing-shape-only. Conventional Comments
  labels explicitly re-pointed to mean *attention warranted* rather
  than *author obligation*. Also added the **dogfood run** as a
  first-class test: this feature's own PR body is produced by
  kreviewkit via a genuine independent dispatch, with the user's
  judgement as the pass condition.

- **2026-07-30** · Round-2 revision. Two corrections from the user.
  (1) The "sealed" model was wrong: hardening isn't about file access.
  A reviewer needs to read the **wider branch**, not just the diff —
  four added lines inside a fifty-line method are only judgeable by
  reading the method. Reframed to **read-only**: reads any file and
  git history on the branch, writes nothing (no edits/commits/pushes/
  network/test-execution), and never sees the implementer's
  conversation. Google's reviewer guide corroborates directly ("open
  the whole file when needed", zoom out to the system). Enforcement is
  now "take whichever mechanism the host makes cheap" (read-only tool
  restriction → read-only checkout → prose floor), with sandboxing
  explicitly out of scope as the hoop-jumping to avoid. (2) Added a
  **References** section — fetch-and-inline the distilled guidance,
  keep the URL for later refresh. Verified six sources; Google
  eng-practices (dimensions + navigation order) and Conventional
  Comments (finding-label grammar) are load-bearing, so the skill
  stands on existing review practice rather than re-deriving it. Two
  fixture rows changed to match the new model (worktree-unchanged
  assert; reads-beyond-the-diff seeded case).

- **2026-07-30** · Revised from Planning Review Gate feedback (PR
  #35, four inline comments). Changes: (1) "plan" → **spec**
  throughout, with an explicit definition of what a spec carries;
  bundle inputs restated as project context + spec + diff + optional
  test run report. (2) Triggers reframed — explicit invocation
  primary, implicit pickup as a must-work fallback for the
  single-tool-installed case. (3) Reviewer isolation hardened to
  **sealed**: no filesystem, repo, network, shell, or implementer
  history; caller packages the bundle; briefing must name gaps rather
  than fill them. (4) kdevkit coupling inverted to **role
  indirection** — kdevkit asks for an "independent review-briefing
  tool", kreviewkit advertises the role, install or project context
  binds them.

- **2026-07-29** · Spec drafted from a grounding pass (kdevkit
  SKILL.md, the code-review-gate + cr-reading-order feature specs,
  the pr-review-tui backlog item, project.md) plus prior-art research
  on Sourcery's Reviewer's Guide and CodeRabbit's walkthrough. Key
  framing: kreviewkit is the *spec-aware, human-facing* complement to
  kdevkit's *blind, machine-facing* §7 Code Review Gate — same
  fresh-context primitive, deliberately inverted on inputs. User
  decisions: standalone skill named `kreviewkit`; briefing IS the
  PR/CR body (single artefact); dispatched at the dev→closure handoff.

## Decision Log

<!-- append: decision · rationale · alternatives rejected -->

- **2026-07-31 · Defects route back to the loop; only judgement calls
  reach the PR.** Rationale (user observation on the published briefing):
  a "needs your judgement" item that is really a defect implies the dev
  loop *isn't* complete — and if the loop isn't complete, why generate a
  briefing at all? The two are different kinds. Dividing test: *would
  fixing it make the finding disappear?* Yes → defect, route back on a
  separate channel and regenerate the briefing on fixed work; no,
  because it's a choice → judgement, publish. The generator now returns
  **two channels** and kdevkit treats defects as the next slice. This
  also makes an empty section 4 legitimate rather than suspicious.
  Alternative rejected: keeping one mixed list (what shipped) — it put
  unfinished work on the review surface and made every briefing read as
  a to-do list.
- **2026-07-31 · kdevkit carries a safety floor a generator cannot
  override.** Rationale (user direction): the contract-consulting design
  means a generator declares how it wants to run, so without a floor a
  misconfigured or malicious briefer could ask for write authority, the
  implementer's history, credentials, or network reach — and
  prompt-injected content in a diff must not be able to widen a
  reviewer's authority. The floor is absolute: a generator's contract
  governs *what it needs to read*, never *what it may do*; demanding any
  of the four means refuse, report, don't run. Alternative rejected:
  trusting the generator's declaration (the shipped version) — fine for
  a tool you wrote, unsafe as a general contract.
- **2026-07-31 · Fixture asserts anchor on seeded specifics, not review
  vocabulary.** Rationale: the missing-tests assertion had been patched
  three rounds running and stayed defeatable, because every fix widened
  or narrowed a *word list* while the skill guarantees coverage language
  whenever no test report is supplied. Root cause was matching
  vocabulary, not evidence. Now two conditions — a coverage phrase **and**
  a token from this seed (`title_to_slug` / `tests/`) that appears in no
  boilerplate and no other assert. Conversely the anti-rubber-stamp
  assert was made deliberately **loose**, because rigour there was
  actively harmful: it failed a *correct* briefing whose labels used
  `**question —**` rather than `question:`. Rigour belongs on the
  specific findings; the loose assert only catches a clean bill of
  health. Lesson recorded: **assert on what the seed uniquely produces,
  and never guess an agent's phrasing — read a real output first.**

- **2026-07-31 · The generator owns the contract; kdevkit only
  orchestrates.** kdevkit states that dev-loop completion produces a
  review whose briefing comes from a configured generator, consults that
  generator's own declared contract for what it needs and how it wants to
  run, supplies it, and uses the returned briefing on the PR/CR. It
  defines *nothing* about briefing content, audience, isolation, or
  inputs. Rationale: those are properties of the briefing tool, not of
  the workflow, and putting them in kdevkit both bloated an always-loaded
  critical skill and hard-wired one tool's contract as if it were
  universal. Inverting gives two things — a different briefer with a
  different input contract can be configured without editing kdevkit,
  and kdevkit stays simple, with the detail living in a skill that loads
  only when a briefing is wanted. Alternative rejected: kdevkit
  specifying the dispatch contract (the shipped version) — simpler to
  write, but it makes every future briefer conform to kreviewkit's shape.
- **2026-07-31 · The briefing must not restate its inputs.** The
  reviewer already has the spec and the diff in the review, so recapping
  either is padding that buries the delta. Playback is now orientation
  plus only the design decisions the diff *reveals*; a decision the spec
  states and the diff merely implements is left to the spec. Enforced by
  fixture asserts (no spec section headings echoed back; length bounded)
  because an untested prose rule drifts. Alternative rejected: keeping
  the fuller playback for readers who skip the spec — it optimises for
  someone not doing the review, at the cost of everyone who is.

- **2026-07-30 · The audience is the human about to review — and the
  references mostly aren't.** Rationale: Sourcery, CodeRabbit and
  kdevkit's own §7 gate are *automated reviewers speaking to the
  author* ("fix this"); Google's guide is *standing advice to a
  reviewer* ("look for this"); kreviewkit briefs *this* human on
  *this* change ("here is what shipped and where your attention is
  worth most"). Left implicit, that gap would pull the skill back into
  being another findings-producing reviewer, which already exists.
  Made explicit as an Audience subsection plus a `Taken as` column on
  the References table marking which sources are reviewer-facing
  (mineable directly) vs. author-facing (shape only, framing dropped).
  Consequences: it orients rather than adjudicates; it must never read
  as "already reviewed"; Conventional Comments labels are re-pointed
  to mean *attention warranted* rather than *author obligation*; it
  writes the PR body but never submits an approve/request-changes
  state — the human owns the verdict.
- **2026-07-30 · Dogfood the skill on its own PR as a first-class
  test.** Rationale: fixtures can assert the contract but not
  usefulness, which is the actual goal. This feature's own briefing is
  produced by a genuine independent dispatch and handed to the user as
  their review briefing; the pass condition is the user's judgement
  that it made reviewing easier, not a score. A briefing that reads
  well but conveys nothing new fails, as does one that induces
  rubber-stamping. Alternative rejected: fixtures only — they'd all
  pass on a briefing no human found useful.

- **2026-07-30 · Reviewer is read-only, not sealed — it reads the
  wider branch and writes nothing.** *Supersedes the "sealed" entry
  below.* Rationale: isolation and context are separate axes, and the
  first draft conflated them. A diff-only reviewer can't answer the
  questions that matter most (does this belong here, does it duplicate
  an existing helper, is this addition sitting in a method that should
  have been split) because those are questions about code the diff
  never shows; Google's reviewer guide says to open the whole file and
  zoom out to the system for exactly this reason. What's actually
  worth enforcing is **no write authority and no inherited
  justification** — cheap to enforce, and it doesn't degrade the read.
  Alternative rejected: the sealed/bundle-only reviewer — trades real
  review quality for a purity that was never the point.
- **2026-07-30 · Enforcement takes whatever mechanism the host makes
  cheap; no sandboxing.** Order of preference: host-level read-only
  tool restriction → a read-only/detached checkout → prose prohibition
  as the floor. Rationale: the property is what matters, not the
  mechanism, and a spec that demands container or sandbox isolation
  would sink the feature under hoops. A host offering none of the
  three still gets a useful briefing with a weaker guarantee recorded
  in the Session Log. Alternative rejected: mandating a hard technical
  boundary — disproportionate for a reviewer that only needs to not
  write.
- **2026-07-30 · Stand on existing review guidance; inline it but keep
  the URL.** Rationale: we are not the first to build this. Google's
  eng-practices supplies the review dimensions and the
  broad→main→rest navigation order; Conventional Comments supplies the
  finding-label grammar; Sourcery and CodeRabbit are the artefact prior
  art. Inlining keeps the skill working offline; the retained URL is
  the provenance marker so a later run can re-fetch and update.
  Alternatives rejected: re-deriving a parallel review checklist
  (wasteful and worse); link-only with no inlining (skill breaks when
  offline); caching/build-time fetch machinery (out of proportion —
  explicitly deferred).
- **2026-07-30 · SUPERSEDED · Reviewer is sealed — no filesystem,
  repo, network, shell, or implementer history.** Kept for the record;
  replaced by the read-only entry above after the user pointed out
  that hardening isn't about file access and a reviewer legitimately
  needs to read the wider branch.
- **2026-07-30 · kdevkit dispatches a role, never the product.**
  Rationale: keeps the two skills independently shippable and lets a
  project swap briefing tools without editing kdevkit; mirrors the
  existing `code_review.reviewer` indirection, so there's one pattern
  rather than a new kind of coupling. Binding is by install (single
  installed tool wins) or by the project's own context file.
  Alternative rejected: kdevkit naming kreviewkit — hard-couples a
  critical skill to a specific product and breaks the standalone
  framing.
- **2026-07-30 · Explicit invocation primary, implicit pickup a
  must-work fallback.** Rationale: explicit stays unambiguous once
  several review-ish tools are installed, but a user who installed
  exactly one shouldn't need to know its name — so `description:`
  leads with user-phrased triggers *and* advertises the role.
  Alternative rejected: explicit-only — fails the common
  single-tool-installed case and regresses discoverability.
- **2026-07-30 · Test run report is an optional bundle input.**
  Rationale: it materially improves the V-model coverage read (what
  actually ran and passed, vs. what merely exists in the diff), but
  requiring it would block the standalone path where no run exists.
  Absent → the briefing must state coverage is unverified rather than
  imply it checked. Alternative rejected: required (breaks standalone
  PR review); omitted entirely (leaves coverage claims weaker than
  they need to be).
- **2026-07-30 · "Spec", not "plan".** Rationale: the artefact is
  widely called a spec, and it carries more than a plan — capability,
  test expectations, design in project context, *and* the
  implementation plan. Naming it "plan" understated what the reviewer
  reconciles against.
- **2026-07-29 · Standalone skill, not a kdevkit section.** Reusable
  beyond kdevkit (any spec + diff); keeps kdevkit's always-on context
  lean. Alternative rejected: fold into §8 closure — couples the
  briefing to the closure phase and kills standalone use.
- **2026-07-29 · Briefing IS the PR/CR body (single artefact).**
  Durable why/risk stays at the top of the review
  (Sourcery/CodeRabbit precedent); nothing to keep in sync.
  Alternatives rejected: a durable `review-brief.md` + lean linking
  body (two artefacts drift); Sourcery's description-summary +
  guide-comment split (a second artefact for no gain at this scale).
- **2026-07-29 · Given the spec, unlike §7's gate.** Independence is
  a property of *who* reviews and how little it can reach, not of
  withholding the spec. Reconciling spec-vs-diff and replaying
  decisions is the job, so withholding it (as §7 does to avoid bias)
  would defeat the purpose. Alternative rejected: mirror §7 and
  withhold the spec — leaves the briefing unable to reconcile.
- **2026-07-29 · kdevkit hook is opt-in (`enabled: false` default).**
  A critical skill; the integration must not change existing kdevkit
  behaviour for repos that don't want it. Alternative rejected:
  always-on dispatch — disruptive and forces API cost on every dev
  loop.
- **2026-07-29 · Reuse `code_review`'s reviewer-ref grammar.** One
  reviewer-reference grammar across both kdevkit reviewer configs.
  Alternative rejected: a second bespoke ref syntax.
