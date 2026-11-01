//! Adaptation, with the person's guards. The reflector only proposes
//! findings, as data; this module checks each one, routes it by the
//! harness map, keeps it pending until the person decides, and applies
//! only what they accept. See project.md "My harness", Adaptation.
//!
//! - habits (a persona's memory, a bring-in's tier) are written;
//! - nature (a bring-in, a skill, the harness) is only proposed;
//! - a borrowed bring-in gets a note for its lender, never a change;
//! - what cannot be placed on a persona is asked, never written.

use crate::config::Config;
use crate::contract::Promote;
use crate::delivery::check_id;
use crate::experience::{squash, Bundle, Who};
use crate::files::write_atomic;
use crate::map::{BringIn, Map};
use crate::memory;
use crate::parts::{Route, Source, Tenure};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The most findings one batch carries. The reflector is asked for at
/// most 7, so a reflector that runs a little over loses nothing; past
/// this the batch is no longer short.
pub const CAP: usize = 10;
/// The longest habit entry: one line the person reads whole in the batch.
pub const MAX_ENTRY: usize = 200;
/// A persona memory's types (project.md "Memory").
pub const TYPES: [&str; 4] = ["user", "feedback", "project", "reference"];
/// The shortest quote that counts as evidence.
pub const MIN_QUOTE: usize = 8;
/// What the reflector calls the core, its own agent and the reflect skill.
pub const HARNESS: &str = "harness";

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Habit,
    Nature,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TierName {
    Local,
    Permanent,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Op {
    #[default]
    Add,
    Remove,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Evidence {
    pub quote: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
}

/// One proposed change, as the reflector returns it.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    /// A bring-in (`<part>/<name>`, or its name when only one part has
    /// it), or `harness`. A habit may leave it out and name a persona.
    #[serde(default)]
    pub thing: Option<String>,
    pub kind: Kind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<TierName>,
    #[serde(default)]
    pub op: Op,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persona: Option<String>,
    /// The reflector is not sure which persona this is for.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub guess: bool,
    /// A memory's title and type (`user`, `feedback`, `project`,
    /// `reference`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub memory_type: Option<String>,
    /// One line: what changes.
    pub change: String,
    /// The text a habit writes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
    /// A concrete edit, for a nature finding worked in a worktree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit: Option<String>,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    /// What it contradicts, quoted from the memory or tier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contradicts: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
}

/// What the reflector returns.
#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct Reflection {
    #[serde(default)]
    pub findings: Vec<Finding>,
    /// Where the person corrected the agent, and about which thing.
    #[serde(default)]
    pub corrections: Vec<Correction>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Correction {
    #[serde(default)]
    pub at: Option<String>,
    #[serde(default)]
    pub thing: Option<String>,
}

/// How an accepted item is applied.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase", tag = "is")]
pub enum Action {
    /// A persona's memory, through its server.
    Memory {
        serve: String,
        /// The index's bound path, read to place the index line.
        index: PathBuf,
    },
    /// A bring-in's tier file.
    Tier { tier: String, path: PathBuf },
    /// A backlog item or a worktree to a PR, on the source.
    Proposal {
        source: Option<Source>,
        worktree: bool,
    },
    /// A note for the lender, or for whoever owns a report-only source.
    Note { to: String },
    /// Already in the tier: nothing to do.
    Already { tier: String },
    /// Not placed on a persona yet: the person names one.
    Unplaced,
}

/// Why an item is left out of "ok" and needs its own answer.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Need {
    /// Learned only from tool or web content.
    ToolContent,
    /// A quote is the agent's words, not the person's.
    NotOwnWords,
    Contradiction,
    /// A guess about which persona it is for.
    Persona,
}

/// One undecided item: a pending file.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Pending {
    pub session: String,
    pub n: usize,
    pub finding: Finding,
    /// The bring-in it changes (`<part>/<name>`), or `harness`.
    pub target: Option<String>,
    pub persona: Option<String>,
    pub action: Action,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub needs: Vec<Need>,
    /// The last time applying it failed, and why.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// For a move into a bring-in's text: what it moves.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub moves: Vec<Move>,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Dropped {
    pub change: String,
    pub why: String,
}

/// Every thing a finding may change, as the reflector is shown it.
pub fn targets(map: &Map) -> String {
    let mut s = String::new();
    for b in &map.bring_ins {
        let Some(d) = b.declaration() else {
            continue;
        };
        let tenure = match b.record.as_ref().map(|r| &r.tenure) {
            Some(Tenure::Borrowed { from, .. }) => format!("borrowed from {from}"),
            _ => "own".into(),
        };
        let route = b.route.map(route_name).unwrap_or("report");
        let _ = write!(
            s,
            "  {}: {}; {}, {}, route {route}",
            id(b),
            d.job,
            b.persona.as_deref().unwrap_or("no persona"),
            tenure
        );
        for (tier, key) in tiers(b) {
            let at = value(b, &key).unwrap_or("not bound");
            let kind = match is_memory(b) && tier == TierName::Permanent {
                true => "memory",
                false => "tier",
            };
            let _ = write!(s, "; {} {kind} {key} at {at}", tier_name(tier));
            if kind == "memory" {
                if let Some(index) = value(b, "index") {
                    let _ = write!(s, ", index {}", Path::new(at).join(index).display());
                }
            }
        }
        if let Some(st) = &d.state {
            if st.local.is_some() || st.permanent.is_some() {
                let _ = write!(s, " (promote {})", promote_name(st.promote));
            }
        }
        s.push('\n');
    }
    s.push_str("  harness: the core, the reflector and the reflect skill\n");
    s
}

