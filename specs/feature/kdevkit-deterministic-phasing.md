# Making phase transitions reliable in kdevkit

Branch: `feat/kdevkit-deterministic-phasing`

## What this document is

A proposal to fix one specific bug in how kdevkit tracks its own
progress. It is written to be read start to finish. It covers what the
problem is, what we measured, what we tried and rejected, the design we
arrived at, how we will know it works, and how to build it.

The mechanism described here is now built and tested; the checklist near
the end marks what is done, partly done, and not started.

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
repository. The question is where it lives, and the answer has to satisfy
two things that initially looked opposed: the agent must be able to find
it without guessing, and **the project being worked on should not have to
carry kdevkit's machinery.** A project should be developable by someone
who has never heard of kdevkit, and two features in the same repository
should be able to use different tooling.

**A compiled program installed on the PATH.** Works on every agent.
Rejected: it needs a build step, and a binary is platform-specific.

**A script inside the skill folder, found at run time.** Claude can do
this — it substitutes the skill's own directory into commands. Nothing
else can: Codex has no documented way for a skill to invoke its own
script, and Kiro's own skills use bare relative paths that only resolve
if the working directory happens to be the skill folder. So on two of
three agents the path would have to be guessed, with a silent failure
when the guess is wrong.

**A script inside the skill folder, with its path fixed at install
time.** This is what we chose, and it dissolves the apparent conflict.
The install step already knows exactly where it is putting the skill, so
it writes that absolute path into the instruction text as it deploys.
The agent never resolves anything at run time and never guesses — it
reads a literal path. Nothing lands in the project repository at all.

The earlier draft of this document proposed committing the script into
each project. That was wrong, and the reason is worth recording: we had
assumed the only way to get a reliable path was a repository-relative
one. Substituting at install time gets the same reliability with none of
the project pollution.

On what the script may depend on: since the state lives in commit
messages rather than a JSON document, the checker's output is
`key=value` lines that `awk` handles, so `jq` is not needed. If a later
need for it appears, the install step could carry helpers — but adding a
runtime dependency to reach every supported platform is a real cost, so
the working assumption is POSIX shell, `git`, `awk` and `sed`, and
anything more has to justify itself.

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

## Where this sits among kdevkit's three loops

kdevkit is not one loop, it is three nested ones, and this work
deliberately addresses only the middle one. Being explicit about that is
what keeps the design from quietly breaking the other two.

```
project  →  [optional initiative]  →  feature            outer loop
                                        │
        [optional research] → plan → dev → review → closure   feature loop
                                     │
                    quality → test → code review → fix → …    dev loop
```

**This work operates on the feature loop only.** The stamp records which
feature stage we are in. Nothing here stamps or gates the outer loop or
the dev loop.

Three consequences follow, and each is a robustness requirement rather
than a note.

**The dev loop must not look like feature movement.** The dev loop
iterates: run the quality checks, run the tests, get the change reviewed,
fix what came back, go round again. That produces many commits, and it
produces genuine backwards movement *within* dev. None of that is a
feature-stage change. So every commit made during dev is stamped `dev`,
however many times the inner loop turns, and **inner-loop iterations are
never recorded as going back a stage.** Going back is reserved for
crossing a feature boundary — review sending work to plan, for instance.
Otherwise the return count, which exists to make thrash visible, would be
swamped by normal dev activity and tell us nothing.

**Leaving dev requires the dev loop to have converged, and that is
verified rather than believed.** This is the one place the two loops
genuinely meet. Dev is not finished because the agent says so; it is
finished when the quality checks are clean, the tests pass, and review
findings are resolved.

The obvious implementation is to have the agent record that it ran them,
and check the record. That is worth nothing — it is the same trust we are
trying to remove. So instead **the hook runs the checks itself.** Verified:
pushing with tests passing succeeds; pushing with tests failing is
refused and the failing commit never reaches the remote.

