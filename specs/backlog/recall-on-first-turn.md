# Backlog: recall-on-first-turn

## What

Recall reaches the first turn of a task a memory bears on: a persona
habit ("never use awk in scripts") should reach the first shell question.

## Why

In a real-model Macro run the selector missed a saved habit on the first
turn of a shell question, and the agent suggested what the habit forbids.
The v1 selector is a word lookup; the prompt's words did not match the
memory's.

## Open questions

- Widen the lookup (the memory's description and tags, the turn's fitting
  bring-ins), or move to the local model the selector interface is built
  for?
