# Backlog: resolved-grant-double-slash

## What

The resolved-path form of the skill-state Edit rule is written with a
doubled slash before the glob (`Edit(//<resolved>/writing-style//**)`)
when the resolved config dir ends in a slash. Normalise the path before
building the rule so both forms read `.../writing-style/**`.

## Why

Seen on a real install behind a symlinked HOME (2026-10-13). A manual-mode
write into the dir still went through without a prompt, so it is
cosmetic today, but it is a rule a reader cannot recognise at a glance and
it depends on Claude tolerating the doubled slash.

## Open questions

- None; recognise and remove the old doubled form on the next install
  (it is mAId's recorded value).
