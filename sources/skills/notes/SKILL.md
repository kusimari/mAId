---
name: notes
description: Capture reminders, insights, 1:1 notes, and conversation transcripts into an Obsidian-shaped vault. Single verb "Add note for X" classifies the kind, writes the file, and links topics.
version: 1.0.0
tags: [notes, knowledge, obsidian, capture]
---

# notes — capture into a personal knowledge vault

You begin every response that uses this skill with the literal
line `[notes] applies` on its own line.

## When to apply

- The user says some variant of **"Add note for &lt;X&gt;"**
  (also: "save this", "remember this", "capture this", "note
  to self", "log this 1:1").
- The user asks to **find / list** notes ("find notes related
  to X", "list notes by topic Y", "what reminders do I have").

Capture is **always user-driven**. Never volunteer "want me
to capture that?" mid-session — the user invokes this skill
explicitly.

## The vault

Default: `$HOME/notes`. Override with the `$NOTES_VAULT`
env var.

The override is the **recommended** setup for any user
who wants the vault on more than one machine — point
`$NOTES_VAULT` at a directory inside an iCloud / Dropbox
/ git-synced folder so captures from any host land in the
same place. The skill itself is stateless; the vault on
disk is the cross-machine substrate.

Layout (create missing directories on first capture; never
delete or rename existing ones):

```
$NOTES_VAULT/
├── inbox/                              unsorted captures (rare)
├── reminders/<YYYY-MM-DD>-<slug>.md    dated to-dos
├── insights/<YYYY-MM-DD>-<slug>.md     thoughts + topic links
├── people/<person>.md                  one file per person; append
├── conversations/<YYYY-MM-DD>-<slug>.md transcripts + pre-amble
└── topics/<topic>.md                   first-class topic pages
```

If the user's existing layout differs (e.g. `todo/` instead
of `reminders/`), **ask** before creating new directories —
do not silently create a parallel structure.

### Named vaults (future)

A user may keep more than one vault — e.g., a personal
vault and a work vault — separated for sync, sharing, or
content-class reasons. The future shape:

- Each named vault is pointed to by a per-name env var:
  `$NOTES_VAULT_<NAME>` (uppercase). Example:
  `NOTES_VAULT_WORK=/path/to/work-vault`.
- `add note for <name>: <X>` → write into
  `$NOTES_VAULT_<NAME>` if set; error otherwise.
- `add note for: <X>` (no qualifier) → default to
  `$NOTES_VAULT`, then `$HOME/notes`.

Until a named vault is configured, `add note for
<name>: …` produces a "no vault configured for
&lt;name&gt;" message and stops without writing.

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

All kinds share `date / kind / links`. Add fields per kind.

### Reminder

```markdown
---
date: 2026-05-22
kind: reminder
due: 2026-05-26
links: [[topic-A]]
---

<one-line reminder body>.
```

### Insight

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

```markdown
---
date: 2026-05-22
kind: conversation
source: audio:~/Downloads/<recording>.m4a
tags: [#tag-A, #tag-B]
links: [[topic-A]]
---

## Pre-amble

<one paragraph: who was in the conversation, what it was
about, and the goal of the capture>.

## Transcript

…
```

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

Three queries, plugin-first ladder.

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
   rg -l --no-heading -- "X|\[\[X\]\]|#X" "$NOTES_VAULT"
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
find "$NOTES_VAULT/<kind>" -type f -mtime -<N> -printf '%T@ %p\n' \
  | sort -rn | head -n 10 | cut -d' ' -f2-
```

Default `<N>` = 14 days unless the user specifies.

## Behaviour rules

- **Never overwrite** an existing file (insight, reminder,
  topic stub). Append to per-person files; for everything
  else, slug-collision means find a suffix (`-2`, `-3`).
- **Never copy audio** into the vault. Transcript only.
- **Never volunteer captures.** Only act on explicit verbs.
- **Honour `$NOTES_VAULT`** — when set, never fall back to
  `$HOME/notes`.
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
