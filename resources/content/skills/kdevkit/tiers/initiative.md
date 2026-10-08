# kdevkit - initiative tier (tier module)

Carries the **initiative tier**: a goal too big for one branch,
delivered as several features, either guided by the user feature by
feature or run for them by a **ringmaster** session.

**Read this when** an initiative is in play: an initiative verb fires,
the work names a file in `$SPEC_ROOT/initiative/`, or a feature spec
carries `Part of initiative:`. It applies during any phase.

The initiative template and interview live in `interviews.md`.

## 10 · Initiative tier

An initiative is a goal that lands as several features, one branch
each. A large change that ships as one branch stays a feature.

### Three ways to work; the user picks

- **A feature**: the normal flow (§3-§8), no initiative.
- **A guided initiative**: the user plans the streams (or asks for a
  proposal) and starts and steers each feature session themselves.
  Each feature opens a PR into `initiative/<name>`, so every feature
  gets a human review.
- **A ringmaster initiative**: the user approves the spec and says
  "run initiative"; one session does the rest (below). No PR until the
  end.

Both initiative modes share the spec, the Streams table, the branch
shape and the final PR, so the user can switch mid-way: "run
initiative" picks up from the Streams table.

### The split of work

- **The user** writes what they want: the **Goal**, the
  **Experience** (what they can do and observe when it is done), the
  **Macro test** that proves it, and the **Constraints**.
- **The ringmaster** (ringmaster mode: the session the user runs the
  initiative from) decides the features, runs each one in its own
  session and worktree, and plays the user at every feature gate. It
  judges like the senior developer beside the user would (the judge:
  `kyodakit` unless `judgement:` names another), against the
  Experience and Macro test. It never edits code.
- **A feature session** runs the normal feature flow on its own
  branch. Under a ringmaster, the ringmaster's messages are its
  user's words.

### Verbs

Things the user says in an ordinary agent session; the session does
the work with the agent's own tools, not a wrapper.

- **"start initiative `<name>`"**: make the initiative's worktree
  (`git worktree add <worktrees>/initiative-<name> -b
  initiative/<name> main`, in the project's worktree convention),
  interview the user (`interviews.md`), write
  `$SPEC_ROOT/initiative/<name>.md` there, and commit
  `plan(<name>): initial spec`. Stop for approval.
- **"start `<feature>` for initiative `<name>`"** (guided): add a
  Streams row if there is none, make the feature's worktree cut from
  `initiative/<name>`, and run the normal feature flow there with the
  user as the user. Its PR targets `initiative/<name>` (push that
  branch first if the remote lacks it; the user chose guided mode, so
  this push is theirs), and closure squash-merges into it with the
  message §8.6 asks for.
- **"run initiative `<name>`"**: become its ringmaster (below). Also
  the way to resume one: everything needed is on disk.
- **"close initiative `<name>`"** (either mode): run Finish (step 6
  below) once every Streams row is merged. A ringmaster reaches it
  on its own.
- **"show initiatives"**: list the `initiative/*` branches, local and
  remote, with each one's Goal line. Read-only.

### Git shape

```
main ──────────────────────────────●  merge commit (--no-ff), the user's
  └─ initiative/<name> ─ f1 ─ f2 ─ f3     one squash commit per feature
       └─ feat/<feature>   (worktree per feature, cut from the initiative branch)
```

- Each feature squash-merges into `initiative/<name>`, with the
  message §8.6 asks for. Under a ringmaster, feature branches are
  never pushed and open no PR; guided, each has its PR.
- The initiative reaches `main` as one merge commit, so
  `git log --first-parent main` shows the initiative and
  `git log <merge>^1..<merge>^2` its features. Its message is
  authored like §8.6's squash message and passed explicitly
  (`git merge --no-ff -m`, or the forge's merge-commit message
  field), never the host default: subject `feat(<name>): <the
  Goal in one line>`, body the Goal and one line per feature.
- When a feature lands while another is in flight, the other rebases
  onto `initiative/<name>` and re-runs its gates before its next stop.

### The ringmaster loop

1. **Ground.** Read `project.md`, the initiative spec, its Streams
   table, and `git worktree list`. Work from the initiative's own
   worktree, made by "start initiative"; if it is missing (a fresh
   machine), add it for the existing `initiative/<name>` branch.
2. **Plan the streams** if the table is empty: the fewest features
   that each ship something the Macro test can see, with what each
   needs before it can start. Commit `plan(<name>): streams`.
3. **Spawn** every stream whose needs have merged:
   `git worktree add <worktrees>/<feature> -b feat/<feature>
   initiative/<name>`, then a background feature session (Claude
   Code: the Agent tool, not isolated, since the worktree exists;
   other hosts: their headless CLI started in the worktree). Its
   brief, per §9's packet contract:
   - Receives: "load kdevkit and its judge the way this agent loads
     skills" (by name, since an install path changes as the project
     installs), plus the path of each one's `SKILL.md` as installed
     right now, for a session whose agent cannot find them by name
     (an agent may hold the skill list it started with); the worktree path (and that the session's
     shell may not keep a `cd` between commands, so every command
     names it);
     the initiative spec path and its stream row; what merged
     streams settled that this one builds on, and facts already
     checked, each marked "verify what you build on"; "run kdevkit's feature flow for `<feature>`; I am
     your user; stop at each gate and reply with what the gate's
     PR body would carry"; any paid-test allowance from Constraints.
   - Excluded: this session's history and the other streams' work.
   - Returns: at each stop, the gate it reached and the spec path.
