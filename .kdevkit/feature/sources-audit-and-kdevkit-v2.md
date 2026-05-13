# Feature: sources-audit-and-kdevkit-v2

## Git Setup

- Branch: `refactor/sources-audit-and-kdevkit-v2`
- Base: `main` @ `ed48215`

## Feature Brief

Prune `sources/skills/` down to what's genuinely unique to me, and
reshape the kdevkit methodology so it stops assuming `.kdevkit/` —
the broader convention is a `specs/` (or `docs/specs/`) tree at
the repo root. Split the methodology into three distinct surfaces:
project invariants (maintained), per-feature specs (lifecycle),
and a future-work backlog. Dogfood the result by migrating this
project's own spec tree.

## Requirements

### Scope of this cycle

- **Audit `sources/skills/`.** Classify each skill as
  unique-to-me or harness-default. Keep the unique ones, remove
  the duplicates.
  - `development/` — remove (duplicates Claude Code harness
    defaults).
  - `git/` — remove as a standalone skill; migrate the two
    genuinely non-default rules into kdevkit's git section:
    _squash-merge preferred_ and _no `Co-Authored-By` trailer_.
  - `kdevkit/` — keep and refactor (see below).
  - `writing-style/` — keep as-is.

- **Clean duplicated rules out of the tool-specific entrypoints.**
  `sources/claude/CLAUDE.md` and `sources/kiro/KIRO.md` each carry
  a verbatim "kdevkit Standing Rules" block that restates the
  skill. Remove the blocks; the skill is the source of truth.

- **Restructure the kdevkit methodology** into three distinct
  parts with clear lifecycles:
  1. **Project invariants** (maintained, change rarely): one
     `project.md` with a fixed section order — Mission,
     Architecture, Tech Stack, Layout, Testing, Deployment.
  2. **Feature specs** (lifecycle: draft → in-flight → done):
     one file per feature with Requirements, Design, Test
     Strategy, Implementation Plan, plus Session/Decision logs.
  3. **Backlog** (open list of wanted future work): per-item
     files, not a single list doc.

- **Make kdevkit tree-location-agnostic.** The skill auto-detects
  in order: `specs/`, then `docs/specs/`, then `.kdevkit/`. First
  match wins. The word "kdevkit" remains the moniker for the
  methodology; the directory name is no longer load-bearing.

- **Scaffold a notes/knowledge skill stub.** Create
  `sources/skills/notes/SKILL.md` as a one-paragraph placeholder
  describing intent (read/update an Obsidian-like store). Real
  design is deferred — capture the design questions in a backlog
  entry.

- **Dogfood.** Migrate this project's `.kdevkit/` tree to
  `specs/`. Convert `project.md` to the new fixed-section shape.
  Move `feature/` → `specs/feature/`, `feature-wip/` →
  `specs/backlog/` (rename reflects new terminology).

### Out of scope

- Designing the notes-skill storage layer.
- Adding new registry entries beyond the notes-skill scaffold.
- Touching `maid/` CLI code — this is a sources-and-methodology
  change, not a tool change. Registry remains unchanged since the
  notes skill lives inside `sources/skills/` which is already
  managed as a directory.

### Acceptance

- `deno task test` passes.
- `./tests/functional/run` passes.
- `deno task status` shows no drift in managed symlinks.
- The kdevkit skill, read cold by a fresh session in a repo with
  only `specs/`, successfully picks up project context and feature
  files without mentioning `.kdevkit`.
- This repo's own spec tree is under `specs/` after the change.

## Design

### Tree shape and auto-detect

Spec tree lives under one of three roots, resolved in order:

```
specs/          ← preferred, created fresh when none exists
docs/specs/
.kdevkit/       ← legacy; kept only to avoid forcing migration
```

kdevkit's detection rule: at session start, look for the three in
order. First hit wins and becomes `$SPEC_ROOT` for the session. If
none exists and the user begins feature work, create `specs/`.
Never auto-migrate an existing `.kdevkit/` — that's a human
`git mv` decision because it touches CI and `.gitignore`-type
wiring.

Layout under `$SPEC_ROOT`:

```
$SPEC_ROOT/
├── project.md       maintained invariants (rarely changes)
├── feature/         in-flight + completed feature specs
└── backlog/         per-item files for wanted future work
```

Rename `feature-wip/` → `backlog/`. Terminology shift: "backlog"
reads as a list of wanted things; "feature-wip" read as
paused-mid-flight, which isn't what lives there.

