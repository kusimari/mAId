# Making phase transitions reliable in kdevkit

Branch: `feat/kdevkit-deterministic-phasing`

## What this document is

A proposal to fix one specific bug in how kdevkit tracks its own
progress. It is written to be read start to finish. It covers what the
problem is, what we measured, what we tried and rejected, the design we
arrived at, how we will know it works, and how to build it.

Nothing has been built yet. The point of this document is to get the
design wrong on paper rather than in code.

## Background: what kdevkit is

kdevkit is a set of instructions that guides an AI coding agent through
building a feature. It is not a program. It is markdown files that the
agent reads, in the same way a new team member might read a
CONTRIBUTING guide — except the agent reads them fresh in every
session, and follows them by choosing to.

A feature moves through four stages:

1. **Plan** — write down what we are building and why, as a spec file
   checked into the repository.
2. **Dev** — build it, committing as you go, running the tests.
3. **Review** — a human reads the change and gives feedback.
4. **Closure** — merge it, tidy up, record what was learned.

Work can also go backwards. If review finds that the *requirement* was
wrong, you go back to planning; if the requirement was right but the
code is wrong, you go back to dev. The rule is to return to the stage
where the mistake was actually made.

Each stage has its own instruction file, and the feature's current
stage is written down in a `## Handoff` section inside the spec file, so
that a new session can pick up where the last one left off. That
handoff section is the thing this document is about.

## The problem

The agent is told, in prose: *when you finish dev, update the handoff
section to say you have moved to review.*

In a short session, it does. In a long one, it often does not — and
nothing notices. The spec still says `Phase: dev` after dev is over. The
next session reads that, believes it, and redoes work or skips a gate.

This is not a hypothetical. We measured it. A roughly 300-line
instruction file, read fresh and then asked to repeat its own rules
back, was followed correctly:

| Coding agent | Fresh session | After ~4.6KB of unrelated conversation |
|---|---|---|
| Claude Code | ~100% | ~100% |
| Kiro | ~100% | slight dip, inconclusive |
| Codex | ~50%, rising to 80% after rewriting the prose | **~33%** |

The last cell is the finding. We rewrote the instructions to be clearer,
added a checklist, added a self-check at the end — and it worked, in a
fresh session. Under a realistic amount of prior conversation, the
improvement vanished entirely and the same rule was dropped in the same
way as before.

So the conclusion is uncomfortable but clear: **for at least one agent
we support, no amount of better writing makes a prose instruction
survive a long session.** And kdevkit's whole premise is work that spans
sessions.

## What we tried, and what we learned

We spent a long research session on this, partly reading what other
people have built and partly running experiments. Here is what changed
our minds, roughly in the order it changed them.

### "Force the agent to do it" is not available

The obvious fix is to make the transition mandatory rather than
requested. Every agent we support has a mechanism that can intercept an
action and refuse it. So: intercept, check, refuse if wrong.

That works for *stopping* something. It does not work for *starting*
something. None of these mechanisms can force the agent to take an
action it never attempted. If the agent simply never updates the
handoff, there is nothing to intercept.

We then looked at whether anyone else had solved this, and found that
nobody had even tried. Every comparable system we examined **gates**
transitions but does not **force** them:

- **gastown**, a multi-agent workspace manager and the most
  machinery-heavy example we found, still relies on an agent choosing to
  run a "done" command.
- **GitHub's spec-kit** enforces nothing at all — its stages are just
  prompt files, and their ordering is a recommendation.
- **Roo Code**, which has explicit modes for architecture, coding and
  debugging, states plainly that nothing forces you to move from one to
  the next.

So we stopped trying to guarantee the *right* transition happens, and
settled on a weaker but achievable goal: **a wrong record must never
become the accepted truth, and a session that has lost its memory must
be able to recover the truth from the repository.**

### Replacing five rules with one command does not help

Next idea: instead of five rules to remember, give the agent one command
to run — `phase advance --to review` — and put the logic in the command.
One thing to remember is easier than five.

We found a project that had tried exactly this and measured the result.
`spec-workflow-mcp` asks its agent to call a logging tool after
completing each task. Across dozens of features, that tool was called
**zero times**. Not rarely. Never.

