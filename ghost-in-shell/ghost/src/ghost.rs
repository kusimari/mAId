//! The ghost: every part's persona and bring-ins fused into one model,
//! each persona kept distinct. Built on every call from what the shell has
//! installed and each unit's declaration, never stored: a unit installed
//! later, by any means, joins the next call. See the initiative spec
//! `harness-core`, "What forms the ghost".

use crate::contract::{self, Declaration, Slot};
use crate::installed::Installed;
use crate::parts::{self, BringIn as Record, Route};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Persona {
    pub part: String,
    pub persona: Option<String>,
    /// Whether the part says what it means; without it, its persona is not
    /// known and none of its units is read for a declaration.
    pub declared: bool,
}

/// What the ghost knows of one bring-in's declaration.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase", tag = "is")]
pub enum Declared {
    Declared(Box<Declaration>),
    /// No declaration: the standard's own description stands in for `job`.
    Undeclared {
        description: Option<String>,
    },
    /// The record or the unit could not be taken; nothing else is known.
    Unreadable {
        why: String,
    },
}

impl From<Slot> for Declared {
    fn from(s: Slot) -> Declared {
        match s {
            Slot::Declared(d) => Declared::Declared(d),
            Slot::Undeclared { description } => Declared::Undeclared { description },
        }
    }
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct BringIn {
    pub part: String,
    pub persona: Option<String>,
    pub name: String,
    /// `None` when the record itself could not be read.
    pub record: Option<Record>,
    pub route: Option<Route>,
    pub declared: Declared,
}

impl BringIn {
    pub fn declaration(&self) -> Option<&Declaration> {
        match &self.declared {
            Declared::Declared(d) => Some(d.as_ref()),
            _ => None,
        }
    }

    /// What it is for: its declared job, else its standard's description.
    pub fn job(&self) -> Option<&str> {
        match &self.declared {
            Declared::Declared(d) => Some(&d.job),
            Declared::Undeclared { description } => description.as_deref(),
            Declared::Unreadable { .. } => None,
        }
    }
}

#[derive(Serialize, Debug, Clone, PartialEq, Default)]
pub struct Ghost {
    pub personas: Vec<Persona>,
    /// In part order, then the order each part lists them.
    pub bring_ins: Vec<BringIn>,
    /// Bring-in names more than one part brings in.
    pub clashes: Vec<String>,
    /// The shell's records it could not read the shape of, each with why.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub not_understood: Vec<String>,
}

impl Ghost {
    /// The ghost of what the shell has installed, as seen from `here`.
    pub fn from(installed: &dyn Installed, here: Option<&Path>) -> anyhow::Result<Ghost> {
        Ok(Ghost {
            not_understood: installed.not_understood(here),
            ..Ghost::of(&parts::of(installed, here)?)
        })
    }

