---
name: kdevkit-refactor-shrink-always-on-context
description: kdevkit's always-on SKILL.md is ~1250 lines and grows with every feature. Three candidate refactors, from cheapest to most structural — (1) compress the prose itself, (2) split by tier and by feature stage the way setup.md/interviews.md were split, (3) do (2) but with a code wrapper driving stage and loop transitions instead of prose. Pick one, or sequence them.
metadata:
  type: backlog
---

# kdevkit — shrink the always-on context

## What

`resources/content/skills/kdevkit/SKILL.md` is **~1250 lines, loaded
every session**, and it grows with each feature that touches the
workflow (the kreviewkit feature added ~90 for one opt-in step, even
after deliberately pushing its schema into `setup.md` and the briefing
contract into the generator's own skill). The two deferred files
(`setup.md` 249, `interviews.md` 256) already prove the split works;
the always-on half is the problem.

Three candidate approaches, cheapest first. They are not mutually
exclusive — 2 subsumes nothing from 1, and 3 is 2 plus a driver.

### 1 · Compress the prose

Keep the single always-on file; make it much smaller by writing it
much more tightly. No structural change, so nothing about loading or
dispatch changes.

**Open question, and the reason this isn't already done:** *how?* The
prose is long because it carries rationale, and the rationale is
load-bearing — several rules exist precisely because an agent
mis-read a terser earlier version (§6's Plan-commit ordering, §7's
re-pin, the `description:`-vs-body trigger rule). Naive compression
risks re-introducing the failures the verbosity was added to prevent.
Needs a method, not just an edit pass. Candidates to explore:
rule-then-rationale ordering so the rationale is skimmable but
present; moving *why* into the spec tree and leaving *what* in the
skill; a worked-example appendix in a deferred file.

### 2 · Split across files by tier and stage

Extend the split that `setup.md` / `interviews.md` started, along the
seams the workflow already has:

- **By tier** — project / initiative / feature (§10's initiative
  mechanics only matter when an initiative is in play; §1–§2's project
  layer is mostly session-start).
- **By feature stage** — plan-spec / dev-loop / closure. A session in
  the dev loop does not need closure's eight steps or planning's four
  interviews resident.

Always-on then shrinks to: detect the spec tree, resolve entry mode,
load the *relevant* stage file, plus the genuinely cross-cutting §9
rules. This is the same "operational vs. deferred" rule the skill
already states for itself, applied more aggressively.

**Open questions:** what stays cross-cutting (§9's Conventional
Commits, public-repo hygiene, and Review Gates plausibly must be
always-on); whether stage detection is reliable enough to load the
right file (a mid-session phase transition must pull the next one);
whether more files raises the per-session read count enough to lose
the savings.

### 3 · Split as in 2, but drive transitions with code

Same file split, but instead of prose telling the agent which stage it
is in and when to move, a **wrapper over the underlying agent uses
code** to track the loop/stage and load the right context — the
mechanical part (which phase, which gate next, which file to read)
becomes deterministic, and the skill files carry only the judgement
each stage needs.

This is the most structural option and the most likely to actually
fix the growth curve, since new workflow rules land in a stage file
rather than the always-on one. It also removes the class of bug this
repo keeps hitting — an agent reading phase rules top-to-bottom and
executing them out of order (see the §7 Review Briefing section that
had to be physically moved to match execution order).

**Open questions:** mAId already ships a Rust build-tool and `kaimux`,
so a wrapper has precedent — but does it belong there or as a
separate crate? How does it stay tool-agnostic across
claude/kiro/codex, which is the project's whole mission? Does a
code-driven loop conflict with the "skills are plain markdown
symlinks, no runtime" deploy invariant (the browser-MCP precedent
shows a runnable resource is possible, but it's the exception)? What
happens when a user drives kdevkit *without* the wrapper — does the
prose still have to stand alone, which would defeat the savings?

## Why

- **The always-on file is the cost center.** Over-stuffed context
  measurably degrades agent performance and costs tokens on every
  session — a point `project.md` and the skill itself both make, and
  which the skill is now the largest violator of in this repo.
- **The growth is structural, not incidental.** Every workflow feature
  adds always-on prose. The kreviewkit feature deliberately pushed as
  much as possible out (schema → `setup.md`, briefing contract → the
  generator's own skill) and *still* added ~90 lines. Without a
  refactor the trend continues.
- **Ordering bugs track length.** The longer the file, the more the
  agent's reading order diverges from execution order. This feature
  hit exactly that and fixed it by moving a section; option 3 removes
  the class.
- **The seams already exist.** project/initiative/feature and
  plan/dev/closure are the skill's own structure, and the
  `setup.md`/`interviews.md` split already validated deferring content
  behind a trigger.

## Open questions

- **Which option, or which sequence?** 1 is cheapest and might be
  enough; 2 is the natural extension of what's proven; 3 is the only
  one that changes the growth curve. Plausible sequence: 2 first (it
  is safe and mechanical), then 1 on each resulting file (smaller
  scope, easier to judge), and 3 only if the wrapper earns its keep.
- **How is the refactor verified?** This is the hard part. It is a
  pure-prose change to the most critical skill in the repo, so the
  only real evidence is functional: the existing `kdevkit-*.smoke`
  fixtures (planning / dev-loop / closure / agents-md) passing
  tri-tool before and after, as an A/B. Budget for that up front —
  a refactor that can't be A/B'd shouldn't ship.
- **Does compression regress trigger reliability?** `description:`
  length is separately budgeted (see `project.md` Testing — four
  descriptions at 815 chars matched 3/3 where 3830 matched
  unreliably). A refactor must not blow that budget, and
  `discovery` fixtures are the check.