fn route_name(r: Route) -> &'static str {
    match r {
        Route::Write => "write",
        Route::Backlog => "backlog",
        Route::Worktree => "worktree",
        Route::Report => "report",
    }
}

fn tier_name(t: TierName) -> &'static str {
    match t {
        TierName::Local => "local",
        TierName::Permanent => "permanent",
    }
}

fn promote_name(p: Promote) -> &'static str {
    match p {
        Promote::Auto => "auto",
        Promote::Propose => "propose",
        Promote::Off => "off",
    }
}

fn id(b: &BringIn) -> String {
    format!("{}/{}", b.part, b.name)
}

fn value<'a>(b: &'a BringIn, key: &str) -> Option<&'a str> {
    b.record
        .as_ref()
        .and_then(|r| r.values.get(key))
        .map(String::as_str)
}

fn tiers(b: &BringIn) -> Vec<(TierName, String)> {
    let Some(st) = b.declaration().and_then(|d| d.state.as_ref()) else {
        return vec![];
    };
    let mut out = vec![];
    if let Some(k) = &st.local {
        out.push((TierName::Local, k.clone()));
    }
    if let Some(k) = &st.permanent {
        out.push((TierName::Permanent, k.clone()));
    }
    out
}

/// A persona memory: a permanent tier with a memory index (project.md
/// "Memory").
fn is_memory(b: &BringIn) -> bool {
    b.declaration().is_some_and(|d| {
        d.state.as_ref().is_some_and(|s| s.permanent.is_some()) && d.config.contains_key("index")
    })
}

/// The bring-in a finding names: `<part>/<name>`, or a name only one part
/// records.
fn resolve<'a>(map: &'a Map, thing: &str) -> Option<&'a BringIn> {
    let declared = map.bring_ins.iter().filter(|b| b.declaration().is_some());
    match thing.split_once('/') {
        Some((part, name)) => declared
            .into_iter()
            .find(|b| b.part == part && b.name == name),
        None => {
            let hits: Vec<&BringIn> = declared.filter(|b| b.name == thing).collect();
            (hits.len() == 1).then(|| hits[0])
        }
    }
}

/// Check the reflection, and keep what passes as the session's pending
/// items, replacing any it had. Answers the kept items and what was
/// dropped.
pub fn check(
    config: &Config,
    map: &Map,
    bundle: &Bundle,
    reflection: Reflection,
) -> Result<Checked> {
    let session = &bundle.session;
    check_id("session id", session)?;
    let texts: Vec<(Who, String)> = bundle
        .texts()
        .into_iter()
        .map(|(w, t)| (w, squash(t)))
        .collect();
    let saved = moves::saved(map, bundle);
    let (mut kept, mut dropped) = (vec![], vec![]);
    for (i, f) in reflection.findings.into_iter().enumerate() {
        let drop = |why: String| Dropped {
            change: f.change.clone(),
            why,
        };
        if i >= CAP {
            dropped.push(drop(format!("over the batch cap of {CAP}")));
            continue;
        }
        // Who said each quote: every source it is found in.
        let mut sources: Vec<Vec<Who>> = vec![];
        let mut bad = None;
        if f.evidence.is_empty() {
            bad = Some("no evidence".to_string());
        }
        for e in &f.evidence {
            let q = squash(&e.quote);
            if q.chars().count() < MIN_QUOTE {
                bad = Some(format!("quote {:?} is too short", e.quote));
                break;
            }
            let whos: Vec<Who> = texts
                .iter()
                .filter(|(_, t)| t.contains(&q))
                .map(|(w, _)| *w)
                .collect();
            if whos.is_empty() {
                bad = Some(format!("quote {:?} is not in the session", e.quote));
                break;
            }
            sources.push(whos);
        }
        if let Some(why) = bad.or_else(|| shape(&f).err()) {
            dropped.push(drop(why));
            continue;
        }
        if let Some(s) = saved_already(map, &saved, &f) {
            dropped.push(drop(format!(
                "saved in memory this session ({} {})",
                s.memory, s.path
            )));
            continue;
        }
        match place(map, &f) {
            Err(why) => dropped.push(drop(why)),
            Ok((target, persona, action, mut needs)) => {
                // Only the person's own words make a habit without its
                // own yes. Every quote, so one harmless quote of the person
                // cannot carry the rest.
                if f.kind == Kind::Habit {
                    needs.extend(not_own_words(&sources));
                }
                if f.contradicts.is_some() {
                    needs.push(Need::Contradiction);
                }
                kept.push(Pending {
                    session: session.clone(),
                    n: kept.len() + 1,
                    finding: f,
                    target,
                    persona,
                    action,
                    needs,
                    error: None,
                    moves: vec![],
                });
            }
        }
    }
    // What a decided move moved, or another session's move waits with,
    // is not offered here.
    let mut held = crate::ledger::moved(config)?;
    held.extend(waiting_moves(config, session)?);
    for (f, moves) in moves::items(map, &saved, &held) {
        match place(map, &f) {
            Err(why) => dropped.push(Dropped {
                change: f.change.clone(),
                why,
            }),
            Ok((target, persona, action, needs)) => kept.push(Pending {
                session: session.clone(),
                n: kept.len() + 1,
                finding: f,
                target,
                persona,
                action,
                needs,
                error: None,
                moves,
            }),
        }
    }
    clear(config, session)?;
    for p in &kept {
        save(config, p)?;
    }
    Ok(Checked {
        kept,
        dropped,
        saved,
    })
}

