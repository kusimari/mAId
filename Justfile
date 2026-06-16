_default:
    @just --list --unsorted --list-submodules

# ── modules ──────────────────────────────────────────────────────
# Per-area verbs live next to the area they operate on. Invoke as
# `just resources::install`, `just kaimux::build`, etc.
mod resources
mod kaimux

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
