//! Where the ghost keeps its own things. Every location is a config value
//! with a default under the user's state dir, so a binary runs from a hook
//! with no Justfile and no checkout.

use anyhow::{anyhow, Result};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// The ghost's own state (delivery record, pending decisions, what an
    /// adapter cached of the shell): `MY_HARNESS_STATE`, default
    /// `${XDG_STATE_HOME:-$HOME/.local/state}/my-harness`.
    pub state: PathBuf,
}

/// A path from the environment. An empty value is unset; a relative one is
/// a mistake, not the cwd. Public so a shell reads its own locations by
/// the same rule.
pub fn path_var(var: &impl Fn(&str) -> Option<String>, k: &str) -> Result<Option<PathBuf>> {
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
        Ok(Config { state })
    }

    /// Where a shell's adapter keeps what it derived of the shell.
    pub fn shells(&self) -> PathBuf {
        self.state.join("shells")
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
    fn the_state_sits_under_the_users_state_dir() {
        let c = from(&[("HOME", "/h")]).unwrap();
        assert_eq!(c.state, PathBuf::from("/h/.local/state/my-harness"));
        assert_eq!(
            c.shells(),
            PathBuf::from("/h/.local/state/my-harness/shells")
        );
        let c = from(&[("HOME", "/h"), ("XDG_STATE_HOME", "/x")]).unwrap();
        assert_eq!(c.state, PathBuf::from("/x/my-harness"));
        let c = from(&[("MY_HARNESS_STATE", "/s")]).unwrap();
        assert_eq!(c.state, PathBuf::from("/s"));
    }

    #[test]
    fn a_relative_location_is_refused_and_a_relative_xdg_ignored() {
        assert!(from(&[("HOME", "/h"), ("MY_HARNESS_STATE", "s")]).is_err());
        let c = from(&[("HOME", "/h"), ("XDG_STATE_HOME", "x")]).unwrap();
        assert_eq!(c.state, PathBuf::from("/h/.local/state/my-harness"));
        assert!(from(&[]).is_err());
    }
}
