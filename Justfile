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

# ── build-tool hygiene ───────────────────────────────────────────
# These verbs operate on the Rust crate that implements install /
# uninstall / status. They never touch $HOME.

# Workspace unit tests for build-tool (sub-second; tempfile-fake-HOME).
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

# Full build-tool quality + test gate.
ci: fmt-check lint check test
