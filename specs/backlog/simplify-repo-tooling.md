# Backlog: simplify-repo-tooling

## What

mAId is now managed with three tools: Rust (build-tool, kaimux), just
(the verb surface) and nix (the dev shell and the installable
profile). Cut the repo's tooling down to two clear declarations and
nothing beside them:

- **Build and run:** what is needed to build mAId and run its verbs.
- **Environments:** the dev environment (to work on mAId) and the
  runtime environment (what installed resources need), each declared
  once.

## Why

The tools grew by accretion: rust-toolchain.toml beside the flake's
rust-overlay, `.envrc`, Just recipes that wrap cargo, shell scripts
beside Rust, and layout knowledge that lives in both the Justfile and
`deploy.rs`. Each extra declaration is one more place to keep in step.

## Questions to settle

- Which declarations are redundant (for example the toolchain pinned
  in both `rust-toolchain.toml` and `flake.nix`), and which one wins?
- Which Just recipes are a thin wrapper over one cargo or nix command,
  and should go?
- Which shell scripts (`resources/browser/manage`, the test scripts)
  belong in build-tool, and which are genuinely shell?
- Is the profile layout (`share/maid/skills`, `bin/`, the marketplace)
  declared once, in the flake, with everything else reading it?

## Done when

One place declares the dev environment, one the runtime environment,
and the build/run verbs need nothing those two do not provide; each
removed declaration is recorded with why.
