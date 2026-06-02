# Backlog: kdevkit-initiative-tier

## What

Add **initiative** as a fourth tier in the kdevkit skill, slotted
between project (timeless invariants) and feature (one branch's
contract). Today the skill has:

- **Project** — `specs/project.md`. Cross-feature invariants.
- **Feature** — `specs/feature/<name>.md`. One branch, one CR, one
  squash-merge.
- **Backlog** — `specs/backlog/<name>.md`. Wanted-but-not-now.

The new tier:

- **Initiative** — `specs/initiative/<name>.md`. Multi-feature work
  that can't fit one branch. Carries the "why" + the ordered
  sequence of features (streams) that deliver it, plus a status
  table updated by each stream's closure commit.

Concretely the skill needs:

1. A new template for `initiative/<name>.md` (sections: Why,
   Streams, Decisions taken at the initiative level, Status).
2. §1 (Locate the spec tree) gains a recognized `initiative/`
   subdirectory alongside `feature/` and `backlog/`.
3. New verbs: "start initiative `<name>`", "show initiatives",
   "stream `<n>` for `<initiative>`".
4. §6 Planning gets a "this feature is part of initiative X" link
   that the agent fills in automatically when the feature spec
   sits under an active initiative.
5. §8 Closure gets one more step: if the closing feature is part
   of an active initiative, update the initiative's Status table
   row (branch, CR, status: shipped, ship date, one-line learning).
   Stage as part of the `close(<feature>):` commit.

## Why

Two real cases where the project ↔ feature gap hurt:

1. **`sp-api-turing` v1 vertical refinement.** A round of CR review
   on `feat/v1-listings-impl` produced design realizations that
   required restructuring (folder names, package boundaries,
   project mission), which then required re-implementing. Three
   coupled streams that need to ship as separate CRs but are part
   of one decision. Without an initiative tier, the "why these
   three branches" lives in chat, not in the repo. With it, the
   initiative spec is the persistent root that survives compaction
   and tells the next agent session "you're in stream 2 of 3."

2. **mAId itself.** The `maid-as-flake-package` backlog item is
   really one stream of a "make mAId installable" initiative that
   includes the package wrapper, the home-manager integration, and
   possibly the `--port`/daemon mode. Today they're either one
   feature spec (too big) or three backlog items (no glue between
   them). An initiative gives them a shared root with a dependency
   ordering.

The general pattern: any time a CR review or a planning
conversation produces "this needs to land as several CRs in
order," the work is an initiative, not a feature.

## How it composes with existing tiers

- **Project** stays timeless. Initiatives are time-bound —
  they finish when their last stream ships and the spec moves to
  an `archive/` (or is deleted, depending on whether the learnings
  belong in project.md).
- **Feature** is unchanged in shape. The only addition is an
  optional `Part of initiative: [[<name>]]` link near the top of
  the feature spec.
- **Backlog** is unchanged. A backlog item can be promoted to
  either a feature OR an initiative, depending on whether one
  branch will close it out.
- **Project's `## Active initiatives`** subsection (new) — a
  one-line index of in-flight initiatives, like the backlog index
  but for initiatives. Removed when the initiative ships.

## Initiative file template (draft)

```markdown
# Initiative: <name>

## Why

<!-- The realization or external trigger. One paragraph. -->

## Streams

<!-- Ordered list. Each stream is one branch / one CR.
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

The Status table is the persistence mechanism. A stream's closure
commit updates its row; future sessions read the table to know
where the initiative is.

## Open questions

1. **Where does `initiative/` go in the §1 auto-detect order?** It
   doesn't matter for detection (project.md auto-detect is the
   gate); but it matters when the agent's "what should I read at
   session start" pass runs. Probably: project.md → active
   initiatives index → load any initiatives the entry cue refers
   to → feature(s) for the current branch.

2. **Cross-stream merge order.** What if Stream 2 is in the dev
   loop and Stream 1's CR gets a comment that requires changes?
   The mainline-rebase semantics need to be worked out: does
   Stream 2's branch rebase onto the new mainline after Stream 1
   re-ships, or does Stream 2 stay on the original base and pick
   up Stream 1's changes only after merge? Lean toward "Stream 2
   rebases" since the streams are sequential by design, but the
   mechanics need explicit spec text so a future session doesn't
   improvise.

3. **Initiative closure verb.** Is there a `close(<initiative>):`
   commit at the end (a docs/cleanup commit on a separate branch
   that archives the initiative spec), or does the last stream's
   closure naturally archive it? Probably the latter — the last
   stream's `close(<feature>):` commit moves the initiative spec
   to `archive/` if the project keeps an archive, or `git rm`s
   it if not. Adds one more step to §8.

4. **Cross-repo initiatives.** mAId is one repo; sp-api-turing is
   another. An initiative that spans both would need a different
   surface (probably a top-level "program" in
   `~/env-workplace/Gorantls-env/programs/`). Out of scope for
   this backlog — start with single-repo initiatives, add
   cross-repo if a real case needs it.

5. **kdevkit version bump.** This is a breaking-shape change
   (new directory, new commit-type, new closure step). Probably
   v2.7.0 or v3.0.0 depending on how strict the agent's existing
   "no `initiative/` directory" assumptions turn out to be in
   practice.
