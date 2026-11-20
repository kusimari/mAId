# Feature: turn-bring-ins

## Git Setup

- Branch: `feat/turn-bring-ins/work`
- Base: `initiative/harness-core`
- Worktree: `mAId-worktrees/turn-bring-ins`

## Feature Brief

Every turn bring-in this repo installs tells the ghost what it is for,
when it fits, which few lines come first while it applies, and where its
own memory lives, so turn fit and adaptation have something to work from.
That covers mAId's six skills, the browser server, and the vendor's
document skills, which are borrowed and so get their declaration without
being changed. writing-style's style rules become two tiers, pending and
permanent, each at a location bound where it runs.

Part of initiative: [[harness-core]]

## Handoff

- **Stage:** closed
- **Ready for:** squash-merge into `initiative/harness-core` by the
  ringmaster
- **Carry forward:** none open here. The kiro and codex loading checks,
  the record sync keyed on a server's declaration, and a travelling
  permanent tier are backlog items.
- **Deliberately left:** turn fit and the use of `rules` (stream 2).

### Crossings

<!-- Append one line per crossing; never edit or delete a line. -->

- planning → dev
- dev → review
- review → closure
- closure → closed

## Requirements

### What the person sees

- `maid-core genome` lists every turn bring-in mAId installs with its
  job, none of them undeclared: kdevkit, kyodakit, kreviewkit,
  writing-style, notes, browser, the browser server, and the vendor's
  document skills.
- writing-style's line says it keeps its own memory, and where: a
  pending tier and a permanent tier, each a location under mAId's config
  dir. A fresh install, with no rule taught yet, shows writing-style
  ready.
- `just status --whole` shows the same bring-ins ready or not ready
  (why), never undeclared. The document skills say which commands they
  need that are missing, as plain `just status` already does.
- The browser server is a bring-in while some harness has it
  registered: it appears after `just install` (or
  `just resources::install-browser-mcp`) and goes with its registration.
- The vendor's document skills are unchanged on disk: nothing of the
  vendor's is edited, copied or wrapped. They stay borrowed and report
  only.
- Every skill still loads and behaves as before in every agent, with or
  without the core.

### What writing-style does

- `new rule: …` appends to the pending tier, as today.
- Promotion moves a rule from the pending tier to the permanent tier,
  a second file beside the first, newest at top, in the same entry shape
  as today.
- At session start both tiers apply on top of the body, as today; the
  pending offer and the promoted-rules line are unchanged.
- A rules file from before this change still works: its promoted
  section applies, and moves to the permanent tier the next time a rule
  is promoted.
- When the session names other locations for the two tiers, the skill
  uses those.

## Test Strategy

### Unit (`just test`, load-bearing)

Each names the change that would break it; temp dirs only.

- **Rules pointer** (core): a pointer resolves to its heading's section,
  ending at the next heading of the same or a higher level; a heading
  inside a code fence is not a heading; a missing file, an unknown
  anchor, an absolute path or a `..` is an error with a reason.
