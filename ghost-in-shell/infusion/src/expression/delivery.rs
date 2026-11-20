//! The delivery record: what was sent to each session, so a hook and an
//! on-demand call never send the same thing twice. One empty file per sent
//! item, `<dir>/<session>/<item>@<hash>`, made with `create_new`, so when
//! two carriers race with the same content exactly one of them sends, with
//! no lock.
//!
//! An item is recorded when it is handed to the carrier, before the carrier
//! places it: delivery is at most once, and a carrier that fails to place
//! what it was given calls again with reset.
//!
//! Beside the marks, a session may keep a small state file of an entry
//! point's own (`Drift` keeps when the essentials were last placed). Its
//! name holds no `@`, so it is never read as a mark, and a reset clears it.
//! See the initiative spec `harness-core`, "What forms the ghost"
//! (delivery record).

use crate::call::{Call, Carrier, Infusion, Item};
use anyhow::{bail, Result};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub struct Record {
    dir: PathBuf,
}

impl Record {
    pub fn new(dir: impl Into<PathBuf>) -> Record {
        Record { dir: dir.into() }
    }

    /// Whether this call should send `text` as `item` to `session`; when it
    /// should, the send is recorded. Content sent before and since replaced
    /// is sent again, since the session last saw the replacement.
    pub fn claim(&self, session: &str, item: &str, text: &str) -> Result<bool> {
        let dir = self.session(session)?;
        check_id("item", item)?;
        fs::create_dir_all(&dir)?;
        let mark = format!("{item}@{:016x}", fnv1a(text));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dir.join(&mark))
        {
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return Ok(false),
            Err(e) => return Err(e.into()),
        }
        let older = format!("{item}@");
        for e in fs::read_dir(&dir)?.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with(&older) && name != mark {
                remove(&e.path())?;
            }
        }
        Ok(true)
    }

    /// Whether `session` already holds `text` as `item`, without claiming.
    pub fn holds(&self, session: &str, item: &str, text: &str) -> Result<bool> {
        let dir = self.session(session)?;
        check_id("item", item)?;
        Ok(dir.join(format!("{item}@{:016x}", fnv1a(text))).exists())
    }

    /// Whether `session` was sent any version of `item`.
    pub fn sent_any(&self, session: &str, item: &str) -> Result<bool> {
        let dir = self.session(session)?;
        check_id("item", item)?;
        let prefix = format!("{item}@");
        Ok(fs::read_dir(&dir).is_ok_and(|rd| {
            rd.flatten()
                .any(|e| e.file_name().to_string_lossy().starts_with(&prefix))
        }))
    }

    /// The state file `name` of `session`, if one was written.
    pub fn state(&self, session: &str, name: &str) -> Result<Option<String>> {
        let at = self.state_path(session, name)?;
        match fs::read_to_string(&at) {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Write the state file `name` of `session` whole.
    pub fn set_state(&self, session: &str, name: &str, text: &str) -> Result<()> {
        Ok(crate::files::write_atomic(
            &self.state_path(session, name)?,
            text.as_bytes(),
        )?)
    }

    fn state_path(&self, session: &str, name: &str) -> Result<PathBuf> {
        check_id("state", name)?;
        if name.contains('@') {
            bail!("state {name:?} would read as a mark");
        }
        Ok(self.session(session)?.join(name))
    }

    /// Forget what `session` was sent: its next append sends everything.
    pub fn reset(&self, session: &str) -> Result<()> {
        match fs::remove_dir_all(self.session(session)?) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        }
    }

    fn session(&self, session: &str) -> Result<PathBuf> {
        check_id("session id", session)?;
        Ok(self.dir.join(session))
    }
}

