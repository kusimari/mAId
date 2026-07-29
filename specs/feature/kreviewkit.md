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
**human-consumable review briefing** for a completed change: an
independent agent is handed *the plan* and *the diff*, and writes
a report that plays the feature back, reconciles what was planned
against what was built, and points the human at what most needs
their judgement. The briefing **is** the pull-request / code-review
body — the skill's output is what opens (or updates) the PR/CR
alongside the diff.

It pairs with `kdevkit` — dispatched at the **dev → closure
handoff** so the human has a real briefing to review before they
give the closure cue — but stands on its own: point it at any plan
and diff and it writes the briefing, no kdevkit spec tree required.

It is the **complement** to kdevkit's §7 Code Review Gate, not a
duplicate of it. That gate is a *blind, machine-facing* reviewer,
deliberately denied the feature spec so it judges the diff against
the project's invariants without bias, and it emits a pass/fail
score. kreviewkit is *plan-aware and human-facing*: it is given the
plan on purpose, because reconciling plan-vs-diff and replaying the
design decisions is the whole point, and it emits prose a human
reads. Between them they close both axes kdevkit §9 names — the
gate checks *diff-vs-project*; kreviewkit checks *diff-vs-plan* and
hands the human a map.

## Requirements

<!-- The experience layer — the cues the agent recognises and the
     artefacts it produces. Skill "experience" = triggers + output,
     not internal dispatch mechanics (those are Design). -->

### Trigger experience

- **Fires on a review-briefing cue.** Bare-task triggers the skill
  must self-recognise: *"brief the review"*, *"write the review
  brief"*, *"prep this for review"*, *"review what was done"*,
  *"open the PR/CR for this"*, *"summarise this change for a
  reviewer"*. The description leads with these so the skill
  self-triggers from a bare task (per project.md's "triggers belong
  in description" rule).
- **Works with or without kdevkit.** Given a plan (a kdevkit
  feature spec, a linked issue, or a pasted intent) and a diff, it
  writes the briefing. When no explicit plan exists, it says so and
  reconstructs intent from commits / PR description rather than
  silently pretending there was a plan.
- **Announces in the response, not in the artefact.** The agent's
  chat reply that runs this skill opens with the literal line
  `[kreviewkit] applies` (the self-announce contract that makes
  activation/discovery testable — see project.md Testing). The
  **briefing artefact itself stays clean** — no marker line leaks
  into the PR/CR body.

### The briefing (the artefact a human reads)

The output is one report with four sections, in this order. It is
written as the PR/CR body.

1. **Playback — the feature as experience.** What the user can now
   do, the salient user-observable behaviour, and the load-bearing
   design decisions *with the alternatives that were weighed*. A
   reviewer who reads only this section understands what shipped and
   why it is shaped the way it is.
2. **Plan ↔ diff reconciliation.** What the plan promised vs. what
   the diff delivers — unmet requirements, scope that crept in,
   silent plan amendments, unrelated changes bundled in (the §9
   spec-discipline anti-patterns, surfaced *after the fact*). Includes
   a **V-model coverage read**: do functional/integration tests
   actually map onto the requirements, and unit tests onto the design
   primitives, the plan claimed? Gaps are named.
3. **Where to focus.** A risk-ranked, *why*-annotated reading map
   using kdevkit's existing vocabulary — *Read for intent / Read for
   contract / Read for plumbing* — that tells the reviewer where to
   spend attention and where they can skim. Sequence or flow diagrams
   appear **only where control flow is non-trivial**, never as
   decoration.
4. **Needs your judgement.** The short list of calls only a human
   can ratify: risky areas, decisions the reviewer found contestable,
   and anything the automated gates structurally could not verify.

- **Independent by construction.** The briefing is written by a
  fresh agent that is **not** the one that produced the code, so the
  read is not self-justifying. Independence is a property of *who
  reviews* (a fresh context), not of *what it sees* — unlike the §7
  gate, kreviewkit is given the plan on purpose.
- **Honest, not celebratory.** The briefing surfaces gaps, risks,
  and unmet plan items plainly; it is a reviewer's aid, not a
  marketing summary. A briefing that finds nothing to focus on for a
  non-trivial diff is itself a smell.

### kdevkit integration experience

- **Dispatched at the dev → closure handoff.** When kdevkit is
  driving, after the §7 Agent-dev Review Gate (push done) and before
  the §8 closure cue, kdevkit dispatches kreviewkit; its briefing
  becomes the PR/CR body the human reviews before saying "close it".
- **Opt-in, non-disruptive.** The hook is additive and configurable
  from `project.md`'s `## Agent Development > kdevkit` block. A repo
  that does not enable it sees kdevkit behave exactly as today.
- **Maps onto kdevkit's §9 body contract.** kreviewkit's four
  sections subsume kdevkit's §9 body shape (Why + phase content +
  Reading order): Playback carries the Why/Approach, Where-to-focus
  *is* the Reading order, and the two new sections (reconciliation,
  needs-judgement) enrich it. No conflicting body shapes.

