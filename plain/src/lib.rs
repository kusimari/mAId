//! plain: the plain install. It puts the person's resources (skills,
//! agents, servers, plugins) into each base harness, and is the one place
//! that manages what is installed. It knows nothing of the ghost. See the
//! initiative spec `harness-core`, "Plain, then the ghost", and project.md
//! "My harness".
//!
//! The modules, as an install runs:
//!
//! ```text
//! resources/ ─▶ validate ─▶ stage ─▶ profile ─▶ install ─▶ place (each agent) ─▶ record
//!                                                 uninstall ◀─┘  status
//! ```
//!
//!   validate.rs  each resource is well formed before anything is placed
//!   stage.rs     the package: built from resources/ into a prefix, its version
//!   profile.rs   where the package lives: nix generations, or a dir
//!   install.rs   place on each agent, then record what was placed
//!   place.rs     how a unit reaches each agent: plugins, links, vendors
//!   external.rs  the pinned pieces from other repos: the vendor plugins, pensieve
//!   record.rs    the install record: what a package placed, its one writer
//!   uninstall.rs remove what install placed, as the record names it
//!   status.rs    what is placed, and every package's record checked
//!   claude_config.rs  where Claude Code keeps its user config
//!   memory.rs    the personal store, its server on claude, Claude's memory off
//!   reads.rs     Claude's grants on mAId's skill files and skill state
//!   settings.rs  Claude's user settings file, kept key for key
//!   harvest.rs   copying each harness's own memory into a store
//!   check.rs     the skill checks before and after install
//!   drive.rs     driving an agent and scoring its reply, for check
//!   shared.rs    what every module speaks: agents, the registry, roots
//!   time.rs      dates, for when a package was first installed
//!
//! The library target exists so each module is unit-testable apart from
//! the binary, and so the tests under `tests/` can use the crate.

pub mod check;
pub mod claude_config;
pub mod drive;
pub mod external;
pub mod harvest;
pub mod install;
pub mod memory;
pub mod place;
pub mod profile;
pub mod reads;
pub mod record;
pub mod settings;
pub mod shared;
pub mod stage;
pub mod status;
pub mod time;
pub mod uninstall;
pub mod validate;
