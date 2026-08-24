# Initiative: kdevkit — decompose the workflow, harden the gates

## Why

kdevkit is one 1246-line always-on `SKILL.md` driving four tiers
and three phases in a single agent session. Two failure modes
follow from that shape, both observed live in this repo:

- **Drift.** Long sessions executing a long document quietly stop
  honouring it. The mechanism is documented: recall degrades as
  context grows, and it "emerges across all models" as "a
  performance gradient rather than a hard cliff" — which is
  exactly the symptom (rules dropped silently, not loudly). A
  one-line change once drew a full six-section spec; the Review
  Briefing section had to be physically moved because reading
  order diverged from execution order.
- **Soft gates.** The Code Review Gate dispatches one generic
  reviewer that receives neither the project's own authoring
  conventions nor any project-specific notion of what matters. A
  convention violation scored 90/100 and shipped past the gate
  (2026-07-15); the user caught it by hand.

The fix is decomposition plus hardening: phases become
independently loadable modules handed to fresh agents, with the
checked-in spec as the handoff; and the review gate becomes a
multi-lens panel a project can extend.

Full research, alternatives weighed, and the interaction record
live in `specs/feature/kdevkit-decompose-and-harden.md` (the
planning-phase spec, retained as the decision archive). This file
is the implementable contract; it should stand alone.

## Streams

1. **module-split** (`refactor/kdevkit-module-split`) — split
   `SKILL.md` by stage into loadable modules; decide and enforce
   what stays always-on. Prereq: none. **Shipped 2026-08-04**
   (1246 → 572 always-on). Left open: the prose-compression pass
   inside each module, tracked in
   `backlog/kdevkit-refactor-shrink-always-on-context.md` — it has
   no stream of its own and the core landed 72 lines over target.
2. **phase-handoff** (`feat/kdevkit-phase-handoff`) — the
   checked-in spec as the phase handoff record; a stage runs in
   one session or its own. Prereq: 1.
3. **gate-packets** (`feat/kdevkit-gate-packets`) — every
   dispatched gate takes an enumerated packet and returns a file;
   reviewer panel with project-owned lenses; retire the 0–100
   score. Prereq: 2.
4. **deterministic-phasing** (`feat/kdevkit-deterministic-phasing`)
   — code owns phase transitions rather than an agent remembering
   the top of a file. **Blocked pending the code-vs-prose
   boundary decision** (see Decisions, D-open-1). Prereq: 3.
5. **session-orchestration** (`feat/kdevkit-session-orchestration`)
   — how phases move between separate agents vs. a master running
   subs. Mechanism undecided; `kaimux` may not be it. **Blocked on
   the same boundary decision.** Prereq: 4.
6. **cross-tool-verification** (`test/kdevkit-cross-tool`) — run
   the whole decomposed workflow on **codex** and **kiro** to
   confirm it holds outside Claude Code. Prereq: 3 (streams 1–3
   are the portable surface; run before 4–5 harden anything
   tool-specific).

Streams 1–3 run now, autonomously (see Decisions). Streams 4–5 do
not start until 1–3 are closed. Stream 6 validates the mission
invariant — mAId is tool-agnostic, so a decomposition that only
works on one host is a regression regardless of how well it reads.

## Decisions taken at the initiative level

These bind every stream.

