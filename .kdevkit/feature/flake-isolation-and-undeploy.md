# Feature: flake-isolation-and-undeploy

## Git Setup

- Branch: `feat/flake-isolation-and-undeploy` (off `feature-ai`)
- Base: current tip of `feature-ai`

## Feature Brief

Isolate mAId's tooling (deno) in a repo-local Nix flake loaded via direnv so installing mAId never
leaks `deno` into the user's global `~/.nix-profile`. Add a `maid undeploy` subcommand +
`./install
--uninstall` flag so the repo offers a clean-slate reversal of everything it drops on the
host. Sharpen the three default-skill smoke fixtures so they assert the skill was _loaded_, not just
that the answer happened to look right.

## Requirements

- Repo-local `flake.nix` exposes a devShell with deno; entering the repo with direnv brings deno
  onto PATH.
- `./install` works on a cold machine with no prior deno and no direnv (re-execs via
  `nix develop --command`).
- `./install` must not run `nix profile install` anywhere — no global profile mutation.
- `maid undeploy` removes only symlinks that point into the checkout; foreign symlinks and
  hand-written files at a registry destination are left intact.
- `maid undeploy` is idempotent: a second run over a clean host exits 0, logging "not deployed" per
  entry.
- `./install --uninstall` dispatches to `maid undeploy` and short-circuits the bootstrap/deploy
  path.
- Each of `tests/functional/skills/{development,git,writing-style}.smoke` asserts the skill emitted
  a `[<skill>] applies` preamble.
- Zero changes to `env` or `Gorantls-env`.
- Zero Node/mise work (slack-mcp is handled in a separate session).

## Design

### D-M1 · Flake + direnv

`flake.nix` at repo root, `flake-utils.lib.eachDefaultSystem`, minimal
`pkgs.mkShell { buildInputs = [ pkgs.deno ]; }`. `.envrc` contains `use flake`. `.gitignore` picks
up `result*` (`.direnv/` is already listed). Lockfile committed.

### D-M2 · `install` re-execs via `nix develop`

Replace `ensure_deno()` in `install` with a guard:

```bash
if [ -z "${IN_NIX_SHELL:-}" ] && ! command -v deno >/dev/null 2>&1; then
  if command -v nix >/dev/null 2>&1; then
    exec nix develop --command "$0" "$@"
  fi
  die "Need nix (https://nixos.org/download) or deno on PATH."
fi
```

Drop the `~/.post-nix-rc.d/maid.sh` write / the old `nix profile install` path / the "add
`~/.nix-profile/bin` to PATH" hint.

### D-M3 · `maid undeploy` subcommand

Iterates `maid/registry.ts` REGISTRY. Per entry at `homePath`:

| Destination state                                | Action                          |
| ------------------------------------------------ | ------------------------------- |
| `Deno.lstat` throws (missing)                    | log "not deployed"              |
| symlink, readLink target starts with `checkout/` | remove, log "removed"           |
| symlink, target elsewhere                        | log "skipped (foreign symlink)" |
| non-symlink (file / dir)                         | log "skipped (not managed)"     |

Flags: `--dry-run` (print planned actions only), `--force` (unlink whatever is at the destination —
last resort). Wired into `main.ts` subcommand switch with a short `USAGE` update.

### D-M4 · `./install --uninstall`

Top-level flag in the bash `install` script; skips bootstrap and deploy, calls
`scripts/maid undeploy "${FORWARD[@]}"` via the already-resolved checkout. If `scripts/maid` is
missing somehow, log + exit 0.

### D-M5 · Sharper smoke fixtures

Each `.smoke` file instructs the model to emit `[<skill>] applies` as the first thing in its
response. `expect_substr` asserts on the preamble. Existing SKILL.md bodies unchanged.

## Test Strategy

### Isolation

- Fresh machine, no deno, no direnv: `./install` exits 0 via the nix-develop re-exec.
- `nix profile list` shows no mAId-sourced deno entry after install.
- With direnv allowed: bare shell has `deno`; `./install` short-circuits re-exec (IN_NIX_SHELL set).

### Deploy / skills

- `maid status` reports all 5 registry entries as managed after deploy.
- Fresh `claude` session: dev / git / writing-style prompts trigger the expected skill;
  `tests/functional/run` passes and each output contains the `[<skill>] applies` preamble.
- Fresh Kiro session: `~/.kiro/steering/KIRO.md` resolves.

### Undeploy

- Normal: all 5 removed.
- Foreign: replace `~/.claude/CLAUDE.md` with a hand-written file → `maid undeploy` leaves it;
  removes the other 4.
- Idempotent: second run logs "not deployed" per entry, exits 0.

## Implementation Plan

1. `flake.nix` + `flake.lock` + `.envrc`; update `.gitignore`.
2. Rewrite `install` bootstrap; drop profile-install code, drop post-nix-rc drop-in (the flake +
   direnv replaces that path).
3. `maid/deploy.ts` — add `undeploy()` + result types; reuse `pathExists` helper.
4. `maid/main.ts` — register `undeploy` subcommand, update USAGE.
5. `install` — add `--uninstall` flag that runs the new subcommand.
6. Rewrite the three `.smoke` files.
7. Update `README.md`: direnv / nix-develop fallback / undeploy.
8. Run `deno task check && deno task test && ./tests/functional/run
   --no-tools` before push.

## Session Log

<!-- Newest at top -->

- 2026-05-12 · **Feature closed.** Follow-up work continues in
  `.kdevkit/feature/scaffolding-first-skills-set.md` — Deno-native
  dev loop + CLAUDE/KIRO relocation + scripts/maid deletion.
- 2026-05-12 · `flake.nix` + `.envrc` + `flake.lock` landed. devShell exposes deno 2.7.14. Verified
  via `nix develop --command deno --version`.
- 2026-05-12 · `install` rewritten to re-exec under `nix develop` when deno is missing and nix is on
  PATH; dropped the `~/.post-nix-rc.d/maid.sh` drop-in (direnv replaces that path). `--uninstall`
  flag added.
- 2026-05-12 · `maid undeploy` subcommand shipped with 7 new unit tests (clean / removes /
  idempotent / foreign / force / real-file preserved / dry-run). Full test suite: 22/22 passing.
- 2026-05-12 · `.smoke` fixtures rewritten to assert `[<skill>] applies` preamble. Structural smoke
  (`--no-tools`) passes and now also confirms `kdevkit` skill is visible.
- 2026-05-12 · `maid undeploy --dry-run` on the live host correctly identifies all 5 managed
  symlinks; not executed destructively (would wipe current setup).
- 2026-05-12 · feature file seeded from the implementation plan.

## Decision Log

<!-- Newest at top -->

- 2026-05-12 · Flake lives at the **repo root**, not a `Nix/` subdir. Keeps a future package
  conversion cheap (`url = "github:…/mAId"` instead of `?dir=Nix`).
- 2026-05-12 · `undeploy` refuses to touch non-symlinks at managed destinations. Protects
  user-authored replacements (e.g., someone who copied `CLAUDE.md` out of the checkout and edited it
  in place). `--force` is the escape hatch.
- 2026-05-12 · No `nix profile install` fallback kept. Either nix devShell works, or the user
  installs deno themselves. Clean isolation wins over convenience here.

## Future work (out of scope for this feature)

- Convert mAId into a package-output flake (`packages.<system>.default`) so `~/env` can consume it
  as an input. Gaps: wrapping `scripts/maid` as `pkgs.writeShellScriptBin`, adding a github remote
  or `path:` input convention to `~/env`, re-expressing the 5 registry entries as home-manager
  `home.file` declarations (the real cost).
