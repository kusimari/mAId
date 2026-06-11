_default:
    @just --list --unsorted

# ── resources ────────────────────────────────────────────────────
# These verbs operate on $HOME — they read or write the symlinks
# the AI tools consume. Validation of content runs inside `install`.

# Validate content and create/refresh $HOME-facing symlinks.
install:
    cargo run -p build-tool --release --quiet -- install

# Remove install-managed symlinks.
uninstall:
    cargo run -p build-tool --release --quiet -- uninstall

# Report each managed symlink's state.
status:
    cargo run -p build-tool --release --quiet -- status

# Drive the AI tools (claude --print, kiro-cli) against the
# installed content to confirm skills load correctly. Costs API
# credits and takes minutes — gated behind a confirmation prompt.
[confirm("This costs API credits and takes minutes. Continue? (y/N)")]
verify:
    resources/tests/run

# Drive a single fixture (e.g. `just verify-one kdevkit`).
verify-one name:
    resources/tests/run "{{ name }}"

# ── kaimux ───────────────────────────────────────────────────────
# tmux-pane orchestrator for coding-agent sessions. Builds in release
# and lands a flat binary at dist/kaimux. No symlinking; users invoke
# the binary directly (typically via the tmux keybinding kaimux setup
# installs).

# Build kaimux in release; copy to dist/kaimux.
kaimux-build:
    cargo build -p kaimux --release
    mkdir -p dist
    cp target/release/kaimux dist/kaimux

# Kaimux unit tests (sub-second).
kaimux-test:
    cargo test -p kaimux

# Kaimux end-to-end integration test (real tmux, requires tmux + jq).
kaimux-integration: kaimux-build
    kaimux/tests/integration.sh

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
