# Feature: writing-style-behavioral-verification

## Git Setup

- Branch: `feat/writing-style-behavioral-verification`
- Base: `main` (faf1fb2)

## Feature Brief

Bring the `writing-style` skill's verification up to the coverage
bar the `browser`, `kdevkit`, and `notes` skills already meet. A
repo-wide audit of every skill against `project.md` Testing's
three verification styles (substring / semantic-judge /
behavioral) found those three well-covered — each load-bearing
behavior has a fixture in the style its artefact calls for — and
`writing-style` the sole outlier: one substring fixture that only
checks the skill *announces itself*, with three of its four
behaviors (formatter, strict mode, learning loop) having no
fixture at all.

This feature closes that gap for the two highest-value behaviors.
It replaces the recitation probe with a **fixture set**:

1. a sharpened load-check (substring),
2. a **formatter** fixture (judge) — the core "prose actually
   comes out styled" test, and
3. a **learning-loop** fixture (judge) — the `new rule:` teach
   turn produces a correctly-shaped `## Learned rules` entry.

All three are judge/substring, not behavioral. `writing-style`'s
load-bearing output is *prose in the reply* (a rewritten passage,
a change log, a described file edit), not a filesystem artefact —
exactly the "genuinely non-artefact behavior" `project.md` Testing
reserves semantic/judge for.

Out of scope, deferred: a true **behavioral** fixture for the
learning loop (it would need the skill to accept a redirected
write target so a test doesn't mutate the installed, symlinked
`SKILL.md`); and a **strict-mode** fixture. Both are recorded
below and, for the learning-loop write path, routed to the
existing `writing-style-mcp-server` backlog.

## Requirements

The experience layer has one audience: the person running
`just resources::verify-skills`, whose observable surface is which
fixtures pass and what a failure tells them. A fixture is correct
when a compliant agent passes it and a non-compliant one (a no-op,
an echo, an agent that only announces the skill) fails it.

- Running the `writing-style` fixtures against `claude`, `kiro`,
  and `codex` exercises the skill's *behavior* — that styled prose
  and a change log actually come out, and that a teach turn
  captures a rule — not merely that the skill loaded.
- The **load-check** fixture passes when the reply carries the
  `[writing-style] applies` marker. It is explicitly the set's
  load/self-announce check, not a stand-in for behavior coverage.
- The **formatter** fixture asks the agent to "format this in my
  style" a passage seeded with concrete, in-guide violations. It
  passes only when the reply both (a) rewrites the passage with
  those violations corrected per the guide, and (b) emits the
  change log the skill's Formatter section requires. An agent that
  echoes the input unchanged, or rewrites without a change log,
  fails.
- The **learning-loop** fixture gives a `new rule: …` teach turn.
  It passes when the reply commits to appending a dated entry to
  `## Learned rules` in the shape the skill defines (date · rule ·
  optional example/source) and confirms per the skill's contract.
  A conversational "I prefer X" phrasing (a non-trigger) must not
  be what's tested — the fixture uses a real prefix trigger.
- Every fixture runs against all three agents; all are required
  (matching the tri-tool bar the other skills meet).
- No `writing-style` behavior regresses: the skill's `SKILL.md`
  content is unchanged by this feature — only fixtures are added
  and one is sharpened.

## Test Strategy

Per `project.md`'s two layers:

- **`just test` (unit, load-bearing, §8 Test Gate).** This feature
  adds `.smoke` fixture files and touches no Rust. The content
  validator and symlink state machine are unaffected; the gate
  must stay green. No new unit test — there is no new code path.
- **`just resources::verify-skills` (functional, user-driven,
  credit-costing).** Where the new fixtures live. Per `project.md`
  "Functional tests are user-driven," the agent **prepares** the
  fixtures and names the exact command; the user runs it. Do not
  run it this session.

Success criteria, mapped to the functional layer (each is a
fixture a compliant agent passes and a no-op agent fails):