/// The memory this session already saved that a habit would add again:
/// on the memory it goes to (its persona's, when it names none), the same
/// file, name or text.
fn saved_already<'a>(map: &Map, saved: &'a [Saved], f: &Finding) -> Option<&'a Saved> {
    if f.kind != Kind::Habit || f.op != Op::Add {
        return None;
    }
    let memory = match f.thing.as_deref().filter(|t| !t.is_empty()) {
        Some(t) => Some(id(resolve(map, t).filter(|b| is_memory(b))?)),
        None => memory_of(map, f.persona.as_deref()).map(id),
    };
    let file = f
        .title
        .as_deref()
        .map(|t| format!("{}/{}.md", memory::ROOT, slug(t)));
    let same = |a: &str, b: &str| {
        let a = squash(a).to_lowercase();
        !a.is_empty() && a == squash(b).to_lowercase()
    };
    let texts: Vec<&str> = [f.entry.as_deref(), Some(f.change.as_str())]
        .into_iter()
        .flatten()
        .collect();
    saved.iter().find(|s| {
        let on = match &memory {
            Some(m) => *m == s.memory,
            // With no persona either, the person places it, so it stays.
            None => f.persona.is_some() && s.persona == f.persona,
        };
        on && (file.as_deref() == Some(s.path.as_str())
            || f.title
                .as_deref()
                .zip(s.name.as_deref())
                .is_some_and(|(a, b)| same(a, b))
            || texts.iter().any(|t| same(t, &s.rule) || same(t, &s.body)))
    })
}

/// What a reflection's check keeps, drops, and found saved in memory.
pub struct Checked {
    pub kept: Vec<Pending>,
    pub dropped: Vec<Dropped>,
    pub saved: Vec<Saved>,
}

/// What the batch shows and a write puts in later context is one plain
/// line each, so the person sees all of it.
fn shape(f: &Finding) -> Result<(), String> {
    for (name, v) in [
        ("change", Some(f.change.as_str())),
        ("entry", f.entry.as_deref()),
        ("title", f.title.as_deref()),
        ("type", f.memory_type.as_deref()),
    ] {
        if v.is_some_and(|v| v.contains(['\n', '\r'])) {
            return Err(format!("its {name} spans more than one line"));
        }
    }
    if f.kind == Kind::Habit
        && f.entry
            .as_deref()
            .is_some_and(|e| e.chars().count() > MAX_ENTRY)
    {
        return Err(format!("its entry is longer than {MAX_ENTRY} characters"));
    }
    if f.title.as_deref().is_some_and(|t| t.contains(['[', ']'])) {
        return Err("its title holds [ or ], which an index line cannot carry".into());
    }
    if f.change.contains("](") {
        return Err("its change holds ](, which would read as an index link".into());
    }
    match f.memory_type.as_deref() {
        Some(t) if !TYPES.contains(&t) => {
            Err(format!("type {t} is not one of {}", TYPES.join(", ")))
        }
        _ => Ok(()),
    }
}

/// Why a habit's quotes are not all the person's: a quote also found in
/// tool or told text is tool content (the agent may only be repeating
/// it); one found only in the agent's words is not the person's.
fn not_own_words(sources: &[Vec<Who>]) -> Option<Need> {
    let others: Vec<&Vec<Who>> = sources
        .iter()
        .filter(|w| !w.contains(&Who::Person))
        .collect();
    if others.is_empty() {
        None
    } else if others
        .iter()
        .any(|w| w.iter().any(|x| matches!(x, Who::Tool | Who::Told)))
    {
        Some(Need::ToolContent)
    } else {
        Some(Need::NotOwnWords)
    }
}

type Placed = (Option<String>, Option<String>, Action, Vec<Need>);

/// Where a finding goes, by the map; `Err` says why it goes nowhere.
fn place(map: &Map, f: &Finding) -> Result<Placed, String> {
    let thing = f.thing.as_deref().filter(|t| !t.is_empty());
    if thing == Some(HARNESS) {
        if f.kind == Kind::Habit {
            return Err("the harness has no tier; a change to it is nature".into());
        }
        // The harness (the core, the reflector, the reflect skill) ships
        // with the part that ships the core's adapter, so that is its
        // source.
        let adapter = map
            .bring_ins
            .iter()
            .find(|b| b.name == "claude-adapter")
            .and_then(|b| b.record.as_ref())
            .and_then(|r| r.source.clone());
        let worktree = f.route.as_deref() == Some("worktree") && f.edit.is_some();
        return Ok((
            Some(HARNESS.into()),
            None,
            Action::Proposal {
                source: adapter,
                worktree,
            },
            vec![],
        ));
    }
    let b = match thing {
        Some(t) => {
            Some(resolve(map, t).ok_or_else(|| format!("{t} names nothing in the harness"))?)
        }
        None => None,
    };
    let Some(b) = b.or_else(|| memory_of(map, f.persona.as_deref())) else {
        return match f.kind {
            Kind::Habit => Ok((
                None,
                f.persona.clone(),
                Action::Unplaced,
                vec![Need::Persona],
            )),
            Kind::Nature => Err("names nothing in the harness".into()),
        };
    };
    let mut needs = vec![];
    if f.guess
        || f.persona
            .as_deref()
            .is_some_and(|p| Some(p) != b.persona.as_deref())
    {
        needs.push(Need::Persona);
    }
    let record = b.record.as_ref();
    if let Some(Tenure::Borrowed { from, .. }) = record.map(|r| &r.tenure) {
        return Ok((
            Some(id(b)),
            b.persona.clone(),
            Action::Note { to: from.clone() },
            needs,
        ));
    }
    let action = match f.kind {
        Kind::Nature => match b.route {
            Some(Route::Report) | None => Action::Note {
                to: "the owner".into(),
            },
            Some(route) => Action::Proposal {
                source: record.and_then(|r| r.source.clone()),
                worktree: route == Route::Worktree
                    && f.route.as_deref() == Some("worktree")
                    && f.edit.is_some(),
            },
        },
        Kind::Habit => habit(b, f)?,
    };
    Ok((Some(id(b)), b.persona.clone(), action, needs))
}