For anything that genuinely must be recorded rather than re-run, the
record is keyed to the **tree hash** it was verified against, so editing
any file invalidates it. Verified: changing one file changes the tree
hash, so a claim carried over from before the edit is detectably stale.
That closes the obvious laundering route of running the tests, then
editing, then claiming green.

**What we are deliberately not doing: enforcing the dev loop's internal
order.** "Quality checks before tests before review" is a sequence of
shell invocations and agent dispatches. Git cannot see any of them —
verified that running the checks, the tests, or a review produces zero
commits, so there is nothing for a stamp to ride on and nothing for the
commit hook to observe. Gating that order would require intercepting tool
calls, which means agent-level hooks, which **Kiro's CLI does not have.**
It would therefore work on two agents and silently do nothing on the
floor — precisely the asymmetry this design exists to avoid.

So the dev loop is enforced on its **outcome, not its route**. A dev loop
that reaches review with failing tests is refused. A dev loop that ran the
linter after the tests, with both clean, is not something worth spending
the floor's portability on.

**The outer loop is out of scope, and inert rather than broken.** A
project- or initiative-level session also commits — writing the project
document, or an initiative plan. Those commits must not be stamped with a
feature stage, because they have no feature. The hook's self-scoping check
covers this: it looks for a work-in-progress *feature* spec naming the
branch, so an initiative branch finds none and the hook does nothing.
That is a deliberate boundary, not an accident, and it needs a test —
otherwise a later change to the detection rule could start stamping
initiative work with a feature stage and nobody would notice.

One naming decision follows from all this. The stamp is scoped to the
loop it describes, so the outer loop can add its own later without a
migration and without ambiguity about which loop a line refers to. A bare
`Phase:` would have to be reinterpreted the moment project-level tracking
arrives; a name that says which loop it belongs to will not.

## The design

Four pieces. Only one of them is new code, and it is small.

### 1. The instruction files — unchanged in kind

The markdown the agent reads, split so that each stage loads only what
applies to it. This keeps everything that requires judgement: what
"finished" means, when to go back a stage, what good work looks like. No
judgement moves into code, ever.

### 2. The checker — a shell script shipped with kdevkit

Lives next to the instruction files, wherever kdevkit is installed. The
install step writes its absolute path into the instructions, so the agent
reads a literal path and never has to work one out. **Nothing is added to
the project being worked on.** It answers factual questions and has no
opinions:

- Are all the checklist items in the implementation plan ticked?
- Is there a commit on this branch that looks like real work rather than
  planning?
- Does the branch exist on the remote?
- Is there exactly one handoff section in the spec?
- Has the dev loop converged on the current commit — quality checks
  clean, tests run and passed, review findings resolved? These are the
  facts that let the feature leave dev, and they are the only place the
  inner loop touches this design.

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

### 3. Two git hooks — shipped with kdevkit, active only in the feature's worktree

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

**Neither hook is committed, and neither affects the rest of the
repository.** Git hooks are not versioned content — they are a local
setting pointing at a directory — so there is no mechanism by which they
could reach a branch at all.

**The hook decides for itself whether it applies.** This matters because
kdevkit must work whether or not the feature has its own worktree, and
the scoping git offers is per-worktree, not per-branch. In a single
checkout where you simply switch branches, a hooks path applies to
*every* branch including the main one. Verified — an earlier draft of
this document claimed otherwise and was wrong.

So the scoping is done by the hook, not by where it is installed. Its
first two lines ask: is this the project's default branch, and does a
work-in-progress spec name this branch? If either answer says no, the
hook exits immediately having done nothing. Verified across three
branches in a single checkout with no worktrees:

| Commit made on | Hook acts |
|---|---|
| the default branch | no |
| an unrelated branch | no |
| a feature branch with a spec naming it | **yes** |

This works in both modes. If the feature does have its own worktree, the
hooks path can additionally be set for that worktree only, which is a
second layer rather than the mechanism. Either way the default branch is
untouched, another branch is unaffected, another feature may use
completely different tooling, and a project cloned without kdevkit
contains nothing belonging to it.

