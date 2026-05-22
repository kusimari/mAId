# Feature: notes-skill

## Git Setup

- Branch: `feat/notes-skill` (off `main`)
- Base: current tip of `main`

Promoted from `specs/backlog/notes-skill-design.md`. The backlog
file's What/Why are folded into the Feature Brief and
Requirements below; once this spec lands, the backlog file gets
a resolution header pointing here.

## Feature Brief

`sources/skills/notes/` ships today as a `status: stub`
placeholder. Every coding/chat session surfaces material worth
keeping — reminders, insights, 1:1 takeaways, longer
conversation transcripts — but there's no skill to land it
into a durable store, so it slips away.

This feature ships v1 of the notes skill: a single-verb
capture entry point ("Add note for &lt;X&gt;") that classifies
the note kind, writes it into an Obsidian-shaped vault under
`$HOME/notes/`, and links it via `[[wiki-links]]` and `#tags`.
Retrieval is in scope for v1 — the skill prefers existing
Obsidian plugins (Dataview, Omnisearch, Smart Connections,
Local Graph), falls back to `ripgrep` when plugins don't
answer the query, and explicitly defers embedding-based
semantic search to a future milestone.

The skill is capture-only at the volunteering level — it never
interrupts a session with "want me to capture that?". The user
drives every capture via the verb.

## Requirements

1. **Single invocation verb.** "Add note for &lt;X&gt;"
   triggers the skill. The skill classifies kind from `<X>` —
   no separate per-kind verbs.
2. **Four note kinds.** Reminder, insight, 1:1/interview note,
   conversation upload. Each has a frontmatter template and a
   destination directory.
3. **Vault location.** `$HOME/notes/` by default. Configurable
   via `NOTES_VAULT` env var; the skill respects it if set.
4. **Vault layout (auto-created).** `inbox/`, `reminders/`,
   `insights/`, `people/`, `conversations/`, `topics/`. Skill
   creates missing directories on first capture.
5. **Linking.** `[[topic]]` for first-class topic pages
   (auto-create a stub in `topics/<topic>.md` when a topic is
   referenced for the first time); `#tag` for loose
   categorization. Insights and 1:1s carry `topics:` in
   frontmatter; conversations carry `tags:`.
6. **Retrieval ladder.** When the user asks "find notes
   related to &lt;X&gt;" / "list notes by topic &lt;X&gt;" /
   "list recent &lt;kind&gt;":
   1. **Primary path: Obsidian plugins.** Skill emits the
      Dataview / Omnisearch / Smart Connections / Local Graph
      query the user pastes into Obsidian. Skill must document
      which plugin answers which question.
   2. **Fallback: ripgrep.** When the user prefers terminal /
      no Obsidian, skill runs `rg` over the vault with topic /
      tag / kind filters and returns hits.
   3. **Out of scope for v1:** embedding-based semantic search.
7. **Conversation transcripts.** User may pass a transcript
   string or an audio path. If audio: skill transcribes with a
   default tool (whisper.cpp via `whisper` on `$PATH`, or
   `whisper-cpp` if present). Skill stores the transcript
   only — never persists the audio file.
8. **Capture-only volunteering.** Skill never spontaneously
   asks "want me to capture that?" — every capture is
   user-driven.
9. **Plays nicely with Obsidian.** Frontmatter shape, link
   syntax, and folder layout are valid Obsidian conventions.
   User can sync the vault their own way (git, iCloud, none).
   Out of scope for the skill.

## Design

### Classification

The classifier reads the suffix after "Add note for" and routes
based on shape:

- Starts with `remind me to …`, `remember to …`, contains a
  date/time hint, or is phrased as a TODO → **reminder**.