fn remove(p: &Path) -> io::Result<()> {
    match fs::remove_file(p) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// A session id or item id becomes a file name, so it must be one plain
/// component that cannot reach outside the record.
pub fn check_id(what: &str, s: &str) -> Result<()> {
    let plain = !s.is_empty()
        && s.len() <= 128
        && !s.starts_with('.')
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
    if !plain {
        bail!("{what} {s:?} is not letters, digits, '-', '_' and '.'");
    }
    Ok(())
}

/// FNV-1a: fixed across builds, unlike std's hasher, so an upgrade of the
/// core does not re-send everything.
pub fn fnv1a(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

// ─────────────────────────────────────────────────────────────────
// Delivering a call's items: what the session is not already holding.
// ─────────────────────────────────────────────────────────────────

pub fn deliver(ctx: &Infusion, call: &Call, items: Vec<Item>) -> Result<Vec<Item>> {
    deliver_within(ctx, call, items, usize::MAX)
}

/// The items this call places, in order, at most `budget` bytes of text in
/// all: what an append carrier's session already holds is left out first,
/// then the last items over the budget, and only what is placed is
/// recorded.
pub fn deliver_within(
    ctx: &Infusion,
    call: &Call,
    items: Vec<Item>,
    budget: usize,
) -> Result<Vec<Item>> {
    let fresh = unheld(ctx, call, items)?;
    claim(ctx, call, within(fresh, budget))
}

/// `items` less what this call's session already holds, after the reset
/// the call asks for. Only an append carrier's session holds anything.
pub fn unheld(ctx: &Infusion, call: &Call, items: Vec<Item>) -> Result<Vec<Item>> {
    let session = call.session.as_deref();
    let record = Record::new(ctx.config.delivery());
    if let Some(s) = session {
        check_id("session id", s)?;
        if call.reset {
            record.reset(s)?;
        }
    }
    let mut fresh = vec![];
    for item in items {
        let held = match (session, call.carrier) {
            (Some(s), Carrier::Append) => record.holds(s, &item.id, &sent(&item))?,
            _ => false,
        };
        if !held {
            fresh.push(item);
        }
    }
    Ok(fresh)
}

/// The first of `items` that fit in `budget` bytes of text: it stops at the
/// first one over, since what comes after (the last fits, the short form,
/// then candidates) matters less than what came before.
pub fn within(items: Vec<Item>, budget: usize) -> Vec<Item> {
    let mut used = 0;
    items
        .into_iter()
        .take_while(|i| {
            used += i.text.len();
            used <= budget
        })
        .collect()
}

/// Record `items` as sent to the call's session, and return those this
/// call places: on an append carrier another carrier may have placed one
/// since, and exactly one of them does.
pub fn claim(ctx: &Infusion, call: &Call, items: Vec<Item>) -> Result<Vec<Item>> {
    let Some(session) = call.session.as_deref() else {
        // Without a session nothing can be remembered, so all of it goes.
        return Ok(items);
    };
    let record = Record::new(ctx.config.delivery());
    let mut out = vec![];
    for item in items {
        let placed = match call.carrier {
            Carrier::Transient => true,
            Carrier::Append => record.claim(session, &item.id, &sent(&item))?,
            Carrier::OnDemand => {
                record.claim(session, &item.id, &sent(&item))?;
                true
            }
        };
        if placed {
            out.push(item);
        }
    }
    Ok(out)
}

/// What a carrier may place: the text, or the data a JSON carrier takes.
fn sent(item: &Item) -> String {
    format!("{}\n{}", item.text, item.data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use tempfile::TempDir;

    #[test]
    fn the_first_send_is_recorded_and_the_same_again_is_not_sent() {
        let dir = TempDir::new().unwrap();
        let r = Record::new(dir.path());
        assert!(r.claim("s1", "genome", "a").unwrap());
        assert!(!r.claim("s1", "genome", "a").unwrap());
        assert!(
            r.claim("s2", "genome", "a").unwrap(),
            "another session has its own record"
        );
    }

    #[test]
    fn changed_content_is_sent_and_so_is_a_return_to_the_old() {
        let dir = TempDir::new().unwrap();
        let r = Record::new(dir.path());
        assert!(r.claim("s", "genome", "a").unwrap());
        assert!(r.claim("s", "genome", "b").unwrap());
        assert!(r.claim("s", "genome", "a").unwrap());
        assert!(!r.claim("s", "genome", "a").unwrap());
    }

    #[test]
    fn holds_tells_what_was_sent_and_claims_nothing() {
        let dir = TempDir::new().unwrap();
        let r = Record::new(dir.path());
        assert!(!r.holds("s", "genome", "a").unwrap());
        assert!(
            !r.holds("s", "genome", "a").unwrap(),
            "asking claims nothing"
        );
        assert!(r.claim("s", "genome", "a").unwrap());
        assert!(r.holds("s", "genome", "a").unwrap());
        assert!(!r.holds("s", "genome", "b").unwrap());
        assert!(r.sent_any("s", "genome").unwrap());
        assert!(!r.sent_any("s", "other").unwrap());
        assert!(!r.sent_any("t", "genome").unwrap());
    }

    #[test]
    fn reset_sends_again_and_a_reset_of_nothing_is_fine() {
        let dir = TempDir::new().unwrap();
        let r = Record::new(dir.path());
        r.reset("s").unwrap();
        assert!(r.claim("s", "genome", "a").unwrap());
        r.reset("s").unwrap();
        assert!(r.claim("s", "genome", "a").unwrap());
    }

    #[test]
    fn of_two_senders_at_once_exactly_one_sends() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();
        let start = Arc::new(Barrier::new(8));
        let sent: usize = (0..8)
            .map(|_| {
                let (path, start) = (path.clone(), start.clone());
                std::thread::spawn(move || {
                    start.wait();
                    Record::new(path).claim("s", "genome", "a").unwrap()
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|t| usize::from(t.join().unwrap()))
            .sum();
        assert_eq!(sent, 1);
    }

    #[test]
    fn an_id_that_could_leave_the_record_is_refused() {
        let dir = TempDir::new().unwrap();
        let r = Record::new(dir.path().join("delivery"));
        for bad in ["", "..", ".", "a/b", "../x", "a\0b", ".hidden"] {
            assert!(r.claim(bad, "genome", "a").is_err(), "{bad:?}");
            assert!(r.reset(bad).is_err(), "{bad:?}");
            assert!(r.claim("s", bad, "a").is_err(), "{bad:?}");
        }
        assert!(!dir.path().join("x").exists());
        assert!(r.claim("0f3c-uuid_like.1", "genome", "a").unwrap());
    }

    #[test]
    fn a_state_file_is_the_sessions_own_and_a_reset_clears_it() {
        let dir = TempDir::new().unwrap();
        let r = Record::new(dir.path());
        assert_eq!(r.state("s", "drift.json").unwrap(), None);
        r.set_state("s", "drift.json", "{}").unwrap();
        assert_eq!(r.state("s", "drift.json").unwrap().as_deref(), Some("{}"));
        assert_eq!(r.state("t", "drift.json").unwrap(), None);
        assert!(!r.sent_any("s", "drift.json").unwrap(), "not a mark");
        assert!(r.set_state("s", "x@y", "{}").is_err());
        assert!(r.set_state("s", "../x", "{}").is_err());
        r.reset("s").unwrap();
        assert_eq!(r.state("s", "drift.json").unwrap(), None);
    }

    #[test]
    fn the_hash_is_fixed() {
        assert_eq!(fnv1a(""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a("a"), 0xaf63_dc4c_8601_ec8c);
    }
}
