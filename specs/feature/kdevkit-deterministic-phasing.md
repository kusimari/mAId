# Making phase transitions reliable in kdevkit

Branch: `feat/kdevkit-deterministic-phasing`

## What this document is

A proposal for one part of kdevkit: making the record of where work stands
live somewhere it cannot be forgotten. It is written to be read start to
finish — what the problem is, what we measured, the design, how we will
know it works, and how to build it.

Some of the mechanism is built. The checklist near the end marks what is
done, partly done, and not started.

## What kdevkit is for

kdevkit is a **pragmatic framework for building software with coding
agents**. Someone picking up a piece of work — at project, initiative or
feature altitude — is guided through it: what stage the work is in, what
finishing that stage means, what to do when something turns out to be
wrong. The guidance uses judgement, because building software does. It is
not a pipeline.

Two things carry the record, and both are ordinary artefacts rather than
new machinery:

- **Branches** hold what happened — the commits, in order, on the working
  branch for that piece of work.
- **Specs** hold what is intended and how this project works: the project
  itself, each feature, how contributions are configured, what reviews
  should focus on. The spec tree is the project's own documentation, not a
  side-file for the framework.

**The person doing the building is called the builder, and a coding agent
can be one.** That is the point of the framework being written down
carefully: an agent should be able to pick up a feature or an initiative
and run the whole flow, in the role a human would otherwise play. What
makes that possible is that the builder can **reconstruct where things
stand, what has been tried, and what went wrong where — without the
conversation that produced it.**

So this feature has two jobs, and they pull in slightly different
directions:

1. **Make the flow hold across coding agents without the human
   constantly correcting it.** Today a human notices when an agent drifts
   and steers it back. That correction is the cost being removed.
2. **Make kdevkit itself drivable by a higher-order builder** — an agent
   working at initiative altitude, defining the macro plan and running the
   streams inside it, treating each feature the way a human treats a
   ticket.

The tension between them is real and is resolved in the design: **enforce
that the record is consistent and articulate; never enforce permission.**
A gate exists to make a builder *say* what it is doing, not to stop it
doing it. A framework that blocks a legitimate judgement cannot be driven
by anyone, human or agent.

## The problem

The record of where work stands lives in prose the builder is asked to
maintain. kdevkit tells the agent: *when you finish dev, update the
handoff section to say you have moved to review.*

In a short session it does. In a long one it often does not — and nothing
notices. The spec still says `Phase: dev` after dev is over. The next
session reads it, believes it, and redoes work or skips a gate. A human
watching catches this and corrects it, which is exactly the cost we are
trying to remove.

This is not hypothetical. A roughly 300-line instruction file, read fresh
and then asked to repeat its own rules back, was followed correctly:

| Coding agent | Fresh session | After ~4.6KB of unrelated conversation |
|---|---|---|
| Claude Code | ~100% | ~100% |
| Kiro | ~100% | slight dip, inconclusive |
| Codex | ~50%, rising to 80% after rewriting the prose | **~33%** |

We rewrote the instructions, added a checklist, added a self-check. It
worked in a fresh session and the improvement vanished entirely under a
realistic amount of prior conversation.

So: **for at least one agent we support, no amount of better writing makes
a prose instruction survive a long session** — and a framework whose whole
premise is work spanning sessions cannot rest the record on prose.

Two consequences that shape everything below. The record has to be written
by something that cannot forget. And when a builder decides to go back a
stage, *what it decided and why* has to survive, because that is the only
thing a later builder — or an initiative-level one — can read to know that
dev kept failing because the design was wrong.

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

### Guidance first, refusal second

The checker's main job is to answer *where am I and what is expected of
me*, and only then to refuse things. That ordering matters because the
builder — human or agent — is making the decisions.

So every answer carries the facts it was based on, and a refusal always
says what would resolve it. "Three plan items unticked" is not useful on
its own. "Three items unticked — tick them, or record why you are
proceeding without them" tells a builder what to do next. A refusal a
builder cannot act on turns into a retry loop; one that names the way
forward turns into a decision.

### Proceeding anyway, on the record

