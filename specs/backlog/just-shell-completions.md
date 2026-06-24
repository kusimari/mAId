---
name: just-shell-completions
description: Verb + flag completions for `just` inside the direnv flake shell — like git's. Mechanism is researched and proven; parked because the shell-side ceremony isn't worth it yet.
metadata:
  type: backlog
---

# `just` shell completions inside the flake shell

## What

When working in this repo (inside the `use flake` direnv shell),
Tab-completing `just <TAB>` should offer the recipes/verbs
(`test`, `check`, `ci`, `fmt`, submodule recipes like
`kaimux::build`, `resources::install`, each with its doc-comment)
and `just --<TAB>` should offer flags — the way `git` completes.

Scope is deliberately repo-local: completion only needs to work
inside this directory's shell, not globally. `just` isn't on
`PATH` outside the flake anyway.

## Why parked

The mechanism is fully understood and verified (see below), but
it can't be made *purely* flake-contained — direnv's design
forces exactly one generic, shell-side line. That line is small
and reusable across all flake projects, but setting it up plus a
rebind/rewind hook is more ceremony than the payoff justifies
right now. Revisit when either the completion friction starts to
bite or we're already touching shell-rc setup for another reason.

## What's already true (no work needed)

- `just` 1.50 (from the flake) ships completions out of the box.
  Its nix package already includes
  `…/just-1.50.0/share/zsh/site-functions/_just` — a clap
  dynamic-completer stub (`source <(JUST_COMPLETE=zsh just)`)
  that produces live recipe + flag candidates against *this*
  repo. Verified: recipes carry their doc-comments, submodule
  recipes (`kaimux::build`, `resources::install`, …) appear,
  and `--<TAB>` lists flags with descriptions.

So nothing needs generating; the completion file exists in the
flake closure. The only gap is getting zsh to bind it.

## Why it can't be pure-flake (the core finding)

How zsh completion works:
- `compinit` scans every dir in `fpath` **once, when it runs**
  (at shell startup), and builds the command→function map.
- `fpath` itself is seeded from the `FPATH` env var, but is a
  live mutable array.

The wall:
- direnv does **not** spawn a subshell — it hooks the existing
  shell's `precmd` and `eval`s `direnv export zsh`, which carries
  **only environment-variable mutations**. Shell functions and
  `compdef` map entries are not env vars, so they never cross.
  (Demonstrated: a function defined in `.envrc` did not survive
  `direnv export`; an exported var did.)
- A flake `shellHook` runs in a throwaway **bash** subprocess
  (`nix print-dev-env`), in the wrong language (`compinit`/
  `compdef`/`autoload` are zsh builtins), at the wrong time, and
  only its env-var residue is diffed back. So the `shellHook`
  cannot do the rebind itself.
- Even when direnv carries an extended `FPATH`, the binding does
  **not** appear: `compinit` already ran at startup, before the
  `cd`, so the newly-added dir is never rescanned. Verified:
  - `FPATH` carried, no rescan → `_comps[just] = NONE`
  - `FPATH` + `autoload -Uz _just; compdef _just just` (or a
    `compinit` rerun) → `_comps[just] = _just` ✓

This is exactly direnv issue #443 ("load completions from
`.envrc`") — open since 2019, never implemented. There is no
built-in direnv mechanism.

## The proven solution (when we pick this up)

Split across the two halves direnv forces:

1. **In repo — `flake.nix` `shellHook`:**
   - prepend `just`'s `…/share/zsh/site-functions` to `FPATH`
     (derive from the just store path);
   - export a marker var, e.g. `DIRENV_ZSH_COMPLETIONS=<that dir>`.
   A `shellHook` that `export`s a var *does* survive direnv
   (`nix print-dev-env` emits `eval "${shellHook}"`). Verified.

2. **Shell-side, one-time — `~/.post-nix-rc`** (the writable rc
   that the read-only nix-managed `~/.zshrc` sources): a generic
   `precmd` hook, marker-driven, that binds on entry and unbinds
   on exit. Sketch:

   ```zsh
   typeset -gA _dz_bound
   _dz_sync_completions() {
     emulate -L zsh
     typeset -A want
     if [[ -n $DIRENV_ZSH_COMPLETIONS ]]; then
       local dir f cmd
       for dir in ${(s.:.)DIRENV_ZSH_COMPLETIONS}; do
         for f in $dir/_*(N); do
           cmd=${${f:t}#_}; want[$cmd]=1
           [[ -n ${_dz_bound[$cmd]} ]] && continue
           fpath=($dir $fpath); autoload -Uz "_$cmd"
           compdef "_$cmd" "$cmd" 2>/dev/null; _dz_bound[$cmd]=1
         done
       done
     fi
     local cmd                                  # exit: marker gone → unbind
     for cmd in ${(k)_dz_bound}; do
       [[ -n ${want[$cmd]} ]] && continue
       unfunction "_$cmd" 2>/dev/null
       unset "_comps[$cmd]" 2>/dev/null; unset "_dz_bound[$cmd]"
     done
   }
   autoload -Uz add-zsh-hook
   add-zsh-hook precmd _dz_sync_completions
   ```

   Rewind is automatic for the env half: direnv reverts `FPATH`
   and the marker on exit. The `compdef` binding is shell state,
   so the hook's exit loop unbinds what it bound (idempotent,
   handles multiple flakes). Ordering is fine: direnv prepends
   its own `precmd`, so the marker is fresh before this hook
   reads it.

   The unbind loop is cosmetic — on exit `just` also leaves
   `PATH`, so a stale `_just` just completes nothing. Could drop
   it to halve the hook if we want minimal.

The hook is generic (binds any `_*` files the marker points at),
so it's written once and benefits every flake project, not just
this repo.

## Out of scope

bash/fish parity (just ships those completion files too, same
mechanism); making it work outside the flake shell (not wanted).

## Provenance

Researched in a 2026-06 session: confirmed direnv carries env
only (not functions), `shellHook` runs in bash and only its env
diff survives, the just nix package ships `_just`, and that
`FPATH` alone is insufficient without a post-`compinit` rebind —
all verified empirically against this repo's flake.
