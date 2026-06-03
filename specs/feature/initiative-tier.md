# Feature: initiative-tier

## Git Setup

- Branch: `feat/initiative-tier`
- Base: `main` at `54a458e` (post-A merge)

## Feature Brief

Add **initiative** as a fourth tier in the kdevkit skill, slotted
between project (timeless invariants) and feature (one branch's
contract). An initiative captures multi-feature work that can't
fit on one branch — the *why* plus the ordered *streams* (each
stream = one feature = one branch / CR / squash-merge) that
deliver it, plus a Status table updated by each stream's
closure commit. Initiatives carry the persistent "you're in
stream 2 of 3" context that today lives only in chat and
disappears after compaction.

The skill describes *how* to work across the project / initiative
/ feature / dev tiers. *Where* it works — single repo, multi-repo,
monorepo, cross-repo program — is orthogonal and explicitly out
of scope for the tier definitions; the prose stays repo-shape
agnostic.

## Requirements

### New tier

- **`specs/initiative/<name>.md`** — one file per initiative.
  Sections: Why, Streams, Decisions taken at the initiative
  level, Status (table). Format finalized in the §Design
  template.
- **§1 Locate the spec tree** auto-detects `initiative/` as a
  recognized subdirectory alongside `feature/` and `backlog/`.
  Detection cue: `$SPEC_ROOT/initiative/` exists.
- **`project.md` gains an optional `## Active initiatives`
  one-line index** — like the backlog index but for in-flight
  initiatives. Removed when the initiative ships.

### Session-start read order

- Read sequence: `project.md` → active-initiatives index →
  current initiative (if the entry cue references one or the
  current feature is auto-linked to one) → feature(s) for the
  current branch.
- The index lets the agent skip loading every initiative file
  unconditionally; only the one(s) referenced by the entry
  cue or the current feature load.

### New verbs

- **"start initiative `<name>`"** — write
  `specs/initiative/<name>.md` from the template; populate
  `## Active initiatives` index in `project.md`; commit as
  `plan(<initiative>): initial spec`.
- **"show initiatives"** — list active initiatives from the
  index.
- **"stream `<n>` for `<initiative>`"** — start a feature
  whose Git Setup names the initiative as its parent. Auto-
  populates the feature spec's `Part of initiative: [[X]]`
  link. Otherwise behaves as a normal feature start (§3).

### Auto-link feature → initiative

- When §6 Planning runs for a feature whose spec sits under an
  active initiative (or the entry cue identifies it as a
  stream of one), planning automatically adds a
  `Part of initiative: [[<name>]]` line near the top of the
  feature spec's body (after `## Feature Brief`). Low-friction;
  no prompt.
- Heuristic: a feature is "under an active initiative" if the
  initiative's Streams list names the feature's branch or
  feature-spec basename.

### Closure updates Status table

- §8 Closure gains one more step (between the spec-reconcile
  and the close-commit): if the closing feature is a stream of
  an active initiative, update the initiative's Status table
  row — branch, CR, status (`shipped`), ship date, one-line
  learning. Stage as part of the same `close(<feature>):` commit
  that handles spec reconcile + project.md verify + backlog
  cleanup.
- If this is the **last** stream (all other rows in the Status
  table are `shipped`), the same commit also archives the
  initiative spec — `git rm specs/initiative/<name>.md` and
  remove the row from `project.md`'s `## Active initiatives`
  index. No separate `close(<initiative>):` commit; the last
  stream's `close(<feature>):` does the work.

### Cross-stream rebase semantics

- Streams are sequential by design. When Stream `n+1` is
  in-flight and Stream `n`'s CR receives review changes that
  re-ship to `main`:
  1. Stream `n+1` rebases onto the new `main` HEAD.
  2. Resolve conflicts; re-run §7 Quality + Test + Code
     Review Gates for the slice that intersects the rebased
     change.
  3. Force-push the rebased branch (only after §7 reverifies).
