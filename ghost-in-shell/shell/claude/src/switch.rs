//! On and off: the ghost's plugin enabled or disabled with Claude Code's
//! own command, for every session (user), one project (project), or one
//! person in one project (local); for one launch, `claude --settings
//! '{"enabledPlugins":{"ghost@ghost-in-shell":false}}'`. Nothing is
//! reinstalled. See the initiative spec `harness-core`, "Plain and ghost
//! on Claude Code".

use crate::package::PLUGIN_ID;
use crate::plugins::Cli;
use anyhow::{bail, Result};
use plain::claude_config::ConfigDir;
use std::path::Path;

/// The scopes Claude Code's settings have, most specific first.
pub const SCOPES: &[&str] = &["local", "project", "user"];

/// Switch the ghost on or off at `scope`, from the project at `here`.
pub fn switch(cli: &Cli, on: bool, scope: &str, here: &Path) -> Result<()> {
    if !SCOPES.contains(&scope) {
        bail!("scope {scope:?}: want user, project or local");
    }
    let verb = if on { "enable" } else { "disable" };
    // Claude Code takes a project's scopes from where it runs: the root.
    cli.run_in(
        Some(project_root(here)),
        &["plugin", verb, PLUGIN_ID, "--scope", scope],
    )?;
    println!("ghost    {verb}d at {scope}");
    Ok(())
}

/// The project a dir is in: the nearest dir up with `.git`, else the dir.
/// Both on/off and `state` work there, so they agree from a subdir.
pub fn project_root(here: &Path) -> &Path {
    here.ancestors()
        .find(|d| d.join(".git").exists())
        .unwrap_or(here)
}

/// Whether the ghost is on at `here`, and the scope that decides it: the
/// most specific settings file that names it, a project's read at its root.
/// None names it: off.
pub fn state(config: &ConfigDir, here: &Path) -> (bool, Option<&'static str>) {
    let here = project_root(here);
    let files = [
        ("local", here.join(".claude/settings.local.json")),
        ("project", here.join(".claude/settings.json")),
        ("user", config.settings()),
    ];
    for (scope, file) in files {
        let v: Option<serde_json::Value> = std::fs::read_to_string(&file)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok());
        if let Some(on) = v
            .as_ref()
            .and_then(|v| v["enabledPlugins"][PLUGIN_ID].as_bool())
        {
            return (on, Some(scope));
        }
    }
    (false, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(p: &Path, s: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, s).unwrap();
    }

    #[test]
    fn the_most_specific_scope_that_names_the_ghost_decides() {
        let dir = TempDir::new().unwrap();
        let home = dir.path().join("home");
        let here = dir.path().join("work");
        let config = ConfigDir::new(&home, None);
        let on = |v: bool| format!(r#"{{"enabledPlugins":{{"{PLUGIN_ID}":{v}}}}}"#);
        assert_eq!(state(&config, &here), (false, None));
        write(&home.join(".claude/settings.json"), &on(true));
        assert_eq!(state(&config, &here), (true, Some("user")));
        write(&here.join(".claude/settings.json"), &on(false));
        assert_eq!(state(&config, &here), (false, Some("project")));
        write(&here.join(".claude/settings.local.json"), &on(true));
        assert_eq!(state(&config, &here), (true, Some("local")));
        std::fs::create_dir_all(here.join(".git")).unwrap();
        let sub = here.join("src/deep");
        std::fs::create_dir_all(&sub).unwrap();
        assert_eq!(
            state(&config, &sub),
            (true, Some("local")),
            "from a subdir, the project's own settings"
        );
    }
}
