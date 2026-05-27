# Feature: kdevkit-feature-and-agent-dev-loops

## Git Setup

- Branch: `feat/kdevkit-feature-and-agent-dev-loops`
- Base: `main`

## Feature Brief

Codify kdevkit's three-loop mental model — project → feature →
agent-dev — and add the two missing close-outs. The agent-dev
loop gains a terminal **Review Gate** (always-on after Quality +
Test pass) that opens a PR or CR with a `Why + Approach` body
on the host detected from `git remote` or configured in
`project.md`. The feature loop gains an explicit close-out
sequence — squash-merge to main, branch cleanup local + remote,
soft `project.md` verification, interactive backlog cleanup,
and an offer to tear down the worktree if the work happened in
one. Worktree usage is recommended at feature-start when
`project.md` declares a preference, but never gates the
automation.

## Requirements

### Mental model

- Three loops, named explicitly in the skill body so other
  sections can reference them: **project loop** (invariants,
  cross-feature), **feature loop** (one branch, one merge),
  **agent-dev loop** (Quality → Test → Push → Review).
- Each inner loop's terminal step is the trigger for re-entering
  the outer loop.

### Worktree handling (feature-start recommendation)

- At feature-start (§3), check `project.md` for a worktree
  preference signal — either an `## Agent Development` →
  `kdevkit` block declaring `prefer_worktree: true|false`, or a
  bullet in **Hard constraints** mentioning worktrees.
- If preference is "prefer," suggest
  `git worktree add ../<repo>-<feature> -b <branch>`. Do not
  auto-run.
- If silent, continue branch-only without prompting.

### Agent-dev loop close-out (Review Gate, §8)

- Always-on after both Quality and Test gates pass — not gated
  on worktree mode.
- **Refuse-on-fail.** If either gate failed (or finished with
  residual issues per the §8 self-review threshold), do not open
  a review. Surface the failure; require an explicit override.
- **Review-tool detection** — resolution order:
  1. `git remote get-url origin` → host match.
     `github.com` → `gh` (built-in mapping; other public hosts
     can be added as the project grows).
  2. `## Agent Development` → `kdevkit` block in `project.md`
     declaring the per-project review CLI (commands for
     create / edit / list-by-branch / merge).
  3. Ask the user once and offer to write the answer into
     `project.md` so future sessions skip this step.
- **Body shape — light.** Title: `type(scope): subject`. Body:
  `Why` (one paragraph — motivation) + `Approach` (bullets —
  the actual changes). Suggest `Verification` /
  `Reading guide` / `Pairs with` sections when the diff is
  large enough to warrant them; do not require them.
- **Body grep.** Run §7's internal-marker grep against the
  prepared body string before submission. Hit → fail loud,
  surface lines, abort.
- **Update vs. create.** Look up an existing review for the
  branch (`gh pr list --head <branch>` or per-project
  equivalent). Update body if found; otherwise create.
- **Return URL** as the last line of inner-loop output.

### Feature close-out loop (new §9)

- Trigger: explicit human cue — "feature done" / "close it" /
  "ship it" / "merge it" / equivalent.
- **Squash merge** to `main` by default — one logical commit
  per feature on the main history.
  - Single-commit branch: squash and plain merge are equivalent;
    either is fine.
  - Multiple commits: squash. If the branch contains work for
    several logical features, break into multiple squash merges
    per logical feature.
  - Repo with non-linear main as the norm: surface choice before
    going non-default.
  - Repo enforcing fast-forward only on main: use local
    `git merge --squash` + commit + push.
- **Branch cleanup** — delete feature branch local + remote
  without per-step prompts. `gh pr merge --delete-branch` or,
  on the local-merge path, `git branch -D <feat>` +
  `git push origin --delete <feat>` + `git fetch --prune`.
  Surface the deletion as one line.
- **`project.md` verify (soft).** Reuse §6's existing offer:
  _"Shall I update `project.md` with what changed?"_. User
  decline is fine; close-out continues either way.
