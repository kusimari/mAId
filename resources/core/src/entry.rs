//! The five entry points, as functions of one call shape every carrier
//! uses: a hook, a framework adapter, an MCP tool or resource, a skill or
//! the CLI. Each returns items for the carrier to place; the delivery
//! record decides what a carrier that keeps history is sent again.

use crate::config::Config;
use crate::contract::Scope;
use crate::delivery::{self, Record};
use crate::environment::{self, Host, Readiness};
use crate::map::{self, BringIn, Declared};
use crate::parts::Tenure;
use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::fmt::Write as _;
use std::fs;
use std::io;

/// How a carrier keeps what it is given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Carrier {
    /// Kept in the session's history: send only what changed.
    Append,
    /// Sent with one call only: send everything every time.
    Transient,
    /// Asked for: return everything, and record it as sent.
    OnDemand,
}

#[derive(Debug, Clone)]
pub struct Call {
    pub session: Option<String>,
    pub carrier: Carrier,
    /// Forget what the session was sent before answering: after a
    /// compaction, a clear or a resume.
    pub reset: bool,
    /// What the carrier itself passes in, as it passes it.
    pub input: Value,
}

/// One thing to place, as text and as data.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Item {
    pub id: String,
    pub text: String,
    pub data: Value,
}

/// What every entry point needs to answer.
pub struct Ctx {
    pub config: Config,
    pub host: Host,
}

/// Who the session is, at its start: the picture `genome` sends.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Picture {
    pub personas: Vec<map::Persona>,
    /// The persona this session most likely acts through: the only one,
    /// when there is one.
    pub acting: Option<String>,
    pub session: Vec<Line>,
    pub turn: Vec<Line>,
    pub environment: Vec<environment::Row>,
    /// Part lists the core would not take, so the picture is of part of
    /// the ghost.
    pub refused: usize,
    /// Bring-in names more than one part records.
    pub clashes: Vec<String>,
    pub pending: Pending,
}

/// One bring-in, as the picture names it.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Line {
    pub part: String,
    pub name: String,
    pub job: Option<String>,
    pub persona: Option<String>,
    pub borrowed_from: Option<String>,
    /// Where its state lives: each state key's bound value, or that the
    /// key is not bound, so a bring-in with memory always says so.
    pub state: Vec<String>,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Pending {
    pub count: usize,
    pub at: String,
}

pub fn genome(ctx: &Ctx, call: &Call) -> Result<Vec<Item>> {
    let p = picture(ctx)?;
    let item = Item {
        // One item: any change re-sends the whole picture. A carrier that
        // needs less can split it into one item per section.
        id: "genome".into(),
        text: render(&p),
        data: serde_json::to_value(&p)?,
    };
    deliver(ctx, call, vec![item])
}

/// What this turn needs. Not filled yet: answers nothing.
pub fn expression(ctx: &Ctx, call: &Call) -> Result<Vec<Item>> {
    deliver(ctx, call, vec![])
}

/// What happened in a session. Not filled yet: answers nothing.
pub fn experience(ctx: &Ctx, call: &Call) -> Result<Vec<Item>> {
    deliver(ctx, call, vec![])
}

/// Marks an outcome done. Not filled yet: answers nothing.
pub fn sleep(ctx: &Ctx, call: &Call) -> Result<Vec<Item>> {
    deliver(ctx, call, vec![])
}

/// Turns experience into changes. Not filled yet: answers nothing.
pub fn adaptation(ctx: &Ctx, call: &Call) -> Result<Vec<Item>> {
    deliver(ctx, call, vec![])
}

fn deliver(ctx: &Ctx, call: &Call, items: Vec<Item>) -> Result<Vec<Item>> {
    let Some(session) = call.session.as_deref() else {
        // Without a session nothing can be remembered, so all of it goes.
        return Ok(items);
    };
    delivery::check_id("session id", session)?;
    let record = Record::new(ctx.config.delivery());
    if call.reset {
        record.reset(session)?;
    }
    let mut out = vec![];
    for item in items {
        let fresh = match call.carrier {
            Carrier::Transient => true,
            Carrier::Append => record.claim(session, &item.id, &sent(&item))?,
            Carrier::OnDemand => {
                record.claim(session, &item.id, &sent(&item))?;
                true
            }
        };
        if fresh {
            out.push(item);
        }
    }
    Ok(out)
}