The cause was not that the instruction was unclear. It was *where the
instruction sat*: it came after the step that marks the task complete.
The agent marked the task done, considered the job finished, and never
reached the next line.

Our planned command would have sat in exactly the same place — after
commit and push, at the point where the work already feels finished. So
this idea was not an improvement. It was the same bug with a shorter
instruction.

That failure is what produced the actual design. If the agent cannot be
relied on to record something *after* finishing, then the record has to
be a **side effect of the finishing itself**.

### The thing the agent cannot skip is a commit

An agent doing dev work has to make commits. That is not optional and
not forgettable — it is the work.

Git can attach a line to the bottom of every commit message
automatically, using a hook. This is not exotic: the `Signed-off-by:`
line on Linux kernel patches works this way, and Gerrit has used a
hook-inserted `Change-Id:` line as durable identity for about fifteen
years.

So the phase can be stamped into the commit message by git, at the
moment of committing, without the agent doing anything. And the same
hook can refuse the commit if the claimed state and the actual state
disagree.

We tested the details, because the details decide whether this works:

| Hook | Can add the line | Can refuse the commit | Still runs with `--no-verify` |
|---|---|---|---|
| `commit-msg` | yes | yes | **no, skipped** |
| `prepare-commit-msg` | yes | yes | **yes** |

That difference matters. An agent that hits a refusal will sometimes
retry with `--no-verify` to get past it. Using `prepare-commit-msg`
closes that door.

### Storing state in the spec file is the weak point

We had assumed the handoff section was fine as a place to keep the
current phase. Two projects showed us otherwise.

**Backlog.md** stores task state as markdown in git, deliberately, and
is well used. But it had a bug where two processes updating a task at
once would silently lose one of the updates — measured at **seven of
eight concurrent writes lost**, with both writers reporting success. The
fix was file locking.

**taskmaster-ai** keeps its state in a JSON file and accumulated a long
run of problems: race conditions between multiple editor windows,
several kinds of corruption caused by its own commands, and stale reads
after a user hand-edited the file. It is now moving to a database for
the transactions and to a line-per-entry format so git can merge it.

kdevkit's version is the fragile end of this. One machine-readable field
sits among four free-prose fields, inside a document humans are actively
encouraged to edit, and the instruction is to replace the entire block
each time — which is the worst possible shape for git merges. And
kdevkit runs each feature in its own git worktree, so two sessions
touching one spec is a normal occurrence, not an edge case.

Commit messages avoid all of this. Each commit is its own entry, so
there is nothing to overwrite. Nobody hand-edits history casually. And
when the feature branch is squashed into the mainline, the per-commit
messages go away, so none of this bookkeeping pollutes the main history
— which is the behaviour we want anyway.

### Where the checking code can live

We needed a small amount of code to answer factual questions about the
repository. Three options:

**A compiled program installed on the PATH.** Works well on every agent
we support. Rejected because it means a build step and an install step
for something that should travel with the repository.

**A script bundled inside the skill folder.** This works on Claude,
which substitutes the skill's own directory into commands. It does not
work elsewhere: Codex has no documented way for a skill to invoke its
own script, and Kiro's own skills use bare relative paths that only
resolve if the working directory happens to be the skill folder. So on
two of three agents the path would have to be guessed — reintroducing
exactly the unreliability we are trying to remove, with a silent failure
when the guess is wrong.

**A script committed in the repository at a fixed path.** Works
everywhere, because the working directory is the repository root. No
build, no install, no guessing. This is what we chose.

One constraint follows: the script must use only tools that are
definitely present. `jq` and `python3` are not — on this machine they
come from a user-specific package profile, and the project's own
development shell does not include them. So: POSIX shell, `git`, `awk`,
`sed`.

### Asking an AI to do the checking is worse than it sounds

We seriously considered making the checker itself an AI sub-session:
zero installation, works anywhere a skill works.

Every question the checker needs to answer has an exact answer
obtainable with `grep` — are all the checklist items ticked, does a
commit of the right type exist, is there exactly one handoff section. An
AI answering those adds nothing but variance.

