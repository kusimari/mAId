# Backlog: reader-json-edges

## What

Say in reader rule 1 what a reader does with JSON text that parses in some
languages and not others, and add a conformance case for each:

- a number beyond double range (`1e400`);
- a `\u` escape that decodes to a lone surrogate (`"\ud800"`).

## Why

mAId's reader (serde_json) rejects both as not JSON, so a part list holding
one is refused and a config file holding one is unreadable. Python and JS
parse both (`inf`, a lone surrogate), so their readers check the entry
instead. Two readers that follow the rules today can report different
states and exit codes. Seen while writing rule 14's string-only `value`
(`reader-rules-values`).

## Open questions

- Likely rule: both make the text not JSON, matching mAId's reader, since
  rule 1 already reads text strictly.
