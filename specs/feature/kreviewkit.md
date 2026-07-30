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
sealed, independent reviewer is handed a **bundle** — project
context, the feature **spec**, the diff, and (where available) the
test run report — and writes a report that plays the feature back,
reconciles what the spec called for against what was built, and
points the human at what most needs their judgement. The briefing
**is** the pull-request / code-review body.

**Sealed** is the load-bearing property: the reviewer gets *nothing*
beyond the bundle it is handed — no filesystem, no repo, no network,
no conversation history. This is deliberately the position of a human
reviewer who receives only a PR. It cannot go and look something up,
so what it sees is exactly what it was given, and its read is
reproducible from the bundle alone.

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

- **Independent and sealed.** The briefing is written by a fresh
  reviewer that is **not** the agent that produced the code, and that
  reviewer sees only its bundle — no filesystem, no repo, no network,
  no implementer conversation. Independence is a property of *who*
  reviews and *how little* it can reach; it is not achieved by
  withholding the spec (which §7's gate does for a different reason).
- **Honest, not celebratory.** The briefing surfaces gaps, risks, and
  unmet spec items plainly; it is a reviewer's aid, not a marketing
  summary. A briefing that finds nothing to focus on for a
  non-trivial diff is itself a smell.
- **Says what it was not given.** Missing spec, missing test report,
  or a diff it could not fully interpret are stated in the briefing.
  Because it cannot go looking, naming the gap is the honest move.

### Role-based integration experience (no hard coupling)

- **The calling workflow asks for a role, not a product.** kdevkit
  says "dispatch an independent review-briefing tool to produce a
  briefing for human consumption"; it must not name kreviewkit. Any
  tool that fills the role and honours the bundle contract can serve.
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
| States its own contract: four sections, sealed-but-spec-aware, briefing = PR/CR body | `kreviewkit.smoke` `--- playback ---` | playback |
| Refuses to reach beyond the bundle — names the gap instead of reading the filesystem | `kreviewkit.smoke` `--- playback ---` | playback |
| Given a spec + diff, produces the four-section briefing with a risk-ranked reading map | `kreviewkit.smoke` `--- enact ---` | enact |
| Catches spec↔diff drift — scope creep / unmet requirement / missing test coverage surfaces in section 2 | `kreviewkit.smoke` `--- enact ---` (seeded drift) | enact |
| kdevkit dispatches *a role*, not kreviewkit by name, at dev→closure | `kdevkit-dev-loop.smoke` (extended) | functional |

- The sealed-reviewer claim is verified two ways: a **playback**
  fixture for the stated contract, and — where the host can restrict
  tools — a behavioral `enact` run whose bundle references a file
  that exists on disk but is *not* in the bundle; a compliant
  briefing names the gap instead of quoting the file's contents.
- **Wrong-answer cues** inline in each `expect:` narrative: acting as
  the implementer instead of an independent reviewer; reading the
  repo/filesystem instead of working from the bundle; rubber-stamping
  (no focus items on a non-trivial diff); leaking `[kreviewkit]
  applies` into the briefing artefact; a flat file list instead of a
  risk-ranked intent/contract/plumbing map; treating the spec as
  ground truth rather than a claim to reconcile against the diff;
  kdevkit naming kreviewkit directly instead of asking for the role.
- `tools: claude,kiro` for the judge fixtures (cross-tool evidence
  for a new skill), `claude` default elsewhere.
- Prefer behavioral (`--- setup ---`/`--- assert ---`) where the
  briefing lands as an inspectable file; fall back to judge narrative
  for the irreducibly-prose independence/tone claims.

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

### The bundle contract (sealed reviewer)

kreviewkit reuses kdevkit's **fresh-context agent call** primitive
(the same one §2 verify and the §7 gate use), with two differences:
the inputs are inverted (it *is* given the spec), and the reviewer is
**sealed**.

The **caller packages the bundle**; the reviewer only reads it. That
split is what makes sealing enforceable — the reviewer never needs a
path, so it never needs the filesystem.

Bundle contents (passed as content, not as paths):

- ✅ **Project context** — `project.md` and a repo-root `AGENTS.md`
  where one exists; whatever equivalent plays that part when kdevkit
  isn't in use.
- ✅ **The feature spec** — the full statement (capability, test
  expectations, design-in-project-context, implementation plan). This
  is what §7's gate withholds and kreviewkit needs.
- ✅ **The diff vs. base.**
- ✅ **Decision / Session logs** where the spec carries them (the
  "alternatives weighed" that Playback replays).
- ⚪ **Test run report, where available** (optional). Lets section 2
  read coverage against what actually ran and passed rather than
  merely what exists in the diff. Absent → the briefing states
  coverage is unverified. Optional rather than required because not
  every invocation has one (a standalone PR review often won't), and
  a hard requirement would block the standalone path.

Explicitly **out of the bundle** — the reviewer has no:

- ❌ **Filesystem or repo access.** It cannot open a file, walk the
  tree, or check out the branch.
- ❌ **Network access.** No fetching issues, docs, or CI results.
- ❌ **Shell / build / test execution.** It does not run the suite;
  it reads the report if given one.
- ❌ **Implementer conversation history.** No inherited
  justification.

Enforcement, best-effort by host: **prefer host-level tool
restriction** (dispatch the reviewer with an empty or read-nothing
toolset) and fall back to an explicit prohibition in the skill prose
where a host cannot restrict tools. The prose states the rule
unconditionally so a compliant agent honours it either way, and the
briefing is required to *name gaps* rather than fill them — which is
the observable signature of a sealed reviewer.

Rationale: this is the human-reviewer analogy taken literally. A
reviewer who can wander the repo can silently repair a thin bundle,
which hides exactly the problem worth surfacing (an unreviewable
change), and makes the read non-reproducible. Sealing turns "the
briefing was thin" into evidence about the change rather than noise
about the reviewer.

The contract is described abstractly in the skill prose (portable
across Claude Code / Kiro / Codex), matching how §7 stays
host-agnostic. No host-specific incantations in the body unless an
empirical check proves the abstract contract unactionable (the same
escape-hatch pattern §7 used).

### Briefing generation

The four sections map to bundle sources:

- **Playback** ← diff + Decision Log (decisions & alternatives) +
  the spec's capability/experience statement.
- **Reconciliation** ← spec requirements + implementation plan vs.
  diff; §9 anti-pattern checklist applied retrospectively; V-model
  coverage = declared test expectations cross-checked against test
  changes in the diff and, where supplied, the test run report.
- **Where to focus** ← risk read of the diff, bucketed into kdevkit's
  Read-for-intent / -contract / -plumbing groups; diagrams gated on
  non-trivial control flow.
- **Needs your judgement** ← residue: contestable decisions, gate
  blind spots, high-risk surfaces, and anything the bundle left
  unverifiable.

### Output binding — the briefing is the PR/CR body

The reviewer *returns* the briefing; the **caller** writes it as the
PR/CR description (the reviewer can't — it has no network or fs).
Rationale (Sourcery/CodeRabbit prior art): the durable why/risk
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
- [ ] Revise per Planning Review Gate feedback: spec terminology,
  bundle inputs + optional test report, explicit-primary /
  implicit-fallback triggers, sealed reviewer, role indirection.
  Push; update the PR body; wait for the planning → dev cue.

### Dev phase

- [ ] **kreviewkit SKILL.md.** Author the skill: description
  (user-phrased triggers first, role advertised), self-announce
  contract, the four-section briefing contract, the sealed-bundle
  contract, standalone-vs-kdevkit modes, briefing-as-PR/CR-body
  binding.
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
- [ ] **Fixtures.** `kreviewkit.smoke` (playback incl. the
  sealed-reviewer claim + enact incl. a seeded spec↔diff drift case)
  and extend `kdevkit-dev-loop.smoke` for role-based dispatch.
  `--dry-run` before any paid run.
- [ ] **Quality + Test + Code Review + Push** for the branch; open /
  update the Agent-dev Review Gate. (Self-applicable: this feature's
  own PR body should itself be a kreviewkit briefing, produced by a
  genuine sealed dispatch.)

### Closure phase

- [ ] Reconcile markers; soft `project.md` verify (Layout/Testing/
  Agent-Development touched); backlog cleanup ask; `close(...)`
  commits; Closure Review Gate; squash-merge; branch cleanup.

### Risk notes

- *Stacked base.* Branched off
  `feat/fixtures-discovery-vs-content-split`, not `main`. If that
  branch changes or lands first, rebase this one (§10 cross-stream
  rebase mechanics) before closure. Confirm merge order at closure.
- *Sealing is only as good as the host.* Where a host can't restrict
  the reviewer's toolset, sealing rests on prose compliance. The
  bundle-references-an-unbundled-file fixture is the check that
  catches a leaky reviewer; treat a failure there as a real finding,
  not a fixture bug.
- *Bundle packaging is now load-bearing.* Because the reviewer can't
  fetch anything, a caller that packages a thin bundle produces a
  thin briefing. The briefing naming its gaps is the mitigation —
  verify that behaviour explicitly rather than assuming it.
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

- **2026-07-30 · Reviewer is sealed — no filesystem, repo, network,
  shell, or implementer history.** Rationale: the human-reviewer
  analogy taken literally (a reviewer receives a PR, not a machine).
  A reviewer that can wander the repo silently repairs a thin bundle,
  which hides the very problem worth surfacing and makes the read
  non-reproducible. Sealing converts "the briefing was thin" into
  evidence about the change. Enforcement prefers host-level tool
  restriction, falling back to prose prohibition; the observable
  signature is a briefing that *names* gaps. Alternative rejected: a
  fresh-context-but-tool-enabled reviewer — convenient, but lets the
  reviewer's own digging substitute for a reviewable change.
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
