# Backlog: notes-mcp-server

## What

Promote the v1 `notes` skill (markdown-only, agent follows
the contract using its own filesystem tools) into a typed
MCP server consumed by every harness mAId deploys to —
Claude Code, Kiro, MeshClaw, future Claude Desktop / AWS
Q-Desktop.

Tools the server would expose:

- `add_note(text, kind?, topics?, due?, with?, audio_path?, transcript?)`
- `find_related(query)`
- `list_by_topic(topic)`
- `list_recent(kind?, n?)`
- `transcribe_audio(audio_path)` — wraps whisper /
  whisper-cpp, returns transcript text

## Why

Five wins over the v1 skill-only shape:

1. **Stable tool names** — the agent doesn't need to
   re-read SKILL.md and re-derive the classifier each turn.
2. **Atomic writes** — server takes a file lock per
   capture; rules out two captures stomping on the same
   slug or person file.
3. **Bundled transcription** — `transcribe_audio` is a
   typed tool with a single supported invocation path,
   instead of every harness shelling out to whisper.
4. **Structured retrieval** — `find_related` can return a
   typed list (path + score + excerpt) instead of asking
   the user to paste a Dataview block.
5. **Cross-harness uniformity** — one server, one config
   merged into each consumer's MCP config file
   (`~/.claude.json`, `~/.kiro/settings/mcp.json`,
   `~/.meshclaw/mcp.json`).

## Open questions

- **Runtime.** Deno (matches mAId stack) using
  `npm:@modelcontextprotocol/sdk`, or Node + TypeScript?
- **mAId platform work.** New registry kind
  `mcp-server` with surgical JSON-merge deploy
  (`__managed_by: "maid"` marker pattern, mirroring
  `Gorantls-agents/items/meshclaw/item.py`). Schema
  validator extension for the manifest shape. New tests
  for JSON-merge deploy/undeploy idempotence.
- **Server discovery of vault path.** `$NOTES_VAULT` env
  var passed through the server config, or per-vault
  marker file (`$NOTES_VAULT/.notes-vault.yaml`)?
- **Concurrency.** POSIX `flock` or advisory file lock?
  How does the server behave under cross-machine sync
  conflicts (iCloud / Dropbox creates conflict copies)?
- **Embedding-based retrieval.** Once the server is in
  place, semantic search becomes feasible — local
  embeddings (via Ollama? sentence-transformers?) indexed
  on capture. Cost vs. value: TBD until v1 has real
  notes.

## Trigger to promote

Promote when one of these is true:

- Two simultaneous captures collide (file overwrite or
  duplicate slug).
- The classifier is consistently wrong on a kind, and a
  typed tool with explicit `kind` parameter would have
  prevented it.
- Whisper invocation diverges across harnesses (one calls
  it differently from another).
- A new harness is added that doesn't have shell access
  (no Bash) but does support MCP.
