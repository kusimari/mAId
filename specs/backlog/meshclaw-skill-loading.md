# Backlog: meshclaw-skill-loading

## What

Make mAId-authored skills (`notes`, `writing-style`, future
ones) reachable from MeshClaw conversations on this
machine, so a Slack DM "Add note for X" or "format this in
my style" produces the same behaviour as the same prompt in
local Claude Code.

The integration point is **Gorantls-agents** — the harness
that already installs MeshClaw on this machine and manages
surgical merges into `~/.kiro/settings/mcp.json` and
`~/.kiro/agents/<agent>.json` via the `__managed_by`
pattern (see
`Gorantls-agents/items/meshclaw/item.py:227-247`).

## Why

mAId is becoming the user's *personal agentic system* —
one source of truth for skills/agents/commands deployed
into every harness. MeshClaw is the Slack-driven harness;
without skill-loading there, the user has to context-switch
out of Slack to capture or format anything. Once wired,
"add note for: <thing>" works as a Slack DM and the vault
on disk receives the same file as a local Claude Code
session would have written.

## Open questions

- **Which Gorantls-agents item owns this?** A new
  `items/maid-skills/` that walks `~/.claude/skills/`
  (which is already a symlink into the mAId checkout) and
  injects each skill name into MeshClaw's kiro agent
  spec? Or a per-skill item (`items/notes-skill-ref/`,
  `items/writing-style-ref/`)?
- **What field in MeshClaw's kiro agent spec carries
  skills?** Read
  `Gorantls-agents/items/connected-workspace/agent-spec.json`
  for the shape; check for `instructions[]`, `skills[]`,
  or `mcpRegistry` extension.
- **Reload behaviour.** MeshClaw daemon caches agent
  config at startup. After Gorantls-agents writes the
  merge, does the user need to restart `meshclaw
  gateway`, or is there a config-reload hook?
- **Decoupling from mAId checkout location.** The
  Gorantls-agents item must reference the deployed path
  (`~/.claude/skills/<name>/SKILL.md` — a symlink), not
  the mAId checkout. Otherwise moving the checkout breaks
  MeshClaw.
- **Scope.** Today the mAId skills are: `kdevkit`,
  `notes`, `writing-style`. Wire all three? Or only
  `notes` + `writing-style` (the user-facing ones), since
  `kdevkit` is for development-mode work that doesn't
  apply in a Slack DM?

## Acceptance

A Slack DM "Add note for: review the Coral migration
design Tuesday" → MeshClaw replies with the path of the
created note → file appears at
`$NOTES_VAULT/reminders/2026-05-DD-…md` with the same
frontmatter as a local Claude Code capture would produce.
Same vault, same shape, different harness.
