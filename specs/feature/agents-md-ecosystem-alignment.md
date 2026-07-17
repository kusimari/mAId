# Feature: agents-md-ecosystem-alignment

## Git Setup

- Branch: `feat/agents-md-ecosystem-alignment`
- Base: `main` at `10c8c9d`
- Worktree: `mAId-worktrees/agents-md-ecosystem-alignment` (teardown offered at §8.8)

## Feature Brief

Bring kdevkit's context-capture model into deliberate alignment with
where the AI-coding-agent community landed by mid-2026 — **AGENTS.md**
as the vendor-neutral operational context file (OpenAI → Linux
Foundation Agentic AI Foundation, Dec 2025) and the three-layer
context model the ecosystem converged on independently — while
proving the alignment holds by dogfooding it through a rebuilt,
behavioral test harness that exercises the skill across **three**
coding agents (claude, kiro, codex), not two.

Two capabilities land together:

1. **kdevkit teaches and honors the three-layer context model.** An
   agent running kdevkit recognizes the operational layer (repo-root
   `AGENTS.md`), the project-knowledge layer (`project.md`), and the
   transient per-feature layer (the feature spec), reads operational
   commands from `AGENTS.md` where one exists, never corrupts
   `AGENTS.md` convention with kdevkit scaffold, and bubbles durable
   content up out of the feature spec at closure.

2. **The kdevkit test suite becomes behavioral and tri-tool.** The
   17 recitation-probe fixtures collapse into a small set of
   behavioral fixtures that seed a scratch repo, drive the agent
   through a kdevkit phase, and assert on the artefacts produced —
   run across claude, kiro, and codex so a skill that works on all
   three is robust generally.

Not an initiative — one branch, one CR. All the skill changes edit
the same `kdevkit` skill surface; they are one focused diff, not
sequential streams.

## Requirements

<!-- Experience layer: what an agent/human using kdevkit observes
     differently. Smell test applied — internal file mechanics live
     in Design. -->

### R1 · Three-layer context model, explicitly taught

kdevkit declares three context layers and their homes:

- **operational** → repo-root `AGENTS.md` — build/test/lint commands,
  code style, PR/commit conventions (the community "README for
  agents");
- **project-knowledge** → `project.md` — mission, architecture, tech
  rationale, constraints, which test layer is load-bearing;
- **per-feature** → the feature spec — transient, exists only while
  the feature is in flight.

An agent reading both `project.md` and `AGENTS.md` does not see the
same build/test commands duplicated across both.

### R2 · kdevkit never corrupts AGENTS.md convention

Anything the agent writes that lands in a repo-root `AGENTS.md` still
reads as a normal, lean AGENTS.md to any tool or human. The agent
never writes kdevkit-internal scaffold — fixed section headers,
HTML-comment prompts, Session/Decision logs, the `## Active
initiatives` index — into `AGENTS.md`. That scaffold stays in
`project.md` and the spec tree.

### R3 · Operational commands defer to AGENTS.md where present

The §7 dev loop resolves build/test/lint commands from `AGENTS.md`
first, falling back to `project.md`'s Testing section where no
`AGENTS.md` exists — the same first-hit-wins spirit as the `specs/`
→ `docs/specs/` → `.kdevkit/` spec-tree detection.

### R4 · Closure bubbles durable content up out of the transient spec

§8 closure remains the promotion mechanism. Durable *facts* land in
`project.md`'s sections (existing §8.2); operational changes (a new
test command, a changed build step) land in `AGENTS.md` (new); and
binding cross-feature *decisions* — those that constrain future work
— get their rationale folded into the relevant `project.md` section
as the durable "why."

### R5 · Decisions live in the feature spec while worked on; no separate home

The feature spec's Decision Log is the working home for every
decision during the feature. There is no project-level ADR tree and
no standing decisions section — importance is what bubbles up (R4),
into existing `project.md` sections or `AGENTS.md`, not into a new
artefact. Non-binding decisions stay in the feature's Decision Log,
archived in place with the feature.

### R6 · Authoring guidance favors lean + concrete

The templates the agent fills nudge toward exact commands and
explicit boundaries over vague prose, and warn against
auto-generated bloat. Codifies kdevkit's existing compaction
discipline; grounded in the ETH AGENTbench finding that bloated /
auto-generated context files reduce agent success (~3%) and raise
cost (~20%).

