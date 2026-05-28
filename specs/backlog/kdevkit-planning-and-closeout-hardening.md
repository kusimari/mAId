# Backlog: kdevkit-planning-and-closeout-hardening

## What

Three related sharpenings of the kdevkit skill (lives at
`mAId/sources/skills/kdevkit/SKILL.md`), driven by real
session failures that surfaced gaps in the current rule set:

1. **§6 phase gating: "spec-already-drafted" handling.** When the
   user opens a session asking to "develop / address / pick up"
   a feature whose spec already exists on disk, the skill should
   start in **planning phase**, not implementation. Even when
   Requirements / Design / Test Strategy / Implementation Plan
   are all populated, the user wants to gate on whether the spec
   itself is right (was the backlog → spec conversion sound? Is
   the design right? Is the test strategy right?). The current
   §6 rule says "do not chain phases automatically" but the
   spec-validation → implementation transition isn't called out
   as a gated phase boundary. It should be.

2. **§9 close-out: "drive all six steps even when answers are
   none".** The current §9 lists six close-out steps but the
   merge gate (step 2) feels like the close-out, so it's tempting
   to claim done after a successful `gh pr merge --squash`. The
   skill should make explicit that:
   - Step 1 (reconcile in-flight markers / Open Questions) is
     pre-merge, not post-merge — sweep before squashing.
   - Step 5 (backlog cleanup interactive) is mandatory even
     when the answer is "none"; *asking* is the artifact.
   - Step 3 (branch cleanup local + remote + prune) is one
     line, default delete, no permission pause.

3. **§9 close-out: bundle `project.md` updates into the
   squash-merge commit.** When §9.4's soft `project.md` verify
   produces an edit, the close-out should fold that edit into
   the feature branch *before* the squash-merge — either as a
   final commit on the feature branch or by amending the most
   recent commit there — so the merged squash commit on `main`
   carries the docs change. Currently the verify happens *after*
   §9.2 squash-merge, which forces any project.md update to
   land as a separate `docs(project)` commit on `main`,
   breaking the "one logical commit per feature on main"
   invariant declared in §9.2.

   Reorder the §9 steps so the verify is step 1.5 (between
   reconcile and merge), and the merge bundles whatever the
   verify produced. Worked example: in the
   `kdevkit-feature-and-agent-dev-loops` close-out (PR #5,
   merged 2026-05-28), §9.4 surfaced a Layout drift in
   `project.md` only after §9.2 had already merged — leaving
   two commits on main where one was intended.

## Why

Both gaps surfaced in the same session shipping the
testing-first-class-agent-loop feature in
`kusimari/env`:

- **Planning gap** (2026-05-27): I read the fully-drafted spec
  and ran the entire Implementation Plan end-to-end —
  branch + edits + auto-track validation + uncovering and fixing
  pre-existing latent bugs in `env-verify.nix` along the way —
  before the user said "i wanted to be in planning before
  implementation. can you wind back and let's make sure the
  backlog was converted to the right spec, design and ways to
  test it was implemented." A spec being complete on disk does
  not mean it has been *reviewed* with the user.

- **Close-out gap** (2026-05-28): I shipped via `gh pr merge`,
  did the project.md verify offer, and accidentally hit branch
  cleanup partially through `--delete-branch`. I skipped step 1
  (the spec carried three Open Questions that should have been
  the first thing reconciled at close-out) and skipped step 5
  entirely (BACKLOG.md was empty so I didn't surface the
  question). The user asked "did you follow the feature closure
  loop" and I had to recover with a follow-up PR (#32 in the env
  repo) to reconcile the spec's Open Questions and drop an
  obsolete top-level BACKLOG.md.

Both lessons are general kdevkit-skill behaviour, not
project-specific facts. They belong in the skill source
(mAId/sources/skills/kdevkit/SKILL.md) so every project that
uses kdevkit benefits, not just the env repo.

## Open questions

- **Where exactly do the §6 changes go?** Probably a new
  sub-bullet under "Phase gating" naming the
  spec-validation phase explicitly. Possibly a new "When you
  arrive mid-feature" paragraph that calls out how to start.
- **How prescriptive should the §9 reinforcement be?** The
  current language is already a numbered list. Adding "even
  when the answer is none" risks bloating the section.
  Option: bake the emphasis into a one-line "common drift"
  callout at the end of §9, not into each step.
- **Bundle into one feature or split into two?** Both are
  small skill edits. Bundling keeps the feature loop short;
  splitting lets the planning rule ship before the close-out
  rule if review on one is slower.
- **Memory entries to retire on merge.** The current
  per-machine memory at
  `~/.claude/projects/-local-home-gorantls-env-workplace/memory/`
  carries `feedback_planning_before_implementation.md` and
  `feedback_kdevkit_section_9_closeout.md`. Those should be
  deleted from memory when this feature ships (the skill
  rules supersede them).