- **Optional tier** (core): an optional state key bound to a location
  that is not there leaves the bring-in ready; a non-optional one still
  makes it not ready (stream 1's test stays).
- **Shipped declarations** (build-tool, against the real content, like
  `shipped_content_validates`): every shipped skill is declared, scope
  `turn`, with a job and a rules pointer that resolves to at most 30
  lines; writing-style's state is `learned` (local) and `promoted`
  (permanent), promote `propose`, both keys optional; the browser
  server's manifest and the document-skills overlay read as declared
  `turn`; the overlay's `requires` equals the vendor row's `needs`.
- **Validator**: a skill whose declaration is malformed, or whose rules
  pointer does not resolve, fails content validation, so install never
  ships it.
- **Install records** (`stages.rs`): writing-style's `learned` and
  `promoted` are bound to `<config>/writing-style/learned.md` and
  `promoted.md`; the vendor plugin's unit is the overlay in the profile,
  its tenure still borrowed; `chrome-devtools` is recorded, with its
  `server.json` unit, only while an MCP entry of that name is placed;
  `build-tool record` adds it when the browser step places it and drops
  it when the step clears it, keeping every other bring-in.
- **Conformance**: one new case, an optional state location not there,
  reports ready; every existing case passes unchanged.

### Integration (free)

- `nix develop -c resources/tests/rollback` gains: after install,
  `env -i HOME=<temp> <profile>/bin/maid-core environment` shows every
  mAId skill declared, writing-style ready with no rules file, and the
  document skills declared (ready or not ready, never undeclared).
- `nix develop -c resources/tests/browser-mcp` gains: the part list's
  bring-ins hold `chrome-devtools` after install and not after
  uninstall.
- `just resources::verify-skills-dry`: every prompt still builds.

### Paid (four runs, allowed by the ringmaster)

Four runs, check stage, one each:

1. `writing-style-promote` playback on claude, updated for the two
   files: the promoted rule lands in the permanent tier.
2. `writing-style` playback on claude: `new rule:` still appends to the
   pending tier.
3. and 4. writing-style activation on codex and on kiro: a skill whose
   frontmatter carries `metadata` still loads there (claude's
   `plugin validate --strict` already accepts it, per stream 1).

## Design

**Why this shape.** Each declaration lives in its unit's own slot, as
stream 1 settled, so this stream is mostly content: frontmatter
`metadata` on six skills, a `server.json` for the browser server, and an
overlay for the vendor's plugin. Code changes only where content alone
would leave the record wrong: install binds writing-style's tiers and
records the browser server; the core resolves rules pointers and treats a
not-yet-written optional tier as empty.

**Declarations.** All are `scope: turn`. Signals are short lowercase
hints; `files` are globs.

| Unit | job | signals | rules | state, requires |
|---|---|---|---|---|
| kdevkit | spec-driven feature work on a repo with a spec tree, from plan through gates to close | jobs: plan a feature, run an initiative, act on review comments, close a feature, add to backlog · states: planning, dev, review, closure · files: specs/project.md, specs/feature/\*.md, docs/specs/project.md, .kdevkit/project.md | `SKILL.md#phase-gating-cues` | - |
| kyodakit | developer judgement on any coding work, in any language | jobs: design, build, fix, refactor, test, act on review feedback | `SKILL.md#0-stop-where-you-should-stop-or-are-expected-to-stop-ask-well-with-context` | - |
| kreviewkit | brief a human reviewer on a change from its spec and diff | jobs: brief a review, write a PR body · states: review | `SKILL.md#non-negotiables--check-every-one-before-you-return` | - |
| writing-style | write and rewrite prose in my voice, and keep the style rules I teach | jobs: rewrite, draft an email, draft a doc, teach a style rule | `SKILL.md#voice` | config `learned`, `promoted` (both optional); state local `learned`, permanent `promoted`, promote `propose` |
| notes | capture and find notes, reminders and transcripts in my vault | jobs: add a note, set a reminder, capture a transcript, find notes, close notes | `SKILL.md#behaviour-rules` | requires `git` |
| browser | drive my logged-in Chrome on allowlisted sites, attended | jobs: open a page, fill a form, click, read a value off a page | `SKILL.md#safety-posture-non-negotiable` | - |
| chrome-devtools (`server.json`) | the browser tools: my logged-in Chrome over DevTools, allowlisted sites only | - | - | - |
| document-skills (overlay) | read, create or edit Word, Excel, PowerPoint and PDF files | files: \*.docx, \*.xlsx, \*.xlsm, \*.csv, \*.tsv, \*.pptx, \*.pdf | - | requires: the vendor row's `needs` |

notes declares no state: the vault is the user's data, found by the
skill's own selector, not a memory of the skill's. The browser server has
no rules of its own; the browser skill carries them, so the same text
never comes from two bring-ins.

**Rules pointer.** `<file>#<anchor>`: `file` relative to the unit's
manifest dir (no absolute path, no `..`), `anchor` a heading's slug as
GitHub makes it (lowercase; drop what is not a letter, digit, space,
`-` or `_`; each space a `-`). The rules are that heading's section, up
to the next heading of the same or a higher level, skipping fenced code.
`contract::rules(unit_path, pointer) -> Result<String, String>` in the
core resolves it, so the validator, the shipped-content test and stream
2's expression use one reader. A small hand-rolled heading scan with
fence tracking; a markdown crate (pulldown-cmark) would be a new
dependency for one function.

**Borrowed units: an overlay recorded by the part.** The part ships a
manifest in the borrowed unit's own standard holding only `name`,
`description` and the `my-harness` slot, and records it as the bring-in's
`unit`. The vendor's files are never read for the declaration and never
changed; tenure stays borrowed and route report. This needs no new
part-list key, no core change and no new reader rule: `unit` already says
where the declaration is read. mAId's overlay is
`resources/content/overlays/document-skills/plugin.json`, in the profile
at `share/maid/overlays/document-skills/plugin.json`. One overlay for the
plugin, since the part records the plugin, not its four skills; the
shell's own activation still picks the skill.

**The browser server's manifest.** Every MCP server declaration in
this repo lives at `resources/content/servers/<name>/server.json`, in the
profile under `share/maid/servers/` (the ringmaster's ruling, shared with
stream 4, which adds that copy to the flake). The browser's is
`resources/content/servers/browser/server.json`, an MCP `server.json`
(`name`, `description`, `version`, `_meta`). Bring-in `chrome-devtools`
(the name the harnesses list), own, source this repo. The flake copies
`resources/content/overlays` to `share/maid/overlays` the same way, and
the servers dir too if stream 4 has not merged first (then the rebase
keeps one line). Neither is in the plugin dir, so the plugin version
does not change with them.

**Install.**

- `bring_ins()` binds each state key `<k>` of a skill's declaration to
  `<config dir>/<skill>/<k>.md`, reading the declaration from the
  profile's copy. writing-style: `learned.md`, `promoted.md`. Install
  creates neither file nor the dir, as today; the existing `Edit` grant on
  `<config>/writing-style/**` covers both.
- The vendor plugin's record gets `unit: { standard: plugin, path:
  <profile>/share/maid/overlays/<plugin>/plugin.json }`.
- `chrome-devtools` is recorded while an MCP entry of that name is
  placed on some harness (`still_placed`), unit
  `<profile>/share/maid/servers/browser/server.json`. The browser step writes
  its entries after the skills step, so `cmd_record`, which every
  browser record goes through, adds or drops that one bring-in to match
  what is now placed and leaves the rest. A first `just install` then
  records it in one pass.

**Core.** An optional config key's state location that is not there is
an empty tier, not a reason: `environment::readiness` checks existence
only for non-optional keys. Reader rule 15 says so ("an absolute state
location of a non-optional key that does not exist").

**Validator.** `validate_skill` reads the declaration through the core
when the frontmatter has `my-harness.` keys and resolves its rules
pointer; either failing fails the skill.

**writing-style.** The Learning loop writes `## Learned rules` in
`learned.md` (unchanged); Promotion moves the entry to `## Promoted
rules` in `promoted.md` beside it. Session start reads both. One line
covers an older `learned.md` that still has `## Promoted rules`, and one
says to use the locations the session names for the two tiers. The
`writing-style-promote` fixture's expect changes to the two files;
`verify-install` checks `promoted.md` is untouched as it does
`learned.md`. Version 1.2.0.