### R7 · mAId's own AGENTS.md claims are accurate

mAId's `project.md` Architecture section and README describe
AGENTS.md adoption truthfully — they do not claim Kiro or Claude
Code as native-support tools on the agents.md roster (neither is
listed there), and they describe the Claude Code relationship as
symlink-bridged (which is mAId's actual belt-and-suspenders design:
`CLAUDE.md` / `AGENTS.md` at one merged source).

### R8 · The skill is verified to be *carried out* across three agents

The behavioral outcome an operator observes: running the kdevkit
verify suite exercises the skill on claude, kiro, **and** codex, and
each agent is shown to actually perform the methodology (produce the
right artefacts), not merely describe it. A skill change that passes
on all three is trusted to be robust; by default all are required.

### R9 · The tested agents are a run parameter

The operator can scope a verify run to a subset of coding agents:
`resources/tests/run --tools <list>` (e.g. `--tools codex`). Default
is all three, and all must pass. The requested set is also the
required set — a requested agent missing from PATH is a failure, and
agents outside the set simply don't run. This lets an operator isolate
one agent when another's backend is slow or flaky, without editing
fixtures.

## Test Strategy

<!-- V-model: functional/behavioral fixtures verify Requirements in
     user-observable (artefact) terms; unit tests verify Design
     primitives. Per project.md, the tri-tool verify suite is
     user-driven — the agent prepares and hands off. -->

This feature **dogfoods itself**: the skill change (R1–R7) is
verified through the new behavioral harness (R8). The harness rebuild
is therefore part of the same diff, not a follow-up.

### Layer 1 — `just test` (Rust unit; load-bearing §7 Test Gate)

Verifies Design primitives that are deterministic and cheap:

- **Registry gains codex entries** → the content-validator / symlink
  state-machine tests extend to cover `.codex/AGENTS.md` and
  `.codex/skills` deploy targets (mirrors the existing Kiro-entry
  coverage). Round-trip install→status→uninstall stays green against
  the tempfile-fake `$HOME`.
- **`tools:` parser accepts `codex`** → unit coverage for the
  fixture-field parse + required-tool resolution (codex failing is a
  fail, not a skip) if that logic moves into a testable boundary;
  otherwise asserted at Layer 2.

No content-validator or schema changes beyond the registry rows.

### Layer 2 — `resources::verify` (behavioral, tri-tool; user-driven)

The heart of the change. Two sub-parts:

**(a) Harness rebuilt for behavioral fixtures + three tools.**

- New fixture format (additive): a fixture may declare a `setup:`
  step that seeds a scratch repo (tmpdir with a seeded `project.md`,
  optional root `AGENTS.md`, a feature spec, a git init) and an
  `assert:` step that inspects the resulting working tree / commits
  after the agent runs. The legacy `expect_substr:` /
  `expected_narrative:` judge path stays for reasoning-shaped checks.
- Per-tool invocation lives behind **one indirection point**
  (`tool_invoke`) so a future CLI-flag change touches one function.
  codex arm: `codex exec` non-interactive, stdin closed, sandbox +
  git-check flags chosen for stability at implementation time.
- codex is a **required tool** for core fixtures: a codex failure is
  a `FAIL`. (kiro/claude keep current treatment; the required-set is
  explicit.)

**(b) The 17 recitation probes collapse to behavioral phase-fixtures.**

Restructure keyed to kdevkit *phases* (stable) not *rules* (grows):

| New behavioral fixture | Absorbs | Drives (asserts on artefacts) |
|---|---|---|
| `kdevkit-planning` | feature-loop, feature-planning, codebase-grounding, requirements-user-facing, initiative-recognition | seed repo + feature ask → spec written, planning entered, grounded in project.md, what/how split |
| `kdevkit-dev-loop` | dev-loop, review-gate, review-config-setup, comment-style, idiomatic-design-and-wiring, feedback-repin | green slice → gate order walked, re-pin on reactive change |
| `kdevkit-closure` | feature-closure, closure-after-long-session, closure-verify-and-anti-patterns, stream-closure, cross-stream-rebase | "ship it" → reconcile, bubble-up, `close(...)` commits, squash |
| `kdevkit-agents-md` (**new**) | — (R1–R4, R6) | repo w/ root AGENTS.md + project.md → reads command from AGENTS.md, AGENTS.md stays lean/uncorrupted, durable change bubbles to right layer |

