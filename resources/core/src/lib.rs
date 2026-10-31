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
//!   delivery     what was sent per session, so nothing is sent twice
//!   entry        genome, expression, experience, sleep, adaptation

pub mod config;
pub mod contract;
pub mod delivery;
pub mod entry;
pub mod environment;
pub mod map;
pub mod parts;
