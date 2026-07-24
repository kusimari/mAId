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
# reminders/2026-05-21-<slug>.md
---
date: 2026-05-21
kind: reminder
due: 2026-05-26
links: [[topic-A]]
---

<one-line reminder body>.
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
# people/<person>.md (append a section)
---
kind: person
---

## 2026-05-21 1:1
links: [[topic-A]], [[topic-B]]

- <bullet>
- <bullet>
```

```yaml
# conversations/2026-05-21-<slug>.md
---
date: 2026-05-21
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

## v1.1 refinements (2026-05-28)

Three changes shipped in `feat/notes-v1-1`, driven by the
first real migration target (`~/notes/personal-vault/scrap.md`)
exposing v1.0 gaps:

1. **Vault selection at invocation time.** Capture and
   retrieval verbs accept an `in <name|path>` qualifier.
   `in <name>` resolves to `$NOTES_VAULT_<NAME_UPPER_SNAKE>`;
   `in <path>` (starts with `/`, `~`, `./`) is used directly;
   bare `add note for …` falls through to `$NOTES_VAULT` →
   `$HOME/notes`. Unset named-vault env var is an error, not
   a fall-through. Lets one user run e.g. a personal
   `personal-vault` vault and a separate work vault from the
   same skill.
2. **Slug-only filenames.** Dropped the
   `<YYYY-MM-DD>-<slug>.md` template for reminders, insights,
   and conversations. Filenames are now `<slug>.md`. Dates
   live in `date:` frontmatter (one-shot kinds) or as
   `## YYYY-MM-DD` section headers (accumulating kinds —
   reminders join people in this category). Avoids a
   directory of single-line dated files for a rolling
   reminders list and keeps related captures co-located.
3. **Optional `source:` for conversations.** Three shapes:
   `audio:<path>` (existing — transcribes), `notes-from:<who>`
   (new — hand-captured), or omitted (new — pure prose).
   Lets verbatim seller-voice captures and customer-quote
   notes use the conversation kind without faking an audio
   path.

## v1.3 revised (PR #10 review, 2026-05-28)

PR #10 review feedback redesigned the v1.3 git story from
per-verb auto-commit into an **open / work / close** session
model. Same branch, follow-up commit on the same PR.

The model:

- **Open** — every notes verb pulls (`git pull --ff-only`)
  before running. Pull failure surfaces verbatim and stops
  the verb.
- **Work** — captures and buffer merges write files and
  return; no `git add`, no `git commit` during work. Dirty
  files accumulate across verbs.
- **Close** — explicit only (`close notes`, `done notes`,
  `wrap up notes`). Pulls again, then `git add` only the
  dirty paths under the seven skill-owned directories
  (`inbox/`, `reminders/`, `insights/`, `people/`,
  `conversations/`, `topics/`, `scratch/`), commits in one
  squash with a path-shaped subject (single file → that
  path; buffer-merge → `merge buffer (<N>)`; multi → comma-
  joined with `…and K more` overflow), then pushes with
  `GIT_TERMINAL_PROMPT=0`.

Edge cases (no `.git`, no remote, detached HEAD / mid-rebase
/ mid-merge, push fails, empty close) consolidated into one
bullet block at the end of the section.

The supplanted per-verb model is preserved below as audit
trail; v1.3 revised is the shipped contract.

## v1.3 refinements (2026-05-28)

Two changes shipped in `feat/notes-v1-3`:

1. **Vault auto-commit on successful verbs.** When the
   vault has a `.git` directory, every successful capture
   or buffer merge auto-commits its writes (path-scoped
   staging, never `git add -A`) and pushes when a remote
   is configured. Successful = every intended write
   landed cleanly; aborted captures, missing transcription
   tools, partial-merge errors, and read-only verbs do not
   commit. Commit messages are minimal and path-shaped:
   `insights/<slug>.md`, `conversations/<slug>.md`,
   `reminders/<file>.md`, `people/<person>.md`,
   `merge buffer (<N>)`. Push runs with
   `GIT_TERMINAL_PROMPT=0` so credential prompts on a fresh
   machine fail fast instead of hanging on a TTY the agent
   can't satisfy; push failure surfaces a one-line warning
   and the commit stays local. Non-git vaults silently
   skip — the skill works unchanged for iCloud-only /
   Dropbox-only / no-sync setups. Closes the multi-machine
   sync loop the v1.1 named-vault env vars opened.