    /// The ghost of parts already found.
    pub fn of(parts: &[parts::Part]) -> Ghost {
        let personas = parts
            .iter()
            .map(|p| Persona {
                part: p.part.clone(),
                persona: p.persona.clone(),
                declared: p.declared,
            })
            .collect();
        let bring_ins: Vec<BringIn> = parts
            .iter()
            .flat_map(|p| {
                p.bring_ins.iter().map(|b| {
                    let (name, record, declared) = match b {
                        Ok(r) => {
                            let (record, declared) = take(r);
                            (r.name.clone(), Some(record), declared)
                        }
                        Err((name, why)) => (
                            name.clone(),
                            None,
                            Declared::Unreadable { why: why.clone() },
                        ),
                    };
                    BringIn {
                        part: p.part.clone(),
                        persona: p.persona.clone(),
                        name,
                        route: record.as_ref().map(Record::route),
                        record,
                        declared,
                    }
                })
            })
            .collect();
        let mut seen: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for b in &bring_ins {
            seen.entry(&b.name).or_default().insert(&b.part);
        }
        let clashes = seen
            .into_iter()
            .filter(|(_, parts)| parts.len() > 1)
            .map(|(n, _)| n.to_string())
            .collect();
        Ghost {
            personas,
            bring_ins,
            clashes,
            not_understood: vec![],
        }
    }
}

/// Shown in place of a secret held as a value.
pub const HIDDEN: &str = "(secret, not shown)";

/// The record as the ghost keeps it, and its declaration. A secret lives
/// elsewhere and a value only names where (`env:<NAME>`); a registration
/// that holds a secret as written makes the bring-in unreadable, and the
/// value is never kept.
fn take(r: &Record) -> (Record, Declared) {
    let slot = match &r.unit {
        None => Slot::Undeclared { description: None },
        Some(u) => match contract::read(u.standard, &u.path) {
            Ok(s) => s,
            Err(why) => return (r.clone(), Declared::Unreadable { why }),
        },
    };
    let Slot::Declared(d) = &slot else {
        return (r.clone(), slot.into());
    };
    let mut record = r.clone();
    let mut literal = vec![];
    for (key, _) in d.config.iter().filter(|(_, k)| k.secret) {
        if let Some(v) = record
            .values
            .get_mut(key)
            .filter(|v| !v.starts_with("env:"))
        {
            *v = HIDDEN.into();
            literal.push(key.as_str());
        }
    }
    match literal.first() {
        Some(key) => (
            record,
            Declared::Unreadable {
                why: format!("config {key} is secret, so its value must be env:<NAME>"),
            },
        ),
        None => (record, slot.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::installed::{Kind, Unit};
    use serde_json::json;
    use std::fs;
    use tempfile::TempDir;

    fn part(v: serde_json::Value) -> parts::Part {
        parts::Part::from_json(&v).unwrap()
    }

    fn skill(dir: &Path, name: &str, meta: &str) -> String {
        let p = dir.join(format!("{name}.md"));
        fs::write(
            &p,
            format!("---\nname: {name}\ndescription: about {name}\nmetadata:\n  my-harness.contract: \"1\"\n{meta}---\n"),
        )
        .unwrap();
        p.display().to_string()
    }

    #[test]
    fn two_parts_keep_their_personas_and_bring_ins_apart() {
        let dir = TempDir::new().unwrap();
        let notes = skill(
            dir.path(),
            "notes",
            "  my-harness.scope: turn\n  my-harness.job: capture notes\n",
        );
        let g = Ghost::of(&[
            part(json!({"part": "home", "persona": "personal",
                "bring_ins": [{"name": "notes", "unit": {"standard": "skill", "path": notes}}]})),
            part(json!({"part": "job", "persona": "work", "bring_ins": [{"name": "triage"}]})),
        ]);
        let personas: Vec<_> = g
            .personas
            .iter()
            .map(|p| (p.part.as_str(), p.persona.as_deref()))
            .collect();
        assert_eq!(
            personas,
            [("home", Some("personal")), ("job", Some("work"))]
        );
        assert_eq!(g.bring_ins[0].job(), Some("capture notes"));
        assert_eq!(
            g.bring_ins[1].declared,
            Declared::Undeclared { description: None }
        );
        assert!(g.clashes.is_empty());
    }

    #[test]
    fn the_same_bring_in_from_two_parts_is_kept_twice_and_a_clash() {
        let g = Ghost::of(&[
            part(json!({"part": "a", "bring_ins": [{"name": "mem"}]})),
            part(json!({"part": "b", "bring_ins": [{"name": "mem"}, {"name": "other"}]})),
        ]);
        assert_eq!(g.bring_ins.iter().filter(|b| b.name == "mem").count(), 2);
        assert_eq!(g.clashes, ["mem"]);
    }

    #[test]
    fn a_secret_held_as_a_value_makes_that_bring_in_unreadable_and_is_never_echoed() {
        let dir = TempDir::new().unwrap();
        let s = skill(dir.path(), "s", "  my-harness.scope: session\n  my-harness.job: j\n  my-harness.config.token: \"[secret] t\"\n");
        let g = Ghost::of(&[part(json!({"part": "a", "persona": "p", "bring_ins": [
            {"name": "leaky", "unit": {"standard": "skill", "path": s}, "values": {"token": "hunter2"}},
            {"name": "fine", "unit": {"standard": "skill", "path": s}, "values": {"token": "env:TOKEN"}}]}))]);
        match &g.bring_ins[0].declared {
            Declared::Unreadable { why } => assert!(
                why.contains("env:<NAME>") && !why.contains("hunter2"),
                "{why}"
            ),
            other => panic!("{other:?}"),
        }
        assert!(g.bring_ins[1].declaration().is_some());
        assert!(!serde_json::to_string(&g).unwrap().contains("hunter2"));
    }

    #[test]
    fn an_unreadable_record_or_unit_is_kept_with_why() {
        let g = Ghost::of(&[part(json!({"part": "a", "bring_ins": [
            {"name": "bad", "route": "sometimes"},
            {"name": "gone", "unit": {"standard": "skill", "path": "/nowhere/SKILL.md"}}]}))]);
        assert!(g.bring_ins[0].record.is_none());
        assert!(matches!(
            g.bring_ins[0].declared,
            Declared::Unreadable { .. }
        ));
        assert!(
            matches!(&g.bring_ins[1].declared, Declared::Unreadable { why } if why.contains("cannot read"))
        );
    }

    #[test]
    fn the_ghost_is_built_from_what_is_installed() {
        let dir = TempDir::new().unwrap();
        let d = dir.path().join("garden");
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("SKILL.md"), "---\nname: garden\ndescription: d\nmetadata:\n  my-harness.contract: \"2\"\n  my-harness.scope: turn\n  my-harness.job: j\n---\n").unwrap();
        let mut u = Unit::new(Kind::Skill, "garden");
        u.manifest = Some(d.join("SKILL.md"));
        let g = Ghost::from(&vec![u], None).unwrap();
        match &g.bring_ins[0].declared {
            Declared::Unreadable { why } => assert!(why.contains("contract 2"), "{why}"),
            other => panic!("an unknown contract is refused and named: {other:?}"),
        }
    }
}
