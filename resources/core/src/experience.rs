//! Experience: one session and its sub-sessions as what was said, what the
//! agent was told, what it did, what it touched and where it struggled.
//! A session source packs it in full; the stored copy caps each text and
//! points at the line it came from, so the reflector reads what happened
//! and not a raw log. See project.md "My harness", Adaptation.

use serde::{Deserialize, Serialize};

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