Two rough edges are real and not yet designed away. Matching a spec
against the branch name breaks if the branch is renamed mid-feature; the
more robust signal is that the branch's history already contains a stage
stamp, but that cannot work for the very first commit, so probably either
signal should activate it. And pointing git at a hooks directory
displaces any hooks the user already has — someone using a hook manager
would lose it — so kdevkit's hook needs to hand off to a pre-existing
hook rather than assume it owns the slot.

This also answers whether hooks can be avoided altogether. They cannot,
if the guarantee is to reach every supported agent: the agent's own
interception mechanisms do not exist on Kiro and cannot be shipped by a
project on Codex. But a hook that does nothing outside a kdevkit feature
removes the reason to want to avoid them.

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

### What reaches the main branch

The intent is that someone who clones the project and reads the main
branch sees the feature, and no sign of how it was built. Verified with a
real remote, a feature branch carrying stage stamps, a squash merge, and
a fresh clone: the clone has one clean commit for the feature, no hooks
configured, no hook files, no kdevkit files, and no stage bookkeeping
anywhere in its history.

That result depends on one thing being set up, and is otherwise false.
**With git's default squash message, every branch commit message is
copied into the squash commit body** — so the main branch ends up
containing a transcript of the feature's internal commits, stage lines
and all. Tested; it reads exactly as badly as it sounds. The lines no
longer parse as machine-readable footers, being indented, but anyone
running `git log` reads them.

So the squash commit message must be **authored**, not accumulated. It
should be a summary of the feature — what was built, why, and how it was
approached — drawn from the spec's own requirements, design and
implementation sections. That is the record worth keeping in permanent
history. A list of the branch's intermediate commit subjects is not.

Two things deliberately do reach the main branch, and both are fine:

- **The feature's spec file.** It is checked-in documentation, which is
  the point of kdevkit. Its handoff section is cleared at closure, so a
  newcomer reads the spec's content and finds no stage state in it.
- **Nothing else.**

The feature branch itself remains visible on the remote while its pull
request is open, and after merge if it is not deleted. Its stage stamps
are readable there by anyone with access. That is acceptable: the branch
is the working record, and the guarantee being made is about the main
branch.

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
all.** The checker is a shell script the agent runs by a literal path,
and the hooks are run by git rather than by the agent. That is why the
weakest agent we support still gets the main guarantee — and why none of
it lands in the project being worked on.

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

## What this looks like in use

Two walkthroughs of the same feature, to show what a person actually
experiences. The work is trivial on purpose; the point is the machinery
around it.

### With Claude Code

**Starting.** You are in the project. You say: *"plan a feature to add a
`--quiet` flag to the CLI."* The instructions are already installed, so
the agent recognises this as planning work, creates a branch and a
worktree, and writes a spec — what the flag does, the requirements, a
checklist of steps. You read it and push back on a requirement; it
revises. Nothing unusual so far; this is kdevkit today.

Behind the scenes, creating the worktree also pointed git at kdevkit's
hooks for that worktree only. You did not do anything, and your main
checkout is untouched.

**Committing the plan.** The agent commits the spec. Git adds a line to
the bottom of the commit message: `Phase: planning`. You did not ask for
it and the agent did not remember to do it.

**Moving to dev.** You say *"looks good, build it."* The agent runs the
checker, which reports the facts: the spec has requirements, a design,
and a checklist with nothing ticked; there are no work commits. Moving to
dev is allowed. It records the move, and the next commit carries
`Phase: dev`.

**Building.** It edits the source, commits, ticks checklist items as it
completes them, runs the tests. Every commit gets stamped. If it tried to
commit with the checklist half-ticked while claiming the stage was
review, the commit would be refused with a message saying exactly which
items were outstanding.

**A refusal you might actually see.** It finishes, and tries to push
before the tests pass. The pre-push hook refuses: *"cannot publish — the
test command has not been run on the current commit."* It runs the tests,
then pushes. You never had to notice.

