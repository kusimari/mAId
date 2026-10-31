# Backlog: writing-style-permanent-tier-travels

## What

Let writing-style's permanent tier (`promoted` config key, today bound
by install to `<config dir>/writing-style/promoted.md`) be bound to a
location that travels between environments, such as a file in a store
the person owns, while the pending tier stays local.

## Why

The two tiers exist so a rule is tried locally before it is trusted;
a trusted rule should follow the person to a second machine, as the
harness-core Macro test asks of persona memory. Today no part can
rebind a value: install binds every state key under the config dir.

## Open questions

- How does a person set a bring-in's config value per environment (a
  verb like `just memory-store`, or a values file the part list reads)?
- A store's format belongs to its memory server, and no part writes
  anything else in a store; where may a skill's tier live in one?
