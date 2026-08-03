//! build-tool — mAId's tooling for taking checked-in markdown resources
//! all the way to verified agent behavior. The crate is organised as the
//! pipeline it performs:
//!
//! ```text
//! resources/content/ ─▶ a valid skill ─▶ checked in isolation ─▶ $HOME ─▶ smoke-tested deployed
//!                       (content)        (check · pre-install)   (install)  (smoke · post-install)
//! ```
//!
//! One file per category, each carrying its internal structure in
//! section comments rather than a directory. In place today:
//!
//!   main.rs     the shim: clap surface + dispatch
//!   shared.rs   vocabulary every stage speaks (agents, registry, roots)
//!   stages.rs   the pipeline: content, then install
//!
//! Dependencies run one direction — later stages may read earlier ones
//! and the shared vocabulary, never the reverse — which is readable off
//! the `use` block at the top of each file.
//!
//! The library target exists so the stages are unit-testable
//! independently of the binary, and so cross-stage tests under `tests/`
//! can exercise the crate from outside.

pub mod shared;
pub mod stages;
