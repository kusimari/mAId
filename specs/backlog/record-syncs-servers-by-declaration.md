# Backlog: record-syncs-servers-by-declaration

## What

`build-tool record --kind mcp` keeps the browser server's bring-in in
step with its harness entries through a sync hard-coded to
`chrome-devtools` (`stages::sync_browser_bring_in`). Replace it with one
keyed on the recorded server's own declaration: any MCP entry whose
server ships a `resources/content/servers/<name>/server.json` gets its
bring-in added or dropped the same way.

## Why

`record` should stay a recorder, and a second MCP server with its own
step outside install would otherwise need its own copy of the sync. Do
it before such a server appears.

## Open questions

- Map the harness's listed name to the server dir by the manifest's
  `name`, or by a field in the declaration?
