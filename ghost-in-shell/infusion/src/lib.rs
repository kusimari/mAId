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
pub use call::{Call, Carrier, Close, Ctx, Item, Slice, Where};
pub use experience::experience;
pub use expression::expression;
pub use genome::genome;
pub use sleep::sleep;

/// What the modules' own tests share.
#[cfg(test)]
pub(crate) mod testing {
    use crate::call::{Call, Carrier, Ctx};
    use crate::experience::source::{SessionSource, Sources};
    use anyhow::{bail, Result};
    use ghost::config::Config;
    use ghost::environment::Host;
    use std::path::Path;

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

    /// A context over `<dir>/parts` and `<dir>/state`, reading no session.
    pub fn ctx(dir: &Path) -> Ctx {
        Ctx {
            config: Config {
                parts: dir.join("parts"),
                state: dir.join("state"),
            },
            host: Host::default(),
            sources: Box::new(NoSources),
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
