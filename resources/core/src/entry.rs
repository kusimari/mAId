//! The five entry points, as functions of one call shape every carrier
//! uses: a hook, a framework adapter, an MCP tool or resource, a skill or
//! the CLI. Each returns items for the carrier to place; the delivery
//! record decides what a carrier that keeps history is sent again.

use crate::adapt;
use crate::config::Config;
use crate::contract;
use crate::contract::Scope;
use crate::delivery::{self, Record};
use crate::environment::{self, Host, Readiness};
use crate::experience::{Bundle, Struggle, Who};
use crate::files::write_atomic;
use crate::fit;
use crate::here::Here;
use crate::ledger;
use crate::map::{self, BringIn, Declared, Map};
use crate::parts::Tenure;
use crate::record;
use crate::select::{self, Selector};
use crate::sleep;
use crate::source::{self, Located};
use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

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
    /// How much of the genome a session needs.
    pub slice: Slice,
}

/// How much of the genome a session is given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum Slice {
    /// All of it.
    #[default]
    Whole,
    /// A sub-agent's: who and what, without what waits for the person
    /// (the environment's detail and pending decisions).
    SubSession,
}

impl Call {
    /// The session a carrier's own input names, for a call that names
    /// none: its `session_id`, and with an `agent_id` the sub-session
    /// `<session_id>.<agent_id>`, or `<session_id>.a-<hash>` when that
    /// would not be a plain id.
    pub fn session_from_input(input: &Value) -> Option<String> {
        let session = input.get("session_id")?.as_str()?;
        let Some(agent) = input.get("agent_id").and_then(Value::as_str) else {
            return Some(session.to_string());
        };
        let joined = format!("{session}.{agent}");
        match delivery::check_id("session id", &joined) {
            Ok(()) => Some(joined),
            Err(_) => Some(format!("{session}.a-{:016x}", delivery::fnv1a(agent))),
        }
    }
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
    /// Sessions marked for sleep and not yet reflected on.
    pub waiting: usize,
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
    let p = picture_at(ctx, &here(call))?;
    let item = Item {
        // One item: any change re-sends the whole picture. A carrier that
        // needs less can split it into one item per section.
        id: "genome".into(),
        text: render_slice(&p, call.slice),
        data: serde_json::to_value(&p)?,
    };
    deliver(ctx, call, vec![item])
}

/// Claude Code places a hook's `additionalContext` whole only up to 10,000
/// characters; a turn's items stay under this many bytes, which leaves room
/// for the framing, and a byte count is never below the character count.
const TURN_BYTES: usize = 9_000;
/// No one item may take most of a turn: every piece of text an item takes
/// from a file is cut to these.
const RULES_LINES: usize = 30;
const RULES_BYTES: usize = 2_000;
const FIELD_BYTES: usize = 300;

/// What this turn needs, each as its own item so only what changed is
/// sent: the reinforcement (acting persona, its memory, the work's record),
/// the first rules of each bring-in that fits, and the memories that bear
/// on it. Never the catalog. Input: `prompt` and `cwd`.
pub fn expression(ctx: &Ctx, call: &Call) -> Result<Vec<Item>> {
    let map = map::fuse(&ctx.config.parts)
        .with_context(|| format!("cannot read {}", ctx.config.parts.display()))?;
    let here = here(call);
    let prompt = call.input["prompt"].as_str().unwrap_or_default();
    let acting = fit::persona(&map, &here);
    let record = work_record(&map, &here);
    // Whether this session was told the item before, so that leaving what
    // it says is said once, and only to a session that was told it.
    let told = |id: &str| match (call.session.as_deref(), call.carrier, call.reset) {
        (Some(s), Carrier::Append, false) => Record::new(ctx.config.delivery())
            .sent_any(s, id)
            .unwrap_or(false),
        _ => false,
    };
    let mut items: Vec<Item> = [
        reinforce_persona(&map, acting.as_deref(), told(PERSONA)),
        reinforce_record(record.as_ref(), told(RECORD)),
    ]
    .into_iter()
    .flatten()
    .collect();
    let state = record.as_ref().and_then(|r| r.state.as_deref());
    let mut seen_rules = std::collections::BTreeSet::new();
    let mut fitted = 0;
    for f in fit::ranked(&map, prompt, state, &here) {
        if fitted == fit::CAP {
            break;
        }
        let b = f.bring_in;
        let rules = rules_of(b);
        if let Ok(Some(text)) = &rules {
            if !seen_rules.insert(text.clone()) {
                continue;
            }
        }
        // The cap counts fits sent, so rules two parts share do not use it.
        fitted += 1;
        let mut text = format!(
            "[my-harness] fits this turn: {} ({}): {}\n",
            clip(&b.name, FIELD_BYTES),
            clip(b.persona.as_deref().unwrap_or(&b.part), FIELD_BYTES),
            clip(b.job().unwrap_or("no job given"), FIELD_BYTES)
        );
        if let Ok(Some(r)) = &rules {
            text.push_str(r);
            text.push('\n');
        }
        items.push(Item {
            id: fit_id(&b.part, &b.name),
            text,
            // Only what the bring-in is, not why this prompt fit it: the
            // record hashes the data too, and the same rules must not be sent
            // again because another word matched.
            data: serde_json::json!({"part": b.part, "name": b.name,
                "rules": rules.as_ref().err()}),
        });
    }
    let (memories, _) = select::memories(&map, acting.as_deref());
    let query = select::Query { prompt };
    for m in select::Lookup.select(&query, &memories, select::CAP) {
        items.push(Item {
            id: format!(
                "recall.{:016x}",
                delivery::fnv1a(&m.file.display().to_string())
            ),
            text: format!(
                "[my-harness] memory ({}): [{}]({}) - {}\n",
                m.persona.as_deref().unwrap_or(&m.bring_in),
                clip(&m.title, FIELD_BYTES),
                m.file.display(),
                clip(&m.hook, FIELD_BYTES)
            ),
            data: serde_json::to_value(&m)?,
        });
    }
    deliver_within(ctx, call, items, TURN_BYTES)
}

