# Backlog: test-runner-sandbox-asymmetry

## What

Make `resources/tests/run` apply a consistent, least-privilege sandbox
to every coding agent on the no-workdir (read-only, `assert_response`)
path. Today that path invokes the agents asymmetrically:

- `claude` runs with `--dangerously-skip-permissions`
- `codex` runs with `--sandbox read-only`
- `kiro` runs with `--trust-tools=` (empty trust list)

So a fixture that is meant to be read-only (a judge/substr fixture with
no `--- setup ---` workdir) can still let claude take arbitrary
filesystem actions, while codex is genuinely confined. The fix is to
give claude an equivalent read-only confinement on this path (or drop
the skip-permissions flag and supply a read-only allowlist), so "no
workdir" means the same guarantee for all three agents.

## Why

Surfaced twice. The `writing-style-behavioral-verification` code
review (91/100) and its Closure Review Gate (88/100) both flagged it:
the learning-loop fixture must guard against mutating the installed,
symlinked, version-controlled `SKILL.md`, because the skill's own
Learning-loop contract instructs editing that very file. The fixture
guards by *prompt text only* ("do NOT call any file-editing tool") —
which is the strongest a fixture author can do, but it is not
enforcement. If claude ignores the prompt on the unsandboxed path, a
test run mutates repo source. Closing the sandbox asymmetry moves this
from a prompt-level plea to a runner-level guarantee, and is a
`resources/tests/run` property (not fixable in any single fixture).

## Open questions

- Does the installed `claude` CLI expose a read-only sandbox flag
  equivalent to `codex --sandbox read-only`? If not, what's the least-
  privilege alternative (a scoped allowlist, a temp `$HOME`)?
- Should the read-only path drop `--dangerously-skip-permissions`
  entirely, or is that flag load-bearing for non-interactive runs
  (i.e. would removing it reintroduce interactive permission prompts
  that break the non-interactive harness)?
- Is there a matching asymmetry on the behavioral (`--- setup ---`,
  seeded-workdir) path, or is that one already uniformly confined to
  the scratch repo?
