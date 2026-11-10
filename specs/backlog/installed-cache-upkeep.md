# Backlog: installed-cache-upkeep

## What

The Claude adapter's cache under `<state>/shells/claude/` keeps one file
per location and none is ever removed. Prune old ones (the ghost's uninstall removes the
dir since ghost-package). Also stamp what changes
inside a directory marketplace in place (a plugin's `skills/` and
`.claude-plugin/`), not only its resolved root.

## Why

Left from live-shell's review: the files hold what `.claude.json` holds
(`0600`, args included), and a developer marketplace edited in place keeps
a stale skill list until another record changes.
