# Feature: kdevkit-code-review-gate

## Git Setup

- Branch: `feat/kdevkit-code-review-gate`
- Base: `main` (currently at `c1371cf`)

## Feature Brief

Two related kdevkit-skill tightenings, shipped on one branch:

1. **Code Review Gate.** Replace the dev-loop's self-introspected
   "0–100 self-review score" with a real code review run by a
   separate agent in clean context. The reviewer is dispatched
   between the Test Gate and the Push Gate; sub-threshold findings
   loop back to the start of the dev loop so the agent
   fixes-and-retries the way a human would after a human reviewer's
   comments. The reviewer itself is **not** a new mAId-shipped
   artefact — kdevkit defaults to whatever code review the host
   coding agent ships natively (Claude Code's built-in, Kiro's
   equivalent, Codex's CLI, etc.). Projects that want
   project-specific review semantics declare a reference in
   `project.md`'s `## Agent Development > kdevkit` block; the
   reference may point to a skill, an MCP tool, or a named project
   agent. Threshold + authority (hard-stop vs. soft) are
   project-configurable; the gate's *semantics* are kdevkit's, the
   *syntax* of who reviews and how strictly is the project's.

2. **Planning-phase ordering reinforcement.** Tighten §6's
   "Plan-commit rule" so it enumerates the same commit → push →
   open Planning Review Gate → THEN wait-for-cue order that §3
   already spells out for the spec-already-drafted entry path.
   Today, §6 only says "ship through the Planning Review Gate
   before any code work" — which an agent fresh out of the four
   interviews can read as "ask the user to confirm the spec, then
   commit." The "common ordering mistake" warning lives only in
   §3, so the rule fires only on one of the two entry paths.
   Bundled here because it surfaced during this very planning
   session — the planning agent (me) inverted commit and review.

## Requirements

### Code Review Gate

- **No new mAId-shipped reviewer skill.** Default reviewer is the
  host coding agent's native code-review capability (e.g. Claude
  Code's `/code-review`, Kiro's equivalent). kdevkit dispatches to
  it; the host owns the prose.
- **Configurable per-project reviewer.** `project.md`'s
  `## Agent Development > kdevkit > code_review:` block may name a
  project-specific reviewer (skill / MCP tool / project agent).
  When declared, kdevkit dispatches to it instead of the host
  default.
- **Code Review Gate inserts between Test and Push.** New §7
  ordering: `Quality → Test → Code Review → Push → Agent-dev
  Review`. Tests pass first so the reviewer judges a green diff;
  the reviewer catches practices the deterministic gates can't.
- **Clean-context dispatch.** Reviewer runs in a fresh agent call
  — no feature spec, no session log, no in-progress conversation
  history. Reviewer receives `project.md` + the diff vs. base +
  reviewer-specific config. Feature context is deliberately
  excluded so the review isn't biased.
- **Reuse the score/threshold mechanic.** Reviewer returns
  findings + a 0–100 score. Score ≥ threshold → Push Gate.
  Score < threshold → loop back to start of Quality Gate.
  Default threshold: **70** (matches the old self-review default).
- **Loop-back is full-loop.** Sub-threshold restarts at Quality —
  same iteration counter / retry budget mental model as Test
  Gate's fix-and-retry. Default review-loop budget: **2**
  fix-and-retry cycles. After budget exhausted, stop and report
  per §7's existing "refuse-on-fail" semantics.
- **Authority is configurable.** `authority: hard-stop` enforces
  the budget — no Push if reviewer findings remain critical.
  `authority: soft` allows a final iteration with residuals
  noted in Session Log (matches today's permissive behavior for
  projects that prefer it).
- **Old self-review-score retired.** Quality Gate becomes purely
  deterministic — fmt + lint + typecheck. Anything subjective
  moves to the new Code Review Gate.
- **Lightweight setup UX.** When no `code_review:` block exists in
  `project.md` at feature-start, offer a one-line prompt: default
  to host-native, or paste a reference. The choice is written to
  `project.md` so the question doesn't re-fire next session
  (sticky default).
- **Public-repo hygiene unchanged.** §9 internal-marker grep
  still fires at all gates; no new public-repo surface introduced.