2. **Compression pass on `SKILL.md`.** Now that the skill
   is mature (three releases of refinements), the prose
   was tighter than it needed to be. Cuts: collapsed the
   four per-kind frontmatter examples into one combined
   block; merged steps 4–6 of the audio-transcription
   procedure into one paragraph; pulled the
   plugin-first-then-ripgrep ladder up once at the
   retrieval section open; dropped the rationale paragraphs
   (rationale lives in this Decision Log); cut "Known
   limits (v1)" as a backlog meta-note that wasn't a rule
   the agent reads; tightened the Behaviour rules digest to
   genuinely cross-cutting rules. Net length is +26 over
   v1.2 — the compression cut ~50 lines from existing
   material, the new auto-commit section runs ~80 (rules
   needed spelling out to be agent-readable).

## v1.2 refinements (2026-05-28)

Two changes shipped in `feat/notes-v1-2`, driven by (a) PR
#7 review feedback asking the new vault-selector fixture
to use the LLM-as-judge pattern the kdevkit fixtures
already use, and (b) the disconnected-capture gap — the
skill is only reachable from an agent session, but the
user works in Obsidian directly some of the time and needs
a place to land notes for later merge.

1. **Disconnected capture buffer.** New `scratch/`
   directory in the vault layout. `scratch/buffer.md` is a
   single free-form file the user appends to in Obsidian
   while disconnected. `merge buffer in <vault>` drains
   the buffer by running the existing classifier over each
   entry, archives the buffer to
   `scratch/buffer-<YYYY-MM-DD>.md`, and resets `buffer.md`
   to its header-only state. `show me my buffer in <vault>`
   prints the buffer for review. Both verbs are
   vault-selector-aware; bare forms hit the default chain.
   Merge is "add note for X applied in a loop" — no
   special-case rules.
2. **LLM-as-judge fixture for vault selection.** The
   `notes-vault-selector.smoke` fixture gains an
   `expected_narrative:` block matching the kdevkit
   fixture shape (numbered behaviours + explicit
   wrong-answer callouts). The runner already supports
   the field; the v1.1 fixture just didn't use it. A
   sibling `notes-add-note.smoke` fixture covers the
   capture verb end-to-end. The test runner is extended
   to invoke `claude` with `--dangerously-skip-permissions`
   so future fixtures can exercise real writes; current
   fixtures stay read-only by design (they ask the agent
   to *plan*, not write).

## Session Log

<!-- Newest at top -->

- 2026-05-28 · v1.3 revised on PR #10 review: per-verb
  auto-commit replaced with open/work/close session model.
  Open = `git pull --ff-only` before any notes verb. Work =
  captures write but don't commit. Close = explicit verb
  (`close notes` / `done notes` / `wrap up notes`) that
  squash-commits dirty skill-owned paths and pushes.
  SKILL.md `## Vault auto-commit` section replaced with
  tighter `## Vault git: open / work / close` (~35 lines vs.
  ~80). Behaviour rules updated. Fixture
  `notes-git-commit.smoke` rewritten to cover ten
  behaviours under the new model. Net SKILL.md drops from
  443 → 393 lines.
- 2026-05-28 · v1.3 ships vault auto-commit on successful
  verbs (`add note`, `merge buffer`), path-shaped commit
  messages, push-when-remote with `GIT_TERMINAL_PROMPT=0`,
  silent skip on non-git vaults, `vault commit skipped:
  <reason>` on detached HEAD / mid-rebase / mid-merge.
  Compression pass on SKILL.md alongside (collapsed
  per-kind frontmatter examples, retrieval ladder, audio
  transcription steps; cut Known limits and rationale
  paragraphs). New `notes-git-commit.smoke` fixture covers
  the eight behaviours via judge mode.
- 2026-05-28 · v1.2 ships disconnected-capture buffer
  (`scratch/buffer.md` + `merge buffer` + `show me my
  buffer`) plus LLM-as-judge fixtures
  (`notes-vault-selector.smoke` rewritten,
  `notes-add-note.smoke` added). Runner extended to
  bypass permissions on `claude`.
- 2026-05-28 · v1.1 design refinements driven by scrap.md
  migration: vault selector, slug-only filenames, optional
  conversation `source:`. Documented above.
- 2026-05-21 · feature spec drafted from backlog +
  in-conversation decisions (single verb, Obsidian patterns,
  plugin-first retrieval ladder, transcript-only conversation
  storage, capture-only volunteering).

## Decision Log

<!-- Newest at top -->