/// The one memory of a persona, when exactly one part's memory is theirs.
fn memory_of<'a>(map: &'a Map, persona: Option<&str>) -> Option<&'a BringIn> {
    let persona = persona?;
    let hits: Vec<&BringIn> = map
        .bring_ins
        .iter()
        .filter(|b| is_memory(b) && b.persona.as_deref() == Some(persona))
        .collect();
    (hits.len() == 1).then(|| hits[0])
}

fn habit(b: &BringIn, f: &Finding) -> Result<Action, String> {
    if f.entry.as_deref().is_none_or(|e| e.trim().is_empty()) {
        return Err("a habit with no entry to write".into());
    }
    let tiers = tiers(b);
    let want = f.tier.unwrap_or(match is_memory(b) {
        true => TierName::Permanent,
        false => TierName::Local,
    });
    let Some((_, key)) = tiers.iter().find(|(t, _)| *t == want) else {
        return Err(format!("{} has no {} tier", id(b), tier_name(want)));
    };
    let Some(at) = value(b, key) else {
        return Err(format!("{}'s {key} tier is not bound", id(b)));
    };
    if is_memory(b) && want == TierName::Permanent {
        let serve = value(b, "serve").ok_or_else(|| {
            format!(
                "{}'s serve is not bound, so its memory cannot be written",
                id(b)
            )
        })?;
        memory::argv(serve).map_err(|e| e.to_string())?;
        let index = value(b, "index").ok_or_else(|| format!("{}'s index is not bound", id(b)))?;
        let index = contained(at, index)?;
        let title = f
            .title
            .as_deref()
            .filter(|t| !t.trim().is_empty())
            .ok_or("a memory with no title")?;
        let slug = slug(title);
        if f.op == Op::Add && index_has(&index, &slug) {
            return Ok(Action::Already { tier: key.clone() });
        }
        let file = index.with_file_name(format!("{slug}.md"));
        if f.op == Op::Add && file.exists() {
            return Err(format!(
                "a memory file {slug}.md is there but not in the index (remove it from the store, or answer no)"
            ));
        }
        return Ok(Action::Memory {
            serve: serve.into(),
            index,
        });
    }
    let promote = b
        .declaration()
        .and_then(|d| d.state.as_ref())
        .map(|s| s.promote)
        .unwrap_or_default();
    if want == TierName::Permanent && promote == Promote::Off {
        return Err(format!("{}'s permanent tier takes no promotion", id(b)));
    }
    let path = PathBuf::from(at);
    if !path.is_absolute() {
        return Err(format!("{}'s {key} tier is not a file path", id(b)));
    }
    if path.is_dir() {
        return Err(format!("{}'s {key} tier is a directory", id(b)));
    }
    let entry = f.entry.as_deref().unwrap_or_default();
    if f.op == Op::Add {
        // Seen in either tier, it is already learned.
        for (_, k) in &tiers {
            if value(b, k).is_some_and(|p| tier_has(Path::new(p), entry)) {
                return Ok(Action::Already { tier: k.clone() });
            }
        }
    }
    Ok(Action::Tier {
        tier: key.clone(),
        path,
    })
}

/// The index path: `<dir>/<file>` under the tier's location, since the
/// server serves that one dir as the memory tool's root, and the index
/// and every memory sit directly in it.
fn contained(at: &str, index: &str) -> Result<PathBuf, String> {
    let rel = Path::new(index);
    let parts: Vec<_> = rel.components().collect();
    if rel.is_absolute()
        || parts.len() != 2
        || parts
            .iter()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(format!("index {index} is not <dir>/<file> in its store"));
    }
    Ok(Path::new(at).join(rel))
}

fn index_has(index: &Path, slug: &str) -> bool {
    fs::read_to_string(index).is_ok_and(|t| t.contains(&format!("]({slug}.md)")))
}

/// A tier file's entry text, without its `- <date> · ` lead. A line with
/// no date (written by hand) is still an entry.
fn entry_text(line: &str) -> Option<String> {
    let rest = line.strip_prefix("- ")?;
    let rest = match rest.split_once(" · ") {
        Some((date, r)) if date.len() == 10 && date.as_bytes()[4] == b'-' => r,
        _ => rest,
    };
    Some(squash(rest).to_lowercase())
}

/// Whether a tier line's entry is this one: the whole text, or the whole
/// text after a promoted rule's `<Section>: ` lead.
fn same_entry(line: &str, want: &str) -> bool {
    entry_text(line)
        .is_some_and(|e| e == want || e.split_once(": ").is_some_and(|(_, rest)| rest == want))
}

fn tier_has(path: &Path, entry: &str) -> bool {
    let want = squash(entry).to_lowercase();
    fs::read_to_string(path).is_ok_and(|t| t.lines().any(|l| same_entry(l, &want)))
}

/// A file name from a title: lowercase letters and digits, `-` between.
pub fn slug(title: &str) -> String {
    let s: String = title
        .chars()
        .map(|c| match c.is_ascii_alphanumeric() {
            true => c.to_ascii_lowercase(),
            false => '-',
        })
        .collect();
    let s = s
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    match s.is_empty() {
        true => "memory".into(),
        false => s.chars().take(60).collect(),
    }
}

