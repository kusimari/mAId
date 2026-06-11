# Feature: kdevkit-functional-style-and-idiomatic-libs

## Git Setup

- Branch: `feat/kdevkit-functional-style-and-idiomatic-libs`
- Base: `main` at `e6b8273` (post resources-and-kaimux restructure)

## Feature Brief

Teach the kdevkit skill **two distinct rules** that fire at two
different phases of the dev workflow, so the agent reaches for
elegant, idiomatic solutions instead of re-deriving logic by
hand:

1. **Design-time discovery (§6).** Before settling on *how* a
   piece of work is built, actively look at what the language
   and ecosystem already offer, and surface the well-known
   library or idiom that already does the job. The
   justification is **inherited expertise** — a battle-tested
   dependency encodes edge cases and practice the agent would
   otherwise re-derive badly — *not* DRY. This is a research
   move during design, not a coding-style preference.

2. **Dev-time wiring (§7).** When writing the code, wire the
   logic in the shape the language and surrounding codebase
   already speak — functional / fluent where that reads as
   *what needs to be done* (chains over mutable accumulators;
   library calls over hand-rolled state machines), reaching
   first for what's already in reach. Read for intent, not
   mechanism.

The drift this guards against (per the backlog's What/Why):
design declares "validate frontmatter," dev writes a 50-line
hand-rolled YAML-ish parser, when the honest implementation
was "load YAML with the well-known parser + a struct." The
mismatch is invisible at planning time and only surfaces at
review when a reader asks "why is this so much code for so
little work." The same pattern hits filesystem walks, state
machines, and error accumulation.

Both rules land in **always-on `SKILL.md`** (not the deferred
`interviews.md`) so they fire on continue / pick-up entries,
not only on fresh-feature start. The §6 Design interview
prompt in `interviews.md` is sharpened to ask the discovery
question at interview time as well.

Out of scope: per-language idiom tables (the rule stays
generic — "the idiom *this* language speaks"); a §7 Code
Review Gate change (the gate dispatches host-native review
whose prompt we don't control, and the reviewer's context
deliberately excludes the feature spec, so a rule there
wouldn't reach the in-loop reviewer); migrating existing
specs.

## Requirements

<!-- The experience layer — for a skill change, the cues the
     agent recognises and the artefacts it produces. The
     "user" of a skill is the agent reading it; the
     observable surface is what the agent says and does when
     the skill is loaded. Smell test (SKILL.md §6): the names
     of the *skill's own* sections (§6, §7) are part of the
     skill's surface, so naming them here is fine; what stays
     out is the diff-level detail (which file, which line) —
     that's Design. -->

What an agent operating under the updated kdevkit does
differently, observably:

### Design-time discovery (the §6 behavior)

- When designing a piece of work whose "how" is non-trivial,
  the agent **names the well-known library or language idiom
  that already does the job** before committing to a
  hand-rolled implementation. For the backlog's canonical
  case ("validate frontmatter"), the agent surfaces "load
  YAML with the established parser + a typed struct" rather
  than describing a hand-written parser.
- The agent **justifies the choice by inherited expertise**
  — the library encodes vetted edge-case handling and
  community practice — explicitly *not* by "don't repeat
  yourself."
- The discovery move is **gated on "well-known and earns its
  weight."** The agent does not propose pulling a new
  dependency for a trivial job that a few honest lines or an
  already-present import handles; it weighs the dependency's
  weight against the hand-rolled alternative and says so when
  the hand-roll wins (the xshell-vs-duct case from the
  backlog).
- The agent records the alternative it weighed as a
  **Decision Log nudge** for load-bearing design choices
  ("considered X library; chose hand-roll because …" or vice
  versa) — recommended, not mandatory for every helper.
- The discovery step extends §6's existing **"Ground first"**
  preamble: alongside surveying the codebase corners the
  feature touches, the agent surveys what the language /
  ecosystem already offers for the problem.

### Dev-time wiring (the §7 behavior)

- When writing an implementation slice, the agent **frames
  each function around what a caller would say it does**, then
  wires the logic in the shape the language and surrounding
  codebase already speak — defaulting to functional / fluent
  (chains, iterator combinators, library calls) over
  hand-rolled mutable state machines **when that reads more
  clearly as the intent**.
- The agent **reaches first for what's already in reach** —
  stdlib, an existing dependency, an already-imported helper —
  over hand-rolling equivalent logic, and **matches the
  surrounding code's conventions** rather than importing a
  foreign style.
- The rule is **legibility, not dogma**: the agent does not
  force a fluent chain where a typed pattern-match or a plain
  loop is the honest, clearer tool, and does not refactor
  working code between equivalent forms without a readability
  or correctness gain.

### Scope / framing the agent observes

- The agent treats these as **two rules at two phases**, not
  one rule mentioned twice: discovery is a design-time
  research move (§6); idiomatic wiring is a dev-time
  coding-style default (§7).
- The agent keeps the rule **language-agnostic** — it speaks
  of "the idiom this language / codebase already offers,"
  never a fixed per-language library list.
- The function-level "what a caller would say it does" is the
  **caller's intent** (a different layer from feature-level
  Requirements), so the agent does **not** fold this into the
  §6 Requirements smell test — the two stay distinct.

## Test Strategy

<!-- V-model: the contract change is "what the agent says/does
     when the skill is loaded." That's a functional signal,
     verified by a judge-mode smoke fixture. Unit tests
     (build-tool) are unaffected — only markdown content
     changes — but must stay green as a regression net. -->

### Functional / Integration (the contract signal)

**New fixture:
`resources/tests/skills/kdevkit-idiomatic-design-and-wiring.smoke`**
— judge mode (`prompt:` + `expect_substr:` + `expected_narrative:`
+ `tools: claude,kiro`), matching the house style of
`kdevkit-requirements-user-facing.smoke`.

The prompt presents the canonical drift case and asks the
agent to cover both phases. Draft prompt:

> Load the `kdevkit` skill … Begin with the literal line
> `[kdevkit] applies`. Then answer in 6–10 sentences: I'm
> about to design and build a feature whose spec says
> "validate the frontmatter of a markdown file." Walk me
> through (a) what you do at **design** time before deciding
> how to build it, and (b) how you write the **code** for it
> once the design is set.

The `expected_narrative` credits four behaviors:

1. **Design-time discovery as an active research move.** The
   agent says it looks at what the language / ecosystem
   already offers and surfaces the well-known library/idiom
   (for this case: load YAML with the established parser +
   a typed struct) before committing to a hand-rolled
   approach. Naming the specific library is not required;
   "the well-known YAML library for this language" suffices.
2. **Justified by inherited expertise, not DRY.** The agent
   grounds the choice in the library encoding vetted
   edge-cases / community practice — explicitly not "to avoid
   repeating code." An answer that justifies it *only* as DRY
   is a wrong answer.
3. **Guarded — well-known and earns its weight.** The agent
   notes it weighs the dependency's weight against the
   hand-rolled alternative and won't pull a heavy/unknown dep
   for a trivial job (or names that some jobs are lighter
   hand-rolled). Records the alternative considered (Decision
   Log nudge).
4. **Dev-time idiomatic wiring.** The agent describes writing
   the code in the shape the language/codebase already speaks
   — functional/fluent or library calls over a hand-rolled
   state machine, reaching for what's already in reach,
   reading as intent — *and* frames this as legibility not
   dogma (won't force a chain where a plain loop / typed
   match is clearer).

Wrong answers: hand-rolling a parser without looking at what
exists; justifying library use solely as DRY; proposing a new
heavy dependency for a trivial job without weighing it;
collapsing the two phases into one (no distinction between
"look at design time" and "wire idiomatically at dev time");
prescribing a fixed per-language library list; forcing a
functional chain as a rigid rule rather than a legibility
default.

### Existing functional smokes — regression net

The other 13 kdevkit fixtures must stay green. Audit each
`expected_narrative` for assertions that would conflict with
the new §6 subsection or the §7 dev-loop preamble:

- `kdevkit-codebase-grounding.smoke` — asserts the §6 "Ground
  first" behavior (reads project.md, scans specs, surveys
  codebase, no new artefact). The discovery step *extends*
  Ground-first; confirm the fixture's "four behaviors" aren't
  made wrong by an added "survey the ecosystem" clause (it
  shouldn't be — the fixture lists what the agent MUST do, and
  adding a survey clause doesn't negate any of them). Patch
  only if a literal contradiction surfaces.
- `kdevkit-dev-loop.smoke` — asserts §7's five gates in order.
  The new dev-loop *preamble* sits before the Quality Gate and
  doesn't add or remove a gate; confirm the "five gates"
  assertion is untouched. Expected: no change.
- Remaining 11 — focus on planning / closure / initiative /
  review mechanics; expected no overlap.

### Unit tests — regression net

`just test` (`cargo test --workspace`) — the build-tool unit
suite (content validator + symlink state machine). Only
markdown content changes land; no Rust changes. All tests
must stay green. No new unit test (the contract signal is the
judge fixture, not a unit boundary). The content validator
may assert SKILL.md structure — if it checks heading
presence/order, confirm the new subsections don't trip it.

### Smoke / install — symlink resolution

`just install` then `just status` confirms the kdevkit
symlinks still resolve after the content edits. The
build-tool's `structural_install_to_real_directory_layout`
integration test covers the install round-trip in the
fake-`$HOME`; it runs under `just test`.

### Functional tests are user-driven (per project.md)

Per project.md's "Functional tests are user-driven" rule, the
agent does **not** run `just verify` / `just verify-one`
autonomously — it costs API credits and is gated behind a
`[confirm]` prompt. The agent prepares the fixture and names
the exact command
(`just verify-one kdevkit-idiomatic-design-and-wiring` for
the new fixture; `just verify` for the full surface) and
hands off. The user runs it. This feature carries **no**
per-feature override — the change is small-surface and the
user-driven cadence is fine.

### Quality gate

`just fmt-check && just lint && just check` after the content
+ fixture edits. The edited files are markdown and a `.smoke`
text fixture — `cargo fmt`/`clippy`/`check` operate on the
Rust crate only, so these are effectively no-ops for this
diff but must stay green (and catch any accidental Rust
touch). `just ci` bundles all four + `just test`.

## Design

<!-- The how layer. Lead with rationale: why two rules in two
     sections, why always-on, why no §7-gate change. -->

### Why two rules in two sections, not one

The user's framing is explicit: discovery and wiring are
**different moves with different justifications at different
times**. Conflating them loses signal. Design-time discovery
is an *active research action* justified by **inherited
expertise** (a well-known library encodes vetted practice);
dev-time wiring is a *coding-style default* justified by
**legibility** (code reads as intent). Encoding them as one
bullet would force one justification onto both and blur when
each fires. So: §6 gets the discovery rule, §7 gets the
wiring rule.

This also answers the backlog's open question #1 (where it
lands): **both** §6 and §7, as two distinct rules.