- Rationale: streams ARE sequential. Stream `n+1`'s diff is
  meaningful only against `n`'s shipped state; staying on the
  pre-rebase base means reviewing against a stale mainline.
- This is the only place §9's "new commits, never amends"
  rule yields — rebase is required for the sequential-stream
  contract to hold.

### Conventional Commits

- **`plan(<initiative>):`** — initiative-spec authoring (the
  spec for the initiative itself, not its streams).
  Touches only `specs/initiative/<name>.md` (rarely
  `project.md` to update the Active-initiatives index).
- **`close(<feature>):`** semantics extended — when the closing
  feature is a stream of an active initiative, the closure
  commit also touches the initiative's Status table (and
  potentially archives the spec on last-stream close).
- No new `close(<initiative>):` type — last-stream closure
  reuses `close(<feature>):`.

### Repo-shape orthogonality

- Tier definitions (project / initiative / feature / dev)
  describe *how* to work. *Where* the work lives (single repo,
  multi-repo, monorepo) is orthogonal and not encoded into the
  tier names or template fields.
- A short reference subsection ("Working across repos") in §10
  or §1 sketches how the tiers map onto common repo shapes —
  single-repo (default; everything in `specs/`), multi-repo
  (each repo carries its own `specs/`; cross-repo programs use
  a separate top-level program tree off-skill). This is
  guidance, not contract.

## Test Strategy

- **`test:unit`** — irrelevant. No schema or deploy logic
  touched. Stays green.
- **`test:smoke`** — irrelevant for behavior; symlink-resolution
  check still runs as the regression net.
- **`test:functional` (judge mode)** — *deferred to D
  (kdevkit-compaction)*. Per the planning conversation, D
  carefully plans both the SKILL.md compaction and the
  matching test surface together; bundling B's initiative
  fixtures into D's commit avoids a double test-surface change
  and lets D design the fixture set against the post-B SKILL.md
  shape.
