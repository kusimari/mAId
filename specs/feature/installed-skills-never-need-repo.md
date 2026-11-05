# Feature: installed-skills-never-need-repo

## Git Setup

- Branch: `feat/installed-skills-never-need-repo/work`
- Base: `initiative/my-harness`
- Worktree: `mAId-worktrees/installed-skills-never-need-repo`

## Feature Brief

Once mAId is installed, a session never needs the mAId repo. A skill that
keeps user state (today: writing-style's learned and promoted rules) keeps
it in a user-owned file it loads on top of its shipped body, and Claude
reads and writes that file with no permission prompt. No skill tells the
runtime to find, read or edit a checkout, and content validation fails a
skill that does.

Part of initiative: [[my-harness]]

## Handoff

- **Stage:** closed
- **Ready for:** squash-merge into `initiative/my-harness` by the
  ringmaster
- **Carry forward:** none open. The skill-state grant, its narrowing to
  stateful skill dirs, the two path forms and the repo-pointer check are in
  project.md ("Skill state (claude)"); a new stateful skill joins
  `reads::STATEFUL_SKILLS`.
- **Deliberately left:** codex, kiro and agy grants are the initiative's
  Next round, not a backlog item. Not wanted, so not filed: seeding the
  shipped learned rule and the em-dash body (the user's promotion cycle,
  ruled), Claude-version drift of the grant (accepted), and the accepted
  review notes (`MAID_CONFIG` apart from the skill's path, a relative
  `MAID_CONFIG`, the loose `just install` match, collapsed "another
  part's" skips).

### Crossings

<!-- Append one line per crossing; never edit or delete a line. -->

- research → planning
- planning → dev
- dev → planning · RETURN · fault: requirements · issue: the grant covered
  all of `<config>/maid`, so an agent could rewrite the browser allowlist
  with no prompt (code review, security) · fix: grant only each stateful
  skill's dir, amend R4 and the design · done when: no rule mAId writes
  covers `browser-allowlist` or `memory-store` (unit test) and rollback
  checks the narrowed rule
- planning → dev
- dev → review
- review → closure

## Why

Seen in a session on 2026-10-13 (writing-style promotion): the skill told
the agent to edit `resources/content/skills/writing-style/SKILL.md` "in a
mAId checkout", so it edited the repo source, which Claude does not read
at runtime, and the edit had to be reverted. The installed copy is
read-only, so promoted rules had nowhere to go. `learned.md` under
`~/.config/maid/writing-style/` prompted on every read and write, because
only the installed plugin tree is granted.

## Requirements

- "promote learned rules" (or the session-start offer, accepted) moves each
  confirmed rule out of the pending list into a promoted list in the user's
  writing-style file. The agent never edits a `SKILL.md`, the installed
  copy, or a checkout, and never asks for one.
- Promoted and pending rules in the user's file apply in every session, on
  top of the shipped body, and win where they conflict with it.
- The session-start promotion offer counts only the user's pending rules;
  the shipped skill carries none.
- After `just install`, Claude reads and writes a stateful skill's files
  under `${XDG_CONFIG_HOME:-~/.config}/maid/<skill>/` (today
  `writing-style/`) with no permission prompt, in the default (manual)
  mode, also when HOME sits behind a symlink. The rest of that dir (the
  browser allowlist, the memory store's file) is not granted.
- `just uninstall` removes exactly what install added for that; anything
  the user or another part put in the same settings stays. `just status
  --whole` reports it as mAId's.
- An existing user file is never overwritten or migrated by install.
- No shipped skill text points the runtime at the repo; `just install`
  (content validation) and `just test` fail, naming file and line, when a
  skill says `resources/content/`, "mAId checkout" or `just install`.

## Test Strategy

### Unit (`just test`)

- Content check: a temp skill tree with each pattern, in `SKILL.md` and in
  a module under a subdir, fails with `file:line`; a clean tree passes.
  `shipped_content_validates` runs it on the real tree, so today's
  writing-style and browser text fails it before the content fix.
- `reads.rs`, per grant: the existing plugin-dir cases unchanged, plus the
  config grant: its `Edit` rules added beside the user's own `allow`
  entries; rerun byte-identical; an identical rule adopted; another part's
  rule neither adopted nor added; uninstall removes only recorded rules of
  mAId's shape; a stale rule (config path changed) replaced; a config path
  behind a symlink yields a rule for each form, a plain one a single rule;
  no plugin in the profile still grants the config dir.

### Integration (free)

- `tests/reads_install.rs`: install then uninstall through the stage
  functions, temp HOME and config, stub claude; the settings file ends as
  it started; the part list records each rule with its value; `--whole`
  finds them.
- `resources/tests/rollback`: the config grant there after install, gone
  after uninstall, beside the plugin-dir grant it already checks.

### Paid

- All three probe runs are spent (Session Log); no more in dev.
- writing-style fixture: a new `playback` section (where a promoted rule
  goes; what the agent never edits), checked free with
  `just resources::verify-skills-dry`; the paid run is the user's.

## Design

**One user file, two lists.** `${XDG_CONFIG_HOME:-~/.config}/maid/
writing-style/learned.md` holds `## Learned rules` (pending) and gains
`## Promoted rules`: promotion moves an entry there, rewritten terse and
prefixed by its target section (`- Punctuation: ...`), keeping its example.
The skill reads the file at load; both lists apply on top of the body and
win on conflict.

**The shipped `## Learned rules` section goes**, with no install-time
seeding: its one entry is already in the user's file. Also removed:
writing-style's and kdevkit's trailing `## Session Log` sections, which
tell the runtime to append to the shipped `SKILL.md`. browser's "point the
user at `just install`" becomes "say the browser capability is not
installed".

**The grant: `Edit` allow rules on every form of each skill state dir.**
Install adds `Edit(/<config>/<skill>/**)` to `permissions.allow` in Claude's
user settings for each skill in `reads::STATEFUL_SKILLS` (today
writing-style), where `<config>` is `${XDG_CONFIG_HOME:-$HOME/.config}/maid`,
and the same rule on the resolved path when it differs. The config dir
itself is never granted: it holds the browser allowlist. Claude checks a path both as
written and as it resolves, so through a symlinked HOME both rules are
needed; with both, Read and Write pass with no prompt and no working
directory (probe 3, shape g). `additionalDirectories` stays the plugin dir
only.

`<config>` comes from a new `MAID_CONFIG` the resources Justfile exports;
`MAID_MEMORY_STORE` defaults under it. The resolved form canonicalises the
deepest existing ancestor and rejoins the rest, since install does not
create the dir (the rule lets the first write create it).

**Code: `reads.rs` holds both grants.** A grant is one list under
`permissions` (`additionalDirectories` or `allow`), the elements mAId
wants there, and a shape test for what mAId may have recorded (plugin dir:
absolute, ends `marketplace/plugins/maid`; rule: `Edit(/<abs>/<stateful
skill>/**)`, no glob or rule characters; no `..`). A config path whose rule
would fail that test is granted nothing and recorded skipped, so install
never writes what uninstall cannot remove. Per element the M7 rules hold unchanged: add, adopt an identical
one, skip another part's, replace a stale recorded one, and remove only
recorded ones on uninstall. The part list gets one `setting` entry per
element, each with its `value` (`record_named` takes a list). whole-status
needs no change (reader rule 14 checks each value).

**Content check** in `check_content`: every `.md` under each skill dir,
line by line (linked dirs not followed), `resources/content/`, `just
install`, `just resources::` and "mAId checkout", case-insensitive, reported with the other validation errors.

**project.md:** "Skill reads (claude)" covers the config grant; a skill
state sentence (user-owned files under `<config>/<skill>/`, loaded on top of
the shipped body; updating the shipped body is a maintainer action, an
edit under `resources/content/` and `just install`, never a runtime one);
Hard constraints list the new rules.

## Implementation Plan

- [x] Content check in `check_content`, with unit tests (fails on today's
  tree)
- [x] writing-style, browser and kdevkit text fixed; writing-style
  playback section; `verify-skills-dry` clean
- [x] `MAID_CONFIG` in the Justfile and `shared.rs`
- [x] `reads.rs` grants (plugin dir, config rules); stage wiring; unit and
  `reads_install` tests
- [x] `resources/tests/rollback` checks the config grant
- [x] project.md: skill state, grant, constraints, testing

- *Risk note:* `permissions.allow` is the user's busiest list; mAId only
  ever adds or removes its own recorded rule strings.

## Session Log

- 2026-10-13 · Grounding. Repo pointers in skills: writing-style
  (promotion, lines 219-225), browser (`just install`); `## Session Log`
  append instructions in writing-style and kdevkit `SKILL.md`. The user's
  real `learned.md` already holds the shipped spaced-hyphen entry.
- 2026-10-13 · Paid probe 1 (haiku, temp HOME, `--permission-mode
  default`, plain `/tmp` paths): `additionalDirectories` element: Read
  allowed, Write and Edit denied. `Read(//y/**)` + `Edit(//y/**)` allow
  rules, no working dir: Read, Write (new file) and Edit allowed.
- 2026-10-13 · Paid probe 2 (same, every path through a symlinked parent
  `link -> real`): `additionalDirectories` on the link path: Read allowed,
  Write denied. Allow rules `Read`+`Edit` on the link path: both denied.
  `additionalDirectories` + `Edit` on the link path, or + `Edit` on the
  resolved path: Read allowed, Write denied. `Edit` rule alone: Read and
  Write denied.
- 2026-10-13 · Paid probe 3 (allowed by the ringmaster; HOME itself a
  symlink, files under `$HOME/.config/<x>`, addressed by the link path):
  `Edit` on the link and resolved paths, no `additionalDirectories` (g):
  Read and Write allowed. The same plus `additionalDirectories` on the
  link path (f) or both paths (i), and `Read`+`Edit` rules on both paths
  (j): allowed. `additionalDirectories` plus `Edit` on the resolved path
  only (h): Read and Write denied. Shape g is the least.
- 2026-10-13 · Code review (one dispatch, three lenses): correctness FAIL
  (high: a rule not of mAId's shape could be written and never removed),
  security FAIL (high: the grant covered `browser-allowlist` and
  `memory-store`), comment-hygiene PASS WITH NOTES. Fixed: grant narrowed
  to skill dirs; rules mAId could not recognise never written; path
  normalised; dangling link kept as given; `just resources::` added to the
  check and dropped from browser; `md_files` no longer follows linked
  dirs; integration test now behind a symlinked root.
- 2026-10-13 · Code review cycle 2: correctness, security and
  comment-hygiene all PASS WITH NOTES. Fixed after it (local, tested; no
  third cycle, budget 2): a config path of the wrong shape now also removes
  mAId's earlier rules; `{ } \ !` excluded from rule paths; the
  `config_dir` doc names everything in the dir; project.md names the
  cross-session reach of the grant. Left: the notes listed in the Handoff.
- 2026-10-13 · Ringmaster rulings at review applied: promoted rules are
  dated and the newest three shown at session start; version drift noted.
  Paid run (budget +1) on 3eb7625, `check writing-style-promote --agent
  claude --kind playback`, temp HOME: PASS.
- 2026-10-13 · Closure. Backlog answer: this feature closes its own item
  (`installed-skills-never-need-repo`, promoted into this spec); no other
  item is closed. `writing-style-mcp-server`'s `promote_rule` still
  described editing the body and `SKILL.md`; it now moves the entry into
  the user file's `## Promoted rules`. project.md sections touched
  (Architecture, Layout, Testing, Deployment, Hard constraints) were
  updated in dev; nothing more at close.

## Decision Log

- Ruled at review: planted rules must not steer unseen, so at session start
  writing-style also states the count and newest three promoted rules
  (dated, newest at top) · no state needed · rejected: "since last
  listed" (needs a seen-marker the skill would have to keep).

- Grant only `<config>/<skill>/` for each stateful skill, from a list in
  `reads.rs` · the config dir holds the browser allowlist, which project.md
  says no agent can widen · rejected: moving skill state under
  `<config>/skills/` (migrates the user's existing file).

- Promotion target: a `## Promoted rules` list in the existing user file ·
  one file, one grant path · rejected: a second file.
- Shipped learned entry removed, not seeded at install · the only user
  already has it; install writes no user data · rejected: seed at install
  (code for no current case), promote into the body (user's own cycle).
- Grant: `Edit` allow rules on the config path as written and as resolved,
  nothing else · probe 3 shape g grants Read and Write through a symlinked
  HOME · rejected: `additionalDirectories` alone (no writes, probe 1), a
  rule on one form only (denied through a symlink, probes 2 and 3 h), an
  added `additionalDirectories` element or `Read` rules (no gain, probe 3
  f, i, j).
- Ruled by the ringmaster at the planning gate: no install-time seeding;
  kdevkit's Session Log removed too; Claude only; install does not create
  the config dir.
