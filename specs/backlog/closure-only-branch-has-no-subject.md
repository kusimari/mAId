---
name: closure-only-branch-has-no-subject
description: 'A branch whose only commits are closure has no dev-phase subject, so closure step 5 has nothing legal to rewrite the title to and step 6 has nothing to source a squash subject from. Pre-existing; surfaced by the authored-squash-message work.'
metadata:
  type: backlog
---

# Backlog: a closure-only branch has no subject to rewrite to

## What

`phases/close.md` step 5 rewrites the PR title to "the dominant
agent-dev subject (`feat(<scope>): subject` etc.) — *not* the
`close(<feature>):` subject". Step 6 then sources the authored squash
subject from that title.

On a branch whose commits are **all** closure — a stream that closes
without shipping code — there is no dev-phase subject. Step 5 is
required to rewrite the title to something that does not exist, and is
forbidden from using the only subject the branch has.

Not hypothetical: `4da8a94` on `main` is
`close(kdevkit-crossings-record): close out the deterministic-phasing
initiative (#51)`, a branch whose whole content was closure, merged
through a review tool.

## Why it matters

Step 6 now *demands* an authored subject, so the undefined case is
reachable in a way it wasn't before — an agent hitting it either stalls
or picks something and doesn't say so. Small blast radius (a rare
branch shape), but it is an instruction with no legal answer.

## Trigger to promote

Next time closure runs on a branch with no dev commits, or next time
anyone edits step 5's title rule.

## Notes

A carve-out was written into step 6 during `squash-message-authored`
("use the `close(<feature>):` subject there") and then **removed**. Two
reasons, both recorded in that spec's Decision Log: it sat outside that
feature's four requirements, and it contradicted step 5 three lines
above — the two could not both be obeyed, which is worse than the gap.
It was also unreachable on the path its own worked example lives on,
since step 6 takes step 5's title verbatim wherever a review tool
exists, which is kdevkit's default.

Fixing this properly means **step 5 and step 6 changing together**, so
title and merge subject agree on every branch shape. That is the whole
of the work; the step 6 half alone is what was tried and reverted.
