//! Where the ghost keeps its own things. Every location is a config value
//! with a default under the user's state dir, so a binary runs from a hook
//! with no Justfile and no checkout.

use crate::old_name;
use anyhow::{anyhow, Result};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// The ghost's own state (delivery record, pending decisions, what an
    /// adapter cached of the shell): `GHOST_IN_SHELL_STATE`, default
    /// `${XDG_STATE_HOME:-$HOME/.local/state}/ghost-in-shell`.
    pub state: PathBuf,
    /// Whether the prefix casts a wide net for candidates (D1): on, unless
    /// `CANDIDATES` is `off`, which leaves memories matched on the prompt's
    /// own words and no near fits.
    pub candidates: bool,
    /// Each old name this was read from, said in a line.
    pub old_names: Vec<String>,
}

/// Where the ghost's own state is.
pub const STATE: &str = "GHOST_IN_SHELL_STATE";
/// The switch for the prefix's candidates.
pub const CANDIDATES: &str = "GHOST_IN_SHELL_CANDIDATES";

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
        let mut old_names = vec![];
        let state = match get(old_name::var(&var, STATE, old_name::STATE, &mut old_names))? {
            Some(s) => s,
            None => {
                let base = match get("XDG_STATE_HOME") {
                    Ok(Some(b)) => b,
                    // The XDG spec says to ignore a relative value.
                    _ => get("HOME")?
                        .ok_or_else(|| anyhow!("HOME is not set"))?
                        .join(".local/state"),
                };
                old_name::state_dir(&base, &mut old_names)
            }
        };
        let switch = old_name::var(&var, CANDIDATES, old_name::CANDIDATES, &mut old_names);
        let candidates = var(switch).as_deref() != Some("off");
        Ok(Config {
            state,
            candidates,
            old_names,
        })
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
        assert_eq!(c.state, PathBuf::from("/h/.local/state/ghost-in-shell"));
        assert_eq!(
            c.shells(),
            PathBuf::from("/h/.local/state/ghost-in-shell/shells")
        );
        let c = from(&[("HOME", "/h"), ("XDG_STATE_HOME", "/x")]).unwrap();
        assert_eq!(c.state, PathBuf::from("/x/ghost-in-shell"));
        let c = from(&[(STATE, "/s")]).unwrap();
        assert_eq!(c.state, PathBuf::from("/s"));
        assert!(c.old_names.is_empty());
    }

    #[test]
    fn an_old_variable_is_read_when_the_new_one_is_unset_and_said() {
        let c = from(&[("HOME", "/h"), (old_name::STATE, "/old")]).unwrap();
        assert_eq!(c.state, PathBuf::from("/old"));
        assert_eq!(
            c.old_names,
            ["MY_HARNESS_STATE is the old name of GHOST_IN_SHELL_STATE; read for this release only"]
        );
        let c = from(&[(STATE, ""), (old_name::STATE, "/old")]).unwrap();
        assert_eq!(c.state, PathBuf::from("/old"), "an empty value is unset");
        let c = from(&[(STATE, "/new"), (old_name::STATE, "/old")]).unwrap();
        assert_eq!((c.state, c.old_names), (PathBuf::from("/new"), vec![]));
        let c = from(&[("HOME", "/h"), (old_name::CANDIDATES, "off")]).unwrap();
        assert!(!c.candidates);
        assert_eq!(c.old_names.len(), 1);
        let c = from(&[
            ("HOME", "/h"),
            (CANDIDATES, "on"),
            (old_name::CANDIDATES, "off"),
        ])
        .unwrap();
        assert!(c.candidates && c.old_names.is_empty());
    }

    #[test]
    fn the_old_state_dir_is_used_where_it_is_until_the_new_one_is_there() {
        let dir = tempfile::TempDir::new().unwrap();
        let base = dir.path().to_str().unwrap();
        let at = |n: &str| dir.path().join(n);
        std::fs::create_dir_all(at("my-harness/parts")).unwrap();
        let c = from(&[("HOME", "/h"), ("XDG_STATE_HOME", base)]).unwrap();
        assert_eq!(
            (c.state, c.old_names),
            (at("ghost-in-shell"), vec![]),
            "plain's old part lists alone are no ghost's state"
        );
        std::fs::create_dir(at("my-harness/pending")).unwrap();
        let c = from(&[("HOME", "/h"), ("XDG_STATE_HOME", base)]).unwrap();
        assert_eq!(c.state, at("my-harness"));
        assert_eq!(c.old_names.len(), 1);
        assert!(
            c.old_names[0].contains("move all of it but parts/ to"),
            "{:?}",
            c.old_names
        );
        std::fs::create_dir(at("ghost-in-shell")).unwrap();
        let c = from(&[("HOME", "/h"), ("XDG_STATE_HOME", base)]).unwrap();
        assert_eq!((c.state, c.old_names), (at("ghost-in-shell"), vec![]));
    }

    #[test]
    fn a_relative_location_is_refused_and_a_relative_xdg_ignored() {
        assert!(from(&[("HOME", "/h"), (STATE, "s")]).is_err());
        let c = from(&[("HOME", "/h"), ("XDG_STATE_HOME", "x")]).unwrap();
        assert_eq!(c.state, PathBuf::from("/h/.local/state/ghost-in-shell"));
        assert!(from(&[]).is_err());
    }

    #[test]
    fn candidates_are_on_unless_switched_off() {
        assert!(from(&[("HOME", "/h")]).unwrap().candidates);
        assert!(
            from(&[("HOME", "/h"), (CANDIDATES, "on")])
                .unwrap()
                .candidates
        );
        assert!(
            !from(&[("HOME", "/h"), (CANDIDATES, "off")])
                .unwrap()
                .candidates
        );
    }
}
