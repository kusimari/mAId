# Feature: kdevkit-prose-and-repin

## Git Setup

- Branch: feat/kdevkit-prose-and-repin
- Base: main

## Feature Brief

<!-- The capability layer — what can the user now do that
     they couldn't before? -->

Two paired kdevkit guidance rules that govern *where prose about
a change lives* and *when design-altitude re-checks fire*, both
landing in the §7 dev-loop region of `SKILL.md`:

1. **Comment style** — a code comment carries present-tense
   *intent* (the why a future reader can't read off the code) and
   stays terse; it does not restate the line below it, narrate the
   decision trail, or retell the bug that led here. That history
   belongs in the commit / PR / Decision Log.
2. **Feedback re-pin loop** — the stepping-back rituals
   (pin to `project.md`, survey what exists, find the right owner,
   decide altitude) re-fire when a dev-loop change is triggered by
   CR/verify feedback *or* a self-initiated reactive change — not
   only at the planning phase — whenever that change introduces,
   moves, renames, or re-scopes a component or alters a contract.

Both are guidance, not gates: the Code Review Gate may note a
violation but does not hard-stop on it.

## Requirements

<!-- The experience layer — the cues the agent recognises and the
     artefacts it produces. -->

### Comment style (agent behaviour when writing/reviewing comments)

- When the agent writes a code comment, it states the present-tense
  reason the code exists, the non-obvious constraint, or the gotcha
  a future reader would trip on — and stops there.
- The agent does **not** write a paraphrase of the adjacent line,
  the history of how the code got here, or the alternatives
  rejected — those go to the commit message / PR / Decision Log.
- For an external reference, the agent writes a terse pointer
  (e.g. `see project.md "<section>"`), not a retelling of the
  source's content.
- The rule reads as a legibility default continuous with "Write
  for intent," not a new ceremony; the Code Review Gate may flag a
  history-narrating comment but does not hard-stop on phrasing.

### Feedback re-pin loop (agent behaviour when acting on a finding)

- When the agent is about to act on CR/PR feedback or a verify
  finding — or makes its own mid-dev reactive change — and that
  change introduces a new component, moves/renames one, changes
  where something is installed/owned, or alters a public contract,
  the agent first runs a short re-pin check **before** writing the
  fix:
  1. **Owner** — does `project.md` already name a layer/module/repo
     whose responsibility this falls under? Put it there, not at
     the point of failure.
  2. **Altitude** — is the fix at the right tier, or patching a
     symptom one level below where the cause lives?
  3. **Reuse / idiom** — does an existing mechanism already do
     this that the fix should extend rather than duplicate?
  4. **Symmetry** — if the change adds an install/create/enable,
     is the inverse (uninstall/delete/disable) covered?
- A pure local fix (off-by-one, wrong string, missing guard) does
  **not** trip the check — the trigger is *displaced design*, not
  every edit.
- When the four questions are trivially "yes, right spot," the
  agent leaves one log line and proceeds — the check is reasoning
  surfaced in the Session/Decision Log, not a phase gate.
- The trigger is "a change is being made reactively," covering both
  external feedback and the agent's own verify-driven changes.

## Test Strategy

<!-- Success criteria mapped onto project.md test layers. -->

SKILL.md is prose loaded by an AI tool, so the load-bearing
evidence is the judge-mode functional fixture, per `project.md`
Testing ("SKILL.md prose revisions add `just verify` (judge mode)
as their A/B evidence").

### Unit (`just test`) — the §8 Test Gate default

- The build-tool content validator must still pass against the
  edited `SKILL.md` (valid frontmatter, file present in registry,
  install round-trip). No new unit tests — the change is prose, not
  build-tool logic. `just test` green is the gate.

### Functional (`just verify-one`) — user-driven A/B evidence

Two new judge-mode `.smoke` fixtures under
`resources/tests/skills/`, each `tools: claude,kiro`:

- **`kdevkit-comment-style`** — prompt asks the agent (with kdevkit
  loaded) what it writes in a code comment vs. what it leaves out.
  `expected_narrative` passes when the answer keeps intent in the
  comment, routes history/decision-trail/bug-narrative to the
  commit/PR/Decision Log, treats external refs as terse pointers,
  and frames it as a legibility default (not a hard gate). Wrong
  answers: comment restates the code; comment narrates history;
  rule presented as a hard-stop.
- **`kdevkit-feedback-repin`** — prompt gives a scenario where a
  CR comment / verify finding prompts a change that *moves where
  something is installed*. `expected_narrative` passes when the
  agent runs the Owner/Altitude/Reuse/Symmetry re-pin before fixing
  and places the change at the right owner rather than the point of
  failure, AND notes the cost guard (trivial-yes → one log line,
  local bugfixes don't trip it). Wrong answers: jumps straight to
  the smallest red→green fix; ritualizes every fix.

Per `project.md`, functional tests are **user-driven** — the agent
prepares the fixtures and names the commands
(`just verify-one kdevkit-comment-style`,
`just verify-one kdevkit-feedback-repin`) but does not run them.
The §8 Test Gate stops at `just test`.

## Design

<!-- The "how" layer. Lead with rationale. -->

**Placement rationale (the load-bearing decision).** The backlog
for comment-style leaned toward a §9 bullet "next to Conventional
Commits." On review that's the wrong home: a code comment is about
*how the code reads*, which is exactly what §7 "Write for intent"
governs — the dev-time legibility rule. Conventional Commits
governs *commit prose*, a different surface. The two rules share a
*separation* (intent in the comment ↔ history in the commit), but
the comment rule's natural owner is §7. So comment-style extends
the existing "Write for intent" section with a short comment
clause and cross-references §9 Conventional Commits only for the
where-history-goes split. This also unifies the feature: both
rules live in the §7 dev-loop region.

Considered and rejected: a standalone §9 bullet (splits a
legibility rule away from the legibility section); a worked
before/after example inside SKILL.md (bloats the always-on file —
the example stays in this spec and the backlog, per the
skill-file placement rule's "keep always-on lean").

**Re-pin placement.** The backlog notes both §7's Code Review Gate
loop-back and §8 closure feedback currently send the agent
straight to "implement the fix." The re-pin check is the first
step of *acting on* a finding. It lives as a short always-on
subsection in §7 (the dev loop owns reactive changes), positioned
so the §7 Code Review Gate score-handling loop-back and §8 closure
both reach it. It cross-references §6 "Reach for what exists" and
the requirements smell test — it is those same disciplines re-fired
on the feedback path, keyed to *what* (a design decision is being
made) rather than *when* (planning).

**Trigger wording.** The classifier must self-fire reliably without
snagging trivial fixes. Heuristic verb set:
*introduces / moves / renames / re-scopes a component, or changes a
contract* trips it; *local bugfix* (off-by-one, wrong string,
missing guard) does not. The trigger phrase is "a change made
reactively" so it covers the agent's own verify-driven changes, not
just external feedback.

**Honest scope limit (carried into the prose).** A re-pin validates
against the *project's own* design (`project.md` + codebase); it
cannot surface *ecosystem* knowledge that lives in external docs. The
rule claims the project-internal win (owner placement, altitude,
reuse, symmetry) and does not over-promise.

**Insertion points (source `SKILL.md`, currently v3.4.0):**

- Comment clause → appended inside `### Write for intent
  (dev-time, always-on)` (§7, ~line 461–479), after the "Legibility
  is the goal, not dogma" paragraph.
- Re-pin subsection → new `### Re-pin on reactive change` under §7,
  placed after the Code Review Gate's score-handling loop (so the
  loop-back's "Treat the highest-severity findings as the next
  implementation slice" reaches it) — or immediately before
  `### Inputs`, whichever keeps the dev-loop reading order intact.
  Final position chosen at dev time against the live file.
- Bump `version:` frontmatter (3.4.0 → 3.5.0; minor — additive
  always-on guidance).

**Public-repo hygiene.** The motivating cases in both backlog items
cite internal names (an internal initiative, a chat-MCP server, an
internal install-layer script, an internal toolchain manager, an
internal bin path, vendor-specific gnupg guidance). None of these
may appear in `SKILL.md`, the fixtures, commits, or the PR. The
re-pin prose uses a generic, hobbyist-flavoured illustration
instead (e.g. "a fix restored a PATH entry at the failing script
instead of in the install module that owns PATH wiring"), with no
internal identifiers. The internal-marker grep runs at the Planning
Review Gate and before push.

## Implementation Plan

<!-- One slice per item. Both rules ship as one combined slice
     (per the packaging decision), with one combined dev commit
     and the two fixtures. -->

- [x] **Slice 1 — both §7 prose rules + two fixtures (combined).**
  - Append the comment-style clause to §7 "Write for intent."
  - Add the §7 "Re-pin on reactive change" subsection (4-question
    check + cost guard + scope-limit note + cross-refs).
  - Bump `version:` 3.4.0 → 3.5.0.
  - Add `resources/tests/skills/kdevkit-comment-style.smoke`
    (judge mode).
  - Add `resources/tests/skills/kdevkit-feedback-repin.smoke`
    (judge mode).
  - Quality Gate (`just fmt-check` + `just lint` + `just check`),
    Test Gate (`just test`), Code Review Gate.
  - Hand off the two `just verify-one` commands to the user.

- *Risk note:* the always-on file grows — keep both additions
  terse; if either runs long, the worked example stays in the spec,
  not SKILL.md.
- *Risk note:* trigger wording that snags trivial fixes would make
  the re-pin check noisy. The fixture's wrong-answer list pins
  "ritualizes every fix" as a failure to guard against this.
- *Risk note:* public-repo leak — the source backlog text is full
  of internal names; scrub at authoring time, grep before push.

## Session Log

<!-- append: date · what was done · decisions made -->

- 2026-06-22 · Promoted two backlog items (`kdevkit-comment-style`,
  `kdevkit-feedback-repin-loop`) into one combined feature.
  Confirmed via merge-base diff that neither in-flight worktree
  (`feat/kaimux`, `feat/kiro-observation-only`) touches the kdevkit
  skill files — the `git diff main` "removals" were just those
  branches sitting behind main's v3.4.0. No collision.
- 2026-06-23 · Rebased onto main (`0eefca5`, kaimux merge #24);
  committed plan + opened Planning Review Gate (PR #27) from the
  `mAId-prose-repin` worktree. Plan approved → dev loop.
- 2026-06-23 · Slice 1 built: comment-style clause + "Re-pin on
  reactive change" §7 subsection + Code-Review loop-back cross-ref;
  version 3.4.0 → 3.5.0; two judge fixtures. Quality Gate green
  (fmt-check/lint/check), Test Gate green (53 passed; content
  validator + frontmatter tests exercise the edited SKILL.md).
  Code Review Gate: **92/100** (host-native fresh-context, threshold
  70) → pass. Two low findings, both ordering/phrasing ergonomics
  ("no change required"): re-pin subsection precedes its gate caller
  in linear reading (back-ref in loop-back step 2 closes it);
  both new rules anchor to §6 (distinct mechanics, lists
  disambiguate). Public-repo hygiene confirmed clean by reviewer.
- 2026-06-23 · Functional tests run (user-authorized, costs
  credits). Repointed managed symlinks to this worktree via
  `resources::install --force`, confirmed installed skill = v3.5.0,
  ran both fixtures, then restored symlinks to the primary checkout
  (recorded originals first). Both fixtures **fully pass** — all 8
  checks (claude + kiro × substr + judge). Judges confirm intended
  behavior on both harnesses. No content issues; nothing to fix.
  Note: `just resources::verify-one` is `[confirm]`-gated — feed a
  single `y` (`printf 'y\n' |`), not `yes |`, which floods claude's
  stdin past its 10MB limit and yields a false FAIL.

## Decision Log

<!-- append: decision · rationale · alternatives rejected -->

- **Comment-style home = §7 "Write for intent," not a §9 bullet.**
  A code comment is about how code reads (legibility), which §7
  owns; Conventional Commits (§9) owns commit prose. Cross-ref §9
  for the history→commit split. Rejected: standalone §9 bullet
  (separates a legibility rule from the legibility section).
- **No worked before/after example in SKILL.md.** Keeps the
  always-on file lean (skill-file placement rule); the example
  stays in this spec and the backlog.
- **Re-pin trigger = any reactive change (external + self).** The
  motivating case was self-initiated from a verify finding, so the
  trigger is "a change made reactively," not "feedback arrived from
  outside." Rejected: external-feedback-only (misses self-initiated
  displaced design).
- **One combined slice, one fixture pair.** Both rules are short
  §7-prose additions to the same file and ship together; one dev
  commit, two judge fixtures. Rejected: two separate slices
  (unnecessary ceremony for coupled one-file edits).
- **Generalize internal motivating examples.** Public-repo hard
  constraint — the backlog's internal references are rewritten to
  hobbyist-flavoured illustrations in SKILL.md and fixtures.
