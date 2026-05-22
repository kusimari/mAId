# Backlog: notes-skill-design

> **RESOLVED 2026-05-22.** Promoted to
> `specs/feature/notes-skill.md`. Open questions answered:
> single "Add note for X" verb with classifier; Obsidian
> vault at `${NOTES_VAULT:-$HOME/notes}` with the layout
> below; `[[wiki]]` + `#tag` linking; capture + retrieval
> with Obsidian-plugin-first ladder (Smart Connections /
> Dataview) and ripgrep fallback; transcripts stored, audio
> never copied. Embedding-based search and the v2
> MCP-server promotion tracked separately at
> `specs/backlog/notes-mcp-server.md`. Original questions
> below preserved for the audit trail.


## What

Design the notes/knowledge skill whose SKILL.md currently ships
as a `status: stub` placeholder. The skill should let me capture
four kinds of items into a personal knowledge store and connect
them semantically:

- Reminders / to-dos ("remember this, I want to work on it
  later").
- Insights — thoughts worth keeping, with links into one or more
  related topics.
- 1:1 and interview notes.
- Conversation uploads — audio or transcript + pre-amble, stored
  as a referenceable artefact.

## Why

Every coding/chat session surfaces useful material that slips
away because there's no place to land it. Today I rely on
scattered files and memory. An Obsidian-like store gives me a
durable substrate that connects threads across conversations.
This unblocks the "connect my thoughts to related ones" pattern
that a flat note file can't do.

Shipping this as a skill (rather than an external app) keeps
capture inline with the session I'm already in — the assistant
offers to write it down rather than me context-switching.

## Open questions

- **Store format.** Obsidian vault (markdown + `[[wiki-links]]`)
  is the leading candidate because of tool compatibility and
  graph semantics. Alternatives: flat markdown with YAML
  frontmatter; SQLite-backed store.
- **Store location.** Inside the repo? A separate checkout?
  `$HOME/notes/`? Relationship to git — sync, or just
  filesystem?
- **Link conventions.** `[[topic]]` wiki-links, tags (`#topic`),
  or both? How do insights "thread into multiple topics" in
  practice?
- **Read/write/search API.** What does the skill actually do at
  runtime? `append-note`, `find-related`, `list-by-topic`?
  Write-only for first pass, or search from day one?
- **Conversation audio.** Where does the audio file live? Do we
  run transcription locally, or assume the user provides a
  transcript? What metadata attaches to it?
- **Templates per kind.** Reminders/insights/1:1/conversation
  each need a different frontmatter shape. Define them up front
  or let the skill evolve them?
- **Scope boundary.** Does this skill only capture, or also
  surface — "hey, this reminds me of a note you took two weeks
  ago"?