Worse, constrained output makes this actively dangerous. If you require
the answer to be one of a fixed set of values, the model will pick one
even when none of them is right — the documented behaviour is that it
"will always try to adhere to the provided schema." A checker that
cannot say "I don't know" is a checker that confidently says the wrong
thing. And Kiro has no way to constrain output at all.

We found no example of anyone using an AI for a mechanical state check.
The nearest relevant project, Probity, deliberately splits the two:
deterministic rules by default, AI only for genuine judgement calls.

### Restricting what a stage can do beats checking what it did

This came from looking at editor-based agents, and it was the most
useful single idea we found. Cline, Roo Code and Kilo Code all have
modes — planning, coding, asking — and **none of them enforces the order
of the modes.** What they enforce is what each mode is *capable* of:

- Cline's planning mode cannot modify files or run commands at all, and
  its documentation says the constraint is deliberate.
- Roo's architect mode can only edit markdown.
- Kilo's planning mode can only write into its own plans directory.

This is stronger than checking afterwards, because there is nothing to
check — the stage physically cannot do the wrong thing. And every agent
we support has some version of it.

### One thing that nearly caught us out

The mechanism we rely on can fail without saying so. There is a known
class of bug where an interception hook stops working mid-session and
the result looks identical to a session where nothing was wrong. As one
bug report put it: a hook that does not fire looks exactly like a hook
with nothing to object to.

We tested the specific case that worried us most — kdevkit runs features
in git worktrees, and there was a report of hooks dying when switching
into one. It does not reproduce when the session *starts* in the
worktree, which is how kdevkit works. Good news.

But we did find a real one: running Claude with the `--bare` flag
disables hooks entirely, exits successfully, and warns about nothing.
That flag must never be used where we expect enforcement.

The general lesson stands, and it is the reason the design puts the
guarantee in git rather than in the agent: **git running a hook is not
subject to the agent's session state at all.**

## The design

Four pieces. Only one of them is new code, and it is small.

### 1. The instruction files — unchanged in kind

The markdown the agent reads, split so that each stage loads only what
applies to it. This keeps everything that requires judgement: what
"finished" means, when to go back a stage, what good work looks like. No
judgement moves into code, ever.

### 2. The checker — a shell script committed to the repository

Lives at a fixed path in the repository. It answers factual questions
and has no opinions:

- Are all the checklist items in the implementation plan ticked?
- Is there a commit on this branch that looks like real work rather than
  planning?
- Does the branch exist on the remote?
- Is there exactly one handoff section in the spec?

It has no configurable thresholds, now or later — if a question needs a
tunable number to answer, it is a judgement call and belongs in the
instructions instead. It always prints the facts it found alongside any
conclusion, so a conclusion can never hide its inputs. And if it cannot
determine something, it says so, and "cannot determine" is treated as
"no".

Its verbs:

```
phase facts                 print what is true about this repo
phase show                   current stage, plus the facts, plus how
                             many times we have gone backwards
phase check --to dev         is that move allowed from here?
phase advance --to dev       record the intent to move
phase return --to plan       record going back, with reasons
```

### 3. Two git hooks — also committed to the repository

Git runs these itself. The agent is not involved and does not need to
know they exist.

**On every commit**, the first hook asks the checker what is true,
writes the current stage into the commit message as a line at the
bottom, and refuses the commit outright if the claim and the facts
disagree. Because it writes the stage *from the facts* rather than from
the agent's claim, the agent cannot get it wrong by forgetting.

**Before every push**, the second hook re-checks and refuses to publish
a branch whose history does not add up. This deliberately gates the
thing the agent wants — publishing its work — rather than trailing
behind it.

### 4. The stage capability list

A small configuration file saying what each stage may touch. Planning
may write specs but not source. Review may read but not write. Where the
agent supports limiting its own tools, this is applied so the stage
cannot overstep in the first place.

### Where the state actually lives

In the commit messages, as a line like `Phase: dev`, plus `Return-To:`
when work goes backwards and `Acked-By:` to record who approved the
move.

The four reasons this beats a file:

