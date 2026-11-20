# Backlog: justfile-argument-quoting

## What

Quote every user-typed recipe argument where a Justfile puts it into a
shell: `{{ quote(x) }}` in place of `"{{ x }}"`. Today the root Justfile's
`agent` and `whole`, and plain's `pattern`, `name` and `kind`, go into bash
inside double quotes, so a `"` or `$(...)` in an argument runs. `just
memory-store`, `just install` and `just ghost check-skills` already quote.

## Why

Found by the ghost-package stream's code review (security, low). The person
types these arguments, so the risk is small, but a Justfile is a shortcut
and should pass what it is given and nothing more. It predates that stream.

## Open questions

- None: one mechanical pass over the recipes, with a test that an argument
  holding `$(...)` reaches the command as text.
