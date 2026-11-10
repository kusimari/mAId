//! Ghost status: the ghost's view of what Claude Code has installed. Each
//! bring-in of a part that declares itself, with its persona and whether
//! it is ready here, and each of Claude Code's records whose shape is not
//! understood. Plain's view of the same items (still there, registered,
//! runnable) is `just status`. See the initiative spec `harness-core`,
//! "Plain and ghost on Claude Code".

use crate::installed::Claude;
use anyhow::{Context, Result};
use ghost::environment::{self, Host, Readiness};
use plain::claude_config::ConfigDir;
use std::collections::BTreeSet;
use std::fmt;

/// One bring-in, and whether it is ready here.
#[derive(Debug)]
pub struct BringInLine {
    pub part: String,
    pub persona: Option<String>,
    pub name: String,
    pub readiness: Readiness,
}

/// The ghost's bring-ins on Claude Code, and the records not understood.
#[derive(Debug, Default)]
pub struct BringIns {
    pub lines: Vec<BringInLine>,
    pub not_understood: Vec<String>,
}

/// The bring-ins of each part that declares itself, as the ghost finds
/// them in what Claude Code has installed at the user's scope. A part that
/// declares nothing is plain's business, reported by `just status`.
pub fn bring_ins(claude: &ConfigDir, host: &Host) -> Result<BringIns> {
    let installed = Claude {
        config: Some(claude.clone()),
        cache: None,
        vars: None,
    };
    let ghost = ghost::Ghost::from(&installed, None).context("cannot read what Claude Code has")?;
    let declared: BTreeSet<&str> = ghost
        .personas
        .iter()
        .filter(|p| p.declared)
        .map(|p| p.part.as_str())
        .collect();
    let lines = ghost
        .bring_ins
        .iter()
        .filter(|b| declared.contains(b.part.as_str()))
        .map(|b| BringInLine {
            part: b.part.clone(),
            persona: b.persona.clone(),
            name: b.name.clone(),
            readiness: environment::readiness(b, host),
        })
        .collect();
    Ok(BringIns {
        lines,
        not_understood: ghost.not_understood.clone(),
    })
}

impl fmt::Display for BringIns {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut part = None;
        for b in &self.lines {
            if part.is_none() {
                writeln!(f, "bring-ins")?;
            }
            if part != Some(&b.part) {
                match &b.persona {
                    Some(p) => writeln!(f, "  {} ({p})", b.part)?,
                    None => writeln!(f, "  {}", b.part)?,
                }
                part = Some(&b.part);
            }
            write!(f, "    {:<14} {}", b.readiness.label(), b.name)?;
            match b.readiness.why() {
                Some(why) => writeln!(f, " ({why})")?,
                None => writeln!(f)?,
            }
        }
        for n in &self.not_understood {
            writeln!(f, "not understood  {n}")?;
        }
        Ok(())
    }
}

/// `ghost-claude status`: the ghost's profile and generation; its plugin,
/// on or off here and at which scope; mAId's plugin against the one the
/// ghost was installed with; each bring-in as the ghost sees it; each
/// record not understood; the decisions and sessions waiting.
pub fn status(s: &crate::install::Setup, here: &std::path::Path) -> Result<u8> {
    use crate::package::{version_in, PLUGIN_ID};
    println!("{}", s.profile().status());
    match version_in(&s.profile) {
        Some(v) => {
            let (on, scope) = crate::switch::state(&s.claude, here);
            let at = scope.map_or("no scope names it".into(), |sc| format!("at {sc}"));
            let on = if on { "on" } else { "off" };
            println!("plugin   {PLUGIN_ID} {v}: {on} here ({at})");
        }
        None => println!("plugin   {PLUGIN_ID} not installed (run: just ghost install)"),
    }
    let plain = crate::reuse::plain_plugin(&s.installed())?;
    match plain.as_ref().map(|u| (u, crate::reuse::version_of(u))) {
        Some((u, v)) => {
            let v = v.unwrap_or_else(|| "no version".into());
            match u.enabled {
                true => println!("plain    mAId's plugin {v}"),
                false => println!("plain    {} ({v})", crate::install::DISABLED),
            }
        }
        None => println!("plain    mAId's plugin not installed"),
    }
    if let Some(w) = crate::install::revision_warning(s)? {
        println!("{w}");
    }
    print!("{}", bring_ins(&s.claude, &Host::from_env())?);
    let config = ghost::config::Config {
        state: s.state.clone(),
    };
    let pending = infusion::genome::pending_in(&config)?;
    let waiting = infusion::sleep::waiting(&config)?.len();
    println!("pending  {} decision(s) at {}", pending.count, pending.at);
    println!("sleeping {waiting} session(s) waiting to be reflected on");
    Ok(0)
}
