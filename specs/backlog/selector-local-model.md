# Backlog: selector-local-model

## What

A local model behind the `Selector` interface, to pick the memories that
may bear on a turn by meaning rather than by shared words.

## Why

The word lookup, widened in stream 11 (`prompt-prefix`) by the fitting
bring-ins' words and the named files' languages, still leaves out a
memory that shares no word with any of them (an accepted limit). The
`Selector` interface was built so a model can replace the lookup with no
change to its callers.

## Open questions

- Which model, how it ships in the profile, and its cost per prompt
  against the hook's 10-second timeout.
- Whether reading each memory file's frontmatter (left out of stream 11)
  is needed first.
