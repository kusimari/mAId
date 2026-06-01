# Backlog: cr-reading-order

## What

Add a **Reading order** section to every PR/CR description the
agent opens, to reduce the cognitive load on the reviewer when a
single CR spans multiple commits, multiple files, or multiple
specs.

The reading order is a short numbered list — typically 3–7 entries
— that names the files (or specs) in the order a reviewer should
read them, with a one-line "what to focus on" hint per entry.

Surfaces affected:

- The kdevkit §6 Planning Review Gate body (currently: Why + Spec
  summary + Open questions).
- The kdevkit §7 Agent-dev Review Gate body (currently: Why +
  Approach + optional Verification / Reading guide / Pairs with).
- The kdevkit §8 Closure Review Gate body.

§7 already mentions an optional **Reading guide**; this backlog
makes it a default-on pattern (with an opt-out for trivial CRs)
and makes it explicit on the planning + closure surfaces too.

The agent should generate this section automatically based on
what landed in the CR — it has the diff and the commit list, so
it can sequence files in dependency order and pick a meaningful
hint per entry.

## Why

When the reviewer is the same human who instructed the agent,
the cost of "where do I start?" friction is real. Two
observations from CR-2 (sp-api-turing) prompted this:

1. CR-2 has three commits (`chore(workspace)`, `plan(...)`,
   `chore(specs)`) touching ten files across four package
   trees + the project spec. A reviewer landing on the diff has
   no obvious entry point. A reading order would say: read
   project.md first (intent), then v1-listings-impl.md (the
   contract), then per-package amendments (depend on the
   contract), then the chore commits last (mechanical).

2. CR-1 (specs-restructure) had a top-down structure too — the
   "fold invariants into project.md" decision was the
   load-bearing change; everything else was rename/move
   plumbing. A reading order makes that obvious without the
   reviewer having to reverse-engineer it from the diff.

The convention also encourages the agent to actively *think*
about the reviewer's path — which is a forcing function for
spotting inverted-order issues like "this commit references a
spec that doesn't land until commit 3."

## Open questions

1. **Default-on vs opt-in?** kdevkit §7 currently has Reading
   guide as optional. Switching to default-on for any CR with
   ≥3 files or ≥2 commits would catch the cases that need it
   without burdening trivial CRs. Threshold could be a knob.

2. **Format.** Numbered list with `path:hint` is the simplest
   shape. Alternatives:
   - Group by phase: "Read for intent: …; read for contract: …;
     read for plumbing: …."
   - Tag each entry with its commit so reviewers can cross-link
     to the commit-level diff.

3. **Position in body.** Top of body (before Why), or at the
   end (after Verification). Top is easier to find; end avoids
   pushing the reviewer past the rationale. Lean toward top
   since the section is *for navigation*, not for context.

4. **Closure-time rewrite.** kdevkit §8 rewrites the body to
   final shape — should the reading order persist across the
   planning → dev → closure transitions, or be regenerated each
   time? Likely regenerate at each gate since the file set
   evolves.

5. **Where the rule lives.** Two candidates:
   - Local mAId-side feedback memory or skill update if the
     pattern is project-agnostic.
   - kdevkit §7's body-shape paragraph (lifts Reading guide
     from optional to default-on with a threshold).
   Probably the kdevkit edit, since it's a workflow rule, not a
   tool-agnostic deployment concern.