| Criterion | Fixture | Style |
|---|---|---|
| Skill loads / self-announces | `writing-style` | substr |
| Formatter rewrites + emits change log | `writing-style-formatter` | substr (marker) + judge |
| `new rule:` captures a shaped entry | `writing-style-learning-loop` | substr (marker) + judge |

The judge fixtures follow `browser-safety.smoke`'s proven shape:
a marker `expect_substr:` as the load-check plus a rich
`expected_narrative:` that names what a right answer covers *and*
what wrong answers look like, so the judge has a discriminating
rubric.

### Why not behavioral, and why not a greppable proxy

- **Formatter / learning-loop output is prose in the reply.** The
  behavioral harness (`--- setup --- / --- assert ---`) asserts
  against files in a scratch repo. The formatter writes no file;
  the learning loop's only file write targets the installed,
  symlinked `SKILL.md` — a test must not mutate it. So neither has
  a safe filesystem artefact to assert on today.
- **A greppable substring proxy is too weak here.** The runner's
  `expect_substr:` is presence-only (`grep -qiF`). The
  discriminating signals for styled prose are *absences* — no
  em-dash where the spaced-hyphen learned rule applies, no
  "utilize", no exclamation mark — which a presence check cannot
  assert. A marker substr stays as the load-check; the judge does
  the behavior discrimination.

## Implementation Plan

- [ ] Sharpen `resources/tests/skills/writing-style.smoke` into a
      clear load-check: keep the `[writing-style] applies` marker
      substr; tighten the prompt so it reads as the set's
      load/self-announce check.
- [ ] Add `resources/tests/skills/writing-style-formatter.smoke`:
      a "format this in my style" prompt over a passage seeded
      with in-guide violations (a long word the Vocabulary section
      names, a hedging adverb, business-speak, an exclamation
      mark); `expect_substr:` on the marker; `expected_narrative:`
      asserting the rewrite corrected the seeded violations and a
      change log was emitted, with explicit wrong-answer cases.
- [ ] Add `resources/tests/skills/writing-style-learning-loop.smoke`:
      a `new rule: …` teach turn; `expect_substr:` on the marker;
      `expected_narrative:` asserting a dated `## Learned rules`
      entry in the skill's shape and the confirmation line, with
      wrong-answer cases (treating a conversational "I prefer…" as
      a trigger; silently editing the body instead of appending).
- [ ] All three fixtures carry `tools: claude,kiro,codex`.
- [ ] Quality Gate (`just fmt-check` + `just lint` + `just check`)
      and Test Gate (`just test`) stay green — fixtures are
      content, but run the gates to confirm nothing regressed.
- [ ] Hand off the exact `just resources::verify-skills-one …`
      commands for the user to run (functional layer is
      user-driven; not run this session).

## Decision Log

- **Judge-mode, not behavioral, for writing-style (2026-07-27).**
  Considered giving the learning loop a real behavioral fixture by
  extending the skill to accept a redirected write target (as
  `notes` takes `in <path>`). Chose to defer: it expands the
  skill's contract and overlaps the `writing-style-mcp-server`
  backlog, which already owns a redirectable, file-locked write
  path. writing-style's core output is prose, which `project.md`
  Testing explicitly routes to semantic/judge.
- **Dropped the strict-mode fixture from this feature
  (2026-07-27).** Formatter + learning-loop are the highest-value
  behaviors; strict mode is a session-flag guardrail on top of the
  same styling logic the formatter fixture already exercises.
  Recorded as a follow-up rather than built now.
- **Audit outcome (2026-07-27).** browser / kdevkit / notes each
  already have a fixture per load-bearing behavior in the right
  style; writing-style was the only skill below the bar. So the
  repo-wide audit resolves to a single-skill fix, not a
  cross-skill sweep.

## Session Log

- 2026-07-27 · Promoted from backlog. Read all four skills and all
  11 existing fixtures; built the coverage map. Confirmed with the
  user: judge-mode only, tri-tool, fixtures = sharpened load-check
  + formatter + learning-loop; strict-mode and behavioral
  learning-loop deferred.
