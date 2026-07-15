# Backlog: kdevkit — right-size the ceremony to the change

## What

Give kdevkit an explicit, always-on rule that scales its
process weight to the size of the change, so a one-line edit
does not get a four-interview spec, R1–Rn requirements, a Test
Strategy table, and a formal Planning Review Gate.

Today the skill has the *mechanism* for this
(`planning_phase: false` in the `kdevkit` block, and the
backlog tier for small items) but no *guidance on when to
reach for it*. SKILL.md §5–§6 read as "always run the three
phases," so an agent applies the full planning ceremony by
default on any change in a kdevkit-managed repo — including
trivial ones. The opt-out exists but is easy to miss in the
moment.

Add a short "altitude of ceremony" rule near the top of §5
(Run feature session) that classifies the incoming work and
picks a lane:

- **Trivial / mechanical** (one-line edit, config value, a
  change that inherits all its behavior from existing code) —
  skip the four interviews and the Planning Review Gate; a
  one-line Decision Log entry (or just the commit message)
  captures any real fork. Go straight to the dev loop
  (Quality → Test → Code Review → Push).
- **Small feature with a genuine design fork** — no full spec,
  but record the fork. A backlog-style note or a Decision Log
  line is enough; the Code Review Gate still runs.
- **Real feature** (multi-file, new surface, cross-repo,
  sequential streams) — the full §6 planning phase as written.

The signal is "how much undetermined design is there," not
"is this repo kdevkit-managed." The gates that protect
correctness (Quality / Test / Code Review) stay on for every
lane; only the *planning paperwork* scales down.

## Why

- **Observed live (2026-07-15).** Task was a one-line addition
  to a package-install array in a bootstrap script, plus a
  small non-fatal post-install step. The agent produced a full
  6-section feature spec (Feature Brief, R1–R4, Test Strategy,
  Design, Implementation Plan, Session + Decision logs), two
  planning commits, and a formal Planning Review Gate before
  writing any code. The user pushed back: "why did you need
  such an elaborate spec for such a simple feature?" The spec
  even admitted the change inherits all its behavior from an
  existing loop — which is exactly the signal that the trivial
  lane applied.
- **The one useful artifact was tiny.** There was a single real
  design fork (run the tool's first-run setup? make it fatal?).
  That deserved one Decision Log line, not a spec section apiece
  for requirements/tests/design.
- **The opt-out is under-advertised.** `planning_phase: false`
  is documented as a project-wide setting in §2/§4, but nothing
  tells the agent to *reach for the lighter lane per-change*.
  Result: process cargo on small work, which is exactly the
  friction that makes a spec-driven workflow feel heavy.
- **Correctness gates were never the problem.** The complaint
  is about planning paperwork, not about review or tests. The
  rule should scale *planning*, and explicitly keep
  Quality/Test/Code Review on for every lane.

## Open questions

- **Where the rule lives.** Best fit is a new always-on
  subsection at the head of §5 (it gates the whole session
  arc). Alternatively fold into §5's "Phase-gating cues." Avoid
  scattering it — one source of truth.
- **How to classify without a new interview.** The
  classification itself must not become ceremony. Likely a
  3-line heuristic the agent self-applies silently, surfacing
  only its choice ("this is a one-line change; skipping the
  planning phase — say so if you want the full spec").
- **Per-change vs. per-project.** Should the lighter lane be a
  per-change judgment (default) with `planning_phase: false` as
  the per-project hard override, or should the skill nudge
  toward setting `planning_phase: false` on repos that are
  mostly small edits? Probably both: per-change heuristic
  always on; suggest the project setting when the agent notices
  a pattern of trivial changes.
- **Interaction with the backlog tier.** For "small feature
  with a fork," is a backlog note the right home, or a
  slimmed-down feature file? Define the minimum viable record
  so it doesn't drift back into a full spec.
- **Default bias.** When genuinely unsure which lane, which way
  to lean? Suggest: lean light and *offer* the full spec, rather
  than defaulting to full and making the user ask for less
  (the failure mode observed).

## Trigger to promote

- Another instance of over-ceremony on a small change is
  flagged (this is the second-order signal; the observed case
  above was the first).
- A batch of kdevkit edits is scheduled anyway — bundle this
  with them.
- A project's owner decides small edits there should default to
  `planning_phase: false` and wants the skill to support that
  lane cleanly.

## Note on editing the skill

`resources/content/skills/kdevkit/SKILL.md` is the source
behind the managed skills symlink — edit it here in the repo,
not under `~/.claude/skills/kdevkit/`. Changes land in the next
session. Per the skill's own multi-file split, always-on rules
(this one) belong in `SKILL.md`, not the deferred `setup.md` /
`interviews.md`.