fn pending_path(config: &Config, session: &str, n: usize) -> PathBuf {
    config.pending().join(format!("{session}-{n}.json"))
}

fn save(config: &Config, p: &Pending) -> Result<()> {
    let at = pending_path(config, &p.session, p.n);
    write_atomic(&at, serde_json::to_string_pretty(p)?.as_bytes())
        .with_context(|| format!("cannot write {}", at.display()))
}

/// The session's undecided items, by number.
pub fn pending(config: &Config, session: &str) -> Result<Vec<Pending>> {
    check_id("session id", session)?;
    let dir = config.pending();
    let rd = match fs::read_dir(&dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e).with_context(|| format!("cannot list {}", dir.display())),
    };
    let prefix = format!("{session}-");
    let mut out = vec![];
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some(n) = name
            .strip_prefix(&prefix)
            .and_then(|r| r.strip_suffix(".json"))
            .and_then(|n| n.parse::<usize>().ok())
        else {
            continue;
        };
        let text = fs::read_to_string(e.path())?;
        let p: Pending = serde_json::from_str(&text)
            .with_context(|| format!("{} is not a pending item", e.path().display()))?;
        if p.n == n && p.session == *session {
            out.push(p);
        }
    }
    out.sort_by_key(|p| p.n);
    Ok(out)
}

/// The keys of every move waiting in another session's batch.
fn waiting_moves(config: &Config, session: &str) -> Result<Vec<String>> {
    let dir = config.pending();
    let rd = match fs::read_dir(&dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e).with_context(|| format!("cannot list {}", dir.display())),
    };
    let mut out = vec![];
    for e in rd.flatten() {
        let Ok(text) = fs::read_to_string(e.path()) else {
            continue;
        };
        if let Ok(p) = serde_json::from_str::<Pending>(&text) {
            if p.session != session {
                out.extend(p.moves.into_iter().map(|m| m.key));
            }
        }
    }
    Ok(out)
}

fn clear(config: &Config, session: &str) -> Result<()> {
    for p in pending(config, session)? {
        remove(&pending_path(config, session, p.n))?;
    }
    Ok(())
}

fn remove(p: &Path) -> io::Result<()> {
    match fs::remove_file(p) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// The decision batch: what this session saved in memory, one line per
/// item, then what was dropped.
pub fn batch(session: &str, saved: &[Saved], items: &[Pending], dropped: &[Dropped]) -> String {
    let mut s = String::new();
    match items.len() {
        0 => {
            let _ = writeln!(s, "Reflection on {session}: nothing to decide.");
        }
        n => {
            let _ = writeln!(
                s,
                "Reflection on {session}: {n} to decide. Answer \"ok\" for every item not marked \"answer it\", or per item (\"1 yes, 2 no\"), or \"none\", or \"skip\" to leave this session out."
            );
        }
    }
    // Each item and each drop is one line, whatever its texts hold, so no
    // text can pass for another item.
    if !saved.is_empty() {
        let _ = writeln!(
            s,
            "  saved in memory this session (it stays there until you remove it):"
        );
        for v in saved {
            let who = v
                .persona
                .as_deref()
                .map(|x| format!(" [{x}]"))
                .unwrap_or_default();
            let thing = v
                .thing
                .as_deref()
                .map(|t| format!("{t}, in "))
                .unwrap_or_default();
            let _ = writeln!(
                s,
                "    {}",
                squash(&format!(
                    "{thing}{}{who} {}: \"{}\"",
                    v.memory, v.path, v.rule
                ))
            );
        }
    }
    for p in items {
        let _ = writeln!(s, "  {}. {}", p.n, squash(&line(p)));
    }
    if !dropped.is_empty() {
        let _ = writeln!(s, "  dropped {}:", dropped.len());
        for d in dropped {
            let _ = writeln!(s, "    {}", squash(&format!("{}: {}", d.change, d.why)));
        }
    }
    s
}

fn line(p: &Pending) -> String {
    let f = &p.finding;
    let thing = p.target.as_deref().unwrap_or("no thing named");
    let who = p
        .persona
        .as_deref()
        .map(|x| format!(" [{x}]"))
        .unwrap_or_default();
    let op = match f.op {
        Op::Add => "add",
        Op::Remove => "remove",
    };
    let what = f.entry.as_deref().unwrap_or(&f.change);
    let head = match &p.action {
        Action::Memory { .. } => format!(
            "habit, {thing} memory{who}: {op} {} \"{}\" ({}): \"{what}\"",
            f.memory_type.as_deref().unwrap_or("feedback"),
            f.title.as_deref().unwrap_or("memory"),
            f.change.lines().next().unwrap_or_default().trim()
        ),
        Action::Tier { tier, .. } => format!("habit, {thing} {tier}{who}: {op} \"{what}\""),
        Action::Already { tier } => {
            format!("habit, {thing}{who}: \"{what}\", already in {tier}, no action")
        }
        Action::Proposal { source, worktree } => {
            let how = match worktree {
                true => "worktree to a PR",
                false => "backlog item",
            };
            let on = source_at(source.as_ref());
            format!("{how}, {thing} (on {on}): {}", f.change)
        }
        Action::Note { to } => format!("note for {to}, {thing}: {}", f.change),
        Action::Unplaced => format!("habit, no persona placed{who}: {op} \"{what}\""),
    };
    let quote = f
        .evidence
        .first()
        .map(|e| format!("; evidence \"{}\"", e.quote))
        .unwrap_or_default();
    let mut tags: Vec<&str> = p
        .needs
        .iter()
        .map(|n| match n {
            Need::ToolContent => "answer it: learned only from tool or web content",
            Need::NotOwnWords => "answer it: a quote is not your own words",
            Need::Contradiction => "answer it: contradicts what is there",
            Need::Persona => "answer it: which persona?",
        })
        .collect();
    if let Some(e) = &p.error {
        tags.push(e);
    }
    let tags = match tags.is_empty() {
        true => String::new(),
        false => format!(" ({})", tags.join("; ")),
    };
    format!("{head}{quote}{tags}")
}

/// What the person answered.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    Yes,
    No,
    Persona(String),
}