- **Dogfood evidence.** This feature is *not* itself an
  initiative (it's one feature, one branch). The dogfood
  comes when the next multi-stream effort uses the new tier;
  that's a real-case demo, not a test.

The post-D work this feature hands off:

- One judge fixture: `kdevkit-initiative-start.smoke` — "start
  initiative `<name>`" → judge: "does the agent create
  `specs/initiative/<name>.md` from the new template, update
  the project.md index, and commit as `plan(<initiative>):`?"
- One judge fixture: `kdevkit-stream-closure.smoke` — feature
  spec carrying `Part of initiative: [[X]]` → judge: "does
  closure update X's Status table row?"

Recorded in D's spec for follow-through.

Existing kdevkit fixtures stay green:
`kdevkit.smoke`, `kdevkit-feature-loop.smoke`,
`kdevkit-feature-planning.smoke`, `kdevkit-feature-closure.smoke`,
`kdevkit-dev-loop.smoke`, `kdevkit-review-gate.smoke`,
`kdevkit-review-config-setup.smoke`. Regression net.

Quality gate: `deno task fmt && deno task lint && deno task
check`. Run after the SKILL.md edit slice.

## Design

The diff lands in `sources/skills/kdevkit/SKILL.md` only — same
shape as A. No code, no template files, no new artefacts. Touch
points by section:

- **§1 Locate the spec tree.** Add `initiative/` as a recognized
  subdirectory in the auto-detect line. Two-line edit.
- **§2 Load project context.** Add **Active initiatives**
  paragraph: project.md MAY carry `## Active initiatives` as
  an in-flight-only index (one line per active initiative,
  removed at last-stream close). Format: `- **<name>**
  (`initiative/<name>.md`) — <one-line intent>`.
- **§3 Load feature context.** Add a new bullet to the entry
  cues: `"start initiative <name>"` / `"show initiatives"` /
  `"stream <n> for <initiative>"`. Add a paragraph
  on session-start read order: project.md → active-initiatives
  index → current initiative (if referenced) → feature.
- **§5 Run feature session.** Add a one-line note: when a
  feature is a stream of an active initiative, the planning
  phase auto-populates the `Part of initiative:` link.
- **§6 Feature planning.** Add the auto-link rule (one bullet)
  + extend the feature-file template to optionally show the
  `Part of initiative: [[<name>]]` line near the top (after
  `## Feature Brief`).
- **§8 Closure.** Add a new step **3.5** between backlog cleanup
  and the close-commit: if the closing feature is a stream of
  an active initiative, update the Status table row; if last
  stream, also archive the initiative spec and remove the
  Active-initiatives index entry. Stage all in the same
  `close(<feature>):` commit.
- **§9 Conventional Commits.** Add `plan(<initiative>):` to
  the type list (alongside the existing `plan(<feature>):` /
  `close(<feature>):`). Note the cross-stream rebase carve-out
  to "new commits, never amends."
- **New: §10 Initiative tier — template + workflow.** A new
  section after §9 carrying:
  - The initiative file template (Why / Streams / Decisions /
    Status).
  - The cross-stream rebase mechanics (when a parent stream
    re-ships).
  - The repo-shape orthogonality note ("Working across repos").

The trade-off considered: append the initiative material to
existing sections (no §10 added) vs. create §10 as a unified
home. Picked §10 because the initiative-specific mechanics
(template, rebase, repo-shape) cluster naturally; scattering
them across §1, §2, §3, §6, §8 makes future readers chase
references. The triggering rules go inline with the relevant
section (§1 detection, §3 cues, §6 auto-link, §8 closure step);
the *content* (template body, rebase steps, repo guidance)
lives in §10.

### Initiative file template

```markdown
# Initiative: <name>

## Why

<!-- The realization or external trigger. One paragraph. -->

## Streams

<!-- Ordered list. Each stream = one branch / one CR.
     Format: 1. **<name>** (`<branch>`) — <one-line intent>.
              Prereq: <previous stream id, or "none"> -->

## Decisions taken at the initiative level

<!-- Anything that binds *all* streams. Per-stream decisions
     belong in that stream's feature spec. -->

## Status

| Stream | Branch | CR | Status | Shipped | Learnings |
|---|---|---|---|---|---|
| 1 | ... | ... | planning | — | — |
```

### Cross-stream rebase mechanics

When Stream `n+1` is in-flight and Stream `n` re-ships to
`main` after CR review:

1. From Stream `n+1`'s branch: `git fetch origin && git rebase
   origin/main`. Resolve conflicts.
2. Re-run §7 Quality + Test + Code Review Gates for the slice
   that intersects the rebased change. Threshold semantics
   unchanged.
3. Force-push: `git push --force-with-lease`. Only after §7
   reverifies — never push a rebased branch with stale gates.
4. Update the open CR/PR body if the rebase substantially
   changed the diff (e.g. shrunk because `n`'s changes are
   now in main).

This is the only place §9's "new commits, never amends" rule
yields — the sequential-stream contract requires rebasing,
and `--force-with-lease` keeps the operation safe against
concurrent pushes.

### Working across repos (guidance, not contract)

The skill operates on a `$SPEC_ROOT` resolved relative to a
single working directory. Common shapes:

- **Single-repo** (default): `$SPEC_ROOT = specs/` (or
  `docs/specs/`, `.kdevkit/`). All four tiers (project,
  initiative, feature, backlog) live here.
- **Multi-repo, per-repo specs**: each repo carries its own
  `specs/`. An initiative whose streams span repos is awkward
  — the initiative spec lives in one repo by convention, and
  each cross-repo stream's feature spec lives in the repo
  where the stream's branch lives. Cross-repo references use
  fully-qualified paths or repo names.
- **Cross-repo program** (multiple repos under one umbrella):
  out of scope for the kdevkit skill. A separate top-level
  "program" surface (in a workspace-level directory, not
  inside any one repo) is the right shape; the skill does not
  encode this.

The tier definitions are repo-shape agnostic; this guidance
shows how they map onto common shapes without baking
assumptions into the templates.

## Implementation Plan

One slice. Larger surface than A (multiple §-sections touched +
new §10 added) but no test-surface changes here (all bundled
into D).

1. **Edit `sources/skills/kdevkit/SKILL.md`.**
   - **§1**: add `initiative/` to the spec-tree auto-detect
     paragraph.
   - **§2**: add `## Active initiatives` paragraph to the
     project.md template description.
   - **§3**: add the three new entry cues + session-start
     read-order paragraph.
   - **§5**: add the one-line auto-link note.
   - **§6**: add the auto-link rule + optional template line
     for `Part of initiative:`.
   - **§8**: add new closure step 3.5 (Status table update +
     last-stream archive).
   - **§9**: add `plan(<initiative>):` to Conventional Commits
     types; add the cross-stream rebase carve-out to the
     "new commits, never amends" rule.
   - **New §10 Initiative tier** between current §9 and
     `## Session Log`. Sections: template, rebase mechanics,
     repo-shape guidance.
   - Bump frontmatter `version` to `3.0.0` — signal-of-change
     for personal-use skill (per user direction, not
     semver-for-consumers).
   - Update frontmatter `description` to mention the four
     tiers (project/initiative/feature/backlog) so it's
     greppable.
2. **Run Quality Gate.** `deno task fmt && deno task lint
   && deno task check`. SKILL.md is markdown; lint/check
   no-op for `.md`.
3. **Run Test Gate.** `deno task test:unit`. Should be green
   (no schema or deploy code touched).
4. **Run Code Review Gate.** `code_review.reviewer:
   host-native`, threshold 70, hard-stop, retry-budget 2.
   Reviewer sees `project.md` + the diff; no feature spec.
5. **Push.** Open Agent-dev Review Gate per §7. Body:
   Approach + Reading order.
6. **Closure.** Per session override (autonomous): §8
   reconcile + soft project.md verify + backlog ask
   (answer: this feature closes its own backlog item, which
   was `git mv`'d at branch start; no other items closed) +
   close-commit if needed + Closure Review Gate + squash-merge.

Risk notes:

- *§10 size.* The new §10 carries the template + rebase
  mechanics + repo guidance — three chunks. Risk: it grows
  unbalanced compared to existing §1–§9 prose. Mitigation:
  keep each subsection short; D will likely move parts of
  §10 (template) to a deferred file.
- *Auto-link heuristic.* The "feature is under an active
  initiative" check matches by branch name or feature-spec
  basename. Risk: false positives if two initiatives reference
  the same feature name. Mitigation: the prose says "if
  unambiguous; otherwise prompt." First-cut is auto-link
  unambiguous-only; ambiguous cases get a one-line ask.
- *Cross-stream rebase footgun.* Force-push with
  `--force-with-lease` is safe against concurrent pushes but
  not against stale local state. Mitigation: the prose
  enumerates fetch → rebase → reverify → push order; reviewer
  flags any deviation.
- *Repo-shape ambiguity.* The "guidance, not contract" framing
  for §10's repo paragraph means projects with unusual repo
  layouts won't get prescriptive answers. Mitigation:
  acceptable trade-off — the skill stays portable; teams that
  want a prescriptive cross-repo flow can write their own
  program-level tree.

## Session Log

<!-- append: date · what was done · decisions made -->

- 2026-06-03 · backlog → feature promotion · ran §6 four
  interviews around existing What/Why · resolved 5 open
  questions: read-order = project→initiatives→current→feature;
  rebase semantics = Stream n+1 rebases after Stream n
  re-ships; closure = last-stream's close(<feature>): archives
  initiative; version = 3.0.0 (signal, not semver); cross-repo
  = orthogonal — tier definitions stay repo-shape agnostic,
  separate guidance subsection covers single/multi/program
  shapes · test fixtures bundled into D per user direction.

## Decision Log

<!-- append: decision · rationale · alternatives rejected -->

- **Read order: project.md → active-initiatives index →
  current initiative → feature.** Rationale: initiatives are
  always-on context like project.md once active; loading by
  index (only initiatives the entry cue or current feature
  references) avoids unconditional-load tax.
  Alternatives rejected: (a) load initiative on demand only
  when a feature carries a `Part of:` link — risks the agent
  missing initiative-level decisions when switching features;
  (b) no auto-load — loses the "next session knows where it
  is in the stream" persistence the backlog calls out.
- **Cross-stream rebase: Stream n+1 rebases onto new
  mainline after Stream n re-ships.** Rationale: streams are
  sequential by design; Stream n+1's diff is meaningful only
  against n's shipped state. Staying on a stale base reviews
  against a fictional mainline. Force-push with
  `--force-with-lease` keeps the rebase safe.
  Alternatives rejected: (a) Stream n+1 stays on original
  base — diff drifts; (b) Stream n+1 rebases on every push of
  Stream n — overkill churn.
- **Initiative closure: last-stream's `close(<feature>):`
  archives the initiative.** Rationale: one closure verb,
  scoped per feature. The §8 step extends naturally; no new
  branch + CR ceremony for the archive operation.
  Alternatives rejected: (a) separate `close(<initiative>):`
  branch — adds CR cost per initiative; (b) initiative spec
  stays as permanent record — initiatives are time-bound by
  definition; permanent retention dilutes the meaning.
- **Version bump: 3.0.0.** Rationale: kdevkit is for personal
  use; versioning is a signpost ("this is when initiatives
  landed"), not a semver contract. 3.0.0 reads cleanly in
  later grep / git-log scans.
  Alternatives rejected: 2.8.0 (additive minor) — would work
  semantically but loses the signpost value.
- **Cross-repo: tier definitions stay repo-shape agnostic;
  separate guidance subsection.** Rationale: how to work
  (tiers) and where to work (repo layout) are orthogonal.
  Baking repo assumptions into the tier names (e.g.
  `cross-repo-initiative`) couples them and limits the
  skill's portability across teams with different layouts.
  Alternative rejected: in-scope cross-repo program design —
  would substantially expand B's surface; deferred to a
  future "program tier" feature if a real case demands it.
- **Auto-link feature → initiative on planning.** Rationale:
  low-friction; the link populates without a prompt when
  unambiguous. Ambiguous cases (multiple active initiatives
  reference the same feature name) get a one-line ask.
  Alternatives rejected: (a) always prompt — adds a phase
  pause; (b) manual-only — loses the persistence benefit.
- **Status table updated by closing stream's commit.**
  Rationale: one moment of truth; matches the §8 closure
  pattern (other staged edits land in the same commit).
  Alternatives rejected: (a) start + close updates — two
  write moments, drift risk; (b) manually-edited table —
  loses auto-track benefit.
- **Test fixtures bundled into D (kdevkit-compaction).**
  Rationale: D plans the SKILL.md compaction and matching
  test surface together; bundling B's fixtures avoids a
  double test-surface change and lets D design the fixture
  set against the post-B SKILL.md shape. Recorded as
  follow-through items in D's spec.
  Alternative rejected: ship fixtures on B's branch — costs
  twice the test-surface churn.
- **§10 added as a unified home for initiative tier
  content.** Rationale: initiative-specific mechanics
  (template, rebase, repo guidance) cluster naturally;
  scattering them makes future readers chase references.
  Triggering rules go inline with relevant sections; content
  lives in §10.
  Alternative rejected: no §10, content scattered into
  §1/§2/§3/§6/§8 — readability loss.