/// What a carrier may place: the text, or the data a JSON carrier takes.
fn sent(item: &Item) -> String {
    format!("{}\n{}", item.text, item.data)
}

pub fn picture(ctx: &Ctx) -> Result<Picture> {
    let map = map::fuse(&ctx.config.parts)
        .with_context(|| format!("cannot read {}", ctx.config.parts.display()))?;
    // Two parts may deliver one persona; it is still the only one.
    let named: std::collections::BTreeSet<&str> = map
        .personas
        .iter()
        .filter_map(|p| p.persona.as_deref())
        .collect();
    let acting = match named.len() {
        1 => named.first().map(|one| one.to_string()),
        _ => None,
    };
    let (mut session, mut turn) = (vec![], vec![]);
    for b in &map.bring_ins {
        match (&b.declared, b.declaration().map(|d| d.scope)) {
            (Declared::Unreadable { .. }, _) => {}
            (_, Some(Scope::Session)) => session.push(line(b)),
            // Undeclared units are offered by their own description.
            _ => turn.push(line(b)),
        }
    }
    Ok(Picture {
        personas: map.personas.clone(),
        acting,
        session,
        turn,
        environment: environment::derive(&map, &ctx.host),
        refused: map.refused.len(),
        clashes: map.clashes.clone(),
        pending: pending(ctx)?,
    })
}

fn line(b: &BringIn) -> Line {
    let record = b.record.as_ref();
    let values = record.map(|r| &r.values);
    Line {
        part: b.part.clone(),
        name: b.name.clone(),
        job: b.job().map(str::to_string),
        persona: b.persona.clone(),
        borrowed_from: record.and_then(|r| match &r.tenure {
            Tenure::Borrowed { from, .. } => Some(from.clone()),
            Tenure::Own => None,
        }),
        state: b
            .declaration()
            .and_then(|d| d.state.as_ref())
            .into_iter()
            .flat_map(|s| s.keys())
            .map(|k| match values.and_then(|v| v.get(k)) {
                Some(v) => v.clone(),
                None => format!("{k} not bound"),
            })
            .collect(),
    }
}

