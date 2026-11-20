# Backlog: ghost-on-other-shells

## What

Carry the ghost on codex, kiro-cli and agy as it is carried on Claude
Code: an adapter per shell that places the genome at session start,
expression on each prompt and the sleep mark at the end, plus a session
source so reflection works there. The initiative `harness-core` maps each
shell's carriers in its System design; only Claude Code's are built.

## Why

Today the README and project.md say only Claude Code carries the ghost;
on the other shells the skills work as plain skills, the reflect skill
says reflection is not available, and nothing is learned from those
sessions.

## Open questions

- The probes the System design marks *(test)* (kiro's agentSpawn and
  session files, codex's session end) decide each adapter's shape.
- codex reads the same plugin dir as Claude Code, so its hooks need their
  own file name, as Claude's do.
