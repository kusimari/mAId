//! Paid: Claude Code with a real model, to settle what a request cannot
//! show, that the model acts on what the adapter places. Four runs in all,
//! two per test, each stopping at its first failure.
//!
//! The model tests are ignored, so `just test` runs only the free guard
//! test. Run them by hand, and only as granted, from the shell whose model
//! access Claude Code should use:
//!
//! ```text
//! MAID_PAID=1 CLAUDE_BIN=<claude> \
//!   cargo test -p ghost-in-shell-tests --test claude_paid -- --ignored --test-threads 1
//! ```
//!
//! Claude Code keeps the shell's HOME and environment, so its model access
//! is the shell's own and no credential is handed over. Three locations go to
//! a temp world: `CLAUDE_CONFIG_DIR`, `MY_HARNESS_STATE`
//! and `XDG_CACHE_HOME`, where Claude Code keeps its MCP logs whatever its
//! config dir. A run refuses to start if any of them, or the world, would
//! land under HOME. On a stub of the model API, nothing else was written
//! under HOME; other inherited locations are not redirected. A login kept
//! only in the real Claude config dir is not seen there; model access from
//! the environment is.

use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

use ghost_in_shell_tests::ClaudeHome;
const RUNS: usize = 2;
const MARKER: &str = "DEPLOY-MARKER-7";

/// A temp world with Claude's config dir holding mAId's part (a memory on a
/// store, a turn skill whose first rule asks for a marker), the ghost's
/// state, and a plugin built from the repo's Claude adapter files with the
/// cargo-built core.
fn world() -> TempDir {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    fs::create_dir_all(d.join("claude-config")).unwrap();
    fs::create_dir_all(d.join("cache")).unwrap();
    fs::create_dir_all(d.join("work")).unwrap();
    // mAId's part as Claude Code holds it: a plugin with the turn skill and
    // the memory's declaration, and the memory registered on its store.
    let kit = d.join("units/maid");
    fs::create_dir_all(kit.join(".claude-plugin")).unwrap();
    fs::write(
        kit.join(".claude-plugin/plugin.json"),
        json!({"name": "kit", "version": "1.0.0", "repository": "https://example.org/me/maid"})
            .to_string(),
    )
    .unwrap();
    fs::create_dir_all(kit.join("skills/deployer")).unwrap();
    fs::write(kit.join("skills/deployer/SKILL.md"), format!("---\nname: deployer\ndescription: deploy things\nmetadata:\n  my-harness.contract: \"1\"\n  my-harness.scope: turn\n  my-harness.job: deploy a release\n  my-harness.signals.jobs: deploy\n  my-harness.rules: SKILL.md#rules\n---\n## Rules\n\nThe first line of every reply about a deploy is the literal line {MARKER}.\n")).unwrap();
    fs::create_dir_all(kit.join("servers/personal-memory")).unwrap();
    fs::copy(
        ghost_in_shell_tests::content().join("servers/personal-memory/server.json"),
        kit.join("servers/personal-memory/server.json"),
    )
    .unwrap();
    let claude = ClaudeHome::in_dir(&d.join("claude-config"));
    claude.plugin("kit@kit", &kit);
    let store = d.join("personal-store");
    fs::create_dir_all(store.join("memory")).unwrap();
    claude.mcp(
        "personal-memory",
        "/bin/true",
        &["--store", &store.display().to_string()],
    );
    ghost_in_shell_tests::staged_ghost(d);
    dir
}

/// Every place a run writes, each refused if it is or resolves under
/// `home`: a run keeps the real HOME, so nothing may land there.
fn outside(home: &Path, places: &[PathBuf]) -> Result<(), String> {
    let home = home
        .canonicalize()
        .map_err(|e| format!("HOME {}: {e}", home.display()))?;
    for p in places {
        let at = p
            .canonicalize()
            .map_err(|e| format!("{}: {e}", p.display()))?;
        if at.starts_with(&home) {
            return Err(format!("{} is under HOME {}", at.display(), home.display()));
        }
    }
    Ok(())
}

fn ask(world: &Path, prompt: &str) -> String {
    assert_eq!(
        std::env::var("MAID_PAID").as_deref(),
        Ok("1"),
        "paid: set MAID_PAID=1"
    );
    let (config, state, cache) = (
        world.join("claude-config"),
        world.join("state"),
        world.join("cache"),
    );
    let home = std::env::var_os("HOME").expect("paid: HOME is the shell's own");
    if let Err(why) = outside(
        Path::new(&home),
        &[world.into(), config.clone(), state.clone(), cache.clone()],
    ) {
        panic!("paid: refused before any model call: {why}");
    }
    let claude = std::env::var("CLAUDE_BIN").unwrap_or_else(|_| "claude".into());
    let mut cmd = Command::new(claude);
    cmd.args(["-p", prompt, "--output-format", "text"])
        .arg("--plugin-dir")
        .arg(world.join("ghost").join(shell_claude::package::PLUGIN_DIR))
        .current_dir(world.join("work"))
        .env("CLAUDE_CONFIG_DIR", &config)
        .env("MY_HARNESS_STATE", &state)
        .env("XDG_CACHE_HOME", &cache);
    let out = cmd.output().unwrap();
    assert!(
        out.status.success(),
        "claude: {}{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
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

#[test]
fn a_place_under_home_is_refused_and_one_outside_passes() {
    let (home, away) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let inside = home.path().join("claude-config");
    fs::create_dir(&inside).unwrap();
    // A link from outside into HOME resolves under it.
    let link = away.path().join("state");
    std::os::unix::fs::symlink(&inside, &link).unwrap();
    assert!(outside(home.path(), &[away.path().into()]).is_ok());
    assert!(outside(home.path(), &[away.path().into(), inside]).is_err());
    assert!(outside(home.path(), &[link]).is_err());
    assert!(outside(home.path(), &[home.path().into()]).is_err());
}
