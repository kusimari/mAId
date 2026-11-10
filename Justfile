_default:
    @just --list --unsorted --list-submodules

# ── modules ──────────────────────────────────────────────────────
# Per-area verbs live next to the area they operate on. Invoke as
# `just plain::install-skills`, `just ghost::status`, `just kaimux::build`.
mod plain
mod ghost 'ghost-in-shell/shell/claude/Justfile'
mod kaimux

# ── install ──────────────────────────────────────────────────────
# Everything mAId installs, for every coding agent (or one: claude|kiro|
# codex|agy). Today that is resources; kaimux and other tools join the
# profile later.

# The latest install takes over links an older mAId install left
# (including checkout links from before the profile) and updates the
# claude/codex plugin. Any other symlink, file or dir at a managed path
# is left alone and reported, the rest still installs, and the run exits
# non-zero.
# Build this checkout into the mAId profile and point the agents at it.
install agent="":
    just plain::setup {{ quote(agent) }}

# Return every agent to the install before this one; run again to go further back.
rollback:
    just plain::rollback-profile

# A step that fails, such as one leaving a location alone as install
# does, is reported, the rest still runs, and the run exits non-zero.
# Remove the plugin, the links, the MCP registration, and (for all agents) the profile.
uninstall agent="":
    #!/usr/bin/env bash
    set -u
    rc=0
    just plain::uninstall-skills "{{ agent }}" || rc=1
    just plain::uninstall-browser-mcp "{{ agent }}" || rc=1
    {{ if agent == "" { "just plain::uninstall-profile || rc=1" } else { "" } }}
    exit $rc

# A step that fails stops the report and fails the run.
# Report the profile, plugins, links, MCP and the memory store; `--whole`: check every part's list on each harness.
# `--whole` goes in either place: `just status --whole [agent]` or `just status <agent> --whole`.
status agent="" whole="":
    #!/usr/bin/env bash
    set -eu
    if [ "{{ agent }}" = --whole ]; then exec just plain::status-whole "{{ whole }}"; fi
    if [ "{{ whole }}" = --whole ]; then exec just plain::status-whole "{{ agent }}"; fi
    [ -z "{{ whole }}" ] || { echo "status: unknown argument {{ whole }} (want: --whole)" >&2; exit 2; }
    just plain::status-profile
    just plain::status-skills "{{ agent }}"
    just plain::status-browser-mcp "{{ agent }}"
    just plain::status-memory

# ── memory ───────────────────────────────────────────────────────
# Your personal store is a checkout you own; see `just plain::status-memory`.

# Set your personal memory store (an existing checkout); `just install` registers its server.
# A relative path is taken from where you ran just, not the checkout.
memory-store path:
    just plain::set-memory-store {{ quote(if path =~ '^/' { path } else { invocation_directory() / path }) }}

# Copy each agent's own memory into a store dir, leaving the originals: --to <dir> [--project <path-prefix>].
memory-harvest *FLAGS:
    just plain::harvest-memory {{ FLAGS }}

# The installable Macro test against this machine's real install, free.
[confirm("This reinstalls mAId several times and leaves no earlier install to roll back to. Continue? (y/N)")]
verify-install:
    plain/tests/verify-install

# The same, also asking claude, kiro and codex what they see (9 model calls).
[confirm("This reinstalls mAId several times, leaves no earlier install to roll back to, and costs API credits (9 model calls). Continue? (y/N)")]
verify-install-paid:
    plain/tests/verify-install --ask-agents

# ── workspace hygiene ────────────────────────────────────────────
# These verbs operate on the Rust workspace itself. They never touch
# $HOME or AI tools.

# Workspace unit tests for every member.
test:
    cargo test --workspace

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

lint:
    cargo clippy --workspace --all-targets -- -D warnings

check:
    cargo check --workspace

# Every my-harness declaration the ghost will read.
validate:
    just ghost::validate

# Full workspace quality + test gate.
ci: fmt-check lint check validate test
