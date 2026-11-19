//! Essentials: what a turn keeps in reach. The acting persona and its
//! memory, the goal and next step from the work's own record, and the
//! first rules of each turn bring-in that fits (at most `fit::CAP`). Each
//! has its full item, sent when it changes, and its short form, sent again
//! once drifted (`Reinforcement`). See the initiative spec `harness-core`,
//! "Each turn: expression".

use crate::call::Item;
use crate::expression::fit::{self, Fit};
use crate::expression::here::Here;
use crate::expression::reinforcement::{self, Short};
use crate::expression::{clip, delivery, one_line, record, FIELD_BYTES, TAG};
use ghost::contract::{self, Scope};
use ghost::{BringIn, Ghost};
use std::collections::BTreeSet;
use std::fmt::Write as _;

/// No one item may take most of a turn: a rules section is cut to these.
pub const RULES_LINES: usize = 30;
pub const RULES_BYTES: usize = 2_000;

pub const PERSONA: &str = "reinforce.persona";
pub const RECORD: &str = "reinforce.record";

/// One essential: its full item, when there is one to send, and its short
/// form, when it has one.
#[derive(Debug, Clone)]
pub struct Essential {
    pub full: Option<Item>,
    pub short: Option<Short>,
}

/// A turn's essentials, persona first, then the record, then the fits.
pub struct Essentials<'a> {
    pub acting: Option<String>,
    pub all: Vec<Essential>,
    /// The bring-ins that fit, in order.
    pub fits: Vec<&'a BringIn>,
    /// Bring-ins that fit past the cap, best first: candidates to offer.
    pub past_cap: Vec<&'a BringIn>,
    /// Every bring-in that fits, sent or not.
    pub matched: Vec<&'a BringIn>,
}

impl<'a> Essentials<'a> {
    /// The essentials of a turn on `prompt` at `here`. `told` says whether
    /// the session was told an item before, so leaving a persona or a
    /// record is said once, and only to a session that was told one.
    pub fn of(
        ghost: &'a Ghost,
        here: &Here,
        prompt: &str,
        told: &dyn Fn(&str) -> bool,
    ) -> Essentials<'a> {
        let acting = fit::persona(ghost, here);
        let record = work_record(ghost, here);
        let mut all = vec![
            persona(ghost, acting.as_deref(), told(PERSONA)),
            work(record.as_ref(), told(RECORD)),
        ];
        let state = record.as_ref().and_then(|r| r.state.as_deref());
        let mut seen_rules = BTreeSet::new();
        let (mut fits, mut past_cap) = (vec![], vec![]);
        let matched: Vec<&BringIn> = fit::ranked(ghost, prompt, state, here)
            .into_iter()
            .map(|Fit { bring_in, .. }| bring_in)
            .collect();
        for &b in &matched {
            let rules = rules_of(b);
            if let Ok(Some(text)) = &rules {
                if !seen_rules.insert(text.clone()) {
                    continue;
                }
            }
            // The cap counts fits sent, so rules two parts share do not use it.
            if fits.len() == fit::CAP {
                past_cap.push(b);
                continue;
            }
            fits.push(b);
            all.push(fitting(b, rules));
        }
        Essentials {
            acting,
            all: all
                .into_iter()
                .filter(|e| e.full.is_some() || e.short.is_some())
                .collect(),
            fits,
            past_cap,
            matched,
        }
    }

    /// The full items, in order.
    pub fn items(&self) -> Vec<Item> {
        self.all.iter().filter_map(|e| e.full.clone()).collect()
    }

    /// The item ids of the fits.
    pub fn fit_ids(&self) -> BTreeSet<String> {
        self.fits.iter().map(|b| fit_id(&b.part, &b.name)).collect()
    }
}

/// The first turn bring-in whose `record` names one here, whether or not
/// it fits this turn, since its state is part of what decides the fit.
fn work_record(ghost: &Ghost, here: &Here) -> Option<record::Record> {
    ghost
        .bring_ins
        .iter()
        .filter_map(|b| b.declaration())
        .filter(|d| d.scope == Scope::Turn && !d.record.is_empty())
        .find_map(|d| record::find(&d.record, here).ok().flatten())
}

