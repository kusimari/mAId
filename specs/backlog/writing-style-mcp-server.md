# Backlog: writing-style-mcp-server

## What

Give the `writing-style` skill a small typed MCP server, consumed by
every harness mAId deploys to, for the parts that are still prose the
agent follows by hand:

- `format_passage(text, mode?)`: returns `{ rewritten, changes: [{ rule,
  before, after, section }] }`, a typed change log instead of free-form
  bullets.
- `lint_passage(text)`: returns `[{ rule, span, suggestion }]`, the
  strict-mode check.
- `set_strict(enabled)`: a strict-mode flag that persists across
  sessions.

Taught rules are out of scope: they are saved to the persona's memory
through the memory server's own tools, and a reflection offers to move
them into the skill's text (project.md, "Skill state" and Adaptation).

## Why

1. **Persistent strict mode.** Today it is session-scoped; the user
   re-enables it every session.
2. **Typed change log.** Structured changes can be rendered as a diff in
   harnesses that support it.
3. **Cross-harness uniformity.** The same server and flag state in every
   harness that loads MCP servers.

## Open questions

- **Embedding hook.** Should the server pull `#style`-tagged notes from
  the notes vault as in-context examples for `format_passage`, or is
  that later?
- **Diff rendering.** Define a rendering contract per harness for the
  typed change log, or leave it to each harness?
- **Where the flag lives.** A file in writing-style's config dir, or the
  persona's memory like the taught rules?

## Trigger to promote

- The user keeps re-enabling strict mode every session and wants it to
  stick.
- A harness without good edit-tool support but with MCP support arrives.
