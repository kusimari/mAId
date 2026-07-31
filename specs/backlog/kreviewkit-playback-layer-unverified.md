---
name: kreviewkit-playback-layer-unverified
description: kreviewkit and the kdevkit briefing dispatch shipped with only the enact fixture ever executed (6× on claude). The playback narratives — the sole test of the generator-owns-the-contract inversion and the defects-route-back rule — have never been scored, and neither fixture has run on kiro or codex despite declaring all three tools.
metadata:
  type: backlog
---

# kreviewkit — the playback layer was never run

## What

Run the fixture kinds and tools that `kreviewkit` and the kdevkit
Review Briefing step declare but have never executed:

```
just resources::verify-skills-kind playback          # both fixtures, all tools
just resources::verify-skills-one kreviewkit         # or: one fixture, all kinds
```

What is currently proven, and what isn't:

| Surface | Status |
|---|---|
| `kreviewkit.smoke --- enact ---` (behavioral, 10 asserts) | **PASS ×6**, claude only |
| `kreviewkit.smoke --- playback ---` | **never run** |
| `kdevkit-dev-loop.smoke --- playback ---` (extended for the briefing step) | **never run** |
| `activation` / `discovery` (generated) | **never run** |
| Either fixture on `kiro` or `codex` | **never run** (both declare `claude,kiro,codex`) |

## Why

- **The playback narratives are the only test of this session's two
  biggest design decisions.** The enact fixture verifies the *briefing
  artefact* — wider-branch reading, spec↔diff reconciliation,
  read-only, no verdict. It cannot verify either of the rules the
  feature was reshaped around late: that **kdevkit consults the
  generator's declared contract** rather than defining it, and that
  **defects route back to the loop** instead of into the briefing.
  Both live only in `expect:` narratives nobody has scored.
- **The one behavioural failure this feature hit was found by running
  a fixture, not by reading it.** Tightening the defect-routing rule
  made a correct briefing fail a stale assertion — caught only because
  the enact fixture actually ran. The unrun narratives have had no
  equivalent check.
- **`project.md`'s own sampling rule is unmet.** "A single passing run
  proves nothing — this was mis-called 'fixed' twice off one sample.
  Run 3–5 and record the ratio." The enact kind clears that bar on
  claude; nothing else has a single sample.
- **Cross-tool is where kdevkit has failed before.** kdevkit silently
  didn't self-trigger on codex until its `description:` was
  front-loaded. `kreviewkit` is new, and its description was rewritten
  late in the feature (634 → 361 chars) to fit the shared listing
  budget — a change that plausibly affects `discovery` and has never
  been tested on any tool.

## Open questions

- **Run the whole matrix, or just the gaps?** `verify-skills-kind
  playback` covers the two narratives across all tools; adding
  `activation` / `discovery` / `integration` for `kreviewkit` catches
  trigger reliability. Cost scales with kinds × tools.
- **Is a playback failure a fix to the skill or to the narrative?** The
  narratives were written alongside the prose they describe, so a
  failure is ambiguous between "the skill is wrong" and "the narrative
  over-specifies." The diagnostic pair is explicit-vs-implicit: if
  `playback` passes and `discovery` fails, it is triggering, not
  content.
- **Should the kdevkit dispatch get its own fixture?** It is currently
  one clause inside `kdevkit-dev-loop.smoke`'s already-long judge
  narrative, so a terse-but-correct answer that omits it fails the
  whole check and localises nothing. Splitting the briefing claim into
  its own `playback` section would isolate the signal — the repo's own
  established pattern is one concern per fixture.
