# Backlog: ctx-concept-name

## What

Rename `infusion::call::Ctx` to a concept word (what an entry point is
handed: the shell the ghost is in, its config, its host, its session
sources), with the other renames stream 12 makes.

## Why

Left from live-shell at the ringmaster's ruling: every entry point and
test takes `Ctx`, so the rename is its own change, and stream 12 renames
`maid-core` and the verbs already.

## Open questions

- The word: `Infusion` (what the crate does with it), or `Shell` widened.
