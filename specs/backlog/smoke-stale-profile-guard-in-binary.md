---
name: smoke-stale-profile-guard-in-binary
description: The guard that stops smoke running against a profile that is not this checkout's build lives in Just, so a direct cargo smoke run and verify-skills-kind are unguarded. Move it into build-tool once the Deploy trait has its second impl.
metadata:
  type: backlog
---

# Stale-profile guard in the binary

## What

`_profile-is-current` (resources/Justfile) compares the store path the
checkout would build with the installed profile and runs before
`smoke-skills`, `verify-skills` and `verify-skills-one`. Not covered:
`cargo run -p build-tool -- smoke` and `verify-skills-kind` (its recipe
leaves the kind-to-stage mapping to the binary).

## Why

Smoke reads a frozen copy, so a stale profile passes on old text and
spends paid runs proving nothing. It stayed in Just because a trait method
would have to be carried by the second `Deploy` impl that native plugins
(initiative `installable`, stream 2) add; revisit once that exists.
