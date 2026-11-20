# Feature: reader-rules-values

## Git Setup

- Branch: `feat/reader-rules-values/work`
- Base: `initiative/my-harness`

## Feature Brief

A whole-status reader written in another language from reader rule 14 gets
the same state as mAId's for every setting entry: how a `value` compares and
which key a dotted name finds leave nothing to the reader's language.

Part of initiative: [[my-harness]]

## Handoff

- **Stage:** closed
- **Ready for:** nothing; the squash-merge into `initiative/my-harness` is
  the ringmaster's
- **Carry forward:** none open. String-only `value` is for now (ruled);
  the contract says so in rule 14 and nothing else depends on it. The JSON
  parse split is backlog `reader-json-edges`.
- **Deliberately left:** an adoption made before this change that another
  list also records stays mAId's (no migration); a claimed new target makes
  mAId remove its own old value. Neither is wanted work.

### Crossings
<!-- Append one line per crossing. Never edit or delete a line. -->
- planning → dev · EXCEPTION · skipping: planning review · why: rules
  already ruled by the ringmaster
- dev → review
- review → dev · RETURN · fault: requirements · issue: the ringmaster added
  one-value settings to the adoption rule (Claude's `autoMemoryDirectory`
  left an identical hand-set value alone) · fix: adopt it in `memory.rs`,
  one adoption rule in rule 2, a unit test · done when: an identical value
  is recorded `adopted`, stays so, and uninstall removes it
- dev → review
- review → dev · RETURN · fault: design · issue: adoption let two part
  lists claim one value · fix: read the other lists first and skip a value
  one records · done when: such a value is recorded `another part's` and
  left on uninstall, in unit and stage tests
- dev → review
- review → closure
- closure → closed

## Requirements

- Rule 14 in `specs/project.md` (`### My harness`, Reader rules) says:
  - a `value` that is a string is compared as a string, code point for code
    point, with JSON escapes decoded and no normalisation;
  - the entry is present when the setting, or for a list one element of it,
    is a string equal to `value`; a setting or element that is not a string
    never matches;
  - a `value` that is present and not a string: not checkable, decided
    before the config file is read;
  - a name is first a top-level key equal to the whole name, and only when
    there is none a dotted path of nested objects; a key with dots below the
    top level is never found as one key.
- mAId's reader gets the result the text gives on every conformance case.
- One adoption rule for every setting a part owns (rule 2): a value, or a
  list element, exactly what the part would write is recorded `adopted`,
  reported `already present, adopted`, and removed on uninstall. Install
  of Claude's `autoMemoryDirectory` follows it, as the plugin-read
  directory already does. A value another part's list records as placed is
  never adopted: it is recorded `skipped: "another part's"` and left on
  uninstall, so two lists never both claim one value.
- Generic terms only.

## Test Strategy

- A new conformance case `setting-values` (claude settings), red against
  today's `whole.rs` where it bites: number value, bool value, a value that
  is not a string with no such key (not checkable, not missing), a scalar
  string equal and not equal, a non-string list element, escape decoded vs a
  differently composed string, a top-level dotted key beside a nested path,
  a dotted key one level down that must not be found.
- `list-setting`'s expectations move with the rule (its number value becomes
  not checkable; its non-equal scalar string becomes missing), and its
  `about` line is rewritten.
- A `memory.rs` unit test: an identical hand-set `autoMemoryDirectory` is
  adopted, stays adopted on reinstall with or without claude, and
  uninstall removes it (seen failing with adoption disabled).
- Unit tests in `part.rs` (which values other lists claim), `memory.rs`
  and `reads.rs` (a claimed identical value is skipped, not adopted, and
  uninstall leaves it), and the two stage tests (`memory_install.rs`,
  `reads_install.rs`) through a real parts dir; the stage tests and the
  `part.rs` test fail with the claim check disabled.
- `whole.rs` unit tests for settings still pass. `just ci` passes. No paid
  tests.

## Design

`whole.rs` `setting()`: before reading the file, a `value` that is present
and not a string returns not checkable. After lookup (unchanged: top-level
key, else the dotted path), a string `value` matches the found setting when
it is that string, or an array with an element that is that string. No
`value` (or null): present when found, as today.