fn here(call: &Call) -> Here {
    Here::at(call.input["cwd"].as_str().map(Path::new))
}

/// The work's own record: the first turn bring-in whose `record` names one
/// here, whether or not it fits this turn, since its state is part of what
/// decides the fit.
fn work_record(map: &Map, here: &Here) -> Option<record::Record> {
    map.bring_ins
        .iter()
        .filter_map(|b| b.declaration())
        .filter(|d| d.scope == Scope::Turn && !d.record.is_empty())
        .find_map(|d| record::find(&d.record, here).ok().flatten())
}

const PERSONA: &str = "reinforce.persona";
const RECORD: &str = "reinforce.record";

/// The acting persona and its memory, restated only where it can change:
/// with one persona the genome has said all there is. Where none is known,
/// one line, and only to a session that was told one.
fn reinforce_persona(map: &Map, acting: Option<&str>, told: bool) -> Option<Item> {
    let personas: std::collections::BTreeSet<&str> = map
        .personas
        .iter()
        .filter_map(|p| p.persona.as_deref())
        .collect();
    if personas.len() < 2 {
        return None;
    }
    let text = match acting {
        Some(persona) => {
            let memory: Vec<String> = map
                .bring_ins
                .iter()
                .filter(|b| b.persona.as_deref() == Some(persona))
                .filter(|b| b.declaration().is_some_and(|d| d.scope == Scope::Session))
                .filter_map(|b| {
                    let l = line(b);
                    (!l.state.is_empty())
                        .then(|| clip(&format!("{} ({})", l.name, l.state.join(", ")), FIELD_BYTES))
                })
                .take(10)
                .collect();
            match memory.is_empty() {
                true => format!("[my-harness] acting through {persona}; no memory of its own\n"),
                false => format!(
                    "[my-harness] acting through {persona}; its memory: {}\n",
                    memory.join(", ")
                ),
            }
        }
        None if told => "[my-harness] persona: none known here\n".into(),
        None => return None,
    };
    Some(Item {
        id: PERSONA.into(),
        text,
        data: serde_json::json!({ "acting": acting }),
    })
}

/// The work's record: goal, state and next. Where there is none, one line,
/// and only to a session that was told one.
fn reinforce_record(record: Option<&record::Record>, told: bool) -> Option<Item> {
    let text = match record {
        Some(r) => {
            let mut text = format!(
                "[my-harness] the work's record: {}\n",
                clip(&r.file.display().to_string(), FIELD_BYTES)
            );
            for (label, v) in [("goal", &r.goal), ("state", &r.state), ("next", &r.next)] {
                if let Some(v) = v {
                    let _ = writeln!(text, "  {label}: {}", clip(v, FIELD_BYTES));
                }
            }
            text
        }
        None if told => "[my-harness] record: none here\n".into(),
        None => return None,
    };
    Some(Item {
        id: RECORD.into(),
        text,
        data: serde_json::json!({ "record": record }),
    })
}

/// A bring-in's first rules, read from its unit through its `rules` pointer.
fn rules_of(b: &BringIn) -> std::result::Result<Option<String>, String> {
    let (Some(d), Some(unit)) = (
        b.declaration(),
        b.record.as_ref().and_then(|r| r.unit.as_ref()),
    ) else {
        return Ok(None);
    };
    match &d.rules {
        None => Ok(None),
        Some(pointer) => contract::rules(&unit.path, pointer).map(|r| {
            let lines: Vec<&str> = r.lines().take(RULES_LINES).collect();
            Some(clip(&lines.join("\n"), RULES_BYTES))
        }),
    }
}

/// `text` cut to at most `max` bytes at a character boundary, with control
/// characters other than newlines and tabs dropped, since it comes from
/// files the work owns and goes into a model's context.
fn clip(text: &str, max: usize) -> String {
    let clean: String = text
        .chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .collect();
    if clean.len() <= max {
        return clean;
    }
    let mut end = max.saturating_sub(3);
    while !clean.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &clean[..end])
}

/// A fit's item id: its part and name, or a hash of them when either is
/// not plain enough or long enough to be an id as it is, so two names
/// never share one.
fn fit_id(part: &str, name: &str) -> String {
    let id = format!("fit.{part}.{name}");
    let plain = |x: &str| {
        x.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_".contains(c))
    };
    match plain(part) && plain(name) && delivery::check_id("item", &id).is_ok() {
        true => id,
        false => format!("fit.{:016x}", delivery::fnv1a(&format!("{part}\0{name}"))),
    }
}

