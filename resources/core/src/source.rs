//! Session sources: where a session can be read from (a shell's
//! transcript, a framework's own session store, traces), behind one
//! interface, so experience works across shells. Only Claude Code's is
//! built; any other name says so.

use crate::claude::Claude;
use crate::config::Config;
use crate::experience::Bundle;
use anyhow::{anyhow, bail, Result};
use serde_json::Value;
use std::path::PathBuf;

/// The source every call uses unless its input names another.
pub const DEFAULT: &str = "claude-code";

/// A session found in a source: its id and the files that hold it.
#[derive(Debug, Clone, PartialEq)]
pub struct Located {
    pub source: String,
    pub session: String,
    pub transcript: PathBuf,
    /// Its sub-sessions' own files.
    pub subs: Vec<PathBuf>,
}

pub trait SessionSource {
    fn name(&self) -> &'static str;
    /// Find a session from its id, the carrier's input, or both.
    fn locate(&self, session: Option<&str>, input: &Value) -> Result<Located>;
    /// The whole session and its sub-sessions, every text in full.
    fn pack(&self, at: &Located) -> Result<Bundle>;
}

/// The source the input names (`"source"`), else the default.
pub fn named(config: &Config, input: &Value) -> Result<Box<dyn SessionSource>> {
    let name = input["source"].as_str().unwrap_or(DEFAULT);
    match name {
        DEFAULT => {
            let root = config
                .claude
                .clone()
                .ok_or_else(|| anyhow!("no Claude Code dir: HOME and CLAUDE_CONFIG_DIR unset"))?;
            Ok(Box::new(Claude { root }))
        }
        other => bail!("no session source for {other} yet"),
    }
}
