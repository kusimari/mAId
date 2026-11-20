# Backlog: reuse-check-untracked-resources

## What

A test that the ghost's reuse check agrees with what plain's install
builds when `resources/` holds a file git does not track: under nix both
see tracked files only, without nix both see the working tree. Run it
through `just ghost install` in the rollback script (or an equivalent
free test), with nix and without.

## Why

Left from ghost-package's review: the check was changed to want what
plain's install would build (`#default` under nix) after a working-tree
staging misjudged such a checkout, but no test runs the untracked case.

## Open questions

- None.