/// What happened in a session: packs it, stores the experience, and says
/// where it is and what it holds. Naming no session, the sessions waiting
/// to be reflected on.
pub fn experience(ctx: &Ctx, call: &Call) -> Result<Vec<Item>> {
    let named = call.session.is_some()
        || !call.input["transcript_path"].is_null()
        || !call.input["cwd"].is_null();
    let item = match named {
        false => {
            let marks = sleep::waiting(&ctx.config)?;
            let mut text = match marks.len() {
                0 => "no sessions wait to be reflected on\n".to_string(),
                n => format!("sessions to reflect on: {n}\n"),
            };
            for m in &marks {
                let _ = writeln!(text, "  {} ({}, {})", m.session, m.reasons.join(", "), m.at);
            }
            Item {
                id: "waiting".into(),
                text,
                data: serde_json::to_value(&marks)?,
            }
        }
        true => {
            let (_, bundle, at) = pack(ctx, call)?;
            Item {
                id: "experience".into(),
                text: summary(&bundle, &at),
                data: serde_json::json!({ "session": bundle.session, "at": at }),
            }
        }
    };
    deliver(ctx, call, vec![item])
}

/// Marks a session for sleep: from its end event, or the person's close
/// or reflect. Nothing else happens until the person reflects.
pub fn sleep(ctx: &Ctx, call: &Call) -> Result<Vec<Item>> {
    let (_, at) = locate(ctx, call)?;
    let input = &call.input;
    // Claude Code's end event carries its own `reason` (`other`, `clear`,
    // ...); to the mark it is the end, whatever ended it.
    let reason = match (input["hook_event_name"].as_str(), input["reason"].as_str()) {
        (Some("SessionEnd"), _) => sleep::END,
        (_, Some(r)) => r,
        (_, None) => "close",
    };
    let m = sleep::mark(&ctx.config, &at, input["cwd"].as_str(), reason)?;
    if reason == sleep::END {
        // The session has ended: there is no one to tell.
        return Ok(vec![]);
    }
    let item = Item {
        id: "sleep".into(),
        text: format!(
            "session {} marked for sleep ({})\n",
            m.session,
            m.reasons.join(", ")
        ),
        data: serde_json::to_value(&m)?,
    };
    deliver(ctx, call, vec![item])
}

/// Where a call's session is: its mark's transcript when it has one, else
/// what the input names, in the source the input names.
pub fn locate(ctx: &Ctx, call: &Call) -> Result<(Box<dyn source::SessionSource>, Located)> {
    let mut input = call.input.clone();
    if let Some(m) = match call.session.as_deref() {
        Some(s) => sleep::read(&ctx.config, s)?,
        None => None,
    } {
        if input["transcript_path"].is_null() {
            input["transcript_path"] = m.transcript.into();
        }
        if input["source"].is_null() {
            input["source"] = m.source.into();
        }
    }
    let src = source::named(&ctx.config, &input)?;
    let at = src.locate(call.session.as_deref(), &input)?;
    Ok((src, at))
}

/// The session in full as its brief showed it to the reflector.
fn briefed(ctx: &Ctx, call: &Call) -> Result<Bundle> {
    let (_, at) = locate(ctx, call)?;
    let path = ctx.config.briefed(&at.session);
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => anyhow::bail!(
            "no brief for session {}: findings are checked against the session the brief showed, so get the brief first",
            at.session
        ),
        Err(e) => return Err(e).with_context(|| format!("cannot read {}", path.display())),
    };
    serde_json::from_str(&text).with_context(|| format!("{} is not an experience", path.display()))
}

/// Pack the call's session and store its capped experience: the session,
/// every text in full, and where the stored copy is.
pub fn pack(ctx: &Ctx, call: &Call) -> Result<(Located, Bundle, PathBuf)> {
    let (src, at) = locate(ctx, call)?;
    let bundle = src.pack(&at)?;
    let path = ctx.config.experience().join(format!("{}.json", at.session));
    let stored = serde_json::to_string_pretty(&bundle.clone().capped())?;
    write_atomic(&path, stored.as_bytes())
        .with_context(|| format!("cannot write {}", path.display()))?;
    Ok((at, bundle, path))
}

fn summary(b: &Bundle, at: &std::path::Path) -> String {
    let person = b.said.iter().filter(|e| e.who == Who::Person).count();
    let mut counts: std::collections::BTreeMap<Struggle, usize> = Default::default();
    for (kind, _) in b.struggles() {
        *counts.entry(kind).or_default() += 1;
    }
    let friction: Vec<String> = counts
        .iter()
        .map(|(k, n)| {
            format!(
                "{n} {}",
                serde_json::to_value(k)
                    .unwrap_or_default()
                    .as_str()
                    .unwrap_or("?")
            )
        })
        .collect();
    format!(
        "experience of {} ({}): {} said ({person} by the person), {} told, {} did, {} touched; friction: {}; {} sub-sessions\n  at {}\n",
        b.session,
        b.source,
        b.said.len(),
        b.told.len(),
        b.did.len(),
        b.touched.len(),
        match friction.is_empty() {
            true => "none".to_string(),
            false => friction.join(", "),
        },
        b.sub_sessions.len(),
        at.display()
    )
}