### §6 placement — new always-on subsection

A new subsection in §6, sibling to the existing **"Requirements
smell test (always-on)."** The two pair naturally and form a
matched set:

- The **smell test** keeps library names *out* of
  Requirements ("a library is a *how*, not a *what*").
- The **new discovery rule** puts the *right* library *into*
  Design (actively go look; surface the well-known one).

Proposed heading: **"Reach for what exists (design-time,
always-on)."** Content:

1. The discovery move — before deciding *how*, survey what the
   language / ecosystem already offers; surface the well-known
   library or idiom that already does the job.
2. The justification — inherited expertise (vetted edge cases,
   community practice), explicitly **not** DRY.
3. The guard — well-known *and* earns its weight; don't pull a
   new/heavy dependency for a trivial job a few honest lines
   or an existing import handles; name when the hand-roll
   wins.
4. The Decision-Log nudge — for load-bearing design choices,
   record the alternative weighed ("considered X; chose Y
   because …"). Recommended, not mandatory (answers backlog
   open Q2: heuristic + nudge, not a hard ban).

Plus a one-line extension to §6's existing **"Ground first"**
prose: "…and survey what the language / ecosystem already
offers for the problem, not just the codebase corners the
feature touches."

### §7 placement — dev-loop preamble

§7 currently jumps from its inputs subsection straight to the
Quality Gate with no "how to write the code" prose. A short
always-on preamble (a few lines) sits before the Quality Gate:

