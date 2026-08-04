# Feature: kdevkit — split SKILL.md into per-stage modules

Part of initiative: [[kdevkit-decompose-and-harden]] (stream 1 of 6)

Branch: `refactor/kdevkit-module-split`
Worktree: `maid-worktrees/kdevkit-module-split`

## Feature Brief

kdevkit's `SKILL.md` is 1246 lines loaded on every session. After
this stream it is a small always-on core plus per-stage modules
that load only for the stage in play — so a dev-loop session
carries dev rules, not planning interviews and closure's eight
steps.

Behaviour does not change. Every rule that fires today still
fires; it just arrives from a module rather than from one file.
This is the enabling stream: streams 2–3 add their rules to a
module instead of growing the always-on file.

## Why

- **Drift is caused, not incidental.** Recall degrades as context
  grows — a gradient, not a cliff — which is why kdevkit sessions
  drop rules quietly rather than failing loudly. The always-on
  file is the largest single contributor.
- **Measured waste.** A dev-loop session needs roughly 590 of the
  1246 lines. Planning's 176 and closure's 122 are resident for
  work that will never touch them.
- **The ceiling is external.** Cursor's published guidance is
  "keep rules under 500 lines" and "split large rules into
  multiple, composable rules." kdevkit is at 2.5×.
- **The seams are already proven here.** `setup.md` (249) and
  `interviews.md` (256) are deferred today and work. The four
  `kdevkit-*.smoke` fixtures already split planning / dev-loop /
  closure / agents-md, so the A/B evidence exists before the
  refactor starts.

## Requirements

What the user (a coding agent driving kdevkit, or a human reading
it) observes.

- **R1.** A session that enters at one stage loads that stage's
  rules and the always-on core; it does not load other stages'
  modules.
- **R2.** Every rule that fires today still fires, with the same
  meaning and the same trigger. No rule is dropped, weakened, or
  reworded to fit a smaller file.
- **R3.** The always-on core states, for each module, **when that
  module must be read** — in the words the agent can act on at
  the moment it applies, not as a table of contents.
- **R4.** A stage transition pulls the next module. An agent
  mid-session that moves plan → dev is not stuck with planning
  rules.
- **R5.** The skill still installs and loads on all three agents
  (claude, kiro, codex) with no change to how a user installs it.
- **R6.** A reader can find any rule from the core: no rule is
  reachable only by knowing which file to open.

## Design

- **Module set.** `SKILL.md` (always-on core) plus
  `phases/plan.md`, `phases/dev.md`, `phases/review.md`,
  `phases/close.md`, `tiers/initiative.md`. `setup.md` and
  `interviews.md` stay as they are — already deferred, already
  correct.
- **What stays always-on.** Spec-tree detection, project-context
  load, entry-mode resolution, the stage-trigger table (R3), and
  §9's cross-cutting rules (Conventional Commits, author
  identity, working state, Review Gates, public-repo hygiene,
  commit hygiene, spec-discipline anti-patterns). These fire
  regardless of stage, so deferring them would break R2.
- **Loading mechanism: inline-Read, the pattern already in use.**
  `setup.md` and `interviews.md` are pulled by an explicit
  instruction to inline-Read at the moment of need. Modules use
  the same mechanism — no new primitive, and it works on every
  host because it is just a file read.
- **Module file convention.** Each module opens by stating what it
  carries and when it fires, matching `setup.md`'s and
  `interviews.md`'s existing headers.
- **Deployment needs no change.** `build-tool` validates
  `SKILL.md` only (`shipped_content_validates` and the frontmatter
  checks are per-skill, and a test asserts sibling files are *not*
  parsed), and both registry kinds resolve at the skill-directory
  level. New files under `skills/kdevkit/` are picked up by the
  existing symlink. Verified by reading the registry and
  `links_for`.