- **Phase boundaries are actor boundaries, not artifact
  boundaries.** Four phases: plan → dev → human review → closure.
  The actor change *is* the context change, which is what buys
  isolation. Rejected: spec-kit's `specify → plan → tasks →
  implement`, where all steps are one actor doing one kind of
  thinking (a boundary there costs re-grounding and buys nothing),
  and which has no review or closure phase at all.

- **Any phase may return to any earlier phase. The criterion is:
  at which layer did the fault enter?** Return there, not to where
  it surfaced. A wrong requirement found in review returns to
  plan; a correct plan built wrong returns to dev. Every return
  writes one line in the handoff record naming what entered where
  — which is also what keeps a loop-back from becoming a silent
  plan amendment (§9). The phases are a cycle with a declared
  re-entry rule, not a pipeline. No from→to matrix.

- **The handoff record is a section of the WIP feature spec.**
  Being checked in is the load-bearing property: every phase gets
  it for free, and a session can die or restart without losing
  what crossed the last boundary. **Each phase writes its state
  into the spec before handing on** — the write is part of the
  phase, so a phase that ended without one did not finish. That
  makes an unwritten handoff an assertable signal. Target size
  800–1500 tokens. No new artefact.

- **Every dispatched gate takes an enumerated packet and returns
  a file.** The packet states what is included *and* what is
  excluded. Files rather than return values because the host's
  agent dispatch returns free-form text that the parent "may
  summarize" — a prose contract is unenforceable, and a file is
  inspectable and testable. Applies uniformly to code review,
  review briefing, and structural verify.

- **Conventions reach a reviewer by being stated, not inferred.**
  `AGENTS.md` states the rule flatly; `project.md` carries the
  *why* where it constrains future work. No third file — a
  project that already keeps a style guide gets it included in the
  packet; kdevkit does not create one. Test for which layer a line
  belongs to: *if removing it changes what the agent writes, it's
  an AGENTS.md rule; if it changes what the agent decides, it's
  project.md rationale.* Rejected: a dedicated project-idiom
  reviewer lens — a reviewer would have to infer what a bundle can
  simply state.

- **The maker-knowledge firewall holds everywhere.** No
  implementer reasoning, session narrative, or self-assessment
  reaches any reviewer or briefing generator. A model reviewing
  its own output reuses the reasoning that produced it.

- **Prose must stand alone.** Every stream's output has to work
  for someone driving kdevkit bare, with no wrapper and no runtime
  — skills deploy as plain markdown symlinks. Code may make a
  transition reliable where present; it may never *be* the
  transition. This is the constraint streams 4–5 must resolve
  before starting, not discover at review.

- **Reviewer verdicts are severities, aggregated strictest-wins in
  code.** The 0–100 score and `threshold` are retired: a numeric
  verdict makes the same diff flap between labels across runs.
  Severity and confidence are independent axes — confidence gates
  the verdict, never suppresses a finding. A crashed lens yields
  `INCOMPLETE`, never a pass: a false failure is recoverable, a
  false clean is not.

- **Each stream ships with A/B evidence.** The four
  `kdevkit-*.smoke` fixtures, before and after, 3+ runs sampled
  because trigger behaviour is probabilistic. A refactor that
  can't be A/B'd doesn't ship.

- **Autonomous execution, streams 1–3.** The agent runs each
  stream as the human would: plan, spec, commits, push, PR,
  quality gates, `just test`, fresh-context code review, briefing,
  reconcile, squash-merge, cleanup — **and merges unattended**.
  Coding agent of choice: **claude**. This is deliberate scope
  expansion of `project.md`'s "agentic runs must stop at
  `just test`" for this initiative only, granted by the user
  2026-08-04; the general rule stands for other work. Paid
  tri-tool `verify-skills` remains a human call (stream 6 is where
  cross-tool evidence gets gathered).

### Open at the initiative level

- **D-open-1 · Where exactly is the code/prose line?** Streams 4
  and 5 both turn on it. Working position: answering an admission
  or transition question is judgement (prose, must stand alone);
  collecting the observable state that a question is answered
  against is not (code, optional accelerator). Settle at the head
  of stream 4 with the bare-path degradation spelled out. Until
  then, 4 and 5 stay blocked.

- **D-open-2 · Session orchestration mechanism.** Separate agents
  per phase vs. a master running subs. `kaimux`'s session registry
  is flat today (keyed by pane id, no parent), and the kaimux spec
  deliberately chose "no extra parent process." Prior art argues
  against a persistent coordinator that *remembers* — state should
  live in the spec, with the orchestrator reading it. Stream 5.

## Status

| Stream | Branch | CR | Status | Shipped | Learnings |
|---|---|---|---|---|---|
| 1 · module-split | `refactor/kdevkit-module-split` | [#40](https://github.com/kusimari/mAId/pull/40) | shipped | 2026-08-04 | Moves-only + a rule inventory made a prose refactor reviewable; the defects all landed in the *new* prose (a stale trigger row, a vacuous assert), not the moved text. Fresh-context reviewers caught what every deterministic gate missed. Shipped without the paid A/B — that gap is now the compression pass's precondition. |
| 2 · phase-handoff | `feat/kdevkit-phase-handoff` | [#41](https://github.com/kusimari/mAId/pull/41) | shipped | 2026-08-06 | Four review passes, three failed, all on the same defect class: an assert satisfiable without the agent doing the work — twice including the *fix* itself. Filed `kdevkit-adversarial-assert-discipline` so the construction-and-replay habit becomes a Test Gate step instead of memory. Closed `kdevkit-spec-consolidation-before-dev` — this stream is that backlog item. Shipped without the paid A/B, same gap as stream 1. |
| 3 · gate-packets | `feat/kdevkit-gate-packets` | [#42](https://github.com/kusimari/mAId/pull/42) | shipped | 2026-08-06 | **Five** review rounds on one fixture assert — a new record for this initiative. Every round found a real, distinct bug (schema drift between two copies, four paraphrase bypasses, a self-inflicted vacuous-pass from nested shell quoting, an unsalted temp path, stderr never checked). One round-6 defect (a genuine prose ambiguity in the ceremony-lane rule, plus its own missing test coverage) came from the review briefing, not the code-review gate — both mechanisms caught something the other didn't. Retired the 0-100 score for severities; one dispatch, three perspectives, not three subagents, per the research. Extended `kdevkit-adversarial-assert-discipline` with three more trap classes. Shipped without the paid A/B, same carried-forward gap as streams 1-2. Then a first real paid tri-tool run found three unanimous/near-unanimous prose failures spanning streams 2-3 (consolidation skipped, deferred work not filed, `Phase:` field left stale) — fixed and confirmed on claude in a follow-on branch; kiro/codex not yet run. |
| 4 · deterministic-phasing | `feat/kdevkit-deterministic-phasing` | — | **blocked — awaiting user** (D-open-1) | — | — |
| 5 · session-orchestration | `feat/kdevkit-session-orchestration` | — | **blocked — awaiting user** (D-open-1) | — | — |
| 6 · cross-tool-verification | `test/kdevkit-cross-tool` | — | not started | — | — |
