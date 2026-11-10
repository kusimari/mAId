---
name: writing-style
description: Rewrite, format, clean up, tighten, or reword prose in my voice; draft an email or doc as me; "new rule:" to teach a style rule. Opens with `[writing-style] applies`.
version: 1.3.1
tags: [narrative, writing, email, announcement, formatter, learning]
metadata:
  my-harness.contract: "1"
  my-harness.scope: turn
  my-harness.job: write and rewrite prose in my voice, and keep the style rules I teach in my persona's memory until they move into this skill
  my-harness.signals.jobs: rewrite, draft an email, draft a doc, teach a style rule
  my-harness.rules: SKILL.md#voice
  my-harness.config.learned: "[optional] an older pending rules file, read until its rules move into this skill"
  my-harness.config.promoted: "[optional] an older promoted rules file, read until its rules move into this skill"
  my-harness.bind.learned: "path:${MAID_CONFIG:-${XDG_CONFIG_HOME:-$HOME/.config}/maid}/writing-style/learned.md"
  my-harness.bind.promoted: "path:${MAID_CONFIG:-${XDG_CONFIG_HOME:-$HOME/.config}/maid}/writing-style/promoted.md"
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

### Your saved rules

When you load this skill, find the user's style rules and apply them on
top of the sections below. They win where they conflict with them.

1. **In memory.** Search the memory of the persona this session acts
   through, with that memory's own tool (`memory_search` for
   `writing-style`). Each memory tagged `bring-in: writing-style` is one
   rule.
2. **Older files.** The files your session's startup report names for
   writing-style, as `locations: learned <path>, promoted <path>`: rules
   under `## Learned rules` and `## Promoted rules`. When it names none,
   there are none: never look for them anywhere else. They are the user's
   data from before: read them, and never write or delete them.

Then say in one line how many rules apply and quote the newest three:
*"N saved rules apply; newest: &lt;list&gt;"*. Any session can save a
rule, so a rule nobody taught must never steer unseen.

**A task that fixes its own opening defers that line.** When the
request's contract sets what comes first (Formatter, Strict mode,
"answer this and nothing else"), do the task and put the line *after*
the output. Never drop it silently, and never defer it more than once
per session. This defers the line, never the announce line above.

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

On a prefix match, save the rule to the persona's memory now, with that
memory server's own tools:

1. **Pick the memory:** the one of the persona this session acts
   through. If there are several and none is acting, ask which, once. If
   the session has no memory tool, say *"No memory here: this rule holds
   for this session only."* and apply it for the rest of the session.
2. **Find its section** (Voice / Sentence structure / Punctuation /
   Vocabulary / Paragraph structure / POV / Emphasis / Other patterns),
   and write the rule terse and declarative, in the body's voice.
3. **No duplicates:** `memory_search` for the rule's key words. If a
   writing-style memory already says it, change that one with
   `memory_str_replace` instead.
4. **Save it:** `memory_create` at `/memories/writing-style-<slug>.md`:

   ```markdown
   ---
   name: "writing-style: to, not in order to"
   description: "Vocabulary: \"to\", not \"in order to\""
   metadata:
     type: feedback
     bring-in: writing-style
   ---

   Vocabulary: "to", not "in order to"
   *example:* "in order to reach the team" → "to reach the team"
   ```

   The `bring-in: writing-style` line is what marks it as one of these
   rules. The example line is optional.
5. **Index it:** view `/memories/MEMORY.md` and `memory_insert` after its
   last line (or `memory_create` it when missing):
   `- [writing-style: <short rule>](writing-style-<slug>.md) - <Section>: <rule>`.

Then confirm with one short line:
*"Saved to memory as `<path>`. At close it is offered to move into this
skill."*

Never write `learned.md` or `promoted.md`, and never edit this
`SKILL.md` or any copy of it. The installed copy is read-only.

### Moving rules into the skill

A rule in memory is the temporary tier; this skill's text is where it
belongs once it holds. When the user closes the session and reflects
("close this session", "reflect"), the reflection lists the rules saved
this session and offers one item: move them, with any still in the older
files, into this skill's text, as a proposal on its source. The user
decides. When it lands, the proposal names the memories to remove, and
removing them is the user's.

On "promote learned rules", "review learned rules" or "move my rules
into the skill": list the saved rules, and say the move is offered at the
next reflection, which "reflect" starts. Do not edit anything yourself.

## Known limits (v1)

- **Concurrency.** Each rule is its own memory file, so two
  sessions teaching at once keep both rules; their index lines can
  still race, and the later write wins.
- **Strict mode is session-scoped.** Every new session
  starts off; re-enable each session if you want it on
  consistently. v2 promotes this to MCP-server state.
