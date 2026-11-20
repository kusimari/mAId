# Feature: saved-in-session

## Git Setup

- Branch: `feat/saved-in-session/work`
- Base: `initiative/harness-core`

## Feature Brief

Fix R4-1 and R4-2 from Macro test run 4 (`/tmp/harness-core-macro/report.md`,
"Run 4"). The session always knows where each bring-in's bound files are,
so no skill falls back to a default path and reads the real HOME. A memory
the session saves in the persona's store, tagged or not, shows in the close
batch as saved in memory, and the reflection does not propose it again.

Part of initiative: [[harness-core]]

## Handoff

- **Stage:** closed
- **Ready for:** the ringmaster's squash-merge into
  `initiative/harness-core`
- **Carry forward:** none open here. The real-model check of R4-1 and
  R4-2 is the initiative's next Macro run; notes' vault is
  `backlog/notes-vault-from-config.md`.
- **Deliberately left:** none. An exact-text duplicate match and any
  absolute config value counting as a location are the design as
  shipped (project.md says both), not wanted work.

### Crossings
<!-- Append one line per crossing. Never edit or delete a line. -->
- start → planning
- planning → dev
- dev → review
- review → closure
- closure → closed

## Requirements

- R1. **Bound files in the genome.** The startup report names each
  bring-in's bound file and dir locations by key, beside where its state
  lives: writing-style's `learned` and `promoted` paths. Never a secret, a
  command or a relative value. The genome stays under 8,000 bytes for this
  repo's install with a second part.
- R2. **No default path in skill text.** writing-style reads its older
  files only where the session names them, and reads none when the session
  names none. No file a skill ships names `XDG_CONFIG_HOME`, `~/.config`,
  `$HOME/.config` or `${HOME}/.config`. browser stops naming its
  allowlist's default path and says the agent never looks for the file.
- R3. **Saved in memory, tagged or not.** Every memory a successful
  `memory_create` / `memory_str_replace` call wrote this session (or in a
  sub-session) on a persona memory's server, and that is in its store now,
  is a "saved in memory" line in the batch. A tagged one keeps today's
  line and feeds the move item. An untagged one shows its memory, persona,
  path and description, and feeds no item.
- R4. **No duplicate habit.** A habit finding to add a memory this session
  already saved (same file, or same text) is dropped with "saved in memory
  this session". The reflector is told to propose nothing for any memory
  the session saved.
- R5. **Macro step 3, both shapes.** The persona-memory habit is either a
  batch item (the correction not saved in session) or a saved-in-memory
  line (the agent saved it). Both pass the integration test, and in the
  second, "ok" writes only the move proposal.

## Test Strategy

- Unit (`entry.rs`): a genome line shows `locations: learned <path>,
  promoted <path>` for bound absolute paths, and leaves out an `env:`
  secret, a relative index and a JSON `serve`.
- Unit (`shipped_fit.rs`): this repo's install with writing-style's files
  bound: the genome names both paths and stays under 8,000 bytes.
- Content (`build-tool/tests/integration.rs`): no file a skill ships
  contains `XDG_CONFIG_HOME`, `~/.config`, `$HOME/.config` or
  `${HOME}/.config`.
- Unit (`adapt/tests.rs`): an untagged in-session save is listed as saved
  in memory with no thing, gives no move item, and a reflector habit for
  the same file or text is dropped as saved this session. The existing
  "others are not" test changes: untagged and unknown-tag saves are now
  listed; another server's, a failed one, a missing file and a path that
  leaves the store still are not.
- Integration (`tests/adaptation.rs`): Macro step 3 in both shapes.
- Gates: `just ci`, `resources/tests/rollback`, `claude_live` on Claude
  Code 2.1.314 against the loopback stub, `just resources::verify-skills-dry`.
  No paid run.

## Design

- **Genome.** `entry::Line` gets `locations`, a key-to-path map: each
  declared config key, not a state tier and not secret, whose bound value
  is an absolute path. `render_slice` appends `; locations: <key> <path>,
  ...`. State stays as it is. Paths only, no read of the file.
- **writing-style SKILL.md.** Step 2 reads `learned.md` / `promoted.md`
  at the paths the startup report names for writing-style's `learned`
  and `promoted`; none named, no older files. The default path goes.
- **browser SKILL.md.** The allowlist is the file set when the browser
  server was installed; the agent never reads or looks for it. No path.
