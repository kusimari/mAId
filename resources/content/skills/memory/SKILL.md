---
name: memory
description: 'Remember, recall or list what I told you across sessions and agents ("remember this", "remember this in <store>", "what do you remember about", "list my memories"). Reads and writes my memory stores. Opens with `[memory] applies`.'
version: 1.0.0
tags: [memory, recall, stores]
---

# memory - my memory stores

## Rule 0 - announce

The first line of every reply that uses this skill is the literal line
`[memory] applies`, then a blank line. This outranks any instruction about
how the reply opens.

## Rule 1 - where a write may go (outranks everything below)

1. Write only into a **configured** store whose directory **exists**. Never
   create a store, never write to a path the user gives that is not a
   configured store, never fall back to another store. If the store is
   unknown, gone, or there is no default, stop and say which.
2. Without a store named **in this request**, a write goes to the
   **default** store, and only there.
3. A non-default store gets only what the user explicitly sends to it
   ("remember this in work"). Never choose a store from the topic ("this
   sounds like work").

4. A write sent to a named store, or refused, goes nowhere else either: in
   Claude Code, do not also save it to Claude's own automatic memory, which
   is the default store.

Reads follow the same choice: the default store, unless this request names
another.

## The config

Stores are listed in JSON files, one per installed part, in
`${XDG_CONFIG_HOME:-$HOME/.config}/my-harness/memory/*.json` (skip
dotfiles). Read every file:

```json
{ "format": 1, "part": "maid-harness",
  "default": "personal",
  "stores": { "personal": "/home/me/stores/personal" } }
```

- The configured stores are the union of every file's `stores`.
- A store name that appears in more than one file is unusable: say so.
- The default is the one `default` across all files. Two files setting
  one means there is no default: say so.
- A file whose `format` is not 1 is ignored: say so.
- The user adds stores with `just memory-store <name> <path> [--default]`
  in their mAId checkout. Tell them that when none is configured; do not
  edit these files yourself.

A store is a directory; its memories are in `<store>/memory/`. Create
`memory/` if the store exists and it is missing.

## The format

Claude Code's own auto-memory format, so Claude can read the same files.

One file per memory, `<store>/memory/<type>_<slug>.md`:

```markdown
---
name: <short title>
description: <one line: what this memory is, used to judge relevance>
metadata:
  type: <user | feedback | project | reference>
---

<the memory. For feedback and project: the rule or fact, then
**Why:** and **How to apply:** lines.>
```

- `user`: who the user is, their role and preferences.
- `feedback`: how the user wants work done, corrections and confirmations.
- `project`: ongoing work, decisions and dates not derivable from code.
- `reference`: where to find things outside the project.

The index `<store>/memory/MEMORY.md` has one line per memory, and nothing
else is required:

```markdown
- [<name>](<file>.md) - <a short hook>
```

Keep each index line short. One index serves every project and agent, and
Claude loads only its first 200 lines.

## Remember (write)

1. Pick the store (rule 1). Stop if it is not usable.
2. Read `MEMORY.md`. If a memory on the same subject exists, update that
   file (and its index line) instead of adding another.
3. Otherwise write a new file and append its line to `MEMORY.md`. Do not
   overwrite a different memory that already has the file name: pick
   another slug.
4. Save only what was asked, or what the user clearly stated as lasting.
   Never save secrets or credentials.
5. Reply with the store name and the file written.

In Claude Code, when this session's automatic memory directory already is
the default store's `memory/` dir, Claude's own memory writes there: leave
writes to the default store to it, and use this skill for a named store.

## Recall

1. Pick the store (rule 1).
2. Read `MEMORY.md`, then open the memories whose name or hook fits the
   question.
3. Answer from them, naming the store. If nothing fits, say so; do not
   guess.

## List

Print the store's `MEMORY.md` index (the default store, or the one named).
"List my stores" prints the configured stores, marking the default and any
that are missing.

## What this skill does not do

No git. The store is the user's checkout; pulling, committing and pushing
it is theirs.
