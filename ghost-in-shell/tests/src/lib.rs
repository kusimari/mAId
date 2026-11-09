//! What the tests that span crates share: where the repo and the ghost's
//! own things are, the fixture session, and the installed binary. The
//! tests themselves are in `tests/`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// The repo root: this crate sits at `ghost-in-shell/tests`.
pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the checkout is there")
}

/// The ghost's own Claude plugin, as the checkout holds it.
pub fn plugin() -> PathBuf {
    repo().join("ghost-in-shell/shell/claude/plugin")
}

/// The person's resources, as the checkout holds them.
pub fn content() -> PathBuf {
    repo().join("resources/content")
}

/// The binary the carriers call, built from this checkout. Cargo hands a
/// test the path of its own package's binaries only, so this builds
/// `shell-claude`'s once per test binary and reads the path cargo gives.
pub fn core() -> &'static str {
    static CORE: OnceLock<String> = OnceLock::new();
    CORE.get_or_init(|| {
        let out = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args([
                "build",
                "--quiet",
                // The lock is committed, as the flake's build takes it: a
                // test never rewrites it or reaches the network for it.
                "--locked",
                "--message-format",
                "json-render-diagnostics",
                "-p",
                "shell-claude",
                "--bin",
                "maid-core",
            ])
            .current_dir(repo())
            .output()
            .expect("cargo on PATH");
        assert!(
            out.status.success(),
            "cargo build -p shell-claude: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .filter_map(|v| v["executable"].as_str().map(PathBuf::from))
            .find(|p| p.file_name().is_some_and(|n| n == "maid-core"))
            .expect("cargo says where it put maid-core")
            .display()
            .to_string()
    })
}

/// The fixture Claude Code session (`tests/fixtures/claude/-work`).
pub const SESSION: &str = "0b7c6a8e-1111-4222-8333-944455556666";

/// The fixture projects dir, as the checkout holds it.
pub fn fixture_projects() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/claude")
}

/// A copy of the fixture projects dir at `<dir>/claude`.
pub fn claude_projects(dir: &Path) -> PathBuf {
    let to = dir.join("claude");
    copy(&fixture_projects(), &to);
    to
}

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let t = to.join(e.file_name());
        match e.file_type().unwrap().is_dir() {
            true => copy(&e.path(), &t),
            false => {
                std::fs::copy(e.path(), t).unwrap();
            }
        }
    }
}
