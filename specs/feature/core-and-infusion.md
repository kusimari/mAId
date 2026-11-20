# Feature: core-and-infusion

## Git Setup

- Branch: `feat/core-and-infusion/work`
- Base: `initiative/harness-core`
- Worktree: `mAId-worktrees/core-and-infusion`

## Feature Brief

The ghost gets a core: one program, installed with mAId, that reads every
part present, fuses their personas and bring-ins into one harness map,
works out what of it is ready where the session runs, and answers the
five entry points (`genome`, `expression`, `experience`, `sleep`,
`adaptation`) the same way for a hook, an adapter or an on-demand call.
This stream fills `genome` and the record of what was sent per session;
the other four answer, and do nothing yet, until their streams fill them.

Part of initiative: [[harness-core]]

## Handoff

- **Stage:** closed
- **Ready for:** squash-merge into `initiative/harness-core` by the
  ringmaster
- **Carry forward:** none open here. The content-addressed profile rule
  is in project.md (Architecture, "Install = a profile"); the rollback
  guard for `bin/maid-core` and the memory server's own source go to
  streams 2 and 4 through the ringmaster's briefs.
- **Deliberately left:** delivery record upkeep, filed as
  `backlog/core-delivery-record-upkeep.md`; the vendor skills and the
  memory server stay undeclared until streams 5 and 4.

### Crossings

<!-- Append one line per crossing; never edit or delete a line. -->

- planning → dev
- dev → review
- review → dev · RETURN · fault: implementation · issue: `share/maid/rev`
  in the profile makes every commit, docs-only too, a new generation and
  gives up the content-addressed profile · fix: drop it; install reads
  the revision (and a dirty marker) from the checkout it runs from ·
  done when: two installs with a spec-only commit between them leave the
  profile's store path the same, and the part list still records the
  revision and the dirty marker
- dev → review
- review → closure
- closure → closed

## Requirements

### What the person sees

- `maid-core genome` prints the session picture, a few lines per
  section, in a stable order:
  - **personas**: every part present, its persona, and which one this
    session most likely acts through (the only one when there is one;
    otherwise none, said plainly);
  - **session bring-ins**: one line each: name, job, persona, own or
    borrowed (from whom), and where its state lives;
  - **turn bring-ins**: a catalog, one line each: name, job, and whether
    it keeps its own memory and where;
  - **environment**: how many bring-ins are ready, and for each one that
    is not, why (a config value not bound, a location not there, a
    command it needs not on PATH, its manifest not readable, no
    declaration);
  - **pending**: how many decisions wait, and where.
- `--json` gives the same picture as one JSON object, for adapters.
- Called with a session id by a carrier that keeps history, `genome`
  sends the picture once: a second call for the same session sends
  nothing, until the picture changes or the call asks for a reset (after
  a compaction, a clear or a resume). A sub-session (a sub-agent) has its
  own id and gets its own picture. Called on demand, it always returns the whole
  picture. A carrier with no history gets the whole picture every time.
- `maid-core expression | experience | sleep | adaptation` exist, take
  the same session and carrier options, and today return nothing and
  succeed, so a carrier wired early is harmless.
- `maid-core map` prints the harness map (every part's personas and
  bring-ins, fused, with what each declares and what its part recorded);
  `maid-core environment` prints the environment alone.
- The core runs from the installed profile with no checkout and no dev
  shell, under a bare environment (`env -i HOME=…`). Every location it
  reads is a config value with a default under the user's state dir.
- Adding or removing a part's list, or a bring-in in one, changes the
  next `genome` and nothing else.

### What a part does

- A part's list (the existing file under the parts dir) may now name its
  persona and its bring-ins, each with its source and pinned revision,
  its tenure (own, or borrowed from whom and since when), its route, the
  config values bound here, and where its manifest is. A list without
  them is read as before.
- A bring-in declares the contract in its standard's own slot: an Agent
  Skill's frontmatter `metadata`, a plugin manifest's `extensions`, an
  MCP server manifest's `_meta`. The unit stays a plain unit of its
  standard; a shell that does not know the slot ignores it.
- A config value marked secret is never written into a part list; the
  list names where the secret comes from instead. Nothing the core
  prints shows a secret.

