# Feature: writing-style-v1.1

## Git Setup

- Branch: `feat/writing-style-v1.1` (off `main`)
- Base: current tip of `main`

## Feature Brief

`sources/skills/writing-style/SKILL.md` ships at v1.0.0
today as a passive style reference — the assistant reads it
at session start and the rules influence its prose. v1.0
has no invokable formatter, no way to flag drafts before
they're sent, and no mechanism to capture style updates the
user notices mid-session.

v1.1 adds three things to the same SKILL.md (existing rules
unchanged):

- **Formatter verb.** "Format this in my style: &lt;passage&gt;"
  rewrites a passage against the rules and returns the
  rewrite plus a bulleted change log.
- **Strict mode (opt-in guardrail).** "Strict mode on" /
  "off" toggles a session-scoped flag. When on, the
  assistant flags style violations on any prose draft
  before sending.
- **Explicit-teach learner.** "New rule: I dislike 'in
  order to' — always 'to'" appends a dated entry under a
  managed `## Learned rules` section. The symlinked
  `~/.claude/skills/writing-style/SKILL.md` resolves into
  the mAId checkout, so the edit lands in the source file
  in place. At wrap-up the skill offers to promote stable
  entries into the curated body sections.

The MCP-server promotion (formatter as a typed tool with
diffable change log; learner state with file locking;
embedding retrieval against `#style` notes) is captured as
a backlog item for v2.

## Requirements

1. **Formatter verb.** Trigger phrases: "format this in my
   style", "rewrite to my voice", "fix this in my style".
   Output is the rewritten passage plus a bulleted change
   log explaining each substantive edit ("cut hedge
   'arguably'", "merged two ideas → one sentence",
   "removed business-speak 'leverage'"). Pure
   prompt-driven; no runtime state.
2. **Strict mode toggle.** "Strict mode on", "strict on",
   "guardrails on" → enables flagging for the session.
   "Strict mode off" / "guardrails off" → disables.
   Default off. State is **session-scoped** in v1: when
   the session ends, the flag resets.
3. **Strict mode behaviour.** When on, any prose draft the
   assistant produces (replies, emails, PR descriptions,
   docs) is preceded by a bullet list of style
   violations with span quotes; the assistant offers to
   apply the fixes before sending.
4. **Explicit-teach trigger.** Strict prefix match only:
   `new rule:`, `add rule:`, `style rule:`. Phrases like
   "I prefer X" or "stop using Y" are **not** triggers
   (too easy to fire on conversational use). On a prefix
   match, the assistant appends to `## Learned rules` in
   the source SKILL.md.
5. **Learned-rule entry shape.** Each entry: date,
   one-line rule, optional example, source-tag. Newest
   at top of the section.

   ```markdown
   ## Learned rules

   <!-- Newest at top. Entries are explicit teach turns.
        At wrap-up, promote stable ones into the body
        sections above. -->

   - 2026-05-22 · prefer "to" over "in order to"
     · *example:* "in order to" → "to"
     · source: explicit teach
   ```

6. **Promotion offers.** Two triggers:
   - **Next-session offer.** When the skill loads at
     session start and `## Learned rules` has any
     entries, the assistant offers promotion as the
     first thing in the response (after the
     `[writing-style] applies` line).
   - **On-demand verb.** "Promote learned rules" /
     "review learned rules" → assistant lists pending
     entries with proposed target sections and asks the
     user to confirm each.

   On confirmation, the assistant edits the corresponding
   body section (Voice / Sentence structure / Punctuation
   / etc.) and removes the entry from `## Learned rules`.
7. **Concurrency limit (v1).** Two parallel sessions
   teaching at the same time can race. Document this in
   SKILL.md as a known v1 limit; v2 MCP server adds a
   file lock.
8. **Frontmatter version bump** to `1.1.0`.

## Design

### Three new sections in SKILL.md

Existing sections (Voice / Sentence structure / Punctuation
/ Vocabulary / Paragraph structure / POV / Emphasis / Other
patterns) remain unchanged. Append:

- `## Formatter` — describes the verb, the trigger
  phrases, the output shape (rewritten passage + change
  log), and the rule that *every* edit in the change log
  must cite the section it came from
  ("Vocabulary: 'leverage' → 'use'").
- `## Strict mode` — describes the toggle phrases, the
  on-state behaviour (lint-before-send), and that the
  flag is session-scoped.
