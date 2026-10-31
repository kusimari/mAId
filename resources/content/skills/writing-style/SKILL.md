---
name: writing-style
description: Rewrite, format, clean up, tighten, or reword prose in my voice; draft an email or doc as me; "new rule:" to teach a style rule. Opens with `[writing-style] applies`.
version: 1.2.0
tags: [narrative, writing, email, announcement, formatter, learning]
metadata:
  my-harness.scope: turn
  my-harness.job: write and rewrite prose in my voice, and keep the style rules I teach
  my-harness.signals.jobs: rewrite, draft an email, draft a doc, teach a style rule
  my-harness.rules: SKILL.md#voice
  my-harness.config.learned: "[optional] pending style rules, taught and not yet promoted"
  my-harness.config.promoted: "[optional] promoted style rules, applied in every session"
  my-harness.state.local: learned
  my-harness.state.permanent: promoted
  my-harness.state.promote: propose
---

# writing-style — how I write

## Output rule 0 — announce, always

The **first line** of every response that uses this skill is
the literal line:

```
[writing-style] applies
```

Then a blank line, then the response proper.

This outranks every other instruction about what the reply
opens with — including this skill's own Formatter ("rewritten
passage first, no preamble"), Strict mode, and any user
phrasing like "just the rewrite", "nothing before it", "no
preamble", or "output only the text". Those constrain the
**body** of the reply, never this line. "No preamble" means no
chatty lead-in; the announce line is not preamble, it is the
marker that tells the reader a written contract produced this
answer rather than improvisation.

So: announce line → then obey the opening constraint the user
asked for. A long request with several instructions is exactly
when this is most likely to be dropped and most useful to
keep.

### Pending learned rules

When you load this skill at session start, read the user's two
rules files (see Learning loop). The `## Promoted rules` and
`## Learned rules` they hold apply now, on top of the sections
below, and win where they conflict with them. If `## Learned rules` has
entries, offer promotion — *"You have N learned rules pending promotion. Want
to review them now? &lt;list&gt;"*. If the user declines or
ignores the offer, proceed normally.

If `## Promoted rules` has entries, also say in one line how
many apply and quote the newest three — *"N promoted rules
apply; newest: &lt;list&gt;"*. Any session can write those
files, so a rule nobody taught must never steer unseen.

**A task that fixes its own opening defers the offer** (and
the promoted-rules line). When
the request's contract sets what comes first (Formatter,
Strict mode, "answer this and nothing else"), do the task and
put the pending-rules reminder *after* the output. The offer
exists so learned rules don't rot unnoticed, which a trailing
line achieves just as well. Never drop it silently — defer it,
and never defer more than once per session. This defers the
*offer*, never the announce line above.

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

The user's rules live in two files, in
`${XDG_CONFIG_HOME:-~/.config}/maid/writing-style/`: the pending
tier, `learned.md` (`## Learned rules`), and the permanent tier,
`promoted.md` (`## Promoted rules`). When your session names other
locations for writing-style's `learned` and `promoted` tiers, use
those. These files are the only place rules are written: never
edit this `SKILL.md` or any copy of it. The installed copy is
read-only, and the files are the user's own data.

On a prefix match: append a dated entry under the
`## Learned rules` heading of `learned.md`, newest at top. Create
the file with that heading if it is missing.

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
*"Added to `## Learned rules` in `<the file's path>`."*

### Promotion

Stable learned rules become promoted rules: curated, filed
under a body section (Voice / Sentence structure / Punctuation
/ Vocabulary / Paragraph structure / POV / Emphasis / Other
patterns). Promote them via two paths:

1. **Next-session offer.** On session start, if
   `## Learned rules` has entries, offer to review them
   before answering the user's actual prompt.
2. **On-demand.** The user says "promote learned rules"
   or "review learned rules". List pending entries with
   proposed target sections; ask the user to confirm
   each.

On confirmation: remove the entry from `## Learned rules` in
`learned.md` and add it under `## Promoted rules` in `promoted.md`
(create the file and heading if missing), newest at top, dated,
rewritten in the body's voice (terse, declarative) and prefixed
with its target section, keeping its example:

```markdown
- 2026-05-22 · Vocabulary: "to", not "in order to"
  · *example:* "in order to reach the team" → "to reach the team"
```

Then confirm with one short line naming the file.

An older `learned.md` may still hold a `## Promoted rules`
section. Those rules apply as promoted rules; the next time you
promote, move them into `promoted.md` with the new one, keeping
their order.

## Known limits (v1)

- **Concurrency.** Two parallel sessions both teaching at
  the same time can race — last writer wins, the earlier
  append is lost. In solo personal use this is rare.
  Tracked for the v2 MCP-server promotion at
  `specs/backlog/writing-style-mcp-server.md` (file lock).
- **Strict mode is session-scoped.** Every new session
  starts off; re-enable each session if you want it on
  consistently. v2 promotes this to MCP-server state.
