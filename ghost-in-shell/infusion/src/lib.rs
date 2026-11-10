//! infusion — the five entry points, one file each: `genome`,
//! `expression`, `experience`, `sleep`, `adaptation`. Each answers the
//! same way for any carrier (a hook, a framework adapter, an MCP tool or
//! resource, a skill or a CLI), and nothing here knows a shell: a shell
//! hands in the call and its session sources (`call`). What forms the
//! ghost is `ghost`. See the initiative spec `harness-core`, "The five
//! entry points".
//!
//!   call         the call every entry point takes, and the items it answers with
//!   genome       who the session is, at its start
//!   expression   what this turn needs: fit, recall, the work's record, delivery
//!   experience   a session packed: said, told, did, touched, friction
//!   sleep        marks: sessions waiting to be reflected on
//!   adaptation   findings, guards, the decision batch, routes, the ledger
//!   files        writing the ghost's own files whole

pub mod adaptation;
pub mod call;
pub mod experience;
pub mod expression;
pub mod files;
pub mod genome;
pub mod sleep;

pub use adaptation::adaptation;
pub use call::{Call, Carrier, Close, Infusion, Item, Shell, Slice, Where};
pub use experience::experience;
pub use expression::expression;
pub use genome::genome;
pub use sleep::sleep;

/// What the modules' own tests share.
#[cfg(test)]
pub(crate) mod testing {
    use crate::call::{Call, Carrier, Infusion, Shell};
    use crate::experience::source::{SessionSource, Sources};
    use anyhow::{bail, Result};
    use ghost::config::Config;
    use ghost::environment::Host;
    use ghost::parts::Part;
    use ghost::Ghost;
    use std::fs;
    use std::path::{Path, PathBuf};

    /// A shell whose ghost is the parts written as JSON in a dir, one
    /// `<part>.json` each, so a test says the parts outright.
    pub struct Parts(pub PathBuf);

    impl Shell for Parts {
        fn ghost(&self, _: Option<&Path>) -> Result<Ghost> {
            Ok(ghost_of(&self.0))
        }
    }

    /// The ghost of the parts written in `dir`.
    pub fn ghost_of(dir: &Path) -> Ghost {
        let mut files: Vec<PathBuf> = fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        files.sort();
        let parts: Vec<Part> = files
            .iter()
            .filter_map(|p| serde_json::from_str(&fs::read_to_string(p).ok()?).ok())
            .filter_map(|v| Part::from_json(&v).ok())
            .collect();
        Ghost::of(&parts)
    }

    /// A shell with no session source: what a test that reads no session
    /// hands in. Reading one says so rather than reaching for a file.
    pub struct NoSources;

    impl Sources for NoSources {
        fn named(&self, name: Option<&str>) -> Result<Box<dyn SessionSource>> {
            bail!(
                "no session source for {} here",
                name.unwrap_or("this shell")
            )
        }
    }

    /// A context whose ghost is the parts in `<dir>/parts`, its state in
    /// `<dir>/state`, reading no session.
    pub fn ctx(dir: &Path) -> Infusion {
        Infusion {
            config: Config {
                state: dir.join("state"),
            },
            host: Host::default(),
            sources: Box::new(NoSources),
            shell: Box::new(Parts(dir.join("parts"))),
        }
    }

    /// A call for `session` on `carrier`.
    pub fn call(session: Option<&str>, carrier: Carrier, reset: bool) -> Call {
        Call {
            session: session.map(str::to_string),
            carrier,
            reset,
            ..Call::default()
        }
    }
}