### `project.md` — fixed section template

Six sections, fixed order. Each has a one-line intent prompt that
guides what to write; the prompt stays as a comment so a future
session can re-read it.

```markdown
# Project: <name>

## Mission

<!-- What this project exists to do, and who it serves. One
     paragraph. Change only when the goal itself changes. -->

## Architecture

<!-- The logical shape: components, responsibilities, how they
     talk. Diagrams optional; words mandatory. Not a file-layout
     listing — see Layout for that. -->

## Tech Stack

<!-- Languages, runtimes, frameworks, key libraries. Versions
     only where version matters. -->

## Layout

<!-- Directory tree with a one-line annotation per entry. What
     lives where, not why. -->

## Testing

<!-- How this project is tested: unit, integration, smoke,
     manual. Which commands run which suite. Which are
     load-bearing vs. nice-to-have. -->

## Deployment

<!-- How code reaches users. Build, release, install, symlink,
     container, whatever applies. If the project isn't deployed
     in a traditional sense, describe how it's consumed. -->
```

### Feature file template

Unchanged from today's kdevkit skill — the shape already works.
Lives at `$SPEC_ROOT/feature/<feature-name>.md`.

### Backlog item template

Per-item files at `$SPEC_ROOT/backlog/<item-name>.md`. Minimal
shape:

```markdown
# Backlog: <item-name>

## What

<!-- One paragraph: what this is, not how. -->

## Why

<!-- Motivation — what prompted the idea. Links to the
     conversation/incident if applicable. -->

## Open questions

<!-- Things that would need to be decided before this becomes a
     feature spec. Blockers, dependencies, unknowns. -->
```

Promoting to a feature spec is `git mv $SPEC_ROOT/backlog/<name>.md
$SPEC_ROOT/feature/<name>.md`, then filling in Requirements /
Design / etc. around the existing What/Why.

### kdevkit skill — restructuring

Current skill is 193 lines; post-refactor it stays self-contained
but reorganizes around the three surfaces:

1. **§ Load project context** — look for `$SPEC_ROOT/project.md`
   via the three-way auto-detect. Replace all `.kdevkit/`
   references with `$SPEC_ROOT/`.
2. **§ Load feature context** — `$SPEC_ROOT/feature/<name>.md`;
   on miss, check `$SPEC_ROOT/backlog/<name>.md` (not
   `feature-wip/`).
3. **§ Backlog** — new short section: when the user describes
   something they want but not now, write
   `$SPEC_ROOT/backlog/<name>.md` with the three-section template.
4. **§ Git practices** — add two lines: _squash merge preferred_
   and _no `Co-Authored-By` trailer on commits_. Rest of git
   rules trimmed to what's genuinely beyond harness default.
5. **§ Session behaviour, Quality → Test → Push** — unchanged.
6. Remove §6 ("Repo-specific toolchains") — absorbed into the
   Deployment section of `project.md`.

### Skill deletions

- Delete `sources/skills/development/` — content is
  harness-default.
- Delete `sources/skills/git/` — two non-default rules migrate
  into kdevkit.

### Notes skill stub

New directory `sources/skills/notes/SKILL.md`. Single-file stub.
Status: **not yet implemented** — marker so no session mistakes
it for ready-to-use. The stub captures intent so a later design
phase has a starting point:

```markdown
---
name: notes
description: (WIP — not yet implemented) Capture and connect
  notes in a personal knowledge store (Obsidian-like).
version: 0.0.1
status: stub
tags: [notes, knowledge, obsidian]
---

# notes — personal knowledge skill (stub)

**Status: not implemented.** This file reserves the slot and
records intent; the behaviour below is a design target, not a
contract. Do not try to "use" this skill yet — if the user asks
for note-capture today, tell them it's a pending design.

## Intended scope

- **Reminders / to-dos.** "Remember this, I want to work on it
  later." Stored as a dated item in the notes store.
- **Insights.** A thought worth keeping, with links to related
  thoughts on the same topic. A single insight can thread into
  multiple topics.
- **1:1 and interview notes.** Structured captures of
  conversations — who, when, what came up.
- **Conversation uploads.** Audio or transcript of a longer
  conversation, paired with a pre-amble the user provides; both
  land in the store as a referenceable artifact.

## Deferred design decisions

See `$SPEC_ROOT/backlog/notes-skill-design.md` for the open
questions: store format (Obsidian vault? flat markdown?), link
conventions, read/write/search API, how conversation audio is
handled.
```