4. **Play the user at each stop.** Read the artefact on disk (the
   spec, the briefing, the diff against `initiative/<name>`), never
   only the summary. Then resume the session with either the cue
   (`spec looks good`, `ship it`) or a return naming fault layer,
   issue, fix and done-when (§5). Ask what a senior reviewer would:
   does this serve the Experience, is it the least that does, does
   the test prove it.
5. **Merge.** On closure, the feature session makes its `close()`
   commit, stops, and replies with the squash message it proposes
   (§8.6). The ringmaster checks the message against the diff,
   squash-merges the branch into `initiative/<name>` with it, updates the Streams row, and spawns whatever
   that unblocked. The feature's worktree and branch stay, for the
   user to inspect, until the initiative closes.
6. **Finish** (both modes). When every stream has merged:
   - Run the Macro test on the initiative branch and record the
     result in the spec.
   - Write the initiative PR's body. With `review_brief` enabled,
     dispatch the review briefing (§7, `phases/review.md`) at
     initiative level; otherwise write the §9 body covering the same
     ground (requirements as understood, design decisions and their
     effect on the existing design, the changes in review order, what
     the tests say). The briefing receives the initiative spec as
     the spec, each feature spec, the diff `main...initiative/<name>`,
     and the Macro test result. Route its defects back as streams' fixes,
     as for any feature.
   - Commit `close(<name>):` (Streams all merged; the spec stays as
     the record), and remove the initiative's feature worktrees and
     branches.
   - Stop for the user with the briefing and the merge message
     drafted: pushing the branch, the PR, and the merge commit to
     `main` are theirs.

### The record

Nothing new: git log, the initiative spec, and each feature spec's
Handoff (§5). Whoever runs the streams (the ringmaster, or the user
when guiding) writes only:

- **Streams**: one row per feature, `planned → running → merged`
  (or `blocked: <why>`).
- **Decision Log**: one line per call it made for the user,
  `- Ruling: <what> · why: <why> · cost if wrong: <cost>`.

A ringmaster resuming in a new session rebuilds from those, the
branches, and the worktrees. A stream whose session is gone gets a
fresh one on its worktree; its spec's Handoff says where it was.

### When the ringmaster stops for the user

Only for:

- an outward or irreversible action: push, PR, merge to `main`,
  publish, deleting anything not made by this initiative;
- a question whose answer changes the Experience or Macro test;
- three rounds on the same problem without progress (the judge's
  rule);
- spend the Constraints do not allow, such as paid tests over budget.

Anything else it decides, records as a Ruling, and carries on. It
does not ask whether to continue.