**"Write for intent (dev-time)."** Content: frame each
function around what a caller would say it does; wire the
logic in the shape the language / surrounding codebase already
speaks — functional / fluent or library calls over hand-rolled
state machines **when that reads as the intent**; reach first
for what's already in reach (stdlib, existing deps, imported
helpers); match surrounding conventions; legibility is the
goal, not dogma (don't force a chain where a plain loop or
typed match is clearer; don't refactor between equivalent
forms without a gain).

### Why both always-on (SKILL.md), not interviews.md

`interviews.md` is inline-Read only on fresh-feature start.
Both rules must fire on **continue / pick-up** entries too —
the agent picks up a half-built feature and writes more code,
or revisits a design. Following the established precedent set
by the Requirements smell test (kdevkit-separate-what-from-how),
always-on content that fires every session belongs in
SKILL.md. The added context is two short subsections; cost is
modest and justified.

The §6 **Design interview prompt** in `interviews.md` *also*
gets sharpened (it fires at fresh-feature design time) — today
it says "Lead with rationale — why this shape, what was
considered and rejected." Addition: "…including **what
well-known library or language idiom already does this job** —
name it before designing a hand-rolled alternative." This is
the interview-time echo of the always-on §6 rule.

### Why no §7 Code Review Gate change

The backlog's open Q1 floated a §7 reviewer "is this
hand-rolling something a library does?" finder angle. Rejected:
the §7 Code Review Gate dispatches a **host-native** reviewer
(per `code_review.reviewer: host-native`) whose prompt kdevkit
doesn't author, and the dispatch contract **deliberately
excludes the feature spec** from the reviewer's context. A
rule added there wouldn't reach the in-loop reviewer reliably.
Authoring-time (§6 design, §7 wiring) is where the choice is
actually made; host-native code-quality review catches the
"why so much code" smell downstream on its own. Keeping the
change to authoring-time also keeps the diff small and the two
rules co-located with the phases they govern.