Humans do this constantly: *known issue, shipping anyway, follow-up
filed.* A framework with no way to express it blocks the first time
judgement disagrees with a precondition, and then nobody can drive it —
least of all an agent at initiative altitude.

So every gate has an exception path, and the exception is **expensive by
construction rather than forbidden**:

- it must name what is being skipped and why,
- it is recorded on the branch like a return,
- it is counted, and the count is visible at every altitude above it.

Nothing is prohibited. Everything is legible. That is the same trade the
return record makes, and it is what keeps a gate from becoming a wall.

The obvious worry is that a builder simply excepts past everything. The
answer is visibility, not prohibition: a feature that reached closure with
four recorded exceptions looks exactly as bad as it is, to a human or to
an initiative-level builder reading the same record.

### Repeated failure is evidence about a different stage

If the dev loop cannot get the tests passing after several attempts, the
fault is probably not in the code. That is ordinary engineering judgement,
and the framework should put the evidence in front of the builder rather
than making it notice on its own.

So attempts and returns are counted, and the counts are surfaced —
"third time round this loop" — as **input to a decision, never as a
decision.** The code counts; the builder judges where the fault entered.
No threshold decides anything on its own, because the right response to
three failed attempts depends entirely on what the failures were.

### Which layer the fault entered

A return needs to say more than "go back one". The layers a fault can
enter are finer than the stages:

| Layer | What being wrong here means |
|---|---|
| requirements | we built the wrong thing |
| design | right thing, wrong shape |
| implementation | right shape, wrong code |
| test | the code may be fine; the test was wrong |

The builder judges which one, because nothing observable distinguishes
them. The framework's job is to make that judgement **survive** — the
layer, the problem, what would resolve it, and how we will know it is
resolved are all recorded on the branch. Those four things are the record
a later builder reads. Discarding them, which an earlier version of this
design did, removes the reason the feature exists.

The stages stay four. Design is a *layer*, not a stage, because returning
to "planning" to rework a design is the same stage doing different work,
and splitting it would multiply the transition table without telling a
builder anything it did not already know from the fault layer.

### Working at more than one altitude

The same framework applies to a feature, an initiative, and a project. So
the tooling must not be hardcoded to features: which altitude a piece of
work sits at is read from the spec that names the branch, and the stage
record is scoped to that altitude so a feature's stage and an initiative's
stage never collide.

An initiative-level builder needs one thing beyond that, and it is the
capability most clearly missing today: **it must be able to ask across
units.** *Which streams in this initiative are blocked, and why?* That is
a question about many branches at once, and answering it every cycle is
how a higher-order builder decides what to do next.

### Describing itself

"Self-documented enough for a coding agent to drive it" has to mean
legible to a program, not only readable by a person. A builder that has
just started should be able to ask the framework what altitudes exist,
what stages each has, what finishing a stage requires, and what a return
or an exception must record — rather than inferring it from prose it may
not have loaded.

That is a small surface: the shape of the framework, printed on request,
from the same source the enforcement uses. Two copies of that knowledge —
one in prose and one in code — is the drift this feature set out to
remove.

### Who owns the map

A phase module states **what it must achieve**, not what follows it. Dev's
instruction file says dev is finished when the plan is ticked, an
implementation commit exists and the gates have been observed passing —
and then asks `phase advance --next`. It never names `review`.

This matters for two reasons. A module that named its successor would
duplicate the table of legal moves, and the duplicate would drift the
moment a stage was added or reordered. And a module that knows only its
own exit condition can be dropped into a different sequence unchanged —
adding a `review.md` later requires no edit to `dev.md`.

So the division of labour is:

- **The agent decides** whether the work is actually finished, and when
  something is wrong, which layer the fault entered. Both are judgement,
  and neither is computable.
- **The tooling owns** which moves exist and whether the observable facts
  permit one. It refuses `--next` exactly as it refuses a named move, so
  asking "where next" is not a way around a gate.