**project.md.** The contract gains the pointer format and the overlay
rule; "Skill state" names the two files; rule 15 the optional-tier
wording; Browser-control MCP its `server.json`; Layout the two new
files; Testing the new checks.

## Implementation Plan

- [x] Core: `contract::rules` with its tests; optional tiers in
  `environment`; rule 15 wording and the conformance case.
- [x] Declarations on the six skills; the validator reads them;
  `shipped` test; writing-style's two tiers in its body and fixture.
- [x] `server.json` and the document-skills overlay; flake copies them;
  install binds tiers, records the overlay and the browser server;
  `cmd_record` keeps the server in step; unit tests.
- [x] Rollback and browser-mcp test additions; project.md (contract,
  skill state, rule 15, browser, Layout, Testing); `verify-install`.

- *Risk note:* codex or kiro might reject a skill whose frontmatter
  carries `metadata` with dotted keys. The Agent Skills spec allows a
  string map there; paid runs 3 and 4 settle it. If one rejects it, the
  fallback is a stream-1 question (the slot), not a workaround here.
- *Risk note:* a heading rename breaks a rules pointer. The validator
  fails install on it, so it cannot ship broken.
- *Risk note:* stream 4 also touches `bring_ins()` in `stages.rs` (the
  memory server's unit) and the flake's servers copy. This stream stays
  off `personal-memory` and the Claude plugin manifest's
  `extensions."my-harness"` entry, which are stream 4's, and rebases onto
  whichever merges first.

## Session Log

- 2026-10-31 · grounded on the initiative spec, `core-and-infusion.md`,
  project.md "My harness", `contract.rs`, `environment.rs`, `parts.rs`,
  `stages.rs` (`bring_ins`, `cmd_record`), `shared.rs` (`BRING_INS`),
  the flake, `resources/browser/manage`, the six skills and the
  writing-style fixtures. Found: no skill carries `metadata`; the vendor
  plugin is recorded with no unit; a state location that does not exist
  makes a bring-in not ready, which would show writing-style not ready
  before any rule is taught; the browser step records after the skills
  step, so a bring-in derived only at the skills step would lag one
  install; `writing-style-promote` asserts promotion stays in one file.
- 2026-10-31 · dev: slices 1-4. Core: `contract::rules` and the
  optional-tier rule, each watched failing before passing; a conformance
  case for the optional tier. Six skills declared; writing-style split
  into `learned.md` and `promoted.md`, its promote fixture extended to
  the older single-file path; the validator reads declarations and the
  shipped servers and overlays. Install binds tiers through
  `bind_state` after `bring_ins()` (clippy refused an eighth argument,
  and this keeps `bring_ins()`'s signature, which stream 4 also edits);
  the vendor plugin's unit is the overlay; `build-tool record` of an MCP
  entry syncs the browser server's bring-in. `just ci`,
  `resources/tests/browser-mcp`, `resources/tests/rollback` and
  `verify-skills-dry` green. Rebased onto `initiative/harness-core`
  (spec-only commits upstream).
- 2026-10-31 · free check of codex loading `metadata`: `codex debug
  prompt-input` (codex 0.166, temp `HOME` and `CODEX_HOME`, no model
  call) lists all six skills with their descriptions. Codex also listed
  a skill with broken YAML, so its reader is lenient and `metadata`
  cannot break it. kiro has no such command (it needs a login), and the
  check stage carries a skill's text inline, so a check-stage run on
  kiro does not exercise its loader; only a smoke-stage run does.
- 2026-10-31 · Code Review Gate (one lens, correctness: no path in the
  diff is a named-risk path): PASS WITH NOTES. Took them all: fences
  close only on the same marker at least as long; headings may be
  indented up to three spaces and drop closing `#`s; `build-tool record`
  resolves its dirs before writing; the rollback name check matches
  exactly; tests for a skipped browser entry, a server's broken rules
  pointer, and `STATEFUL_SKILLS` matching the skills with state.
- 2026-10-31 · paid runs 1 and 2, check stage, claude: both PASS.
- 2026-10-31 · rebased onto `initiative/harness-core` with stream 4
  merged: conflicts in `stages.rs` (its `Placed`, `Sources`, adapter
  record beside the browser server and the tier binding), the flake's
  servers copy (kept once), project.md and the rollback test (both
  checks kept; the memory server is declared now, so the declared-check
  covers it). `memory_install.rs` passes the config dir. `just ci`,
  browser-mcp, rollback and `verify-skills-dry` green after.
- 2026-10-31 · review briefing sent back three defects, fixed: a missing
  blank line in project.md; `sync_browser_bring_in` keeps an existing
  record as install wrote it, so a later record from another checkout
  does not rewrite its revision (test added); a sync failure after the
  entry is written says so.
- 2026-10-31 · closure. Gates re-run on HEAD `1d327a0` before the close
  commit: `just ci` pass, `resources/tests/rollback` all 31 checks pass,
  `verify-skills-dry` all prompts build. No unticked plan items.
  project.md: Architecture, Layout and Testing were updated in dev and
  still read true; Tech Stack, Deployment and Hard constraints are
  untouched (no dependency, install path or invariant changed).
  Backlog: this feature closes none of the existing items; filed
  `kiro-loads-my-harness-frontmatter` (with codex activation unproven),
  `record-syncs-servers-by-declaration` and
  `writing-style-permanent-tier-travels`. Paid runs: 2 of 4 spent, both
  PASS; 2 returned to the initiative's pool.

## Decision Log

Calls made for the ringmaster from the initiative spec, all accepted at
the Planning Review Gate (2026-10-31):

- Ruling: a borrowed unit gets its declaration from an overlay, a
  manifest in the unit's own standard that the part ships and records as
  the bring-in's `unit`; the vendor's files are never read for it or
  changed · why: the least: `unit` already says where a declaration is
  read, so no new key, reader rule or core change · cost if wrong: an
  inline `declaration` key in the part list later; overlays convert to
  it one for one.
- Ruling: the vendor's document skills stay one bring-in (the plugin),
  one overlay · why: the part installs and records the plugin; the shell
  picks among its skills · cost if wrong: four records and overlays.
- Ruling: the overlay's `requires` equals the vendor row's `needs`, held
  by a test · why: plain status reads `needs` today; one test is less
  than moving status onto the core · cost if wrong: drop `needs` and
  read the overlay.
- Ruling: `rules` is `<file>#<heading slug>`, the heading's section,
  resolved by one reader in the core, at most 30 lines for mAId's
  bring-ins · why: points at lines already in the skill, so nothing is
  written twice; one reader for the validator and stream 2 · cost if
  wrong: change the resolver; pointers are data.
- Ruling: writing-style's tiers are two files, `learned.md` (local,
  pending) and `promoted.md` (permanent), config keys `learned` and
  `promoted`, promote `propose` · why: two tiers each at its own
  location, so each can be bound apart and written by location; one
  file for both makes the two keys the same value · cost if wrong: bind
  both keys to one file again; the skill change reverts.
- Ruling: install binds a skill's state key `<k>` to
  `<config dir>/<skill>/<k>.md` · why: one rule, no table, and it keeps
  today's `learned.md` · cost if wrong: a per-skill table.
- Ruling: an optional state key whose location is not there is an empty
  tier and leaves the bring-in ready · why: a tier the skill creates on
  first write is not missing; a store that must exist stays a
  non-optional key · cost if wrong: one rule in `environment.rs` and
  rule 15.
- Ruling (ringmaster): every MCP server declaration lives at
  `resources/content/servers/<name>/server.json`, copied to
  `share/maid/servers/`; the browser's at `servers/browser/`. Stream 4
  owns `personal-memory` and the Claude plugin's `extensions` entry.
- Ruling: the overlay lives beside it, at
  `resources/content/overlays/<plugin>/plugin.json` · why: the same
  content-dir shape the ringmaster set for servers · cost if wrong: a
  path.
- Ruling: the browser server's declaration is a `server.json` shipped in
  the profile; bring-in `chrome-devtools`, own, recorded while placed,
  kept in step by `build-tool record`; no rules or config of its own ·
  why: the slot stream 1 settled; the browser skill already carries the
  rules; the allowlist location is fixed in the launcher, not bound ·
  cost if wrong: a config key later.
- Ruling: content validation reads declarations and resolves rules
  pointers · why: a bad declaration or a renamed heading never ships ·
  cost if wrong: none.
- Ruling: every bring-in here is `turn`; notes declares no state · why:
  the initiative lists them as turn bring-ins; notes' vault is the
  user's data, not the skill's memory · cost if wrong: a field edit.
- Ruling (ringmaster): the four paid runs, check stage only, after the
  free gates; a failure stops for the ringmaster before any re-run.
- Ruling (ringmaster): the old single-`learned.md` path is covered by a
  test; the fixture change lands with the behaviour change.
- Ruling (ringmaster, review): kiro run declined (no stream touches
  the real HOME; kiro is mapped, not built); a codex listing is enough
  for this initiative; the browser sync inside `record` is accepted for
  now; migration of an older `learned.md` stays in prose (the files are
  the user's data); rules pointers stay as chosen, for the user at the
  initiative review.
