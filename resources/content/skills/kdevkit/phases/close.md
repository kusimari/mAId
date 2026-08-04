# kdevkit — closure (stage module)

Carries the **closure phase**: reconciling in-flight markers, the
persistent-layer verify, backlog cleanup, initiative status update,
the Closure Review Gate, squash-merge, and branch / worktree
teardown.

**Read this when** the closure cue has fired — `"close it"` /
`"ship it"` / `"merge it"` / `"feature done"`. Closure is gated on
that explicit cue: passing gates are necessary but never
sufficient (`SKILL.md` §9, no premature closure).

## 8 · Closure

Closes the **feature loop**. Trigger: an explicit cue —
`"feature done"` / `"close it"` / `"ship it"` / `"merge it"`.

The closure cycle reuses `phases/review.md` §7's **comment-prefix
convention** for
any agent-authored CR/PR comments posted during reconcile or
the Closure Review Gate.

Steps 1–3 stage spec / docs / backlog edits as
`close(<feature>):` commits before the §8.6 squash; step 3
must be asked even when the answer is "none" — *asking is the
artifact*.

**1 · Reconcile in-flight markers.** Sweep
`$SPEC_ROOT/feature/<feature>.md`. Implementation Plan items
in checkbox shape: literal grep for `- [ ]` markers; tick to
`- [x]` if quietly done, or move out (backlog or follow-up
feature). Implementation Plan items in older prose-numbered
shape: read each and resolve. Then sweep open Decision Log
entries and unresolved questions the same way. The merged
spec is "done in place" — do not move directories. Stage
edits.

**2 · Persistent-layer verify (per touched section).** Closure
bubbles durable content up out of the transient feature spec into
the two persistent layers (§2 Context layers). For each
`project.md` section the feature touched — Mission, Architecture,
Tech Stack, Layout, Testing, Deployment, Hard constraints, Agent
Development — ask one targeted question: _"Did this feature change
what's documented under \<section\>?"_. Asking is mandatory;
declining the edit is fine. Stage any accepted edits.

**Operational changes go to `AGENTS.md`, not `project.md`.** If
the feature changed a build/test/lint command or another
operational fact and the repo keeps a root `AGENTS.md`, the edit
lands there (kept lean, per §2's convention) — `project.md`
Testing keeps only the layer semantics. **Binding decisions
bubble up as rationale:** a Decision Log entry that constrains
*future* features gets its *why* folded into the relevant
`project.md` section (not copied verbatim, not a standing
decisions log). Non-binding decisions stay in the feature spec's
Decision Log, archived in place with the feature.

Decide which sections were touched from the diff:

- **Tech Stack** — a dependency added/removed, or a runtime
  version moved.
- **Layout** — a top-level directory or file gained/lost,
  per project.md's tree.
- **Testing** — a test command added/removed, or a layer's
  semantics changed.
- **Deployment** — the deploy/install path or registry
  changed.
- **Architecture** — a documented moving part gained or
  lost a responsibility.
- **Mission** — meaningful shift in what the project is
  for. Rare.
- **Hard constraints** — a new invariant, or an old one
  weakened.
- **Agent Development** — a `kdevkit` (or other skill)
  block key changed, or a new skill-scoped preference
  landed.
- **AGENTS.md (operational)** — a build/test/lint command,
  code-style rule, or PR/commit convention changed, and the
  repo keeps a root `AGENTS.md`. The edit lands there, lean.

Untouched sections aren't asked about. The asking is the
artifact; the user can answer "no, project.md is fine"
for every touched section and closure proceeds.

**3 · Backlog cleanup (interactive).** List
`$SPEC_ROOT/backlog/`; ask: _"Which backlog items did this
feature close out? Pick any, or 'none'."_ `git rm` the chosen
ones; asking is mandatory even when the answer is "none".

**3.5 · Initiative Status update (auto).** If the closing
feature is a stream of an active initiative (the feature spec
carries `Part of initiative: [[<name>]]` near the top), update
the initiative's Status table row: branch, CR, status =
`shipped`, ship date, one-line learning. Stage the edit. If
this is the **last** stream (every other row in the Status
table is already `shipped`), the same staged edit also
archives the initiative spec — `git rm
$SPEC_ROOT/initiative/<name>.md` and remove the line from
`project.md`'s `## Active initiatives` index (the index is a
bullet list; the Status table is the per-initiative file). No
separate `close(<initiative>):` commit; the last stream's
`close(<feature>):` does the work. See §10 for the table
format.

**4 · Commit + push.** Staged closure edits land in one or
more `close(<feature>):` commits per §9. Push.

**5 · Closure Review Gate.** Apply §9 Review Gates. Body
rewritten to final shape; phase-specific content: **Approach**
+ **Verification** (required at close-out) + optional **Spec
& docs touched at close-out**. **Title rewritten** to the
dominant agent-dev subject (`feat(<scope>): subject` etc.) —
*not* the `close(<feature>):` subject — so the squash-merge
commit on `main` reads as a feature ship, not a closure
mechanic.

**6 · Squash merge to `main`** — one logical commit per
feature. Exceptions:

- Single-commit branch: squash and plain merge are equivalent.
- Branch with *several* logical features (rare): one squash
  merge per logical feature.
- Non-linear `main` by convention: squash still works; surface
  before going non-default.
- FF-only `main`: squash locally, then commit and push (review
  tool can't be the merger).

**7 · Branch cleanup.** Delete the feature branch local +
remote; prune stale refs. Default delete, one line, no
permission pause.

**8 · Worktree teardown — offer-only.** Non-primary worktree →
surface path and offer removal. Do not auto-remove — artifacts
may be worth inspecting.

