//! What is installed in the shell: the units it has, where their files are,
//! whether each is enabled, and how each MCP server is registered. The
//! ghost builds itself from this, however the units got there; each
//! shell's adapter implements `Installed` from the shell's own records.
//! See the initiative spec `harness-core`, "What the ghost reads".

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// What a shell has installed, as seen from a location (a project's own
/// units and settings count there).
pub trait Installed {
    /// Every unit the shell has, enabled or not.
    fn units(&self, here: Option<&Path>) -> anyhow::Result<Vec<Unit>>;

    /// Each of the shell's own records whose shape this adapter does not
    /// understand, named with why, so a shell that changed its formats is
    /// said to have, not taken to have nothing installed.
    fn not_understood(&self, _here: Option<&Path>) -> Vec<String> {
        vec![]
    }

    /// A variable of the environment the shell runs units in, for a
    /// declared path such as `${XDG_CONFIG_HOME:-$HOME/.config}/...`.
    fn var(&self, name: &str) -> Option<String> {
        std::env::var(name).ok().filter(|v| !v.is_empty())
    }
}

/// The kinds of unit a shell installs.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Skill,
    Agent,
    Mcp,
    Plugin,
}

/// One installed unit.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct Unit {
    pub kind: Kind,
    /// What the shell calls it: a skill's dir, an agent's name, an MCP
    /// server's registered name, a plugin's name.
    pub name: String,
    /// The file that carries its declaration: a skill's `SKILL.md`, an
    /// agent's file, a plugin's manifest. An MCP server has none of its
    /// own; a plugin may ship its `server.json`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest: Option<PathBuf>,
    /// The dir its files are in: a plugin's install path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dir: Option<PathBuf>,
    /// Where the shell got it, when it came through a marketplace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Origin>,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registration: Option<Registration>,
    /// A plugin's skills.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contents: Vec<Unit>,
}

/// The marketplace a plugin came from, and when and at which revision.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug, Default)]
pub struct Origin {
    pub marketplace: String,
    /// Its repo, when the marketplace is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rev: Option<String>,
    /// The date it was installed, `YYYY-MM-DD`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
}

/// How an MCP server is registered with the shell.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug, Default)]
pub struct Registration {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

impl Unit {
    pub fn new(kind: Kind, name: impl Into<String>) -> Unit {
        Unit {
            kind,
            name: name.into(),
            manifest: None,
            dir: None,
            from: None,
            enabled: true,
            registration: None,
            contents: vec![],
        }
    }
}

/// A fixed list, for a shell already read and for tests.
impl Installed for Vec<Unit> {
    fn units(&self, _: Option<&Path>) -> anyhow::Result<Vec<Unit>> {
        Ok(self.clone())
    }
}
