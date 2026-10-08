_default:
    @just --list --unsorted --list-submodules

# ── modules ──────────────────────────────────────────────────────
# Per-area verbs live next to the area they operate on. Invoke as
# `just resources::install-skills`, `just kaimux::build`, etc.
mod resources
mod kaimux

# ── install ──────────────────────────────────────────────────────
# Everything mAId installs, for every coding agent (or one: claude|kiro|
# codex|agy). Today that is resources; kaimux and other tools join the
# profile later. Kiro's browser MCP goes into a named sub-agent only.

# The latest install takes over links an older mAId install left
# (including checkout links from before the profile) and updates the
# claude/codex plugin. Any other symlink, file or dir at a managed path
# is left alone and reported, the rest still installs, and the run exits
# non-zero.
# Build this checkout into the mAId profile and point the agents at it.
install agent="" kiro_sub="":
    #!/usr/bin/env bash
    set -u
    just resources::install-profile || exit 1
    rc=0
    just resources::install-skills "{{ agent }}" || rc=1
    just resources::install-browser-mcp "{{ agent }}" "{{ kiro_sub }}" || rc=1
    exit $rc

# Return every agent to the install before this one; run again to go further back.
rollback:
    just resources::rollback-profile

# Remove the plugin, the links, the MCP registration, and (for all agents) the profile.
uninstall agent="" kiro_sub="":
    just resources::uninstall-skills "{{ agent }}"
    just resources::uninstall-browser-mcp "{{ agent }}" "{{ kiro_sub }}"
    {{ if agent == "" { "just resources::uninstall-profile" } else { "true" } }}

# Report the profile generation, the plugins, the links, and the MCP registration.
status agent="" kiro_sub="":
    @just resources::status-profile
    @just resources::status-skills "{{ agent }}"
    @just resources::status-browser-mcp "{{ agent }}" "{{ kiro_sub }}"

# The installable Macro test against this machine's real install, free.
[confirm("This reinstalls mAId several times and leaves no earlier install to roll back to. Continue? (y/N)")]
verify-install:
    resources/tests/verify-install

# The same, also asking claude, kiro and codex what they see (9 model calls).
[confirm("This reinstalls mAId several times, leaves no earlier install to roll back to, and costs API credits (9 model calls). Continue? (y/N)")]
verify-install-paid:
    resources/tests/verify-install --ask-agents

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

# Full workspace quality + test gate.
ci: fmt-check lint check test
