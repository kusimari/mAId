# Backlog: repin-pensieve-published

## What

Re-pin the `pensieve` flake input from the local `git+file` checkout to
its published GitHub commit (`github:<owner>/pensieve/<rev>`), and
`nix flake lock` again so no local path stays in `flake.nix` or
`flake.lock`.

## Why

The memory-mcp feature pinned pensieve from a local checkout because it
was not published yet. A local path must not reach a pushed branch, so
this has to happen before any PR of the initiative.

## Open questions

- None: blocked only on the user publishing pensieve.