/// The person's decisions: `"ok"`, `"none"`, or `{"<n>": "yes"|"no"|
/// {"persona": "<name>"}}`.
pub fn answers(items: &[Pending], decide: &Value) -> Result<Vec<(usize, Answer)>> {
    match decide {
        Value::String(s) if s == "ok" => Ok(items
            .iter()
            .filter(|p| p.needs.is_empty() && p.action != Action::Unplaced)
            .map(|p| (p.n, Answer::Yes))
            .collect()),
        Value::String(s) if s == "none" => Ok(items.iter().map(|p| (p.n, Answer::No)).collect()),
        Value::String(s) if s == "skip" => {
            bail!("skip dismisses the session; it is not a decision")
        }
        Value::Object(m) => {
            let mut out = m
                .iter()
                .map(|(k, v)| {
                    let n: usize = k
                        .parse()
                        .with_context(|| format!("{k} is not an item number"))?;
                    let a = match v {
                        Value::String(s) if s == "yes" => Answer::Yes,
                        Value::String(s) if s == "no" => Answer::No,
                        Value::Object(o) => match o.get("persona").and_then(Value::as_str) {
                            Some(p) => Answer::Persona(p.into()),
                            None => bail!("item {n}: name a persona"),
                        },
                        other => bail!("item {n}: {other} is not yes, no or a persona"),
                    };
                    Ok((n, a))
                })
                .collect::<Result<Vec<_>>>()?;
            out.sort_by_key(|(n, _)| *n);
            Ok(out)
        }
        other => bail!("decide is \"ok\", \"none\", \"skip\" or per item, not {other}"),
    }
}

/// One item's outcome.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Outcome {
    pub n: usize,
    pub thing: String,
    /// `accepted`, `declined`, `already`, or `failed`.
    pub outcome: String,
    pub text: String,
}

/// Apply the person's answers. Decided items leave pending; one that fails
/// to apply stays, with why.
pub fn decide(config: &Config, map: &Map, session: &str, decide: &Value) -> Result<Vec<Outcome>> {
    let items = pending(config, session)?;
    let answered = answers(&items, decide)?;
    let bulk = decide.as_str() == Some("ok");
    // Every answer is checked before any is applied, so a bad one changes
    // nothing.
    for (n, answer) in &answered {
        if !items.iter().any(|p| p.n == *n) {
            bail!("no item {n} waits for session {session}");
        }
        if let Answer::Persona(name) = answer {
            if memory_of(map, Some(name)).is_none() {
                bail!("item {n}: persona {name} has no one memory to write to");
            }
        }
    }
    let mut out = vec![];
    for (n, answer) in answered {
        let mut p = items.iter().find(|p| p.n == n).cloned().expect("checked");
        let thing = p.target.clone().unwrap_or_else(|| "unplaced".into());
        let result = match &answer {
            Answer::No => Ok(("declined", "declined".to_string())),
            Answer::Yes | Answer::Persona(_) => {
                if let Answer::Persona(name) = &answer {
                    let b = memory_of(map, Some(name)).expect("checked");
                    p.finding.thing = Some(id(b));
                    p.finding.persona = Some(name.clone());
                }
                // Placed again on the map as it is now: a pending file
                // only records what was shown, and the part lists may
                // have changed since.
                match place(map, &p.finding) {
                    Ok((_, _, Action::Unplaced, _)) => {
                        out.push(Outcome {
                            n,
                            thing,
                            outcome: "waiting".into(),
                            text: "needs a persona: answer {\"persona\": \"<name>\"}".into(),
                        });
                        continue;
                    }
                    // An item "ok" took that needs its own answer now
                    // (the map changed since the batch) waits for it.
                    Ok((_, _, _, needs)) if bulk && !needs.is_empty() => {
                        out.push(Outcome {
                            n,
                            thing,
                            outcome: "waiting".into(),
                            text: "the harness changed since the batch: answer it on its own"
                                .into(),
                        });
                        continue;
                    }
                    Ok((target, persona, action, _)) => {
                        p.target = target;
                        p.persona = persona;
                        p.action = action;
                        apply(config, &p).map(|t| match p.action {
                            Action::Already { .. } => ("already", t),
                            _ => ("accepted", t),
                        })
                    }
                    Err(why) => Err(anyhow::anyhow!("{why}")),
                }
            }
        };
        match result {
            Ok((outcome, text)) => {
                remove(&pending_path(config, session, n))?;
                out.push(Outcome {
                    n,
                    thing: p.target.clone().unwrap_or(thing),
                    outcome: outcome.into(),
                    text,
                });
            }
            Err(e) => {
                p.error = Some(format!("{e:#}"));
                save(config, &p)?;
                out.push(Outcome {
                    n,
                    thing,
                    outcome: "failed".into(),
                    text: format!("{e:#}"),
                });
            }
        }
    }
    if pending(config, session)?.is_empty() {
        done(config, session)?;
    }
    Ok(out)
}

