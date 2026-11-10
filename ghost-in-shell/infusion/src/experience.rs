//! experience: one session and its sub-sessions as what was said, what
//! the agent was told, what it did, what it touched and where it
//! struggled. A session source packs it in full; the stored copy caps each
//! text and points at the line it came from, so the reflector reads what
//! happened and not a raw log. See the initiative spec `harness-core`,
//! "The five entry points".

pub mod source;

use crate::call::{Call, Infusion, Item};
use crate::experience::source::{Located, SessionSource};
use crate::expression::delivery::deliver;
use crate::files::write_atomic;
use crate::sleep;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::path::PathBuf;

/// The longest text the stored experience keeps of any one entry.
pub const CAP: usize = 400;

/// Where a text came from, which decides how far it is trusted.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Who {
    /// The person's own words.
    Person,
    /// The agent's words and actions.
    Agent,
    /// Standing text the agent was given: skills, hooks, summaries.
    Told,
    /// What a tool returned, the web included.
    Tool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Entry {
    /// `<file>:<line>`.
    pub at: String,
    pub who: Who,
    /// What sort of told text: skill, hook, compaction, system.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    pub text: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Did {
    pub at: String,
    pub tool: String,
    /// The call's input, as compact JSON.
    pub input: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Entry>,
    /// Its result was an error.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub failed: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Struggle {
    /// The person stopped the agent.
    Interrupt,
    /// The person refused a tool use.
    Refusal,
    /// A tool failed.
    Error,
    /// The agent repeated a call that had failed.
    Retry,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Friction {
    pub at: String,
    pub kind: Struggle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    pub text: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct Bundle {
    pub session: String,
    pub source: String,
    pub transcript: String,
    pub said: Vec<Entry>,
    pub told: Vec<Entry>,
    pub did: Vec<Did>,
    pub touched: Vec<String>,
    pub friction: Vec<Friction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sub_sessions: Vec<Bundle>,
    /// Lines the source could not read, counted rather than failing.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub skipped_lines: usize,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

impl Bundle {
    /// Every text in the session and its sub-sessions, with who it came
    /// from: what the evidence guard searches.
    pub fn texts(&self) -> Vec<(Who, &str)> {
        let mut out: Vec<(Who, &str)> = vec![];
        out.extend(self.said.iter().map(|e| (e.who, e.text.as_str())));
        out.extend(self.told.iter().map(|e| (e.who, e.text.as_str())));
        for d in &self.did {
            out.push((Who::Agent, d.input.as_str()));
            out.extend(d.result.iter().map(|r| (r.who, r.text.as_str())));
        }
        out.extend(self.friction.iter().map(|f| {
            let who = match f.kind {
                Struggle::Interrupt => Who::Person,
                Struggle::Retry => Who::Agent,
                Struggle::Refusal | Struggle::Error => Who::Tool,
            };
            (who, f.text.as_str())
        }));
        for s in &self.sub_sessions {
            out.extend(s.texts());
        }
        out
    }

    /// The copy that is stored and read: every text cut to `CAP`.
    pub fn capped(mut self) -> Bundle {
        for e in self.said.iter_mut().chain(self.told.iter_mut()) {
            e.truncated |= cut(&mut e.text);
        }
        for d in &mut self.did {
            d.truncated |= cut(&mut d.input);
            if let Some(r) = &mut d.result {
                r.truncated |= cut(&mut r.text);
            }
        }
        for f in &mut self.friction {
            f.truncated |= cut(&mut f.text);
        }
        self.sub_sessions = self.sub_sessions.into_iter().map(Bundle::capped).collect();
        self
    }

    /// Friction counted by kind and tool, sub-sessions included.
    pub fn struggles(&self) -> Vec<(Struggle, Option<&str>)> {
        let mut out: Vec<(Struggle, Option<&str>)> = self
            .friction
            .iter()
            .map(|f| (f.kind, f.tool.as_deref()))
            .collect();
        for s in &self.sub_sessions {
            out.extend(s.struggles());
        }
        out
    }
}

/// Cut `s` to `CAP` characters; whether it was cut.
fn cut(s: &mut String) -> bool {
    match s.char_indices().nth(CAP) {
        Some((i, _)) => {
            s.truncate(i);
            true
        }
        None => false,
    }
}

/// Collapse every run of whitespace to one space, so a quote matches
/// however the transcript wrapped it.
pub fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// What happened in a session: packs it, stores the experience, and says
/// where it is and what it holds. Naming no session, the sessions waiting
/// to be reflected on.
pub fn experience(ctx: &Infusion, call: &Call) -> Result<Vec<Item>> {
    let named = call.session.is_some() || call.at.transcript.is_some() || call.at.cwd.is_some();
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

/// Where a call's session is: its mark's transcript when it has one, else
/// what the call names, in the source the call names.
pub fn locate(ctx: &Infusion, call: &Call) -> Result<(Box<dyn SessionSource>, Located)> {
    let mut at = call.at.clone();
    if let Some(m) = match call.session.as_deref() {
        Some(s) => sleep::read(&ctx.config, s)?,
        None => None,
    } {
        if at.transcript.is_none() {
            at.transcript = Some(PathBuf::from(m.transcript));
        }
        if at.source.is_none() {
            at.source = Some(m.source);
        }
    }
    let src = ctx.sources.named(at.source.as_deref())?;
    let located = src.locate(call.session.as_deref(), &at)?;
    Ok((src, located))
}

/// Pack the call's session and store its capped experience: the session,
/// every text in full, and where the stored copy is.
pub fn pack(ctx: &Infusion, call: &Call) -> Result<(Located, Bundle, PathBuf)> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(who: Who, text: &str) -> Entry {
        Entry {
            at: "t:1".into(),
            who,
            kind: None,
            text: text.into(),
            truncated: false,
        }
    }

    #[test]
    fn capping_cuts_long_texts_at_a_char_boundary_and_marks_them() {
        let long = "é".repeat(CAP + 5);
        let b = Bundle {
            said: vec![entry(Who::Person, &long), entry(Who::Person, "short")],
            sub_sessions: vec![Bundle {
                told: vec![entry(Who::Told, &long)],
                ..Default::default()
            }],
            ..Default::default()
        }
        .capped();
        assert_eq!(b.said[0].text.chars().count(), CAP);
        assert!(b.said[0].truncated);
        assert!(!b.said[1].truncated);
        assert!(b.sub_sessions[0].told[0].truncated);
    }

    #[test]
    fn texts_name_who_said_them_across_sub_sessions() {
        let b = Bundle {
            said: vec![entry(Who::Person, "p")],
            did: vec![Did {
                failed: false,
                at: "t:2".into(),
                tool: "Read".into(),
                input: "{}".into(),
                truncated: false,
                result: Some(entry(Who::Tool, "r")),
            }],
            sub_sessions: vec![Bundle {
                said: vec![entry(Who::Agent, "a")],
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_eq!(
            b.texts(),
            [
                (Who::Person, "p"),
                (Who::Agent, "{}"),
                (Who::Tool, "r"),
                (Who::Agent, "a")
            ]
        );
    }

    #[test]
    fn squash_collapses_whitespace() {
        assert_eq!(squash("  a\n\tb  c "), "a b c");
    }
}
