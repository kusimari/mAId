---
name: installed-skills-never-need-repo
description: Once mAId is installed, no skill should send the runtime to the mAId checkout, and every runtime file a skill reads or writes must be reachable without a permission prompt.
metadata:
  type: backlog
---

## What

Once installed, a session never needs the mAId repo:

- No skill tells the runtime to find, read or edit the repo. Promoting a
  learned rule is a runtime action that writes a user-owned file the skill
  loads on top of its shipped body (for writing-style, a `promoted`
  section under `${XDG_CONFIG_HOME:-~/.config}/maid/writing-style/`). The
  repo changes only when the maintainer chooses to.
- Install grants the agent read and write to the user-owned files skills
  use (`${XDG_CONFIG_HOME:-~/.config}/maid/**`), the same way it already
  grants reads of the installed plugin tree, owned and recorded per the
  part rules.
- Shipped learned rules move out of the installed `SKILL.md` into the
  user file at install, so there is one runtime source.
- A check flags skill text that points the runtime at a repo path
  (`resources/content/...`, "in a mAId checkout", `just install`).

## Why

Seen in a session on 2026-10-13 (writing-style promotion): the skill told
the agent to edit `resources/content/skills/writing-style/SKILL.md` "in a
mAId checkout", so it edited the repo source, which Claude does not read
at runtime, and the edit had to be reverted. The installed copy is
read-only, so promoted rules had nowhere to go. `learned.md` under
`~/.config/maid/writing-style/` prompted on every read and write, because
only the installed plugin tree is granted.

## Open questions

- Grant `~/.config/maid/**` through `permissions.additionalDirectories`
  (as the plugin-read grant does) or through allow rules: the first also
  allows edits there, which is the intent for user-owned skill state.
- Whether other skills (notes, kdevkit) carry the same repo pointers.
