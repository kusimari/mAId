# Backlog: kdevkit — separate "what" from "how" in feature spec interviews

## What

Update the kdevkit skill's feature interviews so the resulting
spec keeps user-facing requirements ("what") strictly separate
from implementation design ("how"), with tests sitting between
them as the contract.

Concretely, three changes to the skill:

1. **Interview prompts in `interviews.md` get explicit
   "user-facing only" framing.** The Requirements interview
   currently asks "what does the feature do" and accepts
   answers like "fires a hook on PreToolUse and writes the
   state to sessions.json under flock." That's how, not what.
   New framing: "describe the feature as if the reader will
   only ever read `--help` and use it — no internal verbs,
   no file paths the user doesn't see, no library names.
   What does the user type, and what do they observe?"

2. **Feature spec template (`interviews.md`) gains a fixed
   section ordering:**
   ```
   ## Feature Brief
   ## Requirements — launch experience    (what the CLI does, observed)
   ## Requirements — runtime experience   (what the user sees, observed)
   ## Hard constraints
   ## Prior art (optional)
   ## Runtime prerequisites
   ## Test Strategy
       ### Functional / Integration       (validates requirements externally)
   ## Design                              (the "how": schemas, plumbing, libraries)
   ## Unit Tests                          (validates design primitives)
   ## Design rationale
   ## Implementation Plan
   ```
   The split between Functional/Integration (before Design)
   and Unit (after Design) is the load-bearing piece —
   functional tests live with requirements because they
   validate user-visible behavior, unit tests live with
   design because they test design primitives.

3. **Skill guidance in `SKILL.md` (or `interviews.md`) adds a
   "smell test" the agent runs before writing each
   Requirements section:** if the bullet names a Rust trait,
   a config file path the user wouldn't see, an event name
   in a hook protocol, an internal subcommand, or a library
   — it belongs in Design, not Requirements. Move it.

## Why

Three wins, in priority order:

1. **The spec becomes readable as a contract by someone who
   isn't going to read the code.** Today's specs (e.g. an
   earlier draft of `agent-session-orchestrator.md`) braid
   "the user runs `setup --key X`" with "this writes tagged
   entries to `~/.claude/settings.json` with
   `x-agent-orch-managed: true`." A reviewer who only cares
   whether the UX is right has to filter the impl out
   themselves. The split lets them stop at the end of
   "Requirements + Test Strategy → Functional/Integration"
   and know what the tool does.

2. **Functional/integration tests validate requirements,
   not design.** When tests sit BEFORE design, they're
   forced to phrase assertions in terms the user would see:
   "the dashboard shows a `▶` icon" rather than "the
   `state` field of the matching record in
   `sessions.json` is `Working`." When tests sit after
   design, the path of least resistance is to assert on
   internal field names — which then ossifies the design
   into the test surface and makes refactoring expensive.
   Putting tests before design closes that drift before it
   starts.

3. **Design changes don't ripple back into the
   Requirements section.** When `Working/Waiting/Done`
   internally became `Working/Waiting/Done` with `Idle` as
   a render-time decay, the user-facing spec didn't need
   to change at all — the user still sees four icons. With
   the split, that's automatic; without it, every internal
   refactor risks editing user-facing prose by accident.

## Context — where the need surfaced

In the `agent-session-orchestrator` feature work
(`feat/agent-orch-fix` branch), the first spec draft
mixed:

- "user runs `setup --key X`" (what) with
  "writes a tagged entry to `~/.claude/settings.json`"
  (how)
- "rows show four states" (what) with
  "`Notification` event maps to `waiting`" (how)
- "fzf is the picker" (how, in a section meant to be what)

The reviewer (the user) called this out and asked for a
rewrite. The rewrite worked (cleanly separated, tests
moved before Design), but the *next* spec we author will
hit the same friction unless the skill itself encodes the
discipline.

## Acceptance criteria

- After updating the kdevkit skill, running the feature
  interviews on a new feature produces a spec where:
  - the Requirements sections contain zero references to
    library names, on-disk file formats, internal verb
    names, or function/trait names
  - the Test Strategy section's Functional/Integration
    cases describe assertions in user-observable terms
    (what's printed, what's shown, what tmux says, etc.)
  - the Design section is where all those names finally
    appear
- The feature spec template in `interviews.md` reflects
  the section order above
- The skill's "smell test" guidance is short enough to
  fit in the always-on `SKILL.md` (one paragraph, four
  bullets) — not buried in a deferred file

## Out of scope

- Migrating existing specs to the new layout. They can
  be left as-is until next time they're substantially
  rewritten.
- Any change to how feature interviews collect their
  data (the four interviews stay, just with sharper
  framing on the Requirements one).
- Project-level (`project.md`) structure — this is a
  feature-spec change.

## Relates to

- `specs/feature/agent-session-orchestrator.md` (the
  feature work that surfaced the gap)
- `~/tool-workplace/ai-workspace/mAId/sources/skills/kdevkit/`
  (the skill files to edit when this gets promoted)
