# Feature: cr-reading-order

## Git Setup

- Branch: `feat/cr-reading-order`
- Base: `main` (e4bc596)

## Feature Brief

Promote the optional "Reading guide" line in kdevkit §7 + §8 to a
required **Reading order** section in §6, §7, and §8 — always-on,
grouped by phase (*Read for intent / Read for contract / Read
for plumbing*). Forces top-down review paths into every PR/CR
the agent opens, and gives the agent a forcing function for
catching inverted-order issues during CR composition.

Bundled hygiene move: deprecate the
`specs/backlog/maid-as-flake-package.md` backlog item — its
proposal conflicts with current `project.md` Hard constraints
(no installed binary on `$PATH`, no global state mutation),
and the parenthetical in `project.md:181-183` that cites it as
"the future flake-package shape" needs to go with the file so
nothing dangles.

## Requirements

1. **Always-on Reading order across all three review gates.**
   §6 Planning, §7 Agent-dev, §8 Closure each carry a required
   **Reading order** section in the CR body.
2. **Grouped-by-phase format.** Three buckets: *Read for
   intent:* (specs / project.md / feature briefs), *Read for
   contract:* (the load-bearing source — SKILL.md, registry,
   schema), *Read for plumbing:* (mechanical / fixture /
   chore changes). Buckets may be empty for trivial CRs but
   the section header is mandatory.
3. **§7 body shape change.** Promote *Reading guide*
   (currently optional, line-shape: file order with
   compare-against hints) to **Reading order** required;
   drop the "Don't impose more structure on small diffs"
   sentence.
4. **§8 body shape change.** Same promotion at closure.
5. **§6 body shape change.** Add Reading order between Spec
   summary and Open questions.
6. **Smoke fixtures match the new contract.** The two
   judge-narrative `.smoke` fixtures that mention "Reading
   guide" describe it as required-and-grouped instead of
   optional.
7. **Backlog deprecation.** `specs/backlog/maid-as-flake-package.md`
   removed; the parenthetical in `specs/project.md` Hard
   constraints (lines 181–183) trimmed so the bullet ends at
   "no `nix profile install` anywhere in the install path."
8. **Version bump.** `sources/skills/kdevkit/SKILL.md`
   frontmatter `version: 2.5.0 → 2.6.0`.

## Test Strategy

Per `project.md` Testing section (four-layer surface; agentic
runs stop at `test:smoke`):

- **`deno task test:unit`** (load-bearing) — schema parsing
  + deploy/undeploy invariants. Edits are markdown-only, so
  the suite should remain green; this is the §7 Test Gate
  default.
- **`deno task test:smoke`** (after deploy) — confirms the
  new SKILL.md reaches `~/.claude/skills/kdevkit/SKILL.md`
  through the symlink. Run after the §7 Quality+Test pass
  to verify deploy invariants survived the markdown edit.
- **`deno task test:functional`** (judge mode) — *user-driven
  per project convention; agent does not run it*. Two
  fixtures evaluate the new contract:
  - `kdevkit-dev-loop` — must accept the new "Reading order
    grouped by phase is required" narrative.
  - `kdevkit-feature-closure` — must accept the new closure
    body shape narrative.

Quality gate: `deno task fmt` + `deno task lint` + `deno task
check` after the implementation slice.

Success criterion: every CR opened by an agent following
kdevkit v2.6.0 carries a grouped-by-phase Reading order in
all three review gates. Self-applicable: this feature's own
§7 + §8 CRs must demonstrate the contract.

## Design

Single-source-of-truth edits live in
`sources/skills/kdevkit/SKILL.md`. Three review-gate paragraphs
change shape:

- **§6 Planning Review Gate** (current line 316):
  `Body: Why + Spec summary (R/T/D/I one-liners) + Open questions`
  → `Body: Why + Spec summary (R/T/D/I one-liners) + Reading
  order (grouped by phase) + Open questions`