Neither half is sufficient. The agent without the tooling forgets the
map under load, which is the bug this feature exists to fix. The tooling
without the agent cannot tell whether the work is any good. The value is
the pairing, and the line between them is: **judgement is the agent's,
the map is the code's.**

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
| Whether the work is good | The builder, always |
| Which layer a fault entered | The builder; the framework records it |
| Whether to proceed despite a gate | The builder, on the record |
| Which instruction file to load | The router, from what the checker reports |
| What comes after this stage | The checker's table — never the phase module |
| Whether the work in this stage is finished | The agent, against the module's stated exit condition |
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

Any earlier stage can be returned to, and that is not restricted —
deciding where a mistake belongs is judgement, and code cannot do it.

What the framework enforces is that the decision **survives**. A return
records four things, and refuses without them:

| Recorded | Why a later builder needs it |
|---|---|
| the layer at fault | tells it whether to rethink the requirement, the design, or the code |
| the problem | tells it what was actually wrong |
| what would resolve it | tells it what to do |
| how we will know | tells it when it is done |

These are written onto the branch, not into a scratch file, because the
branch is what a later builder reads. Returns are counted, so repeated
bouncing is visible rather than hidden.

A return also **rewinds what counts as evidence**: work done before the
fault was work on the old understanding, so reaching a later stage again
needs fresh work. And a return is open while it is the newest thing on the
branch — the work that addresses it is what closes it.

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

Statements a person can check by reading the repository. They say nothing
about how they are achieved. Grouped by the two jobs.

**The record is trustworthy.**

1. A builder starting with no knowledge of prior work can determine the
   current stage from the repository alone.
2. A commit whose stage claim the repository contradicts does not land.
3. Statement 2 holds even with `--no-verify`.
4. The record never falls behind reality: work that has plainly been done
   is reflected without anyone remembering to say so.
5. The record never moves backwards except through a recorded return.
6. When something cannot be determined, no transition happens.

**Decisions survive.**

7. A return records the layer at fault, the problem, what would resolve
   it, and how we will know — all readable from the branch afterwards.
8. After a return, reaching a later stage again requires fresh work.
9. Returns and repeated attempts are counted, and the counts are readable
   without parsing prose.
10. A builder can proceed past a gate by recording what it is skipping and
    why; doing so is counted and visible.

**The flow holds without correction.**

11. A builder following only what the framework tells it can get a feature
    from planning to review without a human intervening.
12. Iterating the dev loop does not change the stage and does not count as
    going back.
13. A feature cannot leave dev until its gates have been observed passing,
    or an exception has been recorded.
14. Recorded evidence is invalidated by changing the files it covered.
15. Nothing is stamped or refused outside the piece of work it belongs to
    — not the default branch, not an unrelated branch, not another
    altitude.
16. It works whether or not the work has its own worktree.

**A higher-order builder can drive it.**

17. The framework can describe itself on request: the altitudes, the
    stages, what finishing a stage requires, and what a return or
    exception must record.
18. A builder can ask which units of work are blocked and why, across a
    whole initiative, without reading each branch by hand.
19. The same verbs work at feature and initiative altitude, and the two
    stage records do not collide.

**Everywhere.**

20. After a feature merges, no stage bookkeeping remains on the main
    branch, and the merge commit carries an authored summary rather than a
    transcript of branch commits.
21. With none of this installed, kdevkit behaves as it does today.
22. Statements 1-19 hold on claude, codex and kiro.

## How we will test it

Tests are written from the statements above, not from the design. An
assertion may describe what someone would find in the repository; it may
not describe which script ran. If the mechanism were rebuilt differently,
these should still pass.

### Four layers, and what each can and cannot prove

**Unit — one statement at a time, no agent.** A throwaway repository
driven with git commands. Fast, free, repeatable, every build. Proves the
mechanism behaves. Cannot prove the pieces work together, and cannot
prove a builder will use them.

**Lifecycle — the statements together, no agent.** A piece of work driven
from nothing to closed: every stage, the dev loop turning, a return two
stages back that then recovers, a long session with detours. Proves the
whole holds when the steps are taken. Still cannot prove anyone takes
them.

**Fixture integrity — free, and it guards the paid layer.** Runs each
agent fixture's setup with *no agent work* and requires the assert to
fail; also guts the record instead of doing the work and requires that to
fail. Without this a paid run can report a clean pass while testing
nothing — which has already happened once here.

