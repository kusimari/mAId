//! The `maid` plugin as plain stages it passes Claude Code's strict
//! validation: it carries nothing of the ghost (no `extensions`, hooks or
//! MCP config), so nothing in it is a field Claude Code refuses.
//!
//! Ignored by default: it needs `claude` on PATH. Free, no model; a temp
//! config dir.
//!
//!   cargo test -p plain --test strict -- --ignored

use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

#[test]
#[ignore = "needs claude; run with --ignored"]
fn the_staged_maid_plugin_passes_strict_validation() {
    let dir = TempDir::new().unwrap();
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../resources");
    let out = dir.path().join("out");
    plain::stage::plain(&resources, &out, &[]).unwrap();
    let ran = Command::new("claude")
        .args(["plugin", "validate", "--strict"])
        .arg(out.join(plain::stage::PLUGIN))
        .env("CLAUDE_CONFIG_DIR", dir.path().join("config"))
        .output()
        .expect("claude on PATH");
    assert!(
        ran.status.success(),
        "{}{}",
        String::from_utf8_lossy(&ran.stdout),
        String::from_utf8_lossy(&ran.stderr)
    );
}
