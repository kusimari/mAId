//! Where Claude Code keeps its user config, and so where this shell's
//! own things are: its settings, its agents and skills, its plugins and
//! its transcripts. The ghost's own locations are `ghost::config`.

use anyhow::Result;
use ghost::config::path_var;
use std::path::{Path, PathBuf};

/// Where Claude Code keeps its user config, as 2.1.314 places it: with
/// `CLAUDE_CONFIG_DIR` set, everything is in that dir and nothing is read
/// from `$HOME`; unset, the dir is `$HOME/.claude` and the global file
/// `$HOME/.claude.json`. So the default is not the variable set to
/// `$HOME/.claude`, and a `claude` started for this config must get the
/// variable exactly as `var()` says.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConfigDir {
    home: Option<PathBuf>,
    var: Option<PathBuf>,
}

impl ConfigDir {
    /// `home` with the variable at `var`, or unset.
    pub fn new(home: &Path, var: Option<&Path>) -> ConfigDir {
        ConfigDir {
            home: Some(home.to_path_buf()),
            var: var.map(Path::to_path_buf),
        }
    }

    pub fn from_env() -> Result<Option<ConfigDir>> {
        ConfigDir::from_vars(|k| std::env::var(k).ok())
    }

    /// `None` when neither `HOME` nor the variable is set.
    pub fn from_vars(var: impl Fn(&str) -> Option<String>) -> Result<Option<ConfigDir>> {
        let set = path_var(&var, "CLAUDE_CONFIG_DIR")?;
        // With the variable set Claude needs no HOME, so a bad one is not ours to refuse.
        let home = match path_var(&var, "HOME") {
            Err(_) if set.is_some() => None,
            h => h?,
        };
        let var = set;
        Ok((home.is_some() || var.is_some()).then_some(ConfigDir { home, var }))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn dir(pairs: &[(&str, &str)]) -> Option<ConfigDir> {
        let vars: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        ConfigDir::from_vars(|k| vars.get(k).cloned()).unwrap()
    }

    #[test]
    fn claudes_config_is_under_home_without_the_variable() {
        for pairs in [
            &[("HOME", "/h")][..],
            &[("HOME", "/h"), ("CLAUDE_CONFIG_DIR", "")],
        ] {
            let c = dir(pairs).unwrap();
            assert_eq!(c.dir(), PathBuf::from("/h/.claude"));
            assert_eq!(c.settings(), PathBuf::from("/h/.claude/settings.json"));
            assert_eq!(c.global(), PathBuf::from("/h/.claude.json"));
            assert_eq!(c.projects(), PathBuf::from("/h/.claude/projects"));
            assert_eq!(c.var(), None);
        }
    }

    #[test]
    fn claudes_config_is_all_in_the_variables_dir_when_set() {
        let c = dir(&[("HOME", "/h"), ("CLAUDE_CONFIG_DIR", "/c")]).unwrap();
        assert_eq!(c.settings(), PathBuf::from("/c/settings.json"));
        assert_eq!(c.global(), PathBuf::from("/c/.claude.json"));
        assert_eq!(c.projects(), PathBuf::from("/c/projects"));
        assert_eq!(
            (c.var(), c.home()),
            (Some(Path::new("/c")), Some(Path::new("/h")))
        );
        let c = dir(&[("CLAUDE_CONFIG_DIR", "/c")]).unwrap();
        assert_eq!(c.global(), PathBuf::from("/c/.claude.json"));
        assert_eq!(c.home(), None);
    }

    #[test]
    fn claudes_config_needs_home_or_an_absolute_variable() {
        assert_eq!(dir(&[]), None);
        let vars = |k: &str| (k == "CLAUDE_CONFIG_DIR").then(|| "c".to_string());
        assert!(ConfigDir::from_vars(vars).is_err());
        let home = |k: &str| (k == "HOME").then(|| "h".to_string());
        assert!(ConfigDir::from_vars(home).is_err());
        let c = dir(&[("HOME", "h"), ("CLAUDE_CONFIG_DIR", "/c")]).unwrap();
        assert_eq!((c.dir(), c.home()), (PathBuf::from("/c"), None));
    }
}