1. The agent cannot forget to write it, because git writes it.
2. Two sessions cannot overwrite each other's record, because each
   commit is a separate entry.
3. It travels with the branch automatically.
4. It disappears when the branch is squashed into mainline, so none of
   this bookkeeping ends up in the permanent history.

The handoff section stays in the spec, but only for the parts that are
genuinely prose and genuinely useful to a human: what to carry forward,
what was deliberately left undone, what the next stage should expect. No
machine-readable field remains in it.

One piece of short-lived state — "the next commit should move us to
review" — is written inside the `.git` directory, where it is specific
to one worktree and can never be committed by accident.

### Who decides what

| Decision | Decided by |
|---|---|
| Which instruction file to load | The router, from what the checker reports |
| Whether a move forward is allowed | The checker, from facts |
| Whether to actually make that move | Whoever is supervising the session |
| Which stage a mistake belongs to | The agent proposes; it must give reasons |
| Whether that reasoning is recorded | The checker, which refuses vague answers |
| Whether a stage overstepped | The capability list |

On supervision: this is usually a human, but it need not be. If a
project-level session breaks work into features and runs each one, then
*it* is the supervisor for those sessions. So approval has to be
something a program can express, and we record who gave it. Approval is
an audit record, not a guarantee — one AI approving another's work is
two guesses agreeing.

### Going backwards

Any earlier stage can be returned to. That is not restricted, because
deciding where a mistake belongs is judgement and code cannot do it.

What *is* enforced is that the return is not vague. It must name the
stage at fault, the problem, and what would resolve it. Returns are
counted, so repeated bouncing between stages becomes visible instead of
hidden. And until the recorded problem is discharged, the feature cannot
move forward again — otherwise "go back" becomes a way of escaping a
check that cannot be passed.

## How each agent gets this

The important part: **three of the four pieces need no per-agent work at
all.** The checker and the hooks are files in the repository, and git
runs the hooks. That is why the weakest agent we support still gets the
main guarantee.

**Claude Code** — instruction files are symlinked into its skills
directory, as today. The capability list becomes tool restrictions, so a
stage cannot use a tool outside its remit. Optionally, hooks declared in
the skill's own front matter add a second net that catches file edits
git cannot see; these travel with the skill and need no separate
install. Each stage can run in its own clean context using the agent's
built-in forking, so no terminal multiplexing is required.

**Codex** — instruction files are symlinked per-skill. Checker and git
hooks work identically. Tool restrictions come from its sandbox
settings, which live in the user's own configuration rather than the
repository, so the project cannot ship them and we treat them as
optional. Stage isolation is done by running each stage as a separate
session.

**Kiro** — instruction files are symlinked into its skills location.
Checker and git hooks work identically, and this is the whole argument
for putting the guarantee in git: Kiro's command-line tool has no hook
mechanism at all and cannot constrain its output, yet it still gets
everything except capability restriction. Restricting tools on Kiro is a
launch-time flag rather than configuration, so it only applies when the
session is started through a wrapper. That is a limitation we are
accepting, not solving.

**Any future agent** needs two things: it must read instruction files
from a directory, and it must be able to run a shell command. It then
gets everything git provides for free.

### Which agents we are not supporting, and why

We chose the weakest agent we will support rather than inheriting one.
The bar is three capabilities: a *directory* of instruction files (not a
single flat file, which cannot hold per-stage modules), the ability to
run a shell command unattended, and some way to restrict what tools a
context may use. Kiro is the weakest agent that clears that bar.

Below it, and rejected:

- **Cloud agents** such as Jules and Devin. There is nothing installed
  locally at all — no instruction directory, no hooks, no script.
  Supporting them would mean giving up per-stage instructions and
  capability restriction in order to reach agents that cannot run the
  human review stage anyway.
- **Aider.** Running a shell command is a confirmation-gated
  *suggestion*, and there is no per-tool restriction.

Above Claude Code is **Amp**, which can intercept a tool call and
respond with allow, reject, modify, or substitute a result outright, and
can refuse to let a session end. Its plugins live in the repository, so
a project can ship enforcement — exactly the thing Codex cannot do. It
is not portable and explicitly offers no backward compatibility, so we
treat it as a picture of what good looks like rather than something to
depend on.