- **Deliberately out of scope for this stream:** compressing the
  prose inside each module (that is the per-file pass, once the
  split makes each file small enough to judge), deterministic
  phase transitions (stream 4), and the handoff record (stream 2).
  Moves only; rewrites are a separate, reviewable change.

### The one real risk, and how it is handled

Moving a rule out of always-on context is a *behavioural* change
even when the text is identical, because a rule that is no longer
resident can fail to fire. Two guards:

1. **The trigger, not the file, is the deliverable.** For each
   moved block, the core must say when to read it in
   act-on-it-now terms. A wrong trigger is the failure mode, so
   triggers get reviewed harder than the moves.
2. **Rules whose whole value is being resident stay resident.**
   §9's cross-cutting set is the test: a public-repo grep that
   only loads when you remember to load it is worse than no rule.

## Test Strategy

| Success criterion | Layer | How |
|---|---|---|
| Content still validates and installs (R5) | unit | `just test` — `shipped_content_validates` reads the real content tree |
| Symlinks resolve after the split (R5) | manual | `just resources::status-skills` after an install |
| No rule lost in the move (R2) | review | rule-inventory diff: every heading and rule in the old file accounted for in exactly one new location |
| Stage rules still recited correctly (R2, R3) | functional | the four `kdevkit-*.smoke` fixtures, judge-mode, before and after |
| Triggers fire from a bare task (R1, R4) | functional | same fixtures — they enter at a stage and must reach that stage's rules |

The functional layer is the load-bearing evidence for a prose
refactor and it costs credits, so per `project.md` it is a
hand-off, not an agentic run. Named exactly at the Test Gate.

**The rule inventory is this stream's real test.** A mechanical
before/after accounting of every rule, with its new home and its
trigger — cheap to produce, and the only way to make R2 checkable
rather than asserted.

## Implementation Plan

- [x] 1 · Rule inventory of today's `SKILL.md`: every `##`/`###`
      block, its line count, its stage, and whether it is
      always-on or stage-scoped. This is the contract for the move.
- [x] 2 · Create `phases/plan.md` from §6; leave the core citing
      it with a trigger.
- [x] 3 · Create `phases/dev.md` from §7's quality/test/code-review
      and dev-time rules.
- [x] 4 · Create `phases/review.md` from §7's briefing,
      comment-prefix convention, and agent-dev gate.
- [x] 5 · Create `phases/close.md` from §8.
- [x] 6 · Create `tiers/initiative.md` from §10.
- [x] 7 · Rewrite the core: detection, entry mode, stage triggers,
      §9 cross-cutting. Target under 500 lines; report the actual.
- [x] 8 · Verify the inventory: every rule in exactly one place,
      every module has a trigger, nothing orphaned.
- [x] 9 · `just test`; install and `status-skills` to confirm
      symlinks resolve.
- [x] 10 · Update `project.md` Layout if the skill's file shape is
      described there; update the skill's own "Multi-file shape"
      and "Skill-file placement" sections, which currently name
      three files.

## Session Log

<!-- Newest at top. -->

- **2026-08-04 · Briefing round 2: one new defect (D3), fixed.**
  Regenerating the briefing on the fixed work surfaced something
  the earlier passes missed — and it is the failure mode this
  stream's own Design section names as primary, so worth recording
  plainly. The core's trigger row for `phases/review.md` still
  said the `[agent]:` prefix rule "lives there," written when the
  split first landed and never updated when a later commit
  promoted that rule into always-on §9. So the table told an agent
  to read a 170-line module for a rule already in its context —
  **partly un-doing the promotion it was meant to complement.**
  Fixed: the row now says the rule is resident in §9 and the
  module carries the rationale.

  Two lessons: promoting a rule needs a sweep of everything that
  *pointed* at its old home, not just the two sites holding the
  text; and a wrong trigger really is invisible to every gate on
  this branch — no test caught this, a fresh reader did.

