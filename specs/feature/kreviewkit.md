# Feature: kreviewkit

## Git Setup

- Branch: `feat/kreviewkit` (pre-created worktree).
- Base: `feat/fixtures-discovery-vs-content-split` (`588b06c`) —
  stacked on an in-flight feature branch at user direction, not
  `main`. Closure merge order depends on the base branch landing
  first (stream-like ordering; see Risk notes).

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

1. **Playback — the feature as experience.** What the user can now
   do, the salient user-observable behaviour, and the load-bearing
   design decisions *with the alternatives that were weighed*. A
   reviewer who reads only this section understands what shipped and
   why it is shaped the way it is.
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
4. **Needs your judgement.** The short list of calls only a human
   can ratify: risky areas, decisions the reviewer found contestable,
   and anything the automated gates structurally could not verify.

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

1. The project's own context names the tool (`review_brief.reviewer`
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
  reviewer: <ref>          # optional; omit to auto-resolve the
                           # single installed review-briefing tool
```

`<ref>` reuses the grammar kdevkit's `code_review` block already
defines (`host-native` / `skill:<name>` / `mcp:<server>.<tool>` /
`agent:<name>`), so there is one reviewer-reference grammar, not two.
Omitting `reviewer:` is the common case: resolution falls to the
installed-role lookup above.

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

- [ ] **Probe the read-only enforcement options** before authoring, so
  the prose commits to something real: check what each host makes
  cheap (read-only tool restriction / detached read-only worktree /
  prose-only). Record the finding in the Session Log; pick the
  cheapest workable mechanism per host and let prose be the floor.
- [ ] **kreviewkit SKILL.md.** Author the skill: description
  (user-phrased triggers first, role advertised), self-announce
  contract, the four-section briefing contract, the read-only reviewer
  contract, standalone-vs-kdevkit modes, briefing-as-PR/CR-body
  binding, and the inlined reference guidance with `see <url>`
  pointers.
- [ ] **Confirm no registry/validator change needed.** `just test`;
  `just resources::install-skills` + `status-skills`; verify the
  codex `FanOut` picks up the new dir. If the fan-out misses it,
  that's a real registry slice — surface it.
- [ ] **kdevkit role-based dispatch hook.** Additive §7→§8 hook +
  `review_brief:` resolution (named ref → single installed role →
  ask once and persist). Names the role, not kreviewkit.
- [ ] **project.md docs.** Document `review_brief:` under
  `## Agent Development > kdevkit`; declare mAId's own setting
  (dogfood).
- [ ] **Fixtures.** `kreviewkit.smoke` — playback incl. the
  read-only isolation claim; enact incl. a seeded spec↔diff drift
  case, a reads-beyond-the-diff case, and the worktree-unchanged
  assert — and extend `kdevkit-dev-loop.smoke` for role-based
  dispatch. `--dry-run` before any paid run.
- [ ] **Dogfood run.** Dispatch kreviewkit independently on this
  branch and use the returned briefing as this PR's body (see Test
  Strategy → Dogfood run). Hand it to the user as their review
  briefing; fold any findings back as dev-loop slices.
- [ ] **Quality + Test + Code Review + Push** for the branch; open /
  update the Agent-dev Review Gate.

### Closure phase

- [ ] Reconcile markers; soft `project.md` verify (Layout/Testing/
  Agent-Development touched); backlog cleanup ask; `close(...)`
  commits; Closure Review Gate; squash-merge; branch cleanup.

### Risk notes

- *Stacked base.* Branched off
  `feat/fixtures-discovery-vs-content-split`, not `main`. If that
  branch changes or lands first, rebase this one (§10 cross-stream
  rebase mechanics) before closure. Confirm merge order at closure.
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