**Review.** You say *"get it ready for review."* It pushes, opens the
pull request, and the stage becomes review. You read the change and find
that the flag should suppress warnings too — something the requirement
never said. That is a planning-stage mistake, not a coding one.

You say so. The agent records going back: the stage at fault is
requirements, the problem is that warning suppression was never
specified, and the resolution is to amend the requirement and extend the
tests. That is now on the record, it counts as one return, and **the
feature cannot move forward to closure until it is discharged.**

**Closure.** Requirement amended, code updated, tests extended, reviewed
again, and you say *"ship it."* It squash-merges, and writes the merge
message itself as a summary of the feature — what it does, why, how it
was built — taken from the spec. None of the intermediate commit
messages, and none of the `Phase:` lines, appear on the main branch. The
worktree goes away and with it the hooks setting.

Someone who clones the project tomorrow sees one commit adding a
`--quiet` flag, plus the spec file as documentation. No hooks, no stage
lines, nothing to tell them kdevkit was involved.

**If your session had died** at any point — crashed terminal, closed
laptop, context exhausted — a new session reads the branch, finds the
last stamped stage, and continues. It does not ask you where things
stood, and it does not trust a stale line in a document.

### With Codex

Same feature, same four stages, same spec, same commit stamps, same
refusals, same clean merge. **Everything in the walkthrough above happens
identically**, because all of it is done by git and a shell script.

Three differences, all of which you would notice only if you looked:

**Stage isolation is done with separate sessions.** Claude can give each
stage its own clean context inside one session. Codex cannot, so moving
from planning to dev means starting a fresh Codex session in the same
worktree. It reads the branch, learns the stage from the commit history,
loads the dev instructions, and carries on. This is the case the whole
design exists to make safe — a new session with no memory, recovering
state from the repository.

**The stage cannot be prevented from overstepping.** On Claude, the
planning stage is denied the editing tools outright, so it *cannot* touch
source code. On Codex the equivalent setting lives in your personal
configuration rather than the project's, so kdevkit cannot ship it. The
planning stage is therefore asked not to edit source, and if it does
anyway, the commit hook catches it after the fact rather than the tool
restriction preventing it up front. Detection instead of prevention.

**The second safety net is missing unless you opt in.** Claude can run
extra checks on file edits as well as commits. Codex can too, but only
from your own configuration, so by default the guarantee rests on the git
hooks alone. That is the floor this design was built to, and it is why
the guarantee lives in git.

**What is the same, and this is the point:** a Codex session that has
lost the thread cannot record a stage it has not reached, cannot push an
inconsistent branch, and cannot silently leave a stale stage behind for
the next session to believe. The agent measured at roughly a third
adherence on prose instructions gets the same guarantee as the one at
nearly full adherence, because the guarantee stopped depending on the
agent.

### What a person has to do differently

Close to nothing, which is the intent. You install kdevkit as you do
today. You are occasionally told *no* with a reason — the refusals above.
You provide the same judgement you provide now: is this plan right, is
this change good, which stage was at fault. The bookkeeping you currently
have to notice going wrong stops needing to be noticed.

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
7. After a feature is merged, no stage bookkeeping remains on the main
   branch, and the main branch has no hooks configured or installed.
8. The merge commit on the main branch carries an authored summary of the
   feature — what was built, why, and how — and does not contain a copy
   of the branch's intermediate commit messages.
9. With none of this installed, kdevkit behaves exactly as it does
   today.
10. When the checker cannot determine an answer, no transition happens.
11. Where the agent can restrict tools, a stage cannot act outside its
    remit.
12. A feature works the same way whether or not it has its own worktree,
    and in a single checkout the machinery stays inert on every branch
    that is not a kdevkit feature.
13. Iterating the dev loop — quality, tests, review, fix, repeat — does
    not change the recorded feature stage and does not count as going
    back a stage, however many times it turns.
14. A feature cannot leave dev until the dev loop has converged: quality
    clean, tests passed, review findings resolved — with the checks
    observed running, not merely reported as having run.
