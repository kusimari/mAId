# Backlog: ghost-install-scope

## What

When `ghost install` finds that Claude Code lacks this checkout's mAId
plugin, it installs plain for every agent (`plain::install::all(root,
None)`: claude, kiro, codex, agy). Limit it to Claude Code
(`Some(Agent::Claude)`), the only shell the ghost is in, and say so in
the install's output, its `--dry-run` and project.md's "The ghost's
install".

## Why

The ghost needs only Claude Code's plain. A person who ran `just install
claude`, or who keeps another agent on an older mAId on purpose, has
every other agent rewritten by a ghost install they read as Claude-only.
The final Macro run had to check by hand that the ghost would reuse plain
before installing it, to keep its scratch world to one agent.
`ghost install --dry-run` now says when it would write for every agent;
this item stops it doing so.

## Open questions

- Recommended: limit to Claude. When the ghost gains another shell
  (`ghost-on-other-shells`), it installs plain for each shell it is
  installed in, no more.
- Whether `ghost install` should also refuse, rather than install plain,
  when another agent's plain is from a different revision. Recommended:
  no; it is not the ghost's to judge.