**Agent runs — the only layer that tests the actual claim.** A real
builder, on claude, codex and kiro. Everything above proves the tooling;
only this proves the framework. Nothing substitutes for it, and it is
worth the money precisely because the first two layers are written by
someone who knows the design and will unconsciously test what they built.

### What the agent runs must measure

The metric is **correction burden** — how often a human has to step in —
not obedience. Every defect found in this work so far was found by a human
noticing something wrong, which is the cost the feature exists to remove.
So the fixtures assert:

- **Did the builder get from A to B unaided?** Statement 11. This needs a
  fixture that poses a situation and reads the end state, with no
  intervening correction.
- **Did the decision survive?** Statement 7. Not "did it go back" but "can
  the four things it recorded be read afterwards".
- **Did it reach for the right mechanism when judgement was needed?** When
  a gate cannot be passed honestly, did the builder record an exception or
  a return, rather than either stalling or quietly proceeding?
- **Did it stay inside its own work?** Statement 15, with detours onto
  other branches.

Two rules on sampling, from this project's own experience: **one run
proves nothing** — at least three per fixture per agent, with the ratio
recorded rather than a verdict; and every fixture also runs **under load**,
with unrelated prior conversation prepended, because the entire problem
only appears under load.

### Rules the assertions must obey

Each of these exists because it caught a real defect here.

- **An assertion must fail if the work was not done.** Write down what a
  no-op builder leaves behind and what a careless one leaves behind, and
  confirm both fail.
- **Never assert only an absence.** "No stale stage" is satisfied by
  deleting the field. Assert it exists, is legal, and is right.
- **Assert against the artefact a reader would see** — a fresh clone, not
  the working copy; the remote, not the local branch.
- **Seed the project's real formats.** A fixture that seeds a format the
  project does not use tests nothing: this is how a format mismatch
  survived every layer here undetected.
- **Check exit codes and error text, not just success.**

### Which statements each layer proves

| Statements | Unit | Lifecycle | Agent |
|---|---|---|---|
| 1-6 · record is trustworthy | ✓ | ✓ | ✓ (1, 4) |
| 7-10 · decisions survive | ✓ | ✓ | ✓ (7, 10) |
| 11 · no correction needed | — | — | **✓ only here** |
| 12-16 · flow holds | ✓ | ✓ | ✓ (12, 15) |
| 17-19 · higher-order use | ✓ | ✓ (18) | ✓ (19) |
| 20-21 · merge and degradation | — | ✓ | ✓ (21) |
| 22 · all three agents | — | — | **✓ only here** |

Statement 11 and statement 22 have no non-agent proof at all. That is the
argument for the paid layer stated as plainly as it can be.

## How to build it

Ordered so each step is useful alone. Items marked `[x]` are done, `[~]`
partly, `[ ]` not started.

- [x] 1 · The checker's `facts` verb, and a test per fact.
- [x] 2 · The closed table of stage moves, and `check`, including the
  cannot-determine answer.
- [x] 3 · The commit-time hook: stamp the stage from evidence, refuse
  contradictions, survive `--no-verify`.
- [x] 4 · Derive the stage from evidence so the record cannot fall behind,
  never inferring stages that are human acts.
- [x] 5 · A return rewinds what counts as evidence.
- [x] 6 · The pre-push gate, running the project's declared checks.
- [x] 7 · The hook scopes itself, and chains any hook already installed.
- [ ] 8 · **Fix the spec-format match.** The checker identifies a unit of
  work by matching the branch name in a spec; it matches a format this
  project does not use, so the mechanism is currently inert on every real
  spec. Test against the project's actual specs, not a seeded format.
- [ ] 9 · **Persist the return record.** The four fields are validated and
  then discarded. Write them onto the branch and make them readable.
  Statement 7.
- [ ] 10 · **Add the exception path.** Proceed past a gate by recording
  what and why; count it; surface it. Statements 10 and 11.
- [ ] 11 · **Fix the push gate's control flow**, which currently makes its
  own refusal message unreachable and blocks pushes when a project
  declares no gates.
