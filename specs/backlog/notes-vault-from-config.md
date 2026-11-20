# Backlog: notes-vault-from-config

## What

notes takes its default vault from a config value bound per environment,
named in the session's genome, the way writing-style's files are. With
nothing bound and no `$NOTES_VAULT`, it stops and says no vault is set,
as it already does for a named vault. No `$HOME/notes` default.

## Why

From harness-core stream 10 (saved-in-session): a skill that falls back to
a default path reaches the real HOME in a run that isolates only the
ghost's own locations (Macro run 4, R4-1, a read). notes' fallback would
be a write. No Macro step runs notes, so it was left out of that stream.

## Open questions

- How the user's existing `$HOME/notes` vault keeps working: bind it at
  install, or keep `$NOTES_VAULT` as the binding.
- The notes smoke tests that name the default chain change with it.
