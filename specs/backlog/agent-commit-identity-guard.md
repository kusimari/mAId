# Backlog: agent-commit-identity-guard

> **Status: deferred.** Pick this up in a dedicated feature-dev
> session by promoting to `specs/feature/`.

## What

Before any agent-driven commit (in mAId, Gorantls-agents, or
anywhere the kdevkit skill is in scope), verify
`git config user.email` matches an expected identity for the
repo's remote host. Refuse the commit if it doesn't, with a
clear message about how to fix.

The kdevkit SKILL.md already owns agent commit conventions
(Conventional Commits, no `Co-Authored-By`, no amends, repo in
working state after each commit). It has no runtime identity
check today. This adds one.

## Why

The user's machines pin git identity at clone-time in the
env-workplace bootstrap layers — Layer-1 sets a global default,
Layer-2 pins env, Layer-5a pins mAId, Layer-5b pins
Gorantls-agents and Gorantls-store. For any repo the bootstrap
touches, fallthrough to git's `<unix-user>@<hostname>` default
can't fire.

The residual leak surface is agent-driven:

1. An agent runs `git clone <new-public-repo>` ad-hoc into
   `~/env-workplace/<new>/` outside the bootstrap inline
   blocks. The new checkout has no `--local` identity, inherits
   the global — which on a corp dev-desktop is the corp
   identity. First commit leaks corp email into public history.
2. An agent runs `git init` for a brand-new repo. Same
   fallthrough.
3. An agent commits inside a bootstrap-pinned repo on a
   fresh-cloned machine where bootstrap hasn't yet run.

Original intent and design exploration (`includeIf` directive in
nix, validating pre-commit hook) is recorded in env-side spec
`env/.kdevkit/feature/wip/global-commit-identity-per-repo.md`
git history. The nix-side approach was rejected: `env/home/`
is location-blind by convention (`user-host.nix` derives
username/hostname from shell), so importing a
`gitdir:~/env-workplace/**` rule into the nix layer would leak
build-script path-shape into a module that doesn't know it.
The fix belongs here, in agent tooling.

## Open questions

- **Where the rule lives.** Three shapes:
  - Extend `mAId/sources/skills/kdevkit/SKILL.md` (Commits
    section) only — pure prose; agent reads
    `git config user.email` and compares before committing. No
    new code.
  - Same prose rule plus a `maid` subcommand
    (`maid git check-identity`) that does the check (reads
    remote, looks up expected identity, exits non-zero on
    mismatch). Agents call it as the first step of the Push
    gate. Reusable across mAId and Gorantls-agents.
  - Spin out a dedicated `git` skill at
    `mAId/sources/skills/git/SKILL.md` — pull git rules out of
    kdevkit into their own home; identity guard goes there.
- **How expected identities are specified** without leaking
  corp values into public mAId. Three shapes:
  - **Read-from-local-config** — guard reads
    `git config --global` and `--local`, refuses if local is
    unset on a remote whose host has a known expected identity.
    No map in mAId. Simplest.
  - **Private-location host map** — small map
    (`github.com → public`, `code.amazon.com → corp`, etc.)
    lives in Gorantls-env or Gorantls-agents (private). Guard
    reads via known path. Adds a config-loading dependency.
  - **Block-list / leak-signature detector** — guard refuses
    any commit where `user.email` ends with `@<hostname-FQDN>`
    (the leak signature). Doesn't need a map; doesn't catch
    cross-leaks (corp identity on public repo).
- **Cross-repo scope.** Both mAId and Gorantls-agents load the
  kdevkit skill. Should the same guard cover both, or does
  Gorantls-agents need a private extension?
- **Failure UX.** Refusing a commit mid-session blocks the
  agent's working state. Agree the recovery dance —
  `git config --local user.email <expected>` then retry — and
  bake it into the refusal message.

## Trigger to promote

Promote when one of these is true:

- An agent-authored commit goes out with the wrong identity
  (corp on public, or hostname-fallback anywhere) and the leak
  is observed in commit history.
- A new public repo is created ad-hoc by an agent and the
  first commit's email leaks the dev-desktop hostname.
- The kdevkit skill is updated for unrelated reasons and
  identity guidance can land in the same revision without
  scope creep.

## Out of scope

- Rewriting historical commits to scrub past leaks. Noisy and
  not worth it.
- Any nix-side change. Considered and rejected — see Why.
- Auditing or replacing the existing `ensure_git_identity`
  shell helpers in Gorantls-env. Those continue to handle
  script-driven clones; this guard handles agent-driven ones.
