//! The ghost's uninstall: its plugin, and its marketplace while that
//! points at its profile, whether or not a record names them (an install
//! that stopped partway placed them first); then its profile, the
//! adapter's cache of Claude Code's records, and the record.
//! Plain is left whole. See the initiative spec `harness-core`, "Plain,
//! then the ghost".

use crate::install::Setup;
use crate::package::{MARKETPLACE, MARKETPLACE_DIR, PLUGIN_ID};
use anyhow::Result;
use plain::record::{self, What};
use plain::shared::Agent;
use std::fs;

/// `ghost-claude uninstall`.
pub fn uninstall(s: &Setup) -> Result<u8> {
    let placed = record::placed(&s.record(), Agent::Claude)?;
    let has = |kind: What, name: &str| placed.iter().any(|i| i.kind == kind && i.name == name);
    let cli = s.cli();
    let ours = s.profile.join(MARKETPLACE_DIR);
    let market = cli.marketplace(MARKETPLACE)?;
    let from_ours = market.as_deref() == Some(ours.as_path());
    if (has(What::Plugin, PLUGIN_ID) || from_ours) && cli.listed(PLUGIN_ID)?.is_some() {
        cli.run(&["plugin", "uninstall", PLUGIN_ID, "--scope", "user"])?;
        println!("removed  claude plugin {PLUGIN_ID}");
    }
    match market {
        Some(at) if at == ours => {
            cli.run(&["plugin", "marketplace", "remove", MARKETPLACE])?;
            println!("removed  claude marketplace {MARKETPLACE}");
        }
        Some(at) => println!(
            "left     claude marketplace {MARKETPLACE} at {} (not the ghost's)",
            at.display()
        ),
        None => {}
    }
    s.profile().uninstall()?;
    println!("removed  {}", s.profile.display());
    let shells = s.state.join("shells");
    if shells.exists() {
        fs::remove_dir_all(&shells)?;
        println!("removed  {}", shells.display());
    }
    if let Ok(here) = std::env::current_dir() {
        for line in left_switched(&here) {
            println!("{line}");
        }
    }
    record::record(
        &s.record(),
        Agent::Claude,
        &[What::Marketplace, What::Plugin],
        None,
        vec![],
    )?;
    Ok(0)
}

/// The project and local settings at `here`'s project that still name the
/// ghost: Claude Code leaves them when the plugin goes, and they are the
/// person's to remove.
pub fn left_switched(here: &std::path::Path) -> Vec<String> {
    let root = crate::switch::project_root(here);
    [".claude/settings.json", ".claude/settings.local.json"]
        .iter()
        .map(|f| root.join(f))
        .filter(|f| {
            fs::read_to_string(f)
                .ok()
                .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                .is_some_and(|v| !v["enabledPlugins"][PLUGIN_ID].is_null())
        })
        .map(|f| {
            format!(
                "left     {} still names {PLUGIN_ID} in enabledPlugins; remove it there",
                f.display()
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_project_and_local_switches_left_behind_are_named() {
        let dir = tempfile::TempDir::new().unwrap();
        let here = dir.path();
        assert!(left_switched(here).is_empty());
        fs::create_dir_all(here.join(".claude")).unwrap();
        fs::write(
            here.join(".claude/settings.local.json"),
            format!(r#"{{"enabledPlugins":{{"{PLUGIN_ID}":false}}}}"#),
        )
        .unwrap();
        fs::write(
            here.join(".claude/settings.json"),
            r#"{"enabledPlugins":{}}"#,
        )
        .unwrap();
        let left = left_switched(here);
        assert_eq!(left.len(), 1, "{left:?}");
        assert!(left[0].contains("settings.local.json"), "{left:?}");
    }
}
