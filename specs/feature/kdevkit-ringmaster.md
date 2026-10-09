# Feature: kdevkit-ringmaster

## Git Setup

- Branch: feat/kdevkit-ringmaster
- Base: main (cd76b3d)
- Worktree: `../mAId-worktrees/kdevkit-ringmaster`

## Feature Brief

The user picks how to work: a single feature, an initiative they guide
feature by feature, or an initiative a ringmaster runs for them. For
an initiative the user writes what they want once (Goal, Experience,
Macro test, Constraints). Guided, they start and review each feature
(each with a PR into the initiative branch). Run by a ringmaster, the
session they start it from splits the work into features, runs each in
its own worktree session, plays the user at every feature gate, merges
finished features into the initiative branch, and stops only for
outward actions and real questions. Either way the initiative reaches
`main` as one merge commit with one commit per feature under it, and a
review briefing written for the whole.

## Handoff

- **Stage:** closed
- **Ready for:** nothing - shipped
- **Carry forward:** none. Open work is backlog
  `mechanize-session-confinement` and `skills-state-what-not-commands`.
- **Deliberately left:** a guided run, the guided-to-ringmaster switch
  and a resumed ringmaster (not yet exercised); the multi-repo guidance
  and the detailed cross-stream rebase steps from the old module; hosts
  without worktrees

### Crossings

<!-- Append one line per crossing; never edit or delete a line. -->

- planning → dev · EXCEPTION · skipping: the Planning Review Gate ·
  why: the user set the design in conversation and asked for it built
  in this session
- dev → review
- review → closure
- closure → closed

## Requirements

- The user picks how to work: a feature, a guided initiative (they
  start and steer each feature; each opens a PR into the initiative
  branch), or a ringmaster initiative. They can switch from guided to
  ringmaster mid-way.
- The verbs are said in an ordinary agent session; no wrapper.
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
- The initiative's PR body is a review briefing at initiative level:
  how the requirements were understood, the design decisions and how
  they changed the existing design, the major changes in review
  order, and what the tests say.
- Merged features keep their worktree and branch until the initiative
  closes; closing removes them all.

## Test Strategy

- `kdevkit-ringmaster.smoke` (playback, tri-tool): a feature session
  states the resident rules (whose cues, gates as stops, git limits,
  who merges where). Dry-run checked; paid run is the user's.
- Acceptance: the `installable` initiative run end to end from this
  session, in ringmaster mode. It proves the ringmaster loop, the
  initiative briefing, and worktrees kept until close.