### Backlog entries created this cycle

- `$SPEC_ROOT/backlog/notes-skill-design.md` — design questions
  for the notes skill (store format, link conventions, API).

### Entrypoint files

`sources/claude/CLAUDE.md` and `sources/kiro/KIRO.md` keep only
the session-start routing block. The "kdevkit Standing Rules"
block in each is removed — the skill is the source of truth. This
is consistent with the principle: every coding agent has a
different structure; sources adapt to each. The entrypoint is
routing.

### Dogfood migration

In the same branch:

1. `git mv .kdevkit specs`
2. `git mv specs/feature-wip specs/backlog`
3. Rewrite `specs/project.md` to the six-section template.
4. Convert the three existing items in `specs/backlog/` (was
   `feature-wip/`) to the What/Why/Open-questions shape where
   they don't already fit. Light touch — preserve content.
5. Update `maid/.kdevkit/project.md` references across the repo:
   - `mAId/README.md` if it mentions `.kdevkit/`
   - `sources/skills/kdevkit/SKILL.md` (the skill itself —
     updated as part of the skill refactor above)

### Registry change: expose skills to Kiro

Add one entry to `maid/registry.ts`:

```ts
{
  home_subpath: ".kiro/steering/skills",
  source_subpath: "sources/skills",
  kind: "dir",
}
```

Rationale: the project's spirit is "adapt sources to whatever
each coding agent expects." Today Kiro only sees
`~/.kiro/steering/KIRO.md`; skills aren't reachable from a Kiro
session. Symlinking `sources/skills/` into Kiro's steering tree
closes that gap and lets the Kiro smoke tests actually exercise
skill loading.

Adds a single entry; no deploy logic changes.

Adding `notes/` or removing `development/` and `git/` inside
`sources/skills/` needs no further registry work — the directory
is managed as a whole.

## Test Strategy

Goal: **when a user launches Claude or Kiro in this repo, the
skills we kept are actually loaded and applied.** Nothing more
elaborate. The harness already exists at `tests/functional/run`;
this feature extends it rather than building a new one.

### Layers

1. **Structural (unchanged)** — symlinks resolve, `SKILL.md` files
   are reachable through `~/.claude/skills/<name>/`. These are
   cheap and run always, including with `--no-tools`.

2. **Claude smokes (extend existing)** — one `.smoke` fixture per
   kept skill. Each fixture runs `claude --print "<prompt>"` and
   greps for a marker only the loaded skill would produce.

3. **Kiro smokes (new)** — same shape, invoked via
   `kiro-cli chat --no-interactive "<prompt>"`. Same fixture file
   drives both: one prompt, two tool invocations, each asserted
   independently.

### Fixture shape

Same `.smoke` format as today, one new optional field:

```
prompt: <one-line prompt that forces the skill to announce itself>
expect_substr: <substring the response must contain>
tools: claude,kiro          # optional; defaults to claude only
```

A fixture with `tools: claude,kiro` runs both CLIs and both must
pass. `tools: claude` (or omitted) keeps today's behaviour.

### Prompt convention

Each prompt forces a literal marker so the check is unambiguous
and doesn't depend on model phrasing:

> _"Load the `<skill>` skill from ~/.claude/skills/\<skill\>/SKILL.md
> (or ~/.kiro/steering/ for Kiro). Begin your response with the
> literal line `[<skill>] applies` on its own line, then answer:
> \<one short question only the skill knows\>."_

The grep asserts `[<skill>] applies`. If that line is present,
the skill was loaded and the session is behaving per its rules.

### Fixtures this cycle

- `writing-style.smoke` — keep. Existing prompt is fine.
  `tools: claude,kiro`.
- `kdevkit.smoke` — **new**. Prompt asks: _"what directory does
  kdevkit look for first for the spec tree?"_. Expect
  `[kdevkit] applies` and an auto-detect answer mentioning
  `specs`. `tools: claude,kiro`.
- `notes.smoke` — **new**. Prompt asks the skill to describe its
  status. Expect `[notes]` **and** the word `stub` or `not
  implemented` — proves the stub's "do not use yet" marker lands.
  `tools: claude,kiro`.
- `development.smoke` — **delete** (skill goes).
- `git.smoke` — **delete** (skill goes).

### Harness changes

`tests/functional/run` gains:

