# Backlog: notes — stop auto-creating empty topic stubs

## What

Change the `notes` skill's **Linking** rule so a `[[topic]]`
reference to a nonexistent `topics/<topic>.md` does **not**
auto-create a one-line stub. Leave the wikilink dangling;
create `topics/<topic>.md` only when there is real substance
to put in it.

Current rule (`resources/content/skills/notes/SKILL.md`,
under `## Linking`):

> When a `[[topic]]` reference is written and
> `topics/<topic>.md` doesn't exist, create a one-line stub
> (`kind: topic` + `# <topic>`).

Proposed rule:

> When a `[[topic]]` reference is written and
> `topics/<topic>.md` doesn't exist, **do not create a
> stub.** Leave the link dangling — Obsidian renders
> unresolved links fine — and write `topics/<topic>.md` only
> when it accrues real substance (a definition, links,
> accumulated notes). Never overwrite an existing topic page.

## Why

- **Empty stubs are noise.** Every new `[[topic]]` mint
  produces a substance-less `kind: topic` + `# <topic>` file.
  The `topics/` dir already holds ~180 such stubs, most of
  them 26–50 bytes. They inflate the vault, clutter search,
  and pad the Obsidian graph with nodes that carry no
  information.
- **Obsidian handles dangling links natively.** Unresolved
  `[[wikilinks]]` render as clickable placeholders and still
  show in the graph as unresolved nodes. The stub buys
  nothing Obsidian doesn't already give for free.
- **Substance-gated creation matches intent.** A topic page
  should exist because someone had something to say about the
  topic — not as a side effect of referencing it once.
- Observed live: an `[[opinion-intelligence]]` reference in
  `conversations/comms-strategy-jam.md` auto-minted an empty
  stub that had to be manually removed.

## Open questions

- **Migration of existing stubs.** ~180 empty stubs already
  exist in the Gorantls-store vault. One-shot cleanup
  (delete files that are frontmatter + `# <title>` only), or
  leave them and only change go-forward behavior? Deleting
  turns their inbound links dangling — which is the desired
  end state, but it's a bulk vault mutation worth doing
  deliberately.
- **Definition of "substance."** Frontmatter + heading + one
  sentence? A link? Needs a bright line so the skill doesn't
  re-litigate on every write.
- **Retrieval impact.** The Dataview/ripgrep retrieval
  queries assume `topics/<topic>.md` may exist. Dangling
  links won't appear as files — confirm the "list notes by
  topic X" query still works off `[[X]]` outlinks + frontmatter
  `topics:` without needing the topic file present. (It should,
  since it keys on outlinks, not file existence.)

## Trigger to promote

- The empty-stub clutter in `topics/` becomes a real drag on
  search or graph navigation.
- A vault cleanup pass is scheduled anyway (bundle the rule
  change + one-shot stub deletion together).
- The user re-flags manually deleting an auto-minted stub
  (already happened once).

## Note on editing the skill

`resources/content/skills/notes/SKILL.md` is the source
behind the managed skills symlink — edit it here in the repo,
not under `~/.claude/skills/notes/`. Changes land in the next
session.
