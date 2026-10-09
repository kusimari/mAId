//! Install for a plugin agent whose CLI is not on PATH. Its own test
//! binary, because it empties PATH for the whole process.

use build_tool::deploy::{Deploy, Plugins, State};
use build_tool::shared::Agent;
use std::fs;
use tempfile::TempDir;

/// With no CLI there is no plugin to install, so the old links stay:
/// reaping them would leave the agent with no mAId skills at all.
#[test]
fn old_links_stay_when_the_agents_cli_is_missing() {
    let home = TempDir::new().unwrap();
    let h = home.path();
    let profile = h.join(".local/state/maid/profile");
    let manifest = profile.join("share/maid/marketplace/plugins/maid/.codex-plugin/plugin.json");
    fs::create_dir_all(manifest.parent().unwrap()).unwrap();
    fs::write(&manifest, r#"{"name":"maid","version":"1.0.0-a"}"#).unwrap();
    let skills = profile.join("share/maid/skills");
    fs::create_dir_all(h.join(".codex/skills")).unwrap();
    fs::create_dir_all(h.join(".claude")).unwrap();
    std::os::unix::fs::symlink(&skills, h.join(".claude/skills")).unwrap();
    std::os::unix::fs::symlink(skills.join("notes"), h.join(".codex/skills/notes")).unwrap();

    let empty = TempDir::new().unwrap();
    std::env::set_var("PATH", empty.path());
    let p = Plugins {
        home: h.to_path_buf(),
        profile,
    };
    let reports = p.install(None, false, false).unwrap();

    for agent in [Agent::Claude, Agent::Codex] {
        let label = format!("{} plugin maid@maid", agent.name());
        let r = reports.iter().find(|r| r.label == label).unwrap();
        assert_eq!(r.state, State::NoCli(agent.cli()));
    }
    assert!(fs::symlink_metadata(h.join(".claude/skills")).is_ok());
    assert!(fs::symlink_metadata(h.join(".codex/skills/notes")).is_ok());
}
