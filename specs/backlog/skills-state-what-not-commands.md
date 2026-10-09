# Backlog: skills-state-what-not-commands

## What

Try writing kdevkit (and the other skills) with no commands at all:
each instruction says what must happen and what must be true after,
and the coding agent running the skill picks the command or tool for
its own CLI. Today's shape, from `specs/project.md` ("It has to
outlast its agents"), is what-then-how: a generic rule plus per-agent
how lines checked against each CLI. This item asks whether the how
lines can go.

## Why

Every how line is a maintenance cost: a new agent, or an upgrade that
renames a flag, means re-checking and editing them. If agents reliably
find their own way from a well-stated what, the skills get shorter and
portable for free.

## Questions to settle

- Where does a bare what fail? Candidates: sandbox and confinement
  profiles (codex's grant is not something an agent would guess),
  dispatching a fresh-context agent, resuming a session, making a
  dispatched agent's output land in a file. These may be the places a
  how earns its keep, or where the what needs to be sharper.
- What evidence decides it: an A/B on the existing fixtures, with the
  how lines stripped in one arm, run on claude, kiro and codex, both
  arms audited for equal demand (`specs/project.md`, Testing).
- If some hows must stay, where they live: a small per-agent appendix
  the skill points at, rather than inline in the rule.

## Done when

The A/B has run, and kdevkit either drops its how lines or keeps only
those the A/B showed an agent cannot do without, with that recorded.