- Not proven: guided mode and the guided-to-ringmaster switch (no
  guided run yet); the codex and kiro spawn paths (no model has run
  them; codex's sandbox profile was checked without one); the
  briefing's replay of the ringmaster's calls, added after the
  acceptance run; the `feat/<feature>/work` branch shape and the
  per-agent "how" lines from the third review (checked against each
  CLI's help and `codex sandbox`, not run by a model); "no wrapper"
  holds by construction (the verbs are prose the session acts on,
  with no script added).

## Design

- Every kdevkit file states what to do generically, then how per
  agent (`specs/project.md`, "It has to outlast its agents"): one
  "read into this session" definition, one per-agent dispatch list
  in `SKILL.md` §2, "forge" for the review tool and "agent" for the
  coding agent.
- `tiers/initiative.md` rewritten: the three modes and the switch
  from guided to ringmaster, the verbs (including the guided
  "start `<feature>` for initiative" and "close initiative"), the git
  shape, the ringmaster loop, Finish for both modes (Macro test,
  initiative-level briefing or the §9 body when `review_brief` is off,
  `close(<name>)`, worktree teardown), the record, the stop list.
- `phases/close.md` 3.5: a guided stream closes into the initiative
  branch itself; a ringmaster stream stops after its close commit.
- `interviews.md`: the initiative template (Goal / Experience / Macro
  test / Constraints / Streams / Decision Log) and its four
  interviews; a stream's spec takes `initiative/<name>` as base.
- `SKILL.md` (resident, so it outranks modules): the ringmaster's
  messages are the cues and gates are stops; the safety floor lets a
  feature session commit in its own worktree only; `close(<initiative>)`.
- `phases/close.md` 3.5: a stream stops after its close commit; the
  ringmaster merges into the initiative branch.
- `setup.md` and `SKILL.md` §2: the `## Active initiatives` index and
  its verify rule are gone; a session finds its initiative from the
  entry cue or the feature spec's `Part of initiative:` line.
- `specs/project.md`: the index section is gone; the Layout line
  matches.
- `kreviewkit`: an "At initiative level" section lifts each of its
  four sections from feature detail to the initiative's decisions.

## Implementation Plan

- [x] Module, template, resident rules, closure step.
- [x] Fixture, dry-run, `just ci`.
- [x] Review on #55: three modes with a guided verb, the index
      dropped, worktrees kept until close, an initiative-level
      briefing (kreviewkit).
- [x] Briefing defects fixed: closed-initiative lifecycle in every
      verify rule, `show initiatives`, the safety-floor exception, the
      resident merge destination, the initiative worktree, the merge
      message.
- [x] First real run: the `installable` initiative.

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

- 2026-10-08 · Acceptance: `installable` run in ringmaster mode from
  one session, with no user input between "run initiative" and the
  close-out. Five streams (one added mid-run when the paid Macro test
  found kiro reading the wrong path, one from the initiative briefing's
  defects), two in parallel; one RETURN (stream 1, a constraint
  breach); four planning and five review stops answered by the
  ringmaster with rulings in the initiative's Decision Log; the
  Macro test passed 26/26; the initiative briefing went from 2 defects
  to none. Fixes the run forced into this module: the brief names the
  worktree in every command, carries what earlier streams settled,
  loads skills by name with a path fallback (an agent can hold the
  skill list it started with); a stream proposes its squash message and
  answers closure's backlog step itself; a defect-list stream gets the
  planning cue in its brief. Not exercised: guided mode, the switch to
  ringmaster, a resumed ringmaster.

- 2026-10-09 · Closure. Persistent layers: `specs/project.md` already
  carries this feature's durable edits (Layout's `initiative/` line, the
  index section removed, "It has to outlast its agents" under Writing a
  skill); no further change, since the ringmaster's mechanics live in
  the skill, not the project. Backlog: none closed; two filed
  (`mechanize-session-confinement`, `skills-state-what-not-commands`).

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
  follows from keeping the spec as the record. Replaced: the index and
  both verify rules are gone (see the index ruling below).
- The initiative's merge commit message is authored and passed
  explicitly: `feat(<name>): <Goal in one line>`, body the Goal and
  one line per feature · §8.6's rule, applied to the one merge it did
  not cover.
- Three modes, chosen by the user · the user's review: kdevkit serves a
  plain feature, a guided initiative, and a ringmaster one. Guided
  features each get a PR into the initiative branch.
- The `## Active initiatives` index is dropped in every mode · the
  user's review; it served the agent's journaling, and the branch list
  plus the feature spec's link cover that. Replaces the earlier
  "index on the initiative branch" ruling.
- Feature worktrees and branches stay until the initiative closes ·
  the user's review; how well an agent runs as ringmaster is not yet
  known, so the evidence stays inspectable.
- The initiative PR body comes from the existing review-briefing role
  at initiative level · the user's review; considered a separate
  initiative summary, rejected as a second mechanism for the same job.
- Spawn and resume are named per host (Claude Code's Agent tool and
  `SendMessage`, `codex exec` / `codex exec resume`, `kiro-cli chat
  --no-interactive` / `--resume-id`) · the user's review: the module
  must work beyond Claude Code; flags checked against the installed
  CLIs.
- The initiative briefing replays the ringmaster's calls (rulings and
  each stream's closure answers) for the user to ratify · the user's
  review: a stream's closure answers are a human's calls made by the
  ringmaster, and the initiative PR is where the human sees them.
- Worktree confinement is enforced where the host can (codex's
  sandbox, a narrow permission profile checked without a model) and
  stays an instruction elsewhere, with backlog
  `mechanize-session-confinement` for Claude Code, kiro and the paid
  check · the user's review: mechanize what should not be an
  instruction; the earlier tooling attempt lost on phase adherence,
  which does not carry over to a safety boundary.
- kdevkit and kreviewkit state each rule generically, then how per
  agent, labelled and checked against the installed CLIs ("not known
  yet" where it is not): "inline-Read" becomes "read into this
  session", defined once; "subagent" becomes a dispatched agent;
  bare "host" becomes agent or forge · the user's review, applying
  project.md's "It has to outlast its agents".
- A stream's branch is `feat/<feature>/work`, cut from the initiative
  branch, in both modes · the user's review: the extra level gives
  each stream its own ref directory, so codex's sandbox grants that
  stream's branch only (checked with `codex sandbox`, no model);
  sibling streams, the initiative branch and new branches are now
  blocked, and only `/tmp` stays an instruction.
- Ship without a further initiative run, and keep the what-then-how
  refactor in this PR · the user's call at the third review: the
  branch shape and per-agent paths get their first run in the next
  initiative (backlog `mechanize-session-confinement`); the refactor
  rewords without changing behaviour.