- [ ] 12 · **Surface the counts** — returns, exceptions, attempts — as
  decision input. Statement 9.
- [ ] 13 · **Make it altitude-generic**: read the altitude from the spec
  that names the branch; scope the stage record to it. Statement 19.
- [ ] 14 · **Add the across-units query**: which units are blocked and
  why. Statement 18.
- [ ] 15 · **Add self-description**: the framework's shape, printed from
  the same source the enforcement uses. Statement 17.
- [ ] 16 · **Reachability**: name the install and verify steps in the
  prose, and substitute the tool's real path at install time. Without this
  no real deployment wires anything up.
- [ ] 17 · Make every refusal name what would resolve it.
- [ ] 18 · Add a shell linter to the quality gate; the guarantee now lives
  in shell and nothing checks it.
- [ ] 19 · Authored merge summary. Statement 20.
- [ ] 20 · Per-stage capability limits where the agent supports them.
- [ ] 21 · Rebuild the agent fixtures around correction burden, seeded
  with real formats; three samples per agent, fresh and under load, ratios
  recorded.
- [ ] 22 · Correct the inaccurate claims in the existing specs.

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
- **Nothing yet forces `advance` to be called.** The evidence-derivation
  fix means a forgotten `advance` no longer produces a *wrong* record, but
  the stage then moves without an `Acked-By` attribution — so the audit
  trail of who approved a move is only as complete as the agent's
  diligence. Whether that matters depends on whether approval is meant to
  be evidence or merely a courtesy, which is not settled.
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
- **2026-09-01 · The intent is a framework a builder is guided through,
  not a guard against a forgetful agent.** Rationale: the original problem
  statement was "the agent forgets to update the handoff", which produced a
  design made almost entirely of refusals. The actual purpose is that any
  builder — human, or a coding agent playing that role at feature or
  initiative altitude — can reconstruct where things stand, what has been
  tried, and what went wrong where, without the conversation that produced
  it. Consequences: guidance before refusal; the return record becomes the
  point rather than a side effect of a gate; gates force articulation, not
  obedience, so every gate gains a recorded-exception path; counts are
  surfaced as decision input; and the tooling must work at more than one
  altitude and be able to answer questions across units. Rejected: keeping
  the guard framing and treating the higher-order use as a later feature —
  it changes what the record must contain, which is not a bolt-on.

- **2026-09-01 · Enforce record consistency, never permission.** Rationale:
  the two jobs — a flow that holds without human correction, and a flow an
  agent can drive — conflict wherever a gate can refuse a legitimate
  decision, because the driver is then stuck with no way forward. A gate
  that makes a builder *say* what it is doing serves both. Exceptions are
  made expensive by construction (named, counted, visible at every altitude
  above) rather than forbidden. Rejected: hard gates with no override,
  which dead-end the first time judgement disagrees; and soft gates with no
  record, which is where we started.

- **2026-09-01 · Design is a fault layer, not a stage.** Rationale: a
  return must say which layer the fault entered — requirements, design,
  implementation or test — and that precision belongs in the record, not in
  the transition table. Returning to planning to rework a design is the
  same stage doing different work. Rejected: splitting design into its own
  stage, which multiplies the table without telling a builder anything the
  fault layer does not already say.

- **2026-09-01 · A phase module states its exit condition; the tooling
  owns the map.** Rationale: a module that names its successor duplicates
  the table of legal moves and will drift when a stage is added, and it
  cannot be reused in a different sequence. `advance --next` asks the
  tooling where the module's exit condition leads, and is gated exactly as
  a named move is, so it is not a way around a check. The division: the
  agent judges whether the work is done and which layer a fault entered;
  the code owns which moves exist and whether the facts permit one.
  Rejected: keeping the destination in each module's prose, which is the
  duplication this feature set out to remove.

- **2026-09-01 · Per-project gate commands live in `specs/project.md`,
  not git config.** Rationale: git config is per-clone, so a fresh clone
  would silently have no gates to run and verification would report
  nothing to check; and the project already declares its reviewer in that
  block, so there is one place to look. Rejected: git config, which does
  not travel with the repository.

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