## What must be true when we are done

These are the statements the tests are written against. They describe
outcomes a person can check, and deliberately say nothing about how they
are achieved.

1. A session starting with no knowledge of prior work can determine the
   feature's current stage from the repository alone.
2. A commit whose stage claim contradicts the repository does not land.
3. Statement 2 still holds when the commit is made with `--no-verify`.
4. Going back a stage records the stage at fault, the problem, and what
   would resolve it.
5. After going back, the feature cannot move forward again until the
   recorded problem is discharged.
6. Going back is countable, so repeated bouncing is visible.
7. After a feature is merged, no stage bookkeeping remains on the
   mainline branch.
8. With none of this installed, kdevkit behaves exactly as it does
   today.
9. When the checker cannot determine an answer, no transition happens.
10. Where the agent can restrict tools, a stage cannot act outside its
    remit.
11. Statements 1 to 9 hold on Claude, Codex and Kiro.

## How we will test it

The tests are written from the statements above, not from the design. An
assertion may describe what someone would find in the repository. It may
not describe which script ran, or in what order. If we rebuilt this a
completely different way, these tests should still pass unchanged.

### Two kinds of test

**Tests with no AI involved.** Set up a throwaway repository, run git
commands the way a person would, check the result. These cover
statements 2, 3, 5, 6, 7 and 9 — the guarantees that must hold no matter
how any model behaves. They are fast, free, repeatable, and run on every
build.

**Tests that drive real agents.** These extend the project's existing
fixture format, which already works the right way: each fixture poses a
task the way a person would phrase it, lets the agent work, and then
checks the repository afterwards. It never inspects the agent's
reasoning. These cover statements 1, 4, 8, 10 and 11, because those are
the ones that depend on an agent actually behaving.

### Rules for writing the assertions

Every one of these exists because it caught a real defect in earlier
work on this project.

- **An assertion must fail if the work was not done.** For each new
  assertion, write down what a lazy agent that does nothing would leave
  behind, and what a careless agent that does the wrong thing would
  leave behind, and confirm both fail. An assertion that passes without
  the work being done is worse than none.
- **Never check only that something is absent.** "No stale stage" is
  satisfied by deleting the field entirely. Check that the field exists,
  *and* holds a legal value, *and* is not the value it should have moved
  on from.
- **Watch out for the template.** The spec template lists every legal
  stage on one line, so a naive match succeeds against untouched
  boilerplate. Exclude it explicitly.
- **Check exit codes and error output, not just success output.** A
  silent failure that prints nothing otherwise reads as a pass.
- **One run proves nothing.** Sample at least three runs per fixture per
  agent and record the ratio, not a verdict.
- **Test under load.** Every agent-driven fixture must also run with a
  large block of unrelated prior conversation prepended, because the
  entire problem being solved only appears under load. A fixture that
  passes fresh and fails loaded has found the bug, not a flake.

### Coverage

| Statement | Test |
|---|---|
| 1 | Set up a half-finished feature branch, start a session knowing nothing, ask what state the feature is in. It should name the right stage without asking. |
| 2 | Hand-write a commit claiming a stage the repository contradicts. The commit should not exist afterwards, and the error should say why. |
| 3 | Repeat with `--no-verify`. Same outcome. |
| 4 | Set up a review-stage branch where the original requirement was wrong. Feed in that review outcome. The recorded reason should name the stage, the problem and the resolution. |
| 5 | With an undischarged problem recorded, try to move forward. Refused. Discharge it. Now allowed. |
| 6 | Record two returns. The count reads as two, without parsing prose. |
| 7 | Merge a feature branch. No stage bookkeeping appears in mainline history or files. |
| 8 | Remove the checker, unset the hook path, run the existing dev-loop fixtures. Results should match today's recorded baseline. |
| 9 | Put the repository into a state the checker cannot classify. No transition happens, and it says why. |
| 10 | In the planning stage, give the agent a task that would require editing source. No source file changes. |
| 11 | Every agent-driven fixture runs on all three agents, fresh and under load. |

### Ways this could be cheated, each needing its own test

