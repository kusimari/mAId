---
name: kaimux-functional-manual-walkthrough
description: Walk kaimux/tests/functional-manual.md end-to-end on a real terminal — the human-driven UX validation that complements the automated F1–F8 + integration suite.
metadata:
  type: backlog
---

# Kaimux functional-manual walk-through

## What

Walk `kaimux/tests/functional-manual.md` end-to-end on a real
terminal — i.e. attach to `kaimux-test-dashboard`, drive the
fzf picker, exercise `<prefix> O` keybind round-trips, peek
into pane snippets, kill an agent the way a user actually
would. The doc names every step.

## Why deferred

Both PRs that shipped the kaimux design (PR #24 and PR #26)
covered the automated test layers (unit / integration /
functional-automated). The manual walk-through is what
catches UX regressions that the automated assertions can't —
fzf rendering quirks, keybind feel, picker ergonomics with N
agents, the side preview window, the `enter`-to-jump round-
trip. None of the automated layers attaches the user to a
real terminal with a real fzf running.

The actual code is unlikely to surprise — F1–F8 cover the
state machine + render output deterministically — but UX
issues like "the picker flickers when an agent is mid-tool"
or "the preview pane bleeds into the row above" are only
visible when a human watches.

## When to do this

Before any UX-affecting change to kaimux ships. The manual
walk-through is not a CI gate but should run as part of any
"v1.1 ship" or "redesigned picker rows" work.

## How

```sh
# In a real terminal, NOT inside tmux:
just kaimux::build
# Walk the doc step-by-step:
$EDITOR kaimux/tests/functional-manual.md
```

The doc is itself the script — there's no automation here on
purpose. Each step has a "you should see" line for the human
to check against.

## Provenance

Deferred from PR #24 (kaimux design land) and PR #26 (kiro
observation-only). Both PRs shipped without this walk-through
on the strength of the automated layers. The only PR that
required this would be one that touches fzf flags, the
side-preview shape, the keybind action chain, or the picker's
event-driven update loop.
