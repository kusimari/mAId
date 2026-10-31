//! Paid: Claude Code with a real model, to settle what a request cannot
//! show, that the model acts on what the adapter places. Four runs in all,
//! two per test, each stopping at its first failure.
//!
//! Never run by `just test`. Run by hand, and only as granted:
//!
//! ```text
//! MAID_PAID=1 MAID_PAID_ENV="<auth variable names>" \
//!   cargo test -p maid-core --test claude_paid -- --ignored --test-threads 1
//! ```
//!
//! `MAID_PAID_ENV` names the variables Claude Code needs for its model
//! (an API key, or a provider's settings), passed through by name only.
//! Claude Code runs in a temp HOME, so nothing real is read or written.

use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

const CORE: &str = env!("CARGO_BIN_EXE_maid-core");
const RUNS: usize = 2;
const MARKER: &str = "DEPLOY-MARKER-7";

/// A temp HOME with mAId's part (a memory on a store, a turn skill whose
/// first rule asks for a marker) and a plugin built from the repo's Claude
/// adapter files with the cargo-built core.
fn world() -> TempDir {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let parts = d.join("home/.local/state/my-harness/parts");
    fs::create_dir_all(&parts).unwrap();
    fs::create_dir_all(d.join("work")).unwrap();
    let unit = d.join("units/deployer/SKILL.md");
    fs::create_dir_all(unit.parent().unwrap()).unwrap();
    fs::write(&unit, format!("---\nname: deployer\ndescription: deploy things\nmetadata:\n  my-harness.scope: turn\n  my-harness.job: deploy a release\n  my-harness.signals.jobs: deploy\n  my-harness.rules: SKILL.md#rules\n---\n## Rules\n\nThe first line of every reply about a deploy is the literal line {MARKER}.\n")).unwrap();
    let store = d.join("personal-store");
    fs::create_dir_all(store.join("memory")).unwrap();
    let server = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../content/servers/personal-memory/server.json")
        .canonicalize()
        .unwrap();
    let mine = json!({"format": 1, "part": "maid-harness", "persona": "personal", "harnesses": {},
        "bring_ins": [
            {"name": "deployer", "unit": {"standard": "skill", "path": unit}},
            {"name": "personal-memory", "unit": {"standard": "mcp", "path": server},
                "values": {"store": store, "index": "memory/MEMORY.md"}}]});
    fs::write(parts.join("maid-harness.json"), mine.to_string()).unwrap();
    let adapter = Path::new(env!("CARGO_MANIFEST_DIR")).join("../content/adapters/claude");
    let plugin = d.join("plugin");
    fs::create_dir_all(plugin.join(".claude-plugin")).unwrap();
    fs::create_dir_all(plugin.join("hooks")).unwrap();
    let decl: Value =
        serde_json::from_str(&fs::read_to_string(adapter.join("my-harness.json")).unwrap())
            .unwrap();
    let manifest = json!({"name": "maid", "version": "1.0.0", "description": "mAId's skills",
        "author": {"name": "mAId"}, "extensions": {"my-harness": decl},
        "hooks": "./hooks/claude.json", "mcpServers": "./claude-mcp.json"});
    fs::write(
        plugin.join(".claude-plugin/plugin.json"),
        manifest.to_string(),
    )
    .unwrap();
    for (from, to) in [
        ("hooks.json", "hooks/claude.json"),
        ("mcp.json", "claude-mcp.json"),
    ] {
        let text = fs::read_to_string(adapter.join(from))
            .unwrap()
            .replace("@maid-core@", CORE);
        fs::write(plugin.join(to), text).unwrap();
    }
    dir
}

fn ask(world: &Path, prompt: &str) -> String {
    assert_eq!(
        std::env::var("MAID_PAID").as_deref(),
        Ok("1"),
        "paid: set MAID_PAID=1"
    );
    let claude = std::env::var("CLAUDE_BIN").unwrap_or_else(|_| "claude".into());
    let mut cmd = Command::new(claude);
    cmd.args(["-p", prompt, "--output-format", "text"])
        .arg("--plugin-dir")
        .arg(world.join("plugin"))
        .current_dir(world.join("work"))
        .env_clear()
        .env("HOME", world.join("home"))
        .env("PATH", std::env::var("PATH").unwrap_or_default());
    for name in std::env::var("MAID_PAID_ENV")
        .unwrap_or_default()
        .split_whitespace()
    {
        if let Ok(v) = std::env::var(name) {
            cmd.env(name, v);
        }
    }
    let out = cmd.output().unwrap();
    assert!(
        out.status.success(),
        "claude: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

#[test]
#[ignore = "paid: two model runs"]
fn a_fresh_session_knows_its_persona_and_store_from_the_genome() {
    for run in 1..=RUNS {
        let w = world();
        let store = w.path().join("personal-store");
        let reply = ask(
            w.path(),
            "In one line, without using any tool: which persona are you acting through, and at what path is its memory store?",
        );
        assert!(
            reply.contains("personal") && reply.contains(&store.display().to_string()),
            "run {run}: {reply}"
        );
    }
}

#[test]
#[ignore = "paid: two model runs"]
fn a_fitting_bring_ins_first_rule_is_followed() {
    for run in 1..=RUNS {
        let w = world();
        let reply = ask(
            w.path(),
            "Without using any tool: in two lines, how would you deploy the release of a small static site?",
        );
        assert!(reply.trim_start().starts_with(MARKER), "run {run}: {reply}");
    }
}
