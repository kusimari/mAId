# Backlog: mcp-remove-ownership

## What

Make the browser MCP's claude and codex registrations ownership-checked,
as kiro's is: install writes `chrome-devtools` only when it is absent or
already runs mAId's launcher, and uninstall removes it only when it runs
mAId's launcher. A same-named server another tool registered is left
alone and recorded as skipped, "not mAId's".

## Why

Rule 2 ("a part owns only what it wrote"). Today `resources/browser/manage`
removes and re-adds claude's entry by name, and replaces and removes
codex's by name, so another tool's `chrome-devtools` would be overwritten
on install and deleted on uninstall. Kiro's registration got the check
in `kiro-global-mcp`, since mAId edits that file itself.

## Open questions

- How to read a server's command: `codex mcp get` has `--json`;
  `claude mcp get` prints text only, or claude's config file is read
  directly.
