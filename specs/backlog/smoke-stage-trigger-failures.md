---
name: smoke-stage-trigger-failures
description: The first real post-install smoke run found 5 failures in 28 tests, all in triggering rather than content — kreviewkit discovery fails on both agents, notes discovery on claude, and two closure integrations. Structurally untouched by the inline-skill change, since smoke only runs implicit kinds.
metadata:
  type: backlog
---

# Smoke-stage failures: triggering, not content

## What

The first post-install `smoke` run ever executed (claude + codex, against
a deployment in a throwaway `$HOME`) reports **21 pass, 5 fail, 2 skip**:

| failure | kind |
|---|---|
| `skill:kreviewkit discovery via claude` | discovery |
| `skill:kreviewkit discovery via codex` | discovery |
| `skill:notes discovery via claude` | discovery |
| `kdevkit-closure integration via claude` | integration |
| `kreviewkit integration via codex` | integration |

Discovery is 5/8; integration 16/18.

## Why these are the expected place to fail

Both smoke kinds are **implicit**: the prompt is the task alone, and the
agent must find the right skill unaided, competing against every other
installed skill for a capped, shared description listing. That is
precisely the surface `project.md` documents as probabilistic and
requiring 3-5 samples. Discovery failing where the paired explicit
`playback`/`enact` kinds passed is the diagnostic pair working as
designed: **content is right, triggering is weak.** No bisection needed.

`kreviewkit` failing on both agents is unsurprising and already known:
`specs/backlog/kreviewkit-playback-layer-unverified.md` records that its
description was rewritten late (634 -> 361 chars) to fit the shared
listing budget and that its discovery had **never been tested on any
tool**. This is the first evidence, and it is negative.

## Not caused by the inline-skill change

Smoke runs only implicit kinds, whose prompt is `format!("{task}\n")` —
byte-identical before and after that change (verified against commit
`03dcd2c`). The inline change only affects explicit prompts, which smoke
never builds. So these failures are pre-existing and were simply never
observable, because the pre-install/post-install split did not exist and
nobody had run this half.

## Open questions

- **Sample before acting.** One sample each. `project.md`'s own rule is
  3-5 runs with the ratio recorded. A single discovery failure may be
  variance; three across two agents on the same skill is more likely real.
- **`kreviewkit`'s description is the first suspect.** It is the shortest
  and most recently rewritten. Compare its trigger words against the
  tasks in its fixture's `enact` section — discovery fires on that task.
- **Is the listing budget the cause?** Five skills now share the cap. If
  so, this is a project-level tension rather than a per-skill fix, and
  worth measuring (which descriptions get truncated, at what total).
- **`kdevkit-closure integration via claude`** — the explicit `enact`
  counterpart passed, so this too is triggering. But closure is a
  workflow with no announce marker, so its evidence is artefacts; worth
  checking whether the implicit task phrasing reads as a closure request
  at all.
- **kiro was excluded** from this run: it cannot authenticate under a
  redirected `$HOME`, so its credentials are absent and every invocation
  fails at login. A real kiro smoke run needs the operator's own `$HOME`,
  which makes it a genuinely attended test.
