//! Where the core reads and writes. Every location is a config value with a
//! default under the user's state dir, so the binary runs from a hook with
//! no Justfile and no checkout.

use anyhow::{anyhow, Result};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Every part's list: `MY_HARNESS_PARTS`, default `<state>/parts`.
    pub parts: PathBuf,
    /// The core's own state (delivery record, pending decisions):
    /// `MY_HARNESS_STATE`, default `${XDG_STATE_HOME:-$HOME/.local/state}/my-harness`.
    pub state: PathBuf,
    /// Claude Code's transcripts: `$CLAUDE_CONFIG_DIR/projects`, default
    /// `$HOME/.claude/projects` (Claude Code's own setting, not ours).
    pub claude: Option<PathBuf>,
}

impl Config {
    pub fn from_env() -> Result<Config> {
        Config::from_vars(|k| std::env::var(k).ok())
    }

    /// The same, from any lookup, so tests never read the real environment.
    pub fn from_vars(var: impl Fn(&str) -> Option<String>) -> Result<Config> {
        // An empty value is unset; a relative one is a mistake, not the cwd.
        let get = |k: &str| -> Result<Option<PathBuf>> {
            match var(k).filter(|v| !v.is_empty()) {
                None => Ok(None),
                Some(v) => {
                    let p = PathBuf::from(&v);
                    p.is_absolute()
                        .then_some(Some(p))
                        .ok_or_else(|| anyhow!("{k} must be an absolute path (got {v:?})"))
                }
            }
        };
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
        let claude = match get("CLAUDE_CONFIG_DIR")? {
            Some(c) => Some(c),
            None => get("HOME")?.map(|h| h.join(".claude")),
        }
        .map(|c| c.join("projects"));
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
