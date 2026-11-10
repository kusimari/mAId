# Backlog: drop-old-part-lists

## What

Remove the old parts folder (`my-harness/parts`, `MY_HARNESS_PARTS`) from
plain one release after live-shell: mAId's migration of
`maid-harness.json`, whole-status reading old lists, and the conformance
cases for both old shapes.

## Why

Kept for one release so a package that still writes an old part list (the
work package) keeps its status checks while it moves to an install
record.

## Open questions

- Which release: once the work package writes an install record.
