_default:
    @just --list --unsorted --list-submodules

# ── modules ──────────────────────────────────────────────────────
# Per-area verbs live next to the area they operate on. Invoke as
# `just resources::install-skills`, `just kaimux::build`, etc.
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

lint: lint-shell
    cargo clippy --workspace --all-targets -- -D warnings

# The phase guarantee lives in shell, so it gets linted like the Rust does.
# A `set -e` defect in the push gate reached review because nothing checked.
lint-shell:
    shellcheck -s sh resources/content/skills/kdevkit/tools/feature-loop \
        resources/content/skills/kdevkit/tools/hooks/*

check:
    cargo check --workspace

# Full workspace quality + test gate.
ci: fmt-check lint check test
