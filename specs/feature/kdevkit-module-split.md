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

- **2026-08-04 · Split landed; inventory closes.** Always-on
  `SKILL.md` **1246 → 511** lines (59% cut). Modules: `plan` 177,
  `dev` 206, `review` 174, `close` 134, `tiers/initiative` 107.
  A dev-loop session now carries 511 + 206 = **717** vs 1246, and
  no session carries all of it.

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
