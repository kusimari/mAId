# Feature: squash-message-authored

## Git Setup

- Branch: `fix/squash-message-authored`
- Base: `main` @ `4da8a94`

## Feature Brief

A feature closed out by kdevkit lands on `main` as one commit whose
message was *written* — a subject naming the ship, a body explaining
why it exists — rather than assembled from whatever the branch
happened to commit along the way. Closure authors that message as part
of the merge; the repository default changes so a merge performed
without one degrades to the PR body instead of a commit transcript.

## Handoff

- **Stage:** dev
- **Ready for:** review, once the gates pass
- **Carry forward:** `0bc742a` next to PR #50 is the worked example of
  what step 6 has to ask for — the body on `main` is reflowed prose,
  the PR body is a review document. Read both before wording the rule.
  Second: the new fixture is **post-install only**, because
  `check-skills` cannot see a deferred module's prose.
- **Deliberately left:** `delete_branch_on_merge` stays `false` —
  close.md step 7 deletes branches explicitly, so the setting is
  redundant rather than wrong. Raised at the Planning Review Gate and
  not taken up.

### Crossings

<!-- Append one line per crossing; never edit or delete a line.
     A RETURN needs four parts: fault / issue / fix / done when.
     An EXCEPTION needs two: skipping / why. -->

- backlog → planning
- planning → dev

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
- **R4 — The rule is stated once**, at the merge step. Closure is the
  only phase that merges, so no other module restates it.

## Test Strategy

**Unit — none to add.** `just test` stays green as a no-regression
check; nothing under `resources/build-tool/` or `kaimux/` is touched.

**Functional — one new fixture,**
`resources/tests/skills/kdevkit-squash-message.smoke`, behavioral,
tri-tool. A transcript landing on `main` looks like a successful merge
and is only noticed years later by whoever reads history, which is the
silent-failure class `project.md` says earns a fixture.

Seed a scratch repo with a `main` and a feature branch of *three*
commits with transcript-flavoured subjects (`plan(...)`,
`feat(...)`, `fix: the assert`), plus a review-stage spec. Task: close
it out and squash-merge into the local `main`. Assert on `main`'s tip,
pairing each negative with a positive so a no-op agent fails:

- the body does **not** contain `fix: the assert` or the `plan(...)`
  subject — the transcript did not land;
- the body **does** carry prose — more than one line, a why-sentence
  rather than a list of subjects;
- the subject matches `feat(`, not `close(` or `plan(`.

**The fixture is post-install only.** `close.md` is a deferred module
and the pre-install stage carries only `SKILL.md`, so a `check-skills`
failure here would be reporting on the stage, not the rule. Agentic
runs stop at `just test` plus the free
`just resources::verify-skills-dry`; the paid command is the user's:

```
just resources::install-skills
just resources::verify-skills-one kdevkit-squash-message
```

Sample 3–5 runs per agent and record the ratio — adherence behavior is
probabilistic.

**Settings** — verified once by hand, recorded in the Session Log:

```
gh api /repos/kusimari/mAId \
  --jq '{squash_merge_commit_title, squash_merge_commit_message}'
```

Expected `PR_TITLE` / `PR_BODY`.

## Design

No source *is* the commit message — not the commit transcript, not the
PR body. It has to be authored, and kdevkit never said so. That gap is
the fix; the repository settings only decide what happens when nobody
authors one.

**The fix — `phases/close.md` step 6.** The squash merge carries an
authored message. Subject = the title the Closure Review Gate just
rewrote at step 5, which step 6 has never consumed. Body = why the
feature exists, for someone reading `main` with no other context: the
same *Why*-first discipline §9's Review Gates already impose on a PR
body, minus the parts that only mean something inside a review tool
(Reading order, the verification dump, cross-branch pointers). Step 6
is the only place this belongs — closure is the only phase that
merges — so `SKILL.md` needs no edit (R4).

**The floor — two repository settings.**

- `squash_merge_commit_message`: `COMMIT_MESSAGES` → `PR_BODY`
- `squash_merge_commit_title`: `COMMIT_OR_PR_TITLE` → `PR_TITLE`

