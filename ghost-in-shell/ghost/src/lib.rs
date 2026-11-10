//! ghost — what forms the ghost: every part present, its persona and its
//! bring-ins, fused into one ghost; what of it is ready where the session
//! runs; and where the ghost keeps its own things. It knows no shell and
//! no entry point. See the initiative spec `harness-core`, "What forms
//! the ghost".
//!
//! Dependencies run down this list:
//!
//!   config       where the ghost keeps its own things
//!   contract     what a bring-in declares, read from its standard's slot
//!   installed    what the shell has installed: the trait each adapter implements
//!   binding      a declared config key's value, from how the shell registered it
//!   parts        each repo's persona and bring-ins, found in what is installed
//!   ghost        fusion: every part's personas and bring-ins, one ghost
//!   environment  the ghost as resolved here: bound, reachable, ready
//!   time         UTC stamps

pub mod binding;
pub mod config;
pub mod contract;
pub mod environment;
pub mod ghost;
pub mod installed;
pub mod parts;
pub mod time;

pub use ghost::{BringIn, Declared, Ghost, Persona};
