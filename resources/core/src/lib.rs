//! maid-core — the core of my harness: it reads every part present, fuses
//! their personas and bring-ins into one harness map, derives what of it
//! is ready where the session runs, and answers the five entry points the
//! same way for a hook, an adapter or an on-demand call. See project.md
//! "My harness".
//!
//! Dependencies run down this list:
//!
//!   config       where things are: the parts dir and the state dir
//!   contract     what a bring-in declares, read from its standard's slot
//!   parts        what each part records about its persona and bring-ins
//!   map          fusion: every part's personas and bring-ins, one map
//!   environment  the map as resolved here: bound, reachable, ready
//!   here         where a session is: its folder, repo root, branch, files
//!   record       the work's own record: goal, state, next step
//!   fit          turn fit and the persona guess
//!   select       recall: the few memories that bear on a turn
//!   delivery     what was sent per session, so nothing is sent twice
//!   files        writing the core's own files whole
//!   experience   a session packed: said, told, did, touched, friction
//!   source       where sessions are read from; `claude` is Claude Code's
//!   sleep        marks: sessions waiting to be reflected on
//!   time         UTC stamps
//!   memory       a persona's memory, written through its own server
//!   adapt        findings, guards, the decision batch, routes
//!   ledger       what reflections saw and the person decided
//!   entry        genome, expression, experience, sleep, adaptation
//!   mcp          the on-demand carrier: an MCP server over the entry points

pub mod adapt;
pub mod claude;
pub mod config;
pub mod contract;
pub mod delivery;
pub mod entry;
pub mod environment;
pub mod experience;
pub mod files;
pub mod fit;
pub mod here;
pub mod ledger;
pub mod map;
pub mod mcp;
pub mod memory;
pub mod parts;
pub mod record;
pub mod select;
pub mod sleep;
pub mod source;
pub mod time;

/// Test fixtures shared by the modules' tests.
#[cfg(test)]
pub(crate) mod fixtures {
    use std::fs;
    use std::path::{Path, PathBuf};

    /// The fixture Claude Code session (`tests/fixtures/claude/-work`).
    pub const SESSION: &str = "0b7c6a8e-1111-4222-8333-944455556666";

    /// A copy of the fixture projects dir at `<dir>/claude`.
    pub fn claude_projects(dir: &Path) -> PathBuf {
        let to = dir.join("claude");
        copy(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude"),
            &to,
        );
        to
    }

    fn copy(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for e in fs::read_dir(from).unwrap().flatten() {
            let t = to.join(e.file_name());
            match e.file_type().unwrap().is_dir() {
                true => copy(&e.path(), &t),
                false => {
                    fs::copy(e.path(), t).unwrap();
                }
            }
        }
    }
}
