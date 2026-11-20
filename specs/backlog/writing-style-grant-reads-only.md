# Backlog: writing-style-grant-reads-only

## What

Narrow Claude's grant on writing-style's dir under mAId's config dir from
`Edit(/<config>/writing-style/**)` to read access, once the person's
older `learned.md` and `promoted.md` rules have moved into the skill.

## Why

writing-style no longer writes there: a taught rule is saved to the
persona's memory, and the older files are only read until their rules
move into the skill's text. An `Edit` grant lets any session write rules
a later session loads; reads alone are all the skill needs now.

## Open questions

- `reads::List::Allow` recognises only `Edit(...)` rules as mAId's; a
  read rule needs its own shape, and uninstall must still remove the
  earlier `Edit` rule.
- Once no person has older files left, should the grant go entirely?