/// A session's batch is settled: its mark is decided and the full copy
/// of the session taken for the brief goes; the capped experience stays.
pub fn done(config: &Config, session: &str) -> Result<()> {
    crate::sleep::set_decided(config, session)?;
    remove(&config.briefed(session))?;
    Ok(())
}

/// Leave sessions out without applying anything: their undecided items
/// and full copies go, their marks stop waiting, and the ledger says so.
pub fn dismiss(config: &Config, sessions: &[String]) -> Result<()> {
    for s in sessions {
        check_id("session id", s)?;
        if crate::sleep::read(config, s)?.is_none() && pending(config, s)?.is_empty() {
            bail!("no session {s} is marked or waits for a decision");
        }
    }
    for s in sessions {
        clear(config, s)?;
        crate::sleep::set_decided(config, s)?;
        remove(&config.briefed(s))?;
        crate::ledger::dismissed(config, s)?;
    }
    Ok(())
}

fn apply(config: &Config, p: &Pending) -> Result<String> {
    let f = &p.finding;
    let entry = f.entry.as_deref().unwrap_or_default();
    match &p.action {
        Action::Already { tier } => Ok(format!("already in {tier}")),
        Action::Unplaced => bail!("not placed on a persona"),
        Action::Memory { serve, index } => write_memory(serve, index, f, &p.session),
        Action::Tier { tier, path } => {
            write_tier(path, tier, f.op, entry, &p.session)?;
            Ok(format!("{tier}: {}", path.display()))
        }
        Action::Proposal { source, worktree } => {
            let at = propose(config, p, source.as_ref(), *worktree)?;
            Ok(format!("proposal at {}", at.display()))
        }
        Action::Note { to } => {
            let at = note(config, p, to)?;
            Ok(format!("note at {}", at.display()))
        }
    }
}

/// A frontmatter value as a double-quoted scalar, which a JSON string is.
fn yaml(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_default()
}

fn write_memory(serve: &str, index: &Path, f: &Finding, session: &str) -> Result<String> {
    let argv = memory::argv(serve)?;
    let title = f.title.as_deref().unwrap_or("memory");
    let slug = slug(title);
    let file = format!("{}/{slug}.md", memory::ROOT);
    let index_name = index
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "MEMORY.md".into());
    let index_tool = format!("{}/{index_name}", memory::ROOT);
    let current = match fs::read_to_string(index) {
        Ok(t) => Some(t),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(e).with_context(|| format!("cannot read {}", index.display())),
    };
    let hook = f.change.lines().next().unwrap_or_default().trim();
    let line = format!("- [{title}]({slug}.md) - {hook}");
    let calls: Vec<(&str, Value)> = match f.op {
        Op::Add => {
            let kind = f.memory_type.as_deref().unwrap_or("feedback");
            let body = format!(
                "---\nname: {}\ndescription: {}\nmetadata:\n  type: {kind}\n---\n\n{}\n\nLearned in session {session}, accepted by the person.\n",
                yaml(title),
                yaml(hook),
                f.entry.as_deref().unwrap_or_default().trim()
            );
            let index_call = match &current {
                None => (
                    "memory_create",
                    json!({"path": index_tool, "file_text": format!("{line}\n")}),
                ),
                Some(t) => {
                    let lines = t.trim_end_matches('\n').split('\n').count();
                    let at = if t.is_empty() { 0 } else { lines };
                    (
                        "memory_insert",
                        json!({"path": index_tool, "insert_line": at, "insert_text": line}),
                    )
                }
            };
            // The memory first: an index line without its file would point
            // at nothing.
            vec![
                ("memory_create", json!({"path": file, "file_text": body})),
                index_call,
            ]
        }
        Op::Remove => {
            let old = current
                .as_deref()
                .and_then(|t| t.lines().find(|l| l.contains(&format!("]({slug}.md)"))))
                .map(|l| format!("{l}\n"));
            let mut calls = vec![];
            if let Some(old) = old {
                calls.push((
                    "memory_str_replace",
                    json!({"path": index_tool, "old_str": old, "new_str": ""}),
                ));
            }
            calls.push(("memory_delete", json!({"path": file})));
            calls
        }
    };
    let answers = memory::call(&argv, &calls)?;
    let errors: Vec<String> = answers.into_iter().filter_map(Result::err).collect();
    match errors.is_empty() {
        true => Ok(format!("memory {file}")),
        false => bail!("the memory server said: {}", errors.join("; ")),
    }
}

