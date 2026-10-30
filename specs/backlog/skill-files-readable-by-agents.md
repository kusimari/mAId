# Backlog: skill-files-readable-by-agents

## What

Reading mAId's own installed skill files never prompts, in any session or
any agent a session spawns (including background ones), across rebuilds,
for Read-tool and shell reads alike, without the user editing settings by
hand. That covers every file a skill tells the agent to read, not just
`SKILL.md` (kdevkit's `phases/*.md`, `interviews.md`, `tiers/*.md`,
`setup.md`; kyodakit's `SKILL.md`).

## Why

The marketplace at `~/.local/state/maid/marketplace/plugins` is a symlink
into the Nix store (`/nix/store/<hash>-maid/share/maid/marketplace/...`).
Claude Code resolves symlinks before checking permissions, so a read
reports "resolves through a symlink to /nix/store/...-maid/..., which is
outside the allowed working directories". The installed grant (the
profile-side path in `permissions.additionalDirectories`, from
claude-plugin-reads) does not cover the resolved path on this machine.
It gets worse in three cases:

- **Store hash changes.** Every rebuild gives a new hash, so an
  `additionalDirectories` entry naming one hash stops matching the next
  build.
- **Shell reads.** Agents often read modules with `sed -n` or `cat`
  through Bash, which a `Read(//nix/store/*-maid/**)` allow rule does not
  cover.
- **Background agents.** Spawned agents (initiative streams, review and
  briefing dispatches) hit it on every module read, and each read stalls
  until the user answers.

Two stopgaps are in one user's settings by hand (2026-10-13/14): a Read
rule for the symlink path, a Read rule for the store-path pattern, and
`additionalDirectories` entries for the marketplace path and one resolved
store path. The last breaks on the next rebuild.

## Options to weigh

- (a) Install writes a hash-agnostic grant, e.g. `additionalDirectories:
  //nix/store/*-maid/share/maid/marketplace/plugins`, if globs work there;
  otherwise it rewrites the current hash on every install, owned and
  removed per the part rules.
- (b) Install the marketplace as a real copy in a stable location, not a
  symlink, so the path an agent reads is the path a rule names.
- (c) Skills tell agents to read modules only with the Read tool by their
  announced base directory, plus a matching Read rule.

## Open questions

- Check which of these Claude Code honours for symlink-resolved paths,
  and for Bash reads, before choosing (probes in a temp HOME whose HOME
  is itself behind a symlink, as this machine's is).
- Why the claude-plugin-reads probe passed in a temp HOME but the real
  install still prompts.
- Remove the hand stopgaps the fix makes redundant (adopt or report them;
  the user's call).
- codex, kiro: equivalents, next round.
