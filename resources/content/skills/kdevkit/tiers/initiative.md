# kdevkit - initiative tier (tier module)

Carries the **initiative tier**: a goal too big for one branch,
delivered as several features by a **ringmaster** session that runs
them for the user.

**Read this when** an initiative is in play: an initiative verb fires
("start initiative", "run initiative", "show initiatives"), the work
names a file in `$SPEC_ROOT/initiative/`, or you are a feature session
whose brief names an initiative. It applies during any phase.

The initiative template and interview live in `interviews.md`.

## 10 · Initiative tier

An initiative is a goal that lands as several features, one branch
each. A large change that ships as one branch stays a feature.

### The split of work

- **The user** writes what they want: the **Goal**, the
  **Experience** (what they can do and observe when it is done), the
  **Macro test** that proves it, and the **Constraints**. Approving
  that spec is the user's one gate.
- **The ringmaster** (the session the user runs the initiative from)
  decides the features, runs each one in its own session and
  worktree, and plays the user at every feature gate. It judges like
  the senior developer beside the user would (the judge: `kyodakit`
  unless `judgement:` names another), against the Experience and
  Macro test. It never edits
  code.
- **A feature session** runs the normal feature flow (§3-§8) on its
  own branch. The ringmaster's messages are its user's words.

### Verbs

- **"start initiative `<name>`"**: interview the user (`interviews.md`),
  write `$SPEC_ROOT/initiative/<name>.md`, add the `## Active
  initiatives` index line, commit `plan(<name>): initial spec` on a
  new branch `initiative/<name>` cut from `main`. Stop for approval.
- **"run initiative `<name>`"**: become its ringmaster (below). Also
  the way to resume one: everything needed is on disk.
- **"show initiatives"**: list the index. Read-only.

### Git shape

```
main ──────────────────────────────●  merge commit (--no-ff), the user's
  └─ initiative/<name> ─ f1 ─ f2 ─ f3     one squash commit per feature
       └─ feat/<feature>   (worktree per feature, cut from the initiative branch)
```

- Each feature squash-merges into `initiative/<name>`, with the
  message §8.6 asks for. Feature branches are never pushed and open
  no PR.
- The initiative reaches `main` as one merge commit, so
  `git log --first-parent main` shows the initiative and
  `git log <merge>^1..<merge>^2` its features.
- When a feature lands while another is in flight, the other rebases
  onto `initiative/<name>` and re-runs its gates before its next stop.

### The ringmaster loop

1. **Ground.** Read `project.md`, the initiative spec, its Streams
   table, and `git worktree list`. Work from the initiative's own
   worktree (the project's worktree convention, `initiative-<name>`).
2. **Plan the streams** if the table is empty: the fewest features
   that each ship something the Macro test can see, with what each
   needs before it can start. Commit `plan(<name>): streams`.
3. **Spawn** every stream whose needs have merged:
   `git worktree add <worktrees>/<feature> -b feat/<feature>
   initiative/<name>`, then a background feature session (Claude
   Code: the Agent tool, not isolated, since the worktree exists;
   other hosts: their headless CLI started in the worktree). Its
   brief, per §9's packet contract:
   - Receives: the worktree path; the initiative spec path and its
     stream row; "run kdevkit's feature flow for `<feature>`; I am
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
   commit and stops. The ringmaster squash-merges the branch into
   `initiative/<name>`, updates the Streams row, removes the worktree
   and branch, and spawns whatever that unblocked.
6. **Finish.** When every stream has merged, run the Macro test on
   the initiative branch and record the result in the spec. Commit
   `close(<name>):` (Streams all merged, index line removed; the spec
   stays as the record). Then stop for the user: pushing the branch,
   the PR, and the merge commit to `main` are theirs.

### The record

Nothing new: git log, the initiative spec, and each feature spec's
Handoff (§5). The ringmaster writes only:

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
