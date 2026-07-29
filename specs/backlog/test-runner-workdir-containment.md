---
name: test-runner-workdir-containment
description: Behavioral skill tests can write into the mAId checkout instead of their scratch workdir. Observed during a tri-tool sweep — three notes-git-commit runs committed insight files into the repo itself. The runner must confine an agent to the seeded workdir rather than trusting the prompt's relative path.
metadata:
  type: backlog
---

# Test runner — confine behavioral tests to their scratch workdir

## What

`assert_behavioral` seeds a fresh `mktemp -d`, runs the agent with that
directory as its cwd, and asserts against it. That is containment by
*convention*, not enforcement: the agent still has ambient authority
over the filesystem, and if it resolves a path differently than the
fixture intended, its writes land outside the scratch dir.

Make the confinement real. Options to weigh:

- Sandbox the write surface per agent the way the read-only path
  already differs (codex takes `--sandbox workspace-write`; claude runs
  with `--dangerously-skip-permissions`, which is the outlier — see
  [[test-runner-sandbox-asymmetry]], the same root cause on the
  read-only path).
- Run the agent with `$HOME` pointed at a throwaway dir so
  `$HOME`-relative resolution can't reach the real checkout, being
  careful that the installed skill symlinks still resolve.
- Post-run tripwire: snapshot `git status` in the checkout before and
  after each behavioral test and fail loudly on any change, so a leak
  is caught by the suite instead of by a human noticing stray commits.

## Why

Observed, not hypothetical. During the tri-tool sweep for
`fixtures-discovery-vs-content-split`, three `notes-git-commit` runs
(of six) wrote `insights/wadler-builds-an-immutable-document-tree*.md`
into the mAId checkout **and committed them** — three stray commits
landed on the feature branch, interleaved with real work. Two
`kdevkit-agents-md` runs likewise left an untracked `AGENTS.md` and
`CLAUDE.md` in the repo root.

The fixture's task says `add note in ./ for: …` then `close notes in
./`, and the `close notes` verb runs `git commit`. In the leaking runs
the relative path resolved against the checkout rather than the seeded
workdir, so a *passing* test silently mutated version-controlled
source. That is the dangerous shape: the assert ran in the scratch dir
where the work also happened, so the suite reported PASS while the
repo was being modified. Nothing failed; the damage was found by
reading `git log`.

Cleanup was manual (drop the stray commits, delete the files). The
suite should make this impossible instead.

## Open questions

- Does `claude --print` accept a scoped write sandbox equivalent to
  codex's `workspace-write`? If not, is a throwaway `$HOME` plus a
  pre/post `git status` tripwire the best available guard?
- Do the installed skill symlinks still resolve under a redirected
  `$HOME`? They point back into the checkout, so a fake `$HOME` needs
  the skills tree linked in or the agent won't find the skill at all —
  which would turn every behavioral test red for the wrong reason.
- Should the tripwire also cover `$HOME` (not just the checkout)? A
  test that writes into the user's real `~/notes` would be just as
  wrong and is not currently detected.
- Is the fixture's `in ./` phrasing worth changing independently? An
  absolute path handed to the agent would be unambiguous, but it makes
  the prompt less like something a user would actually type, which is
  the point of the implicit/integration kind.
