//! The ledger: what each reflection saw and what the person decided, one
//! JSON line each in `<state>/ledger.jsonl`, so the ghost can tell
//! whether it is getting better: the same corrections no longer repeated,
//! fewer refusals, fewer bring-in changes needed.

use crate::adapt::{Correction, Outcome};
use crate::config::Config;
use crate::experience::{Bundle, Struggle};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase", tag = "is")]
pub enum Record {
    /// What one reflection on a session saw. A session reflected on again
    /// counts by its last.
    Seen {
        at: String,
        session: String,
        /// Per thing (`tool:<name>`, or `session`), per kind of struggle.
        friction: BTreeMap<String, BTreeMap<String, usize>>,
        /// One thing per correction the reflector found.
        corrections: Vec<String>,
    },
    /// A session the person left out without reflecting on it.
    Dismissed { at: String, session: String },
    /// One decided item.
    Decided {
        at: String,
        session: String,
        thing: String,
        outcome: String,
        change: String,
    },
}

fn append(config: &Config, records: &[Record]) -> Result<()> {
    if records.is_empty() {
        return Ok(());
    }
    let path = config.ledger();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut text = String::new();
    for r in records {
        text.push_str(&serde_json::to_string(r)?);
        text.push('\n');
    }
    // One write, appended: small enough to land whole.
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut f| f.write_all(text.as_bytes()))
        .with_context(|| format!("cannot write {}", path.display()))
}

fn kind(s: Struggle) -> &'static str {
    match s {
        Struggle::Interrupt => "interrupts",
        Struggle::Refusal => "refusals",
        Struggle::Error => "errors",
        Struggle::Retry => "retries",
    }
}

pub fn seen(config: &Config, bundle: &Bundle, corrections: &[Correction]) -> Result<()> {
    let mut friction: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for (s, tool) in bundle.struggles() {
        let thing = tool.map_or("session".into(), |t| format!("tool:{t}"));
        *friction
            .entry(thing)
            .or_default()
            .entry(kind(s).into())
            .or_default() += 1;
    }
    append(
        config,
        &[Record::Seen {
            at: crate::time::now(),
            session: bundle.session.clone(),
            friction,
            corrections: corrections
                .iter()
                .map(|c| c.thing.clone().unwrap_or_else(|| "unplaced".into()))
                .collect(),
        }],
    )
}

pub fn decided(config: &Config, session: &str, out: &[(Outcome, String)]) -> Result<()> {
    let at = crate::time::now();
    let records: Vec<Record> = out
        .iter()
        .filter(|(o, _)| matches!(o.outcome.as_str(), "accepted" | "declined" | "already"))
        .map(|(o, change)| Record::Decided {
            at: at.clone(),
            session: session.into(),
            thing: o.thing.clone(),
            outcome: o.outcome.clone(),
            change: change.clone(),
        })
        .collect();
    append(config, &records)
}

/// Record that the person left a session out.
pub fn dismissed(config: &Config, session: &str) -> Result<()> {
    append(
        config,
        &[Record::Dismissed {
            at: crate::time::now(),
            session: session.into(),
        }],
    )
}

pub fn read(config: &Config) -> Result<Vec<Record>> {
    let path = config.ledger();
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e).with_context(|| format!("cannot read {}", path.display())),
    };
    // A line that does not read (a write cut short) is left out.
    Ok(text
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect())
}

/// What this session already decided: `<thing>: <outcome>: <change>`.
pub fn decided_in(config: &Config, session: &str) -> Result<Vec<String>> {
    Ok(read(config)?
        .into_iter()
        .filter_map(|r| match r {
            Record::Decided {
                session: s,
                thing,
                outcome,
                change,
                ..
            } if s == session => Some(format!("{thing}: {outcome}: {change}")),
            _ => None,
        })
        .collect())
}

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct Row {
    pub thing: String,
    pub sessions: usize,
    pub corrections: usize,
    pub refusals: usize,
    pub interrupts: usize,
    pub errors: usize,
    pub retries: usize,
    /// Findings the person decided: accepted, declined or already there.
    pub decided: usize,
    pub accepted: usize,
}