Net **17 → 4**; fixture count now tracks phases, not rule-count.
`kdevkit.smoke` (the thin "what dir first" probe) is absorbed into
`kdevkit-planning`. Where an assertion is genuinely about reasoning
(e.g. "recognizes initiative-shape"), the fixture keeps a judge
sub-check rather than forcing an artefact.

### Success criteria

- `just test` green (registry + parser coverage added).
- The 4 behavioral fixtures pass on **claude, kiro, and codex**;
  the 13 absorbed fixtures are deleted (corpus shrinks).
- A deliberate skill break (e.g. remove the AGENTS.md-convention rule
  from SKILL.md) fails `kdevkit-agents-md` on all three tools —
  proving the fixture exercises behavior, not filesystem presence.
- `just resources::status` shows codex symlinks resolved after
  install.

### Cadence (user-driven, per project.md)

`resources::verify` now spends credits across three tools — the
agent prepares fixtures + harness, names the commands
(`just resources::verify`, `just resources::verify-one <name>`), and
hands off. The `[confirm]` gate stays. The agent's §7 Test Gate
remains `just test` only.

## Design

<!-- Rationale first. -->

### Why three layers, not a two-way split (research-grounded)

Deep research (this session; 25 claims verified 3-0, plus two
follow-up verification passes) established that the community did not
draw a two-way "commands in AGENTS.md, everything else in project.md"
line. It converged on **three layers**, and every surveyed tool
implements the middle layer as a *named, separate artefact*:

- operational → `AGENTS.md` (agents.md spec; lean by convention,
  reinforced by ETH AGENTbench "lean wins");
- project-knowledge → Kiro **steering** (`product.md`/`tech.md`/
  `structure.md`), spec-kit **constitution**
  (`.specify/memory/constitution.md`), Cline **memory-bank** (six
  files) — Böckeler names this the always-on "project knowledge"
  layer;
- per-feature → Kiro/spec-kit/OpenSpec **requirements→design→tasks**.

`project.md` *is* kdevkit's project-knowledge layer. So R1 positions
it as a peer of steering/constitution, not as a file to be merged
into AGENTS.md. The only real overlap is the command *strings* in
`project.md`'s Testing section — the one operational thing that
belongs in `AGENTS.md` — hence R3's defer-to-AGENTS.md rule.

### Why closure is the promotion engine (no new decisions tier)

The feature spec is the transient third layer; §8 closure already
lifts durable content out of it (§8.1 reconcile, §8.2 project.md
verify per touched section). The ecosystem's ADR convention
(`doc/adr/NNNN-*.md`) exists because most repos have *no* always-on
knowledge doc — kdevkit has one, so a separate ADR tree would be
redundant ceremony and would contradict the "single project.md,
lower ceremony" call already made in `sources-audit-and-kdevkit-v2`.
The lean move (honoring ETH) is to extend §8.2's promotion to also
target `AGENTS.md` for operational changes and to carry decision
*rationale* (not just facts) into project.md sections. No standing
decisions section that would bloat an always-on file.

### Why behavioral + tri-tool (dogfood the change)

The 17 fixtures test *recall of SKILL.md prose*, not *execution*.
They grow linearly with rules, paraphrase the source (so compaction
forces hand-patching), and can't catch the failure that matters — an
agent that recites §8 perfectly but dumps scaffold into AGENTS.md.
Behavioral fixtures that seed a repo and assert on artefacts test
kdevkit's actual purpose. Running them on three agents operationalizes
the robustness rationale: **if the skill drives claude, kiro, and
codex to the same correct artefacts, it is robust generally** — a
skill that leans on one tool's idiosyncratic prompt-following breaks
on the others.

### Skill-file diff shape (SKILL.md + deferred files)

- **§1 / §2** — teach the three-layer model; `project.md` positioned
  as the project-knowledge layer; add repo-root `AGENTS.md` detection
  as the operational-context home (first-hit-wins, mirroring
  spec-tree detection).