15. Evidence that is recorded rather than re-run is invalidated by any
    change to the files it was verified against.
16. Commits made by project- or initiative-level work carry no feature
    stage at all.
17. Statements 1 to 10 and 12 to 16 hold on Claude, Codex and Kiro.

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
| 7 | Merge a feature branch into a real remote, then clone it fresh. The clone must show no stage bookkeeping anywhere in its history, no hooks path configured, and no hook files. Assert against the clone, not the working copy — the working copy still has the feature branch and will pass by accident. |
| 8 | Merge a feature and read the resulting commit message. It must contain the feature's summary and must *not* contain the subjects of the branch's intermediate commits. Check both directions: a summary that is merely the branch title is as wrong as a transcript. |
| 9 | Remove the checker, unset the hook path, run the existing dev-loop fixtures. Results should match today's recorded baseline. |
| 10 | Put the repository into a state the checker cannot classify. No transition happens, and it says why. |
| 11 | In the planning stage, give the agent a task that would require editing source. No source file changes. |
| 12 | In one checkout with no worktrees, run the same feature through and assert it behaves identically; then commit on the default branch and on an unrelated branch and assert nothing was stamped and nothing was refused. |
| 13 | Drive several dev-loop iterations — failing tests, then a review finding, then a fix. Assert the stage stayed `dev` throughout and the return count is still zero. This is the test that stops normal dev churn from being mistaken for thrash. |
| 14 | With tests failing, attempt to push. Refused, naming the condition, and the failing commit must not reach the remote — assert against the remote, not the local branch. Fix and assert the push now succeeds. |
| 15 | Record evidence against one set of files, change a file, then attempt to advance. Refused as stale. |
| 16 | On a branch holding initiative-level work and no feature spec, commit. Assert no stage was stamped and the commit was not refused. Guards the boundary against a future change to the detection rule. |
| 17 | Every agent-driven fixture runs on all three agents, fresh and under load. |

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

1. [ ] Make the merge message an authored summary. Two parts: set the
   repository's squash-merge option so per-commit messages are discarded
   rather than accumulated, and have closure compose the message from the
   spec's requirements, design and implementation sections. Statements 7
   and 8 both depend on this, and without it the main branch collects a
   transcript of the feature's internal commits.
2. [x] Write only the `facts` verb of the checker — read the repository,
   print plain `key=value` lines, no conclusions. Add a test per fact
   against a seeded repository.
3. [x] Add the list of allowed moves and the `check` verb, including the
   "cannot determine" answer. Test statement 10.
4. [x] Add the commit-time hook: stamp the stage from the facts, refuse
   contradictions. Test statements 2 and 3, including the amend case.
5. [x] Add the going-back verbs with their required fields, the count, and
   the block on moving forward. Test statements 4, 5 and 6, and test
   statement 13 — that dev-loop iterations do not touch the count.
6. [x] Add the dev-loop convergence facts and make them preconditions for
   leaving dev, with the hook running the checks rather than trusting a
   report, and recorded evidence keyed to the tree hash. Test statements
   14 and 15.
7. [x] Add the pre-push hook. Test that an inconsistent branch cannot be
   published.
8. [x] Add the hook's self-scoping check as the first thing it does, and its
   hand-off to any pre-existing hook. Test statement 12 in a single
   checkout with no worktrees, and statement 15 on an initiative branch:
   the default branch, an unrelated branch and initiative work must all
   be untouched.
9. [~] Teach the install tool to write the checker's absolute path into the
   instruction files as it deploys them, and to set the hooks path —
   scoped to the feature's worktree when there is one, locally when there
   is not. Test statement 7 against a fresh clone of a real remote, and
   test that a project cloned without kdevkit contains nothing belonging
   to it.
10. [x] Update the instruction files to mention the checker, and remove the
    machine-readable field from the handoff section, leaving the prose.
11. [ ] Add the capability list and translate it for Claude. Document the
    Codex and Kiro limitations rather than working around them.