### Language scope — generic (answers backlog open Q3)

kdevkit is a workflow skill, not a Rust/Python/Go style guide.
The rule speaks of "the idiom *this* language / codebase
already speaks" and "the well-known library for this
ecosystem." No per-language tables. The agent already knows
the idioms for whatever language it's working in; the skill's
job is to make it *look* and *prefer*, not to enumerate.

### V-model interaction — kept distinct (answers backlog open Q4)

The §6 Requirements smell test separates feature-level
Requirements (what the *feature's user* observes) from Design
(how it's built). The new discovery rule's "what a caller
would say the function does" is the **caller's** intent — a
developer reading/calling the function — which is a different
layer from the feature-level user. So the new §6 subsection
sits *beside* the smell test, not woven into it; the dev-time
wiring rule lives in §7 entirely. No threading through the
V-model framing.

### Version bump

SKILL.md `version` 3.3.0 → **3.4.0** — minor signpost. The §6
discovery rule and §7 wiring preamble are additions to the
kdevkit *contract* (new always-on behavior the agent exhibits
every session). Honest minor bump, parallel to the 3.1→3.2
bump for the separate-what-from-how contract change.

### Diff shape

- **`resources/content/skills/kdevkit/SKILL.md`** — new §6
  subsection (~12 lines) after "Requirements smell test"; one
  line added to "Ground first"; new §7 dev-loop preamble
  (~8 lines) before the Quality Gate; `version` 3.3.0→3.4.0.
  The frontmatter `description` is reviewed for accuracy but
  likely unchanged (it doesn't enumerate §6/§7 internals at
  this granularity).
- **`resources/content/skills/kdevkit/interviews.md`** — one
  clause added to the Design interview prompt (#3 in the
  four-interview list). ~2 lines.
- **`resources/tests/skills/kdevkit-idiomatic-design-and-wiring.smoke`**
  — new judge-mode fixture (~6 lines: prompt + expect_substr +
  expected_narrative + tools).

No `setup.md` change (no template/schema shift). No
`project.md` change (the rule is project-agnostic; lives in
the skill). No Rust change.

### Trade-offs considered

- **One rule vs. two.** Two chosen — the user's explicit
  framing; different justifications (inherited expertise vs.
  legibility) at different phases (design vs. dev). One-rule
  would blur the "look at design time" from "wire at dev
  time" distinction the user cares about.
- **Always-on SKILL.md vs. interviews.md.** SKILL.md for both
  always-on subsections (fires on continue/pick-up);
  interviews.md *additionally* gets the Design-prompt clause
  (fires at fresh design). Matches the smell-test precedent.
- **§6 + §7 vs. §6-only.** Both — the user named the
  design/dev split as the whole point. §6-only would drop the
  wiring half; §7-only would drop the discovery half.
- **Add a §7 reviewer finder vs. not.** Not — host-native
  reviewer prompt isn't kdevkit-authored and excludes feature
  context; the rule wouldn't reach it. Authoring-time
  placement is where the choice happens.
- **Prescriptive ("never hand-roll if a library exists") vs.
  heuristic + nudge.** Heuristic + Decision-Log nudge — a hard
  ban breaks on the xshell-vs-duct case (dep heavier than the
  hand-roll). The guard ("earns its weight") and the nudge
  ("name the alternative weighed") encode judgment, not a
  ban. (Backlog open Q2.)
- **Per-language idiom tables vs. generic.** Generic — kdevkit
  is tool/language-agnostic workflow; the agent knows the
  idioms. (Backlog open Q3.)
- **One judge fixture vs. two (one per phase).** One — the
  canonical drift case ("validate frontmatter") exposes both
  phases at once (look-then-wire), and a single
  `expected_narrative` with four behaviors covers discovery +
  justification + guard + wiring. Two fixtures would
  over-isolate before a real failure shows a granularity gap.

## Implementation Plan

One slice — markdown content edits to two skill files plus one
new fixture. No Rust touched.

- [ ] **Edit SKILL.md §6** — add the "Reach for what exists
  (design-time, always-on)" subsection after the "Requirements
  smell test (always-on)" subsection; add the one-line
  ecosystem-survey clause to the "Ground first" prose.
- [ ] **Edit SKILL.md §7** — add the "Write for intent
  (dev-time)" preamble before the Quality Gate subsection.
- [ ] **Bump SKILL.md `version`** 3.3.0 → 3.4.0; review
  frontmatter `description` for accuracy (likely unchanged).
- [ ] **Edit interviews.md** — add the "what well-known
  library/idiom already does this job" clause to the Design
  interview prompt (#3 of the four interviews).
- [ ] **Add fixture
  `resources/tests/skills/kdevkit-idiomatic-design-and-wiring.smoke`**
  — judge mode per Test Strategy above; public-safe
  "validate frontmatter" example.
- [ ] **Audit existing kdevkit smokes** — re-read
  `kdevkit-codebase-grounding` and `kdevkit-dev-loop`
  `expected_narrative`s for contradiction with the additions;
  patch only on a literal conflict (expected: none).
- [ ] **Quality Gate** — `just fmt-check && just lint &&
  just check` (no-ops for markdown but must stay green).
- [ ] **Test Gate** — `just test` (build-tool unit suite +
  structural install test; all green). `just install` then
  `just status` to confirm kdevkit symlinks resolve.
- [ ] **Code Review Gate** — host-native `/code-review` on the
  green diff (project.md + diff; no feature spec), threshold
  70, hard-stop, retry-budget 2.
- [ ] **Hand off functional smoke to user** — name the command
  (`just verify-one kdevkit-idiomatic-design-and-wiring`; full:
  `just verify`); do not run (user-driven, `[confirm]`-gated).
- [ ] **Push + open Agent-dev Review Gate** — body: Why +
  Approach (the two rules) + Reading order (SKILL.md §6/§7 for
  intent; interviews.md for contract; the fixture for
  plumbing). Hand off to user for PR review.

Risk notes:

- *Always-on context creep.* Two new subsections add to the
  always-on SKILL.md budget. Kept tight (~20 lines total);
  reviewer should flag if either subsection sprawls.
- *Wiring rule misread as dogma.* The §7 preamble must read as
  a legibility *default* with explicit escape hatches (plain
  loop / typed match when clearer), not "always use a fluent
  chain." Risk that the agent over-applies; the fixture's
  wrong-answer list guards against it.
- *Judge flakiness.* Judge-mode fixtures evaluate via a second
  LLM; the `expected_narrative` is broad enough to admit
  phrasing variance ("the well-known library for this
  language") while failing the load-bearing wrong answers
  (DRY-only justification, no design/dev split). If it flakes,
  tighten the narrative, don't loosen it.
- *Content-validator structure check.* If the build-tool's
  validator asserts SKILL.md heading presence/order, the new
  subsections (H3 under existing H2 §6/§7) shouldn't trip it —
  confirm during the Test Gate.

## Session Log

<!-- append: date · what was done · decisions made -->

- 2026-06-11 · backlog → feature promotion + planning ·
  grounded by reading kdevkit SKILL.md/interviews.md/setup.md,
  project.md, the analogous kdevkit-separate-what-from-how
  feature spec, and three example smoke fixtures. Surveyed
  established agent-coding guidance (Anthropic Claude Code
  best-practices, OpenAI codex AGENTS.md, grugbrain.dev) to
  ground the rule in proven phrasing — confirmed the
  two-rule split and the reuse-first / earns-its-weight
  guards against a naive "always use a library" rule. Spec
  written autonomously per user direction (no planning
  review); decisions resolve all four backlog open questions
  (§6+§7 two distinct rules; heuristic+nudge; generic
  language scope; V-model kept distinct).

## Decision Log

<!-- append: decision · rationale · alternatives rejected -->

- **Two rules in two sections (§6 discovery, §7 wiring), not
  one.** Rationale: user's explicit framing — different
  justifications (inherited expertise vs. legibility) at
  different phases (design vs. dev). Alternative rejected:
  single rule mentioned in both sections — blurs when each
  fires and forces one justification onto both.
- **Both rules always-on in SKILL.md; interviews.md Design
  prompt sharpened additionally.** Rationale: both must fire
  on continue/pick-up, not just fresh-feature start (matches
  the Requirements smell-test precedent). The interview prompt
  is the fresh-design echo. Alternative rejected: interviews.md
  only — fails to fire on non-fresh-start paths.
- **No §7 Code Review Gate finder added.** Rationale: the gate
  dispatches a host-native reviewer whose prompt kdevkit
  doesn't author and whose context excludes the feature spec —
  a rule there wouldn't reach it. Authoring-time placement is
  where the choice is made. Alternative rejected: add a "is
  this hand-rolling a library?" reviewer angle — unreliable
  reach, larger diff.
- **Justification is inherited expertise, not DRY.** Rationale:
  user was explicit — a well-known library is worth a
  dependency because it encodes vetted edge-case handling and
  practice, making the design elegant/correct; "shorter code"
  (DRY) is the wrong reason and the fixture treats DRY-only as
  a wrong answer. Alternative rejected: frame as DRY — misses
  the point and would justify pulling deps for trivial
  dedup.
- **Heuristic + Decision-Log nudge, not a hard ban.**
  Rationale: a "never hand-roll if a library exists" rule
  breaks on dep-heavier-than-hand-roll cases (xshell vs.
  duct). The "earns its weight" guard + "name the alternative
  weighed" nudge encode judgment. (Backlog open Q2.)
  Alternative rejected: hard ban — brittle.
- **Generic, language-agnostic phrasing.** Rationale: kdevkit
  is workflow, not a per-language style guide; the agent knows
  the idioms. (Backlog open Q3.) Alternative rejected:
  per-language idiom tables — scope creep, dates fast.
- **V-model framing kept distinct from the new rule.**
  Rationale: "what a caller would say the function does" is the
  caller's intent — a different layer from feature-level
  Requirements. The new §6 subsection sits beside the smell
  test, not woven in. (Backlog open Q4.) Alternative rejected:
  thread through the V-model — conflates two layers.
- **One judge fixture, not two.** Rationale: the canonical
  "validate frontmatter" drift case exposes both phases at
  once; one `expected_narrative` with four behaviors covers
  discovery + justification + guard + wiring. Alternative
  rejected: one fixture per phase — premature granularity.
- **Version 3.3.0 → 3.4.0.** Rationale: new always-on contract
  behavior; honest minor bump (parallel to 3.1→3.2 for
  separate-what-from-how). Alternative rejected: stay at
  3.3.0 — would hide a contract change.
