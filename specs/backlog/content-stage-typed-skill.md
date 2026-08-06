---
name: content-stage-typed-skill
description: The content stage answers with a count while the verify stages re-read each SKILL.md for its announce contract. Fold the contract into a typed Skill so stage 1's output feeds stage 2 and 4, rather than three readers of the same bytes.
metadata:
  type: backlog
---

# Content stage should return a typed `Skill`

## What

`stages::check_content` returns `Result<usize, Vec<String>>` — a count of
validated files. Separately, `harness::skill_announces` reads the same
`SKILL.md` again to decide whether a skill declares a self-announce line,
and it is called twice per test (once via `applicable`, once to pick the
assertion). Fold the announce contract into stage 1's output as a typed
`Skill { name, description, announces }`, so the pipeline's first stage
feeds its verification stages instead of each re-deriving from disk.

## Why

`verify-runner-in-rust` planned this and did not build it — the Design
argued it twice ("two readers of the same bytes for the same reason") and
the plan item was ticked without the type existing. The review briefing
caught the gap, and rather than land a signature change touching every
caller during a defect-fix round it was deferred here.

The cost today is small and not a correctness bug: a few extra reads of a
small file per sweep, and a design claim in the spec that the code does
not match. What makes it worth doing is the second reason, not the first
— `project.md`'s Architecture describes a pipeline whose stages consume
each other's output, and this is the one seam where that is aspirational.

## Open questions

- Does `Skill` belong in `shared` (vocabulary every stage speaks) or as
  stage 1's return type in `stages`? The pipeline argument says the
  latter; the fact that `harness` needs it says the former.
- `check_content` currently walks a directory and reports a count for the
  install verb's `validated N content file(s)` line. Returning
  `Vec<Skill>` preserves that (it is the length) but widens the type the
  install path carries. Worth checking that install does not grow a
  dependency on verification vocabulary.
- Should validation failure stay `Vec<String>`? It predates the
  `UsageError` split and is the one error path in the crate not using
  `anyhow`.
