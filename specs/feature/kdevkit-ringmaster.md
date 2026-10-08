# Feature: kdevkit-ringmaster

## Git Setup

- Branch: feat/kdevkit-ringmaster
- Base: main (cd76b3d)
- Worktree: `../mAId-worktrees/kdevkit-ringmaster`

## Feature Brief

An initiative runs without the user holding its hand. The user writes
what they want once (Goal, Experience, Macro test, Constraints) and
approves it. The session they run it from becomes the ringmaster: it
splits the work into features, runs each in its own worktree session,
plays the user at every feature gate, merges finished features into
an initiative branch, and stops only for outward actions and real
questions. The initiative reaches `main` as one merge commit with one
commit per feature under it.

## Handoff

- **Stage:** dev
- **Ready for:** review, and proof by a first real run
  (`installable`)
- **Carry forward:** the pre-install skill stage carries only
  `SKILL.md`, so the module's behaviour is proven by running an
  initiative, not by the playback fixture
- **Deliberately left:** the multi-repo guidance and the detailed
  cross-stream rebase steps from the old module (folded into one line);
  hosts without worktrees

### Crossings

<!-- Append one line per crossing; never edit or delete a line. -->

- planning → dev · EXCEPTION · skipping: the Planning Review Gate ·
  why: the user set the design in conversation and asked for it built
  in this session

## Requirements

- "start initiative `<name>`" interviews the user for Goal,
  Experience, Macro test and Constraints only, then stops for
  approval.
- "run initiative `<name>`" plans the features, runs them, and plays
  the user at every gate without asking the user, except for outward
  or irreversible actions, questions that change the Experience,
  three stuck rounds, or spend over the Constraints.
- A feature session under a ringmaster treats the ringmaster as its
  user, commits only in its worktree, and never pushes, opens a PR or
  merges.
- The record is git log, the initiative spec and the feature specs.
  A new session resumes an initiative from them alone.
- `main` gets the initiative as a merge commit; `--first-parent`
  shows the initiative, its second parent the features.

## Test Strategy

- `kdevkit-ringmaster.smoke` (playback, tri-tool): a feature session
  states the resident rules (whose cues, gates as stops, git limits,
  who merges where). Dry-run checked; paid run is the user's.
- Acceptance: the `installable` initiative run end to end from this
  session.

## Design

- `tiers/initiative.md` rewritten around the ringmaster: split of
  work, verbs, git shape, the loop, the record, the stop list.
- `interviews.md`: the initiative template (Goal / Experience / Macro
  test / Constraints / Streams / Decision Log) and its four
  interviews; a stream's spec takes `initiative/<name>` as base.
- `SKILL.md` (resident, so it outranks modules): the ringmaster's
  messages are the cues and gates are stops; the safety floor lets a
  feature session commit in its own worktree only; `close(<initiative>)`.
- `phases/close.md` 3.5: a stream stops after its close commit; the
  ringmaster merges into the initiative branch.
- `setup.md` and `SKILL.md` §2: the index line lives on the initiative
  branch; a file with no index line is a closed record, not drift.
- `specs/project.md`: the index comment and Layout line match.

## Implementation Plan

- [x] Module, template, resident rules, closure step.
- [x] Fixture, dry-run, `just ci`.
- [x] Briefing defects fixed: closed-initiative lifecycle in every
      verify rule, `show initiatives`, the safety-floor exception, the
      resident merge destination, the initiative worktree, the merge
      message.
- [ ] First real run: the `installable` initiative.

## Session Log

- 2026-10-08 · Prior art. Wrappers (multi-session TUIs and worktree
  managers) run agents from outside; rejected. Superpowers'
  subagent-driven development is skills-only: a controller dispatches
  fresh subagents with file briefs, reviews between tasks, never fixes
  code itself, logs rulings instead of asking, and stops only for
  destructive, security, outward or hopeless cases; its worktree skill
  says use the harness's own tool first. Claude Code's Agent tool
  runs background subagents, resumes them with `SendMessage`, and
  nests three deep. The ringmaster takes Superpowers' shape and
  kdevkit's existing gates and records.
- 2026-10-08 · Git grouping, checked in a scratch repo: squash each
  feature into `initiative/<name>`, `git merge --no-ff` into `main`;
  `git log --first-parent main` shows one line per initiative and
  `main^1..main^2` its features. The repo allows merge commits.

## Decision Log

- Initiative branch, merged with a merge commit, not squashed · the
  user's call; keeps one commit per feature grouped under the
  initiative.
- Worktrees in the project's convention, made by the ringmaster, not
  the harness's isolation · the user's convention; harness worktrees
  branch from `main`, not the initiative branch.
- Paid tests allowed within the initiative's Constraints · the user's
  call; not every project has skill tests.
- No new journal (no beads-style tracker) · git log and the specs
  already hold it.
- The initiative spec stays after `close(<initiative>)` · same as
  feature specs; the old rule deleted it.
- `show initiatives` lists `initiative/*` branches, not `main`'s index
  · the index line lives on the initiative branch and is removed
  before the merge, so `main` never carries it; considered putting it
  on `main`, which means a commit to `main` per initiative start.
- A closed initiative's file with no index line is not drift · it
  follows from keeping the spec as the record; the verify rule in
  `SKILL.md` §2 and `setup.md` both say so.
- The initiative's merge commit message is authored and passed
  explicitly: `feat(<name>): <Goal in one line>`, body the Goal and
  one line per feature · §8.6's rule, applied to the one merge it did
  not cover.
