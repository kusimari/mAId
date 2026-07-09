---
name: writing-style
description: Voice, tone, and structure for my prose. Includes a formatter verb ("format this in my style"), an opt-in strict mode that flags style violations before sending, and an explicit-teach loop that captures new rules into the curated source.
version: 1.1.0
tags: [narrative, writing, email, announcement, formatter, learning]
---

# writing-style — how I write

You begin every response that uses this skill with the
literal line `[writing-style] applies` on its own line.

When you load this skill at session start, **first** check
the `## Learned rules` section below. If it has any
entries, offer promotion before answering the user's actual
prompt: *"You have N learned rules pending promotion. Want
to review them now? &lt;list&gt;"*. If the user declines or
ignores the offer, proceed normally.

## Voice

- First person singular. Active voice. Say what you mean, not what sounds clever.
- No hedging adverbs (arguably, perhaps, somewhat). Either it is, or it isn't — if uncertain, say so
  explicitly.
- No superlatives unless they're literally true ("fastest known implementation" is fine if
  benchmarked; "truly amazing" is not).
- Humor lands when it's quiet and earned. No forced jokes, no self-deprecation as a tone-setter.

## Sentence structure

- Short sentences by default. Long ones are a choice, not a habit.
- One idea per sentence. If two ideas share a sentence, there's usually a better structure with two
  sentences.
- Don't end paragraphs on throwaway clauses. The last sentence should carry weight.

## Punctuation

- Em-dashes (—) for interruption and aside. Don't overuse; one or two per paragraph max.
- Semicolons for tight coupling between two complete thoughts. Used sparingly.
- Oxford comma, always.
- No exclamation marks. (None.)

## Vocabulary

- Plain words over long ones. "Use" not "utilize". "Show" not "demonstrate". "Because" not "due to
  the fact that".
- Technical terms are fine — they're precise. Jargon for its own sake is not.
- Avoid business-speak: "circle back", "synergy", "leverage", "at the end of the day".

## Paragraph structure

- Lead with the point. Supporting detail follows.
- A paragraph that doesn't say something new is a paragraph to delete.
- Transitions by idea, not by word ("Additionally" / "Moreover" / "Furthermore" are usually noise).

## POV

- First person for experience ("I noticed…"). Second person for guidance ("you'll want to…") when
  writing how-to. Never the royal "we".

## Emphasis

- Bold for the load-bearing word in a sentence. Italics for book titles, technical terms on first
  use, or tone inflection.
- Never UPPERCASE for emphasis. Never bold an entire sentence.

## Other patterns

- Lists when the items are parallel. Prose when they aren't.
- Code blocks for anything the reader might copy-paste.
- Headings when the piece is >3 paragraphs. Below that, prose.

## Formatter

The user can ask you to rewrite a passage against the rules
above. Trigger phrases:

- "format this in my style"
- "rewrite to my voice"
- "fix this in my style"

Output two things, in order:

1. The rewritten passage (no preamble — start with the
   text).
2. A bulleted **change log**: one bullet per substantive
   edit, citing the section the rule came from. Examples:

   - Vocabulary: "leverage" → "use"
   - Voice: cut hedge "arguably" — say it or don't
   - Sentence structure: split two ideas into two
     sentences
   - Emphasis: removed UPPERCASE for emphasis

Cosmetic edits (whitespace, typo fixes) don't need a
bullet. If the input already conforms, say so explicitly:
*"No changes — this already fits the style guide."*

## Strict mode

The user can toggle a session-scoped guardrail flag.

Triggers:

- **On:** "strict mode on", "strict on", "guardrails on"
- **Off:** "strict mode off", "strict off", "guardrails off"

Default is **off**. The flag resets at the end of the
session — every new session starts off.

When strict mode is on, before sending **any prose draft**
(email, doc, PR description, announcement, reply with a
distinct prose passage), prefix the draft with a bullet
list of style violations found in your own draft, with
span quotes:

```
Style check (strict mode):
- Voice: hedge — "I would arguably say…"
- Vocabulary: business-speak — "leverage"

Want me to apply these fixes before sending? (yes / no / specific ones)
```

Wait for confirmation before sending the draft. If the
user says no, send the draft as-is. If yes, apply all
fixes; if specific, apply only those.

Strict mode does **not** apply to short conversational
replies (one or two sentences answering a question). It
applies to substantive prose — anything you'd consider a
"draft".

## Learning loop

The user can teach you new style rules during a session.

**Strict-prefix triggers only:**

- `new rule: …`
- `add rule: …`
- `style rule: …`

Phrases like "I prefer X" or "stop using Y" are **not**
triggers — they're conversational and would generate
false-positive captures.

On a prefix match: append a dated entry to the
`## Learned rules` section below, newest at top. Edit the
file at `~/.claude/skills/writing-style/SKILL.md` (or the
kiro path if Claude isn't where you're running) — the path
is a symlink into the mAId checkout, so the edit lands in
the version-controlled source.

Entry shape:

```markdown
- 2026-05-22 · prefer "to" over "in order to"
  · *example:* "in order to reach the team" → "to reach the team"
  · source: explicit teach
```

The `*example:*` and `source:` fields are optional but
recommended. Use them when the user's framing makes them
natural.

After writing, confirm with one short line:
*"Added to `## Learned rules`. Run `git status` in the
mAId checkout to see the diff."*

### Promotion

Stable learned rules belong in the curated body above
(Voice / Sentence structure / Punctuation / Vocabulary /
Paragraph structure / POV / Emphasis / Other patterns).
Promote them via two paths:

1. **Next-session offer.** On session start, if
   `## Learned rules` has entries, offer to review them
   before answering the user's actual prompt.
2. **On-demand.** The user says "promote learned rules"
   or "review learned rules". List pending entries with
   proposed target sections; ask the user to confirm
   each.

On confirmation: edit the target section to add the rule
in its existing voice (terse, declarative), then remove
the entry from `## Learned rules`. Both edits land in
the same file write.

## Learned rules

<!-- Newest at top. Entries are explicit teach turns
     (`new rule:` / `add rule:` / `style rule:` prefix).
     At session start or on-demand, the assistant offers
     to promote stable entries into the body sections
     above. -->

- 2026-07-08 · use a spaced hyphen " - " for interruptions and asides, not an em-dash
  · *example:* "Define that role first - what problem this person owns - and then bring in the org footprint"
  · source: explicit teach

## Known limits (v1)

- **Concurrency.** Two parallel sessions both teaching at
  the same time can race — last writer wins, the earlier
  append is lost. In solo personal use this is rare.
  Tracked for the v2 MCP-server promotion at
  `specs/backlog/writing-style-mcp-server.md` (file lock).
- **Strict mode is session-scoped.** Every new session
  starts off; re-enable each session if you want it on
  consistently. v2 promotes this to MCP-server state.

## Session Log

<!-- Newest at top. Skills notice patterns during a session and
     append observations here; promote the useful ones into the
     body above at wrap-up. -->
