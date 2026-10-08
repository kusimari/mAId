---
name: install-link-takeover-gaps
description: Two gaps left by the profile install's link takeover - uninstall without --force skips a link an older mAId install left, and a codex link into an old checkout for a skill that no longer exists is never reaped.
metadata:
  type: backlog
---

# Install link takeover gaps

## What

- `uninstall` without `--force` skips (and exits 1 on) a link that
  points at another mAId skills tree. `install` applies the "mAId's own
  link" rule (`is_maids` in `deploy.rs`); `remove` does not.
- A `~/.codex/skills/<name>` link into a pre-profile checkout, for a skill
  no longer shipped, is outside `FanOut`'s expansion (it only scans links
  under the current source), so it stays dangling and unreported.

## Why

Both were review notes on `setup-as-installable`, left because install
takes over the common case first. Native plugins (initiative
`installable`, stream 2) remove the claude and codex links; if that stream
reaps old links with the same rule, close this item there.
