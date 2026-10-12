# Feature: conformance-generic

## Git Setup

- Branch: `feat/conformance-generic/work`
- Base: `initiative/my-harness`

## Feature Brief

The conformance set is shared data every part's reader copies byte for
byte, so it names no part. `list-setting` used paths ending
`marketplace/plugins/maid`; they become generic, with every expected state
unchanged.

Part of initiative: [[my-harness]]

## Handoff

- **Stage:** closed
- **Ready for:** nothing; the squash-merge into `initiative/my-harness` is
  the ringmaster's
- **Carry forward:** none
- **Deliberately left:** `marketplace` stays where it is an entry kind,
  which is reader vocabulary, not a part's name. Not wanted work.

### Crossings
<!-- Append one line per crossing. Never edit or delete a line. -->
- planning → dev · EXCEPTION · skipping: planning and review gates · why:
  test-data rename ruled by the ringmaster
- dev → closure
- closure → closed

## Requirements

- No file under `resources/build-tool/tests/conformance/` names the repo or
  a part (case-insensitive).
- Every case's expected states and exit codes are unchanged.

## Test Strategy

- The conformance test in `just ci` passes with the renamed paths.
- Case-insensitive grep of the conformance dir for `maid` and every skill
  name under `resources/content/skills/` returns nothing.

## Implementation Plan

- [x] `list-setting`: `/s/marketplace/plugins/maid` becomes
  `/s/plugins/some-part` (settings and `own`), and
  `/gone/marketplace/plugins/maid` becomes `/gone/plugins/some-part`
  (`gone`).
- [x] Grep the whole conformance dir; no other hit.

## Session Log

- 2026-10-12 · Closure. Backlog: closes no existing item and adds none
  (`reader-json-edges` adds cases, untouched here). project.md: no section
  touched; its `plugins/maid` lines describe install, not the conformance
  set. `just ci` passes on f3ef3d0 (conformance case `every_conformance_case`
  ok); case-insensitive grep of the conformance dir for the repo and every
  skill name returns nothing.