Differs from today in two places: a non-string `value` was compared as a
serde JSON value against list elements, and a scalar setting ignored
`value` entirely.

## Implementation Plan

- [x] Case `setting-values` and the `list-setting` update, red first.
- [x] `setting()` change; all cases green.
- [x] Rule 14 and rule 3's `value` note in `project.md`.
- [x] `autoMemoryDirectory` adopts an identical value; `Ownership` carries
  `adopted` from the part list; rule 2 states adoption once.
- [x] No adoption of a value another part's list records; both setting
  paths read the other lists (`part::claimed_by_others`).

## Session Log

- 2026-10-12 · Closure. Backlog: this feature closes no existing item
  (`mcp-remove-ownership` is MCP ownership, untouched); it adds
  `reader-json-edges`. project.md: the My harness rules and part list
  notes were edited in dev; Layout, Testing and the rest are unchanged.
  `just ci` and `resources/tests/rollback` pass on f6a2886.
- 2026-10-12 · Code Review Gate on the adoption change (correctness): PASS
  WITH NOTES, no high. Fixed all three lows: without claude, an identical
  unrecorded value records `claude not on PATH`, not `not mAId's`; rule 2
  says "any other value it did not write"; `tests/memory_install.rs` runs
  adoption through the real part list.
- 2026-10-12 · Re-review of the claim check: PASS WITH NOTES, lows only.
  Fixed: "already hold" means recorded on both paths, so reads re-adds a
  recorded element that went missing, as memory does. Left: a claimed new
  target makes mAId remove its own old value (allowed, it is mAId's).
- 2026-10-12 · Code Review Gate on the claim check: PASS WITH NOTES, one
  medium (writing could still double-claim) and three lows; fixed the
  medium, the own-value test and the format check. Left: an adoption made
  before this change that another list also records stays mAId's.
- 2026-10-12 · Ringmaster review: one change (no adoption of another
  part's value), the rest accepted; backlog `reader-json-edges` accepted.
- 2026-10-12 · Review briefing: one defect (the entry above was doubled)
  and one gap (the value check before the file is read had no case with an
  unreadable file); both fixed, case `setting-value-first`.
- 2026-10-12 · Code Review Gate (correctness): PASS WITH NOTES, no high.
  Fixed: the escape case held a raw `é`, not `\u00e9`; rule 14 now says
  "neither null nor a string" and "split at every `.`" (case `a.b.c` beside
  a top-level `a.b`); rule 3's note scoped to settings. The `1e400` and
  lone-surrogate split went to backlog `reader-json-edges`.
- 2026-10-11 · grounded on `whole.rs` `setting()`, `part.rs`,
  `reads.rs`, `memory.rs` (writers: string values only) and the
  `list-setting` case. Lookup order already matches the ruling; the value
  comparison does not. Probed serde_json: `1e400` is "number out of
  range" and a lone `\ud800` is rejected, so such a file is not JSON to
  mAId while other languages parse it.

## Decision Log

- Ruling (ringmaster, review): never adopt what another part owns. Before
  adopting, mAId reads the other part lists; a value one records as placed
  is that part's (`skipped: "another part's"`). A list that cannot be read
  claims nothing: whole-status already reports it, and refusing to adopt
  for it would let any broken file block adoption. Shifted the plan by one
  item.
- The claim check also gates writing, not only adopting (from code
  review): a stale other list that records a value not in settings would
  otherwise end with both lists claiming it once mAId writes it. A value
  mAId already holds stays its own.
- Ruling (ringmaster, review): string-only `value` is for now; adopting
  an identical value a user set, and a changed value reading missing, are
  accepted.
- Ruling (ringmaster): adoption covers every setting a part owns, so
  `autoMemoryDirectory` adopts an identical hand-set value; rule 2 states
  it once and the two setting paragraphs cite it. Shifted the plan by one
  item. The initiative's earlier ruling ("a hand-set value is left alone")
  now holds only for a value that is not identical.
- Non-string `value` is checked before the file is read, the same order
  as rule 5's other not-checkable cases, so an unreadable file does not
  change the state.