- Amending a commit to replace its message, erasing the stamped stage.
- Hand-editing the handoff prose so it disagrees with the commit
  history.
- Making the change through a shell here-document instead of the editing
  tool, so an agent-level hook never fires.
- Going backwards to escape a forward check that cannot be passed.
- Running with the flag that disables hooks, and checking the failure is
  loud rather than silent.

## How to build it

Ordered so that each step is useful on its own and testable when it
lands. No step depends on an unresolved question.

1. Change the repository's squash-merge setting so per-commit messages
   are discarded on merge. This is configuration, not code, and
   statement 7 depends on it.
2. Write only the `facts` verb of the checker — read the repository,
   print plain `key=value` lines, no conclusions. Add a test per fact
   against a seeded repository.
3. Add the list of allowed moves and the `check` verb, including the
   "cannot determine" answer. Test statement 9.
4. Add the commit-time hook: stamp the stage from the facts, refuse
   contradictions. Test statements 2 and 3, including the amend case.
5. Add the going-back verbs with their required fields, the count, and
   the block on moving forward. Test statements 4, 5 and 6.
6. Add the pre-push hook. Test that an inconsistent branch cannot be
   published.
7. Teach the install tool to point git at the hooks and mark the scripts
   executable. Test that a fresh clone plus install gives a working
   guarantee.
8. Update the instruction files to mention the checker, and remove the
   machine-readable field from the handoff section, leaving the prose.
9. Add the capability list and translate it for Claude. Document the
   Codex and Kiro limitations rather than working around them.
10. Extend the agent-driven fixtures for statements 1, 4, 6, 8 and 10.
    Three samples per agent, fresh and under load, ratios recorded.
11. Correct the five inaccurate claims in the existing specs, listed
    below.

### Notes for whoever builds it

Write the checker in POSIX shell using only `git`, `awk` and `sed`.
Make each fact a separate one-line function so it can be tested alone,
and have `facts` print all of them every time, so a conclusion can never
hide the inputs that produced it.

Read the stage with
`git log --format='%(trailers:key=Phase,valueonly=true,unfold=true)'`.
Note that this emits a trailing newline, which will silently break a
naive string comparison. Write with `git interpret-trailers --in-place`
rather than editing the message text yourself.

Put the refusal logic in `prepare-commit-msg`, not `commit-msg`. Only
the former still runs under `--no-verify`, and that difference is the
whole of statement 3. Re-stamp on every commit including amends, because
an amend replaces the entire message.

Make every refusal say what was wrong *and* what would fix it. A
refusal the agent cannot act on turns into a retry loop; a refusal that
names the problem turns into a correction.

Keep the short-lived intent file inside the git directory, at the path
`git rev-parse --git-path kdevkit-intent` returns, so it is
per-worktree and cannot be committed.

## Corrections to existing specs

Five claims in the checked-in specs turned out to be wrong. Two of them
were introduced during this research session and then corrected, which
is worth saying plainly.

1. **"No hooks are shipped for any agent today"** — wrong. The
   project's own terminal session tracker already installs hooks into
   the user's Claude settings and uses them to track session state.
2. **The session tracker's "no extra parent process"** is a note about
   how one wrapped process replaces itself, not a position on
   orchestration. It has been cited as the latter.
3. **The OpenAI citation about re-stating instructions** is narrower
   than claimed: it is about markdown *formatting* drifting over a long
   conversation, not instruction-following in general. The measured
   Codex result stands on its own; this citation should not be leaned
   on.
4. **gastown's "never reuse a branch" rule** could not be found and is
   contradicted by code that explicitly reuses branches. Remove the
   citation.
5. **gastown's argument against a remembering coordinator is real** — I
   wrongly reported it as a misquote mid-session. The distinction is
   worth keeping: routing work through a long-lived agent is fine, but
   having that agent *remember* the progress of the work is the failure
   mode, because it loses the thread when its context is compacted.

## What we decided, and what we turned down