- **Strong llm-as-judge functional tests.** Two new judge
  fixtures + extended `kdevkit-dev-loop.smoke`, both
  `tools: claude,kiro`. The skill is critical → cross-tool
  evidence is non-negotiable.

### Planning-phase ordering reinforcement

- **§6 Plan-commit rule reads as a numbered sequence.** Mirror
  §3's existing enumeration so the order is impossible to miss:
  *(1) finish interviews and write the spec → (2) confirm
  readiness with the user → (3) commit as `plan(<feature>):
  initial spec` → (4) push the feature branch → (5) open the
  Planning Review Gate → (6) THEN wait for the planning → dev
  cue.*
- **"Common ordering mistake" warning lives in §6 too.** The
  warning currently sits only in §3 (entry path: spec already on
  disk). Move (or duplicate) into §6 so the rule fires regardless
  of how the agent reached planning — fresh interviews or
  spec-on-disk entry. Single source of truth preferred; §3 cites
  §6 if duplication doesn't read clean.
- **§5 Phase-gating cues annotate the prerequisite.** Today
  the cue list reads as "wait for cue X to move to phase Y";
  doesn't enumerate "and Y is gated, not the *commit* before Y."
  Annotate each cue with what it gates vs. what must already
  have happened, so the cue is unambiguously *post-review*.
