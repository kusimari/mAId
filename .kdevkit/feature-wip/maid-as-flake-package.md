# Feature (deferred): maid-as-flake-package

> **Status: deferred.** Pick this up in a dedicated feature-dev
> session by promoting to `.kdevkit/feature/`.

## Feature Brief

mAId today ships a **devShell flake** — direnv users get `deno`
on PATH inside the repo; `deno install` writes a shim to
`~/.local/bin/maid`. This feature proposes converting mAId into a
**package-output flake** so `~/env` can consume it as an input
and make `maid` available system-wide via home-manager.

## Requirements (draft)

1. `mAId/flake.nix` exposes `packages.<system>.default` (and
   probably also `packages.<system>.maid`) in addition to the
   current `devShells.<system>.default`.
2. The package wraps `maid/main.ts` + a pinned `deno` runtime so
   `nix run github:<org>/mAId` invokes `maid` directly.
3. `~/env` can add mAId as a flake input
   (`inputs.mAId.url = "github:<org>/mAId"`) and surface it as a
   Tier 1 or Tier 2 package in `home/home.nix`.
4. Existing direnv + devShell flow still works unchanged.
5. `./install` still works unchanged — this feature adds a new
   consumption path; it doesn't remove the per-repo one.

## Design (draft)

### Gap 1 — the package wrapper

```nix
packages.default = pkgs.stdenv.mkDerivation {
  name = "maid";
  src = ./.;
  buildInputs = [ pkgs.deno ];
  installPhase = ''
    mkdir -p $out/bin
    cat > $out/bin/maid <<EOF
    #!/bin/sh
    exec ${pkgs.deno}/bin/deno run --allow-read --allow-write --allow-env \\
      $out/share/maid/maid/main.ts "\$@"
    EOF
    chmod +x $out/bin/maid
    mkdir -p $out/share/maid
    cp -r maid sources registry.ts $out/share/maid/
  '';
};
```

Probably cleaner as `pkgs.writeShellApplication` or
`pkgs.deno2nix`-style builder if that's around by the time this
ships. About 20 lines.

### Gap 2 — source reachability

Surveyed previously: `~/env`'s inputs are all github refs. Two
paths:

- Push mAId to a github remote; `~/env` references
  `github:<org>/mAId`.
- Use a `path:` input in `~/env`. `~/env` doesn't use any today,
  and adding one makes the flake config less portable.

Preferred: github remote (consistent with existing pattern).

### Gap 3 — deploy semantics

`maid deploy` is **imperative** (creates symlinks on demand).
`~/env` is **declarative** (home-manager). Two options:

- **(a) Imperative deploy, post-activation.** The package
  install only lands `maid` on PATH; the user (or a
  home-manager activation hook) still runs `maid deploy` to
  create the symlinks. Simplest.
- **(b) Declarative deploy.** Re-express the 5 registry entries
  as `home.file` declarations inside the mAId flake's
  home-manager module. Gets the full "nix-managed" story — but
  means rewriting `deploy.ts`'s logic (symlink management,
  foreign-symlink detection) as Nix. Non-trivial.

Ship (a) first; (b) as a later follow-up if home-manager users
find imperative deploy awkward.

## Test Strategy (draft)

- `nix build .#maid` succeeds; `result/bin/maid --help` runs.
- `nix run github:<org>/mAId -- status` works without a local
  checkout.
- From `~/env`: add mAId as input + expose the package;
  `home-manager switch`; `which maid` → nix-store path; `maid
  deploy` works.

## Implementation Plan

_Deferred — to be picked up by a dedicated feature-dev session._

## Session Log

<!-- empty; populated when the feature is picked up -->

## Decision Log

<!-- empty; populated when the feature is picked up -->