/// Decisions waiting for the person: one file each in the pending dir.
fn pending(ctx: &Ctx) -> Result<Pending> {
    let dir = ctx.config.pending();
    let count = match fs::read_dir(&dir) {
        Ok(rd) => rd
            .flatten()
            .filter(|e| {
                let n = e.file_name();
                let n = n.to_string_lossy();
                !n.starts_with('.') && n.ends_with(".json")
            })
            .count(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => 0,
        Err(e) => return Err(e).with_context(|| format!("cannot read {}", dir.display())),
    };
    Ok(Pending {
        count,
        at: dir.display().to_string(),
    })
}

/// The picture in a few lines per section, in a stable order.
pub fn render(p: &Picture) -> String {
    let mut s = String::new();
    let personas: Vec<String> = p
        .personas
        .iter()
        .map(|x| match &x.persona {
            Some(name) => format!("{name} ({})", x.part),
            None => format!("no persona ({})", x.part),
        })
        .collect();
    let _ = writeln!(
        s,
        "personas: {}",
        match personas.is_empty() {
            true => "none (no part lists)".to_string(),
            false => personas.join(", "),
        }
    );
    let _ = writeln!(
        s,
        "acting through: {}",
        p.acting
            .as_deref()
            .unwrap_or("not known; ask which persona this work is for")
    );
    for (title, lines) in [
        ("session bring-ins", &p.session),
        ("turn bring-ins", &p.turn),
    ] {
        let _ = writeln!(s, "{title}:{}", if lines.is_empty() { " none" } else { "" });
        for l in lines {
            let mut who = l.persona.clone().unwrap_or_else(|| l.part.clone());
            match &l.borrowed_from {
                Some(from) => who += &format!(", borrowed from {from}"),
                None => who += ", own",
            }
            let _ = write!(
                s,
                "  {}: {} [{who}]",
                l.name,
                l.job.as_deref().unwrap_or("no job given")
            );
            if !l.state.is_empty() {
                let _ = write!(s, "; state: {}", l.state.join(", "));
            }
            s.push('\n');
        }
    }
    let ready = p
        .environment
        .iter()
        .filter(|r| r.readiness == Readiness::Ready)
        .count();
    let _ = writeln!(
        s,
        "environment: {ready} of {} bring-ins ready",
        p.environment.len()
    );
    let undeclared: Vec<&str> = p
        .environment
        .iter()
        .filter(|r| r.readiness == Readiness::Undeclared)
        .map(|r| r.name.as_str())
        .collect();
    if !undeclared.is_empty() {
        let _ = writeln!(s, "  undeclared: {}", undeclared.join(", "));
    }
    for r in &p.environment {
        if let Some(why) = r.readiness.why() {
            let _ = writeln!(
                s,
                "  {} {} ({}): {why}",
                r.readiness.label(),
                r.name,
                r.part
            );
        }
    }
    match p.refused {
        0 => {}
        1 => s.push_str("  refused: 1 part list (see maid-core map)\n"),
        n => {
            let _ = writeln!(s, "  refused: {n} part lists (see maid-core map)");
        }
    }
    if !p.clashes.is_empty() {
        let _ = writeln!(
            s,
            "  clashes: {} ({})",
            p.clashes.len(),
            p.clashes.join(", ")
        );
    }
    let _ = writeln!(
        s,
        "pending: {} decisions ({})",
        p.pending.count, p.pending.at
    );
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::Path;
    use tempfile::TempDir;

    fn ctx(dir: &Path) -> Ctx {
        Ctx {
            config: Config {
                parts: dir.join("parts"),
                state: dir.join("state"),
            },
            host: Host::default(),
        }
    }

    fn call(session: Option<&str>, carrier: Carrier, reset: bool) -> Call {
        Call {
            session: session.map(str::to_string),
            carrier,
            reset,
            input: Value::Null,
        }
    }

    fn part(dir: &Path, name: &str, persona: &str, bring_ins: Value) {
        let parts = dir.join("parts");
        fs::create_dir_all(&parts).unwrap();
        let v = json!({"format": 1, "part": name, "harnesses": {}, "persona": persona, "bring_ins": bring_ins});
        fs::write(parts.join(format!("{name}.json")), v.to_string()).unwrap();
    }

    fn memory(dir: &Path) -> String {
        let p = dir.join("mem.json");
        let decl = json!({"scope": "session", "job": "memory for the persona",
            "config": {"store": {"about": "the store"}}, "state": {"local": "store"}});
        fs::write(
            &p,
            json!({"name": "mem", "_meta": {"my-harness": decl}}).to_string(),
        )
        .unwrap();
        p.display().to_string()
    }

    #[test]
    fn append_sends_the_picture_once_and_again_on_change_or_reset() {
        let dir = TempDir::new().unwrap();
        let c = ctx(dir.path());
        part(dir.path(), "a", "personal", json!([{"name": "x"}]));
        let append = call(Some("s"), Carrier::Append, false);
        assert_eq!(genome(&c, &append).unwrap().len(), 1);
        assert!(genome(&c, &append).unwrap().is_empty());
        part(
            dir.path(),
            "a",
            "personal",
            json!([{"name": "x"}, {"name": "y"}]),
        );
        assert_eq!(genome(&c, &append).unwrap().len(), 1, "a change re-sends");
        assert!(genome(&c, &append).unwrap().is_empty());
        let reset = call(Some("s"), Carrier::Append, true);
        assert_eq!(genome(&c, &reset).unwrap().len(), 1, "a reset re-sends");
        let sub = call(Some("s.sub-1"), Carrier::Append, false);
        assert_eq!(
            genome(&c, &sub).unwrap().len(),
            1,
            "a sub-session has its own record"
        );
    }

    #[test]
    fn on_demand_always_answers_and_counts_as_sent() {
        let dir = TempDir::new().unwrap();
        let c = ctx(dir.path());
        part(dir.path(), "a", "personal", json!([]));
        let asked = call(Some("s"), Carrier::OnDemand, false);
        assert_eq!(genome(&c, &asked).unwrap().len(), 1);
        assert_eq!(genome(&c, &asked).unwrap().len(), 1);
        assert!(genome(&c, &call(Some("s"), Carrier::Append, false))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn transient_and_sessionless_calls_always_get_everything_and_record_nothing() {
        let dir = TempDir::new().unwrap();
        let c = ctx(dir.path());
        part(dir.path(), "a", "personal", json!([]));
        for _ in 0..2 {
            assert_eq!(
                genome(&c, &call(Some("s"), Carrier::Transient, false))
                    .unwrap()
                    .len(),
                1
            );
            assert_eq!(
                genome(&c, &call(None, Carrier::Append, false))
                    .unwrap()
                    .len(),
                1
            );
        }
        assert_eq!(
            genome(&c, &call(Some("s"), Carrier::Append, false))
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn the_unfilled_entry_points_answer_nothing() {
        let dir = TempDir::new().unwrap();
        let c = ctx(dir.path());
        let k = call(Some("s"), Carrier::Append, false);
        for f in [expression, experience, sleep, adaptation] {
            assert!(f(&c, &k).unwrap().is_empty());
        }
    }

    #[test]
    fn the_picture_sorts_bring_ins_by_scope_and_names_their_state() {
        let dir = TempDir::new().unwrap();
        let mem = memory(dir.path());
        part(
            dir.path(),
            "home",
            "personal",
            json!([
                {"name": "mem", "unit": {"standard": "mcp", "path": mem}, "values": {"store": "/s/personal"}},
                {"name": "docs", "tenure": {"kind": "borrowed", "from": "a vendor"}}
            ]),
        );
        part(
            dir.path(),
            "job",
            "work",
            json!([{"name": "gone", "unit": {"standard": "skill", "path": "/nowhere"}}]),
        );
        part(
            dir.path(),
            "lab",
            "work",
            json!([{"name": "unbound-mem", "unit": {"standard": "mcp", "path": mem}}]),
        );
        let p = picture(&ctx(dir.path())).unwrap();
        assert_eq!(p.acting, None, "two personas: not guessed");
        assert_eq!(p.session.len(), 2);
        assert_eq!(p.session[0].state, ["/s/personal"]);
        assert_eq!(p.session[1].state, ["store not bound"]);
        assert_eq!(p.turn.len(), 1);
        assert_eq!(p.turn[0].borrowed_from.as_deref(), Some("a vendor"));
        let text = render(&p);
        for want in [
            "personas: personal (home), work (job), work (lab)",
            "acting through: not known",
            "  mem: memory for the persona [personal, own]; state: /s/personal",
            "  docs: no job given [personal, borrowed from a vendor]",
            "environment: 0 of 4 bring-ins ready",
            "  undeclared: docs",
            "  not ready mem (home): store at /s/personal not there",
            "  unreadable gone (job): cannot read /nowhere",
            "pending: 0 decisions",
        ] {
            assert!(text.contains(want), "{want:?} in:\n{text}");
        }
    }

    #[test]
    fn refused_lists_and_clashes_get_a_line_only_when_there_are_any() {
        let dir = TempDir::new().unwrap();
        part(dir.path(), "a", "personal", json!([{"name": "mem"}]));
        let quiet = render(&picture(&ctx(dir.path())).unwrap());
        assert!(
            !quiet.contains("refused") && !quiet.contains("clashes"),
            "{quiet}"
        );
        part(dir.path(), "b", "work", json!([{"name": "mem"}]));
        fs::write(dir.path().join("parts/c.json"), "not json").unwrap();
        let text = render(&picture(&ctx(dir.path())).unwrap());
        assert!(
            text.contains("  refused: 1 part list (see maid-core map)\n"),
            "{text}"
        );
        assert!(text.contains("  clashes: 1 (mem)\n"), "{text}");
    }

    #[test]
    fn two_parts_with_one_persona_still_act_through_it() {
        let dir = TempDir::new().unwrap();
        part(dir.path(), "a", "personal", json!([]));
        part(dir.path(), "b", "personal", json!([]));
        assert_eq!(
            picture(&ctx(dir.path())).unwrap().acting.as_deref(),
            Some("personal")
        );
    }

    #[test]
    fn a_bad_session_id_is_refused_whatever_the_carrier() {
        let dir = TempDir::new().unwrap();
        let c = ctx(dir.path());
        for carrier in [Carrier::Append, Carrier::Transient, Carrier::OnDemand] {
            assert!(
                genome(&c, &call(Some("../x"), carrier, false)).is_err(),
                "{carrier:?}"
            );
        }
    }

    #[test]
    fn one_persona_is_the_acting_one_and_pending_counts_its_dir() {
        let dir = TempDir::new().unwrap();
        part(dir.path(), "a", "personal", json!([]));
        let pending = dir.path().join("state/pending");
        fs::create_dir_all(&pending).unwrap();
        for f in ["1.json", "2.json", ".tmp.json", "note.txt"] {
            fs::write(pending.join(f), "{}").unwrap();
        }
        let p = picture(&ctx(dir.path())).unwrap();
        assert_eq!(p.acting.as_deref(), Some("personal"));
        assert_eq!(p.pending.count, 2);
    }
}
