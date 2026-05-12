---
name: git
description: How to use git on this machine — branches, commits, staging, destructive operations, and PR hygiene.
version: 1.0.0
tags: [coding, refactor, bug-fix, review, release]
---

# git — how I like to use git

## Branches

- Name `type/description` or `feature-<name>`. Types: `feat/`, `fix/`, `chore/`, `docs/`,
  `refactor/`, `test/`.
- One feature per branch. No omnibus branches.
- Don't rename or delete branches that have been pushed without asking.

## Commits

- Conventional Commits: `type(scope): subject`, imperative mood, subject under ~70 chars.
- Body: 1–2 sentences on the _why_, not the _what_. The diff shows what.
- New commits, not amends. Amend only when the user explicitly asks. If a pre-commit hook fails, fix
  and make a NEW commit — amending destroys previous work.

## Staging

- Never `git add -A` / `git add .`. Add specific files by name so you don't sweep up `.env`,
  credentials, or large binaries.
- Warn if files likely to contain secrets are being staged.

## Destructive ops — never without explicit approval

- `push --force`, `push --force-with-lease` to a shared branch
- `reset --hard`, `checkout .`, `restore .`, `clean -f`
- `branch -D` on a branch that has been pushed
- `rebase -i`, `add -i` (interactive ops can't be driven from tools)
- `--no-verify` or any hook skip
- `--no-gpg-sign` or any signing bypass

Flag the action, state the consequences, ask. Don't use destructiveness as a shortcut when stuck —
diagnose root cause.

## PRs

- Title under 70 chars; details go in the body.
- Body is "Summary" (1–3 bullets) + "Test plan" (checklist).
- Never push to main/master directly; open a PR.
- Don't open PRs automatically unless asked.

## Workflow hygiene

- Check `git status`, `git diff`, recent `git log` before committing — follow the existing commit
  style.
- If there's in-flight uncommitted work you didn't expect, stop and ask — it may be the user's WIP.
