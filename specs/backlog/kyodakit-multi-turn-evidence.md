---
name: kyodakit-multi-turn-evidence
description: Only one kyodakit fixture shows a gain over no skill (review feedback, 9/9 vs 3/9). The checkpoint, hook and real-files rules pass without the skill on one-prompt tasks. Measuring them needs multi-turn tests, and the all-rules recital is too noisy to steer trimming.
metadata:
  type: backlog
---

# kyodakit — evidence that its other rules help

## What

With `--control`, agents pass the checkpoint, tool-hook and test-isolation
fixtures without kyodakit, even after they were made harder. The failures
these rules came from happened in long sessions with many turns, which a
single prompt does not recreate. Two pieces:

- A multi-turn fixture shape in the runner: a scripted sequence of user
  turns against one agent session, asserted at the end.
- A less noisy recital score: it swung about ±3 of 9 between runs, so it
  could not tell a small wording cut from luck at 3 repeats.

## Why

Without this, kyodakit's value is proven for one rule out of six, and any
trim of it can only be guarded by the feedback test.

## Trigger to promote

The next change to kyodakit's rules, or the next attempt to trim it.
