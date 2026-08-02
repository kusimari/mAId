//! build-tool — mAId's tooling for taking checked-in markdown resources
//! all the way to verified agent behavior. The crate is organised as the
//! pipeline it performs:
//!
//! ```text
//! resources/content/ ─▶ a valid skill ─▶ checked in isolation ─▶ $HOME ─▶ smoke-tested deployed
//!                       (1 content)      (2 check · pre-install)  (3 install)  (4 smoke · post-install)
//! ```
//!
//! Four files, one per category, each carrying its internal structure in
//! section comments rather than a directory:
//!
//!   main.rs     the shim: clap Cli/Cmd + dispatch, nothing else
//!   shared.rs   vocabulary every stage speaks (agents, registry, roots)
//!   harness.rs  driving a coding agent + scoring the reply; used by
//!               stages 2 and 4, owned by neither
//!   stages.rs   the pipeline, one section per stage
//!
//! Dependencies run strictly one direction — `stages` → `harness` →
//! `shared` → nothing — which is readable off the `use` block at the top
//! of each file.
//!
//! The library target exists so the stages are unit-testable and so
//! cross-stage tests in `tests/` can exercise the crate from outside.

pub mod shared;