/// Counts by thing, across sessions: each session's last `Seen`, and
/// every decision.
pub fn summary(records: &[Record]) -> Vec<Row> {
    let mut last: BTreeMap<&str, &Record> = BTreeMap::new();
    let mut rows: BTreeMap<String, Row> = BTreeMap::new();
    let mut sessions: BTreeMap<String, std::collections::BTreeSet<&str>> = BTreeMap::new();
    for r in records {
        match r {
            Record::Seen { session, .. } => {
                last.insert(session, r);
            }
            Record::Dismissed { .. } => {}
            Record::Decided {
                session,
                thing,
                outcome,
                ..
            } => {
                let row = rows.entry(thing.clone()).or_default();
                row.decided += 1;
                row.accepted += usize::from(outcome == "accepted");
                sessions.entry(thing.clone()).or_default().insert(session);
            }
        }
    }
    for r in last.values() {
        let Record::Seen {
            session,
            friction,
            corrections,
            ..
        } = r
        else {
            continue;
        };
        for (thing, kinds) in friction {
            let row = rows.entry(thing.clone()).or_default();
            sessions.entry(thing.clone()).or_default().insert(session);
            for (k, n) in kinds {
                match k.as_str() {
                    "refusals" => row.refusals += n,
                    "interrupts" => row.interrupts += n,
                    "errors" => row.errors += n,
                    "retries" => row.retries += n,
                    _ => {}
                }
            }
        }
        for thing in corrections {
            rows.entry(thing.clone()).or_default().corrections += 1;
            sessions.entry(thing.clone()).or_default().insert(session);
        }
    }
    rows.into_iter()
        .map(|(thing, mut row)| {
            row.sessions = sessions.get(&thing).map_or(0, |s| s.len());
            row.thing = thing;
            row
        })
        .collect()
}

/// Sessions the person dismissed, each once.
pub fn dismissed_count(records: &[Record]) -> usize {
    records
        .iter()
        .filter_map(|r| match r {
            Record::Dismissed { session, .. } => Some(session.as_str()),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>()
        .len()
}

pub fn render(rows: &[Row], dismissed: usize) -> String {
    let tail = match dismissed {
        0 => String::new(),
        n => format!("dismissed without reflection: {n} sessions\n"),
    };
    if rows.is_empty() {
        return format!("the ledger is empty: no reflection yet\n{tail}");
    }
    let mut s = String::from(
        "thing: sessions, corrections, refusals, interrupts, errors, retries, findings decided/accepted\n",
    );
    for r in rows {
        s.push_str(&format!(
            "  {}: {}, {}, {}, {}, {}, {}, {}/{}\n",
            r.thing,
            r.sessions,
            r.corrections,
            r.refusals,
            r.interrupts,
            r.errors,
            r.retries,
            r.decided,
            r.accepted
        ));
    }
    s + &tail
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::experience::Friction;
    use tempfile::TempDir;

    fn config(dir: &std::path::Path) -> Config {
        Config {
            parts: dir.join("parts"),
            state: dir.join("state"),
            claude: None,
        }
    }

    fn bundle(session: &str, refusals: usize) -> Bundle {
        Bundle {
            session: session.into(),
            friction: (0..refusals)
                .map(|_| Friction {
                    at: "t:1".into(),
                    kind: Struggle::Refusal,
                    tool: Some("Edit".into()),
                    text: "no".into(),
                    truncated: false,
                })
                .collect(),
            ..Default::default()
        }
    }

    fn outcome(thing: &str, outcome: &str) -> (Outcome, String) {
        (
            Outcome {
                n: 1,
                thing: thing.into(),
                outcome: outcome.into(),
                text: String::new(),
            },
            format!("change to {thing}"),
        )
    }

    #[test]
    fn a_session_reflected_twice_counts_its_last_reflection_and_every_decision() {
        let d = TempDir::new().unwrap();
        let c = config(d.path());
        let fix = |t: &str| Correction {
            at: None,
            thing: Some(t.into()),
        };
        seen(&c, &bundle("s1", 1), &[fix("a/style")]).unwrap();
        decided(&c, "s1", &[outcome("a/style", "accepted")]).unwrap();
        seen(&c, &bundle("s1", 2), &[fix("a/style")]).unwrap();
        decided(
            &c,
            "s1",
            &[outcome("a/mem", "declined"), outcome("a/x", "failed")],
        )
        .unwrap();
        seen(&c, &bundle("s2", 1), &[fix("a/style")]).unwrap();
        let rows = summary(&read(&c).unwrap());
        let by = |t: &str| rows.iter().find(|r| r.thing == t).unwrap().clone();
        assert_eq!(by("tool:Edit").refusals, 3, "s1's last (2) and s2's (1)");
        assert_eq!(by("a/style").corrections, 2);
        assert_eq!(by("a/style").sessions, 2);
        assert_eq!((by("a/style").decided, by("a/style").accepted), (1, 1));
        assert_eq!((by("a/mem").decided, by("a/mem").accepted), (1, 0));
        assert!(
            rows.iter().all(|r| r.thing != "a/x"),
            "a failure is not decided"
        );
        assert_eq!(
            decided_in(&c, "s1").unwrap(),
            [
                "a/style: accepted: change to a/style",
                "a/mem: declined: change to a/mem"
            ]
        );
    }

    #[test]
    fn an_empty_ledger_says_so_and_a_cut_line_is_left_out() {
        let d = TempDir::new().unwrap();
        let c = config(d.path());
        assert!(render(&summary(&read(&c).unwrap()), 0).contains("no reflection yet"));
        fs::create_dir_all(c.state.clone()).unwrap();
        fs::write(c.ledger(), "{\"is\":\"seen\",\"at\n").unwrap();
        assert!(read(&c).unwrap().is_empty());
    }
}
