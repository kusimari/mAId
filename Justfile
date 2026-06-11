_default:
    @just --list --unsorted

# ── resources ────────────────────────────────────────────────────

# Validate frontmatter in resources/content/.
validate:
    cargo run -p build-tool --release --quiet -- validate

# Validate, then create/refresh $HOME-facing symlinks.
deploy:
    cargo run -p build-tool --release --quiet -- deploy

# Remove deploy-managed symlinks.
undeploy:
    cargo run -p build-tool --release --quiet -- undeploy

# Report each managed symlink's state.
status:
    cargo run -p build-tool --release --quiet -- status

# ── tests ────────────────────────────────────────────────────────

# Workspace unit tests (build-tool today; future members join).
test:
    cargo test --workspace

# Structural smoke — symlinks resolve, skills reachable. No API credits.
test-smoke:
    resources/tests/run --no-tools

# Tool-driven functional smoke — costs API credits, slow.
# Confirmation prompt: this is for human use, not agentic runs.
[confirm("This costs API credits and takes minutes. Continue? (y/N)")]
test-functional:
    resources/tests/run

# Single fixture by name (e.g. `just test-fixture kdevkit`).
test-fixture name:
    resources/tests/run "{{ name }}"

# ── quality ──────────────────────────────────────────────────────

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

lint:
    cargo clippy --workspace --all-targets -- -D warnings

check:
    cargo check --workspace

# Full quality + test gate.
ci: fmt-check lint check test