## Test Strategy

<!-- Per project.md's two-layer surface + the five kinds of skill
     test. Agentic runs stop at `just test`; functional (API-cost)
     is user-driven. -->

### Unit (`just test`, load-bearing — §7 Test Gate default)

- The build-tool content validator must accept the new
  `resources/content/skills/kreviewkit/SKILL.md` (valid frontmatter:
  `name`, `description`, `version`). Existing validator/symlink
  tests stay green. No new Rust logic expected — a new skill is
  data, covered by the existing validator path; confirm, don't add
  overhead.

### Smoke / structural (after install)

- `just resources::status-skills` confirms the `kreviewkit` symlink
  resolves through the registry to all three tools. The registry
  symlinks the whole `resources/content/skills/` dir, so a new
  folder is covered without a registry edit — **verify the codex
  `FanOut` path picks up the new dir** (the one place a per-skill
  fan-out could miss it).

### Functional (judge mode; user-driven per project convention)

Fixtures under `resources/tests/skills/`. `activation` + `discovery`
come free from the self-announce contract + `description:`. New
fixtures:

| Behaviour claim | Fixture | Kind |
|---|---|---|
| Loaded when named, announces `[kreviewkit] applies` | generated | activation |
| Bare "brief the review" task triggers the skill unaided | generated | discovery |
| States its own contract: four sections, independent-but-plan-aware, briefing = PR/CR body | `kreviewkit.smoke` `--- playback ---` | playback |
| Given a plan + diff, produces the four-section briefing with a risk-ranked reading map | `kreviewkit.smoke` `--- enact ---` | enact |
| Catches plan↔diff drift — scope creep / unmet requirement / missing test coverage surfaces in section 2 | `kreviewkit.smoke` `--- enact ---` (seeded drift) | enact |
| kdevkit dispatches kreviewkit at dev→closure when enabled | `kdevkit-dev-loop.smoke` (extended) | functional |

- **Wrong-answer cues** inline in each `expect:` narrative: acting as
  the implementer instead of an independent reviewer; rubber-stamping
  (no focus items on a non-trivial diff); leaking the `[kreviewkit]
  applies` marker into the briefing artefact; producing a flat file
  list instead of a risk-ranked intent/contract/plumbing map;
  treating the plan as ground truth rather than a claim to reconcile
  against the diff.
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

The briefing is useful anywhere a plan and a diff exist — reviewing
a colleague's PR, a change made outside kdevkit, a hotfix. Folding
it into kdevkit's §8 would couple it to the closure phase and make
it unusable standalone. A separate skill (the `browser` / `notes`
precedent) keeps it reusable and keeps kdevkit's always-on context
lean. The `k*kit` name signals the "pairs with kdevkit, stands
alone" relationship.

### The dispatch contract (mirror of §7, inverted on inputs)

kreviewkit reuses kdevkit's **fresh-context agent call** primitive
(the same one §2 verify and the §7 gate use). The reviewer is a
fresh agent — never the implementer — so the read is independent.
What it receives is the deliberate inverse of the §7 gate:

Receives:
- ✅ **The plan** — a kdevkit feature spec, or a user-supplied plan
  / intent / linked issue. This is what §7 withholds; kreviewkit
  needs it to play back decisions and reconcile.
- ✅ **The diff vs. base.**
- ✅ **`project.md`** (project invariants — architecture, hard
  constraints, public-repo signal).
