---
name: notes
description: Capture reminders, insights, 1:1 notes, and conversation transcripts into an Obsidian-shaped vault. Single verb "Add note [in <vault>] for X" classifies the kind, writes the file, and links topics.
version: 1.1.0
tags: [notes, knowledge, obsidian, capture]
---

# notes — capture into a personal knowledge vault

You begin every response that uses this skill with the literal
line `[notes] applies` on its own line.

## When to apply

- The user says some variant of **"Add note for &lt;X&gt;"**
  or **"Add note in &lt;vault&gt; for &lt;X&gt;"** (also:
  "save this", "remember this", "capture this", "note to
  self", "log this 1:1").
- The user asks to **find / list** notes ("find notes related
  to X", "list notes in &lt;vault&gt; by topic Y", "what
  reminders do I have").

Capture is **always user-driven**. Never volunteer "want me
to capture that?" mid-session — the user invokes this skill
explicitly.

## The vault

The skill writes into one of three vault sources, resolved
at invocation time:

1. **Inline path** — `add note in <path> for <X>` where
   `<path>` starts with `/`, `~`, or `./`. Use the directory
   directly. If it doesn't exist, ask before creating.
2. **Named vault** — `add note in <name> for <X>` resolves
   to the env var `$NOTES_VAULT_<NAME_UPPER_SNAKE>`. Examples:
   `add note in work for X` → `$NOTES_VAULT_WORK`;
   `add note in gorantls-store for X` → `$NOTES_VAULT_GORANTLS_STORE`.
   If the env var is unset, stop and tell the user
   *"no vault configured for &lt;name&gt; (set
   $NOTES_VAULT_&lt;NAME&gt;)"*. Don't fall through to default.
3. **Default** — `add note for <X>` (no `in <…>` qualifier)
   uses `$NOTES_VAULT` if set, else `$HOME/notes`.

The named-vault env vars are the **recommended** setup for
multi-machine use — point each at a directory inside an
iCloud / Dropbox / git-synced folder so captures from any
host land in the same place. The skill itself is stateless;
the vault on disk is the cross-machine substrate.

Layout (create missing directories on first capture; never
delete or rename existing ones):

```
<vault>/
├── inbox/                              unsorted captures (rare)
├── reminders/<slug>.md                 to-dos; dated sections inside
├── insights/<slug>.md                  thoughts + topic links
├── people/<person>.md                  one file per person; append
├── conversations/<slug>.md             transcripts + pre-amble
└── topics/<topic>.md                   first-class topic pages
```

**Filenames are slug-only.** Dates live inside the file —
in the frontmatter `date:` field for one-shot kinds
(insights, conversations) and as `## YYYY-MM-DD` section
headers for accumulating kinds (reminders, people). This
keeps related entries co-located and lets the user open
e.g. `reminders/inbox.md` to see the whole rolling list,
not a directory of dated single-line files.

If the user's existing layout differs (e.g. `todo/` instead
of `reminders/`), **ask** before creating new directories —
do not silently create a parallel structure.

## Classification

Read the text after "Add note for". Route by these rules,
in order:

1. **Explicit kind prefix wins.** `insight: …`,
   `conversation: …`, `1:1 with <person>: …`,
   `reminder: …` → that kind.
2. **Reminder shape.** Starts with `remind me to`,
   `remember to`, contains a date/time hint
   (`Tuesday`, `next week`, `by Friday`), or is phrased as
   a TODO → **reminder**.