/// Turns experience into changes, with the person's guards. With no
/// findings or decisions in its input: the reflector's brief. With the
/// reflector's findings: the decision batch. With the person's
/// decisions: what was applied.
pub fn adaptation(ctx: &Ctx, call: &Call) -> Result<Vec<Item>> {
    let input = &call.input;
    let map = map::fuse(&ctx.config.parts)
        .with_context(|| format!("cannot read {}", ctx.config.parts.display()))?;
    let item = if !input["dismiss"].is_null() {
        let sessions: Vec<String> = match &input["dismiss"] {
            Value::String(s) if s == "all" => sleep::waiting(&ctx.config)?
                .into_iter()
                .map(|m| m.session)
                .collect(),
            Value::Array(ids) => ids
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_string)
                        .ok_or_else(|| anyhow::anyhow!("dismiss takes session ids"))
                })
                .collect::<Result<_>>()?,
            other => anyhow::bail!("dismiss is \"all\" or a list of session ids, not {other}"),
        };
        adapt::dismiss(&ctx.config, &sessions)?;
        Item {
            id: "dismissed".into(),
            text: match sessions.len() {
                0 => "no sessions to dismiss\n".to_string(),
                n => format!("dismissed {n} sessions: {}\n", sessions.join(", ")),
            },
            data: serde_json::json!({ "dismissed": sessions }),
        }
    } else if input["decide"] == "skip" {
        let session = call
            .session
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("name the session to skip"))?;
        adapt::dismiss(&ctx.config, &[session.to_string()])?;
        Item {
            id: "dismissed".into(),
            text: format!("skipped: nothing from {session} is applied\n"),
            data: serde_json::json!({ "dismissed": [session] }),
        }
    } else if !input["decide"].is_null() {
        let session = call
            .session
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("name the session the decisions are for"))?;
        let before = adapt::pending(&ctx.config, session)?;
        let out = adapt::decide(&ctx.config, &map, session, &input["decide"])?;
        let changes: Vec<(adapt::Outcome, String)> = out
            .iter()
            .map(|o| {
                let change = before
                    .iter()
                    .find(|p| p.n == o.n)
                    .map(|p| p.finding.change.clone())
                    .unwrap_or_default();
                (o.clone(), change)
            })
            .collect();
        ledger::decided(&ctx.config, session, &changes)?;
        let left = adapt::pending(&ctx.config, session)?.len();
        Item {
            id: "applied".into(),
            text: adapt::applied(session, &out, left),
            data: serde_json::to_value(&out)?,
        }
    } else if !input["findings"].is_null() || !input["corrections"].is_null() {
        let mut only = serde_json::Map::new();
        for k in ["findings", "corrections"] {
            if !input[k].is_null() {
                only.insert(k.into(), input[k].clone());
            }
        }
        let reflection: adapt::Reflection = serde_json::from_value(Value::Object(only))
            .context("the reflector's answer is not findings of the stated shape")?;
        let corrections = reflection.corrections.clone();
        let bundle = briefed(ctx, call)?;
        let (kept, dropped) = adapt::check(&ctx.config, &map, &bundle, reflection)?;
        ledger::seen(&ctx.config, &bundle, &corrections)?;
        if kept.is_empty() {
            adapt::done(&ctx.config, &bundle.session)?;
        }
        Item {
            id: "batch".into(),
            text: adapt::batch(&bundle.session, &kept, &dropped),
            data: serde_json::json!({ "items": kept, "dropped": dropped }),
        }
    } else {
        let (at, bundle, path) = pack(ctx, call)?;
        // The evidence is checked against the session as the reflector was
        // shown it: what the session does after this (the reflector's own
        // run, the findings handed back) cannot back a finding.
        let briefed = ctx.config.briefed(&at.session);
        write_atomic(&briefed, serde_json::to_string(&bundle)?.as_bytes())
            .with_context(|| format!("cannot write {}", briefed.display()))?;
        let decided = ledger::decided_in(&ctx.config, &at.session)?;
        Item {
            id: "brief".into(),
            text: adapt::brief(
                &at.session,
                &path,
                &at.transcript.display().to_string(),
                &map,
                &decided,
            ),
            data: serde_json::json!({ "session": at.session, "experience": path }),
        }
    };
    deliver(ctx, call, vec![item])
}

fn deliver(ctx: &Ctx, call: &Call, items: Vec<Item>) -> Result<Vec<Item>> {
    deliver_within(ctx, call, items, usize::MAX)
}