- An `assert_kiro_response` helper mirroring `assert_claude_response`
  but shelling `kiro-cli chat --no-interactive`.
- `skill_smokes()` reads the optional `tools:` field. For each
  tool in the list, it runs the appropriate assertion and skips
  (not fails) if the CLI isn't on `PATH`.
- `--no-tools` still skips both.

### Kiro invocation notes

- `kiro-cli chat --no-interactive "<prompt>"` is the one-shot
  mode. If the CLI needs `--trust-tools=` (no tool use) for a
  clean non-interactive run, the helper sets it. Validate during
  implementation — first pass uses defaults.
- Kiro needs skills exposed via `~/.kiro/steering/skills/`. The
  Design section has been updated to add that registry entry.
  Prompts for Kiro fixtures reference that path instead of
  `~/.claude/skills/`.

### How to run

```
./tests/functional/run                   # all, both tools
./tests/functional/run --no-tools        # structural only
./tests/functional/run kdevkit           # one fixture, both tools
```

### Acceptance

- `./tests/functional/run` exits 0 with every kept skill passing
  the Claude smoke **and** the Kiro smoke.
- A deliberate break (e.g., rename `kdevkit/SKILL.md` to
  `KDEVKIT.md`) causes the `kdevkit.smoke` to fail on both tools
  — confirms the test actually exercises loading, not just
  filesystem presence.

## Implementation Plan

Five commits, each leaving the repo in a working state. Feature
file itself stays at `.kdevkit/feature/` until commit E, then
moves with the dogfood migration — so every earlier commit can
still find its own spec.

### Commit A — `refactor(skills): reshape kdevkit, remove dev/git, stub notes`

1. Rewrite `sources/skills/kdevkit/SKILL.md`:
   - §1 Load project context → `$SPEC_ROOT` auto-detect (`specs/`
     → `docs/specs/` → `.kdevkit/`); create `specs/` fresh when
     none exists; never auto-migrate legacy `.kdevkit/`.
   - §2 Load feature context → `$SPEC_ROOT/feature/<name>.md`;
     backlog fallback at `$SPEC_ROOT/backlog/<name>.md`.
   - New §3 Backlog → per-item file template (What / Why /
     Open questions).
   - §4 Git practices → add _squash merge preferred_ and _no
     `Co-Authored-By` trailer_; drop what's already harness
     default.
   - Remove §6 (Repo-specific toolchains) — absorbed into
     Deployment section of `project.md`.
   - Update `project.md` template block to the six-section fixed
     order with prompt comments.
2. Delete `sources/skills/development/SKILL.md` (and directory).
3. Delete `sources/skills/git/SKILL.md` (and directory).
4. Delete `tests/functional/skills/development.smoke` and
   `tests/functional/skills/git.smoke`.
5. Create `sources/skills/notes/SKILL.md` as the `status: stub`
   placeholder from Design.
6. `deno task fmt && deno task lint && deno task check`.
7. `deno task test` — Deno tests are structural; should still
   pass.
8. `./tests/functional/run --no-tools` — structural smokes only,
   verify no stale fixtures.

### Commit B — `feat(maid): expose sources/skills to Kiro`

1. Add one entry to `maid/registry.ts`:
   ```ts
   { home_subpath: ".kiro/steering/skills",
     source_subpath: "sources/skills", kind: "dir" },
   ```
2. `deno task test` — `deploy_test.ts` should cover the new
   entry via its table-driven assertions; add a case if needed.
3. `deno task deploy` (against real `$HOME`) — new symlink
   appears at `~/.kiro/steering/skills`.
4. `deno task status` — confirm no drift.

### Commit C — `refactor(sources): strip duplicated standing-rules from entrypoints`

1. Edit `sources/claude/CLAUDE.md` — remove the
   `<!-- kdevkit:standing-rules -->` block; keep only the
   session-start routing block at the top.
2. Edit `sources/kiro/KIRO.md` — already only has routing; add
   one line noting `~/.kiro/steering/skills/` now exists.
3. No test changes.

### Commit D — `test(functional): kiro smoke support via kiro-cli`

1. Extend `tests/functional/run`:
   - Add `assert_kiro_response` helper using
     `kiro-cli chat --no-interactive "<prompt>"`.
   - Parse optional `tools:` field from `.smoke` fixtures
     (default `claude`).
   - `skill_smokes()` iterates the tool list; missing CLI →
     skip, not fail.