- ✅ **Decision / Session logs** where a kdevkit spec carries them
  (the "alternatives weighed" that Playback replays).

Excluded:
- ❌ **The implementing agent's live conversation history** — the
  briefing must read the diff and plan as artefacts, not inherit the
  implementer's running justification.

The contract is described abstractly in the skill prose (portable
across Claude Code / Kiro / Codex), matching how §7 stays
host-agnostic. No host-specific incantations in the body unless an
empirical check proves the abstract contract unactionable (the same
escape-hatch pattern §7 used).

### Briefing generation

The four sections map to sources:
- **Playback** ← diff + Decision Log (decisions & alternatives) +
  Requirements (the experience).
- **Reconciliation** ← plan Requirements/Implementation-Plan vs.
  diff; §9 anti-pattern checklist applied retrospectively; V-model
  coverage = cross-check test files in the diff against
  Requirements (functional/integration) and Design (unit).
- **Where to focus** ← risk read of the diff, bucketed into kdevkit's
  Read-for-intent / -contract / -plumbing groups; diagrams gated on
  non-trivial control flow.
- **Needs your judgement** ← residue: contestable decisions, gate
  blind spots, high-risk surfaces.

### Output binding — the briefing is the PR/CR body

The skill writes the briefing directly as the PR/CR description when
it opens or updates the request. Rationale (Sourcery/CodeRabbit prior
art): the durable why/risk framing belongs at the top of the review
where it stays visible as the conversation grows. Single artefact,
no separate file to drift (chosen over a `review-brief.md` + link,
and over Sourcery's description/comment split — both add a second
artefact to keep in sync for no gain at this scale).

### File / surface

- **New:** `resources/content/skills/kreviewkit/SKILL.md` — the
  skill. Single-file to start (browser/notes shape), ~150–250 lines.
- **New:** `resources/tests/skills/kreviewkit.smoke` — fixtures.
- **Edit:** `resources/content/skills/kdevkit/SKILL.md` — a minimal,
  additive dispatch hook at the §7→§8 handoff, gated on a
  `project.md` config key. Kept small; kdevkit is critical.
- **Edit:** `specs/project.md` — document the kdevkit
  `kreviewkit`-dispatch config key under `## Agent Development`, and
  (closure-time) any Layout/Testing bump for the new skill.
- **No registry edit** — the whole skills dir is symlinked; a new
  folder is covered. (Verify codex `FanOut`.)

### Config shape (project.md `## Agent Development > kdevkit`)

```yaml
review_brief:
  enabled: true            # default: false — opt-in, non-disruptive
  reviewer: host-native    # reuse code_review reviewer-ref syntax
```

Reuses the `<ref>` syntax kdevkit's `code_review` block already
defines (`host-native` / `skill:` / `mcp:` / `agent:`), so there is
one reviewer-reference grammar, not two.

## Implementation Plan

<!-- One slice per item. Three-phase per kdevkit. -->

### Planning phase

- [ ] Land this spec as `plan(kreviewkit): initial spec`; push; open
  the Planning Review Gate; wait for the planning → dev cue.

### Dev phase

- [ ] **kreviewkit SKILL.md.** Author the skill: description
  (triggers-first), self-announce contract, the four-section briefing
  contract, the independent-but-plan-aware dispatch contract, the
  standalone-vs-kdevkit modes, briefing-as-PR/CR-body binding.
- [ ] **Confirm no registry/validator change needed.** Run
  `just test`; `just resources::install-skills` + `status-skills`;
  verify the codex `FanOut` picks up the new dir. If the fan-out
  misses it, that's a real registry slice — surface it.
- [ ] **kdevkit dispatch hook.** Additive §7→§8 handoff hook +
  `review_brief:` config resolution. Minimal, opt-in.
- [ ] **project.md docs.** Document `review_brief:` under
  `## Agent Development > kdevkit`; declare mAId's own setting
  (dogfood).
- [ ] **Fixtures.** `kreviewkit.smoke` (playback + enact incl. a
  seeded plan↔diff drift case) + extend `kdevkit-dev-loop.smoke` for
  the dispatch. `--dry-run` before any paid run.
