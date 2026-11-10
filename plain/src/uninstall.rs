//! Uninstall: remove what install placed, and drop from the record what
//! is gone. See project.md "My harness" (the install record).

use crate::install::{record_placed, warn_unrecorded};
use crate::place::{outcome, Deploy};
use crate::record::Dirs;
use crate::shared::Agent;
use anyhow::Result;

pub fn cmd_uninstall(
    target: &impl Deploy,
    parts: &(impl Dirs + ?Sized),
    dry_run: bool,
    force: bool,
    agent: Option<Agent>,
) -> Result<u8> {
    let done = target.uninstall(agent, dry_run, force);
    // What uninstall did not reach is still there; a skip is not.
    let recorded = match dry_run {
        true => Ok(()),
        false => record_placed(target, parts, agent, |i| i.skipped.is_none()),
    };
    let rc = outcome(
        done.inspect_err(|_| warn_unrecorded(&recorded))?,
        dry_run,
        true,
    )?;
    recorded.map(|()| rc)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::install::tests::{content, installable, write};
    use crate::install::{cmd_install, cmd_record};
    use crate::record::{self, What};
    use std::fs;

    #[test]
    fn uninstall_drops_a_skip_but_keeps_what_is_still_there() {
        let (dir, target, parts) = installable();
        cmd_record(
            &parts,
            Agent::Kiro,
            What::Mcp,
            Some("s"),
            None,
            None,
            &["m".into()],
        )
        .unwrap();
        // agy's skills dir is the user's, so install skips it.
        write(&dir.path().join("home/.gemini/config/skills/x"), "mine");
        cmd_install(&target, &content(&dir), &parts, false, false, None).unwrap();
        assert!(fs::read_to_string(record::path(&parts))
            .unwrap()
            .contains("skipped"));
        cmd_uninstall(&target, &parts, false, false, None).unwrap();
        let text = fs::read_to_string(record::path(&parts)).unwrap();
        assert!(
            !text.contains("skipped") && !text.contains("notes"),
            "{text}"
        );
        assert!(text.contains(r#""name": "m""#), "{text}");
    }
}