`PR_TITLE` matters because the existing value uses the *commit's*
subject on a single-commit branch — often `plan(x): initial spec` —
which would put a planning subject on `main` as a feature ship,
ignoring the title step 5 deliberately rewrote (R2).

**Nothing to build.** `gh pr merge --squash --subject … --body …`
already takes both; the change is that the flags become the norm
rather than a remembered override. The settings are one
`gh api -X PATCH`.

## Implementation Plan

- [x] `phases/close.md` step 6 — the authored-message rule: subject
      from the step-5 title, body as a written why, and what to leave
      out relative to the PR body.
- [x] Grep the skill for anything that now restates or contradicts the
      rule (R4); reconcile rather than duplicate.
- [x] New fixture `resources/tests/skills/kdevkit-squash-message.smoke`
      per the Test Strategy, with the vacuity pairing.
- [x] `just test` + `just resources::verify-skills-dry` green.
- [ ] Flip the two repository settings via `gh api -X PATCH` — last,
      and record before/after in the Session Log.

- *Risk note:* the settings flip is the one step whose effect is
  outside this branch and outside git. Reverting the merge does not
  revert it, and it changes every contributor's merges — which is why
  the backlog item declined to do it unilaterally. Do it last; the
  before-values are in the Session Log so it is reversible by hand.
- *Risk note:* this would be the first fixture to write to a scratch
  `main` and read a merge commit back, and
  `specs/backlog/test-runner-workdir-containment.md` records an open
  question about how tightly the runner contains a fixture's workdir.
  If the merge is not expressible there, fall back to a `playback` on
  the rule and file the behavioral half.

## Session Log

<!-- append: date · what was done · decisions made -->

- **2026-09-12 · planning.** Promoted from
  `specs/backlog/squash-message-concatenates-commits.md`, renamed: the
  item named the defect, the feature names the capability. Settings
  recorded for reversibility — `squash_merge_commit_message:
  COMMIT_MESSAGES`, `squash_merge_commit_title: COMMIT_OR_PR_TITLE`,
  `allow_squash_merge: true`, `delete_branch_on_merge: false`.
- **2026-09-12 · dev.** R4 grep came back clean — nothing else in the
  skill states a merge-message rule, and `SKILL.md:511` ("the type
  encodes the on-branch narrative, not the on-`main` shape") reinforces
  it rather than competing. Step 6's single-commit exception needed a
  caveat it didn't have: a plain merge keeps that commit's own subject,
  which defeats R2.
- **2026-09-12 · dev · the assert block was probed before trusting
  it.** Extracted setup + assert and ran six wrong behaviours plus the
  right one against them, since a behavioral assert that can't fail is
  worth nothing. FAIL for: no-op, plain merge, git's default squash
  message, empty body, changelog body (the transcript reflowed as
  bullets), and a `close(` subject. PASS only for an authored message.
  The changelog case is why the assert greps for bare
  Conventional-Commits subjects line by line and not just the two
  seeded strings.

## Decision Log

<!-- append: decision · rationale · alternatives rejected -->

- **The authored message is the fix; the settings are a floor.**
  `0bc742a` next to PR #50 shows the PR body is not the commit message
  in practice here — the body on `main` is reflowed link-free prose,
  the PR body carries pass-rate tables, a branch-relative blob URL and
  a `## What to review` list. Rejected: settings-only (the backlog
  item's own proposal), which swaps a bad default for a mediocre one
  and leaves the real gap.
- **`PR_BODY`, not `BLANK`, as the fallback.** A wrong-but-substantive
  body is recoverable by reading around it; an empty one loses the
  *why* at the only point where correcting it means rewriting `main`.
  Rejected: `BLANK` — fails loud, but the loudness arrives after the
  commit is already permanent.
- **`PR_TITLE`, not `COMMIT_OR_PR_TITLE`.** Makes closure step 5's
  title rewrite authoritative on single-commit branches too. Rejected:
  leaving it — a one-commit branch would put `plan(...)` on `main`.
</content>
