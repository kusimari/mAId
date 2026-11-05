# Backlog: core-delivery-record-upkeep

## What

The core's delivery record keeps one dir per session under
`<state>/delivery/`, and nothing ever removes them. Prune session dirs
nobody has touched for a while (say 30 days, as the profile's generations
are). Also refuse to write through a symlink planted at a session dir:
`create_dir_all` follows one today, so the marker files could land
somewhere else.

## Why

Left from the core-and-infusion stream. Hooks will call the core on every
session start and turn, so the record grows by one small dir per session.
The symlink risk is low because the state dir is the user's own, but the
record should not follow a link out of it.

## Open questions

- Where pruning runs: on each `genome` call (cheap, a stat per dir), or
  at sleep, once that stream adds it.
- The age to keep: as long as the profile's 30 days, or shorter.
