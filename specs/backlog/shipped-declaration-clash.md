# Backlog: shipped-declaration-clash

## What

When two enabled plugins ship a declaration for one MCP server or one
overlay name, say so (a clash, naming both parts) instead of taking the
first in plugin-id order. Consider keying parts on the full repository URL
and showing only its last segment, so a look-alike repository does not
join another's part.

## Why

Left from live-shell's review: no install has two today, and a unit's
source stays accurate either way, so a proposal still goes to the repo
that declared it.
