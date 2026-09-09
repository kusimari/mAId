# Feature: the crossing record, kept in prose

Branch: `kdevkit-deterministic-thru-prose`

## What this is

kdevkit asks an agent to keep a record of where a feature stands, and today
that record is one line in a block the agent is told to rewrite. This makes
the record harder to lose and harder to fake, **without changing any code** —
only the skill's own markdown.

It is one of two ways kdevkit could keep that record. The other, using a small
tool and git commits, is being developed separately on
`kdevkit-deterministic-thru-tooling` and is deliberately absent here. The
skill names the branch point so the invariant reads the same either way.

## The problem

The record lives in prose the builder maintains, and the instruction is
*replace the whole block*. Two things follow.

**A stale record reads as current.** An agent that updates `Carry forward`
but leaves the stage untouched produces a block that looks freshly written
and is wrong. Measured on the wider problem: a ~300-line instruction file,
asked to repeat its own rules back, was followed ~100% of the time by claude
and kiro but **~33% by codex once ~4.6KB of unrelated conversation preceded
it** — and rewriting the prose did not fix it. Full evidence in
`specs/backlog/kdevkit-durable-cross-runtime-adherence.md`.

**Replacing destroys history.** If a feature goes back to planning three
times, nothing records that. Each return overwrites the last, so the third
looks exactly like the first — and "we have been round this loop three times"
is the single most useful signal for deciding the fault is in the design
rather than the code.

## What changes

**The invariant is stated first, and separately from how it is kept.** Every
stage boundary is crossed with judgement, and the crossing is recorded; a
stage that ended without a record did not finish.

**The handoff section gains a second part.** Current state is still replaced
at each crossing. A new `### Crossings` list is **appended to and never
edited**, one line per crossing. Returns become countable, which they were
not before.

**A return must carry four things** — the layer at fault, the problem, the
fix, and how we will know it is fixed. A bare "went back" is the silent plan
amendment §9 already forbids; this gives it a shape that cannot omit the
reason.

**An exception is a first-class move.** A gate that cannot be passed honestly
can be passed on the record, naming what is skipped and why. Nothing is
forbidden; it is made expensive by being written down and counted. Without
this, an agent facing an unpassable gate either stalls or proceeds silently,
and silence is the worse of the two.

**The stage map lives in the always-on file.** Phase modules state their own
exit condition and never name their successor, so adding a stage does not mean
editing the module before it. Going back may skip stages, because the
criterion is which layer the fault entered.

**`Phase:` becomes `Stage:`**, matching the vocabulary used everywhere else.

## What must be true when we are done

Checkable by reading a repository, saying nothing about mechanism.

1. A session starting with no knowledge of prior work can determine the
   current stage from the spec alone.
2. Every crossing appears as a line in `### Crossings`.
3. The crossings list is append-only: an earlier line survives a later
   crossing.
4. A recorded return names the layer at fault, the problem, the fix, and how
   it will be known fixed.
5. Returns are countable without parsing prose.
6. A recorded exception names what was skipped and why.
7. Going back may skip stages; going forward may not.
8. Current state and the crossings log never contradict each other.
9. Statements 1–8 hold on claude, codex and kiro. Antigravity is an intended
   target and is untested here — it needs a machine this work cannot run on.
10. **This is better than what main does today**, measured on the same
    fixtures and agents. If the record is kept no more reliably than before,
    the change is not worth the extra prose.

## How we will test it

**No agent, on every build.** The fixture-integrity suite already proves each
agent-driven fixture fails when the agent does nothing. Nothing else here is
mechanically checkable, because there is no mechanism — which is the honest
cost of the prose path and the reason statement 10 matters.

**With agents.** The existing kdevkit fixtures, which already exercise the
handoff at boundaries, plus one new fixture for the property that did not
exist before: `kdevkit-crossings-appended` seeds a crossing and requires it to
survive, so an agent that rewrites the list rather than appending fails.

**The A/B for statement 10.** The same fixtures on this branch and on main,
three agents, three samples, ratios recorded rather than a verdict. That is
the experiment that says whether this earns its place.

Two rules from prior experience. One sample proves nothing. And every
agent-driven fixture also runs with unrelated prior conversation prepended,
because the whole problem only appears under load.

## How to build it

- [x] 1 · State the invariant in `SKILL.md`, separately from how it is kept,
  and name the branch point so a later tooling version changes nothing above.
- [x] 2 · Add `### Crossings` to the handoff template, append-only.
- [x] 3 · Give `return` its four required parts and `except` its two.
- [x] 4 · Move the stage map into the always-on file; phase modules state exit
  conditions and never name a successor.
- [x] 5 · Rename `Phase:` to `Stage:` across the skill and the fixtures.
- [x] 6 · Fix the dev-loop overview: `code → quality → test → code review →
  push`.
- [x] 7 · Add `kdevkit-crossings-appended`.
- [~] 8 · Run the A/B against main (statement 10). Measured under load:
  claude 3/3, kiro 5/6, codex 3/3 — 11 of 12. Existing handoff fixtures
  show no regression. **What this does not settle** is below.

## Still open

- **Statement 10 is partly evidenced and not settled.** Under load the new
  fixture holds 11 of 12 times, and codex — the agent that measured ~33% on
  prose under this exact kind of load — is 3 of 3. But it is **one fixture**,
  testing the append-only property rather than the whole handoff discipline.
  And there is no main-side number to compare against, because main's skill
  *cannot* run this fixture: it replaces the block wholesale, so a seeded
  crossing is destroyed by definition. The comparison is therefore "a property
  main cannot have" rather than "main does this worse", which is weaker
  evidence than a true A/B.
- **Kiro failed once out of six and did not repeat across three further
  samples**, so it reads as noise. That cannot be proven, because the run
  captured only PASS/FAIL and the diagnostic log was discarded — a process
  error worth not repeating, since a ratio without diagnostics is not
  actionable.
- **Nothing enforces any of this.** That is inherent to the prose path, not a
  gap in the work — and it is why the tooling path is being explored
  separately rather than abandoned.

## Handoff

- **Stage:** dev
- **Ready for:** review, once the A/B against main has run.
- **Carry forward:** the tooling path lives on
  `kdevkit-deterministic-thru-tooling` and must not leak into this branch —
  this branch changes markdown only.
- **Deliberately left:** the tooling arm of the branch point in `SKILL.md`,
  named but not filled in.

### Crossings

<!-- Append one line per crossing. Never edit or delete a line. -->

- (feature opened) → planning
- planning → dev
