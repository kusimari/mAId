# Feature: squash-message-authored

## Git Setup

- Branch: `fix/squash-message-authored`
- Base: `main` @ `4da8a94`

## Feature Brief

A feature closed out by kdevkit lands on `main` as one commit whose
message was *written* — a subject naming the ship and a body
explaining why it exists — rather than assembled from whatever the
branch happened to commit along the way. Closure authors that message
as part of the merge; the repository default is changed so that a
merge performed without one degrades to the PR body instead of a
concatenated commit transcript.

## Handoff

- **Stage:** planning
- **Ready for:** dev, on the planning → dev cue
- **Carry forward:** the merge that motivated this (`0bc742a`, PR #50)
  is the worked counter-example — its body on `main` is neither the
  commit transcript *nor* the PR body. Diff the two before writing
  close.md's rule; the difference is what the rule has to ask for.
- **Deliberately left:** `delete_branch_on_merge` is still `false`
  (see OQ1) — adjacent repository setting, not this defect.

### Crossings

<!-- Append one line per crossing; never edit or delete a line.
     A RETURN needs four parts: fault / issue / fix / done when.
     An EXCEPTION needs two: skipping / why. -->

- backlog → planning

## Requirements

- **R1 — The body on `main` is written, not assembled.** After a
  kdevkit closure, the commit on `main` carries a paragraph-form
  summary of what shipped and why. It does not contain the branch's
  own commit subjects as a list.
- **R2 — The subject on `main` is the feature-ship subject**, the one
  the Closure Review Gate set, no matter how many commits the branch
  carries — including one.
- **R3 — A forgotten override degrades, it doesn't regress.** A squash
  merge performed with no explicit message writes the PR body onto
  `main`, not the commit transcript. This is the floor, not the
  target: R1 is still what closure does.
- **R4 — The rule is stated once.** Closure is the only phase that
  merges, so the instruction lives at the merge step; no other module
  restates it.

## Test Strategy

The change is markdown (a kdevkit module) plus two settings that live
outside the repository. Both halves need different evidence.

### Unit tests

None to add. `just test` must stay green as a no-regression check —
nothing under `resources/build-tool/` or `kaimux/` is touched, and
`shipped_content_validates` covers the new fixture's frontmatter
reaching a valid state.

### Functional / integration

One new fixture, `resources/tests/skills/kdevkit-squash-message.smoke`
— behavioral, tri-tool. This is a load-bearing closure behavior that
fails silently: a transcript on `main` looks like a successful merge
and is only noticed years later by the person reading history, which
is precisely the class of failure `project.md`'s Testing section says
earns a fixture.

Seed: a scratch repo with a `main`, and a feature branch carrying
*three* commits with deliberately transcript-flavoured subjects
(`plan(...)`, `feat(...)`, `fix: the assert`), plus a review-stage
feature spec. Task: close it out and squash-merge into the local
`main`.

Assert on `main`'s tip commit, pairing every negative with a positive
(a no-op agent must fail):

- The body does **not** contain `fix: the assert` or the
  `plan(...)` subject — the transcript did not land.
- The body **does** contain prose: more than one line, and a
  recognisable why-sentence rather than a bullet list of subjects.
- The subject matches the feature-ship shape (`feat(`), not
  `close(` and not `plan(`.

**This fixture is post-install only.** `close.md` is a deferred
module, and a skill's pre-install stage carries only `SKILL.md` — so
`check-skills` cannot see the rule and a failure there would be
telling us about the stage, not the change. The honest command is
`just resources::smoke-skills`.

Agentic runs stop at `just test` plus the free
`just resources::verify-skills-dry`. The paid command is handed to the
user:

```
just resources::install-skills          # the fixture is post-install
just resources::verify-skills-one kdevkit-squash-message
```

Sample 3–5 runs per agent and record the ratio — trigger and
adherence behavior is probabilistic.

### The settings half

One-shot, manually verified, recorded in the Session Log:

```
gh api /repos/kusimari/mAId \
  --jq '{squash_merge_commit_title, squash_merge_commit_message}'
```

Expected: `PR_TITLE` / `PR_BODY`.

## Design

### The backlog item's fix is half of one

The item proposed flipping `squash_merge_commit_message` to `PR_BODY`
on the premise that the PR body "is already written for a reader who
wasn't in the branch." Grounding contradicts that premise, and the
counter-example is in this repository's own history.

PR #50 merged as `0bc742a`. Its body on `main` is a reflowed,
link-free, table-free prose narrative. The PR body for #50 is a
review document: markdown tables of per-agent pass rates, a
branch-relative blob URL, a `## What to review` reading list, a
reference to an unlanded sibling branch. **Neither is the other.**
`PR_BODY` would not have produced what is on `main`; it would have
written the review scaffolding into permanent history.

So the defect is not "the setting points at the wrong source." It is
that **no source is the commit message** — the message has to be
authored, and kdevkit never said so. The setting only decides what
happens when nobody authors one.

### Two layers, and which one is the fix

**The fix** is `phases/close.md` step 6: the squash merge carries an
authored message. Subject = the title the Closure Review Gate just
rewrote (step 5 already does this work; step 6 has never consumed
it). Body = why the feature exists, written for someone reading
`main` with no other context — the same *Why*-first discipline §9's
Review Gates already impose on a PR body, minus the parts that only
make sense inside a review tool (Reading order, the verification
command dump, cross-branch pointers).

Stating it at step 6 satisfies R4: closure is the only phase that
merges, so there is exactly one place for it, and `SKILL.md` needs no
edit.

**The floor** is the two repository settings. They matter only when
the rule is not followed — a merge from the GitHub UI, a different
tool, an agent that skipped the step:

- `squash_merge_commit_message`: `COMMIT_MESSAGES` → `PR_BODY`.
- `squash_merge_commit_title`: `COMMIT_OR_PR_TITLE` → `PR_TITLE`.

`PR_BODY` over `BLANK`: `main`'s history exists to carry the *why*,
and a blank body loses it in the one place that cannot be corrected
cheaply — the commit is already on `main`, so fixing it means
rewriting shared history. `PR_BODY`'s failure mode is cosmetic
(review scaffolding in a commit message) and its content is
substantively right, because the Review Gate contract already
requires the body to explain motivation to a reader who wasn't in the
branch. A loud-but-empty failure is the worse trade here.

`PR_TITLE` over `COMMIT_OR_PR_TITLE`: the existing value uses the
*commit's* subject whenever the branch has exactly one commit. A
single-commit branch is common in this repo, and its one commit may
well be `plan(x): initial spec` — so `main` would read as a planning
commit for a shipped feature, ignoring the title step 5 deliberately
rewrote. `PR_TITLE` makes step 5 authoritative in every case, which
is what R2 asks for.

### Reach for what exists

Nothing to build. `gh pr merge --squash --subject … --body …` already
takes both, and it is the command close.md's step 6 already implies;
the change is that the flags become the norm rather than a
remembered override. The settings change is one `gh api -X PATCH`.
No dependency, no code.

## Implementation Plan

- [ ] `phases/close.md` step 6 — the authored-message rule: subject
      from the step-5 title, body as a written why, and what to leave
      out of it relative to the PR body.
- [ ] Grep the rest of the skill for anything that now restates or
      contradicts the rule (R4); reconcile rather than duplicate.
- [ ] New fixture `resources/tests/skills/kdevkit-squash-message.smoke`
      per the Test Strategy, with the vacuity pairing.
- [ ] `just test` + `just resources::verify-skills-dry` green.
- [ ] Flip the two repository settings via `gh api -X PATCH`, and
      record the before/after in the Session Log.
- [ ] Hand the paid verification command to the user.

- *Risk note:* the settings flip is the one step whose effect is
  outside this branch and outside git. Reverting the merge does not
  revert it, and it changes every contributor's merges — which is why
  the backlog item declined to do it unilaterally. Do it last, and
  state the before-values so it is reversible by hand.
- *Risk note:* this would be the first fixture to write to a scratch
  `main` and read a merge commit back. `specs/backlog/`
  `test-runner-workdir-containment.md` records an open question about
  how tightly the runner contains a fixture's workdir; if the merge
  turns out not to be expressible there, fall back to asserting the
  message the agent *composed* (a `playback` on the rule) and file
  the behavioral half.
- *Risk note:* prose in a deferred module is invisible pre-install.
  A "failure" reported from `check-skills` for this change is a
  statement about the stage, not the rule.

## Session Log

<!-- append: date · what was done · decisions made -->

- **2026-09-12 · planning.** Promoted from
  `specs/backlog/squash-message-concatenates-commits.md` (renamed:
  the backlog item named the defect, the feature names the
  capability). Grounding read `phases/close.md`, the merge commit
  `0bc742a`, and PR #50's body — which is what redirected the design
  away from the item's own proposal. Current settings recorded for
  reversibility: `squash_merge_commit_message: COMMIT_MESSAGES`,
  `squash_merge_commit_title: COMMIT_OR_PR_TITLE`,
  `allow_squash_merge: true`, `delete_branch_on_merge: false`.
- **Open — OQ1.** `delete_branch_on_merge` is `false`, while
  close.md step 7 deletes the branch explicitly. The setting is
  therefore redundant rather than wrong, so it is left alone; raised
  here in case the reviewer wants it in scope.

## Decision Log

<!-- append: decision · rationale · alternatives rejected -->

- **The authored message is the fix; the settings are a floor.**
  Rationale: `0bc742a` proves the PR body is not the commit message
  in practice in this repo. Rejected: settings-only (the backlog
  item's proposal) — it swaps a bad default for a mediocre one and
  leaves the actual gap, which is that no module ever asked for an
  authored message.
- **`PR_BODY`, not `BLANK`, as the fallback.** Rationale: a wrong-but-
  substantive body is recoverable by reading around it; an empty one
  loses the *why* at the only point where correction means rewriting
  `main`. Rejected: `BLANK` (fails loud, but the loudness arrives
  after the commit is already permanent).
- **`PR_TITLE`, not `COMMIT_OR_PR_TITLE`.** Rationale: makes closure
  step 5's title rewrite authoritative on single-commit branches too.
  Rejected: leaving it — a one-commit branch would put `plan(...)` on
  `main` as a feature ship.
