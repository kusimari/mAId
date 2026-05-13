# Feature (deferred): kiro-side-functional-smoke

> **Status: deferred.** Pick this up in a dedicated feature-dev
> session by promoting to `.kdevkit/feature/`.

## Feature Brief

Today `tests/functional/run`'s functional path uses `claude
--print` against three `.smoke` fixtures. There's no equivalent
coverage for `kiro-cli`. Kiro reads from `~/.kiro/steering/KIRO.md`
(one file), which is a different surface shape than Claude's
`~/.claude/skills/<name>/SKILL.md` (many files). Before writing
fixtures, the surface needs to be understood.

## Requirements (draft)

1. Structural check: `~/.kiro/steering/KIRO.md` resolves to the
   expected source — this already lands in the updated structural
   smoke.
2. Functional check: a `kiro-cli` invocation with a prompt that
   should pick up the steering doc produces output matching an
   assertion substring (TBD based on KIRO.md body).
3. Parity question: if Kiro can surface per-skill steering (e.g.,
   `~/.kiro/steering/<name>.md` for each mAId skill), does the
   registry need new entries to unlock that?

## Design (draft)

Pre-work:

- Read Kiro's steering discovery surface. What files does it pick
  up? Is there a per-file metadata / tagging model like Claude's
  skill frontmatter?
- Decide whether mAId's registry grows matching per-skill targets
  under `~/.kiro/steering/<skill>.md`, or whether KIRO.md is and
  should remain the only Kiro-side entry.

Once the surface is understood:

- Add one or more `kiro-cli`-based assertions to
  `tests/functional/run` mirroring the `claude --print` flow.
- If per-skill steering is desired: extend `maid/registry.ts` and
  `tests/deploy_test.ts`.

## Test Strategy (draft)

- Structural path: already passes (the current PR lands that).
- Functional path: mirror `assert_claude_response` with
  `assert_kiro_response`. `kiro-cli` must be on PATH; skip
  gracefully otherwise.

## Implementation Plan

_Deferred — to be picked up by a dedicated feature-dev session.
First step: a discovery pass on Kiro's steering model._

## Session Log

<!-- empty; populated when the feature is picked up -->

## Decision Log

<!-- empty; populated when the feature is picked up -->