/// A bring-in's tier file is markdown: entries `- <date> · <entry>` under
/// its first `## ` heading, newest first. A new file gets the heading
/// `## <Key> rules`.
fn write_tier(path: &Path, key: &str, op: Op, entry: &str, session: &str) -> Result<()> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let mut k = key.chars();
            let head: String = k
                .next()
                .map(|c| c.to_uppercase().chain(k).collect())
                .unwrap_or_default();
            format!("## {head} rules\n")
        }
        Err(e) => return Err(e).with_context(|| format!("cannot read {}", path.display())),
    };
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    match op {
        Op::Add => {
            let new = [
                format!("- {} · {}", crate::time::today(), entry.trim()),
                format!("  · source: reflection on session {session}"),
            ];
            let at = match lines.iter().position(|l| l.starts_with("## ")) {
                // After the heading and the blank lines under it.
                Some(h) => {
                    let mut i = h + 1;
                    while lines.get(i).is_some_and(|l| l.trim().is_empty()) {
                        i += 1;
                    }
                    if i == h + 1 {
                        lines.insert(i, String::new());
                        i += 1;
                    }
                    i
                }
                None => lines.len(),
            };
            for (k, l) in new.into_iter().enumerate() {
                lines.insert(at + k, l);
            }
        }
        Op::Remove => {
            let want = squash(entry).to_lowercase();
            let Some(i) = lines.iter().position(|l| same_entry(l, &want)) else {
                bail!("{entry:?} is not in {}", path.display());
            };
            let mut j = i + 1;
            while lines.get(j).is_some_and(|l| l.starts_with("  ")) {
                j += 1;
            }
            lines.drain(i..j);
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    write_atomic(path, out.as_bytes()).with_context(|| format!("cannot write {}", path.display()))
}

fn quotes(f: &Finding) -> String {
    f.evidence
        .iter()
        .map(|e| match &e.at {
            Some(at) => format!("> \"{}\" ({at})\n", e.quote),
            None => format!("> \"{}\"\n", e.quote),
        })
        .collect()
}

/// Where a proposal goes, as the part list records it.
fn source_at(source: Option<&Source>) -> String {
    match source {
        Some(Source { at: Some(at), .. }) => at.clone(),
        Some(_) => "its source (no remote recorded)".into(),
        None => "its source (not recorded)".into(),
    }
}

/// A backlog-item shaped proposal, under the state dir, naming its source.
fn propose(
    config: &Config,
    p: &Pending,
    source: Option<&Source>,
    worktree: bool,
) -> Result<PathBuf> {
    let f = &p.finding;
    let thing = p.target.as_deref().unwrap_or(HARNESS);
    let name = slug(&f.change);
    let dir = config.proposals().join(slug(thing));
    let stem = format!("{}-{name}", crate::time::today());
    let at = (1..)
        .map(|k| match k {
            1 => dir.join(format!("{stem}.md")),
            k => dir.join(format!("{stem}-{k}.md")),
        })
        .find(|p| !p.exists())
        .expect("a free name");
    let on = format!(
        "{}{}",
        source_at(source),
        source
            .and_then(|s| s.rev.as_deref())
            .map(|r| format!(" at {r}"))
            .unwrap_or_default()
    );
    let how = match worktree {
        true => "Work it in a worktree on the source, into a PR.",
        false => "File it as a backlog item on the source.",
    };
    let mut text = format!("# Backlog: {name}\n\n## What\n\n{} ({thing})\n", f.change);
    if let Some(e) = &f.edit {
        // A fence longer than any backtick run in the edit, so the edit
        // cannot close it and add text outside it.
        let longest = e.split(|c| c != '`').map(str::len).max().unwrap_or(0);
        let fence = "`".repeat(longest.max(2) + 1);
        let _ = write!(text, "\nProposed edit:\n\n{fence}\n{e}\n{fence}\n");
    }
    let _ = write!(
        text,
        "\n## Why\n\nSeen in session {}, accepted by the person:\n\n{}\n## Open questions\n\n- Source: {on}. {how}\n",
        p.session,
        quotes(f)
    );
    if let Some(line) = moves::remove_line(&p.moves) {
        let _ = write!(text, "\n{line}\n");
    }
    write_atomic(&at, text.as_bytes()).with_context(|| format!("cannot write {}", at.display()))?;
    Ok(at)
}

/// Append to the lender's note.
fn note(config: &Config, p: &Pending, to: &str) -> Result<PathBuf> {
    let f = &p.finding;
    let at = config.notes().join(format!("{}.md", slug(to)));
    let mut text = match fs::read_to_string(&at) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => format!("# Notes for {to}\n"),
        Err(e) => return Err(e).with_context(|| format!("cannot read {}", at.display())),
    };
    let _ = write!(
        text,
        "\n## {}, {}\n\n{}\n\n{}",
        p.target.as_deref().unwrap_or("?"),
        crate::time::today(),
        f.change,
        quotes(f)
    );
    write_atomic(&at, text.as_bytes()).with_context(|| format!("cannot write {}", at.display()))?;
    Ok(at)
}

/// What the person sees after deciding.
pub fn applied(session: &str, out: &[Outcome], left: usize) -> String {
    let mut s = format!("Applied for {session}:\n");
    for o in out {
        let _ = writeln!(s, "  {}. {} ({}): {}", o.n, o.outcome, o.thing, o.text);
    }
    if left > 0 {
        let _ = writeln!(s, "  {left} still to decide");
    }
    s
}

/// The reflector's brief: where the experience is, what it may change,
/// what this session already decided, and the answer's shape.
pub fn brief(
    session: &str,
    experience: &Path,
    transcript: &str,
    map: &Map,
    decided: &[String],
) -> String {
    let mut s = format!(
        "Reflect on session {session}.\nExperience: {} (each entry's `at` is <file>:<line> in {transcript}, for the full text).\nTargets:\n{}",
        experience.display(),
        targets(map)
    );
    if !decided.is_empty() {
        let _ = writeln!(
            s,
            "Already decided for this session (do not propose again):"
        );
        for d in decided {
            let _ = writeln!(s, "  {d}");
        }
    }
    s.push_str(
        "Return one JSON object, nothing else: {\"findings\": [{\"thing\", \"kind\": \"habit\"|\"nature\", \"tier\": \"local\"|\"permanent\", \"op\": \"add\"|\"remove\", \"persona\", \"guess\", \"title\", \"type\", \"change\", \"entry\", \"edit\", \"evidence\": [{\"quote\", \"at\"}], \"contradicts\", \"route\": \"backlog\"|\"worktree\"}], \"corrections\": [{\"at\", \"thing\"}]}. At most 7 findings; quote the session word for word.\n",
    );
    s
}

mod moves;
pub use moves::{Move, Saved};

#[cfg(test)]
mod tests;