- **State goes in commit messages, not a file.** Each commit is its own
  entry, so concurrent writes cannot clobber each other; git writes it,
  so the agent cannot forget; it vanishes on squash, so mainline stays
  clean. Turned down: git notes (not pushed by default, orphaned by
  amends, and merges need hand-resolution — all three verified); a
  locked side file (solves the corruption but adds an artifact and still
  needs the agent to call something); the current markdown field (whose
  failure mode is measured elsewhere at seven of eight writes lost).
- **The checker is a committed shell script.** Turned down: a compiled
  program on the PATH (needs building and installing, and reaches no
  agent without a local install); a script inside the skill folder
  (resolves reliably on Claude only).
- **An AI does not do the checking.** Every question has an exact
  answer, constrained output forces a guess where none is right, and
  Kiro cannot constrain output at all. Revisit only if the facts turn
  out to leave a real judgement call behind.
- **Forcing the right transition is out of scope.** Nothing achieves it.
  We guarantee that a wrong record cannot become the truth.
- **Approval is whoever supervises**, human or a parent session, and is
  recorded with who gave it.
- **The research stage is recordable but never gated.** There is no
  factual test for "research is finished", so its exit is an approval,
  not a check.
- **Kiro is the floor; cloud agents and Aider are out.**

## Still open

- **The squash-merge setting has to change before statement 7 can pass.**
  Under the current default, per-commit messages are kept, so the
  bookkeeping would leak into mainline.
- **`git commit --amend -m` erases the stamped stage**, and agents amend
  often. Re-stamping should handle it, but it needs a test proving an
  amended commit cannot launder a false claim.
- **Can tool restrictions be applied per stage on Codex** without
  touching the user's own configuration, or is it launch-time only as on
  Kiro?
- **Is kdevkit being loaded on Kiro in the wrong way?** Kiro now has a
  native skills directory, and we deploy into its steering directory
  instead. If so, our instructions may be loaded as always-on context
  rather than on demand. This affects cost, not correctness, and is
  unverified.
- **How is a silently dead agent-level hook detected?** The idea is for
  the hook to leave a mark that a later check requires, so absence is
  detectable — but that must not reintroduce a state file.

## Handoff

- **Ready for:** dev, once this design is agreed.
- **Carry forward:** the full research record, with sources, is in
  `specs/backlog/kdevkit-durable-cross-runtime-adherence.md`. The five
  spec corrections above are part of this work, not a separate task.
- **Deliberately left:** using an AI for whatever genuine judgement
  remains after the facts are computed — build the facts first and see
  how small the remainder is. Also left: running stages as separate
  terminal sessions, which is its own piece of work.

## Session log

- **2026-08-29 · Research done, design drafted, nothing built.** Eleven
  parallel investigations and three local experiments: whether hooks
  survive a git worktree, how commit-message trailers behave, and
  whether a skill can invoke its own bundled script on each agent. Also
  probed the three agents' command-line capabilities directly. Five
  existing claims corrected. Around $1.75 of paid agent runs.

## Decision log

- **2026-08-29 · Stop trying to force transitions; guarantee instead
  that a wrong record cannot be accepted.** Nothing we surveyed forces
  one, and no agent can force an action it never attempted. Rejected:
  continuing to chase forced invocation, which the
  `spec-workflow-mcp` result shows fails even for a more prominent
  mechanism than a shell command.
- **2026-08-29 · Keep stage state in commit messages.** Written by git
  so it cannot be forgotten, append-only so it cannot be clobbered, and
  discarded on squash so mainline stays clean. Rejected: git notes, a
  locked side file, and the current markdown field, each for reasons
  recorded above.
- **2026-08-29 · The checker is a committed POSIX shell script.** A
  repository-relative path is the only one that resolves on every agent.
  Rejected: an installed binary, a skill-bundled script, and an AI
  sub-session.
- **2026-08-29 · Support down to Kiro and no further.** A supported
  agent must offer an instruction *directory*, an unattended shell, and
  tool restriction. Rejected: cloud agents and Aider, which fail that
  bar and cannot run the human review stage anyway.
- **2026-08-29 · Approval is the supervising context, not necessarily a
  human.** A project-level session may supervise a feature-level one, so
  approval must be expressible by a program and recorded with its
  author. It is an audit record, not a guarantee.