- **2026-08-04 · Briefing generator returned two defects; both
  fixed, loop re-run.** Dispatched the briefing to a fresh-context
  agent (I wrote the code, so per the firewall I can't write my own
  briefing). It found two things that were defects rather than
  judgement calls, so per its contract they go back to the loop
  instead of being published:

  - **D1 · A vacuous assert — and I reintroduced the exact bug I'd
    just fixed.** `grep -qi 'HELLO, WORLD!' test.sh` in the new
    phase-boundary fixture: the `-i` made it match the *seeded*
    `Hello, World!`, so the only check covering the Test Gate's
    artefact passed before any agent ran. Confirmed by replaying
    the setup block. Worse, an agent that implemented the flag,
    ticked the boxes and committed correctly but never updated
    `test.sh` passed all eight asserts. Fixed: case-sensitive
    `grep -qF`, plus a check that `test.sh` mentions the flag.
    **Verified it now fails that agent** — the discrimination table
    is no-op ✗, code-without-test-update ✗, compliant ✓. Lesson
    worth keeping: I "fixed" this bug class once in the same
    session and then re-added it a different way, which is the
    argument for the assert-must-fail-a-no-op rule being
    mechanical rather than remembered.
  - **D2 · The line counts I reported were stale**, including the
    one plan item 7 explicitly asked for. Two review passes added
    61 lines to the core; my log still said 511, so a reader would
    conclude the 500-line target was met by 11 lines when it was
    missed by 72. Restated against `HEAD` and de-duplicated to one
    place, since two copies is how it went stale.

  Everything else it raised is a judgement call for the human and
  stays in the briefing — chiefly whether 572-over-500 is
  acceptable, and whether the two rules promoted into §9 belong in
  a moves-only stream.

