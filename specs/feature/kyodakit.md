# Feature: kyodakit

## Git Setup

- Branch: `feat/kyodakit`
- Base: `main`

## Feature Brief

A skill that makes a coding agent work like the experienced developer beside
the user, in any repo and any language. It comes from reading four long
sessions where the user kept correcting an agent, plus the review comments on
that work. 17 failures came out of that reading. Existing skills were then
checked against them. No outside pack covered them without bringing a
competing workflow, so kyodakit borrows wording and writes the rest.

It stands on its own, like `kreviewkit`, and fills the developer-judgement
role that kdevkit reaches for at every phase. It sits beside code-style
skills, not inside them.

## Requirements

- R1. Stops at a checkpoint the user names, and waits for a yes before going
  on. Approving a result is not approval to commit or publish.
- R2. Before building, uses what exists: the codebase, the standard library,
  the tool's own extension points, installed libraries.
- R3. Tests can fail, and never touch the user's real files.
- R4. Reads all review feedback before changing code, and applies each comment
  everywhere it applies.
- R5. States its stop and writing rules when asked.
- R6. Works in any language, and no rule only makes sense in one language.
- R7. kdevkit reads it explicitly, by role and never by name: at session start
  and with every phase module. A project can name another skill or turn it
  off with `judgement:` in `project.md`.

## Test Strategy

| Req | Fixture | How |
|---|---|---|
| R1 | `kyodakit.smoke` enact | Asked to play back a plan first: `PLAN.md` written, no other change, no commit. |
| R2 | `kyodakit-tool-hook.smoke` | A tool with a documented hook dir: the hook is used, the tool is unchanged, and it works. |
| R3 | `kyodakit-test-isolation.smoke` | A prune script whose default is the user's real folder: a test is added, real notes survive. |
| R4 | `kyodakit-all-feedback.smoke` | Three comments, one applying file-wide, one automated: all three applied. |
| R5 | `kyodakit.smoke` playback | Recites rules 0 and 5. |
| R6 | Fixtures span shell and Python; review of `SKILL.md`. |
| R7 | `kdevkit-judgement-load.smoke` playback: names the read at session start and every phase, and that its stop rule outranks kdevkit's steps. Plus review of the kdevkit diff: it names the role only. |

Each behavioural assert was probed by hand: the untouched setup fails, a
near-miss agent fails, a compliant agent passes.

## Design

One `SKILL.md`, six ranked rules, rule 0 first because order rules are the
ones that slip. kdevkit reads it explicitly with every phase module, the way
it resolves the review-briefing role, and does not wait for the agent to
discover it. Post-install tests showed discovery alone is unreliable
(claude 2/4, kiro 0/4, codex 4/4), while the loaded skill passed 26/27.
Merging it into kdevkit was weighed and rejected: kdevkit's own judgement
rules were lost among its process steps, which is the failure this fixes.
No hook in v1.

## Implementation Plan

- [x] `resources/content/skills/kyodakit/SKILL.md`
- [x] Four fixtures under `resources/tests/skills/`
- [x] kdevkit §9 "Developer judgement" line
- [x] `project.md`: announce list, role dispatch, layout

## Handoff

- **Stage:** dev
- **Ready for:** review, and the paid fixture run by the user
- **Carry forward:** trimming kdevkit's own prose is next, under
  `specs/backlog/kdevkit-refactor-shrink-always-on-context.md`
- **Deliberately left:** a per-prompt hook
- **Tuning:** kyodakit is tuned against its paid fixtures as an autoresearch loop (`phases/dev.md`, "Tuning what an agent uses"). Results in the PR.

### Crossings
<!-- Append one line per crossing. Never edit or delete a line. -->
- planning → dev
