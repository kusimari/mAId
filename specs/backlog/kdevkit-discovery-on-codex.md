---
name: kdevkit-discovery-on-codex
description: On codex, kdevkit does not self-trigger from a natural work request. Told "Ship it" on a finished feature branch, codex does plain git work (creates main, deletes the branch) and never loads the skill; claude and kiro both run the closure phase. Not an install or comprehension problem — codex reads the skill correctly when handed its path.
metadata:
  type: backlog
---

# kdevkit does not self-trigger on codex

## What

`kdevkit`'s `integration` test — a bare work request, no skill named, no
path — passes on claude and kiro and fails on codex. Given "I'm on branch
feat/add-due-dates and the work is done … Ship it", codex:

- read `due.py`, correctly confirmed both plan items were implemented,
- created a local `main`, switched to it, deleted the feature branch,
- never ticked the Implementation Plan boxes, made no `close(...)` commit,
  and never mentioned the spec, reconcile, or kdevkit at all.

Claude and kiro both recognised the same prompt as the closure phase and
reconciled the spec in place.

Narrow the cause down before changing anything, because three things are
already ruled out:

- **Not an install problem.** `~/.codex/skills/kdevkit/SKILL.md` is present
  (48k).
- **Not a comprehension problem.** Handed the path, codex summarises the
  closure phase accurately: "reconciles the feature spec's Implementation
  Plan in place by marking completed items done and moving any unfinished
  items into backlog or a follow-up feature."
- **Not an enact problem.** With the skill named in the prompt, codex does
  the whole thing correctly — both boxes ticked, commit
  `close(add-due-dates): reconcile feature spec`. The one `enact` failure
  in the sweep did not reproduce and was a flake.

So the gap is specifically **discovery**: codex does not connect a natural
development request to this skill.

## Why

This is the failure mode the `discovery` / `integration` test kinds exist to
catch, and it is invisible to every other kind — each of those hands the
agent the skill, so they all pass. A user on codex who says "ship it" gets
plain git behaviour and silently loses the entire methodology: no
reconcile, no closure commit shape, no persistent-layer verify, no backlog
sweep. Nothing errors; the work just isn't done the documented way.

kdevkit is also the skill where this costs the most — it is the largest
(1120 lines) and the one whose value is procedural rather than
informational.

## Open questions

- Is codex's skill selection driven by the `description:` frontmatter? If
  so, does kdevkit's description read as *documentation about a workflow*
  rather than *a thing to do when asked to ship, plan, or close out*? It
  opens "Spec-driven development workflow — four tiers (project /
  initiative / feature / backlog), locate spec tree, load context…", which
  is a summary, not a set of triggers.
- Do the other three skills' descriptions differ in shape? `notes` leads
  with "Capture reminders, insights, 1:1 notes…" and self-triggers on codex,
  as does `writing-style` ("Voice, tone, and structure for my prose") — both
  read as *what the user wants done*. Worth comparing systematically before
  rewriting anything.
- Does codex need an explicit trigger list ("when the user says ship it,
  close it out, plan a feature, start work on…") the way the skill's own
  §5 phase cues already enumerate for its internal use? Those cues exist
  inside the file but may not reach whatever codex indexes.
- Does codex surface skills to the model at all in `exec` mode, or only on
  explicit reference? If the latter, discovery on codex may not be fixable
  from skill content and the honest move is to record codex as
  explicit-invocation-only rather than chase it.
- Multi-file skills: kdevkit ships `SKILL.md` plus deferred `setup.md` /
  `interviews.md`. Does that shape affect how codex indexes it versus the
  single-file skills that do self-trigger?
