//! The call every entry point takes, and what it answers with. One shape
//! for every carrier (a hook, a framework adapter, an MCP tool or
//! resource, a skill or a CLI), so the entry points answer each the same
//! way. Nothing here is a shell's: the shell turns its own input into
//! this (`shell-claude`'s `hook`).

use crate::expression::delivery;
use ghost::config::Config;
use ghost::environment::Host;
use ghost::installed::Installed;
use ghost::Ghost;
use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;

/// How a carrier keeps what it is given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Carrier {
    /// Kept in the session's history: send only what changed.
    Append,
    /// Sent with one call only: send everything every time.
    Transient,
    /// Asked for: return everything, and record it as sent.
    OnDemand,
}

/// How much of the genome a session is given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Slice {
    /// All of it.
    #[default]
    Whole,
    /// A sub-agent's: who and what, without what waits for the person
    /// (the environment's detail and pending decisions).
    SubSession,
}

/// Where a call is: the folder it runs in, and which source holds the
/// session and where, as far as the carrier knows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Where {
    /// The session source, by name; its shell's own when none.
    pub source: Option<String>,
    /// The file the source holds the session in.
    pub transcript: Option<PathBuf>,
    /// The folder the session works in.
    pub cwd: Option<PathBuf>,
}

/// Why a session is done: it ended, or the person said so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Close {
    /// The session ended.
    End,
    /// The person said so: a close, a reflect.
    Named(String),
}

/// What one carrier asks of one entry point.
#[derive(Debug, Clone)]
pub struct Call {
    /// The session, or sub-session, this call is for. Without one nothing
    /// can be remembered, so everything is answered every time.
    pub session: Option<String>,
    /// How this carrier keeps what it is given, which decides what it is
    /// sent again.
    pub carrier: Carrier,
    /// Forget what the session was sent before answering: after a
    /// compaction, a clear or a resume.
    pub reset: bool,
    /// How much of the genome a session needs.
    pub slice: Slice,
    /// Where this call is: the folder, and the session's own file.
    pub at: Where,
    /// What this turn is about.
    pub prompt: Option<String>,
    /// That the session is done, and why.
    pub close: Option<Close>,
    /// The person's own input, as they gave it: the reflector's findings
    /// and the person's decisions (`adaptation`).
    pub input: Value,
}

impl Default for Call {
    fn default() -> Call {
        Call {
            session: None,
            carrier: Carrier::OnDemand,
            reset: false,
            slice: Slice::Whole,
            at: Where::default(),
            prompt: None,
            close: None,
            input: Value::Null,
        }
    }
}

/// A sub-session's id: `<session>.<agent>`, or `<session>.a-<hash>` when
/// that would not be a plain id. A carrier that runs sub-agents names
/// them its own way; the id a record can hold is this.
pub fn sub_session(session: &str, agent: &str) -> String {
    let joined = format!("{session}.{agent}");
    match delivery::check_id("session id", &joined) {
        Ok(()) => joined,
        Err(_) => format!("{session}.a-{:016x}", delivery::fnv1a(agent)),
    }
}

/// One thing to place, as text and as data.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Item {
    pub id: String,
    pub text: String,
    pub data: Value,
}

/// The infusion: what every entry point infuses the ghost through. The
/// shell the ghost is in, where the ghost's own things are, what this host
/// offers, and the sessions this shell can read. See the initiative spec
/// `harness-core`, "The five entry points".
pub struct Infusion {
    pub config: Config,
    pub host: Host,
    pub sources: Box<dyn crate::experience::source::Sources>,
    /// The shell the ghost is in, which says what is installed.
    pub shell: Box<dyn Shell>,
}

/// The shell the ghost is infused into: the ghost as it is there now.
/// Every shell whose adapter says what it has `Installed` is one.
pub trait Shell {
    fn ghost(&self, here: Option<&std::path::Path>) -> anyhow::Result<Ghost>;
}

impl<T: Installed> Shell for T {
    fn ghost(&self, here: Option<&std::path::Path>) -> anyhow::Result<Ghost> {
        Ghost::from(self, here)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sub_sessions_id_is_its_agents_name_or_a_hash_of_it() {
        assert_eq!(sub_session("s1", "a7"), "s1.a7");
        let long = sub_session("s1", &"x/".repeat(80));
        assert!(
            long.starts_with("s1.a-") && delivery::check_id("id", &long).is_ok(),
            "{long}"
        );
    }
}