3. **1:1 / interview shape.** Names a person and frames the
   note around them ("from my 1:1 with Alice", "interviewing
   Bob about the Q3 plan") → **1:1**.
4. **Conversation shape.** Carries `audio:` / `transcript:`
   keys, or pastes a transcript / audio path → **conversation**.
5. **Default → insight,** with a frontmatter line
   `classification: implicit` so the user can find these
   later if the default was wrong.

When the shape is ambiguous (could plausibly be two kinds),
ask **one** disambiguation question before writing.

## Frontmatter templates

All kinds share `kind / links`. Insights and conversations
add `date`. Reminders accumulate dated sections inside a
single rolling file (so the file itself has no top-level
`date`).

### Reminder (append a dated section to a rolling file)

The default rolling file is `reminders/inbox.md`. Use a
topic-area filename (`reminders/q3-launch.md`) when the
user's reminder is clearly scoped.

```markdown
---
kind: reminder
---

# Reminders

## 2026-05-22
- file taxes by Friday (links: [[taxes]])
- send the migration log to myself

## 2026-05-26
- review the Q3 doc
```

If a reminder has a hard `due:` date, capture it inline:
`- <body> — due 2026-06-01`. Section headers track
**capture date** (when added), not due date.

### Insight (one file per insight)

```markdown
---
date: 2026-05-22
kind: insight
topics: [pretty-printing, wadler]
links: [[pretty-printing]], [[wadler]]
---

Wadler's prettier algorithm needs immutable doc trees because
the `<>` combinator must be associative for the optimal-layout
reduction to terminate without quadratic blowup.
```

### 1:1 / interview (append to per-person file)

```markdown
---
kind: person
---

# <person>

## 2026-05-22 1:1
links: [[topic-A]], [[topic-B]]

- <bullet>
- <bullet>
```

If `people/<person>.md` already exists, append the dated
section. Do not rewrite the file head.

### Conversation

`source:` is **optional**. When present, one of:

- `audio:<path>` — triggers transcription (see below).
- `notes-from:<who>` — hand-captured notes / verbatim quotes
  the user typed up. No transcription needed.

Omit `source:` entirely for pure-prose captures with no
discernible source.

```markdown
---
date: 2026-05-22
kind: conversation
source: notes-from:beth-ramirez
tags: [#search-friction, #seller-voice]
links: [[search]], [[discovery]]
---

## Pre-amble

<one paragraph: who was in the conversation, what it was
about, and the goal of the capture>.

## Notes

…
```

For audio-sourced conversations, replace `## Notes` with
`## Transcript`.

## Linking

- **`[[topic]]`** — first-class topic pages. When a
  `[[topic]]` reference is written and `topics/<topic>.md`
  doesn't exist, create a one-line stub:

  ```markdown
  ---
  kind: topic
  ---

  # <topic>
  ```

  Never overwrite an existing topic stub. The user grows
  topic pages over time.

- **`#tag`** — loose categorization in `tags:` frontmatter.
  Use for cross-cutting labels (e.g. `#design-review`,
  `#urgent`) that aren't worth a topic page.

Insights and 1:1s carry `topics:` (drives the graph view in
Obsidian). Conversations carry `tags:`. Reminders may carry
both if they reference an in-flight topic.

## Conversation transcripts

When the user passes `audio: <path>`:

1. Resolve `<path>` (expand `~`).
2. Probe for a transcription tool, in this order:
   - `whisper` (OpenAI whisper CLI)
   - `whisper-cpp`

   If neither is on `$PATH`, stop and tell the user:
   "no transcription tool available; pass a transcript
   instead". Do **not** write a partial conversation file.
3. Run the tool — typical invocation:

   ```bash
   whisper "<path>" --output_format txt --output_dir "<tmpdir>"
   ```

4. Read the produced `.txt` and place it under
   `## Transcript`.
5. Delete the temp directory.
6. Record the audio path under `source: audio:<path>` in
   frontmatter — but **never copy or move the audio file
   into the vault**. The vault stores text only.

## Retrieval

Three queries, plugin-first ladder. The vault selector
(`in <name|path>`) works on retrieval verbs too — resolve
it the same way as for capture, then run the query against
the resolved vault directory (`<vault>` below).

### "Find notes related to X"

1. **Primary: Smart Connections (Obsidian plugin).** Emit a
   code block the user can paste into Obsidian's search:

   ```
   "X" OR [[X]] OR #X
   ```

   Tell the user: *"Paste this into Obsidian's Smart
   Connections search; if you'd rather a terminal answer,
   say 'use ripgrep'."*
2. **Fallback: ripgrep.** When the user picks ripgrep:

   ```bash
   rg -l --no-heading -- "X|\[\[X\]\]|#X" "<vault>"
   ```

   Return file paths plus a short line of context per hit.

### "List notes by topic X"

1. **Primary: Dataview.** Emit:

   ````
   ```dataview
   table date, kind from ""
   where contains(file.outlinks, [[X]]) or contains(topics, "X")
   sort date desc
   ```
   ````

2. **Fallback: ripgrep** over `[[X]]` and frontmatter
   `topics:` containing the term.

### "List recent &lt;kind&gt;"

Use shell — fastest:

```bash
find "<vault>/<kind>" -type f -mtime -<N> -printf '%T@ %p\n' \
  | sort -rn | head -n 10 | cut -d' ' -f2-
```

Default `<N>` = 14 days unless the user specifies.

For accumulating kinds (reminders, people), files contain
many dated sections. "List recent reminders" means: open the
top N rolling files modified in the window, then show their
last few `## YYYY-MM-DD` sections — not just the file
mtimes.

## Behaviour rules

- **Never overwrite** an existing file (insight, conversation,
  topic stub). Append dated sections to accumulating files
  (reminders, people). For one-shot kinds (insights,
  conversations), slug collision means find a suffix
  (`-2`, `-3`).
- **Never copy audio** into the vault. Transcript only.
- **Never volunteer captures.** Only act on explicit verbs.
- **Honour the vault selector.** Inline path > named vault
  env var > `$NOTES_VAULT` > `$HOME/notes`. A named vault
  whose env var is unset is an error, not a fall-through.
- **One disambiguation question max** before writing. If
  still unclear, default to `inbox/` with
  `kind: unsorted` and tell the user where it landed.

## Known limits (v1)

- No semantic search; Dataview / ripgrep only.
- No watch-mode; the skill doesn't surface "this reminds
  me of a note from two weeks ago".
- Transcription depends on `whisper` / `whisper-cpp` being
  on `$PATH`.

These are tracked in `specs/backlog/notes-mcp-server.md` for
the v2 promotion to a typed MCP server.
