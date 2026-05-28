---
name: notes
description: Capture reminders, insights, 1:1 notes, and conversation transcripts into an Obsidian-shaped vault. Single verb "Add note [in <vault>] for X" classifies the kind, writes the file, and links topics. Disconnected-capture buffer for Obsidian-direct sessions; "merge buffer" routes its entries through the same classifier. Successful captures and buffer merges auto-commit (and push when a remote is configured) when the vault is a git repo.
version: 1.3.0
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
- The user says **"merge buffer"** or **"merge buffer in
  &lt;vault&gt;"** to drain disconnected captures from
  `scratch/buffer.md` into the vault.
- The user says **"show me my buffer"** or **"show me my
  buffer in &lt;vault&gt;"** to print the buffer's current
  contents.

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

Named-vault env vars are the recommended setup for
multi-machine use — the vault on disk is the cross-machine
substrate; the skill is stateless.

Layout (create missing directories on first capture; never
delete or rename existing ones):

```
<vault>/
├── inbox/                 unsorted captures (rare)
├── reminders/<slug>.md    to-dos; dated sections inside
├── insights/<slug>.md     thoughts + topic links
├── people/<person>.md     one file per person; append
├── conversations/<slug>.md  transcripts + pre-amble
├── topics/<topic>.md      first-class topic pages
└── scratch/               disconnected-capture buffer
    ├── buffer.md          active buffer (append in Obsidian)
    └── buffer-<DATE>.md   archive of a merged buffer
```

**Filenames are slug-only.** Dates live inside the file —
in the frontmatter `date:` field for one-shot kinds
(insights, conversations) and as `## YYYY-MM-DD` section
headers for accumulating kinds (reminders, people).

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

All kinds share `kind` (and `links` where present). One-shot
kinds (insights, conversations) carry top-level `date:`.
Accumulating kinds (reminders, people) track date in
`## YYYY-MM-DD` section headers and have no top-level `date:`.

```markdown
# reminders/inbox.md  (or reminders/<topic-area>.md when scoped)
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

```markdown
# insights/<slug>.md
---
date: 2026-05-22
kind: insight
topics: [pretty-printing, wadler]
links: [[pretty-printing]], [[wadler]]
---

<one-shot insight body>.
```

```markdown
# people/<person>.md  (append a dated section per encounter)
---
kind: person
---

# <person>

## 2026-05-22 1:1
links: [[topic-A]], [[topic-B]]