- **Backlog cleanup (interactive).** List
  `$SPEC_ROOT/backlog/` contents; ask the user which items this
  feature resolves; `git rm` the chosen ones. No frontmatter
  pointer needed.
- **Worktree teardown (offer-only).** If the working directory
  is a worktree, surface the path and offer
  `git worktree remove <path>`. Do not auto-remove — the
  worktree may have artifacts to inspect.
- **Reconcile in-flight markers** in the feature spec
  (unfinished Implementation Plan items, open Decision Log
  entries) so the spec is a faithful record of what shipped.

### Phase-gating compatibility (§6)

- Push + Review-open is a single phase (Push). Both gates green
  is implicit approval to close the agent-dev loop.
- Feature close-out (§9) is a distinct phase, gated on an
  explicit human cue.

### Public-repo hygiene (§7)

- Extend the existing pre-push internal-marker grep to also run
  against the prepared review body string before submission.

### Out of scope

- Cross-repo "Pairs with" automation (cross-PR linking is
  user-supplied content in the body, not detected).
- Auto-running `git worktree add` at feature-start.
- Auto-removing the worktree at close-out.
- Hard-blocking close-out on `project.md` updates.
- Dry-run preview of the prepared review body before
  submission.

## Design

The skill is markdown — there is no executable component. All
behavior is encoded as instructions the agent follows when the
skill is loaded. The change set:

1. **Frontmatter.** Bump `version: 2.1.0` → `2.2.0`. Extend the
   `description` line with a phrase about the three-loop close-
   outs.

2. **Preamble paragraph (new — right after the opening
   "Self-contained methodology…" block).** Name the three loops
   explicitly so later sections refer to them by name.

3. **§3 (Load feature context).** Append a "Worktree
   recommendation" subsection with the detection logic + the
   suggested `git worktree add` invocation.

4. **§6 (Session behaviour).** Add one sentence to the phase-
   gating clause clarifying Push + Review-open as a single
   phase, and §9 close-out as a distinct phase.

5. **§7 (Public-repo hygiene).** One-line edit to the existing
   pre-push grep paragraph: extend coverage to the prepared
   body string.

6. **§8 (Quality → Test → Push loop) → renamed
   "Quality → Test → Push → Review loop."** Add the Review Gate
   as a fourth gate after Push, with the resolution order, body
   shape, body grep, update-vs-create, and return-URL behavior.

7. **§9 (new — "Feature close-out loop").** Squash merge,
   branch cleanup, soft project.md verify, interactive backlog
   cleanup, worktree teardown offer, in-flight marker
   reconciliation.

The change touches only `sources/skills/kdevkit/SKILL.md` and
this feature spec. No code changes.

## Test Strategy

- **Quality gate (load-bearing):** `deno task fmt` +
  `deno task lint` + `deno task check`. Skill is markdown so
  the gate runs against the broader workspace; the SKILL.md
  must remain a well-formed file.
- **Test gate (load-bearing):** `deno task test` — the existing
  22-test suite. Should pass unchanged; the skill change does
  not touch any consumed surface.
- **Functional smoke (load-bearing):**
  `./tests/functional/run --no-tools` confirms the symlinks
  still resolve. Tool-mode smoke (`./tests/functional/run`)
  with whichever CLI is available confirms the skill still
  loads in a live session — the existing
  `tests/functional/skills/kdevkit.smoke` is a load-only check;
  no new fixture is needed for v1.
- **Manual verification (out-of-band):** in a scratch worktree
  on a public fork, exercise the inner-loop and feature
  close-out behaviors against a trivial diff. This is the only
  way to validate the *behavior* of a markdown skill. Not
  blocking the merge of this feature.

## Implementation Plan

1. Bump frontmatter version + description in
   `sources/skills/kdevkit/SKILL.md`.
2. Insert the "Three loops" preamble paragraph after the
   opening section.
3. Extend §3 with the worktree-recommendation subsection.
4. Add the phase-gating sentence to §6.
5. Edit §7's pre-push-grep paragraph to cover the body string.
6. Rewrite §8: rename, add Review Gate as the fourth gate, fold
   the existing Push Gate language so it points forward to the
   Review Gate.
