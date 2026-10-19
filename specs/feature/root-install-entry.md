# root-install-entry

## What / Why
A machine-setup walk that runs each tool repo's root `./install` (or
`./setup`) found nothing in mAId after the move to `just install`, so
mAId was cloned but never installed, with no warning. A root
`./install` that runs `just install` restores that contract.

## Requirements
- [x] R1 `./install [agent]` at the repo root runs `just install [agent]`.
- [x] R2 Works from a plain shell: re-execs through `nix develop` when
  `just` or `cargo` is not on `PATH`.
- [x] R3 Pass-through only; install logic stays in the Justfile.

## Design
Thin bash wrapper, `cd` to its own dir, `exec` the recipe. Mirrors
the earlier pass-through `install` this repo had before the Justfile.

## Test Strategy
`bash -n install`; `./install` from a shell without `just` on `PATH`
runs the install through the dev shell; `just status` afterwards
reports the new profile generation.

## Implementation Plan
- [x] Add `install`.
- [x] Note it in `project.md` (Install bullet, Layout).

## Handoff

- **Stage:** review
- **Ready for:** merge
- **Carry forward:** pairs with the env-repo change that skips
  `*-worktrees/` in its setup walk, so a worktree's `./install`
  never runs from it.
- **Deliberately left:** no `--uninstall`; `just uninstall` covers it.

### Crossings
<!-- Append one line per crossing. Never edit or delete a line. -->
- dev → review · EXCEPTION · skipping: planning phase · why: user
  asked for a small change without a full plan
