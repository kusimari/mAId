# Feature: fold-back

## Git Setup

- Branch: `feat/fold-back/work`
- Base: `initiative/harness-core`

## Feature Brief

A reader of the README learns how mAId works as one design: the shell, the
ghost, and the ghost's five functions, with what each looks like on Claude
Code. A person adding another persona repo can join the ghost by following
one section of `project.md`, without reading any initiative or feature spec.

Part of initiative: [[harness-core]]

## Handoff

- **Stage:** closed
- **Ready for:** nothing; the ringmaster squash-merges into
  `initiative/harness-core`
- **Carry forward:** none. The other shells are filed as
  `specs/backlog/ghost-on-other-shells.md`; the checklist's dry run goes
  with the initiative's Macro test.
- **Deliberately left:** the initiative spec's own "For any persona part"
  copy, now older than project.md's on items 3, 6 and 9; a stream leaves
  the initiative spec alone.

### Crossings

- start → planning
- planning → dev
- dev → review
- review → closure
- closure → closed

## Requirements

- The README has a short "How mAId works" section: the shell is the base
  harness as its vendor ships it; the ghost is mAId plus every other part
  installed beside it. Then the five functions (genome, expression,
  experience, sleep, adaptation), each with what it does, when it fires on
  Claude Code, and what the person sees: the startup report, the per-turn
  reminders, "close this session" or "reflect", and the decision batch. It
  points to `project.md` for detail.
- `project.md` Architecture reads as one design in that framing: the shell
  and the ghost named once, the core placed among the layers, each of the
  five functions findable by name, and no paragraph stranded away from
  its topic.
- `project.md` has a section "For any persona part": the initiative's
  checklist, true against what was built, with the real part-list keys,
  slot keys, file locations, the overlay rule and `serve`. A person can
  follow it from `project.md` alone, and it names no other part.
- No claim in either file contradicts the code or the initiative's
  rulings (map derived per call, no install-rendered rules file on Claude
  Code, the "never drift" rules, `record`, `resources/content/servers/`,
  overlays, `serve`).

## Test Strategy

Docs only, so the checks are reading checks plus the gates that guard the
repo:

- kdevkit's structural check: the six required `##` headings present, in
  order, once each.
- Every key, path, hook and command the new text names is grepped against
  the code (`resources/core`, `resources/build-tool`, `resources/content`,
  `flake.nix`) before commit; each check is noted in the Session Log.
- The internal-marker grep finds nothing in the diff.
- `just ci` stays green (no code changes, so this only proves nothing
  else moved).

## Design

- **README:** one new `## How mAId works` section after the intro, before
  `## Develop`: two short paragraphs (shell, ghost), a five-row table
  (function, what it does, when it fires on Claude Code, what you see), a
  line on the other shells, and links to `project.md`. The closing
  sentence of Install ("mAId installs no global instruction file") gains a
  clause that the ghost's context comes through the plugin's hooks.
- **project.md Architecture:**
  - the `resources/` list gains the core as a layer;
  - `### My harness` keeps its heading and opens with the framing: shell,
    ghost, part, persona, bring-in, and the five functions, each pointing
    at its paragraph;
  - "The core" splits so genome, expression, experience, sleep and
    adaptation each lead a paragraph; the Claude adapter paragraph stays
    as the carrier map;
  - two stranded paragraphs move to their topic: "The list says what is
    there after install" to the part list, and "The entry is mAId's while
    its command is the profile's `bin/pensieve`" to the memory
    registration.
- **project.md "For any persona part":** a new `##` section after
  Deployment, before Agent Development, so the six required headings keep
  their order. Nine numbered steps, each naming the real keys and pointing
  to the Architecture paragraph that defines the shape.
- **Mission:** one sentence for the ghost, since the goal grew.

## Implementation Plan

- [x] README: "How mAId works", and the Install clause.
- [x] project.md Architecture: framing, core layer, five functions,
      stranded paragraphs moved.
- [x] project.md: "For any persona part" section; Mission sentence.
- [x] Checks: claims grepped against code, structural check,
      internal-marker grep, `just ci`.

## Session Log

- Grounding: read the initiative spec, the five merged feature specs'
  Requirements and Decision Logs, README, project.md, and the core's
  genome renderer (`entry.rs`), adaptation routes (`adapt.rs`), memory
  write (`memory.rs`), the Claude adapter (`adapters/claude/`), the
  reflect skill, and the server and overlay declarations.
- Found: a persona's memory is the one bring-in of that persona whose
  declaration has `state.permanent` and a config key `index`
  (`adapt.rs` `is_memory`, `memory_of`); with two, no habit can be placed.
  The checklist says so.
- Found while editing: Mission said "no global instruction preamble",
  which the genome hook now is in effect; it says "no global instruction
  file", which stays true. The Claude adapter paragraph said "Another
  stream's hook", initiative wording; it now says a part never adds one.
- Found: the hooks ship only in the Claude manifest (`hooks/claude.json`,
  `claude-mcp.json`), so codex gets the skills and no ghost.

- Gates: `just ci` green (before and after the review fixes); six
  required headings in order, once each; internal-marker grep clean.
- Code Review Gate (one `correctness` lens, docs paths only): PASS WITH
  NOTES, no Must Fix. Applied all six Should Fix and both What's Missing
  notes: README's `resources/` list gains the core; Mission and the
  Content layer no longer call the browser the only non-skill resource,
  and say only Claude Code carries the ghost today; README's "nothing is
  sent" now needs no fit and no memory match; a report-routed change is a
  note; checklist item 1 deletes the list only once nothing is left; item
  9 names an index leaving its store; README says a persona habit needs a
  store set.
- Review briefing: one defect, the Mission paragraph left unwrapped;
  re-wrapped. Its judgement items (hand-kept checklist copies, a dry run
  against a scratch persona repo, the README's first-person voice) are
  for the reviewer.
- Review rulings (ringmaster): keep the hand-kept checklist copies; the
  dry run against a fixture second part goes with the Macro test; the
  README section speaks to "you", as the rest of the README does.
  Re-ran the six-heading and marker checks after the voice edit: clean.

- Closure: no `- [ ]` left. project.md verify: Mission and Architecture
  are this feature's own edits; Tech Stack, Layout, Testing, Deployment
  and Agent Development are untouched; no AGENTS.md. Backlog answer:
  none closed out (no item is a docs fold); filed
  `ghost-on-other-shells` from Deliberately left.

## Decision Log

- Decision: "For any persona part" is its own `##` after Deployment, not a
  `###` in Architecture · why: the row asks for its own section, and a
  person adding a repo reads it alone · considered: under Architecture,
  where it would sit inside 600 lines of mAId's own detail.
- Decision: keep the heading `### My harness` · why: code comments cite it
  by name, and this stream changes no code.
- Decision: the checklist states "exactly one memory per persona" as the
  rule, and says that with two no habit can be placed · why: the core
  finds a persona's memory only when exactly one bring-in of that persona
  is a memory (`adapt.rs` `memory_of`).
