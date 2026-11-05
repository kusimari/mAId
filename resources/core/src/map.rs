//! Fusion: the harness map, every part's persona and bring-ins in one
//! model, each persona kept distinct. Derived on every call from the part
//! lists and each unit's slot, never stored: a part that installs later
//! cannot call the core to refresh a stored copy.

use crate::contract::{self, Declaration, Slot};
use crate::parts::{self, BringIn as Record, Refused, Route};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::Path;

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Persona {
    pub part: String,
    pub persona: Option<String>,
}

/// What the map knows of one bring-in's declaration.
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
pub struct Map {
    pub personas: Vec<Persona>,
    /// In part order, then the order each part lists them.
    pub bring_ins: Vec<BringIn>,
    /// Bring-in names more than one part records.
    pub clashes: Vec<String>,
    pub refused: Vec<Refused>,
}

/// Fuse every part list in `dir`.
pub fn fuse(dir: &Path) -> io::Result<Map> {
    let (parts, refused) = parts::read_all(dir)?;
    let personas = parts
        .iter()
        .map(|p| Persona {
            part: p.part.clone(),
            persona: p.persona.clone(),
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
    Ok(Map {
        personas,
        bring_ins,
        clashes,
        refused,
    })
}

/// Shown in place of a secret a part list holds as a value.
pub const HIDDEN: &str = "(secret, not shown)";

/// The record as the map keeps it, and its declaration. A part list is a
/// plain file, so a secret lives elsewhere and the list only names where;
/// a secret held as a value makes the bring-in unreadable and is never kept.
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
    use serde_json::json;
    use std::fs;
    use tempfile::TempDir;

    fn list(dir: &Path, part: &str, persona: &str, bring_ins: serde_json::Value) {
        let v = json!({"format": 1, "part": part, "harnesses": {}, "persona": persona, "bring_ins": bring_ins});
        fs::write(dir.join(format!("{part}.json")), v.to_string()).unwrap();
    }

    fn skill(dir: &Path, name: &str, meta: &str) -> String {
        let p = dir.join(format!("{name}.md"));
        fs::write(
            &p,
            format!("---\nname: {name}\ndescription: about {name}\nmetadata:\n{meta}---\n"),
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
        list(
            dir.path(),
            "home",
            "personal",
            json!([{"name": "notes", "unit": {"standard": "skill", "path": notes}}]),
        );
        list(dir.path(), "job", "work", json!([{"name": "triage"}]));
        let map = fuse(dir.path()).unwrap();
        let personas: Vec<_> = map
            .personas
            .iter()
            .map(|p| (p.part.as_str(), p.persona.as_deref()))
            .collect();
        assert_eq!(
            personas,
            [("home", Some("personal")), ("job", Some("work"))]
        );
        let b: Vec<_> = map
            .bring_ins
            .iter()
            .map(|b| (b.part.as_str(), b.name.as_str(), b.persona.as_deref()))
            .collect();
        assert_eq!(
            b,
            [
                ("home", "notes", Some("personal")),
                ("job", "triage", Some("work"))
            ]
        );
        assert_eq!(map.bring_ins[0].job(), Some("capture notes"));
        assert_eq!(
            map.bring_ins[1].declared,
            Declared::Undeclared { description: None }
        );
        assert!(map.clashes.is_empty());
    }

    #[test]
    fn the_same_bring_in_from_two_parts_is_kept_twice_and_a_clash() {
        let dir = TempDir::new().unwrap();
        list(dir.path(), "a", "personal", json!([{"name": "mem"}]));
        list(
            dir.path(),
            "b",
            "work",
            json!([{"name": "mem"}, {"name": "other"}]),
        );
        let map = fuse(dir.path()).unwrap();
        assert_eq!(map.bring_ins.iter().filter(|b| b.name == "mem").count(), 2);
        assert_eq!(map.clashes, ["mem"]);
    }

    #[test]
    fn the_map_does_not_depend_on_the_order_files_are_written() {
        let (one, two) = (TempDir::new().unwrap(), TempDir::new().unwrap());
        list(one.path(), "a", "p", json!([{"name": "x"}]));
        list(one.path(), "b", "q", json!([{"name": "y"}]));
        list(two.path(), "b", "q", json!([{"name": "y"}]));
        list(two.path(), "a", "p", json!([{"name": "x"}]));
        assert_eq!(fuse(one.path()).unwrap(), fuse(two.path()).unwrap());
    }

    #[test]
    fn a_secret_held_as_a_value_makes_that_bring_in_unreadable_and_is_never_echoed() {
        let dir = TempDir::new().unwrap();
        let s = skill(dir.path(), "s", "  my-harness.scope: session\n  my-harness.job: j\n  my-harness.config.token: \"[secret] t\"\n");
        list(
            dir.path(),
            "a",
            "p",
            json!([
                {"name": "leaky", "unit": {"standard": "skill", "path": s}, "values": {"token": "hunter2"}},
                {"name": "fine", "unit": {"standard": "skill", "path": s}, "values": {"token": "env:TOKEN"}}
            ]),
        );
        let map = fuse(dir.path()).unwrap();
        match &map.bring_ins[0].declared {
            Declared::Unreadable { why } => assert!(
                why.contains("env:<NAME>") && !why.contains("hunter2"),
                "{why}"
            ),
            other => panic!("{other:?}"),
        }
        assert!(map.bring_ins[1].declaration().is_some());
        let all = serde_json::to_string(&map).unwrap();
        assert!(!all.contains("hunter2"), "{all}");
    }

    #[test]
    fn an_unreadable_record_or_unit_is_kept_with_why() {
        let dir = TempDir::new().unwrap();
        list(
            dir.path(),
            "a",
            "p",
            json!([
                {"name": "bad", "route": "sometimes"},
                {"name": "gone", "unit": {"standard": "skill", "path": "/nowhere/SKILL.md"}}
            ]),
        );
        let map = fuse(dir.path()).unwrap();
        assert!(map.bring_ins[0].record.is_none());
        assert!(matches!(
            map.bring_ins[0].declared,
            Declared::Unreadable { .. }
        ));
        assert!(
            matches!(&map.bring_ins[1].declared, Declared::Unreadable { why } if why.contains("cannot read"))
        );
    }
}
