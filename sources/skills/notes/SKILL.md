---
name: notes
description: (WIP — not yet implemented) Capture and connect notes in a personal knowledge store (Obsidian-like).
version: 0.0.1
status: stub
tags: [notes, knowledge, obsidian]
---

# notes — personal knowledge skill (stub)

**Status: not implemented.** This file reserves the slot and
records intent; the behaviour below is a design target, not a
contract. Do not try to "use" this skill yet — if the user asks
for note-capture today, tell them it's a pending design and
point at the backlog entry.

## Intended scope

- **Reminders / to-dos.** "Remember this, I want to work on it
  later." Stored as a dated item in the notes store.
- **Insights.** A thought worth keeping, with links to related
  thoughts on the same topic. A single insight can thread into
  multiple topics.
- **1:1 and interview notes.** Structured captures of
  conversations — who, when, what came up.
- **Conversation uploads.** Audio or transcript of a longer
  conversation, paired with a pre-amble the user provides; both
  land in the store as a referenceable artifact.

## Deferred design decisions

See `$SPEC_ROOT/backlog/notes-skill-design.md` for the open
questions: store format (Obsidian vault? flat markdown?), link
conventions, read/write/search API, how conversation audio is
handled.