- 2026-05-28 · **Open / work / close session model over
  per-verb auto-commit.** Source: PR #10 review. The
  per-verb model churned `git log` with path-shaped
  subjects (one commit per capture), forced a push decision
  on every verb, and split a "session of notes work" into
  N atomic commits when the user thinks of it as one
  session. The session model matches the user's mental
  model: sit down → pull → capture some things → walk away
  with one squash commit. Trade-off: a dirty working tree
  between verbs in the same session — acceptable, the close
  is one verb away. Supersedes the per-verb decision below
  and the closely related "auto-commit triggers per
  successful verb" decision.
- 2026-05-28 · **Explicit-only close, no implicit close.**
  Considered: marker-file-with-timeout, idle-detection
  heuristics, end-of-chat triggers. Rejected — the skill
  has no daemon and no cross-invocation memory; any
  "implicit close" would need state the skill can't
  maintain reliably. User picked discipline over
  speculative state. Trade-off: a forgotten close leaves
  work uncommitted until the next close — visible in
  `git status` and surfaced again at the next close call.
- 2026-05-28 · **`git pull --ff-only` over `--rebase` or
  default merge.** Refuses on divergence with a verbatim
  git error; user resolves by hand. Personal vault is
  rarely divergent — the `--ff-only` is a forcing function
  for "don't run notes work against an out-of-sync vault."
  `--rebase` would replay local commits the work phase
  doesn't make anyway; default-merge would pollute the
  linear history a personal vault wants.
- 2026-05-28 · **Stage by directory scope on close, not by
  verb-tracked paths.** The original v1.3 plan tracked
  every path each verb wrote. The session model removes
  that bookkeeping: the seven skill-owned directories
  (`inbox/`, `reminders/`, `insights/`, `people/`,
  `conversations/`, `topics/`, `scratch/`) are the skill's
  scope; everything dirty in them at close is intentional.
  Smaller contract, less drift surface. Trade-off:
  hand-edits to old files in those directories during a
  session land in the close commit — stated outcome, not
  a bug.
- 2026-05-28 · **Reuse path-shaped commit-message shape
  at close.** User asked "why something different" — the
  close commit subject reuses the v1.3 path-shaped form,
  just folded into one subject. Single file → that path;
  buffer merge → `merge buffer (<N>)`; multi-file →
  comma-joined paths with `…and K more` overflow at ~70
  chars. Vault `git log --oneline` reads as a list of
  what landed.

- 2026-05-28 · **Auto-commit triggers per successful verb,
  not on a session boundary.** Considered: only commit on
  an explicit "wrap up notes" verb. Rejected — the agent
  has no daemon, "session done" has no concrete trigger,
  and users forget. Per-verb commit aligns with the kdevkit
  rule "commit when a coherent unit of work is done";
  multiple captures in one chat = multiple commits, which
  matches what `git log` should show. Aborted captures and
  partial-merge errors don't commit so the audit trail
  doesn't carry half-finished work.
- 2026-05-28 · **Push when remote exists, no per-vault
  opt-in env var.** Considered: gate push on
  `$NOTES_VAULT_<NAME>_AUTOPUSH=1` to avoid surprise
  outbound traffic. Rejected — the named-vault env vars
  already declare the user's intent that the vault is a
  multi-machine substrate, and pushing is the obvious
  follow-on. Push failure is a one-line warning, not an
  error, so the worst case on an unconfigured machine is a
  visible warning the user can ignore. `GIT_TERMINAL_PROMPT=0`
  prevents the agent from hanging on a credential prompt
  it can't satisfy.
- 2026-05-28 · **Path-shaped commit messages over body or
  Conventional Commits.** Three shapes considered:
  Conventional kind-scoped (`notes(insight): <slug>`),
  plain prose with body (`add insight: <slug>`), and
  path-only (`insights/<slug>.md`). Picked path-only — the
  vault is a personal store, not engineering code. `git
  log --oneline` reads as a list of files that landed,
  kind is implicit in the directory, no body to truncate
  or wordsmith, no `add` verb that's true of every commit.
  Trade-off: two reminders captured in the same session
  produce duplicate `reminders/inbox.md` subjects in
  `git log --oneline`; acceptable because each commit's
  diff shows the appended section, and `git log -p
  reminders/inbox.md` resolves it. Considered fixing with
  a hash suffix or body — that reintroduces the noise the
  shape was chosen to avoid.
- 2026-05-28 · **Path-scoped staging, never `git add -A`.**
  A user editing `insights/foo.md` in Obsidian while the
  skill writes `insights/bar.md` must not see their dirty
  `foo.md` swept into the skill's commit. Rule is "stage
  every path the verb wrote or modified, exactly those" —
  including auto-created topic stubs and buffer-merge
  side-effects (archive, reset buffer, merge report). The
  SKILL.md spells out the side-effect set so the reader
  can't shortcut to "stage the destination."