- **§7 Agent-dev Review Gate** (current lines 379–385):
  Promote *Reading guide* (optional) to **Reading order**
  (required); drop the "Don't impose more structure on small
  diffs" sentence — the always-on Reading order is the
  structural floor.
- **§8 Closure Review Gate** (current lines 424–430): Same
  promotion; *Reading order* moves out of the optional list
  into the required body shape.

Format chosen: grouped-by-phase prose (`*Read for intent:* … ;
*Read for contract:* … ; *Read for plumbing:* …`) over a
numbered `path:hint` list. Rationale: the grouped form
encodes the top-down structure explicitly and is shorter for
small CRs (the buckets can collapse to single entries) while
still scaling to multi-package CRs (CR-2 sp-api-turing was the
motivating example — project.md → contract spec → per-package
amendments → chore commits).

Backlog deprecation lives in two file edits:

- `git rm specs/backlog/maid-as-flake-package.md` (whole file).
- `specs/project.md:181-183` — remove the parenthetical
  starting "(Install/uninstall of a `maid` binary is
  reserved…" so the Hard constraints bullet ends cleanly.

Trade-off considered and rejected: keeping Reading order
optional with a size threshold (≥3 files or ≥2 commits) was
the backlog's original framing. The user's explicit guidance
("the skill should have minimal surface so that we don't
waste tokens; always on if simplest is what works") removed
the threshold knob in favour of an unconditional contract.
The judge-mode fixtures will catch regressions if the
always-on rule produces awkward small-CR bodies.

## Implementation Plan

- [x] **Plan-commit + Planning Review Gate.** Committed as
  `plan(cr-reading-order): initial spec` (9999bad); PR #13
  opened with §6 body shape.
- [x] **§6/§7/§8 body-shape edits in SKILL.md.** Shipped as
  `feat(kdevkit): require grouped Reading order in §6/§7/§8`
  (3e7a7e9). Version → 2.6.0. Dropped "Don't impose more
  structure" sentence.
- [x] **Smoke fixture updates.** Shipped as `test(kdevkit):
  match grouped Reading-order contract` (00ef2a8).
- [x] **Backlog deprecation.** Shipped as `chore(specs):
  drop deferred flake-package backlog` (906c66a) — file
  removed and project.md parenthetical trimmed in one
  commit.
- [x] **Quality + Test Gate.** fmt + lint + check clean;
  `test:unit` 22/22; `test:smoke` 9/9.
- [x] **Push + Agent-dev Review Gate.** PR title + body
  rewritten to §7 shape with grouped Reading order over the
  three commits (the self-applicability check).
- [x] **Refactor per CR feedback.** Factored the universal
  CR contract into a new §9 "Review Gates" subsection,
  trimmed §6/§7/§8 to phase-specific lines + `Apply §9
  Review Gates`. Dropped three "Apply: §9 internal-marker
  grep" footers. Shipped as `refactor(kdevkit): factor CR
  body shape into §9 Review Gates` (769b4f3).
- [x] **Judge-mode functional A/B.** User-approved
  `test:functional` run after the §9 refactor:
  `kdevkit-dev-loop` and `kdevkit-feature-closure` both
  PASS judge mode on Claude AND Kiro. Concrete evidence
  the simplification preserved agent behavior.
- [ ] **Closure** on user cue (in progress).

Risk notes:

- Smoke fixtures are judge-mode; the narrative wording is
  load-bearing for the test, but the agent does not run
  judge mode — user does. Risk: I propose narrative wording
  the judge rejects on a later run. Mitigation: keep the new
  wording structurally close to the existing fixture style
  (already grep'd; both fixtures use "required / optional"
  phrasing).
- The `kdevkit-feature-planning.smoke` fixture does NOT
  currently mention CR body shape. Skipping it as out of
  scope; if §6's contract becomes testable in a future
  fixture revision, that's a follow-up.
- Self-applicability: the §7 + §8 CRs for this feature are
  the first to be written under v2.6.0. The agent must
  emit a grouped Reading order on its own CR — this is
  load-bearing evidence the contract is implementable.

## Session Log

<!-- append: date · what was done · decisions made -->

- 2026-06-01 · §8 closure entered. Functional A/B fixtures
  ran user-approved on both Claude + Kiro: `kdevkit-dev-loop`
  and `kdevkit-feature-closure` both PASS substr + judge.
  Judge narratives confirmed Why + Approach + grouped
  Reading order body shape and full eight-step closure
  ordering — concrete evidence the §9 refactor preserved
  agent behavior across both harnesses.
- 2026-06-01 · §9 refactor shipped after PR review feedback
  ("do we need to repeat the CR shape … move to cross-cutting
  and reference?"). Factored the universal CR contract into
  a new §9 "Review Gates" subsection; §6/§7/§8 trimmed to
  phase-specific lines + `Apply §9 Review Gates`. Net 38/44
  ins/del; behavior preserved (verified by the §9 fixture
  run above).
- 2026-06-01 · v2.6.0 contract + smoke fixture updates +
  flake-package backlog deprecation shipped across three
  commits on PR #13.
- 2026-06-01 · feature spec written from approved plan; branch
  `feat/cr-reading-order` cut off `main` (e4bc596). Three
  open questions from the original backlog resolved at plan
  time: trigger = always-on (no threshold); format =
  grouped-by-phase; gates = §6 + §7 + §8. Maid-as-flake-package
  bundled in as backlog deprecation per user direction.

## Decision Log

<!-- append: decision · rationale · alternatives rejected -->

- 2026-06-01 · **Always-on Reading order, no threshold.**
  Reason: minimum agent surface, fewer tokens, no
  decision-cost on every CR. Alternative rejected: ≥3-file
  threshold (the backlog's default proposal) — punted
  because the agent then has to count files and decide,
  and the savings on small CRs don't justify the
  branching logic in the skill.
- 2026-06-01 · **Grouped-by-phase prose, not numbered
  list.** Reason: encodes top-down structure explicitly;
  shorter for small CRs; scales to multi-package CRs.
  Alternative rejected: `1. path — hint` numbered list
  (backlog default) — works but doesn't *teach* the
  reviewer the intent / contract / plumbing split that's
  the point.
- 2026-06-01 · **Bundle backlog deprecation into this
  feature.** Reason: the backlog file's premise
  ("install `maid` system-wide via flake-package") now
  conflicts with `project.md` Hard constraints, and the
  parenthetical in project.md that cites it as "future"
  has no concrete path forward. Cheaper to delete both
  in one feature than open a follow-up. Alternative
  rejected: leave the backlog as a deferred future
  shape — the user's review marked it "not applicable
  with the way we have designed maid now."
- 2026-06-01 · **Factor CR contract into §9 cross-cutting,
  reference from §6/§7/§8.** Reason: PR review surfaced
  that v2.6.0 inlined the same Why + Reading order +
  optional Verification/Pairs-with shape three times
  across the gates — duplication that drifts. Single §9
  source of truth makes the skill easier for humans to
  scan and gives the agent one contract to load instead
  of three near-identical paragraphs. Alternative rejected:
  reply explaining why I kept it inline (each gate
  self-contained) — the gate-specific content was small
  enough that the duplication wasn't earning its weight.
- 2026-06-01 · **Judge-mode A/B as the §9 refactor's
  evidence gate.** Reason: the §9 consolidation is a
  prose refactor, but the contract it carries is what the
  agent emits at every gate. `test:unit` can't catch a
  drift in agent behavior; only `test:functional` (judge
  mode) can. User explicitly approved spending the
  credits to get the simplest skill that the agent
  still works correctly with. Both fixtures passed on
  both harnesses — refactor goes to closure.