- [ ] **Quality + Test + Code Review + Push** for the branch; open /
  update the Agent-dev Review Gate. (Self-applicable: this feature's
  own PR/CR body should itself be a kreviewkit briefing.)

### Closure phase

- [ ] Reconcile markers; soft `project.md` verify (Layout/Testing/
  Agent-Development touched); backlog cleanup ask; `close(...)`
  commits; Closure Review Gate; squash-merge; branch cleanup.

### Risk notes

- *Stacked base.* Branched off `feat/fixtures-discovery-vs-content-split`,
  not `main`. If that branch changes or lands first, rebase this one
  (§10 cross-stream rebase mechanics) before closure. Confirm the
  intended merge order at closure.
- *Editing kdevkit.* The dispatch hook touches a critical, well-tested
  skill. Keep it additive and behind the `enabled: false` default;
  re-read `kdevkit-dev-loop.smoke` after the edit so the existing
  narrative doesn't go stale.
- *Overlap with §7 naming.* "Code Review Gate" (§7, blind, scored)
  vs. kreviewkit (plan-aware briefing) must stay clearly distinct in
  prose and fixtures — same watch-item the code-review-gate feature
  flagged for "Review Gates" vs "Code Review Gate".
- *Self-applicability as evidence.* This feature's own CR body being a
  kreviewkit briefing is the strongest proof the contract is
  implementable — but only if written by a genuinely fresh dispatch,
  not hand-authored by the implementing session.
- *Prior-art alignment.* Section shape deliberately tracks Sourcery's
  Reviewer's Guide (file-level why-map + verification path) and
  CodeRabbit's walkthrough (orientation-before-you-open-a-file,
  effort/risk signal). Diverges only in the explicit plan↔diff
  reconciliation, which those tools lack because they have no plan.

## Session Log

<!-- append: date · what was done · decisions made -->

- **2026-07-29** · Spec drafted from a grounding pass (kdevkit
  SKILL.md, the code-review-gate + cr-reading-order feature specs,
  the pr-review-tui backlog item, project.md) plus prior-art research
  on Sourcery's Reviewer's Guide and CodeRabbit's walkthrough. Key
  framing: kreviewkit is the *plan-aware, human-facing* complement to
  kdevkit's *blind, machine-facing* §7 Code Review Gate — same
  fresh-context primitive, deliberately inverted on inputs (given the
  plan, not denied it). User decisions: standalone skill named
  `kreviewkit`; briefing IS the PR/CR body (single artefact); kdevkit
  dispatches it at the dev→closure handoff.

## Decision Log

<!-- append: decision · rationale · alternatives rejected -->

- **2026-07-29 · Standalone skill, not a kdevkit section.** Reusable
  beyond kdevkit (any plan + diff); keeps kdevkit's always-on context
  lean. Alternative rejected: fold into §8 closure — couples the
  briefing to the closure phase and kills standalone use.
- **2026-07-29 · Briefing IS the PR/CR body (single artefact).**
  Durable why/risk stays at the top of the review (Sourcery/CodeRabbit
  precedent); nothing to keep in sync. Alternatives rejected: a
  durable `review-brief.md` + lean linking body (two artefacts drift);
  Sourcery's description-summary + guide-comment split (a second
  artefact for no gain at this scale).
- **2026-07-29 · Given the plan, unlike §7's gate.** Independence is a
  property of *who* reviews (a fresh, non-implementer agent), not of
  *what* it sees. Reconciling plan-vs-diff and replaying decisions is
  the job, so withholding the plan (as §7 does to avoid bias) would
  defeat the purpose. Alternative rejected: mirror §7 and withhold the
  spec — leaves the briefing unable to reconcile or replay decisions.
- **2026-07-29 · kdevkit hook is opt-in (`enabled: false` default).**
  A critical skill; the integration must not change existing kdevkit
  behaviour for repos that don't want it. Alternative rejected:
  always-on dispatch — disruptive and forces API cost on every dev
  loop.
- **2026-07-29 · Reuse `code_review`'s reviewer-ref syntax.** One
  reviewer-reference grammar across both kdevkit reviewer configs.
  Alternative rejected: a second bespoke ref syntax.