### Status

- `just status --whole` adds, per part, each bring-in it records as
  **ready**, **not ready (why)**, **undeclared** (registered, no
  declaration to check readiness against) or **unreadable (why)**. These
  report; they do not change the exit code. Everything it reported
  before is reported and decided as before.
- `just install` records mAId's persona (`personal`) and its bring-ins
  in mAId's list: each skill (own, this repo, the installed revision),
  the personal memory server (own, its store bound) and the vendor's
  document skills (borrowed from the vendor, report only). `just
  uninstall` removes them with the list.

## Test Strategy

### Unit (`just test`, load-bearing)

Each names the change that would break it, and runs in temp dirs only.

- **Contract readers**, one per slot: a skill's flat `metadata` keys
  read into every field (lists split, config markers read, state keys
  named); a plugin's `extensions` entry and a server manifest's `_meta`
  entry read into the same shape; a unit with no declaration is
  undeclared, not an error; a malformed field makes that bring-in
  unreadable with a reason and leaves the others; unknown keys inside
  the namespace are ignored.
- **Part-list reader**: a format-1 list without the new keys has no
  bring-ins and no persona; tenure, source, route and values read;
  route defaults from the source; a borrowed bring-in is report only
  whatever its list says; a secret value that is not a reference is
  refused for that bring-in and never echoed.
- **Fusion**: two parts give two personas kept apart and each bring-in
  under its own part; the same bring-in name from two parts is kept
  twice and reported as a clash; the map's order does not depend on
  file order.
- **Environment**: each not-ready reason on its own (unbound key,
  missing state location, unset secret reference, command not on a
  PATH handed in, manifest unreadable), and all of them clear gives
  ready.
- **Delivery record**: first send recorded; same content again sends
  nothing; changed content sends again; reset sends again; two threads
  sending the same item at once, exactly one wins; a session id that
  could escape its dir is refused.
- **Entry points**: `genome` per carrier kind as in Requirements;
  through the CLI, `--reset` re-sends the picture and a sub-session id
  gets its own picture; the
  four unfilled ones return nothing; pending counts the pending dir and
  is 0 with none.
- **Part-list writer** (build-tool): persona and bring-ins written,
  sorted; a rerun is byte-identical; harness entries are untouched by a
  bring-in record and the other way round.
- **Whole-status**: the conformance set gains cases for the four
  bring-in states and for a list whose bring-ins are malformed (the
  list is not refused); every existing case passes unchanged.

### Integration (free)

- `cargo test -p maid-core` drives the binary against a temp state dir
  holding mAId's list and a fixture second part: both personas, both
  parts' bring-ins, the environment, `--json` parsing to the same
  picture, the second call with the same session sending nothing.
- `nix develop -c resources/tests/rollback` gains: after install the
  profile has `bin/maid-core`; `env -i HOME=<temp> <profile>/bin/maid-core
  genome` shows the `personal` persona and mAId's skills; the part list
  holds the bring-ins with tenure and revision; uninstall removes them.

### Paid

- None. Nothing here changes what an agent sees in a session; the hooks
  that do are stream 2's.

## Design

**Why this shape.** Hooks and adapters run in a session, from the
installed profile, with no checkout; the build-tool runs only from the
checkout through cargo. So the core is its own crate, packaged into the
profile, and the build-tool uses it as a library for status and install.
One reader of the contract and the part lists, used by both, means
status and the session can never disagree about what is ready.
Considered putting the core in the build-tool and shipping that: it
would put the skill test runner and its dependencies in every session's
path for no use.

**Crate.** `resources/core`, package and binary `maid-core`, a workspace
member. Dependencies: `serde`, `serde_json`, `gray_matter` (already in
the workspace, for skill frontmatter), `clap`, `anyhow`; `tempfile` for
tests. No new crate the workspace does not already carry. The flake
builds it with `rustPlatform.buildRustPackage` from the committed
`Cargo.lock` and links `bin/maid-core` into the profile, as it does
`bin/pensieve`. The build-tool takes it as a path dependency.

**Modules** (dependencies run down the list):