- `## Learning loop` — describes the teach trigger phrases,
  the entry format, and the wrap-up promotion offer.
- `## Learned rules` — the managed list itself, initially
  empty under a comment block.
- `## Known limits (v1)` — concurrency caveat, pointer to
  the v2 backlog.

### Session Log → repurpose

The existing `## Session Log` section at the bottom is
kept (skills observe patterns there during a session). The
distinction:

- **Session Log** — passive observations the *assistant*
  appends mid-session. Promoted at wrap-up.
- **Learned rules** — explicit teach turns the *user*
  drives. Promoted at wrap-up.

Both feed the body but through different signals.

### Concurrency note

Two simultaneous sessions both trying to append to
`## Learned rules` will race — last writer wins, the
earlier append is lost. In solo personal use this is rare;
documented as a known v1 limit. The v2 MCP server adds
POSIX `flock` around teach-rule writes.

### File path

The assistant edits
`~/.claude/skills/writing-style/SKILL.md` (a symlink into
the mAId checkout). The Edit tool follows the symlink and
writes the source file. **`git status` in the mAId
checkout will show the modified file** after a teach turn
— this is the chosen behaviour: rules are version-controlled
artefacts.

## Test Strategy

- **Unit tests:** none new — schema validator already
  covers the frontmatter shape.
- **Structural smoke:** existing
  `tests/functional/run --no-tools` continues to assert
  the symlink resolves.
- **Tool smoke:** update
  `tests/functional/skills/writing-style.smoke` to
  exercise one of the new sections — easiest is the
  formatter-trigger phrase, since it's the most
  discoverable user-facing change.
- **Manual end-to-end** (executed by the user):
  - "Format this in my style: &lt;draft with hedge
    word&gt;" → rewrite + change log citing Voice section.
  - "Strict mode on" → next prose draft is preceded by
    violations; "off" → behaviour reverts.
  - "New rule: I dislike 'in order to' — always 'to'" →
    `git status` in mAId shows
    `sources/skills/writing-style/SKILL.md` modified;
    new entry under `## Learned rules`.
  - End-of-session signal → assistant offers promotion of
    learned rules.

## Implementation Plan

1. Append the four new sections to
   `sources/skills/writing-style/SKILL.md`. Bump
   `version` to `1.1.0` and refine the description to
   mention the formatter / strict mode / teach loop.
2. Author `specs/backlog/writing-style-mcp-server.md`
   capturing the v2 promotion.
3. Update the writing-style functional fixture to
   exercise the formatter verb.
4. Run gates: `deno task fmt && deno task lint && deno
   task check && deno task test &&
   ./tests/functional/run --no-tools`.
5. Commit: `feat(writing-style): add formatter, strict
   mode, teach loop`.
6. Push branch.

### Risks

- **Frontmatter parser fragility.** The existing parser
  rejects block-style YAML lists. As long as the new
  sections only add prose (no new frontmatter fields) the
  validator stays green. **Confirmed:** v1.1 doesn't
  change frontmatter shape beyond `version` and
  `description`.
- **Edit tool symlink behaviour.** Writes through the
  symlink land in the source file. Verified by the v1
  feature's exploration — Deno's symlink and the OS-level
  symlink behave the same way; `Edit` follows the link.

## Session Log

<!-- Newest at top -->

- 2026-05-22 · feature spec drafted from approved plan
  (formatter verb + strict-mode toggle + explicit-teach
  learner; MCP promotion deferred to v2).

## Decision Log

<!-- Newest at top -->

- 2026-05-22 · **Strict mode is session-scoped, not
  persistent.** A persistent flag would need either a
  state file ($XDG_CONFIG_HOME/writing-style/state.json)
  or an MCP server. Both are heavier than the value of
  the feature in v1. Session-scoped means a new session
  starts off the default. Trade-off: the user must
  re-enable strict mode each session if they want it on
  consistently. Accept; revisit in v2.
- 2026-05-22 · **Teach turns edit SKILL.md directly.**
  Alternative: write to a sibling file
  (e.g. `learned-rules.md`) that the assistant reads as
  context. Rejected because it splits the canonical
  source — rules belong in the curated artefact. Trade-off:
  any chat session can dirty the mAId checkout, but the
  user explicitly chose this in planning.
- 2026-05-22 · **Wrap-up promotion is opt-in per entry,
  not bulk.** The assistant lists candidate promotions;
  the user accepts one at a time. Avoids accidental
  promotion of a rule the user later regrets.