/// The items this call places, in order, at most `budget` bytes of text in
/// all: what an append carrier's session already holds is left out first,
/// then the last items over the budget, and only what is placed is
/// recorded.
fn deliver_within(ctx: &Ctx, call: &Call, items: Vec<Item>, budget: usize) -> Result<Vec<Item>> {
    let session = call.session.as_deref();
    let record = Record::new(ctx.config.delivery());
    if let Some(s) = session {
        delivery::check_id("session id", s)?;
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
    // Stop at the first item over the budget: what comes after (the last
    // fits, then recall) matters less than what came before.
    let mut used = 0;
    let fits: Vec<Item> = fresh
        .into_iter()
        .take_while(|i| {
            used += i.text.len();
            used <= budget
        })
        .collect();
    let Some(session) = session else {
        // Without a session nothing can be remembered, so all of it goes.
        return Ok(fits);
    };
    let mut out = vec![];
    for item in fits {
        let placed = match call.carrier {
            Carrier::Transient => true,
            // Another carrier may have placed it since: exactly one does.
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

pub fn picture(ctx: &Ctx) -> Result<Picture> {
    picture_at(ctx, &Here::default())
}

/// The picture as seen from `here`, which the persona guess reads.
pub fn picture_at(ctx: &Ctx, here: &Here) -> Result<Picture> {
    let map = map::fuse(&ctx.config.parts)
        .with_context(|| format!("cannot read {}", ctx.config.parts.display()))?;
    let acting = fit::persona(&map, here);
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
        waiting: sleep::waiting(&ctx.config)?.len(),
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
    render_slice(p, Slice::Whole)
}

pub fn render_slice(p: &Picture, slice: Slice) -> String {
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
    if slice == Slice::SubSession {
        return s;
    }
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
    if p.waiting > 0 {
        let _ = writeln!(
            s,
            "  to reflect on: {} sessions marked for sleep (the reflect skill takes them)",
            p.waiting
        );
    }
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
                claude: Some(dir.join("claude")),
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
            slice: Slice::Whole,
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
    fn with_no_parts_every_entry_point_answers_nothing() {
        let dir = TempDir::new().unwrap();
        let c = ctx(dir.path());
        let k = call(Some("s"), Carrier::Append, false);
        assert!(expression(&c, &k).unwrap().is_empty());
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

    /// A part with a turn skill `name` fitting the job `job`, whose rules
    /// section says `rule`, and a memory whose index holds `memories`.
    fn turn_part(
        dir: &Path,
        part_name: &str,
        persona: &str,
        skills: &[(&str, &str, &str)],
        memories: &str,
    ) {
        let mut bring_ins = vec![];
        for (name, job, rule) in skills {
            let unit = dir.join(format!("units/{part_name}/{name}/SKILL.md"));
            fs::create_dir_all(unit.parent().unwrap()).unwrap();
            fs::write(&unit, format!("---\nname: {name}\ndescription: d\nmetadata:\n  my-harness.scope: turn\n  my-harness.job: {name} work\n  my-harness.signals.jobs: {job}\n  my-harness.rules: SKILL.md#rules\n---\n## Rules\n\n{rule}\n\n## Other\n\nnot sent\n")).unwrap();
            bring_ins.push(json!({"name": name, "unit": {"standard": "skill", "path": unit}}));
        }
        let store = dir.join(format!("{part_name}-store"));
        fs::create_dir_all(store.join("memory")).unwrap();
        fs::write(store.join("memory/MEMORY.md"), memories).unwrap();
        let server = dir.join("server.json");
        let decl = json!({"scope": "session", "job": "memory",
            "config": {"store": {"about": "s"}, "index": {"about": "i"}},
            "state": {"permanent": "store", "promote": "off"}});
        fs::write(
            &server,
            json!({"name": "m", "_meta": {"my-harness": decl}}).to_string(),
        )
        .unwrap();
        bring_ins.push(json!({"name": format!("{part_name}-memory"), "unit": {"standard": "mcp", "path": server},
            "values": {"store": store, "index": "memory/MEMORY.md"}}));
        part(dir, part_name, persona, Value::Array(bring_ins));
    }

    fn said(c: &Ctx, carrier: Carrier, prompt: &str) -> Vec<String> {
        let call = Call {
            input: json!({"prompt": prompt, "cwd": "/"}),
            ..call(Some("s"), carrier, false)
        };
        expression(c, &call)
            .unwrap()
            .into_iter()
            .map(|i| i.id)
            .collect()
    }

    #[test]
    fn expression_sends_what_fits_once_and_only_what_changed() {
        let dir = TempDir::new().unwrap();
        let c = ctx(dir.path());
        turn_part(
            dir.path(),
            "home",
            "personal",
            &[
                ("deployer", "deploy a release", "ALWAYS TAG FIRST"),
                ("writer", "draft an email", "SHORT LINES"),
            ],
            "- [Deploy steps](deploy.md) - how a release is deployed\n",
        );
        let first = said(&c, Carrier::Append, "deploy the release");
        assert_eq!(
            first[0], "fit.home.deployer",
            "one persona: nothing to restate"
        );
        assert!(first[1].starts_with("recall."), "{first:?}");
        assert_eq!(first.len(), 2);
        assert!(
            said(&c, Carrier::Append, "deploy the release").is_empty(),
            "nothing changed"
        );
        assert_eq!(
            said(
                &c,
                Carrier::Append,
                "deploy the release, then draft an email"
            ),
            ["fit.home.writer"],
            "only the new fit"
        );
        assert!(
            said(&c, Carrier::Append, "hello").is_empty(),
            "nothing fits: nothing"
        );
        assert_eq!(said(&c, Carrier::Transient, "deploy the release").len(), 2);
        assert_eq!(said(&c, Carrier::OnDemand, "deploy the release").len(), 2);
    }

    #[test]
    fn a_fit_carries_its_rules_section_and_the_same_rules_from_two_parts_are_sent_once() {
        let dir = TempDir::new().unwrap();
        let c = ctx(dir.path());
        turn_part(
            dir.path(),
            "a",
            "personal",
            &[("deployer", "deploy", "ALWAYS TAG FIRST")],
            "",
        );
        turn_part(
            dir.path(),
            "b",
            "personal",
            &[("deployer", "deploy", "ALWAYS TAG FIRST")],
            "",
        );
        let call = Call {
            input: json!({"prompt": "deploy"}),
            ..call(None, Carrier::Transient, false)
        };
        let items = expression(&c, &call).unwrap();
        let fits: Vec<&Item> = items.iter().filter(|i| i.id.starts_with("fit.")).collect();
        assert_eq!(fits.len(), 1, "{items:?}");
        assert!(
            fits[0].text.contains("## Rules\n\nALWAYS TAG FIRST"),
            "{}",
            fits[0].text
        );
        assert!(!fits[0].text.contains("not sent"));
    }

    #[test]
    fn the_reinforcement_follows_the_acting_persona_and_the_record() {
        let dir = TempDir::new().unwrap();
        let c = ctx(dir.path());
        turn_part(dir.path(), "home", "personal", &[], "");
        let repo = dir.path().join("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::write(repo.join(".git/HEAD"), "ref: refs/heads/feat/hooks/work\n").unwrap();
        fs::create_dir_all(repo.join("specs/feature")).unwrap();
        fs::write(
            repo.join("specs/feature/hooks.md"),
            "# Feature: hooks\n\n## Handoff\n\n- **Stage:** dev\n- **Ready for:** review\n",
        )
        .unwrap();
        let unit = dir.path().join("units/kit/SKILL.md");
        fs::create_dir_all(unit.parent().unwrap()).unwrap();
        fs::write(&unit, "---\nname: kit\ndescription: d\nmetadata:\n  my-harness.scope: turn\n  my-harness.job: specs\n  my-harness.signals.files: specs/feature/*.md\n  my-harness.record: specs/feature/*.md#handoff\n---\n").unwrap();
        let parts = dir.path().join("parts/home.json");
        let mut v: Value = serde_json::from_str(&fs::read_to_string(&parts).unwrap()).unwrap();
        v["bring_ins"]
            .as_array_mut()
            .unwrap()
            .push(json!({"name": "kit", "unit": {"standard": "skill", "path": unit}}));
        fs::write(&parts, v.to_string()).unwrap();
        let at = |cwd: &Path| Call {
            input: json!({"prompt": "go on", "cwd": cwd}),
            ..call(Some("s"), Carrier::Append, false)
        };
        let alone = expression(&c, &at(&repo)).unwrap();
        assert_eq!(alone[0].id, "reinforce.record");
        assert!(
            !alone.iter().any(|i| i.text.contains("acting through")),
            "one persona: the genome said it"
        );
        assert!(
            alone[0].text.contains("goal: Feature: hooks"),
            "{}",
            alone[0].text
        );
        part(dir.path(), "job", "work", json!([]));
        let c = ctx(dir.path());
        let at = |cwd: &Path| Call {
            input: json!({"prompt": "go on", "cwd": cwd}),
            ..call(Some("t"), Carrier::Append, false)
        };
        let items = expression(&c, &at(&repo)).unwrap();
        assert_eq!(items[0].id, "reinforce.persona");
        let text = &items[0].text;
        assert!(
            text.contains("acting through personal; its memory: home-memory ("),
            "two personas, this location points at one: {text}"
        );
        let text = &items[1].text;
        assert!(
            text.contains("goal: Feature: hooks\n  state: dev\n  next: review"),
            "{text}"
        );
        assert!(expression(&c, &at(&repo)).unwrap().is_empty());
        let left: Vec<String> = expression(&c, &at(dir.path()))
            .unwrap()
            .into_iter()
            .map(|i| i.text)
            .collect();
        assert_eq!(
            left,
            [
                "[my-harness] persona: none known here\n",
                "[my-harness] record: none here\n"
            ],
            "leaving the location is said once"
        );
        assert!(expression(&c, &at(dir.path())).unwrap().is_empty());
        let fresh = Call {
            input: json!({"prompt": "go on", "cwd": dir.path()}),
            ..call(Some("fresh"), Carrier::Append, false)
        };
        assert!(
            expression(&c, &fresh).unwrap().is_empty(),
            "a session never told a persona or record is told nothing"
        );
    }

    #[test]
    fn a_session_told_a_persona_and_never_a_record_is_never_told_none_here() {
        let dir = TempDir::new().unwrap();
        turn_part(dir.path(), "home", "personal", &[], "");
        part(dir.path(), "job", "work", json!([]));
        let unit = dir.path().join("units/garden/SKILL.md");
        fs::create_dir_all(unit.parent().unwrap()).unwrap();
        fs::write(&unit, "---\nname: garden\ndescription: d\nmetadata:\n  my-harness.scope: turn\n  my-harness.job: plan the beds\n  my-harness.signals.locations: garden-*\n---\n").unwrap();
        let parts = dir.path().join("parts/home.json");
        let mut v: Value = serde_json::from_str(&fs::read_to_string(&parts).unwrap()).unwrap();
        v["bring_ins"]
            .as_array_mut()
            .unwrap()
            .push(json!({"name": "garden", "unit": {"standard": "skill", "path": unit}}));
        fs::write(&parts, v.to_string()).unwrap();
        let garden = dir.path().join("garden-planner");
        fs::create_dir_all(&garden).unwrap();
        let c = ctx(dir.path());
        let at = |cwd: &Path, prompt: &str| {
            let call = Call {
                input: json!({"prompt": prompt, "cwd": cwd}),
                ..call(Some("s"), Carrier::Append, false)
            };
            expression(&c, &call).unwrap()
        };
        let first = at(&garden, "what goes in the beds");
        assert_eq!(first[0].id, "reinforce.persona", "{first:?}");
        assert!(first[0].text.contains("acting through personal"));
        assert!(
            !first.iter().any(|i| i.text.contains("record:")),
            "{first:?}"
        );
        assert!(
            at(&garden, "what goes in the beds").is_empty(),
            "a second prompt with nothing changed gets nothing"
        );
        let left = at(dir.path(), "and elsewhere");
        let texts: Vec<&str> = left.iter().map(|i| i.text.as_str()).collect();
        assert_eq!(texts, ["[my-harness] persona: none known here\n"]);
    }

    #[test]
    fn the_turn_budget_counts_only_what_is_sent_and_no_one_item_takes_it() {
        let dir = TempDir::new().unwrap();
        let c = ctx(dir.path());
        let skills: Vec<(String, String, String)> = (0..5)
            .map(|i| {
                let rule = format!("RULE {i} {}\n", "TAG FIRST ".repeat(40)).repeat(40);
                (format!("big{i}"), format!("deploy{i}"), rule)
            })
            .collect();
        let spec: Vec<(&str, &str, &str)> = skills
            .iter()
            .map(|(n, j, r)| (n.as_str(), j.as_str(), r.as_str()))
            .collect();
        turn_part(dir.path(), "home", "personal", &spec, "");
        let one = |prompt: &str| {
            let call = Call {
                input: json!({"prompt": prompt}),
                ..call(Some("s"), Carrier::Append, false)
            };
            expression(&c, &call).unwrap()
        };
        let first = one("deploy0 deploy1 deploy2");
        for i in &first {
            assert!(
                i.text.len() <= RULES_BYTES + 200,
                "{} is {} bytes",
                i.id,
                i.text.len()
            );
            assert!(i.text.lines().count() <= RULES_LINES + 2, "{}", i.id);
        }
        assert!(first.iter().map(|i| i.text.len()).sum::<usize>() <= TURN_BYTES);
        assert_eq!(first.len(), 3, "all three fits");
        let second = one("deploy0 deploy1 deploy2");
        assert!(second.is_empty(), "already sent: nothing");
        let third = one("deploy3 deploy4 deploy0");
        let ids: Vec<&str> = third.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(
            ids,
            ["fit.home.big3", "fit.home.big4"],
            "what was sent takes no budget"
        );
    }

    #[test]
    fn the_budget_leaves_out_what_the_session_holds_before_it_trims() {
        let dir = TempDir::new().unwrap();
        let c = ctx(dir.path());
        let item = |id: &str| Item {
            id: id.into(),
            text: "x".repeat(60),
            data: Value::Null,
        };
        let append = call(Some("s"), Carrier::Append, false);
        let sent = deliver_within(&c, &append, vec![item("a")], 100).unwrap();
        assert_eq!(sent.len(), 1);
        let next = deliver_within(&c, &append, vec![item("a"), item("b")], 100).unwrap();
        let ids: Vec<&str> = next.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["b"], "the held item takes no budget");
        let over = deliver_within(&c, &append, vec![item("c"), item("d")], 100).unwrap();
        assert_eq!(over.len(), 1, "the second is over the budget");
        let later = deliver_within(&c, &append, vec![item("d")], 100).unwrap();
        assert_eq!(later.len(), 1, "and was not recorded, so it comes later");
    }

    #[test]
    fn the_same_rules_are_not_sent_again_when_another_word_fits_them() {
        let dir = TempDir::new().unwrap();
        let c = ctx(dir.path());
        turn_part(
            dir.path(),
            "home",
            "personal",
            &[("deployer", "deploy, release", "TAG FIRST")],
            "",
        );
        assert!(said(&c, Carrier::Append, "deploy it").contains(&"fit.home.deployer".to_string()));
        assert!(said(&c, Carrier::Append, "release it").is_empty());
    }

    #[test]
    fn a_name_that_is_not_a_plain_id_is_hashed_and_never_shared() {
        assert_eq!(fit_id("home", "notes"), "fit.home.notes");
        let (a, b) = (fit_id("home", "a.b"), fit_id("home", "a-b"));
        assert!(a.starts_with("fit.") && a != b, "{a} {b}");
        assert!(delivery::check_id("item", &fit_id("p", &"x".repeat(200))).is_ok());
    }

    #[test]
    fn rules_two_parts_share_do_not_use_up_the_cap() {
        let dir = TempDir::new().unwrap();
        let c = ctx(dir.path());
        turn_part(
            dir.path(),
            "a",
            "personal",
            &[
                ("one", "deploy", "SAME"),
                ("two", "deploy", "TWO"),
                ("three", "deploy", "THREE"),
            ],
            "",
        );
        turn_part(
            dir.path(),
            "b",
            "personal",
            &[("one", "deploy", "SAME")],
            "",
        );
        let call = Call {
            input: json!({"prompt": "deploy"}),
            ..call(None, Carrier::Transient, false)
        };
        let fits = expression(&c, &call)
            .unwrap()
            .into_iter()
            .filter(|i| i.id.starts_with("fit."))
            .count();
        assert_eq!(fits, 3);
    }

    #[test]
    fn a_sub_session_slice_keeps_who_and_what_and_drops_what_waits_for_the_person() {
        let dir = TempDir::new().unwrap();
        part(dir.path(), "a", "personal", json!([{"name": "docs"}]));
        part(dir.path(), "b", "work", json!([{"name": "docs"}]));
        let p = picture(&ctx(dir.path())).unwrap();
        let whole = render(&p);
        let sub = render_slice(&p, Slice::SubSession);
        for kept in [
            "personas: ",
            "acting through: ",
            "turn bring-ins:",
            "environment: 0 of 2",
        ] {
            assert!(sub.contains(kept), "{kept:?} in:\n{sub}");
        }
        for dropped in ["undeclared:", "clashes:", "pending:"] {
            assert!(whole.contains(dropped), "{dropped:?} in:\n{whole}");
            assert!(!sub.contains(dropped), "{dropped:?} not in:\n{sub}");
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
    fn the_session_comes_from_the_input_and_a_sub_agent_gets_its_own() {
        let from = |v: Value| Call::session_from_input(&v);
        assert_eq!(from(json!({"session_id": "s1"})).as_deref(), Some("s1"));
        assert_eq!(
            from(json!({"session_id": "s1", "agent_id": "a7"})).as_deref(),
            Some("s1.a7")
        );
        let long = from(json!({"session_id": "s1", "agent_id": "x/".repeat(80)})).unwrap();
        assert!(
            long.starts_with("s1.a-") && delivery::check_id("id", &long).is_ok(),
            "{long}"
        );
        assert_eq!(from(json!({"agent_id": "a7"})), None);
        assert_eq!(from(json!({"session_id": 3})), None);
        assert_eq!(from(Value::Null), None);
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
    fn an_end_event_marks_the_session_and_the_genome_counts_it_waiting() {
        let dir = TempDir::new().unwrap();
        let c = ctx(dir.path());
        let projects = crate::fixtures::claude_projects(dir.path());
        part(dir.path(), "a", "personal", json!([]));
        let s = crate::fixtures::SESSION;
        let end = Call {
            input: json!({"session_id": s, "hook_event_name": "SessionEnd", "cwd": "/work", "reason": "other",
                "transcript_path": projects.join(format!("-work/{s}.jsonl"))}),
            ..call(Some(s), Carrier::Append, false)
        };
        assert!(
            sleep(&c, &end).unwrap().is_empty(),
            "an ended session is told nothing"
        );
        let p = picture(&c).unwrap();
        assert_eq!(p.waiting, 1);
        assert!(render(&p).contains("  to reflect on: 1 sessions marked for sleep"));
        let listed = experience(&c, &call(None, Carrier::OnDemand, false)).unwrap();
        assert!(
            listed[0].text.contains(&format!("  {s} (end, ")),
            "{}",
            listed[0].text
        );
    }

    #[test]
    fn a_close_names_the_session_and_experience_packs_and_stores_it() {
        let dir = TempDir::new().unwrap();
        let c = ctx(dir.path());
        crate::fixtures::claude_projects(dir.path());
        let s = crate::fixtures::SESSION;
        let close = Call {
            input: json!({"cwd": "/work"}),
            ..call(Some(s), Carrier::OnDemand, false)
        };
        assert!(sleep(&c, &close).unwrap()[0].text.ends_with("(close)\n"));
        let got = experience(&c, &call(Some(s), Carrier::OnDemand, false)).unwrap();
        let stored = dir.path().join(format!("state/experience/{s}.json"));
        assert!(stored.is_file());
        assert!(
            got[0].text.contains("(4 by the person)")
                && got[0]
                    .text
                    .contains("friction: 1 interrupt, 1 refusal, 2 error, 1 retry")
                && got[0].text.contains("2 sub-sessions"),
            "{}",
            got[0].text
        );
        let none = experience(&c, &call(None, Carrier::OnDemand, false)).unwrap();
        assert!(none[0].text.contains("sessions to reflect on: 1"));
    }

    #[test]
    fn a_source_not_built_says_so() {
        let dir = TempDir::new().unwrap();
        let k = Call {
            input: json!({"source": "pi", "cwd": "/w"}),
            ..call(Some("s"), Carrier::OnDemand, false)
        };
        let e = sleep(&ctx(dir.path()), &k).unwrap_err();
        assert!(
            e.to_string().contains("no session source for pi yet"),
            "{e}"
        );
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
