---
name: kdevkit-refactor-shrink-always-on-context
description: Option 2 (split by stage) shipped as initiative stream 1 — the always-on SKILL.md is 1246 -> 572 lines. What remains is option 1 (compress the prose inside each resulting module, now that each is small enough to judge) and option 3 (code-driven stage transitions, which is initiative stream 4). This item now tracks the compression pass, which has no stream of its own.
metadata:
  type: backlog
---

# kdevkit — shrink the always-on context

> **Partly shipped, 2026-08-04.** Option 2 landed as stream 1 of the
> `kdevkit-decompose-and-harden` initiative: `SKILL.md` is 1246 → 572
> lines, with `phases/{plan,dev,review,close}.md` and
> `tiers/initiative.md` loaded on demand. Option 3 is that
> initiative's stream 4 (blocked on the code-vs-prose boundary).
>
> **What this item still tracks: option 1, the compression pass** —
> and it is the piece with no owner. The core landed at 572 against
> the 500-line target, and the split deliberately deferred
> compressing prose *inside* each module so that moves stayed
> verifiable against a rule inventory. Each file is now small enough
> to judge, which is the precondition option 1 was waiting for.
>
> The "how is the refactor verified" question below is answered:
> the four `kdevkit-*.smoke` fixtures plus two new ones
> (`kdevkit-module-load`, `kdevkit-phase-boundary`) are the A/B set.
> Stream 1 shipped without that paid run; a compression pass should
> not.

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
