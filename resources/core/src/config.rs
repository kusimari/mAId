//! Where the core reads and writes. Every location is a config value with a
//! default under the user's state dir, so the binary runs from a hook with
//! no Justfile and no checkout.

use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Every part's list: `MY_HARNESS_PARTS`, default `<state>/parts`.
    pub parts: PathBuf,
    /// The core's own state (delivery record, pending decisions):
    /// `MY_HARNESS_STATE`, default `${XDG_STATE_HOME:-$HOME/.local/state}/my-harness`.
    pub state: PathBuf,
    /// Claude Code's transcripts: `ClaudeConfig::projects`.
    pub claude: Option<PathBuf>,
}

/// A path from the environment. An empty value is unset; a relative one is
/// a mistake, not the cwd.
fn path_var(var: &impl Fn(&str) -> Option<String>, k: &str) -> Result<Option<PathBuf>> {
    match var(k).filter(|v| !v.is_empty()) {
        None => Ok(None),
        Some(v) => {
            let p = PathBuf::from(&v);
            p.is_absolute()
                .then_some(Some(p))
                .ok_or_else(|| anyhow!("{k} must be an absolute path (got {v:?})"))
        }
    }
}

/// Where Claude Code keeps its user config, as 2.1.314 places it: with
/// `CLAUDE_CONFIG_DIR` set, everything is in that dir and nothing is read
/// from `$HOME`; unset, the dir is `$HOME/.claude` and the global file
/// `$HOME/.claude.json`. So the default is not the variable set to
/// `$HOME/.claude`, and a `claude` started for this config must get the
/// variable exactly as `var()` says.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClaudeConfig {
    home: Option<PathBuf>,
    var: Option<PathBuf>,
}

impl ClaudeConfig {
    /// `home` with the variable at `var`, or unset.
    pub fn new(home: &Path, var: Option<&Path>) -> ClaudeConfig {
        ClaudeConfig {
            home: Some(home.to_path_buf()),
            var: var.map(Path::to_path_buf),
        }
    }

    pub fn from_env() -> Result<Option<ClaudeConfig>> {
        ClaudeConfig::from_vars(|k| std::env::var(k).ok())
    }

    /// `None` when neither `HOME` nor the variable is set.
    pub fn from_vars(var: impl Fn(&str) -> Option<String>) -> Result<Option<ClaudeConfig>> {
        let set = path_var(&var, "CLAUDE_CONFIG_DIR")?;
        // With the variable set Claude needs no HOME, so a bad one is not ours to refuse.
        let home = match path_var(&var, "HOME") {
            Err(_) if set.is_some() => None,
            h => h?,
        };
        let var = set;
        Ok((home.is_some() || var.is_some()).then_some(ClaudeConfig { home, var }))
    }

    pub fn home(&self) -> Option<&Path> {
        self.home.as_deref()
    }

    /// `CLAUDE_CONFIG_DIR`, if set.
    pub fn var(&self) -> Option<&Path> {
        self.var.as_deref()
    }

    /// The user config dir: settings, agents, skills, plugins, projects.
    pub fn dir(&self) -> PathBuf {
        match (&self.var, &self.home) {
            (Some(v), _) => v.clone(),
            (None, Some(h)) => h.join(".claude"),
            (None, None) => unreachable!("built with HOME or the variable"),
        }
    }

    /// `.claude.json`: user MCP servers among Claude's own state.
    pub fn global(&self) -> PathBuf {
        match (&self.var, &self.home) {
            (Some(v), _) => v.join(".claude.json"),
            (None, Some(h)) => h.join(".claude.json"),
            (None, None) => unreachable!("built with HOME or the variable"),
        }
    }

    pub fn settings(&self) -> PathBuf {
        self.dir().join("settings.json")
    }

    pub fn projects(&self) -> PathBuf {
        self.dir().join("projects")
    }
}

impl Config {
    pub fn from_env() -> Result<Config> {
        Config::from_vars(|k| std::env::var(k).ok())
    }

    /// The same, from any lookup, so tests never read the real environment.
    pub fn from_vars(var: impl Fn(&str) -> Option<String>) -> Result<Config> {
        let get = |k: &str| path_var(&var, k);
        let state = match get("MY_HARNESS_STATE")? {
            Some(s) => s,
            None => {
                let base = match get("XDG_STATE_HOME") {
                    Ok(Some(b)) => b,
                    // The XDG spec says to ignore a relative value.
                    _ => get("HOME")?
                        .ok_or_else(|| anyhow!("HOME is not set"))?
                        .join(".local/state"),
                };
                base.join("my-harness")
            }
        };
        let parts = get("MY_HARNESS_PARTS")?.unwrap_or_else(|| state.join("parts"));
        let claude = ClaudeConfig::from_vars(&var)?.map(|c| c.projects());
        Ok(Config {
            parts,
            state,
            claude,
        })
    }

    pub fn delivery(&self) -> PathBuf {
        self.state.join("delivery")
    }

