# Feature: kaimux functional-manual walk-through

## Git Setup

- Branch: `feat/kaimux-manual-walk`
- Base: `main` @ `0eefca5`
- Worktree: `/local/home/gorantls/tool-workplace/ai-workspace/mAId-kaimux-manual`

## Feature Brief

Walk `kaimux/tests/functional-manual.md` end-to-end on a real
terminal, observing each step's behaviour against the doc's
"you should see" lines. Catch UX regressions the automated
F1–F8 + integration suite can't see — fzf rendering, keybind
feel, picker ergonomics, the side preview window, the
`enter`-to-jump round-trip — and reconcile any drift between
the doc and the current code (the doc predates the four-state
lifecycle and `--session NAME` override, so its glyphs +
session names may not match what ships today).

This is the human-driven UX validation layer that complements
the three automated test layers (unit / integration /
functional-automated). It's not a CI gate; it gates "v1.1
ship" quality.

## Requirements

R1. **Walk the doc step-by-step** on a real terminal. The
human attaches to the dashboard, drives the fzf picker
through real tmux + real claude + real kiro-cli, and observes
each step against the doc's "you should see" line.

R2. **Confirm the doc reads cleanly today.** Any drift between
the walk-through doc and the current code (state names,
glyphs, default session name, keybind UX, registry shape) is
itself a finding — the doc is part of the deliverable. The
doc predates PR #24's review-driven simplification (six-section
restructure, four-state lifecycle, `--session` override,
atomicwrites persistence), so drift is plausible.

R3. **No regression in the user-observable surface** — the
fzf picker renders cleanly, the `<prefix> <KEY>` round-trip
works, `Enter` jumps to the right pane regardless of layout
depth, the watcher keeps the list in sync without flicker.

## Test Strategy

This feature's "tests" are the human's observations. Evidence
captured as follows:

- **Decision Log entries per surfaced finding.** Every
  observation that diverges from "you should see" lands as a
  Decision Log entry on this spec. Format: brief reproducer +
  classification (doc drift / UX regression / code bug).
- **Bug-fix dev slices for code regressions.** A finding that
  reflects an actual code bug spawns a dev-loop slice (impl →
  Quality → Test → Code Review) on this same branch. New tests
  added to the appropriate layer (unit / integration /
  functional-automated) so the bug doesn't return.
- **Doc-fix commits for documentation drift.** A finding that
  is "doc says X, code does Y, code is right" gets fixed in
  `kaimux/tests/functional-manual.md` as a `docs(kaimux):`
  commit on this branch.

The walk itself is the smoke test; the Decision Log + bug-fix
+ doc-fix slices are the artefacts that show the walk
happened.

## Design

Three buckets of work, all on this one branch
(`feat/kaimux-manual-walk`):

1. **Walk** — exercise the doc as written. Capture each
   observation in the Decision Log. Don't fix anything yet;
   stay in observation mode through the full pass so we have
   the complete picture.

2. **Doc-fixes** — for findings classified as doc drift, edit
   `kaimux/tests/functional-manual.md` so the next person
   walking it sees correct behaviour. One `docs(kaimux): ...`
   commit per coherent doc-fix unit.

3. **Code-fixes** — for findings classified as real
   regressions, run a §7 dev-loop slice (Quality + Test +
   Code Review gates) on this branch. One `fix(kaimux): ...`
   commit per bug. Add a test to the appropriate layer so the
   bug stays fixed.

All three buckets land in the same PR so reviewers see the
full set of findings + their resolutions together. Closure
(§8) reconciles the doc with the final code state and ships.

## Implementation Plan

Hybrid: per-step walk-through items (granular, matching the
doc's seven steps) followed by reconcile items (coarser,
scope-dependent on what the walk surfaces).

### Walk

- [ ] Pre-flight: `just kaimux::build`, confirm `claude`,
      `kiro-cli`, `fzf`, `jq`, `tmux ≥ 3.2` are on PATH.
- [ ] Step 1 — three project sessions, mixed layouts (proj-a
      single Claude; proj-b 2-window with split + Claude in
      window 2 left; proj-c vertical split with Kiro top +
      Claude bottom). Submit a quick prompt to each so they
      move off `done`.
- [ ] Step 2 — open the dashboard via bare `kaimux` from the
      viewer session; observe the picker layout, sort order,
      and live update as agents transition state.
- [ ] Step 3 — Pick + switch: `Enter` jumps to the exact pane
      regardless of split depth. Verify proj-b's Claude lands
      on `proj-b:code`'s left pane, proj-c's Claude on the
      bottom pane, proj-c's Kiro on the top pane.
- [ ] Step 4 — Round-trip via the `<prefix> <KEY>` keybind
      (whichever key was passed to `setup --key X`). Picker
      survives the trip with cursor + query state intact.
- [ ] Step 5 — Kiro orphan-file cleanup: pre-place a bogus
      `<cwd>/.kiro/agents/kaimux.json`, exit Kiro, observe
      orphan removed (sibling-protection case if 2 Kiros in
      same cwd).
- [ ] Step 6 — Dead-pid filtering: kill -9 a wrapped agent's
      pid, observe the row drops from the picker.
- [ ] Step 7 — Clean up: exit each agent, kill sessions,
      `kaimux teardown`, verify `~/.claude/settings.json`
      cleaned up.

### Reconcile

- [ ] Open Decision Log entries for each finding from the
      walk; classify each as **doc drift** / **UX regression**
      / **code bug**.
- [ ] Doc-fix slice(s): edit `kaimux/tests/functional-manual.md`
      for every doc-drift finding. One `docs(kaimux):` commit
      per coherent unit.
- [ ] Code-fix slice(s): one `fix(kaimux):` dev-loop slice per
      real bug found, with a regression test added to the
      appropriate layer.
- [ ] Closure verification: walk the **fixed** doc end-to-end
      one more time. Should pass cleanly with zero new
      findings.

### Known drift candidates (to validate during the walk)

These are the spots where the doc most likely lags the code
post-PR-#24:

- **State glyphs + lifecycle**. Doc may name 3-state
  `unknown → running → complete` with `▶ ✓ ·`. Current is
  4-state `Working / Waiting / Done / Idle` with `▶ 💬 ✓ ·`.
- **Session names**. Doc names the dashboard session
  `orchestrator` in some places; current default is `kaimux`
  (overridable via `--session NAME`).
- **Keybind name**. Doc names `M-o` (Alt-o) as the keybind;
  current `setup --key X` accepts any single-char prefix-table
  suffix, so the doc's `M-o` is one example, not a contract.
  The Notification (`💬` waiting) state is new since the doc
  was written.
- **Picker row shape**. Doc describes single-line rows
  (`<glyph> <kind> <cwd-tail> · <prompt>`). Current is
  multi-line with `<icon> <addr>\t<kind>\t<cwd>\t<elapsed>` +
  3-line snippet, NUL-separated.
- **Kiro behaviour**. Doc may describe the old project-config
  write path; current is observation-only with orphan cleanup.

## Decision Log

<!-- Newest at top. Findings from the walk land here. -->

## Session Log

<!-- Newest at top. Update after each unit of work; don't batch. -->

- 2026-06-22: Spec promoted from
  `specs/backlog/kaimux-functional-manual-walkthrough.md` via
  `git mv`, body rewritten around the existing What / Why to
  add Requirements / Test Strategy / Design / Implementation
  Plan per kdevkit §6. Branch `feat/kaimux-manual-walk` cut
  from `main` @ `0eefca5`.