- **2026-08-04 · Review pass 2: PASS WITH NOTES, no blockers.** A
  second fresh-context reviewer enumerated all 131 §-references
  across the seven files and confirmed the map is complete and
  nothing dangles. Fixed its four remaining finds: `close.md`'s
  "see §10 for the table format" pointed at a file that doesn't
  carry the format (it's in `interviews.md`); an unqualified
  `§8.1` sat inside the *emitted* feature-file template, so it was
  being copied into every generated user spec (de-§'d rather than
  qualified — a user's spec shouldn't cite kdevkit internals); and
  `interviews.md` / `setup.md` had stragglers.

  **`setup.md` deserved a different fix than the others.** Its
  reader is the fresh-context verify subagent, which by contract
  receives `project.md` + `setup.md` only — never `SKILL.md`, so
  the §-map is unavailable to it. Rewrote its four refs to be
  self-contained ("the dev loop reads commands from it") instead
  of file-qualified. Worth noting as a general rule: a module
  dispatched without the core must not lean on the core's
  cross-reference table.

  Two more of its calls adopted:
  - **Single-sourced the safety floor.** Pass 1 promoted it to §9
    but left a near-verbatim copy in `review.md` — two copies that
    could diverge. `review.md` now carries only the
    briefing-specific delta and cites §9.
  - **Promoted the `[agent]:` prefix to §9.** Same argument as the
    safety floor: a one-line universal rule that fires in dev,
    review *and* closure shouldn't cost a 176-line module read to
    learn. Elaboration stays in the module.

  Added the two tests it named as the real gaps:
  `kdevkit-phase-boundary.smoke` (behavioral — seeds a reviewed
  spec on a branch, gives only the planning→dev cue, asserts the
  flag works, tests cover the new criterion, plan boxes are ticked
  in the dev commit, and closure did *not* run) and the earlier
  install assertion. **Verified the new fixture discriminates:**
  no-op agent fails, spec-only-without-code fails, compliant
  passes. Two of my own bugs surfaced doing that — an `[A-Z]`
  alternative that matched the seeded `Hello` (a vacuous
  assertion, exactly the retrofitted-test failure mode the repo
  warns about) and `grep -F '- [ ]'` parsing the dash as an
  option. Both fixed; this is why `project.md` requires an assert
  to fail a no-op agent.

  **One honest correction to my own framing.** This wins on the
  common case but a session traversing the *full* arc reads more
  than baseline. The claim is "no session carries rules it isn't
  using," not "strictly fewer lines in all cases." Current figures
  live in the "Split landed" entry below — restated once against
  `HEAD`, since the numbers I first recorded went stale as these
  two review passes grew the core.

  Not adopted, with reasons: the ~72-char wrap regressions on five
  edited lines (cosmetic, and re-wrapping churns the diff a
  reviewer is reading — worth a sweep in the per-file compression
  pass instead); `plan.md`'s stale "single source of truth" claim
  and the `code_review` defaults duplication (both pre-existing,
  and touching them widens a moves-only stream).

- **2026-08-04 · Code Review Gate pass 1: PASS WITH NOTES; two
  blockers fixed.** Fresh-context reviewer (no feature spec, no session
  history) confirmed the move is byte-clean by reconstructing the
  old section ranges and diffing each module — but found the real
  defect this refactor could produce, which the rule inventory
  could not: **the §-number cross-reference web no longer
  resolved.**

  1. *§-map wrong and incomplete.* I claimed "§7 dev" while §7 is
     split across `dev.md` *and* `review.md`, so core pointers to
     the Agent-dev Review Gate and Review Briefing sent readers to
     the wrong file. Fixed: a §→file table (noting §7 spans two
     files), § numbers restored on the demoted headings so search
     resolves uniformly, and every core `see §7` now names a file.
  2. *~15 stale `SKILL.md §N` refs in `setup.md` / `interviews.md`.*
     These name the *filename*, so the "original layout" escape
     hatch didn't cover them. Worst were two "return to SKILL.md
     §6's Plan-commit rule" pointers — the ordering rule the old
     file worked hardest to make unmissable. All repointed.

  Also fixed from that review: the diagram implied a fourth phase
  while the text says three (now shown as the back half of dev,
  with the reason stated); `plan.md` was the only module without a
  header/trigger; dangling cross-module pointers in `close.md` and
  `review.md`; a malformed `project.md` tree entry; three
  pre-existing wrong § refs in `project.md`; README not mentioning
  deferred modules.

  **Two findings I acted on beyond the minimum**, because the
  reviewer's reasoning was better than my original call:
  - **The dispatch safety floor is now resident in §9.** It had
    moved into `review.md`, and it is precisely the rule meant to
    stop prompt-injected diff content widening a dispatched tool's
    authority — a rule that only loads if you remembered to load it
    is the wrong shape. Generalized it from briefing-specific to
    any dispatch, with `review.md` keeping the elaboration.
  - **`§9 outranks any module`**, stated explicitly. A module is
    read *after* the core, so recency would otherwise favour it —
    `project.md` itself says precedence is what holds.

  Added the two tests the reviewer identified as missing: the
  structural install test now asserts a subdirectory module
  resolves through all three tool paths (verified it fails when it
  doesn't), and `kdevkit-module-load.smoke` covers module-loading —
  now the skill's central mechanic and previously untested.

  Unrelated but caught by the marker grep: scrubbed an internal
  repo name from `kdevkit-durable-facts-to-repo-not-agent-memory.md`
  (pre-existing on `main`, a public-repo violation).

- **2026-08-04 · Split landed; inventory closes.** Figures below
  are as-of `HEAD` (they were restated once — the two review
  passes added 61 lines to the core, and the original entry's
  numbers went stale).

  Always-on `SKILL.md` **1246 → 572** lines (54% cut). Modules:
  `plan` 192, `dev` 206, `review` 170, `close` 135,
  `tiers/initiative` 107. A dev-loop session carries 572 + 206 =
  **778** vs 1246 (38% off); the full arc reads 1382, so the claim
  is **"no session carries rules it isn't using,"** not "fewer
  lines in every case."

  **Plan item 7 asked for the actual: 572, against a 500-line
  target — missed by 72.** The overage is the trigger table, the
  §-map, and the two rules promoted *into* §9, i.e. the parts
  whose value is residency. Compression is the deferred per-file
  pass, not this stream.

  **Verification of R2 (no rule lost)**, three ways:
  1. *Line accounting* — every one of the original 1246 lines
     assigned to exactly one destination, no gaps or overlaps
     (checked programmatically).
  2. *Heading diff* — all 48 original `##`/`###` headings present
     in exactly one new file, with two expected exceptions:
     `Multi-file shape` was deliberately replaced by the new
     module table, and `Initiative-stream auto-link` appears
     twice — as it did in the original (a §5 pointer plus the §6
     rule), preserved as-is.
  3. *Rule spot-check* — 12 load-bearing rules confirmed present
     by literal match: Plan-commit ordering, reviewer-is-not-the-
     implementer, asking-is-the-artifact, `--force-with-lease`,
     no-premature-closure, `[agent]:` prefix, briefing safety
     floor, re-pin, test retry budget, public-repo abort,
     worktree-teardown-offer-only, last-stream archive.

  Line-level diff shows 42 old lines absent — all of them the
  meta-prose I intentionally rewrote (old "Multi-file shape"
  describing three files, the preamble's reading-order sentence,
  the stale placement rule). Zero workflow rules among them.

  Gates: `just fmt-check` clean, `just lint` clean, `just test`
  **98 passed**. Install validated 5 content files — sibling
  modules correctly *not* parsed, confirming the design
  assumption. `install-skills` **refused** to take over
  `~/.claude/skills` because it points at the primary checkout;
  that is the guardrail working, so I did not `--force` it.
  Verified module resolution through an equivalent temp symlink
  instead.

- **2026-08-04 · Stream 1 opened.** Grounded before planning:
  read `SKILL.md` in full, `setup.md` / `interviews.md` headers
  for the deferred-file convention, the `kdevkit-dev-loop`
  fixture, and `build-tool`'s registry + `links_for`.
  **Finding: deployment needs no change** — validation is
  `SKILL.md`-only (with a test asserting siblings are not parsed)
  and both registry kinds symlink at the skill-directory level, so
  new module files ride along. That removes the main integration
  risk I expected to plan around.

## Decision Log

<!-- append: decision · rationale · alternatives rejected -->

- **2026-08-04 · Split by stage, not by tier.** Rationale: stage
  is what changes *within* a session, so it is the boundary that
  saves context; tier changes between sessions. Alternative
  rejected: split project/initiative/feature — `tiers/initiative.md`
  is the one tier-shaped module because §10 genuinely only applies
  when an initiative is in play.

- **2026-08-04 · Moves only; no prose compression in this
  stream.** Rationale: a move is verifiable against a rule
  inventory, and a rewrite is not — mixing them makes an A/B
  regression unattributable. Compression lands per-file afterwards,
  when each file is small enough to judge. Alternative rejected:
  compress while moving, to touch each rule once — cheaper in
  effort, but it forfeits the only evidence that behaviour held.

- **2026-08-04 · §9 cross-cutting rules stay always-on.**
  Rationale: their value *is* being resident — a public-repo
  internal-marker grep that loads only when remembered is worse
  than no rule, since it produces false confidence. Alternative
  rejected: defer them into a `cross-cutting.md` for a smaller
  core — would trade the core's size for the rules most costly to
  miss.

- **2026-08-04 · Reuse inline-Read rather than invent a loader.**
  Rationale: it is the mechanism `setup.md` / `interviews.md`
  already use successfully, it needs no host-specific support, and
  it keeps the markdown-symlink deploy invariant intact.
  Alternative rejected: a code-driven loader — that is stream 4,
  and it is blocked on the code-vs-prose boundary.