- 2026-05-28 · **Non-git vault → silent skip; no
  `git init` offer.** Considered: prompt on first capture
  ("Initialize <vault> as a git repo so captures
  auto-commit?"). Rejected — the user may have intentionally
  picked iCloud / Dropbox / no-sync; an offer trains the
  skill to volunteer infrastructure decisions, which v1.0's
  capture-only volunteering rule already forbids. The
  named-vault env vars are the user's intent declaration;
  a `.git` directory is the user's intent confirmation.
- 2026-05-28 · **No `## Agent Development` README block in
  kdevkit.** Considered adding a soft-README-verify step
  to §9 of the kdevkit skill, alongside the existing
  soft-`project.md`-verify. Rejected — README in mAId is a
  thin install/develop/where-to-look pointer, not a
  feature catalogue; volunteering README edits on every
  feature close is noise. project.md's invariants surface
  is what soft-verify exists for. If a future project's
  README *is* a feature catalogue, that's a project-level
  signal handled in `project.md`'s `## Agent Development`
  block, not a kdevkit default.
- 2026-05-28 · **One buffer file over multiple
  disconnected files.** `scratch/buffer.md` is a single
  rolling file rather than e.g. one file per disconnected
  session or one per day. Picked the simplest shape that
  matches how the user already works (`scrap.md` was a
  single rolling file too). Multi-buffer / per-day-buffer
  shapes are cheap to add later if friction shows up;
  doing it now would be premature.
- 2026-05-28 · **Merge is "add note in a loop", not a new
  classifier.** Considered building a batch classifier
  with merge-specific heuristics (e.g. cross-entry topic
  inference). Rejected — a buffer entry is
  indistinguishable from a single-prompt capture, so
  reusing the existing classifier keeps behaviour
  predictable and the spec compact. Side benefit: any
  improvement to the single-prompt classifier
  automatically applies to merges.
- 2026-05-28 · **Archive-and-reset over leave-buffer or
  delete-merged-only.** After a merge, copy the body to
  `scratch/buffer-<DATE>.md` and reset `buffer.md` to
  header-only. Leave-untouched would create
  duplicate-classification risk on the next merge;
  delete-merged-only would pile up ambiguous entries
  across runs without making the next merge easier. The
  archive is the audit trail the user can verify against
  before deleting.
- 2026-05-28 · **`scratch/` over `inbox/` for the buffer.**
  `inbox/` is for classified-but-unsorted captures during
  normal skill use (with `kind: unsorted` frontmatter).
  `scratch/` is for unclassified pre-skill content —
  semantically distinct, deserves its own directory.
- 2026-05-28 · **Slug-only filenames over date-prefixed.**
  v1.0 used `<YYYY-MM-DD>-<slug>.md`. Dropped because (a) a
  rolling reminders list as a directory of dated single-line
  files is hostile to read, (b) related captures fragment
  across many files, (c) Obsidian's graph view doesn't care
  about filename dates — frontmatter `date:` and section
  headers are sufficient. Trade-off: `ls reminders/` no
  longer shows dates; have to open the file. Acceptable
  because the file itself shows `## YYYY-MM-DD` sections.
- 2026-05-28 · **Vault selector at invocation time over
  single env var.** v1.0 had `$NOTES_VAULT` only and a
  sketched-but-unshipped `$NOTES_VAULT_<NAME>` story.
  Promoted to shipped because real use exposes that one
  user runs personal + work vaults. Inline-path option
  (`in /path/to/vault`) added for ad-hoc one-shot writes
  like the scrap.md migration into personal-vault.
- 2026-05-28 · **`source:` is optional.** v1.0 required
  `source: audio:<path>` for conversations, but real
  captures of customer voice and verbatim quotes have no
  audio file. Adding `notes-from:<who>` and allowing
  omission keeps the conversation kind useful without
  fabricating audio paths.
- 2026-05-22 · **Slack-driven harnesses out of scope for
  v1.** v1 ships skills that work in any AI system that
  loads `~/.claude/skills/` or `~/.kiro/steering/skills/`.
  Wiring the skills into a Slack-driven harness — and the
  install mechanism that owns that harness's agent
  config — is a separate feature, designed against the
  install harness rather than bolted on here. Public mAId
  doesn't track that follow-up; it lives in the install
  harness's own spec tree.
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