12. [ ] Extend the agent-driven fixtures for statements 1, 4, 6, 9, 11, 12
    and 13. Three samples per agent, fresh and under load, ratios
    recorded.
13. [ ] Correct the five inaccurate claims in the existing specs, listed
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

- **Should `pre-push` test the working tree or the pushed commit?**
  Running the checks in place tests whatever is currently on disk, which
  is not necessarily what is being pushed. Either require a clean tree
  before pushing, or check the pushed commit out somewhere temporary and
  test that — correct but slower. Not decided.
- **The dev loop's internal order is out of scope on purpose**, because
  git cannot observe non-committing steps and the alternative does not
  reach Kiro. Revisit only if the floor changes.
- **The outer loop is noted but not addressed.** This work
  stamps and gates the feature loop only. Whether the project and
  initiative levels eventually want the same treatment — and whether the
  dev loop wants its own record of how many times it turned — is left
  open deliberately. The scoped stamp name and the inertness tests exist
  so that answering those later does not require unpicking this.
- **Where does the merge summary come from?** Closure has to compose it
  from the spec. kdevkit already generates a review briefing that becomes
  the pull request body, so the raw material may already exist — but a
  briefing is written to help a reviewer decide, and a merge message is
  written to be read years later by someone with no context. They may not
  be the same document. Settle this when building step 1.
- **Branch rename breaks the hook's self-scoping**, because it matches a
  spec against the branch name. The sturdier signal is that the branch's
  history already carries a stage stamp, which cannot cover the first
  commit — so probably either signal should activate it.
- **Setting a hooks path displaces any hooks the user already has.**
  Someone using a hook manager would silently lose it. kdevkit's hook
  must hand off to whatever was there before; the mechanism for finding
  it is not designed yet.
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
- **2026-08-31 · The checker and hooks ship with kdevkit, not with the
  project; the hooks are scoped to the feature's worktree.** Revises an
  earlier decision to commit the script into each project. The path
  problem that drove that decision is solved by substituting the
  absolute install path into the instructions at install time, which is
  reliable on every agent without a run-time lookup. Worktree-scoped
  hook configuration — verified to fire in the feature worktree and not
  in the main checkout — keeps the main branch and every other branch
  free of kdevkit, so a project stays independent of it and two features
  may use different tooling. Rejected: an installed binary (build step,
  platform-specific); a run-time skill-directory lookup (Claude only);
  an AI sub-session as the checker; and doing without hooks, which is
  not possible while Kiro has no interception mechanism and Codex cannot
  let a project ship one.
- **2026-08-31 · The hook scopes itself; worktrees are optional.**
  Revises an earlier claim that worktree-scoped configuration keeps the
  default branch clean. It does not, in a single checkout where branches
  are merely switched — verified. So the hook's own first check decides
  whether it applies, and does nothing on the default branch or on any
  branch without a work-in-progress spec naming it. This works with or
  without worktrees; worktree scoping remains available as a second
  layer. Rejected: requiring worktrees, which would force a workflow on
  every project using kdevkit.

- **2026-08-31 · The merge commit carries an authored summary, not an
  accumulated one.** Verified that git's default squash message copies
  every branch commit message into the main branch, stage lines included.
  The permanent record should be the feature's requirements, design and
  approach, drawn from the spec. Rejected: relying only on the
  repository's squash setting, which prevents the transcript but leaves
  the message as a bare title. Accepted as fine, per review: the feature
  branch stays visible on the remote with its stamps, and the spec file
  itself lands on the main branch as intended documentation.

- **2026-08-29 · Support down to Kiro and no further.** A supported
  agent must offer an instruction *directory*, an unattended shell, and
  tool restriction. Rejected: cloud agents and Aider, which fail that
  bar and cannot run the human review stage anyway.
- **2026-08-29 · Approval is the supervising context, not necessarily a
  human.** A project-level session may supervise a feature-level one, so
  approval must be expressible by a program and recorded with its
  author. It is an audit record, not a guarantee.
