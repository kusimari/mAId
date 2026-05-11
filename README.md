# mAId

Tool-agnostic source of truth for agentic resources — skills,
agents, commands, MCPs — compiled to whatever AI tool happens to
be in use (Claude Code, Kiro, Gemini CLI, and future tools).

## Day-1 install

```
cd ~/workplace/ai-workspace/mAId
./install
```

The install script:

1. Ensures `deno` is on PATH (via `nix profile install nixpkgs#deno`
   if missing).
2. Symlinks `scripts/maid` into `~/.local/bin/maid`.
3. Runs `maid validate && maid deploy` to create directory symlinks
   from `$HOME` into this checkout.

## Day-2 flow

Editing any `SKILL.md` under `sources/skills/<name>/` is live in
the next AI session — the symlinks already point at this tree.
Adding a brand-new skill: just drop a new
`sources/skills/<name>/SKILL.md` and it's visible too (the
directory symlink transparently exposes the new file).

Run `./install` again after `git pull` to pick up new registry
entries or maid CLI changes.

## `maid` subcommands

```
maid validate       Walk sources/ and validate frontmatter.
maid deploy         Create/refresh $HOME-facing symlinks.
  --dry-run         Plan without making changes.
  --force           Replace symlinks that point elsewhere.
maid status         Report each managed symlink's state.
maid --help         Show usage.
```

## Layout

```
mAId/
├── install                  # day-1 entry point (invoked by env/layer-5/run)
├── scripts/maid             # bash wrapper → deno run maid/main.ts
├── maid/                    # Deno TypeScript CLI
│   ├── main.ts
│   ├── schema.ts            # frontmatter parse + validate
│   ├── sources.ts           # walk sources/
│   ├── registry.ts          # $HOME ↔ source path mapping
│   └── deploy.ts            # symlink manager
├── sources/
│   ├── skills/<name>/SKILL.md
│   ├── agents/<name>.md
│   └── commands/<name>.md
├── CLAUDE.md                # user-memory for Claude Code
├── KIRO.md                  # steering for Kiro
└── tests/
    ├── schema_test.ts       # deno test
    ├── deploy_test.ts
    └── functional/          # real-tool round-trip smokes
        ├── run
        └── skills/*.smoke
```

## Testing

```
deno task test                    # unit + integration (schema + deploy)
deno task check                   # typecheck
./tests/functional/run            # structural + real tool invocations
./tests/functional/run --no-tools # structural only (fast)
```
