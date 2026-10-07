---
name: kyodakit-per-prompt-injection
description: Re-inject the developer-judgement rules on every prompt, as ponytail and oh-my-opencode do with hooks, if long real sessions show kyodakit's rules slipping between kdevkit's phase-start reads.
metadata:
  type: backlog
---

# kyodakit — inject the judge on every prompt

## What

kdevkit reads kyodakit at session start and with every phase module. That
is a periodic re-read, not a per-turn one. ponytail appends its rules to
the system prompt every turn through a hook; oh-my-opencode injects rules
the same way. mAId has no hook mechanism yet, so this would be a new kind
of deployed resource, beside the skill symlinks.

## Why

Under `--drift` (~4,000 words between rules and task) the rules held. Real
sessions run far longer, and a one-prompt runner cannot show what happens
on turn 80. Deferred by the user until evidence says it is needed.

## Trigger to promote

A real session where kyodakit's rules visibly slip after the phase-start
read, or a multi-turn test showing it (see
`kyodakit-multi-turn-evidence`).
