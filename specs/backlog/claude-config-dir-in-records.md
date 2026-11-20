# Backlog: claude-config-dir-in-records

## What

Record which Claude config dir each of mAId's Claude entries was placed
in (the settings elements, the auto-memory value, the legacy memory dir,
the `personal-memory` server), and act on a record only in that dir. A
record with no dir reads as `$HOME/.claude` (and `$HOME/.claude.json`).
When the dir a run resolves differs from the recorded one, either remove
from the recorded dir or keep the record and say "recorded in <dir>, not
touched".

## Why

Left from the claude-config-dir stream (correctness review, medium). Since
install follows `CLAUDE_CONFIG_DIR`, the part list says what mAId placed
but not where:

- A person who set the variable before that stream has mAId's entries in
  `~/.claude/settings.json`. The next uninstall looks in the variable's dir,
  finds nothing, and clears the records, so the copies under HOME stay,
  owned by no one.
- Install with one dir, then run with another: uninstall takes a value
  equal to the recorded one in the second dir as mAId's, though the person
  set it there.

No one is known to be in either case today: the only run with the
variable set (Macro test run 3) stopped before install.

## Open questions

- One dir per part list, or per item.
- Whether a mismatch removes from the recorded dir, or only reports.
