//! Sleep marks: a session whose outcome is done, from its end event or
//! from the person's close or reflect, waiting to be reflected on. One
//! file per session under `<state>/sleep/`, so nothing depends on an end
//! event firing: a close marks the session the same way.

use crate::config::Config;
use crate::delivery::check_id;
use crate::files::write_atomic;
use crate::source::Located;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Mark {
    pub session: String,
    pub source: String,
    pub transcript: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Why it was marked, each time: `end`, `close`, `reflect`, ...
    pub reasons: Vec<String>,
    pub at: String,
    /// Every decision of its batch is made, or the person dismissed it
    /// (the ledger says which).
    #[serde(default)]
    pub decided: bool,
}

fn path(config: &Config, session: &str) -> Result<PathBuf> {
    check_id("session id", session)?;
    Ok(config.sleep().join(format!("{session}.json")))
}

pub fn read(config: &Config, session: &str) -> Result<Option<Mark>> {
    let p = path(config, session)?;
    match fs::read_to_string(&p) {
        Ok(t) => {
            Ok(Some(serde_json::from_str(&t).with_context(|| {
                format!("{} is not a mark", p.display())
            })?))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("cannot read {}", p.display())),
    }
}

fn write(config: &Config, m: &Mark) -> Result<()> {
    let p = path(config, &m.session)?;
    write_atomic(&p, serde_json::to_string_pretty(m)?.as_bytes())
        .with_context(|| format!("cannot write {}", p.display()))
}

/// The reason an end event gives.
pub const END: &str = "end";

/// Mark a session. The first mark's source stays; a later one adds its
/// reason and opens the session again, since there is more to reflect on,
/// except the end event after the person reflected and decided: closing
/// is the usual last thing a session does.
pub fn mark(config: &Config, at: &Located, cwd: Option<&str>, reason: &str) -> Result<Mark> {
    let m = match read(config, &at.session)? {
        Some(mut m) => {
            m.reasons.push(reason.into());
            if reason != END {
                m.decided = false;
            }
            m
        }
        None => Mark {
            session: at.session.clone(),
            source: at.source.clone(),
            transcript: at.transcript.display().to_string(),
            cwd: cwd.map(str::to_string),
            reasons: vec![reason.into()],
            at: crate::time::now(),
            decided: false,
        },
    };
    write(config, &m)?;
    Ok(m)
}

pub fn set_decided(config: &Config, session: &str) -> Result<()> {
    if let Some(mut m) = read(config, session)? {
        m.decided = true;
        write(config, &m)?;
    }
    Ok(())
}

/// Marked sessions not yet decided, oldest first.
pub fn waiting(config: &Config) -> Result<Vec<Mark>> {
    let dir = config.sleep();
    let rd = match fs::read_dir(&dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e).with_context(|| format!("cannot list {}", dir.display())),
    };
    let mut out = vec![];
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some(session) = name.strip_suffix(".json") else {
            continue;
        };
        if session.starts_with('.') {
            continue;
        }
        // A mark that cannot be read is left out, not fatal: it only
        // counts what waits.
        if let Ok(Some(m)) = read(config, session) {
            if !m.decided {
                out.push(m);
            }
        }
    }
    out.sort_by(|a, b| (&a.at, &a.session).cmp(&(&b.at, &b.session)));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn config(dir: &std::path::Path) -> Config {
        Config {
            parts: dir.join("parts"),
            state: dir.join("state"),
            claude: None,
        }
    }

    fn located(session: &str) -> Located {
        Located {
            source: "claude-code".into(),
            session: session.into(),
            transcript: format!("/t/{session}.jsonl").into(),
            subs: vec![],
        }
    }

    #[test]
    fn a_mark_is_kept_once_and_later_reasons_are_added() {
        let d = TempDir::new().unwrap();
        let c = config(d.path());
        mark(&c, &located("s1"), Some("/work"), "reflect").unwrap();
        let mut other = located("s1");
        other.transcript = "/elsewhere.jsonl".into();
        let m = mark(&c, &other, None, "end").unwrap();
        assert_eq!(m.reasons, ["reflect", "end"]);
        assert_eq!(m.transcript, "/t/s1.jsonl");
        assert_eq!(m.cwd.as_deref(), Some("/work"));
    }

    #[test]
    fn waiting_holds_marks_not_decided_and_a_new_mark_opens_one_again() {
        let d = TempDir::new().unwrap();
        let c = config(d.path());
        mark(&c, &located("s1"), None, "end").unwrap();
        mark(&c, &located("s2"), None, "close").unwrap();
        set_decided(&c, "s1").unwrap();
        let names = |c: &Config| -> Vec<String> {
            waiting(c).unwrap().into_iter().map(|m| m.session).collect()
        };
        assert_eq!(names(&c), ["s2"]);
        mark(&c, &located("s1"), None, "reflect").unwrap();
        assert_eq!(names(&c).len(), 2);
    }

    #[test]
    fn the_end_after_a_decided_reflection_keeps_it_decided() {
        let d = TempDir::new().unwrap();
        let c = config(d.path());
        mark(&c, &located("s1"), None, "close").unwrap();
        set_decided(&c, "s1").unwrap();
        let m = mark(&c, &located("s1"), None, END).unwrap();
        assert!(m.decided);
        assert_eq!(m.reasons, ["close", "end"]);
        assert!(waiting(&c).unwrap().is_empty());
    }

    #[test]
    fn a_session_id_that_could_leave_the_dir_is_refused() {
        let d = TempDir::new().unwrap();
        assert!(mark(&config(d.path()), &located("../x"), None, "end").is_err());
        assert!(!d.path().join("x.json").exists());
    }
}