    pub fn pending(&self) -> PathBuf {
        self.state.join("pending")
    }

    pub fn sleep(&self) -> PathBuf {
        self.state.join("sleep")
    }

    pub fn experience(&self) -> PathBuf {
        self.state.join("experience")
    }

    pub fn proposals(&self) -> PathBuf {
        self.state.join("proposals")
    }

    pub fn notes(&self) -> PathBuf {
        self.state.join("notes")
    }

    /// The session in full as its brief showed the reflector, kept only
    /// until its batch is decided or dismissed. Callers check the id
    /// (`delivery::check_id`) first.
    pub fn briefed(&self, session: &str) -> PathBuf {
        self.state.join("briefed").join(format!("{session}.json"))
    }

    pub fn ledger(&self) -> PathBuf {
        self.state.join("ledger.jsonl")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn from(pairs: &[(&str, &str)]) -> Result<Config> {
        let vars: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Config::from_vars(|k| vars.get(k).cloned())
    }

    #[test]
    fn defaults_sit_under_the_users_state_dir() {
        let c = from(&[("HOME", "/h")]).unwrap();
        assert_eq!(c.state, PathBuf::from("/h/.local/state/my-harness"));
        assert_eq!(c.parts, PathBuf::from("/h/.local/state/my-harness/parts"));
        let c = from(&[("HOME", "/h"), ("XDG_STATE_HOME", "/x")]).unwrap();
        assert_eq!(c.parts, PathBuf::from("/x/my-harness/parts"));
        assert_eq!(c.claude, Some(PathBuf::from("/h/.claude/projects")));
        let c = from(&[("HOME", "/h"), ("CLAUDE_CONFIG_DIR", "/c")]).unwrap();
        assert_eq!(c.claude, Some(PathBuf::from("/c/projects")));
        let c = from(&[("MY_HARNESS_STATE", "/s")]).unwrap();
        assert_eq!(c.claude, None);
    }

    fn claude(pairs: &[(&str, &str)]) -> Option<ClaudeConfig> {
        let vars: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        ClaudeConfig::from_vars(|k| vars.get(k).cloned()).unwrap()
    }

    #[test]
    fn claudes_config_is_under_home_without_the_variable() {
        for pairs in [
            &[("HOME", "/h")][..],
            &[("HOME", "/h"), ("CLAUDE_CONFIG_DIR", "")],
        ] {
            let c = claude(pairs).unwrap();
            assert_eq!(c.dir(), PathBuf::from("/h/.claude"));
            assert_eq!(c.settings(), PathBuf::from("/h/.claude/settings.json"));
            assert_eq!(c.global(), PathBuf::from("/h/.claude.json"));
            assert_eq!(c.projects(), PathBuf::from("/h/.claude/projects"));
            assert_eq!(c.var(), None);
        }
    }

    #[test]
    fn claudes_config_is_all_in_the_variables_dir_when_set() {
        let c = claude(&[("HOME", "/h"), ("CLAUDE_CONFIG_DIR", "/c")]).unwrap();
        assert_eq!(c.settings(), PathBuf::from("/c/settings.json"));
        assert_eq!(c.global(), PathBuf::from("/c/.claude.json"));
        assert_eq!(c.projects(), PathBuf::from("/c/projects"));
        assert_eq!(
            (c.var(), c.home()),
            (Some(Path::new("/c")), Some(Path::new("/h")))
        );
        let c = claude(&[("CLAUDE_CONFIG_DIR", "/c")]).unwrap();
        assert_eq!(c.global(), PathBuf::from("/c/.claude.json"));
        assert_eq!(c.home(), None);
    }

    #[test]
    fn claudes_config_needs_home_or_an_absolute_variable() {
        assert_eq!(claude(&[]), None);
        let vars = |k: &str| (k == "CLAUDE_CONFIG_DIR").then(|| "c".to_string());
        assert!(ClaudeConfig::from_vars(vars).is_err());
        let home = |k: &str| (k == "HOME").then(|| "h".to_string());
        assert!(ClaudeConfig::from_vars(home).is_err());
        let c = claude(&[("HOME", "h"), ("CLAUDE_CONFIG_DIR", "/c")]).unwrap();
        assert_eq!((c.dir(), c.home()), (PathBuf::from("/c"), None));
    }

    #[test]
    fn each_location_can_be_set_on_its_own() {
        let c = from(&[("MY_HARNESS_STATE", "/s"), ("MY_HARNESS_PARTS", "/p")]).unwrap();
        assert_eq!((c.state, c.parts), ("/s".into(), "/p".into()));
    }

    #[test]
    fn a_relative_location_is_refused_and_a_relative_xdg_ignored() {
        assert!(from(&[("HOME", "/h"), ("MY_HARNESS_PARTS", "p")]).is_err());
        let c = from(&[("HOME", "/h"), ("XDG_STATE_HOME", "x")]).unwrap();
        assert_eq!(c.state, PathBuf::from("/h/.local/state/my-harness"));
        assert!(from(&[]).is_err());
    }
}