- **Acceptance criterion.** A judge-mode functional fixture
  asserts: when an agent completes the four §6 interviews, it
  commits + pushes + opens the Planning Review Gate **before**
  pausing for the planning → dev cue. The fixture's wrong-answer
  cues list the inverted order ("ask for confirmation, then
  commit") so the judge has a concrete failure shape.

## Test Strategy

**Test layers fired for this change (per `specs/project.md`'s
four-layer surface):**

- **`deno task test:unit`** — *no new coverage*. The
  `code_review:` block is read by an LLM at runtime, not by
  `maid` itself; `maid` doesn't validate spec content, so adding
  a unit test would be overhead without payoff. Existing 22
  tests still load-bearing as the §7 Test Gate default.
- **`deno task test:smoke`** — structural symlink check, no
  semantic claims about the new behavior. Run after `deno task
  deploy` to confirm the SKILL.md edit lands through the
  symlink.
- **`deno task test:functional`** (judge mode) — **load-bearing
  for this feature.** Two new judge fixtures + one extended
  fixture, each `tools: claude,kiro`. Cross-tool evidence per Q4
  decision; the skill is critical so the cost is justified.
- **`deno task test:all`** — runs at the agent-dev Push Gate +
  closure for the SKILL.md edit.

**Feature-scope override of the "agentic runs stop at smoke"
rule.** `specs/project.md` codifies that agentic runs hand off
`test:functional` to the user because of API-credit cost. For
this feature, the user has authorised the agent to run
`test:functional` inside the dev loop — functional evidence is
the only artefact that proves the §7 prose changes are
interpreted correctly by both Claude Code and Kiro, and a critical
skill update can't ship on smoke alone. The override applies for
the duration of this branch only and does **not** mutate
`project.md`.

**Success criteria mapped onto fixtures:**

| Behavior claim                                                   | Fixture                                | Layer       |
| ---------------------------------------------------------------- | -------------------------------------- | ----------- |
| Loop is `Quality → Test → Code Review → Push` (between)          | `kdevkit-dev-loop.smoke` (extended)    | functional  |
| Refuse-on-fail still applies to the new gate                     | `kdevkit-dev-loop.smoke` (extended)    | functional  |
| Reviewer runs in clean-context agent call (no feature spec)      | `kdevkit-review-gate.smoke` (new)      | functional  |
| Default reviewer = host's native review when `project.md` silent | `kdevkit-review-gate.smoke` (new)      | functional  |
| Configured reviewer in `project.md` is preferred                 | `kdevkit-review-gate.smoke` (new)      | functional  |
| Sub-threshold loops back to start of dev loop                    | `kdevkit-review-gate.smoke` (new)      | functional  |
| `code_review:` block shape — reviewer + threshold + authority    | `kdevkit-review-config-setup.smoke` (new) | functional  |
| Light setup UX when no block declared (prompt + sticky write)    | `kdevkit-review-config-setup.smoke` (new) | functional  |
| Authority semantics (hard-stop vs. soft + budget)                | `kdevkit-review-config-setup.smoke` (new) | functional  |
| Planning sequence: commit → push → open Planning Review Gate → THEN wait | `kdevkit-feature-planning.smoke` (extended) | functional  |
| Symlink still resolves (SKILL.md edit lands)                     | structural (existing)                  | smoke       |

**Wrong-answer cues (per Q4) appear inline in each
`expected_narrative`** so the judge anchors on a concrete fail
list — pattern matches `kdevkit-dev-loop.smoke`'s closing
"Wrong answers: …" clause.

## Design

### §6 Plan-commit rule — before / after

**Before (current text):**

> Commit the populated spec as `plan(<feature>): initial spec` and
> ship through the Planning Review Gate before any code work; skip
> if `planning_phase: false` (§2).

**After (numbered, mirrors §3):**

> 1. Finish the four interviews and write
>    `$SPEC_ROOT/feature/<feature>.md`.
> 2. Confirm readiness with the user; iterate on the spec if needed.
> 3. **Commit** the spec as `plan(<feature>): initial spec`.
> 4. **Push** the feature branch.
> 5. **Open the Planning Review Gate** (PR/CR with the planning
>    body shape — see below).
> 6. **Then** wait for the planning → dev cue (§5).
>
> The cue gates the *move* to dev — not the planning commit. The
> commit + push + review must happen first so the user has
> something to react to. Reversing this order (waiting for the cue
> before committing) is the most common ordering mistake.
>
> Skip steps 3–6 if `planning_phase: false` (§2).

§3's existing block becomes a brief mention pointing here ("apply
the §6 Plan-commit rule"), so there's a single source of truth for
the order. §5's cue list gets a one-line clarifier on each cue:

> - **Planning → dev**: `"spec looks good"` / `"start build"` /
>   `"plan approved"`. *Fires after the Planning Review Gate is
>   open — see §6 Plan-commit rule for the prerequisite sequence.*

### §7 ordering — before / after

```
                 OLD                                      NEW
                 ───                                      ───
Quality Gate     fmt → lint → typecheck →                 fmt → lint → typecheck
                 SELF-REVIEW SCORE (≥70 → pass)
                 ↓                                        ↓
Test Gate        run tests; budget 2                      run tests; budget 2
                 ↓                                        ↓
Push Gate        push                                     Code Review Gate ← NEW
                 ↓                                        ↓ (≥threshold → continue)
Agent-dev        open/update PR/CR                        ↓ (<threshold → loop back to Quality;
Review Gate                                                  budget 2)
                                                          ↓
                                                          Push Gate
                                                          ↓
                                                          Agent-dev Review Gate
                                                          (PR/CR — unchanged)
```

The Quality Gate slim-down (no self-review-score step) is the
prerequisite for the new gate; the score concept survives but is
now reviewer-produced, not self-introspected.

### Naming

`Code Review Gate` (new) coexists with §9's pre-existing
`Review Gates` (PR/CR contract). They are distinct concepts:

- **Code Review Gate** (§7) — gates the *push* on a peer-style
  code review.
- **Review Gates** (§9) — universal CR/PR submission contract
  applied at planning / dev / closure phase boundaries.

Fixture narratives must say "Code Review Gate" precisely; the
existing review-gate language stays unchanged.

### Configuration shape — `project.md` `code_review:` block

Structured nested block under `## Agent Development > kdevkit`:

```yaml
code_review:
  reviewer: <ref>                       # default: host-native
  threshold: 70                         # default: 70
  authority: hard-stop                  # default: hard-stop; alternative: soft
  retry_budget: 2                       # default: 2
```

All keys optional. Omitting `code_review:` entirely triggers the
setup UX at first feature-start. Once written (even with all
defaults), the block sticks — the question doesn't re-fire.

### `<ref>` syntax

Prefix-tagged so the orchestrator knows what to dispatch:

- `host-native` — the host coding agent's built-in review.
- `skill:<name>` — a skill in the registry (e.g.
  `skill:python-strict-review`). Bare strings without a prefix
  default to `skill:`.
- `mcp:<server>.<tool>` — an MCP server's tool.
- `agent:<name>` — a named project-configured agent.

The reviewer reference is opaque to kdevkit — the skill prose
describes the *contract* (clean context, returns
findings + score), not the dispatch mechanics. Hosts translate.

### Clean-context dispatch — contract

The skill prose describes the contract abstractly per Q2 = (b);
no host-specific cribbing in the body. If the contract proves
too vague during the empirical check (task 7), a "Hosts"
appendix lands as task 8 — a short crib-sheet of known-good
incantations marked advisory.

The contract:

> The Code Review Gate dispatches to the configured reviewer in
> a **fresh-context agent call** — the reviewer must not see the
> feature spec, session log, or in-progress conversation. It
> receives `project.md`, the diff vs. base, and any reviewer
> reference / config. The reviewer returns a findings list +
> score 0–100. The orchestrator compares score to threshold;
> sub-threshold loops back to Quality.

What's passed:

- ✅ `project.md` (project invariants — every reviewer needs the
  architecture / hard-constraints / public-repo signal).
- ✅ Diff vs. base.
- ✅ Reviewer reference + threshold + authority + retry budget.
- ❌ `feature/<name>.md` (deliberately excluded — feature
  context is what we're trying to keep out).
- ❌ Session log / decision log.
- ❌ Conversation history.

Reviewers that legitimately need feature-context (e.g. "did the
implementation match the spec?") must ask for it themselves —
kdevkit's contract is "no feature-spec by default." This keeps
the gate honest about what it's reviewing: the diff against the
project, not the diff against the agent's own plan.

### Loop-back semantics

Sub-threshold path:

1. Reviewer returns findings + score.
2. Append findings (or a one-line summary + reviewer URL) to the
   feature spec's Session Log so they're captured.
3. Treat the highest-severity findings as the next implementation
   slice.
4. Re-enter Quality Gate from the top.
5. Re-run Test Gate.
6. Re-run Code Review Gate.
7. Repeat until score ≥ threshold or `retry_budget` exhausted.

Worst-case loop: `retry_budget=2` (review) × `Test Gate budget=2`
(test) = up to 4 fix-and-retry cycles per slice. Past that, stop
and report per §7's existing refuse-on-fail. Both budgets are
configurable per project; the worst-case is the **default**
worst-case.

### Authority

`hard-stop` (default): exhausting `retry_budget` blocks Push.
The agent surfaces findings, refuses to push, awaits explicit
override.

`soft`: exhausting `retry_budget` allows a final Push with
residuals appended to Session Log — matches today's "fix once,
proceed with residuals" softness for projects that prefer it.

Critical-category configuration (e.g. "security findings are
always hard-stop even in soft authority") is out of scope for
this iteration — single-knob authority is sufficient. If the
empirical check shows it's needed, file a backlog item; don't
add it here.

### Setup UX — light prompt at feature-start

Fires from §3 / §4 when `project.md` lacks a `code_review:`
block. One-line prompt:

> _"This project doesn't declare a code reviewer. Use the host's
> native review (default), or point to a project-specific one
> (`skill:<name>` / `mcp:<server.tool>` / `agent:<name>`)? Reply
> 'default' or paste a reference."_

Action:

- Reply 'default' → write
  `code_review: { reviewer: host-native }` to `project.md`'s
  `## Agent Development > kdevkit` block. Sticky.
- Reply with a reference → write
  `code_review: { reviewer: <ref> }`. Threshold / authority
  inherit defaults.
- Reply 'skip' → proceed without writing; question re-fires next
  session. (Lets a user defer the decision without committing.)

The same prompt fires from §2's first-time `project.md` flow as
a one-liner appended to the project setup interview.

### File / SKILL.md surface

Edits land in:

- `sources/skills/kdevkit/SKILL.md` — §2 (template + Agent
  Development docs), §3 (cite §6 instead of duplicating the
  ordering rule), §3/§4 (setup UX cue), §5 (annotated cue list),
  §6 (numbered Plan-commit rule + ordering-mistake warning), §7
  (Quality slim-down, Code Review Gate prose, loop-back).
- `specs/project.md` — add mAId's own `code_review:` block
  (host-native default) under `## Agent Development > kdevkit`.
  Eats own dogfood + confirms shape parses.
- `tests/functional/skills/kdevkit-dev-loop.smoke` — narrative
  extension only.
- `tests/functional/skills/kdevkit-feature-planning.smoke` —
  narrative extension covering the numbered Plan-commit
  sequence + the inverted-order wrong-answer cue.
- `tests/functional/skills/kdevkit-review-gate.smoke` — new.
- `tests/functional/skills/kdevkit-review-config-setup.smoke` —
  new.

`maid/registry.ts`, schema validators, deploy logic — untouched.
The change is prose + fixture only; deploy is a no-op beyond
re-publishing the symlinked `SKILL.md`.

## Implementation Plan

Three-phase per kdevkit; commits per coherent unit per §9.

### Planning phase

1. Land this spec on the branch as
   `plan(kdevkit-code-review-gate): initial spec` per §6 plan-commit
   rule. Push. Open Planning Review Gate. Wait for the planning →
   dev cue (`"spec looks good"` / equivalent).

### Dev phase

Each step is one logical commit unless noted.

2. **§6 Plan-commit rule — numbered sequence + ordering-mistake
   warning.**
   - Rewrite §6's Plan-commit rule as the numbered (1)–(6)
     sequence above.
   - Move (or duplicate as a citation) §3's "common ordering
     mistake" warning into §6.
   - Update §3's spec-already-drafted block to cite "apply the
     §6 Plan-commit rule" instead of duplicating the steps.
   - Annotate §5 cue list with the prerequisite clarifier.
   - Commit: `feat(kdevkit): enumerate §6 plan-commit order + warn on inverted commit/review`.

3. **§2 + project.md template — `code_review:` block.**
   - Document the block under "Optional `## Agent Development`
     section."
   - Defaults table: reviewer / threshold / authority /
     retry_budget.
   - `<ref>` syntax (prefix-tagged).
   - Commit: `feat(kdevkit): document code_review config block`.

4. **§3 + §4 — light setup UX cue.**
   - One-line prompt language.
   - Sticky-write rule.
   - First-time `project.md` integration.
   - Commit: `feat(kdevkit): add code-review setup UX at feature start`.

5. **§7 Quality Gate slim-down.**
   - Drop "self-review-score" step 4.
   - Quality is now fmt + lint + typecheck.
   - Adjust the Quality narrative so it reads cleanly without
     the score step.
   - Commit: `refactor(kdevkit): remove self-review score from Quality Gate`.

6. **§7 Code Review Gate insertion.**
   - New gate between Test and Push.
   - Clean-context dispatch contract (project.md + diff in;
     feature spec out).
   - Score / threshold / authority / retry-budget semantics.
   - Loop-back rule (back to Quality; worst-case 4 cycles).
   - Refuse-on-fail integration.
   - Update §7 prose pointing into / out of the new gate so
     cross-references stay valid.
   - Commit: `feat(kdevkit): add Code Review Gate between Test and Push`.

7. **mAId `specs/project.md` — declare `code_review:` block.**
   - `## Agent Development > kdevkit > code_review:`
     `{ reviewer: host-native }`.
   - Confirms the shape parses cleanly and dogfoods the new
     defaults.
   - Commit: `feat(kdevkit): declare code-review config in mAId project.md`.

8. **Empirical check — run the new Code Review Gate against
   itself.**
   - On the feature branch's accumulated diff, dispatch a
     fresh-context agent call as described by the §7 prose.
   - If contract is too abstract for the host to act on, file
     task 9 (Hosts appendix) immediately. Otherwise mark the
     contract validated in Decision Log and proceed.
   - No commit unless task 9 fires.

9. **§7 Hosts appendix — conditional.**
   - **Skip if task 8 succeeds.** Only land if the contract
     proves too vague.
   - Short Claude Code crib (Agent tool incantation), Kiro
     placeholder. Marked advisory.
   - Commit: `docs(kdevkit): add Hosts appendix for Code Review Gate dispatch`.

10. **Test fixtures.**
    - **10a · Extend `kdevkit-dev-loop.smoke`.** Narrative gains
      the new gate position, refuse-on-fail covering it, and
      a wrong-answers cue listing skipping the gate / treating
      review as advisory / merging the new gate into Quality.
      `tools: claude,kiro`.
    - **10b · Extend `kdevkit-feature-planning.smoke`.** Narrative
      gains the (1)–(6) numbered Plan-commit sequence; wrong-answer
      cue lists the inverted order ("ask for confirmation, then
      commit") and the single-source-of-truth cite from §3.
      `tools: claude,kiro`.
    - **10c · `kdevkit-review-gate.smoke` (new).** Single judge
      fixture covering: clean-context dispatch, default =
      host-native when `project.md` silent, configured reviewer
      preferred when declared, sub-threshold loops back to start
      of dev loop. Wrong-answers cue listing leaking feature spec
      into reviewer / treating review as a self-introspection /
      looping back only to Quality without re-running Test.
      `tools: claude,kiro`.
    - **10d · `kdevkit-review-config-setup.smoke` (new).** Single
      judge fixture covering: `code_review:` block shape +
      defaults, threshold/authority/retry_budget semantics
      (hard-stop vs. soft), light setup UX (sticky-write,
      'default' / `<ref>` / 'skip' branches). Wrong-answers cue
      listing inventing config keys / skipping sticky-write /
      treating soft as no-budget. `tools: claude,kiro`.
    - One commit per fixture file (4 commits total —
      kdevkit-dev-loop / kdevkit-feature-planning extensions are
      separate from the two new fixtures).

11. **Quality + Test + Code Review + Push for the SKILL.md
    branch itself.**
    - `deno task fmt && deno task lint && deno task check`.
    - `deno task test:unit` (existing 22 tests must stay green).
    - `deno task deploy` then `deno task test:smoke` (symlink
      lands).
    - `deno task test:functional` (per the feature-scope override
      in Test Strategy — agent runs functional this once).
    - Code Review Gate dispatch on the cumulative branch diff.
    - Push, open / update Agent-dev Review Gate.

### Closure phase

12. **Reconcile.** Sweep Implementation Plan / Decision Log /
    open questions. Resolve in place. **Done** — no `[ ]` /
    TODO markers; task 9 (Hosts appendix) confirmed skipped per
    Decision Log; critical-category authority remains a
    deferred future-work item per Decision Log (not promoted to
    backlog at close).
13. **Soft `project.md` verify.** Already updated in task 7;
    confirm no further edits warranted. **Done** — `project.md`
    declares `code_review: { reviewer: host-native }`; no
    further close-time edits warranted.
14. **Backlog cleanup (interactive).** **Skipped at user
    direction** — backlog items added in parallel by other work
    don't need closure review for this feature.
15. **`close(...)` commits.** Stage spec / docs edits. Push.
16. **Closure Review Gate.** Title rewritten to
    `feat(kdevkit): Code Review Gate in dev loop + §6 plan-commit
    ordering` (drops `close(...)` mechanic per §8.5). Body:
    Why + Approach + Verification + Reading order.
17. **Squash merge.** One commit on `main`.
18. **Branch cleanup.** Delete local + remote per §8.7.

### Risk notes

- **§7 cross-references.** `kdevkit-dev-loop.smoke` mentions
  specific gate names; §9 references §7 by name. After tasks
  5–6, run an internal grep on `SKILL.md` + the smoke fixtures
  for stale "Quality Gate" / "self-review" mentions; fix in
  place.
- **Naming ambiguity.** "Code Review Gate" vs §9's "Review
  Gates." Fixture narratives must say the full name. Watch the
  judge for false-PASS where the agent talks about §9's PR/CR
  Review Gates and hits the keyword.
- **Worst-case retry depth.** Default budgets are 2×2 = 4. If
  this proves jittery in practice, tune the default down to 1
  (review) × 2 (test) — but ship with 2 unless the empirical
  check screams.
- **Reviewer context starvation.** Some reviewers may need
  feature-spec context to be useful. Decision: reviewer must
  ask for it; kdevkit's contract is "no feature-spec by
  default." Backstop the decision in Decision Log so future
  edits don't quietly leak it back in.
- **Empirical check could fail.** Task 8 might surface that the
  abstract contract is unactionable on Claude Code without the
  Agent tool name. Task 9 is the prepared escape hatch — adds
  ~10 lines of Hosts appendix. Don't overspecify upfront.
- **Functional-test cost.** 4 fixtures × 2 tools = 8 judge
  calls per `test:functional` run (2 extended + 2 new); the
  agent runs it under the feature-scope override; budget the
  time / API credits before starting task 11.
- **Cross-cutting §6 ordering risk.** Renumbering §6's
  Plan-commit rule and migrating §3's warning into §6 risks
  silently breaking `kdevkit-feature-planning.smoke`'s existing
  narrative. After task 2 lands, re-read the existing fixture
  and either extend in place (preferred) or rewrite — but don't
  let the existing fixture turn into a stale assertion.
- **Sticky-write on 'default'.** Writing
  `code_review: { reviewer: host-native }` is intentional —
  records the intent, avoids re-prompting. If a project later
  changes hosts (Claude → Kiro), `host-native` floats to the
  new host's native reviewer; no migration needed.

## Session Log

<!-- append: date · what was done · decisions made -->

- **2026-06-03** · Closure phase. In-flight reconciliation:
  no `[ ]` / TODO markers; task 9 (Hosts appendix) confirmed
  skipped per the empirical-check Decision Log; critical-
  category authority stays deferred per Decision Log (no
  real-project ask materialised — not promoting to backlog at
  close). Soft `project.md` verify: already declared
  `code_review: { reviewer: host-native }` in task 7; no
  further close-time edits warranted. Backlog cleanup skipped
  at user direction — parallel backlog additions are out of
  scope for this feature's closure.

- **2026-06-01** · `deno task test:functional` ran inside the
  agent-dev loop per the feature-scope override. Final tally:
  50 PASS / 1 FAIL. The lone FAIL is `kdevkit-dev-loop via kiro
  (judge)` and the answer head shows a Kiro environment artifact
  ('Error: Json supplied at /home/gorantls/.kiro/agents/
  meshclaw-knowledge.json is invalid: unknown field
  systemPrompt'). Kiro returned the error string instead of
  invoking the skill; the judge correctly graded the error
  message as "doesn't cover the kdevkit dev loop." Same fixture
  passes via claude, all four kdevkit fixtures touched by this
  feature pass via both tools where Kiro's env worked
  (kdevkit-review-gate kiro judge: PASS;
  kdevkit-review-config-setup kiro judge: PASS; kdevkit-feature-
  planning kiro judge: PASS). The failure is outside this
  feature's scope; surfacing in the spec, not chasing.

- **2026-06-01** · Empirical Code Review Gate dispatched against
  the cumulative SKILL.md + project.md diff (task 8). Two loop
  iterations: loop 1 scored 78 with 7 findings (1 high, 3 medium,
  3 low); loop 2 scored 74 with 8 findings (3 high, 3 medium, 2
  low) — surface coherence catches the loop-1 fixes hadn't
  cleared. All 8 loop-2 findings applied in
  `fix(kdevkit): apply Code Review Gate findings…`. Both
  iterations cleared the 70 threshold; `retry_budget=2`
  exhausted; loop closed. Decision: the abstract dispatch
  contract is actionable on Claude Code (the reviewer subagent
  produced a usable score + JSON without host-specific
  scaffolding) → task 9 Hosts appendix is **not** needed.

- **2026-06-01** · §6 Plan-commit ordering reinforcement
  bundled into this feature mid-planning. The planning agent
  inverted commit and review (asked for spec confirmation
  before committing/pushing). Root cause: §6's Plan-commit
  rule says "ship through the Planning Review Gate before any
  code work" but doesn't enumerate the order; §3's "common
  ordering mistake" warning fires only on the spec-on-disk
  entry path. Fix: numbered (1)–(6) sequence in §6 + warning
  migrated/cited from §3, plus annotated cue list in §5. Adds
  one dev-phase commit (task 2) and one extended judge fixture
  (task 10b).

- **2026-06-01** · Spec drafted from a four-interview planning
  session. Branched `feat/kdevkit-code-review-gate` off `main`
  at `c1371cf`. Key calls: no new mAId-shipped reviewer skill
  (default = host-native); Code Review Gate inserts between
  Test and Push (option b); structured `code_review:` block
  with prefix-tagged `<ref>` syntax; clean-context dispatch
  passes `project.md` + diff but excludes feature spec; loop-back
  is full-loop with budgets 2 (review) × 2 (test); abstract
  dispatch contract with conditional Hosts appendix as escape
  hatch; functional tests run inside the agent-dev loop for this
  feature only (override of `project.md`'s "stop at smoke" rule).

## Decision Log

<!-- append: decision · rationale · alternatives rejected -->

- **2026-06-01 · Abstract dispatch contract is actionable —
  Hosts appendix not needed.** Rationale: an empirical Code
  Review Gate dispatch on Claude Code (using a fresh-context
  Agent-tool subagent) produced a usable score + JSON findings
  list without any host-specific scaffolding in §7. The contract
  ("dispatch in a fresh-context agent call; pass project.md +
  diff + reviewer config") was sufficient. Task 9 (Hosts
  appendix) skipped. Alternative rejected: ship the appendix
  as a safety net — not paying for speculative complexity that
  the empirical check showed isn't needed. Revisit if the
  contract proves vague on a different host (Kiro / Codex).

- **2026-06-01 · §6 Plan-commit rule becomes a numbered
  sequence; "common ordering mistake" warning lives in §6 with
  §3 citing it.** Rationale: the planning agent on this very
  feature inverted commit and review, treating "spec looks
  good" as the cue to *commit* rather than the cue to *move to
  dev after the Planning Review Gate is open*. §3 spells the
  order out explicitly for the spec-on-disk entry path; §6's
  plan-commit rule was only a one-liner ("ship through the
  Planning Review Gate before any code work"). Single source
  of truth in §6; §3 cites. Alternative rejected: leave §6
  alone and rely on §3's warning — only fires on one of two
  entry paths, won't catch a fresh-interview agent.

- **2026-06-01 · Default reviewer = host-native, not a
  mAId-shipped generic skill.** Rationale: at this stage we're
  unsure what a "good generic reviewer" looks like and the host
  agents already ship perfectly serviceable code reviewers.
  Building a generic skill on top would duplicate effort and add
  a layer to maintain. Alternative rejected: ship a generic
  `kdevkit-code-review` skill alongside kdevkit. Revisit if
  host-native proves consistently weak across projects.

- **2026-06-01 · Code Review Gate sits between Test and Push,
  not inside Quality.** Rationale: tests must pass first so the
  reviewer judges a green diff, not a half-broken WIP. Reviewer
  catches practices the deterministic gates can't. Alternative
  rejected: run review inside Quality (as a step) — would
  conflate deterministic and subjective checks and force the
  reviewer to ignore the test failures it should be sensitive to.

- **2026-06-01 · Pass `project.md` + diff to reviewer; exclude
  feature spec.** Rationale: feature context is exactly what
  biases a reviewer ("the agent's plan said this, so the diff is
  fine"). The reviewer should evaluate the diff against the
  project's invariants, not against the agent's own plan.
  Alternative rejected: pass everything (including feature
  spec) — defeats the clean-context purpose. If a reviewer needs
  feature-spec context, it asks for it; the contract default is
  "without."

- **2026-06-01 · Loop-back is full-loop (Quality → Test →
  Review).** Rationale: emulates a human reviewer's
  fix-and-retry cadence; deterministic gates re-validate the
  fix. Alternative rejected: loop to coding only, re-running
  gates on demand — saves time but introduces non-determinism
  the agent could exploit to skip checks.

- **2026-06-01 · Budget 2 (review) × 2 (test); worst-case 4
  cycles.** Rationale: matches Test Gate's existing budget for
  consistency; 4 cycles is enough for a typical prose-skill or
  small refactor without infinite-looping. Alternative
  rejected: budget 1 — too tight; one bad finding shouldn't
  immediately escalate to user.

- **2026-06-01 · Abstract dispatch contract by default; Hosts
  appendix only if empirical check proves it too vague.**
  Rationale: keeps the skill portable; concrete host
  incantations age fast. Alternative rejected: encode hosts
  upfront — locks the skill to today's APIs.

- **2026-06-01 · Functional tests run inside the agent-dev loop
  for this feature.** Rationale: the change is to a critical
  skill; cross-tool functional evidence is the only artefact
  proving both Claude Code and Kiro interpret the new prose
  correctly. Override is feature-scope; `project.md`'s general
  rule (agentic runs stop at smoke) stays intact for future
  features.

- **2026-06-01 · Single-knob authority (hard-stop / soft);
  critical-category overrides deferred to backlog if needed.**
  Rationale: ship the simpler shape first; richer authority
  semantics (e.g. "security always hard-stop even in soft") add
  value only if real projects ask for it. Alternative rejected:
  ship richer authority upfront — speculative complexity.