- `config.rs`: where things are. `MY_HARNESS_PARTS` (as today) and
  `MY_HARNESS_STATE`, defaulting to
  `${XDG_STATE_HOME:-$HOME/.local/state}/my-harness[/parts]`; the
  delivery record and pending decisions live under the state dir. Read
  from the environment with defaults, so the binary needs no Justfile.
- `contract.rs`: the declaration and its three slot readers, into one
  struct: `scope` (session or turn), `job`, `signals` (outcomes, jobs,
  states, locations, files), `rules`, `package`, `config` (key → about,
  secret, optional), `state` (local key, permanent key, promote: auto,
  propose or off), `requires` (commands). Namespace `my-harness`
  everywhere:
  - Agent Skill `metadata` is a flat string map, so fields are flat keys:
    `my-harness.scope: turn`, `my-harness.job: …`,
    `my-harness.signals.jobs: writing, email` (comma lists),
    `my-harness.config.<key>: "[secret] [optional] about"`,
    `my-harness.state.local: <config key>`,
    `my-harness.state.permanent: <config key>`,
    `my-harness.state.promote: propose`,
    `my-harness.requires: git, python`.
  - Plugin manifest: `extensions["my-harness"]`, the same fields as a
    JSON object (lists as arrays, config as objects).
  - MCP server manifest (`server.json`): `_meta["my-harness"]`, as the
    plugin.
- `parts.rs`: the lenient reader of the new part-list keys, beside
  whole-status's own: top-level `persona` (string) and `bring_ins`, an
  array of
  `{ "name", "unit": { "standard": "skill"|"plugin"|"mcp", "path" },
  "source": { "kind": "project"|"store"|"dir"|"vendor", "at", "rev" },
  "tenure": { "kind": "own" } | { "kind": "borrowed", "from", "since" },
  "route": "write"|"backlog"|"worktree"|"report", "values": { key: string } }`.
  Format stays 1: readers already ignore keys they do not know, so every
  existing reader keeps working. Route defaults from the source
  (project → worktree, store and dir → write, vendor → report); borrowed
  is always report. A secret key's value must be `env:<NAME>`; anything
  else makes that bring-in unreadable and is never printed.
