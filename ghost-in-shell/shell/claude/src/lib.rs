//! shell-claude — the ghost in Claude Code: it wires `ghost` and
//! `infusion` into this one shell and knows everything about it. The
//! binary (`src/main.rs`) is the CLI every carrier calls; the Claude
//! plugin the ghost installs is `plugin/`. See the initiative spec
//! `harness-core`, "System design" and "On Claude Code".
//!
//!   installed    what Claude Code has installed, from its own records, cached
//!   hook         Claude's hook and CLI input, as the call infusion takes
//!   transcript   Claude's transcripts, located and packed: its session source
//!   mcp          the on-demand carrier: an MCP server over the entry points
//!   package      the ghost's package: its plugin and executable, into a prefix
//!   reuse        the reuse check: mAId's units in Claude Code, this checkout's or not
//!   install      the ghost's install: profile, reuse or install plain, plugin, record
//!   plugins      Claude Code's own plugin commands, as the install runs them
//!   switch       on and off, at a scope
//!   status       ghost status: each bring-in as the ghost sees it
//!   uninstall    remove what its record names, its profile, the adapter's cache
//!   rollback     its profile one generation back, its plugin updated
//!   validate     every `my-harness` declaration it will read, before an install

pub mod hook;
pub mod install;
pub mod installed;
pub mod mcp;
pub mod package;
pub mod plugins;
pub mod reuse;
pub mod rollback;
pub mod status;
pub mod switch;
pub mod transcript;
pub mod uninstall;
pub mod validate;

/// Test fixtures shared by the modules' tests: the fixture session lives
/// with the tests that span crates (`ghost-in-shell/tests/fixtures`).
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
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/claude"),
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