2. Rewrite `tests/functional/skills/writing-style.smoke` —
   add `tools: claude,kiro`; prompt loads the skill from either
   `~/.claude/skills/` or `~/.kiro/steering/skills/`.
3. Add `tests/functional/skills/kdevkit.smoke` —
   `tools: claude,kiro`; prompt asks what directory kdevkit
   looks for first; expect `[kdevkit] applies` and `specs`.
4. Add `tests/functional/skills/notes.smoke` —
   `tools: claude,kiro`; prompt asks the skill to describe its
   status; expect `[notes]` and `stub` (or `not implemented`).
5. `./tests/functional/run` — full run (structural + Claude +
   Kiro). Every fixture passes on both tools.
6. Deliberate-break check: temporarily rename
   `sources/skills/kdevkit/SKILL.md` → `KDEVKIT.md`; rerun
   `./tests/functional/run kdevkit` — both tool smokes fail.
   Revert the rename. Not committed; sanity check only.

### Commit E — `refactor(specs): migrate .kdevkit → specs/, feature-wip → backlog`

1. `git mv .kdevkit specs`.
2. `git mv specs/feature-wip specs/backlog`.
3. Rewrite `specs/project.md` to the six-section template —
   Mission, Architecture, Tech Stack, Layout, Testing,
   Deployment. Port content from today's `project.md` into the
   right sections. No content loss; reshape only.
4. Create `specs/backlog/notes-skill-design.md` capturing the
   four open-question areas (store format, link conventions,
   read/write/search API, conversation audio handling).
5. Light-touch update of existing items in `specs/backlog/` —
   ensure each matches the What / Why / Open questions shape.
   Preserve substantive content; only re-heading where needed.
6. Grep for stale `.kdevkit/` references across the repo:
   - `mAId/README.md` if it exists.
   - `sources/skills/kdevkit/SKILL.md` (already rewritten in
     Commit A — sanity-verify).
   - Any test fixtures.
7. `deno task test && ./tests/functional/run` — full green.

### Post-implementation (not a commit)

1. Update memory at
   `~/.claude/projects/.../memory/feedback_feature_wip_backlog.md`:
   rename and rewrite for `specs/backlog/` path; update the line
   in `MEMORY.md`.
2. Offer: _"Shall I update `specs/project.md` with what
   changed?"_ — per kdevkit feature-completion hook.
3. Open PR — title `refactor(kdevkit): spec tree v2 + skill
   audit`. Body: why (tree-location agnostic; drop harness-
   default skills; cross-tool skill exposure) + the five-commit
   summary.

### Risks

- **R1 · Stale symlinks after skill deletion.** Removing
  `sources/skills/development/` may leave a dangling
  `~/.claude/skills/development/`. Mitigation: `deno task
  deploy` is supposed to reconcile; verify with `deno task
  status` after Commit A. If drift, `deno task undeploy` →
  `deno task deploy`.

- **R2 · `kiro-cli chat --no-interactive` may need extra flags.**
  First pass uses defaults. If the non-interactive run hangs or
  fails because Kiro tries to call tools, set
  `--trust-tools=` (empty list = trust none = no tool calls).
  Fallback: `--agent default --trust-tools= --no-interactive`.

- **R3 · Kiro steering directory ignores nested `skills/`.** If
  Kiro's session-start routing doesn't walk
  `~/.kiro/steering/skills/`, the registry entry exposes files
  but doesn't wire them into sessions. Mitigation at test time:
  the `kdevkit.smoke` Kiro run will simply fail; investigate via
  `kiro-cli doctor`. Worst case → the Kiro fixtures reference
  the skill by absolute path (`cat ~/.kiro/steering/skills/.../
  SKILL.md` then answer) as a workaround, and a follow-up
  backlog item captures the deeper fix.

- **R4 · Feature file moves during Commit E.** The file I'm
  editing during Commits A–D lives at `.kdevkit/feature/
  sources-audit-and-kdevkit-v2.md`; Commit E `git mv`s it.
  Mitigation: stage all edits to the feature file before the
  `git mv`. Post-mv, further Session/Decision Log entries go
  into `specs/feature/...`.

- **R5 · Deliberate-break check is manual.** Step D-6 is a
  hand-run sanity check, not a permanent test. That's
  intentional — wiring an "expect failure" case into the harness
  costs more than it's worth. Ack the risk and move on.

### Estimate

Small, ~2 hours end to end — the logic is rename/reshape plus
one helper function in bash. No production code touched. Main
unknowns are Kiro CLI behavior (R2, R3); budget an extra hour
for debugging if they surface.