- `map.rs`: fusion. Reads every list, then each bring-in's declaration
  from its unit's slot. The map is personas (part, persona) and bring-ins
  keyed by (part, name), each with its declaration (or undeclared, using
  the standard's own description as job) and its record; clashes listed.
  Derived on every call, never stored: a part that installs later cannot
  call the core to regenerate a stored map, and reading a few small files
  costs less than a stale map.
- `environment.rs`: per bring-in, bound (every non-optional config key
  has a value), reachable (each state location exists; each secret
  reference is set; the manifest reads), ready (bound, reachable, every
  `requires` command on a PATH handed in, using the build-tool's PATH
  rule, moved here). Never reads a secret's value.
- `delivery.rs`: the delivery record, one file per sent item under
  `<state>/delivery/<session>/<item>@<hash>`, created with `create_new`
  so the first sender wins with no lock. Reset removes the session's dir.
  Session ids are taken only as one plain path component; a sub-session
  is just another id, so it starts its own record.
- `entry.rs`: the five entry points as functions of one call shape:
  session id (optional), carrier kind (append, transient, on demand),
  reset (clear the session's record before answering; the CLI's
  `--reset`) and the carrier's input as JSON; each returns items (id, text) the carrier
  formats. Append skips what the record holds and records what it sends;
  on demand returns all and records it; transient returns all and
  records nothing. `genome` builds the picture from the map, the
  environment and the pending count, as one item: any change re-sends
  the whole picture. A carrier that needs less can split it into one
  item per section later, with no change to the record. The other four
  return no items.
- `main.rs`: the clap CLI over `entry.rs` and `map.rs`, text or `--json`.

**Build-tool changes.** `part.rs` writes `persona` and `bring_ins` in
mAId's list through the same atomic rewrite, keeping harness entries as
they are. Install records mAId's bring-ins after its other steps: each
shipped skill (unit at its path under the profile's skills, source
project at this repo's `origin` URL and `HEAD` revision, own), the
personal memory server (own, package `pensieve`, `values.store` the
store path; no unit until stream 4 gives it a manifest), and each
`BRING_INS` plugin (borrowed from the vendor since install, source
vendor at its pinned commit, report). `whole.rs` prints each part's
bring-ins from `environment.rs` after its entries. project.md gains
reader rule 15 (bring-in states, report-only) beside the others, the
new list keys, and the contract slots, all generic.

## Implementation Plan

- [x] Crate `resources/core`: `config`, `contract` (three slots), `parts`,
  `map`, `environment`, with unit tests; the PATH rule moved in from
  `shared.rs`.
- [x] `delivery` and `entry` (genome filled, four answering empty), the
  CLI, and its binary tests with a fixture second part.
- [x] Build-tool: `part.rs` writes persona and bring-ins; install records
  mAId's; uninstall removes them; unit tests.
- [x] Whole-status prints bring-in states; conformance cases; reader
  rule 15 and the list keys in project.md.
- [x] Flake ships `bin/maid-core`; the rollback test checks it under
  `env -i` and the recorded bring-ins; Layout and Tech Stack lines.

- *Risk note:* the flake now builds Rust. The vendored dependency hashes
  come from `Cargo.lock`, so a lock change needs no hash edit, but the
  first build is slower.
- *Risk note:* the vendor plugin's skills have no manifest path the core
  can read without knowing each shell's cache layout, so they show as
  undeclared until stream 5 decides how a borrowed unit gets its
  declaration.
- *Risk note:* a delivery dir per session grows with sessions; pruning
  is left until it matters.

## Session Log

- 2026-10-30 · grounded on `part.rs`, `whole.rs`, `shared.rs`, `memory.rs`,
  `main.rs`, the flake and the Justfiles. The part list has no persona or
  bring-in keys yet; readers ignore unknown keys (reader rule 3), so the
  new keys keep format 1. No skill carries `metadata` yet. The core
  cannot run from the build-tool, which is never installed. Probed with
  `claude plugin validate --strict` in a temp dir: plugin.json
  `extensions` is "Unknown field … Claude Code ignores it at load time";
  a skill with `metadata` flat keys passes.
- 2026-10-31 · dev: crate `maid-core` (config, contract, parts, map,
  environment, delivery, entry, CLI) with 35 unit and 6 binary tests;
  `find_on` and its tests moved in from `shared.rs`, which re-exports it.
  Broke the delivery cleanup and the append rule on purpose: 3 tests
  failed, then restored. clippy wanted the declaration boxed in the two
  slot enums.
- 2026-10-31 · rebased onto `initiative/harness-core` (pensieve pinned to
  its published commit); slices 3-5 done; `just ci` and
  `resources/tests/rollback` green.
- 2026-10-31 · Code Review Gate, round 1 (panel: correctness FAIL,
  security and comment-hygiene PASS WITH NOTES). Must fix: status listed
  the bring-ins of lists it refused, since the core never checked
  `harnesses`. Fixed with the rest: the core refuses what rule 3 refuses
  and status filters by its own refusals; one persona from two parts is
  still the acting one; `personal-memory` is recorded only while placed;
  the revision comes from the profile; a secret held as a value is hidden
  in the map; a remote's user or token is dropped and local remotes
  (`~`, bare names) left out; manifests must be small regular files; the
  session id is checked for every carrier; the Justfile derives the parts
  dir from `MY_HARNESS_STATE`; conformance gains `env` and two cases.
- 2026-10-31 · Code Review Gate, round 2: all three lenses PASS WITH
  NOTES, round 1 resolved. Took the cheap notes: skills and the vendor
  plugin are bring-ins only while some harness has them placed; a
  command name with `/` is never found on PATH; the delivery hash covers
  the item's data too; a null bring-in field is absent; a token with a
  `/` is still stripped; `git` is found by the PATH rule and tests never
  run it; comment fixes.
- 2026-10-31 · the review briefing sent back two defects, fixed: an
  uninstall of one harness now keeps only the bring-ins another harness
  still has (so the list goes with its last one), and a bring-in with
  memory whose state key is unbound says so in the picture.
- 2026-10-31 · review RETURN (implementation): dropped `share/maid/rev`;
  install reads `HEAD` and a dirty marker from the checkout. The rollback
  test now checks a spec-only commit builds the same profile and that the
  recorded revision is this checkout's, dirty or not. Added the genome's
  refused and clashes lines. Delta review (three lenses): PASS WITH
  NOTES; took `--no-optional-locks`, untracked files not dirty (they are
  not built), and the rollback revision documented. Rollback test green
  clean and with a tracked edit (records `-dirty`).
- 2026-10-31 · closure. No unticked plan items. Backlog: this feature
  closes none of the existing items; filed
  `core-delivery-record-upkeep` (pruning old session dirs, refusing a
  planted symlink). project.md: Architecture gains the content-addressed
  profile rule; Testing names the core's tests and the rollback test's
  new checks; Tech Stack, Layout and Deployment were updated in dev.

## Decision Log

Calls made for the user from the initiative spec, all accepted by the
ringmaster at the Planning Review Gate (2026-10-30); the derived map and
the `my-harness` namespace are also initiative rulings.

- Ruling: the core is its own crate, `maid-core`, shipped in the profile;
  the build-tool uses it as a library · why: hooks run checkout-free and
  the build-tool is never installed · cost if wrong: merge the crates
  later, same modules.
- Ruling: the part list is extended in format 1 with `persona` and
  `bring_ins` · why: extend the existing format; every reader already
  ignores unknown keys · cost if wrong: a format 2 later, with a reader
  for both.
- Ruling: a bring-in record points at its unit's manifest and the
  declaration is read from the slot, never copied into the list · why:
  the declaration travels with the bring-in · cost if wrong: an
  unreachable manifest shows as unreadable, not stale.
- Ruling: the harness map is derived on every call, not stored at install
  · why: a part installing later cannot regenerate a stored map without
  depending on the core · cost if wrong: add a write step; the shape is
  the same.
- Ruling: slot keys are `my-harness.<field>` flat keys in skill
  `metadata`, `extensions["my-harness"]` in a plugin manifest,
  `_meta["my-harness"]` in an MCP `server.json` · why: one namespace,
  each standard's own slot · cost if wrong: renaming keys before any
  third-party unit carries them.
- Ruling: a secret's value in a part list is only `env:<NAME>` · why:
  part lists are plain files; the contract marks secrets · cost if wrong:
  another reference kind added beside it.
- Ruling: append carriers skip what the session's delivery record holds;
  on demand always returns all and records it; transient always sends ·
  why: Experience 1 says the on-demand resource returns the same picture
  · cost if wrong: one rule in `entry.rs`.
- Ruling: bring-in states in whole-status report and never change the
  exit code · why: a missing optional tool (say, a document converter)
  should not fail status forever · cost if wrong: one reader rule.
- Ruling: `expression`, `experience`, `sleep`, `adaptation` exist and
  return nothing · why: carrier-neutral shape now, filled by streams 2
  and 3 · cost if wrong: none.
- Ruling: mAId's persona is `personal`; install records its skills, its
  memory server and the vendor document skills (borrowed); the browser
  server waits for stream 5 · why: the bring-ins the build-tool itself
  installs · cost if wrong: one more record call.
- Ruling: route defaults: project → worktree, store and dir → write,
  vendor → report; borrowed is always report · why: the initiative's
  route table and guards · cost if wrong: one table.
- Ruling (ringmaster): reset is part of the call shape (`--reset`), so
  a SessionStart on compact, clear or resume re-sends the picture, and a
  sub-session id starts its own record · why: stream 2's hooks need it ·
  cost if wrong: none.
- Ruling (ringmaster): the picture is one delivered item · why: least
  for now · cost if wrong: split into per-section items in stream 2.
- Decision (review round 1): the core refuses exactly what reader rule 3
  refuses, so a session never sees a bring-in status refuses · considered
  filtering only in status; the session picture would still disagree.
- Ruling (ringmaster, review return): install records the checkout's
  `HEAD`, with `-dirty` when the tree has changes, not a revision written
  into the profile · why: the profile stays content-addressed, so a
  docs-only commit makes no new generation · cost if wrong: after a
  rollback the list names the checkout's revision, not the generation's.
- Ruling (ringmaster): the genome names refused part lists and name
  clashes when there are any, so a session acting on part of the ghost
  can tell.
- Ruling: no paid tests in this stream · why: nothing here reaches a
  session's context.
