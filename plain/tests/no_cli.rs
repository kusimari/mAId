//! Install for a plugin agent whose CLI is not on PATH. Its own test
//! binary, because it empties PATH for the whole process.

use plain::claude_config::ConfigDir;
use plain::place::{Deploy, Plugins, State};
use plain::record::{Item, What};
use plain::shared::Agent;
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
        claude: ConfigDir::new(h, None),
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

    // The part list says what should be there and why it is not.
    let placed = p.placed(None).unwrap();
    for agent in [Agent::Claude, Agent::Codex] {
        let why = format!("{} not on PATH", agent.cli());
        for item in [
            Item::skipped(What::Marketplace, "maid", &why),
            Item::skipped(What::Plugin, "maid@maid", &why),
        ] {
            assert!(placed.contains(&(agent, item)), "{placed:?}");
        }
    }
}

/// With no plugin CLI, the vendor plugins are recorded skipped with its reason.
#[test]
fn vendor_plugins_are_skipped_naming_the_missing_cli() {
    use plain::external::VENDORS;
    use plain::place::Vendors;
    let home = TempDir::new().unwrap();
    let empty = TempDir::new().unwrap();
    std::env::set_var("PATH", empty.path());
    let b = &VENDORS[0];
    let target = Vendors::new(
        home.path().to_path_buf(),
        ConfigDir::new(home.path(), None),
        VENDORS,
        &home.path().join("parts"),
    )
    .unwrap();
    assert!(target.install(None, false, false).unwrap().is_empty());
    let placed = target.placed(None).unwrap();
    for agent in [Agent::Claude, Agent::Codex] {
        let why = format!("{} not on PATH", agent.cli());
        for item in [
            Item::skipped(What::Marketplace, b.marketplace, &why),
            Item::skipped(What::Plugin, b.plugin_id(b.plugins[0]), &why),
        ] {
            assert!(placed.contains(&(agent, item)), "{placed:?}");
        }
    }
}

/// What mAId brought in stays its own with no CLI to list it, so a later
/// run with the CLI back still removes it; the rest is skipped as before.
#[test]
fn vendor_plugins_maid_placed_stay_its_own_with_no_cli() {
    use plain::external::VENDORS;
    use plain::place::Vendors;
    use plain::record;
    let home = TempDir::new().unwrap();
    let parts = home.path().join("parts");
    let b = &VENDORS[0];
    let ours = [
        Item::placed(What::Marketplace, b.marketplace),
        Item::placed(What::Plugin, b.plugin_id(b.plugins[0])),
    ];
    let kinds = &[What::Marketplace, What::Plugin];
    record::record(&parts, Agent::Claude, kinds, None, ours.to_vec()).unwrap();
    let empty = TempDir::new().unwrap();
    std::env::set_var("PATH", empty.path());
    let target = Vendors::new(
        home.path().to_path_buf(),
        ConfigDir::new(home.path(), None),
        VENDORS,
        &parts,
    )
    .unwrap();
    let placed = target.placed(None).unwrap();
    for item in ours {
        assert!(placed.contains(&(Agent::Claude, item)), "{placed:?}");
    }
    let why = "codex not on PATH";
    assert!(
        placed.contains(&(
            Agent::Codex,
            Item::skipped(What::Marketplace, b.marketplace, why)
        )),
        "{placed:?}"
    );
}
