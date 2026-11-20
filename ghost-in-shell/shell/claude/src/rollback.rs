//! The ghost's rollback: its profile one generation back, then its plugin
//! brought to that generation's version, with plain untouched. A
//! generation missing what Claude Code is pointed at is refused. Then the
//! revision warning, if plain and the ghost now differ. See the initiative
//! spec `harness-core`, "Installing and wiring".

use crate::install::{place, revision_warning, Setup};
use anyhow::Result;

/// `ghost-claude rollback`.
pub fn rollback(s: &Setup) -> Result<u8> {
    let to = s.profile().rollback()?;
    println!("ghost    rolled back to generation {to}");
    place(s)?;
    if let Some(w) = revision_warning(s)? {
        println!("{w}");
    }
    Ok(0)
}
