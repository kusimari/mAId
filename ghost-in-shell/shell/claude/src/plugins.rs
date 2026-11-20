//! Claude Code's own plugin commands, as the ghost's install runs them:
//! its marketplace added at the profile, its plugin installed, updated,
//! enabled, disabled or removed, and what Claude Code lists of both. Every
//! `claude` gets the config dir exactly as resolved. See the initiative
//! spec `harness-core`, "Installing and wiring".

use anyhow::{anyhow, bail, Context, Result};
use plain::claude_config::ConfigDir;
use plain::shared::{claude_env, find_on};
use serde_json::Value;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The `claude` on a PATH, for one config dir.
pub struct Cli {
    pub config: ConfigDir,
    pub path: Option<OsString>,
}

/// What `claude plugin list` says of one plugin at the user's scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub version: String,
    pub enabled: bool,
}

impl Cli {
    pub fn from_env(config: ConfigDir) -> Cli {
        Cli {
            config,
            path: std::env::var_os("PATH"),
        }
    }

    /// Run `claude <args>`, failing with what it said.
    pub fn run(&self, args: &[&str]) -> Result<String> {
        self.run_in(None, args)
    }

    /// The same, from the dir `here`, for a project's scopes.
    pub fn run_in(&self, here: Option<&Path>, args: &[&str]) -> Result<String> {
        let claude = find_on(self.path.as_deref(), "claude")
            .ok_or_else(|| anyhow!("claude is not on PATH"))?;
        let mut cmd = Command::new(claude);
        cmd.args(args)
            .env("PATH", self.path.clone().unwrap_or_default());
        if let Some(dir) = here {
            cmd.current_dir(dir);
        }
        claude_env(&mut cmd, &self.config);
        let out = cmd.output().context("cannot run claude")?;
        if !out.status.success() {
            bail!(
                "claude {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// Where Claude Code has marketplace `name`, if it has it.
    pub fn marketplace(&self, name: &str) -> Result<Option<PathBuf>> {
        let v: Value =
            serde_json::from_str(&self.run(&["plugin", "marketplace", "list", "--json"])?)
                .context("claude plugin marketplace list --json: not JSON")?;
        let entries = v
            .as_array()
            .ok_or_else(|| anyhow!("claude plugin marketplace list --json: unexpected shape"))?;
        Ok(entries
            .iter()
            .find(|e| e["name"] == name)
            .and_then(|e| e["path"].as_str())
            .map(PathBuf::from))
    }

    /// What Claude Code lists of plugin `id` at the user's scope.
    pub fn listed(&self, id: &str) -> Result<Option<Listed>> {
        let v: Value = serde_json::from_str(&self.run(&["plugin", "list", "--json"])?)
            .context("claude plugin list --json: not JSON")?;
        let entries = v
            .as_array()
            .ok_or_else(|| anyhow!("claude plugin list --json: unexpected shape"))?;
        Ok(entries
            .iter()
            .find(|e| e["id"] == id && e["scope"] == "user")
            .map(|e| Listed {
                version: e["version"].as_str().unwrap_or_default().to_string(),
                enabled: e["enabled"].as_bool().unwrap_or(false),
            }))
    }
}

/// The calls that bring Claude Code to the plugin at `dir`, given what it
/// has: the marketplace added where it is elsewhere or missing (Claude
/// Code repoints on add), the plugin installed, or updated to `want`.
pub fn place_steps(
    market: &str,
    id: &str,
    dir: &Path,
    registered: Option<&Path>,
    listed: Option<&Listed>,
    want: &str,
) -> Vec<Vec<String>> {
    let s = |a: &[&str]| a.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    let dir = dir.display().to_string();
    let mut steps = vec![];
    if registered != Some(Path::new(&dir)) {
        steps.push(s(&["plugin", "marketplace", "add", &dir]));
    }
    match listed {
        None => steps.push(s(&["plugin", "install", id, "--scope", "user"])),
        Some(l) if l.version != want => {
            steps.push(s(&["plugin", "marketplace", "update", market]));
            steps.push(s(&["plugin", "update", id, "--scope", "user"]));
        }
        Some(_) => {}
    }
    steps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placing_does_only_what_is_missing() {
        let dir = Path::new("/p/share/ghost/marketplace");
        let listed = |v: &str| Listed {
            version: v.into(),
            enabled: true,
        };
        let steps = |r: Option<&Path>, l: Option<&Listed>| place_steps("m", "g@m", dir, r, l, "v2");
        assert_eq!(steps(None, None).len(), 2);
        assert!(steps(Some(dir), Some(&listed("v2"))).is_empty());
        let s = steps(Some(dir), Some(&listed("v1")));
        assert_eq!(s[1], ["plugin", "update", "g@m", "--scope", "user"]);
        let s = steps(Some(Path::new("/elsewhere")), Some(&listed("v2")));
        assert_eq!(
            s,
            [["plugin", "marketplace", "add", "/p/share/ghost/marketplace"]]
        );
    }
}
