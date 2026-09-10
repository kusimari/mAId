---
name: squash-message-concatenates-commits
description: The repository's squash-merge setting is COMMIT_MESSAGES, so every squash commit on main gets a concatenated transcript of the branch's commit messages instead of an authored summary. Each merge needs a manual override to avoid it.
metadata:
  type: backlog
---

# Squash merges concatenate commit messages onto `main`

## What

`allow_squash_merge` is on and `squash_merge_commit_message` is
`COMMIT_MESSAGES`. So a squash merge writes **every branch commit message**
into the commit on `main`, rather than a summary of what the feature does.

Setting it to `PR_BODY` would make the PR body the merge message, which is
already written for a reader who wasn't in the branch.

## Why it matters

`main`'s history is what someone reads years later with no context. A
concatenated transcript of "fix the assert", "rename the trailer", "revert
that" is the *branch's* narrative, not the feature's — and on a long branch
it buries the one paragraph that says what shipped.

It also leaks working detail that the branch deliberately keeps local. The
#50 merge had to pass an explicit `--subject`/`--body` to avoid writing a
three-commit transcript onto `main`; every future squash needs the same
manual override until the setting changes.

## Trigger to promote

Next time anyone touches repository settings, or the first time a squash
lands with a transcript because the override was forgotten.

## Notes

It is a repository setting, not code — one toggle, no diff. Left as a
backlog item rather than changed unilaterally because it affects every
contributor's merges.
