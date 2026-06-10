//! Tiny `sh!()` macro: parse a shell-style command string at runtime
//! via `shell_words::split` and wrap the result in `duct::cmd`.
//!
//! Use this wherever the build crate shells out to another program
//! (`cargo build`, `cp`, `tests/functional/run`). Do **not** use it
//! for filesystem state machines — those want plain `std::fs` so the
//! outcome can be a typed enum.

use anyhow::{anyhow, Result};

/// Parse a shell-style command string into a `duct::Expression`.
///
/// Returns an error if the string is empty or has unbalanced quotes.
pub fn parse_cmd(s: &str) -> Result<duct::Expression> {
    let argv = shell_words::split(s)?;
    let (prog, args) = argv
        .split_first()
        .ok_or_else(|| anyhow!("empty command string"))?;
    Ok(duct::cmd(prog, args))
}

/// `sh!("cargo build --release")` → `duct::Expression`. Parsed at
/// runtime, not compile time — same trade-off as xshell would have
/// in dynamic uses.
#[macro_export]
macro_rules! sh {
    ($s:expr) => {
        $crate::sh::parse_cmd($s)
    };
}