/// The acting persona and its memory. In full only where it can change:
/// with one persona the genome has said all there is. Where none is
/// known, one line, and only to a session that was told one. The short
/// form says it whenever one is known, since the genome drifts too.
fn persona(ghost: &Ghost, acting: Option<&str>, told: bool) -> Essential {
    let several = ghost
        .personas
        .iter()
        .filter_map(|p| p.persona.as_deref())
        .collect::<BTreeSet<&str>>()
        .len()
        > 1;
    let Some(persona) = acting else {
        let full = (several && told).then(|| Item {
            id: PERSONA.into(),
            text: format!("{TAG} persona: none known here\n"),
            data: serde_json::json!({ "acting": null }),
        });
        return Essential { full, short: None };
    };
    let memory: Vec<String> = ghost
        .bring_ins
        .iter()
        .filter(|b| b.persona.as_deref() == Some(persona))
        .filter(|b| b.declaration().is_some_and(|d| d.scope == Scope::Session))
        .filter_map(|b| {
            let l = crate::genome::line(b);
            (!l.state.is_empty())
                .then(|| one_line(&format!("{} ({})", l.name, l.state.join(", ")), FIELD_BYTES))
        })
        .take(10)
        .collect();
    let persona = one_line(persona, FIELD_BYTES);
    let said = match memory.is_empty() {
        true => format!("acting through {persona}; no memory of its own"),
        false => format!(
            "acting through {persona}; its memory: {}",
            memory.join(", ")
        ),
    };
    Essential {
        full: several.then(|| Item {
            id: PERSONA.into(),
            text: format!("{TAG} {said}\n"),
            data: serde_json::json!({ "acting": persona }),
        }),
        short: Some(Short {
            id: PERSONA.into(),
            text: format!("- {said}\n"),
        }),
    }
}

/// The work's record: goal, state and next. Where there is none, one line,
/// and only to a session that was told one.
fn work(record: Option<&record::Record>, told: bool) -> Essential {
    let Some(r) = record else {
        let full = told.then(|| Item {
            id: RECORD.into(),
            text: format!("{TAG} record: none here\n"),
            data: serde_json::json!({ "record": null }),
        });
        return Essential { full, short: None };
    };
    let file = one_line(&r.file.display().to_string(), FIELD_BYTES);
    let mut text = format!("{TAG} the work's record: {file}\n");
    let mut short = format!("- the work's record: {file}\n");
    for (label, v) in [("goal", &r.goal), ("state", &r.state), ("next", &r.next)] {
        if let Some(v) = v {
            let v = one_line(v, FIELD_BYTES);
            let _ = writeln!(text, "  {label}: {v}");
            if label != "state" {
                let _ = writeln!(short, "    {label}: {v}");
            }
        }
    }
    Essential {
        full: Some(Item {
            id: RECORD.into(),
            text,
            data: serde_json::json!({ "record": r }),
        }),
        short: Some(Short {
            id: RECORD.into(),
            text: short,
        }),
    }
}

/// A fitting bring-in: its job and first rules in full, its name and first
/// rule lines short.
fn fitting(b: &BringIn, rules: Result<Option<String>, String>) -> Essential {
    let id = fit_id(&b.part, &b.name);
    let mut text = format!(
        "{TAG} fits this turn: {} ({}): {}\n",
        one_line(&b.name, FIELD_BYTES),
        one_line(b.persona.as_deref().unwrap_or(&b.part), FIELD_BYTES),
        one_line(b.job().unwrap_or("no job given"), FIELD_BYTES)
    );
    if let Ok(Some(r)) = &rules {
        text.push_str(r);
        text.push('\n');
    }
    let short = reinforcement::bring_in(
        id.clone(),
        &b.name,
        rules.as_ref().ok().and_then(|r| r.as_deref()).unwrap_or(""),
    );
    Essential {
        full: Some(Item {
            id,
            text,
            // Only what the bring-in is, not why this prompt fit it: the
            // record hashes the data too, and the same rules must not be sent
            // again because another word matched.
            data: serde_json::json!({"part": b.part, "name": b.name,
                "rules": rules.as_ref().err()}),
        }),
        short: Some(short),
    }
}

/// A bring-in's first rules, read from its unit through its `rules` pointer.
pub fn rules_of(b: &BringIn) -> Result<Option<String>, String> {
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

/// A fit's item id: its part and name, or a hash of them when either is
/// not plain enough or long enough to be an id as it is, so two names
/// never share one.
pub fn fit_id(part: &str, name: &str) -> String {
    part_id("fit", part, name)
}

/// An item id for a bring-in under `kind`: `<kind>.<part>.<name>`, or
/// `<kind>.<hash>` when that would not be a plain id.
pub fn part_id(kind: &str, part: &str, name: &str) -> String {
    let id = format!("{kind}.{part}.{name}");
    let plain = |x: &str| {
        x.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_".contains(c))
    };
    match plain(part) && plain(name) && delivery::check_id("item", &id).is_ok() {
        true => id,
        false => format!(
            "{kind}.{:016x}",
            delivery::fnv1a(&format!("{part}\0{name}"))
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_that_is_not_a_plain_id_is_hashed_and_never_shared() {
        assert_eq!(fit_id("home", "notes"), "fit.home.notes");
        let (a, b) = (fit_id("home", "a.b"), fit_id("home", "a-b"));
        assert!(a.starts_with("fit.") && a != b, "{a} {b}");
        assert!(delivery::check_id("item", &fit_id("p", &"x".repeat(200))).is_ok());
    }
}
