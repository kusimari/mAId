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

- **Stage:** review
- **Ready for:** closure on the cue. Gates are green; the repository
  settings are already live and are not reverted by reverting this
  branch.
- **Carry forward:** the fixture's positive half took four attempts and
  the failure was the same each time — I kept tuning a threshold for a
  property a threshold cannot express. If a future change touches it,
  discount boilerplate, don't raise a floor. Also: cycle 4's seven notes
  were fixed *after* the gate passed, so those fixes carry no gate
  verdict; the 27-case probe is the only evidence for them.
- **Deliberately left:** three things, all named in the fixture's own
  comments rather than only here. (1) Whether a body is a *why* or a
  reworded *what* is not deterministically checkable — the prefixed
  form is caught, the reworded form is not, and nothing else covers it.
  (2) The review-tool arm is unreachable in a scratch seed with no
  forge, so "the subject is the step-5 title *verbatim*" has no
  behavioural check. (3) `delete_branch_on_merge` stays `false`; step 7
  deletes branches explicitly, so the setting is redundant. Raised at
  the Planning Review Gate and not taken up.

### Crossings

<!-- Append one line per crossing; never edit or delete a line.
     A RETURN needs four parts: fault / issue / fix / done when.
     An EXCEPTION needs two: skipping / why. -->

- backlog → planning
- planning → dev
- dev → review · EXCEPTION · skipping: a gate verdict on the last fix
  round · why: the Code Review Gate passed at cycle 4 (PASS WITH NOTES,
  `fail_on: high`), and its seven notes were then fixed; those fixes are
  covered by the 27-case probe but by no review cycle. Budget was
  already overridden once to reach cycle 4.

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

Seed a scratch repo with a `main` and a feature branch of *four*
commits with transcript-flavoured subjects (`plan(...)`, `feat(...)`,
`fix: the assert`, `close(...)`), plus a reconciled spec. The
`close(...)` tip is load-bearing: it is the subject nearest to hand at
merge time and the one step 6 forbids reaching for. Task: squash-merge
into the local `main`. Assert on `main`'s tip, pairing each negative
with a positive so a no-op agent fails:

- the body does **not** contain `fix: the assert`, `plan(...)` or
  `close(...)` — the transcript did not land;
- the body **does** carry something the agent authored — host
  boilerplate (trailers, attribution, URLs) discounted first, then a
  surviving line must carry letters. **No length rule**: `close.md`
  sets none, so a floor is a rule the skill doesn't carry;
- the body is not a pointer to the history the squash is collapsing;
- the subject matches `^feat[(:]` — scope optional — and so is neither
  `close(` nor `plan(`;
- `main`'s *tree* carries `due.py` — otherwise the assert reduces to
  "the tip has a nice message", which an empty commit satisfies;
- no `Reading order` / `Read for intent:` / `## Verification`, and no
  *link* to the doomed branch.

**Whether a body is a why rather than a reworded what has no coverage
— deterministic or judged.** The assert catches the changelog form
that keeps its Conventional-Commits prefixes and nothing more. The
`playback` arm does **not** close this: it asks a different question in
a different run and never sees the body the merge wrote. It is a
recitation arm, and it is pre-install, so it cannot see `close.md`
either. The hole is accepted, not covered.

**The fixture is post-install only.** `close.md` is a deferred module
and the pre-install stage carries only `SKILL.md`, so a `check-skills`
failure here would be reporting on the stage, not the rule. Agentic
runs stop at `just test` plus the free
`just resources::verify-skills-dry`; the paid command is the user's:

```
just resources::install-skills
just resources::verify-skills-one kdevkit-squash-message
```

**Only the `integration` arm can pass.** `playback` and `enact` both
run pre-install, where the prompt carries `SKILL.md` alone, so neither
can see a rule that lives in `phases/close.md` — expect both red, and
don't read it as a regression. To run only the informative arm, the
Just verb has no `--kind` pass-through; go to the binary:

