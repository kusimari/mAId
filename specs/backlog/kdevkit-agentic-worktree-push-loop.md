# Backlog: kdevkit-agentic-worktree-push-loop

## What

Extend the `kdevkit` skill (in `sources/skills/kdevkit/SKILL.md`)
so that when an agent is doing implementation work in a separate
git worktree, the existing Quality → Test → Push loop (§8) closes
the loop end-to-end:

1. Pushes the feature branch to the remote when both gates pass.
2. Opens the appropriate review request — a GitHub PR for public
   repos, a CRUX CR for amazon-internal repos — with a
   structured body that includes a **reading guide** so a human
   reviewer knows where to start.
3. Returns the review URL to the caller.

The skill currently stops at `git push -u origin <feature-branch>`
and leaves "opening a PR is a human decision." That handoff makes
sense when the agent is working in the user's main checkout, but
when the agent is running in a dedicated worktree (no other
context, no other diffs to surprise the human), the natural end
state is "branch pushed + review opened + URL surfaced."

The worktree assumption matters: it's the signal that the agent
has full custody of the diff, no foreign edits are mixed in, and
the human's role is to review rather than to triage what got
included.

## Why

Three wins:

1. **Closes the loop.** Today, the loop's last step is a manual
   navigation to GitHub or `cr`. In a worktree-isolated session,
   the human's next action is always "open the review" — making
   that a separate manual step adds friction with no judgment
   value.
2. **Forces a reading guide.** Reviewers waste time
   reverse-engineering "what should I look at first?" — a
   structured body ("why / approach / verification / reading
   guide" with file order and what to compare against) shrinks
   the review's cold-read cost. Worth doing every time, but
   especially when the diff is agent-authored and the human
   wasn't watching the keystrokes.
3. **Same shape across repos.** Public (GitHub `gh pr create`)
   and Amazon-internal (`cr` CLI) review tools have different
   invocations but the body shape is identical. Codifying the
   body template in the skill keeps PRs and CRs symmetric.

This session was a worked example: two env PRs (#27, #28) and one
Gorantls-env CR opened by the agent, all with the same
`why / approach / verification / reading guide` body shape.

## Open questions

- **Worktree detection.** The skill should only auto-push +
  open-review when it knows it's in an agent worktree. Detection
  options:
  - `git rev-parse --show-toplevel` differs from
    `git rev-parse --git-common-dir`'s parent (`.git/worktrees/...`
    suffix path).
  - An explicit env var (`KDEVKIT_AGENT_WORKTREE=1`) set by the
    coding agent's launcher.
  - A marker file the worktree-spawning tool drops at the root.

  Recommendation: combine — autodetect via `git worktree list`,
  but also honor an opt-in env var so the human can flip it on
  in any checkout when they want the closure-step automation.

- **Review-tool selection.** Today's heuristic: `git remote -v`
  pointing at `github.com` → `gh pr create`; pointing at
  `git.amazon.com` or similar → `cr`. Where else does the skill
  need a branch? Codeberg, GitLab, GitFarm? The §7 public-repo
  hygiene rules already enumerate hosts; reuse that list.

- **Body template.** The shared shape used in this session
  worked: **Why** (paragraph, not bullets), **Approach** (bullet
  list of the actual changes), **Verification** (commands run +
  results), **Reading guide** (numbered file order with "compare
  against X" hints), **Pairs with** (cross-repo links when
  applicable). Codify this in the skill body so every agent-run
  produces the same shape.

- **Gates before push.** The Push Gate as-written already
  requires both Quality and Test gates pass. With auto-PR/CR,
  add: _"refuse to open a review if either gate failed; surface
  what failed and require explicit override."_ Same intent, more
  load-bearing now that the human isn't manually navigating.

- **Public-repo hygiene last-mile check.** §7 already mandates a
  pre-push grep for internal markers. With auto-open-review, the
  PR body itself is also human-visible content — extend the grep
  to the prepared body before submission.

- **Phase gating compatibility.** §6's phase gating says "do not
  chain phases automatically." Auto-push + auto-PR is a *single*
  phase (Push), not a chained one — the user already
  approved-by-action by getting both gates green. But the skill
  should make this explicit so a reader doesn't see it as
  contradicting §6.

- **Dry-run / preview.** Before opening the review, surface the
  prepared title + body to the user (briefly — not full body)
  and let them abort. Cheaper than discovering a typo'd title
  after the URL is live.

- **Update vs. create.** When the branch already has an open PR
  or CR, the loop should *update* (`gh pr edit` body, or
  `cr -r CR-XXX`) instead of creating a duplicate. Detecting an
  existing review on push is straightforward via `gh pr list
  --head <branch>` / `cr` lookup.

- **Close-out on approval — squash merge + branch cleanup.**
  When a feature reaches approval (PR approved + checks green;
  CR approved + reviewers signed off), the loop should also
  drive the close-out without per-step instruction:

  1. **Pick the merge mode.** Default to **squash merge** for
     feature branches — keeps one logical commit per feature on
     the main branch. Evaluate the alternative once before
     defaulting:
     - If the branch has a single commit already, plain merge
       and squash produce the same result; either is fine.
     - If the repo's main branch is itself a non-linear history
       (merge commits as the norm), squash still works but
       surface the choice to the user before going non-default.
     - For repos that explicitly require fast-forward only on
       main (config or convention), use the local
       `git merge --squash` + commit + push pattern (what the
       Gorantls-env CR flow used in this session) since the
       review tool can't be the merger.
  2. **Confirm branch deletion.** After merge, delete the
     feature branch **both local and remote**. For
     `gh pr merge`, pass `--delete-branch`. For the local
     `git merge --squash` path, do `git branch -D <feat>` plus
     `git push origin --delete <feat>` and `git fetch --prune`.
     Surface the deletion (one line) so the user sees it landed
     — but do not pause to ask permission; deletion is the
     declared default for merged feature branches.
  3. **Worktree teardown.** If the agent worktree itself was
     dedicated to this feature, surface the path and offer to
     `git worktree remove` it. Don't auto-remove without
     surfacing — the worktree may have generated artifacts
     (logs, temp files) the user wants to inspect first.

  This closes the full agentic loop: spec → branch → push →
  PR/CR → approval → merge → cleanup, with no manual prompts in
  the steady-state path. The user's role stays at "approve the
  review" and "review the close-out summary."

## Trigger to promote

Promote when one of these is true:

- Two consecutive sessions where the agent stops cleanly at
  "branch pushed" and the user follows with the same manual
  open-review step. That's the friction signal.
- A pattern emerges where review bodies the agent writes look
  near-identical (same Why/Approach/Verification/Reading-guide
  shape, manually re-derived each time). That's the
  template-codification signal.
- A repo gets enough volume that drift between PRs (different
  body shapes, different reading-guide quality) becomes a
  reviewer complaint.