- **Saved finder (`adapt/moves.rs`).** `Saved.thing` becomes
  `Option<String>`. `saved_into` keeps a file with no tag, or a tag no
  declared bring-in has, as `thing: None`, its text the frontmatter
  `description`, else the body's first line. `items` matches only
  `Some(thing)`. `batch` prints an untagged line as `<memory> [persona]
  <path>: "<text>"`.
- **Duplicate drop (`adapt.rs` `check`).** `saved` is computed first. A
  habit `add` whose thing is a memory is dropped when a saved entry on
  that memory has path `/memories/<slug(title)>.md`, a name equal to its
  title, or a description or body equal to its entry or change
  (squashed, lowercased), never on empty text. Naming no memory, it
  matches only the memory of the persona it names; naming neither, it is
  never dropped (the person places it). The index file is never listed as
  a saved memory.
- **Reflector.** Its "rules the person taught" bullet widens to anything
  the session saved to a persona's memory: the core lists it; propose
  nothing for it.
- **Docs.** project.md "Genome" and "Rules saved in memory" say the above
  in a line each. README's genome row adds where the bound files are.
  The content test reads every file a skill ships, not only `SKILL.md`.
- The memory server's own instructions stay as they are (the ruling).

## Implementation Plan

- [x] Genome locations + unit tests + shipped genome size test.
- [x] writing-style and browser SKILL.md, content test, smoke expectations
      that named the default path.
- [x] Saved finder for untagged saves, duplicate drop, reflector text,
      unit tests.
- [x] Integration test in both shapes.
- [x] project.md and README lines.
- [x] Gates: `just ci`, rollback, live on 2.1.314, verify-skills-dry.

## Session Log

- 2026-11-01: grounded on run 4's report, `entry.rs` (the genome line
  carried only state values), `adapt/moves.rs`, `adapt.rs` `check` and
  `place` (the "already" check is by title slug in the index), the
  reflector, and the shipped skills. `XDG_CONFIG_HOME` / `~/.config` appear
  only in writing-style and browser.
- 2026-11-01: Code Review Gate, cycle 1 (correctness lens, PASS WITH
  NOTES, nothing at `fail_on: high`). Fixed: the index file listed as a
  saved memory; duplicates matched only on description or file; a habit
  naming no memory matched any persona; an empty `thing`; browser text
  claiming a refusal names the allowlist file; the content test reading
  only `SKILL.md`; `locations` as pairs in JSON. Left: a command bound as
  one absolute path would be listed as a location (the contract makes
  `serve` a JSON array).
- 2026-11-01: Code Review Gate, cycle 2 (PASS WITH NOTES, nothing at
  `fail_on: high`). Fixed: an empty text matched an empty body; the name
  test now reaches the name branch; the content test also bans
  `${HOME}/.config`. Left: any absolute config value counts as a
  location (project.md says so), and paths are joined with `, `.
- 2026-11-01: review briefing. Fixed its defects: R2, the Test Strategy
  and the browser Design bullet still said what review cycle 1 changed,
  and the Handoff misnamed what followed the rollback run. On its
  question about a habit naming no memory and no persona: it is no longer
  dropped, since what the core drops never reaches the person.
- 2026-11-01: closure. Gates on the final code: `just ci`,
  `verify-skills-dry`, `claude_live` (6) and `claude_installed` (1) on
  Claude Code 2.1.314 against the loopback stub, and
  `resources/tests/rollback`, all passing. No paid run. Backlog answer:
  this stream closes no backlog item (`writing-style-grant-reads-only` is
  related but untouched) and adds `notes-vault-from-config`. project.md:
  Architecture (Genome, Saved in memory, Skill state), Layout (moves.rs)
  and Testing (the content test) updated; Tech Stack and Deployment
  untouched.

## Decision Log

- Genome lists bound locations rather than the skill asking the core:
  the least change, and the skill already reads the genome. Considered a
  core call from the skill; it adds a step every session.
- Ruling (ringmaster): notes' default vault stays out of this stream, as a
  backlog item. Its vault is the user's data, and no Macro step runs notes.
- Ruling (ringmaster): saves are listed per memory server, labelled with
  its persona. The core does not know the acting persona at the batch, and
  a save goes to the server the agent called.
- Review cycle 1 widened the duplicate match (name and body, persona when
  no memory is named), skipped the index, and widened the content test
  to every file a skill ships · why: each was a real miss the first
  design allowed.
- A habit naming neither memory nor persona is never dropped as saved ·
  why: the core cannot tell whose it is, and a drop hides it from the
  person; the batch asks them to place it instead.
