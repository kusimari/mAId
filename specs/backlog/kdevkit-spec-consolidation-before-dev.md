---
name: kdevkit-spec-consolidation-before-dev
description: kdevkit has no step that consolidates a spec after planning converges. The spec accumulates iteration scars — superseded options, one-off Q&A, "revised after research" notes — and then the dev phase reads that as its contract. Add an explicit consolidation at the planning→dev boundary that rewrites the spec as a standalone implementable artifact, with the discussion archived rather than inline.
metadata:
  type: backlog
---

# kdevkit — consolidate the spec before dev, not after

## What

Add a **consolidation step at the planning → dev boundary**: once
iteration stops and decisions are made, the spec is rewritten so
that a reader who saw none of the discussion can implement from
it.

Today §6 ends with the Plan-commit rule (write → commit → push →
open the Planning Review Gate → wait for the cue). Nothing between
the gate going green and the dev loop starting says *"now clean
this up."* So the artifact the dev phase inherits is the artifact
planning happened to end on — which is a transcript, not a spec.

The rule should require, at minimum:

- **Decisions stated as decisions**, not as the argument that
  produced them. "Verdicts are severities, aggregated
  strictest-wins" — not "(a) retire the score *(my
  recommendation)* / (b) keep both / (c) keep as-is."
- **Superseded material removed from the contract**, not struck
  through in place. If R3 was revised after research, the spec
  carries R3-as-decided; the revision history goes to the archive.
- **Per-round Q&A collapsed.** "F1 · Where idiomatic style lives"
  is a reply to a reviewer; the *rule* it settled belongs in the
  spec body, and the reply doesn't.
- **Open questions separated from settled ones**, with owners —
  a reader must be able to tell "this is decided, build it" from
  "this is still live, don't."
- **The discussion archived, not deleted.** The reasoning is worth
  keeping (it's why several rules exist), just not in the
  implementable artifact.

## Why

- **Observed live (2026-08-04, this initiative).** The user:
  *"before we implement we never make the spec file cleaner …
  once all iterations and decisions are made, then the spec should
  remove the intermittent discussions and be a spec that anyone
  other than me should also be able to take as input and
  implement."* The planning spec had reached ~1660 lines carrying
  research, a reverted proposal, two rounds of numbered feedback,
  and three decision blocks in `<details>` — implementable by the
  session that wrote it, and by nobody else.
- **This is exactly the failure kdevkit warns about elsewhere.**
  §2 says over-stuffed context files "measurably *degrade* agent
  performance," and the dev phase's contract *is* context. A spec
  full of superseded options is worse than a shorter one: an agent
  can't tell a rejected alternative from a requirement, so it may
  implement the thing we argued against.
- **It compounds under decomposition.** Once phases run as
  separate agents (this initiative's whole point), the dev agent
  reads the spec *without* the conversation that disambiguates it.
  Iteration scars that are harmless in one long session become
  actively misleading across a handoff. The consolidation is what
  makes the handoff honest.
- **The tiers already imply it and don't do it.** §8 closure
  bubbles durable content *up* into the persistent layers at the
  end. Nothing bubbles *sideways* — planning's mess into
  planning's output — at the point where it matters.

## Sketch

- New subsection at the end of §6, firing on the planning → dev
  cue *before* the dev loop starts: consolidate, commit as
  `plan(<feature>): consolidate spec`, and let the gate body
  reflect the cleaned spec.
- **Where the archive goes** is the main design question. Options:
  (a) a `## Planning Archive` section at the foot of the same file
  — one file, but the file stays long; (b) the PR/CR conversation
  is the archive, since the discussion happened there anyway and
  is already durable — the spec then just *stops* carrying it;
  (c) a sibling `feature/<name>.planning.md`, which §6's refusal
  of `research.md` argues against. **(b) is cheapest and most
  honest** — the review thread is the transcript, so don't
  duplicate it.
- **Consolidation is a rewrite, not an edit pass.** Cheapest
  reliable form is probably: reread the spec, restate every
  settled decision in the imperative, drop anything not needed to
  build, and diff the two for anything dropped that shouldn't
  have been.
- Interacts with the **ceremony lane** — a trivial-lane change has
  no iteration to consolidate, so the step no-ops there.
- Interacts with the **initiative tier**: for multi-stream work
  the consolidated artifact is naturally the *initiative* spec,
  with each stream's feature spec consolidated at its own
  planning→dev boundary. Demonstrated in this initiative
  (`specs/initiative/kdevkit-decompose-and-harden.md` is the
  consolidation of a 1660-line planning spec).

## Open questions

- **Does consolidation need its own gate, or is it a step inside
  the existing Planning Review Gate?** Leaning: a step, since a
  separate gate adds a stop for a mechanical rewrite. But then the
  gate's body should be regenerated from the consolidated spec, or
  reviewers read the pre-consolidation version.
- **How is "implementable by someone else" checked?** The honest
  test is a fresh-context agent reading only the consolidated spec
  and reporting what it can't determine — the same packet
  discipline this initiative applies to reviewers. Cheap, and it
  turns a subjective bar into an artefact.
- **Does the same rule apply at dev → review?** Probably not
  identically: the review phase *wants* the dev narrative (that's
  what the briefing is). Likely planning→dev only.

## Trigger to promote

- Rides with this initiative — it is a rule *about* the
  planning→dev handoff, which is stream 2's territory
  (`phase-handoff`). Fold it in there rather than shipping
  separately.
