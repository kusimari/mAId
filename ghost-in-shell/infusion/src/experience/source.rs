//! Session sources: where a session can be read from (a shell's
//! transcript, a framework's own session store, traces), behind one
//! interface, so experience works across shells. Which sources there are
//! is the shell's own: it hands them in with the call's context.

use crate::call::Where;
use crate::experience::Bundle;
use anyhow::Result;
use std::path::PathBuf;

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
    /// Find a session from its id, where the call says it is, or both.
    fn locate(&self, session: Option<&str>, at: &Where) -> Result<Located>;
    /// The whole session and its sub-sessions, every text in full.
    fn pack(&self, at: &Located) -> Result<Bundle>;
}

/// The session sources a shell offers.
pub trait Sources {
    /// The one `name` names, or the shell's own when none is named. A
    /// name it does not serve is an error, so nothing is read from the
    /// wrong place.
    fn named(&self, name: Option<&str>) -> Result<Box<dyn SessionSource>>;
}