- **§7 dev loop** — "read commands from `project.md`" becomes "read
  build/test/lint from `AGENTS.md` where present, else `project.md`
  Testing." project.md Testing carries layer-semantics + load-bearing
  judgment, points to AGENTS.md for the command strings.
- **§8 closure** — §8.2 extended: promotion targets `AGENTS.md` for
  operational changes and folds binding-decision rationale into
  project.md; add the AGENTS.md-convention guard (never write kdevkit
  scaffold into it).
- **A new always-on subsection** (SKILL.md) — "Context layers &
  AGENTS.md convention" carrying R1/R2 as operational rules; the
  authoring-lean guidance (R6) lands as a terse rule near the
  templates. Per the future-feature placement rule: operational →
  SKILL.md; template/schema text → `setup.md`.
- **`setup.md`** — `project.md` template guidance notes the
  three-layer split + AGENTS.md relationship + lean/concrete
  authoring. No new required section.
- **Version bump** — signpost minor per repo convention.

### Registry + harness diff shape (the non-trivial infra)

- **`resources/build-tool/src/main.rs`** — REGISTRY gains
  `.codex/AGENTS.md` → `resources/content/agents.md` (plain symlink)
  and a **fan-out entry** for codex skills. Discovered mid-dev:
  `~/.codex/skills/` is codex-owned (ships `.system/` inside; user
  skills are siblings), so a whole-directory symlink is wrong (the
  build-tool correctly reports `BlockedByRealDir`). This adds a
  **new registry `kind`** to the build-tool — *fan-out*: symlink each
  child of a source directory into a target directory mAId does not
  own, leaving the tool's own entries (`.system/`) untouched. Symlink
  stays the deploy mode (source edits remain live); the fan-out kind
  applies the same primitive at child granularity. The existing
  whole-path entries become the default `kind`. Codex discovers
  skills by directory scan (presence-based, `~/.codex/skills/<name>/
  SKILL.md`), so per-child symlinks satisfy it natively — no
  config-file mutation, so no content-merge install is needed (that
  heavier mode, already present in `kaimux` for settings.json hooks,
  isn't warranted here). Extend the table-driven deploy tests to
  cover both the symlink row and the fan-out kind (children linked,
  `.system/` preserved, round-trip clean).
- **`resources/tests/run`** — add the codex arm behind `tool_invoke`
  / `tool_available` / fixture-loop `case`; add codex telemetry-noise
  stripping alongside the kiro ANSI strip; implement the `setup:` /
  `assert:` behavioral fixture format; introduce the required-tool
  set (codex fails, not skips, for core fixtures).
- **`resources/content/agents.md`** — verify the global preamble
  still deploys cleanly to the new codex target (it's the same merged
  source).

### mAId content-accuracy diff (R7)

- **`specs/project.md`** Architecture + **`README.md`** — correct the
  AGENTS.md adoption wording (drop Kiro/Claude Code from any
  "native-support roster" claim; describe Claude Code as
  symlink-bridged). Small wording edits; land with the close-out
  since §8.2 already verifies project.md.

### Trade-offs considered

- **Full behavioral now vs. judge-on-transcript now + artefacts
  later.** Full behavioral chosen (user direction) — dogfood the
  skill change through the new harness in one branch. Cost: larger
  diff, a fixture-format redesign inside this feature. Accepted
  because the two halves validate each other.
- **codex required vs. skip-if-absent.** Required (user direction).
  Cost: a broken/misconfigured codex reds the suite. Accepted — the
  robustness claim depends on all three actually running.
- **Invocation pinned in spec vs. chosen at implementation.** Chosen
  at implementation, recorded in Decision Log — the exact codex flags
  may change across CLI versions; the *need* (drive three agents) is
  stable, so the invocation lives behind one indirection point.
- **Separate ADR tree vs. bubble-up into project.md.** Bubble-up (no
  new tier) — kdevkit already has an always-on knowledge doc; a
  second decisions artefact is redundant and fights the lean-file
  discipline.

## Implementation Plan

<!-- One slice per item. Tick in the commit that completes the slice.
     Ordered so the repo stays working at every commit. -->

- [x] **1 · Skill change: three-layer model + AGENTS.md convention.**
  Edit `resources/content/skills/kdevkit/SKILL.md` (§1/§2 layer model
  + AGENTS.md detection; §7 command-resolution defer; §8.2 promotion
  extension + convention guard; new always-on "Context layers &
  AGENTS.md convention" subsection; lean/concrete authoring rule).
  Edit `setup.md` project.md template guidance. Version bump.
  `just ci` (markdown — fmt may rewrap; lint/check no-op).
- [x] **2 · Registry: codex deploy targets + fan-out kind.** Add a
  registry `kind` (default whole-path symlink vs. fan-out per-child
  symlink) to `resources/build-tool/src/main.rs`; add `.codex/AGENTS.md`
  (symlink) and `.codex/skills` (fan-out) rows; extend table-driven
  deploy tests to cover the fan-out kind (children linked, tool-owned
  `.system/` preserved, round-trip clean); `just test` green; `just
  resources::install` + `just resources::status` show codex targets
  resolved without clobbering codex's own skills.
- [ ] **3 · Harness: codex arm + one indirection point.** Add codex
  to `tool_invoke` / `tool_available` / fixture-loop `case`; codex
  telemetry-noise strip; validate the stable invocation live
  (`codex exec` flags); introduce the required-tool set. Prove a
  trivial fixture passes on codex.
- [ ] **4 · Harness: behavioral fixture format.** Implement `setup:`
  (seed scratch repo) + `assert:` (inspect working tree/commits);
  keep the legacy judge path. Unit-cover the `tools:`/required-set
  parsing.
- [x] **5 · Rebuild fixtures: 17 → 4 behavioral.** Author
  `kdevkit-planning`, `kdevkit-dev-loop`, `kdevkit-closure`, and the
  new `kdevkit-agents-md`; `git rm` the 13 absorbed probes. Each
  seeds a repo, drives a phase, asserts artefacts; `tools:
  claude,kiro,codex`.
- [ ] **6 · mAId accuracy (R7).** Correct `specs/project.md`
  Architecture + `README.md` AGENTS.md-adoption wording.
- [x] **7 · Quality + Test Gate + tri-tool behavioral verify.** `just ci` + `just test` green.
  Prepare the tri-tool verify commands and **hand off** to the user
  (per project.md user-driven rule) — do not spend credits
  autonomously.
- [ ] **8 · Code Review Gate.** host-native reviewer, threshold 70,
  hard-stop, retry-budget 2. Reviewer sees project.md + diff, no
  feature spec.
- [ ] **9 · Push + Agent-dev Review Gate.** Body: Approach + Reading
  order (SKILL.md as intent; harness + registry as contract; fixtures
  as plumbing).

<!-- Risk notes -->

- *Risk — behavioral fixtures are slow/flaky across three tools.*
  Each seeds a repo and runs a full agent turn ×3 tools ×4 fixtures.
  Mitigation: user-driven cadence absorbs the cost; `verify-one`
  isolates a single fixture; judge sub-checks kept narrow.
- *Risk — codex sandbox quirks (bubblewrap warning, git-check).*
  Mitigation: invocation behind one indirection point; pick the
  stable sandbox flag at implementation; required-set can be relaxed
  to skip if codex proves environment-fragile (would revisit the
  "codex required" decision with the user).
- *Risk — assert-on-artefacts is a real harness redesign bundled with
  a skill change.* Mitigation: commits 1–2 (skill + registry) stand
  alone and keep the repo working before the harness rebuild lands;
  if commit 4/5 overrun, the skill change is still shippable and the
  fixture rebuild could split to a follow-up (would confirm with the
  user, since bundling was the explicit ask).
- *Risk — deliberate-break check is manual.* The success-criterion
  break test (remove a rule → fixture fails) is a hand-run sanity
  check, not a permanent test. Accepted.

## Session Log

<!-- append: date · what was done · decisions made -->

- 2026-07-16 · Dev + tri-tool verify · Slices 1–6 landed (skill
  3-layer/AGENTS.md convention; codex fan-out registry kind + tests;
  codex harness arm + required-tool notion; behavioral setup/assert
  fixture format; 17→4 fixture rebuild; mAId adoption-wording fix).
  `just ci` green (build-tool 38 tests incl. 4 fan-out; kaimux 53).
  Force-installed the worktree to all three tools (restored to main
  after verify) so claude/kiro/codex all read 3.6.0. Ran the full
  behavioral verify across all three agents. Result: **agents-md
  PASS×3, dev-loop PASS×3, planning PASS (claude+kiro; codex slow)**.
  Two FAILs — **kdevkit-closure on claude and codex** — were a
  *fixture* defect, not a skill defect: the seed asserted the plan was
  done but the seeded code didn't evidence the two plan items, so
  claude/codex correctly refused to tick unconfirmed boxes (kdevkit
  §8.1 discipline firing) while kiro ticked them. A fixture a *correct*
  agent fails is mis-designed. Fixed the seed to include `due.py` that
  visibly implements both plan items so reconcile is unambiguous.
  Observations for the CR: (a) codex's judge/behavioral runs emit
  verbose tool-call narration to stdout — verdict extraction still
  works but the harness could strip codex's `exec`/telemetry noise more
  aggressively; (b) codex is markedly slower (bubblewrap-fallback
  sandbox), which is why it trails in the matrix.

- 2026-07-16 · Closure fixture fixed + matrix green · Re-seeded
  kdevkit-closure with `due.py` that visibly implements both plan items
  so reconcile is unambiguous; also `grep -F --` guard for the `- [ ]`
  pattern. Re-ran across all three: **kdevkit-closure now PASS on
  claude, kiro, codex.** Full kdevkit matrix green (4 fixtures × 3
  tools = 12/12), plus notes/writing-style regression fixtures green —
  `all fixtures passed`. The skill is verified carried out across all
  three agents (R8 satisfied).

- 2026-07-17 · `--tools` parameter added (R9) · A codex-only re-run
  (after a host bwrap/install fix) showed codex's backend can be slow
  or reconnect-loop while claude/kiro are fine — a single flaky agent
  shouldn't hold the suite hostage, and isolating one agent shouldn't
  need fixture edits. Added `resources/tests/run --tools <list>`:
  defaults to all three (all required), narrows to a subset on request;
  the requested set is the required set (absent → fail, out-of-set →
  don't run). Validated: unknown/empty values rejected (exit 2);
  `--tools claude kdevkit-agents-md` runs exactly one agent and passes;
  scoping logic unit-checked under bash for default/single/subset.
  `just ci` green (40 + 53).

- 2026-07-16 · Code Review Gate · Fresh-context review (project.md +
  diff, no feature spec) scored **82/100** (threshold 70 — passes).
  Three findings, all addressed: (MAJOR) fan-out uninstall/status
  enumerated only the source dir, so a skill renamed/removed in source
  orphaned a dangling symlink in codex's dir — fixed `expand()` to also
  scan the home dir for managed symlinks (union by home path) and
  reordered `plan_one` so an orphaned managed symlink is a reapable
  Match even when its source is gone; (MINOR) `uninstall --force` could
  `remove_dir_all` a codex-owned real dir sharing a skill name — mAId
  only ever installs symlinks, so a real dir at a managed path is now
  never removed regardless of `--force`; (MINOR) `prompt:`/`tools:`
  greps lacked `|| true`, so under `set -e` a missing line aborted the
  suite and made the malformed-guard dead code — added `|| true`. Two
  new regression tests (orphan reaping, force-refuse real dir); 40
  build-tool tests green.

- 2026-07-16 · Planning · Promoted from backlog
  (`agents-md-ecosystem-alignment`) in a fresh worktree. Ran the four
  §6 interviews. Grounding: deep-research report (25 claims 3-0) plus
  two follow-up verification agents on (a) steering/knowledge layer
  vs AGENTS.md and (b) where dev-methodology/process lives + the
  epic/initiative gap. Key reframes from the user during interviews:
  (i) three-layer model, project.md = the steering layer, not a
  two-way split; (ii) decisions live in the feature spec and bubble
  up at closure — no separate ADR tier; (iii) test strategy reframed
  to kdevkit's *purpose* — verify the skill is carried out across
  claude/kiro/codex, restructure the 17 fixtures behaviorally; (iv)
  full-behavioral + bundled, dogfood the skill change through the new
  harness; (v) codex is a required third tool, fix the harness until
  it works; invocation may change across versions so hide it behind
  one indirection point. Confirmed on-disk: codex reads
  `~/.codex/skills/` + `~/.codex/AGENTS.md` and has no registry
  entries yet — registry work is real. Verified codex invocation:
  `codex exec --sandbox read-only --skip-git-repo-check "<prompt>"
  </dev/null`. Stopping at the Planning Review Gate.

## Decision Log

<!-- append: decision · rationale · alternatives rejected -->

- **Three-layer context model, project.md = project-knowledge
  layer.** Rationale: the community (Kiro steering, spec-kit
  constitution, Cline memory-bank; Böckeler's SDD taxonomy) converged
  on a named middle layer separate from operational AGENTS.md;
  project.md already *is* that layer. Alternatives rejected: (a)
  two-way "commands→AGENTS.md, rest→project.md" split — not what the
  ecosystem does; (b) merge project.md into AGENTS.md — breaks the
  lean-operational convention and the always-on/knowledge separation.
- **No separate ADR/decisions tier; closure bubbles decisions up into
  project.md.** Rationale: kdevkit already has an always-on knowledge
  doc, so ADR's separate-tree rationale doesn't apply; a standing
  decisions section would bloat an always-on file (ETH "lean wins").
  Alternatives rejected: `$SPEC_ROOT/decisions/NNNN-*.md` tree
  (redundant ceremony, contradicts single-project.md call);
  ADR-lite `## Key decisions` section (still accretes in an always-on
  file).
- **Full-behavioral fixtures + bundled with the skill change (dogfood).**
  Rationale (user direction): the skill change and the harness that
  proves it validate each other in one branch. Alternative rejected:
  judge-on-transcript now, artefact-assertion as a follow-up feature
  — cleaner scoping but doesn't dogfood, and defers the real test of
  "is the methodology carried out."
- **Fixtures restructured by phase (17 → 4), not by rule.** Rationale:
  rule-keyed fixtures grow without bound and paraphrase the source;
  phase-keyed behavioral fixtures are stable and test execution.
  Alternative rejected: add ~7 new recitation probes for R1–R7 —
  compounds the exact problem being fixed.
- **codex is a required third tool; invocation behind one indirection
  point.** Rationale (user direction): robustness across three agents
  ⇒ robustness generally; a skill leaning on one tool's prompt-following
  breaks on others. The exact CLI flags may change across versions, so
  the *need* (drive 3 agents) is pinned, the *command* is not.
  Alternative rejected: codex skip-if-absent like kiro — wouldn't
  guarantee the third agent actually runs, weakening the robustness
  claim.
- **Registry gains a fan-out kind; symlink stays the deploy mode
  (plan amendment, slice 2).** Discovered mid-dev that `~/.codex/skills/`
  is codex-owned (ships `.system/`; user skills are siblings), so the
  whole-directory symlink the original plan assumed is wrong — the
  build-tool reports `BlockedByRealDir`. Investigated symlink vs.
  idempotent-install as the deploy mode (user question). Finding: we've
  hit the first tool that owns its skills dir, but codex discovers
  skills by directory scan (not config registration), so per-child
  symlinks satisfy it — no content-merge needed. Chose to add a
  **fan-out symlink kind** (link each source child into a
  not-owned target dir, preserving the tool's own entries) rather than
  (a) whole-dir symlink (clobbers `.system/`), (b) enumerate per-skill
  registry rows (breaks "add a skill = no registry edit"), or (c) a
  full idempotent copy-and-merge install (heavier than needed; that
  mode already exists in `kaimux` for settings.json hooks and isn't
  warranted for presence-based discovery). Symlink remains the mode —
  source edits stay live — applied at child granularity. Bundled into
  this feature (user direction) since it owns "codex as a real third
  tool."
- **codex invocation chosen at implementation, not pinned in spec.**
  Rationale: `codex exec` flags (sandbox mode, git-check) are
  version-sensitive; recording the working form in the Decision Log at
  implementation time keeps the spec durable. Working form as of
  planning: `codex exec --sandbox read-only --skip-git-repo-check
  "<prompt>" </dev/null`.