- <bullet>
- <bullet>
```

```markdown
# conversations/<slug>.md
---
date: 2026-05-22
kind: conversation
source: notes-from:beth-ramirez   # see "source: is optional"
tags: [#search-friction]
links: [[search]]
---

## Pre-amble

<one paragraph: who, what, goal of capture>.

## Notes
…
```

For audio-sourced conversations, `source: audio:<path>` and
`## Notes` becomes `## Transcript`.

`source:` on a conversation is **optional**. Three shapes:

- `audio:<path>` — triggers transcription (see below).
- `notes-from:<who>` — hand-captured notes / verbatim quotes.
- omit — pure-prose capture with no discernible source.

Reminders with a hard `due:` capture it inline:
`- <body> — due 2026-06-01`. Section headers track
**capture date**, not due date.

If `people/<person>.md` already exists, append the dated
section. Do not rewrite the file head.

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

  Never overwrite an existing topic stub.

- **`#tag`** — loose categorization in `tags:` frontmatter.

Insights and 1:1s carry `topics:` (drives Obsidian's graph
view). Conversations carry `tags:`. Reminders may carry both.

## Conversation transcripts

When the user passes `audio: <path>`:

1. Resolve `<path>` (expand `~`).
2. Probe `$PATH` for a transcription tool, in this order:
   `whisper` (OpenAI whisper CLI), then `whisper-cpp`. If
   neither is found, stop and tell the user "no transcription
   tool available; pass a transcript instead". Do **not**
   write a partial conversation file.
3. Run `<tool> "<path>" --output_format txt --output_dir "<tmpdir>"`.
4. Read the produced `.txt` under `## Transcript`, delete the
   tmpdir, and record `source: audio:<path>` in frontmatter.
   Never copy or move the audio file into the vault — the
   vault stores text only.

## Retrieval

Three queries. Plugin-first ladder: emit the Obsidian plugin
query as a code block the user can paste, and offer the
ripgrep / shell fallback inline. The vault selector
(`in <name|path>`) works on retrieval verbs too — resolve
the same way as for capture.

### "Find notes related to X"

**Smart Connections (Obsidian):**

```
"X" OR [[X]] OR #X
```

**Ripgrep fallback:**

```bash
rg -l --no-heading -- "X|\[\[X\]\]|#X" "<vault>"
```

### "List notes by topic X"

**Dataview (Obsidian):**

````
```dataview
table date, kind from ""
where contains(file.outlinks, [[X]]) or contains(topics, "X")
sort date desc
```
````

**Ripgrep fallback** over `[[X]]` and frontmatter `topics:`.

### "List recent &lt;kind&gt;"

```bash
find "<vault>/<kind>" -type f -mtime -<N> -printf '%T@ %p\n' \
  | sort -rn | head -n 10 | cut -d' ' -f2-
```

Default `<N>` = 14 days unless the user specifies.

For accumulating kinds (reminders, people), files contain
many dated sections — show their last few `## YYYY-MM-DD`
sections, not just file mtimes.

## Disconnected capture

The skill is only reachable inside an agent session. The
`scratch/buffer.md` file gives the user an inline capture
path while in Obsidian directly.

### `scratch/buffer.md` shape

Loose markdown, optional `## YYYY-MM-DD` sections, optional
explicit kind prefixes (`insight: …`, `1:1 with Alice: …`,
`conversation: …`, `reminder: …`) the existing classifier
already understands. No special schema.

When the buffer is initialized, it carries a short header
explaining how to use it. Below the header, the user appends
entries freely.

### "Merge buffer in &lt;vault&gt;"

Read `<vault>/scratch/buffer.md`, walk top-level entries
(each `## …` section, or each blank-line-separated block if
no headers), and **for each entry, run the existing "add note
for X" classifier and write path** — same rules as a
single-prompt capture, applied in a loop. Same disambiguation
budget (one question max; ambiguous-after-one → `inbox/` with
`kind: unsorted`).

After all entries are routed:

1. **Archive** the original buffer body to
   `<vault>/scratch/buffer-<YYYY-MM-DD>.md` (a fresh dated
   copy each merge). If today's archive exists, suffix `-2`,
   `-3` per the slug-collision rule.
2. **Reset** `<vault>/scratch/buffer.md` to its header-only
   state.
3. **Print a merge report** listing every entry and where it
   landed, and write the report to
   `<vault>/inbox/merge-report-<YYYY-MM-DD>.md` for the audit
   trail.

The user manages the dated archives — the skill never
deletes them.

Empty-body buffer → "buffer empty" and exit. Buffer file
missing → say so and stop; the skill does not auto-create
and merge an empty buffer.

### "Show me my buffer in &lt;vault&gt;"

Read and print `<vault>/scratch/buffer.md`. Convenience for
review before merging.

### Bare forms

`merge buffer` and `show me my buffer` (no `in <…>`) use the
default vault chain (`$NOTES_VAULT` → `$HOME/notes`).

## Vault auto-commit

When the vault directory contains a `.git` subdirectory, the
skill commits its writes after every **successful** verb,
and pushes when a remote is configured. The vault is a
personal store, so commits are minimal and path-shaped — no
Conventional Commits machinery, no hand-written subjects.

### Successful verbs commit; aborted and read-only verbs do not

Commit AFTER every intended write for the verb has landed:

- `add note for X` — destination file plus every
  auto-created `topics/<topic>.md` stub.
- `merge buffer` — every classified destination plus every
  topic stub plus the archived `scratch/buffer-<DATE>.md`
  plus the reset `scratch/buffer.md` plus the
  `inbox/merge-report-<DATE>.md`.

Do **not** commit on:

- A capture stopped at the disambiguation question.
- An audio conversation where `whisper` / `whisper-cpp` is
  missing (the skill writes nothing on this path).
- A buffer merge that errors partway through. Surface the
  error and leave the partial state on disk uncommitted; the
  user retries or cleans up.
- Read-only verbs: `show me my buffer`, retrieval verbs.
  These write nothing, therefore commit nothing.

### Staging is path-scoped

Track every path the verb wrote or modified and pass them
explicitly to `git add`. Never `git add -A`, never
`git add <vault>`. A user editing
`insights/foo.md` in Obsidian while the skill writes
`insights/bar.md` must not see their dirty `foo.md` swept
into the skill's commit.

The full side-effect set must be staged — for a capture that
mentions two new topics, that's the destination + both new
topic stubs. For a buffer merge, every line in the merge
report's "what landed where" list, plus the archive, the
reset buffer, and the report itself.

### Commit message — minimal, path-shaped

The vault `git log` reads as a list of files that landed.
Kind is implicit in the directory; no `add` verb (true of
every commit), no body to truncate.

| Verb              | Commit message                            |
| ----------------- | ----------------------------------------- |
| add insight       | `insights/<slug>.md`                      |
| add conversation  | `conversations/<slug>.md`                 |
| add reminder      | `reminders/<file>.md` (usually `inbox.md`) |
| add 1:1           | `people/<person>.md`                      |
| merge buffer      | `merge buffer (<N>)`                      |

Buffer merge keeps the count form because it touches many
paths; a single representative path would mislead.

### Push when remote exists

After the commit lands, run `git remote` in the vault. If
output is non-empty, run `git push` against the upstream of
the current branch with `GIT_TERMINAL_PROMPT=0` so missing
credential helpers fail fast instead of hanging on a TTY
prompt the agent can't satisfy.

If push fails for any reason (no upstream, network, auth,
prompt-disabled), surface a one-line warning —
`vault push failed: <first line of stderr>` — and return the
verb success. The commit is local; the user can retry by
hand. **Do not roll back the commit on push failure.**

If `git remote` is empty, silent skip — no warning.

### Non-git vault → silent skip

`.git` directory absent → write the file as today and
return. No commit, no warning, no offer to `git init`.
iCloud-only / Dropbox-only / no-sync vaults work unchanged.

### Detached HEAD / mid-rebase / mid-merge

If `git symbolic-ref HEAD` errors (detached HEAD, fresh repo
with no commits yet), or `<vault>/.git/rebase-merge` /
`rebase-apply` / `MERGE_HEAD` exist, surface
`vault commit skipped: <reason>` and return. The file write
stands; the user resolves the git state and commits by hand.

## Behaviour rules

- **Never overwrite** an existing file. For accumulating
  kinds (reminders, people), append a dated section. For
  one-shot kinds (insights, conversations), slug collision
  means find a suffix (`-2`, `-3`).
- **Never copy audio** into the vault. Transcript only.
- **Never volunteer captures.** Only act on explicit verbs.
- **Honour the vault selector.** Inline path > named vault
  env var > `$NOTES_VAULT` > `$HOME/notes`. A named vault
  whose env var is unset is an error, not a fall-through.
- **One disambiguation question max** before writing. Still
  unclear → `inbox/` with `kind: unsorted` and tell the user
  where it landed.
- **Never `git add -A`** in the vault. Stage only the paths
  the verb wrote or modified.
- **Commit on success only.** Aborted captures, missing
  transcription tools, partial-merge errors, and read-only
  verbs commit nothing.
- **Push failure is a warning, not an error.** The commit
  stays; the verb succeeds.
