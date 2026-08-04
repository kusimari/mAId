---
name: kdevkit-adversarial-assert-discipline
description: Across two streams and three review passes, every defect found in behavioral fixtures was the same class — an assertion satisfiable without the agent doing the work. Reading never caught one; constructing the non-compliant agent caught all of them. Make that construction a required step of the Test Gate rather than a habit, since the habit demonstrably does not hold.
metadata:
  type: backlog
---

# kdevkit — asserts must be proven against an adversarial agent, mechanically

## What

`project.md` already says a behavioral assert "must fail a no-op
agent (pair a presence check with the absence check) or it proves
nothing." Strengthen it from a statement into a **procedure the
Test Gate runs**, because the statement is being honoured in intent
and violated in practice, repeatedly, by an agent that has read it.

The procedure, for each new or changed behavioral assert:

1. **Replay the fixture's `--- setup ---` in a scratch dir and run
   the `--- assert ---` block unchanged.** If it passes, the assert
   is vacuous — it is matching seeded content.
2. **Construct the *narrowly* non-compliant agent** — one that does
   everything right except the specific thing this assert covers —
   and confirm it fails. A no-op agent is too weak a probe: it fails
   for unrelated reasons, which is how a vacuous assert hides inside
   a fixture that technically "fails a no-op."
3. **Run a fully compliant agent** and confirm it passes. An
   unsatisfiable assert is as bad as a vacuous one and is not
   visible from either of the first two probes.
4. **Record the matrix** in the Session Log: probe → expected →
   actual.

## Why

- **Three consecutive review FAILs on one stream, same class every
  time.** Not the same instance — each fix was correct in intent —
  but every finding reduced to "this passes without the agent
  acting." Twice the *fix itself* was nominal: a "positive existence
  check" that matched the template's own HTML comment, and a
  "portability" comment written above a still-unportable pattern.
- **Reading cannot catch it.** Every one of these survived my own
  review, and each was found either by a reviewer replaying the
  setup or by me finally running the probe. The failure is not a
  knowledge gap about what makes a good assert — it is that the
  property is *empirical* and I kept checking it by inspection.
- **The specific traps recur and are worth naming**, since they are
  not obvious in the moment:
  - Matching text the `--- setup ---` block already seeded (the
    dominant case).
  - An unanchored substring that a *commented-out template* in the
    skill satisfies.
  - A negative check satisfiable by *deleting* the field
    (`test -z` passes on absence), unless paired with an anchored
    positive.
  - GNU-only regex (`\<`, `\s`, `\w`, `\b`) that silently matches
    nothing under mawk/BusyBox — the assert then always passes.
  - `git log --oneline` anchors that break under an inherited
    `log.decorate=full`.
  - A bad revision range whose `fatal:` is swallowed by `|| true`,
    leaving `test -z ""` true.
  - `ls | grep` matching a filename when the content is what
    matters (a zero-byte file passes).
  - Asserting an artefact **no rule requires** — which fails the
    *compliant* agent, and is a spec bug masquerading as a test bug.
- **This is the repo's own test-first argument, arriving by a
  different road.** `kdevkit-dev-loop-vmodel-and-ceremony`'s Rule B
  says a test written after the source "often asserts the code's
  accidental behavior, passes on the first run, and a vacuous
  assertion is indistinguishable from a real one." That is exactly
  what happened, in a repo whose backlog predicted it. The
  confirmed-red step is the missing guard, and for behavioral
  fixtures "red" means *the adversarial agent fails*, not merely
  that something fails.

## Sketch

- The rule belongs in `phases/dev.md`'s Test Gate, which already
  owns "tests land in the same iteration." One short subsection:
  the four probes, and the instruction to record the matrix.
- **It must be right-sized** or it becomes the ceremony this
  initiative is removing. Scope it to *behavioral asserts on a new
  or changed behaviour* — not to unit tests (where the compiler and
  a red-green run already give the signal), and not to every
  pre-existing assert in a fixture being touched for other reasons.
- Consider a runner affordance so the probe is cheap rather than
  hand-rolled each time: `resources/tests/run --replay <fixture>`
  to seed the setup and run the asserts against the untouched tree,
  which mechanises probe 1 (the highest-yield one) for free.
  Probes 2–3 need a hand-written agent and stay manual.
- Interaction with the ceremony lane: a trivial-lane change has no
  new behavioural assert, so this does not fire.

## Open questions

- **Is probe 1 automatable for the whole suite?** If
  `--replay-all` could report "these asserts pass on the untouched
  seed," it becomes a standing check rather than a per-change
  discipline — much stronger. The risk is false positives from
  asserts that *legitimately* hold pre-action (invariants the change
  must preserve), so it likely needs an opt-out marker per line.
- **Does this belong in the reviewer's packet instead?** Stream 3
  makes the code-review gate take an enumerated packet; "verify each
  new assert against an adversarial agent" is a plausible reviewer
  lens. Doing it in the dev loop is cheaper; doing it at review is
  more reliable. Probably both, and the reviewer is the backstop.
- **How is the matrix recorded without bloating the Session Log?**
  One line per probe is fine for three asserts and unreadable for
  thirty. Perhaps only failures-then-fixes get recorded, with a
  single line confirming the rest were probed.

## Trigger to promote

- Rides with stream 3 (`gate-packets`), where the reviewer-lens
  question above is already being decided — or sooner if a fourth
  vacuous assert ships, which would confirm the discipline cannot
  be left to memory.
