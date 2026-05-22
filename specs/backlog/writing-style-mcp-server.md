# Backlog: writing-style-mcp-server

## What

Promote the v1.1 `writing-style` skill (markdown-only,
agent follows the contract using its own Edit tool) into a
typed MCP server consumed by every harness mAId deploys
to.

Tools the server would expose:

- `format_passage(text, mode?)` — returns `{ rewritten,
  changes: [{ rule, before, after, section }] }`. Typed
  change log instead of free-form bullets.
- `lint_passage(text)` — returns
  `[{ rule, span, suggestion }]`. The strict-mode hook.
- `set_strict(enabled)` — server-side persistent flag at
  `$XDG_CONFIG_HOME/writing-style/state.json`.
- `teach_rule(rule, example?, source?)` — appends to
  `## Learned rules` under a POSIX `flock`. Returns the
  resulting diff hunk so the agent can show it.
- `list_pending()` — lists current `## Learned rules`
  entries with proposed target sections.
- `promote_rule(entry_id, target_section)` — atomically
  edits the body section and removes the learned-rules
  entry. Returns the new SKILL.md sha so callers can
  detect concurrent edits.

## Why

Five wins over v1.1:

1. **File-locked teach writes.** Two parallel sessions
   teaching at once is a race in v1; the server takes a
   `flock` so writes serialize. v1 documents this as a
   known limit.
2. **Persistent strict mode.** v1 is session-scoped — the
   user must re-enable each session. Server persists the
   flag.
3. **Typed change log.** v1 returns prose bullets; the
   typed shape is renderable as a UI diff in harnesses
   that support it (Slack reactions, Claude Desktop UI).
4. **Atomic promotion.** Editing the body section AND
   removing the learned-rules entry as one write removes
   the can-promote-but-not-clean-up failure mode.
5. **Cross-harness uniformity.** Same server, same flag
   state, same teach store across Claude Code, MeshClaw,
   future harnesses.

## Open questions

- **Embedding hook.** The v3 vision is "writing-style
  learns from notes". An MCP server is the natural place
  to embed `#style`-tagged notes from the notes vault and
  pull them as in-context examples for `format_passage`.
  Worth designing the hook now or punt to v3?
- **Diff-rendering for change log.** Returning structured
  changes opens UI possibilities (Slack message with
  reactions to accept/reject each fix). Define the
  rendering contract per harness or punt?
- **Promotion target inference.** v1 has the agent guess
  which section a rule belongs in. Server can do this
  too, but should it ask for confirmation? Default to
  "ask"; flag for "auto-promote" if user opts in.

## Trigger to promote

- Two simultaneous teach turns lose data (race
  documented in v1 known limits actually fires).
- User keeps re-enabling strict mode every session and
  wants it to stick.
- A new harness without good Edit-tool support but with
  MCP support arrives (e.g. a phone app).
