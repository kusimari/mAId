# kdevkit — initiative tier (tier module)

Carries the **initiative tier**: when multi-stream work earns an
initiative, the three entry verbs, cross-stream rebase mechanics,
and how the tiers map onto multi-repo shapes.

**Read this when** an initiative is in play — `$SPEC_ROOT/initiative/`
exists and the work references one, an initiative entry verb fires
("start initiative", "show initiatives", "stream `<n>` for
`<initiative>`"), or the current feature spec carries a
`Part of initiative:` link. Unlike the phase modules this is
tier-scoped, not stage-scoped: it can apply during any phase.

The initiative file template and interview shape live in
`interviews.md`.

## 10 · Initiative tier

The fourth tier in kdevkit, slotted between project (timeless)
and feature (one branch). An initiative captures multi-feature
work that can't fit on one branch — the *why* plus the ordered
*streams* (each stream = one feature = one branch / CR /
squash-merge) that deliver it, plus a Status table updated by
each stream's closure commit.

Initiatives are time-bound: created when the multi-stream work
is identified, archived by the last stream's
`close(<feature>):` commit (§8.3.5).

### When to create one

Any time CR review or planning produces "this needs to land as
several CRs in order," the work is an initiative. The signal
is sequential dependency between branches, not just a large
feature. A large feature that can ship as one branch stays a
feature; only when the work has to fan out across multiple
branches in a defined order does it become an initiative.

### Initiative entry verbs

- **"start initiative `<name>`"** — write
  `$SPEC_ROOT/initiative/<name>.md` and populate
  `project.md`'s `## Active initiatives` index with a one-line
  entry. **Inline-Read `interviews.md`** for the initiative
  file template + the three short initiative interviews
  (Why → Streams → initiative-level Decisions). After the
  spec is written, commit as `plan(<initiative>): initial
  spec`, push, and open the Planning Review Gate per §6 / §9
  with phase-specific body content: **Why** + **Streams** +
  **Decisions taken at the initiative level**.
- **"show initiatives"** — list active initiatives from
  `project.md`'s index. Read-only; no commit.
- **"stream `<n>` for `<initiative>`"** — start a feature
  whose Git Setup names the initiative as its parent.
  **Inline-Read `interviews.md`** for the
  template-fill steps (which fields populate from the parent
  initiative's stream entry, which come from the four
  feature interviews). The feature spec's `Part of
  initiative: [[<name>]]` line auto-populates per §6;
  otherwise the flow is a normal §3 feature start followed
  by §6 Planning.

### Cross-stream rebase mechanics

When Stream `n+1` is in-flight and Stream `n` re-ships to
`main` after CR review:

1. From Stream `n+1`'s branch:
   `git fetch origin && git rebase origin/main`. Resolve
   conflicts in place.
2. Re-run §7 Quality + Test + Code Review Gates for the slice
   that intersects the rebased change. Threshold and
   retry-budget semantics unchanged.
3. Force-push: `git push --force-with-lease`. Only after §7
   reverifies — never push a rebased branch with stale gates.
   Plain `--force` is unsafe against concurrent pushes;
   `--force-with-lease` is the contract.
4. If the rebase substantially changed the diff (e.g. shrunk
   because Stream `n`'s changes are now in `main`), update the
   open CR/PR body so reviewers aren't reading against a stale
   summary.

This is the only place §9's "new commits, never amends" rule
yields — the sequential-stream contract requires rebasing.

### Working across repo shapes (guidance, not contract)

Tier definitions (project / initiative / feature / backlog)
are about *how* to work. *Where* the work lives is orthogonal:

- **Single-repo** (default): `$SPEC_ROOT = specs/` (or
  `docs/specs/`, `.kdevkit/`). All four tiers live here.
- **Multi-repo, per-repo specs**: each repo carries its own
  `specs/`. An initiative whose streams span repos is awkward
  — the initiative spec lives in one repo by convention; each
  cross-repo stream's feature spec lives in the repo where the
  stream's branch lives. Cross-repo references use
  fully-qualified paths or repo names.
- **Cross-repo program** (multiple repos under one umbrella):
  out of scope for kdevkit. A separate top-level "program"
  surface (in a workspace-level directory, not inside any one
  repo) is the right shape; the skill does not encode this.

The tier definitions are repo-shape agnostic; this guidance
shows how they map onto common shapes without baking
assumptions into the templates.

