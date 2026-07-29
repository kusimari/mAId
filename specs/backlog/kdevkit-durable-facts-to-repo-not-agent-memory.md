---
name: kdevkit-durable-facts-to-repo-not-agent-memory
description: Add a standing kdevkit rule that durable project facts, working-style preferences, and workflow corrections belong in the repo's versioned context (AGENTS.md / project.md / spec tree), NOT in the agent's private auto-memory. When a correction lands, the agent should reach for the repo doc, and only fall back to auto-memory for genuinely cross-project user facts.
metadata:
  type: backlog
---

# kdevkit — durable facts go to the repo's versioned context, not agent auto-memory

## What

Add an always-on rule to the `kdevkit` skill (§9 cross-cutting hygiene,
where the AGENTS.md / project.md context-layer convention already lives)
that governs **where a newly-learned durable fact is written**:

- **Project facts, working-style preferences, and workflow corrections
  that are true of _this repo / this line of work_ → the repo's versioned
  context**: `AGENTS.md` (operational), `project.md` (project-knowledge),
  or the spec tree (feature/initiative decision logs). Versioned, shared,
  reviewable, and loaded automatically by any agent/human on the repo.
- **Agent private auto-memory** is reserved for facts that are genuinely
  **cross-project and about the _user_** (who they are, global tool
  preferences) — not per-repo mechanics. A per-repo rule in auto-memory is
  invisible to teammates, un-reviewed, and drifts from the code.

The rule should also cover the **correction reflex**: when the user
corrects the agent's working style ("don't do X, do Y"), the agent's first
move is to find the doc where that rule belongs and edit it there — and if
the correction contradicts an existing line in a repo doc, fix that line
(the contradiction is usually why the agent got it wrong).

## Why

Observed failure (BeehiveMono, 2026-07-27): the agent learned "pushing
BeehiveMono branches/CRs is fine — the never-push rule is only for nested
product repos" and wrote it to its **private auto-memory**. The user
corrected: *"don't write to your memory. write to agents or project md
file."* The fact was per-repo working-style, so it belonged in
`AGENTS.md`, where a teammate or a fresh session would actually see it.

Two deeper points this surfaces:

1. **Auto-memory is a silent, unversioned side-channel.** A working-style
   rule stored there never reaches review, never travels with the repo,
   and silently diverges from the AGENTS.md/project.md that contradict it.
   In the incident, the agent hesitated to push precisely because
   `AGENTS.md` still said "All branches are local-only until the user
   guides the remote push" — the right fix was editing that line, not
   memoising an exception privately.
2. **kdevkit already owns the context-layer split** (operational →
   AGENTS.md; project-knowledge → project.md; per-feature → spec). The
   missing piece is a rule about *routing a freshly-learned fact into that
   split*, and an explicit "not private memory" boundary. Today the split
   describes where things live but not the reflex when something new is
   learned mid-session.

## Sketch

- §9 (or §8 closure, where durable content bubbles up) gains a short
  "Where a learned fact goes" rule with the routing above and the
  correction reflex.
- Phrase it harness-agnostically — "the agent's private/auto memory, where
  the harness has one" — since not every tool kdevkit compiles to has an
  auto-memory feature. The rule is about *preferring versioned repo
  context*, which is universal.
- Tie it to the existing "Never corrupt the AGENTS.md convention" and
  "lean beats detailed" rules so it doesn't invite dumping everything into
  AGENTS.md — route to the *right* layer, keep each lean.

## Open questions

- Is there ever a legitimate per-repo use of auto-memory (e.g. a secret /
  path that must not be committed)? If so, carve the exception explicitly
  rather than an absolute ban.
- Should the rule fire proactively (agent offers to write the doc when it
  notices it learned something durable) or only on correction? Leaning
  proactive-at-closure + immediate-on-correction.

## Trigger to promote

- The routing mistake recurs (agent memoises a per-repo rule again), or
- a second repo/user hits the same "why didn't you just put it in
  AGENTS.md" correction — confirming it's a general kdevkit gap, not a
  one-off.
