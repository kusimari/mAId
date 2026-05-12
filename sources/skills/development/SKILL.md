---
name: development
description: How I want code changes made — scope, what to leave out, verification, reversibility, and planning.
version: 1.0.0
tags: [coding, refactor, bug-fix, review]
---

# development — how I want code changes made

## Shape of the change

- Bug fixes stay bug fixes. No drive-by cleanup, no surrounding refactor. Match scope to the
  request.
- Don't design for hypothetical future requirements. Three similar lines beats a premature
  abstraction.
- No half-finished implementations. If it can't be done fully, stop and ask.
- No backwards-compat shims or feature flags for internal code. If you can just change the code, do.

## What to leave out

- Error handling for scenarios that can't happen. Trust internal code and framework guarantees. Only
  validate at system boundaries (user input, external APIs).
- Comments that explain WHAT — well-named identifiers already do that. Only write a comment when the
  WHY is non-obvious: hidden constraint, subtle invariant, workaround, surprising behavior.
- References to the current task in code comments ("used by X", "added for Y flow", "fix for #123").
  That's PR-description stuff; it rots as code evolves.
- Defensive renaming of unused variables to `_var`. If it's unused, delete it.

## Verification

- For UI / frontend changes: run the dev server, use the feature in a browser. Test the golden path
  AND edge cases. Monitor for regressions. Type-check + tests verify _code correctness_, not
  _feature correctness_. If you can't test the UI, say so — don't claim success.
- For library / backend code: unit tests cover the change; the full suite still passes.
- Before declaring done: read back the actual diff, not what you intended to do.

## Reversibility

- Local, reversible actions (editing files, running tests): proceed.
- Hard-to-reverse or shared-state actions (pushing, deleting files, dropping tables, killing
  processes, modifying CI, sending messages): state what you're about to do and ask.
- If you hit an obstacle, don't use a destructive shortcut (e.g. `--no-verify`, `reset --hard`) to
  make it go away. Investigate root cause.

## Planning

- Non-trivial tasks: plan first, align with the user, then execute.
- Trivial tasks (typos, single-line renames, one-file edits): just do them.
- Don't write planning docs unless the user asks for one.
