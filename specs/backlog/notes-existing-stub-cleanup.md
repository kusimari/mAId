# Backlog: notes-existing-stub-cleanup

## What

One-shot cleanup of the ~180 empty `topics/<topic>.md` stubs that
already exist in the personal notes vault. Each is a substance-less
`kind: topic` frontmatter + `# <title>` file (most 26–50 bytes) —
minted by the old `notes` skill rule that auto-created a stub on every
`[[topic]]` reference. That rule is now changed (a `[[topic]]`
reference leaves the link dangling; a page is created only with real
substance), so this is the go-forward-behavior's matching backfill:
delete files that are frontmatter + bare heading only, leaving their
inbound `[[wikilinks]]` dangling (the desired end state — Obsidian
renders unresolved links fine).

## Why

The rule change (shipped in `notes-and-behavioral-verification`) only
affects go-forward behavior; the ~180 pre-existing stubs stay until
cleaned. They inflate the vault, clutter search, and pad the Obsidian
graph with information-free nodes. Deleting them realizes the full
intent of the rule change.

This was deliberately kept out of the rule-change feature: it's a bulk
mutation of the user's *personal vault* (user data), not repo content,
so it must be done deliberately, by hand, outside this repo — not as a
side effect of a skill CR.

## Open questions

- Bright line for "empty stub": frontmatter + a single `# <title>`
  heading and nothing else? Confirm no stub accidentally accrued a
  `topics:` link or a sentence that would make it real substance.
- Do it as a scripted pass (grep for files matching the stub shape,
  review the list, `rm`) or manually? A dry-run listing first,
  regardless.
- Does deleting turn any *Dataview/ripgrep retrieval* queries stale?
  (Shouldn't — they key on `[[X]]` outlinks + frontmatter `topics:`,
  not topic-file existence — but confirm on the real vault.)
