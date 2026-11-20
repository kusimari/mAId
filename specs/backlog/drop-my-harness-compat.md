# Backlog: drop-my-harness-compat

## What

Remove the old name's compatibility path one release after
ghost-in-shell-name: `ghost::old_name` and its callers (the reader's
fallback to the `my-harness` slot, `MY_HARNESS_STATE`,
`MY_HARNESS_CANDIDATES`, the state dir `my-harness` used where it is),
the `old name` lines in `ghost status` and the
genome, `Declaration::under_old_name`, and their tests (the
`bring-ins-old-name` conformance case and the old-name check in
`ghost-in-shell/tests/rollback`). Plain's `MY_HARNESS_PARTS` and the old
parts folder go with `drop-old-part-lists`.

## Why

Kept for one release so a person's state dir, environment and any
declaration written outside mAId (the work part's bring-ins) keep working
while they move to the new name. The warnings say what to rename.

## Open questions

- Which release: once the work part's bring-ins declare under
  `ghost-in-shell`, and the person's `my-harness` state dir is moved.
- Whether to move a state dir still named `my-harness` at that point, or
  only say so.
- Before then, whether `ghost status` should say when both state dirs
  exist (the old one is then not read, so its pending decisions are not
  shown).