- Names a person and frames the note around them ("from my 1:1
  with Alice", "interviewing Bob") → **1:1 / interview**.
- Carries an explicit kind tag in the call ("insight: …",
  "conversation: …") → that kind.
- Long-form text with `transcript:` / `audio:` keys, or a
  pasted transcript / audio path → **conversation**.
- Default → **insight**, with a clarifying note in the file's
  frontmatter that classification was implicit.

When ambiguous, the skill asks **one** question to disambiguate
before writing.

### Vault layout

```
$HOME/notes/                           ← or $NOTES_VAULT
├── inbox/                             unsorted captures (rare; tie-breaker)
├── reminders/<YYYY-MM-DD>-<slug>.md
├── insights/<YYYY-MM-DD>-<slug>.md
├── people/<person>.md                 one file per person; entries appended
├── conversations/<YYYY-MM-DD>-<slug>.md
└── topics/<topic>.md                  auto-stub on first [[topic]] use
```

People files are append-only logs (`## 2026-05-21 1:1` headers
under a single per-person file), not a new file per
conversation. Lets the graph see "Alice" as one node.

### Frontmatter templates

All four kinds share `date`, `kind`, `links`. Reminders add
`due`. Insights add `topics`. 1:1s add `with`. Conversations
add `source`, `tags`.

```yaml
# reminders/2026-05-21-review-coral.md
---
date: 2026-05-21
kind: reminder
due: 2026-05-26
links: [[Coral migration]]
---

Review the Coral migration design with the team Tuesday.
```

```yaml
# insights/2026-05-21-wadler-immutable-doc-trees.md
---
date: 2026-05-21
kind: insight
topics: [pretty-printing, wadler]
links: [[pretty-printing]], [[wadler]]
---

Wadler's prettier algorithm needs immutable doc trees because
the `<>` combinator must be associative for the optimal-layout
reduction to terminate without quadratic blowup …
```

```yaml
# people/alice.md (append a section)
---
kind: person
---

## 2026-05-21 1:1
links: [[Coral migration]], [[Q3 roadmap]]

- alice owns Q3 roadmap rewrite, due end of June
- blocked on infra approval; will follow up Friday
```

```yaml
# conversations/2026-05-21-coral-migration-design.md
---
date: 2026-05-21
kind: conversation
source: audio:~/Downloads/coral-design.m4a
tags: [#coral, #design-review]
links: [[Coral migration]]
---

## Pre-amble

Bob, Alice, and I walked through the Coral migration design
doc; goal was to lock the storage shape before the design
review on Friday.

## Transcript

…
```

### Topic stubs

When a `[[topic]]` reference is written and `topics/<topic>.md`
doesn't exist, the skill creates a one-line stub:

```yaml
---
kind: topic
---

# <topic>
```

This makes the wiki-link resolve in Obsidian's graph view and
gives the user a place to grow notes about the topic over
time. The skill never overwrites an existing topic stub.

### Retrieval API

Three queries, each with the plugin-first ladder:

| Query                     | Obsidian plugin      | Fallback                        |
| ------------------------- | -------------------- | ------------------------------- |
| "find notes related to X" | Smart Connections    | `rg -l 'X|\[\[X\]\]|#X'`        |
| "list notes by topic X"   | Dataview             | `rg -l '\[\[X\]\]\|^topics:.*X'` |
| "list recent &lt;kind&gt;" | Dataview             | `find <kind>/ -mtime -<N>`     |

The skill emits the Dataview query as a code block the user
can paste, and offers to run the ripgrep fallback inline if
the user prefers terminal output.

### Audio transcription

When `<X>` carries `audio: <path>`:

1. Resolve `<path>` (expand `~`).
2. Probe `$PATH` for `whisper` (OpenAI whisper CLI), then
   `whisper-cpp`. First found wins.
3. Run `<tool> <path> --output-format txt --output-dir <tmp>`.
4. Read the resulting `.txt`, place it under `## Transcript`.
5. Delete the temp directory. The original audio file is left
   where it is — the skill never deletes user files; it just
   doesn't reference the audio path in the stored note's body
   beyond the `source: audio:<path>` frontmatter line.

If neither tool is on `$PATH`: skill reports "no transcription
tool available; pass a transcript instead" and stops without
writing anything.

## Test Strategy

- **Unit tests:** none new for the skill content itself —
  `SKILL.md` is markdown the assistant follows. The schema
  validator (`maid/schema.ts`) tests already cover frontmatter
  shape.
- **Structural smoke** (`./tests/functional/run --no-tools`):
  confirms `~/.claude/skills/notes/SKILL.md` resolves to the
  updated source.
- **Tool smoke** (`./tests/functional/run`):
  - Drive `claude --print` with "Add note for X" — assert
    `[notes] applies` announcement (per the existing fixture
    convention).
  - Don't write to a real `$HOME/notes/` from the fixture; it's
    a session-announcement check only.
- **Manual end-to-end** (executed by the user):
  - Reminder, insight, 1:1, conversation-from-transcript,
    conversation-from-audio paths each tried at least once.
  - "Find notes related to X" returns a Dataview block + an
    offer to run ripgrep.
  - Topic stub auto-created on first `[[new-topic]]` use.
  - Existing topic stub not overwritten.

## Implementation Plan

1. **Resolve backlog** — append a resolution header to
   `specs/backlog/notes-skill-design.md` pointing at this
   feature spec. (Don't `git mv`; the backlog file itself
   captured the open questions, and the resolution is part of
   the audit trail.)
2. **Rewrite `sources/skills/notes/SKILL.md`** — frontmatter
   to `version: 1.0.0`, drop `status: stub`, refine
   `description`. Body covers: invocation verb,
   classification rules, vault layout, frontmatter templates
   per kind, topic-stub behavior, retrieval ladder,
   transcription flow, capture-only stance.
3. **Add `tests/functional/<fixture>.smoke`** — match the
   shape of existing fixtures; one-line prompt that forces
   `[notes] applies` announcement.
4. **Run gates** — `deno task fmt && deno task lint && deno
   task check && deno task test && ./tests/functional/run
   --no-tools`. All green before push.
5. **Conventional commit** — `feat(notes): ship v1 capture +
   retrieval skill`. Single commit; backlog resolution
   included.
6. **Push branch** — `git push -u origin feat/notes-skill`.
   PR is a human decision.

### Risks

- **Audio transcription portability.** `whisper` may not be on
  `$PATH` on a fresh machine. Mitigation: graceful "no tool
  available" message, no crash; user can install whisper or
  pass a transcript. Documented in the skill body.
- **Vault shape drift.** If the user manually reshapes the
  vault before the skill is exercised, the skill's
  auto-creation may collide. Mitigation: skill reads the
  existing layout before writing — if `reminders/` is missing
  but `todo/` exists, skill asks rather than creates.
- **Plugin availability.** Skill emits Dataview queries even
  if Dataview isn't installed. Mitigation: skill mentions the
  plugin requirement in its retrieval section, and the
  ripgrep fallback always works without Obsidian.

## Session Log

<!-- Newest at top -->

- 2026-05-21 · feature spec drafted from backlog +
  in-conversation decisions (single verb, Obsidian patterns,
  plugin-first retrieval ladder, transcript-only conversation
  storage, capture-only volunteering).

## Decision Log

<!-- Newest at top -->

- 2026-05-22 · **MeshClaw out of scope for v1.** v1 ships
  skills that work in any AI system loading
  `~/.claude/skills/` or `~/.kiro/steering/skills/`. MeshClaw
  installs through Gorantls-agents, which already does
  surgical merges into kiro config; wiring the skills there
  is a separate feature designed against Gorantls-agents,
  not bolted on here. Tracked in
  `specs/backlog/meshclaw-skill-loading.md`.
- 2026-05-22 · **Skills-first, MCP deferred.** v1 is plain
  markdown — the agent follows SKILL.md and uses its
  existing Bash/Write tools. v2 promotes parts that benefit
  from MCP (audio transcription pipeline, atomic writes,
  structured retrieval, concurrency-safe state). The split
  is driven by friction observed in v1, not speculation.
  Tracked in `specs/backlog/notes-mcp-server.md`.
- 2026-05-21 · **Single verb over per-kind verbs.** "Add note
  for X" with classifier vs. `capture-reminder /
  capture-insight / …`. Single verb wins on UX —
  classification can be wrong but is one disambiguation
  question away from correct, vs. four verbs the user must
  remember. Trade-off: classifier needs explicit-kind escape
  hatches (`insight: …`, `conversation: …`).
- 2026-05-21 · **Obsidian-plugin-first retrieval.** Plugins
  cover the common cases without the skill maintaining an
  index. Ripgrep fallback handles terminal-only sessions.
  Embeddings deferred — heaviest dependency, smallest
  marginal value before we have real notes.
- 2026-05-21 · **Transcript-only conversation storage.**
  Audio file stays where the user put it; skill stores the
  transcript only. Avoids vault bloat and a second
  storage-shape question (where would audio live, how is it
  synced).
- 2026-05-21 · **Capture-only volunteering.** No "want me to
  capture that?" prompts. User drives every capture. Avoids
  noise during normal sessions; revisit if explicit verb
  proves too high-friction.
- 2026-05-21 · **`$HOME/notes/` over in-repo storage.**
  Personal notes don't belong in a tool-agnostic agentic
  resource repo. User syncs the vault their own way.
