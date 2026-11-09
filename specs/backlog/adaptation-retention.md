# Backlog: adaptation-retention

## What

Give the core's adaptation state an end. Sleep marks under
`<state>/sleep/`, capped experience bundles under `<state>/experience/`
and `<state>/ledger.jsonl` only grow: every Claude Code session is marked
when it ends, automated `claude -p` runs included, and a decided or
dismissed session keeps its mark and its bundle. Prune decided marks and
bundles past an age, and decide how long the ledger keeps per-session
records (or roll them up into counts).

## Why

Left from the sleep-and-adaptation stream (ringmaster review: retention
is a backlog item). The `briefed/` full copy is already deleted once a
batch is settled, and "dismiss" lets the person clear waiting sessions,
but nothing removes what stays. Sibling of `core-delivery-record-upkeep`,
which covers the delivery record's session dirs.

## Open questions

- The age to keep: the profile's 30 days, or longer for the ledger,
  since "getting better" is measured across many sessions.
- Where pruning runs: at `sleep`, at `genome`, or a command the person
  runs.
- Whether automated sessions should be marked at all (a way to tell a
  person's session from a script's).