7. Append §9 (Feature close-out loop).
8. Run the local Quality + Test gates against the workspace.
9. Run the functional smoke.
10. Stage + commit per Conventional Commits; push to a feature
    branch; open the PR via the inner-loop close-out (dogfood
    the new behavior).

### Risk notes

- The skill is consumed live by every kdevkit-aware session.
  Misnumbered headings or broken markdown ripple immediately.
  The verification step explicitly reads the diff to confirm
  §1–§9 number cleanly.
- The `## Agent Development` → `kdevkit` block referenced for
  per-project review-CLI config is already mentioned in §2's
  optional-section paragraph but no project currently uses it.
  This feature codifies the first concrete consumer; the schema
  for the block (which keys it accepts) is implicit in §8's
  enumeration ("commands for create / edit / list-by-branch /
  merge"). Future projects that adopt non-`gh` review tools
  will exercise the schema first; expect minor revisions then.

## Session Log

- 2026-05-27 · promoted backlog item
  `kdevkit-agentic-worktree-push-loop.md` to feature
  `kdevkit-feature-and-agent-dev-loops.md` via `git mv`. Filled
  in Requirements / Design / Test Strategy / Implementation
  Plan around the existing What/Why. Original "Open Questions"
  block resolved into the Decision Log below.

## Decision Log

- 2026-05-27 · **Worktree as recommendation, not gate.**
  Original backlog scoped worktree mode as the trigger for
  inner-loop auto-PR. Reshaped: inner-loop close-out is
  unconditional once gates pass. Worktree status is checked at
  feature-start (suggestion) and at close-out (teardown offer).
  Rationale: the close-out value is in *automating the manual
  step*, not in *isolation guarantees*. Branch-only checkouts
  benefit just as much; isolation is a separate property the
  user opts into via `project.md`.

- 2026-05-27 · **Body template — light, not codified.**
  Original backlog mandated `Why / Approach / Verification /
  Reading guide / Pairs with`. Reshaped to `Why + Approach`
  required, deeper sections suggested when warranted. Matches
  §5's existing PR rule ("body: _why_ + approach"). Avoids
  imposing new structure on small diffs and avoids drift
  between this feature's rules and the rest of the skill.

- 2026-05-27 · **Backlog cleanup — interactive, not
  metadata-driven.** Considered a `resolves: [...]` frontmatter
  field on feature specs. Rejected: feature names drift, the
  pointer goes stale, and the close-out moment is exactly when
  the human knows which items are resolved. One prompt at
  close-out is cheaper than maintaining the metadata.

- 2026-05-27 · **`project.md` verify — soft, not hard block.**
  Considered refusing close-out until project.md is updated or
  the user explicitly says "no changes needed." Rejected: §6's
  existing posture is low-friction; tightening here would make
  it inconsistent with the rest of the skill. Prompt + accept
  decline.

- 2026-05-27 · **Review tool detection.** Resolution order
  fixed at: `git remote get-url origin` → `## Agent
  Development` → `kdevkit` block → ask once. Public CLIs named
  in the skill body are limited to `gh` (per §7 public-repo
  hygiene). Internal review tooling is referenced abstractly
  via the `project.md` config block.

- 2026-05-27 · **Dry-run preview — dropped.** Original backlog
  proposed printing title + body before submission and
  requiring confirmation. Dropped: the branch is already
  pushed, the PR/CR is editable post-submission, and the
  preview adds a per-iteration prompt that erodes the "no
  per-step prompts" property of the loop.

- 2026-05-27 · **Update vs. create — kept.** `gh pr list
  --head <branch>` (or per-project equivalent) before
  submission. If a review exists, update body; otherwise
  create. Avoids duplicate reviews on re-pushes within the
  same branch.

- 2026-05-27 · **Phase-gating compatibility (§6).** Single
  sentence in §6 makes Push + Review-open one phase, and §9
  close-out a distinct phase. Resolves the apparent contra-
  diction with §6's "do not chain phases automatically" rule
  without changing that rule.
