# Backlog: memory-store-record-follows-registration

## What

Install records the memory bring-in's `store` from mAId's store setting,
whichever harness the run selects. Only the claude step registers the
server, so after the store setting changes, `just install <not claude>`
records the new store while claude's `personal-memory` still runs on the
old one. The genome and `just status --whole` then report the wrong
store's readiness. Record the store the server was registered with (or keep
the earlier recorded values when the claude step did not run), so the
record always matches the registration.

## Why

Found by the correctness lens in session-bring-ins' Code Review Gate. The
behaviour comes from core-and-infusion's recording of `values.store`; it
became visible once the memory is a declared bring-in whose readiness
depends on that value. Low impact: a full `just install` puts the two back
in step.

## Open questions

- Should the claude memory step's placed entry carry the store it was
  registered with (as a setting carries `value`), or should bring-in
  recording read the store back from `~/.claude.json`?
