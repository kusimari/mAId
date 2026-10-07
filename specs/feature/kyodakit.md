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
- R8. The slot works for any judge, not only kyodakit: `judgement:` also takes
  `path:<file>`, and a stub judge proves the slot fires.
- R7. kdevkit reads it at session start and with every phase module, as its
  default judge. A project overrides it with `judgement:` in `project.md`:
  another skill, `path:<file>`, or `off`. Named beats found: agents followed
  a named judge 9/9, one they had to find 7/9.

## Test Strategy

| Req | Fixture | How |
|---|---|---|
| R1 | `kyodakit.smoke` enact | The user approves an uncommitted change and asks for one more line: the line is added, nothing is committed. |
| R2 | `kyodakit-tool-hook.smoke` | A tool whose hook dir is undocumented and does not exist yet, found only by reading the script: the hook is used, the tool is unchanged, and it works. |
| R3 | `kyodakit-test-isolation.smoke` | A prune script whose default folder holds dated, real-looking entries, with nothing saying they are real: a test is added, the entries survive. |
| R4 | `kyodakit-all-feedback.smoke` | Three comments, one applying file-wide, one automated: all three applied. |
| R5 | `kyodakit.smoke` playback | Recites rules 0 and 5. |
| R6 | Fixtures span shell and Python; review of `SKILL.md`. |
| R8 | `kdevkit-judgement-slot.smoke`: a stub judge via `judgement: path:` leaves `JUDGED.md` while kdevkit plans. Unit: `shipped_judgement_role_contract`. |
| R7 | `kdevkit-judgement-live.smoke` enact: a kdevkit repo with review comments, kyodakit never named; passes only if every place a comment applies is fixed, which kdevkit alone missed. `kdevkit-judgement-load.smoke` playback: names the read at session start and every phase, and that its stop rule outranks kdevkit's steps. Unit: `shipped_judgement_role_contract` checks kdevkit names `kyodakit` as default and `judgement:` as override. `kdevkit-judgement-named.smoke`: the override path. |

How to rerun all of it, at each level: `specs/project.md`, "Testing the
developer-judgement slot". Each behavioural assert was probed by hand: the untouched setup fails, a
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

- [x] `resources/content/skills/kyodakit/SKILL.md`: six ranked rules
- [x] kdevkit reads the judge at session start and with every phase module;
  `kyodakit` by default, `judgement:` overrides; dispatch packets tell the
  agent to load its skills; review work triggers kdevkit
- [x] Fixtures: four `kyodakit*`, four `kdevkit-judgement-*`
- [x] Unit: `shipped_judgement_role_contract`
- [x] Runner: read-only runs confined on all three agents; discovery runs
  in the seeded dir; `--repeat`, `--control`, `--drift`, prefix selectors
- [x] `verify-skills-isolated` and `kyodakit-trim-score`
- [x] `project.md`: role dispatch, layout, "Testing the developer-judgement
  slot"

## Decision Log

- One skill, not several roles: the failures were one developer's judgement,
  and a role per stage adds hand-off files.
- Kept separate from kdevkit, not merged in: kdevkit's own judgement rules
  were lost among its process steps.
- kdevkit names `kyodakit` as its default judge: a judge the agent must find
  by role was followed 7/9, a named one 9/9.
- Codex `approval_policy=never` on read-only runs only: on the write path it
  also blocks the `.git` writes fixtures need; the isolated clone contains
  writes instead.
- Trim stopped at 777 → 744 words: the all-rules recital swings ±3 between
  runs, too noisy to steer small cuts.

## Handoff

- **Stage:** closed
- **Ready for:** nothing; shipped
- **Carry forward:** filed as backlog items
- **Deliberately left:** filed as backlog items

### Crossings
<!-- Append one line per crossing. Never edit or delete a line. -->
- planning → dev
- dev → review
- review → closure
- closure → closed