```
cargo run -p build-tool --release -- verify kdevkit-squash-message --kind integration
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
merges.

R4 turned out to need more than that. Stating the rule once is not the
same as leaving no *competing* claim: both `SKILL.md` §5 and step 5
credited the closure title rewrite with making `main` read as a feature
ship, which is the mechanism this change stops relying on. Both are
corrected to say the rewrite *supplies the subject* step 6 passes.

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
- [x] Flip the two repository settings via `gh api -X PATCH` — last,
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
- **2026-09-12 · dev · settings applied**, with the user's explicit
  go-ahead since the effect is outside git and hits every
  contributor's merges. `squash_merge_commit_message`
  `COMMIT_MESSAGES` → `PR_BODY`, `squash_merge_commit_title`
  `COMMIT_OR_PR_TITLE` → `PR_TITLE`. Confirmed by a fresh read, not
  just the PATCH response. To revert:
  `gh api -X PATCH /repos/kusimari/mAId -f
  squash_merge_commit_message=COMMIT_MESSAGES -f
  squash_merge_commit_title=COMMIT_OR_PR_TITLE`.
- **2026-09-12 · dev · Code Review Gate cycle 1: FAIL**, three must-fix.
  My own probe had found none of them, and the reason is instructive —
  it tested the behaviours I had thought of. Reproduced all three
  independently before fixing.
  - **M1, a false failure.** The body had to occupy ≥2 non-blank
    lines, but `git commit -m <subject> -m <why>` writes the why as one
    unwrapped line and close.md never asked for a wrap. A fully
    compliant 33-word why failed. The line count is gone; word count
    alone carries non-vacuity, lowered to 15 so a terse why passes.
  - **M2, the assert never checked the merge happened.** It read
    main's tip *message* and a commit count, both of which
    `git commit --allow-empty` satisfies with main's tree untouched —
    and a squash of `feat/add-due-dates~2` passes too, shipping the
    spec without `due.py`. Now `git cat-file -e main:due.py`. M2 was
    masked in my probe by M1: my probe used the two-`-m` form, so it
    died on the line count before reaching the real hole.
  - **M3, the anti-changelog check was narrower than its comment
    claimed.** It only fires on lines that keep a Conventional-Commits
    prefix, so the transcript reworded into prose bullets passed while
    the comment told the next reader the hole was closed. No
    deterministic check separates a why from a reworded what, so the
    comment now records the residual gap and a `playback` arm carries
    the judgement.
  - Also fixed: the subject was defined only by back-reference to step
    5's PR title, which does not exist in a no-review-tool world (S1);
    the host-default paragraph could be read as a substitute for
    passing a message and instructed a repo-wide settings change with
    no surfacing (S2); "a plain merge keeps that commit's own subject"
    is true of fast-forward only — `--no-ff` lets you supply one and
    leaves two commits (S3); a dead subject check after `^feat(` had
    made it unreachable (S4); two comments were factually wrong (S6).
  - **A body that is the review brief pasted verbatim also passed**,
    which is the *new* half of the Body rule. Now asserted directly:
    `Reading order`, `Read for intent:` and a `## Verification`
    heading may not appear on `main`.
- **2026-09-12 · dev · probe re-run, 13 behaviours, all correct.**
  FAIL for no-op, `--no-ff`, fast-forward, git's default squash
  message, empty body, `close(` subject, `--allow-empty`, partial
  squash, prefixed changelog, review brief verbatim. PASS for the
  one-line why, a terse 2-line why, and a wrapped why.
- **2026-09-12 · dev · Code Review Gate cycle 2: FAIL.** M2 and M3
  verified closed. But the 15-word floor I put in place of M1's line
  count **reproduced M1's own class** — an 8-word compliant why
  ("Users could not tell which to-dos were urgent.") failed. `close.md`
  sets no length, so any floor above "not empty" is a rule the skill
  doesn't carry. Now 4 words, which is the job the comment claims and
  nothing more.
  - **And I made the wrong-comment mistake a second time.** M3's fix
    said "the playback arm carries that judgement" — it does not.
    Playback asks a different question in a different run and cannot
    see the body the enact run wrote. The reworded-changelog hole has
    no coverage at all, deterministic or judged. The comment now says
    that plainly. Leaving a hole open is fine; claiming it's covered
    is what keeps the next reader from looking.
  - `%b` is empty when an agent writes the why with no blank separator
    — git folds the paragraph into `%s` — so a body that *was* written
    was diagnosed as missing. Now `%B` minus the first line.
  - Setup seeded no `close()` commit, so the prose's sharpest
    prohibition was only tested if the agent volunteered the
    temptation. The branch tip is now
    `close(add-due-dates): reconcile spec and clear handoff`, which is
    also in git's default `SQUASH_MSG`, so it earns a third transcript
    negative. The task changed with it: reconcile is seeded, the merge
    is what's asked.
  - Also: the branch-link exclusion had no assert (added); `^feat(`
    rejected a scopeless `feat:` that Conventional Commits allows
    (now `^feat[(:]`); the merge-count comment cited numbers that only
    hold for an agent doing no closure work; the review-brief regex
    can't fire in this seed and now says it's a tripwire; the playback
    `expect:` had none of the negative calibration its four siblings
    carry; and the single-commit bullet sat under `Exceptions:` while
    prescribing the default, so a label-skimming agent read it as
    "don't squash".
  - Portability: the reseed avoids `sed -i` (differs GNU/BSD) and
    `python3` (not a dependency of this suite).
- **2026-09-12 · dev · probe re-run, 18 behaviours, all correct.** New
  cases: authored subject with git's default body, `fix:` subject,
  `close()` subject, partial squash, branch link in body, scopeless
  `feat:`, missing blank separator, and two whys shorter than the old
  floor.
- **2026-09-12 · dev · Code Review Gate cycle 3 (budget overridden):
  FAIL.** The 4-word floor was **vacuous**: `🤖 Generated with [Claude
  Code](…)` is 5 words, and claude — which appends that footer by
  default — is the first of the three agents this fixture runs. So the
  fixture proved the *host's default message* didn't land, not that a
  message was *authored*. `See the branch history.` (4 words) passed
  too, which is the anti-pattern in its purest form: deferring the why
  to exactly the history the squash is collapsing.
  - **The three attempts are the finding.** 2 lines → false failure;
    15 words → false failure; 4 words → vacuous. A word count cannot
    express "a why exists", and each round I re-tuned the threshold
    instead of changing the tool. The check now **discounts host
    boilerplate** — blank lines, git trailers, attribution and URL
    lines — and requires something to survive, plus a named rejection
    of history-pointer bodies. No threshold at all.
  - Also: the commit-count comment enumerated the causes of a wrong
    count, but a correct squash followed by one more commit on `main`
    also trips it — enumeration dropped. And the branch-name check
    rejected any *mention* while `close.md` forbids only *links*, so
    the fixture was stricter than the prose it tests; now matches only
    URL and markdown-link shapes.
- **2026-09-12 · dev · probe re-run, 23 behaviours, all correct.** Both
  cycle-3 cases now fail, and the false-failure directions pass: a
  3-word why, a bare branch name beside a real why, and a why followed
  by an attribution footer.
- **2026-09-12 · dev · Code Review Gate cycle 4: PASS WITH NOTES** —
  the gate passes at `fail_on: high`. Seven notes, all fixed:
  - Two were my recurring false-failure class. Dropping any URL-bearing
    *line* rejected a compliant one-line why that cited a URL, so the
    URL is now stripped from the line instead. And the history-pointer
    regex had every qualifier optional, so it reduced to `see
    *(history|log)` and rejected "users could not see the history of
    what was due" — it now requires `see` to open a sentence.
  - **The linked history pointer escaped by construction.** The pointer
    check ran on the URL-filtered text, so `See the full commit log:
    https://…` — the worst form of the anti-pattern — was invisible.
    It runs on the raw body now.
  - The changelog check was case-sensitive, so `* Feat: add a --due
    flag` passed while the comment called that "reworded". One `-i`.
  - `test -n` passed on `---`, a lone emoji and bare `Refs:`/`Closes:`.
    Now requires a surviving line to carry letters, which keeps the
    no-threshold property.
  - **My cycle-1 R4 grep was wrong.** `SKILL.md` still said the title
    rewrite happens "so the squash-merge commit on `main` reads as a
    feature ship" — which credits the rewrite with the outcome, exactly
    the mechanism this change forbids relying on. An agent answering
    from `SKILL.md` alone rewrites the title, runs the host merge with
    no message, and lands the transcript. I read that line as
    consistent in cycle 1 and it was load-bearing.
  - The setup comment claimed all four sibling playbacks share this
    one's unanswerable-pre-install position; two don't.
- **2026-09-12 · review · briefing generator returned defects twice**,
  so no briefing was published either time — its contract treats a
  defect as proof the loop isn't complete.
  - **Round 1, five.** The history-pointer check anchored `see` to line
    start or sentence punctuation, so `- See the branch history.` — the
    cycle-3 case the check exists to reject — passed the whole assert;
    the changelog check had the same gap for numbering. `close.md` step
    5 still carried the causal claim I corrected in `SKILL.md` at cycle
    4, three lines above the rule contradicting it. The spec claimed a
    judged `playback` arm covered the why-vs-what question, which the
    fixture comment had already retracted mid-dev — the overclaiming
    sin, in the spec this time. Plus five stale Test Strategy /
    Design statements, and a `--kind` scoping that can't reach through
    the Just verb. And step 6 had no answer for a closure-only branch,
    which `4da8a94` on `main` shows is not hypothetical.
  - **Round 2, two, one of them mine from round 1.** The closure-only
    carve-out I added to `close.md` was not mirrored into the fixture's
    `expect:` narrative, so a correct recitation was described by the
    fixture as a wrong answer. **Third instance of the same twin-pair
    shape** — `SKILL.md` vs step 5, step 5 vs step 6, now step 6 vs the
    fixture that recites it. Also: the attribution filter was
    unanchored, so a compliant why containing the ordinary-English
    "generated with" was discounted to nothing and failed — the
    false-failure class reappearing *inside* the fix that replaced the
    threshold, which is exactly what *Carry forward* warns about.
- **2026-09-12 · dev · probe re-run, 27 behaviours, all correct.**

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
- **The deterministic assert closes the prefixed changelog; a judged
  `playback` arm carries the rest.** `grep` cannot tell a why from a
  reworded what, and a heuristic that tried (reject bodies with
  bullets) would reject compliant prose. Rejected: leaving the
  assert's comment claiming the hole was closed — a check whose comment
  overstates it is worse than the gap, because the next reader stops
  looking.
- **Superseded: the `playback` arm carries nothing.** It asks a
  different question in a different run and never sees the body the
  merge wrote, and it is pre-install, so it cannot read `close.md`
  either. Whether a body is a why rather than a reworded what has **no
  coverage anywhere**, deterministic or judged. The hole is accepted;
  the entry above was the same overclaiming it warns against, one layer
  up.
- **`PR_TITLE`, not `COMMIT_OR_PR_TITLE`.** Makes closure step 5's
  title rewrite authoritative on single-commit branches too. Rejected:
  leaving it — a one-commit branch would put `plan(...)` on `main`.
</content>