## Session Log

- **2026-05-13** — Implementation plan drafted. Five commits in
  order: (A) skill reshape + dev/git delete + notes stub; (B)
  registry entry exposing skills to Kiro; (C) strip duplicated
  standing-rules from entrypoints; (D) Kiro smoke-test support +
  new fixtures; (E) dogfood migration `.kdevkit/` → `specs/`,
  `feature-wip/` → `backlog/`. Feature file itself moves in
  Commit E — all earlier commits operate on the `.kdevkit/` path.
  Five risks logged, largest being Kiro non-interactive behavior
  (R2/R3). Stopping at Implementation Plan → execution phase
  gate.
- **2026-05-13** — Kiro skill-exposure gap resolved: Design updated
  to add one `registry.ts` entry symlinking `sources/skills/` →
  `~/.kiro/steering/skills/`. Matches project spirit ("adapt
  sources to what each coding agent expects"). Stopping at phase
  gate before Implementation.
- **2026-05-13** — Test strategy drafted. Three-layer approach:
  structural (existing) · Claude smokes (extend today's harness) ·
  Kiro smokes (new, via `kiro-cli chat --no-interactive`). One
  `.smoke` fixture per skill, gains an optional `tools:` field so
  a single fixture drives both CLIs. Kept fixtures:
  `writing-style`, `kdevkit` (new), `notes` (new). Deleted:
  `development`, `git`. **Design gap surfaced:** Kiro reads only
  `~/.kiro/steering/KIRO.md` today — skills aren't exposed cross-
  tool. Flagged for implementation-time decision, lean toward
  adding a registry entry (overturns the "no registry changes"
  line in Design). Stopping at Test Strategy → Implementation
  phase gate.
- **2026-05-13** — Design drafted. Open questions from Requirements
  phase resolved: auto-detect creates `specs/` when none exists
  (never auto-migrates legacy `.kdevkit/`); `feature-wip/` →
  `backlog/` with What/Why/Open-questions template; `project.md`
  uses six fixed sections with prompt comments; notes skill ships
  as a marked stub with `status: stub` frontmatter plus a backlog
  entry for the deferred design; entrypoint files keep only
  session-start routing. Stopping at Design → Test Strategy phase
  gate.
- **2026-05-13** — Feature file created. Pre-work audit completed:
  `development/` and `git/` skills are ~90% harness-default;
  `sources/claude/CLAUDE.md` and `sources/kiro/KIRO.md` carry a
  verbatim standing-rules block that duplicates the kdevkit skill.
  Four upfront decisions settled with the user: (i) auto-detect
  `specs/` > `docs/specs/` > `.kdevkit/`; (ii) single `project.md`
  with fixed section order; (iii) keep kdevkit, delete
  development/git; (iv) notes skill is scaffold-only this cycle.
  Branch `refactor/sources-audit-and-kdevkit-v2` cut from `main`.
  Stopping at Requirements → Design phase gate.

## Decision Log

- **2026-05-13 · Auto-detect spec root rather than forcing a
  rename across repos.** Rationale: kdevkit is meant to travel
  between repos; forcing `specs/` everywhere would break
  in-flight projects that already use `.kdevkit/`. Alternatives
  rejected: (a) hard-rename to `specs/` — too disruptive;
  (b) keep `.kdevkit/` — fights the `specs/` convention other
  tools assume.

- **2026-05-13 · Single `project.md` with fixed section order,
  not a file-per-section tree.** Rationale: lower ceremony, one
  file to open to orient in a new repo. Trade-off accepted:
  less cross-repo grep leverage over individual section files.

- **2026-05-13 · Delete `development/` and `git/` skills entirely;
  fold the two genuinely non-default git rules into kdevkit.**
  Rationale: the vast majority of both skills duplicates the Claude
  Code harness default prompt; keeping them creates the false
  impression they carry unique guidance. Alternatives rejected:
  (a) heavy-trim each to the unique bits — still fragments
  guidance across three skills; (b) merge all three into one —
  conflates methodology (kdevkit) with coding discipline
  (development) unnecessarily.

- **2026-05-13 · Notes skill is scaffold + backlog entry only
  this cycle.** Rationale: storage design (Obsidian vault format,
  link conventions, read/write/search API) is a separate design
  phase and blocks on decisions about the store itself. Landing a
  stub now reserves the slot without locking in a design.
