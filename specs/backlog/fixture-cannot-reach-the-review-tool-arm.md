---
name: fixture-cannot-reach-the-review-tool-arm
description: 'Behavioral fixtures seed a bare scratch repo, so any skill rule conditioned on a review tool being present is unreachable: the fixture can only ever test the no-forge branch. Surfaced by squash-message-authored, where "the subject is the step-5 title verbatim" has no arm.'
metadata:
  type: backlog
---

# Backlog: a fixture cannot reach a rule's review-tool branch

## What

`--- setup ---` seeds a scratch git repo with `git init`. It has no
remote, no PR/CR, and no review tool, and the enact task usually says
so explicitly to stop the agent reaching for one.

So any skill rule with two branches — one where a review tool exists,
one where it doesn't — can only ever be tested on the second. The
first is recitation-only.

Concretely, from `squash-message-authored`: `close.md` step 6 says the
squash subject is "the title step 5 just rewrote, **verbatim**" where a
review tool is in play, and derived from the branch's dev commits where
there is none. Only the second has a behavioural check. The first is
the branch that fires in every real repo, including this one.

A related consequence in the same fixture: the "don't paste the review
body wholesale" assert is a **tripwire for a case the seed cannot
create**, since no review body exists there to paste. It is labelled as
such in the fixture, but it means that half of the rule is unverified
too.

## Why it matters

The unreachable branch is usually the *common* one — a repo with a
forge is the normal case and the bare-repo path is the exception. So
the arm that can't be tested is the arm that matters, and a rule can
ship with its main path proven only by reading.

## Trigger to promote

Next time a skill rule branches on review-tool presence and the
behavioural half is judged load-bearing, or next time someone reaches
for a fixture that needs a PR to exist.

## Open questions

- Can a fixture seed something the skill accepts *as* a review surface
  without a network — a local bare repo as `origin`, plus a file the
  task names as the PR body? That covers "read the title from
  somewhere" without a forge, but not `gh`-shaped interaction.
- Is a fake `gh` on `PATH` in the fixture's workdir acceptable, or does
  that test the fake rather than the skill?
- Cheaper alternative: accept the gap permanently and require such
  rules to state their branches in a `playback` narrative, so at least
  the recitation is scored. Note that a `playback` arm is pre-install
  and carries only `SKILL.md`, so this does not work for a rule living
  in a deferred module — see `content-stage-typed-skill.md`.
