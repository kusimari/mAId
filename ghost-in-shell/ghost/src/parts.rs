//! What each part records about itself in its part list: its persona and
//! its bring-ins, each with source, tenure, route, bound values and where
//! its manifest is. The keys sit beside `harnesses` in format 1, which
//! every reader already ignores when it does not know them. See project.md
//! "My harness".
//!
//! This is a reader: a list it cannot take is reported, and a bring-in it
//! cannot take is reported on its own, never failing the others.

use crate::contract::Standard;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Where the unit's manifest is, so its declaration is read from its slot.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct Unit {
    pub standard: Standard,
    pub path: PathBuf,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    /// A git project repo.
    Project,
    /// A git store the person owns.
    Store,
    /// A directory on this machine.
    Dir,
    Vendor,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct Source {
    pub kind: SourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    /// The pinned revision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rev: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase", tag = "kind")]
pub enum Tenure {
    #[default]
    Own,
    Borrowed {
        from: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        since: Option<String>,
    },
}

/// How a finding about this bring-in is applied.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Route {
    Write,
    Backlog,
    /// A worktree to turn into a PR on the source.
    Worktree,
    Report,
}

/// One bring-in as its part records it.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct BringIn {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<Unit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    #[serde(default)]
    pub tenure: Tenure,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<Route>,
    /// The config values bound here. A secret's value is `env:<NAME>`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub values: BTreeMap<String, String>,
}

impl BringIn {
    /// The route a finding takes: borrowed is always report only; otherwise
    /// the recorded one, or the source's default.
    pub fn route(&self) -> Route {
        if matches!(self.tenure, Tenure::Borrowed { .. }) {
            return Route::Report;
        }
        self.route
            .unwrap_or(match self.source.as_ref().map(|s| s.kind) {
                Some(SourceKind::Project) => Route::Worktree,
                Some(SourceKind::Store | SourceKind::Dir) => Route::Write,
                Some(SourceKind::Vendor) | None => Route::Report,
            })
    }
}

/// One part's list, as far as the ghost reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    pub part: String,
    pub persona: Option<String>,
    /// Each bring-in, or its name (or `#<index>`) and why it was not taken.
    pub bring_ins: Vec<Result<BringIn, (String, String)>>,
}

/// A file in the parts dir that is not a part list this reader takes.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Refused {
    pub file: String,
    pub why: String,
}

/// Every non-dot `*.json` in `dir`, in name order. A missing dir has none;
/// one that cannot be listed is an error.
pub fn read_all(dir: &Path) -> io::Result<(Vec<Part>, Vec<Refused>)> {
    let mut files: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(rd) => rd.map(|e| e.map(|e| e.path())).collect::<io::Result<_>>()?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => vec![],
        Err(e) => return Err(e),
    };
    files.retain(|p| {
        let stem = p.file_stem().unwrap_or_default().to_string_lossy();
        !stem.is_empty() && !stem.starts_with('.') && p.extension().is_some_and(|x| x == "json")
    });
    files.sort();
    let (mut parts, mut refused) = (vec![], vec![]);
    for path in files {
        let file = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        match read_one(&path) {
            Ok(p) => parts.push(p),
            Err(why) => refused.push(Refused { file, why }),
        }
    }
    Ok((parts, refused))
}

fn read_one(path: &Path) -> Result<Part, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("cannot read: {e}"))?;
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("not JSON: {e}"))?;
    if v["format"].as_f64() != Some(1.0) {
        return Err("not format 1".into());
    }
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let part = match v["part"].as_str() {
        Some(p) if p == stem => p.to_string(),
        _ => return Err("part is not the file's name".into()),
    };
    harnesses(&v["harnesses"])?;
    let persona = match &v["persona"] {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        _ => return Err("persona is not a string".into()),
    };
    let bring_ins = match &v["bring_ins"] {
        Value::Null => vec![],
        Value::Array(items) => items
            .iter()
            .enumerate()
            .map(|(i, b)| bring_in(i, b))
            .collect(),
        _ => return Err("bring_ins is not an array".into()),
    };
    Ok(Part {
        part,
        persona,
        bring_ins,
    })
}

/// The shape whole-status refuses a list for (reader rule 3), so the ghost
/// and status take the same lists.
fn harnesses(v: &Value) -> Result<(), String> {
    let Value::Object(by_harness) = v else {
        return Err("harnesses is not an object".into());
    };
    let str_or_null = |e: &Value, k: &str| matches!(e[k], Value::Null | Value::String(_));
    for (h, entries) in by_harness {
        let Value::Array(entries) = entries else {
            return Err(format!("harnesses.{h} is not an array"));
        };
        let bad = entries.iter().any(|e| {
            !e.is_object()
                || !e["kind"].is_string()
                || !e["name"].is_string()
                || !str_or_null(e, "scope")
                || !str_or_null(e, "skipped")
        });
        if bad {
            return Err(format!(
                "an entry of harnesses.{h} is not of the format's shape"
            ));
        }
    }
    Ok(())
}

fn bring_in(i: usize, v: &Value) -> Result<BringIn, (String, String)> {
    // A null field is absent (reader rule 4).
    let v = &match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .filter(|(_, x)| !x.is_null())
                .map(|(k, x)| (k.clone(), x.clone()))
                .collect(),
        ),
        other => other.clone(),
    };
    let name = v["name"]
        .as_str()
        .map_or_else(|| format!("#{i}"), str::to_string);
    serde_json::from_value(v.clone()).map_err(|e| (name, e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn list(dir: &TempDir, file: &str, v: Value) {
        fs::write(dir.path().join(file), v.to_string()).unwrap();
    }

    #[test]
    fn a_list_without_the_new_keys_has_no_persona_and_no_bring_ins() {
        let dir = TempDir::new().unwrap();
        list(
            &dir,
            "a.json",
            json!({"format": 1, "part": "a", "harnesses": {}}),
        );
        let (parts, refused) = read_all(dir.path()).unwrap();
        assert!(refused.is_empty());
        assert_eq!(
            parts,
            vec![Part {
                part: "a".into(),
                persona: None,
                bring_ins: vec![]
            }]
        );
    }

    #[test]
    fn every_recorded_field_is_read() {
        let dir = TempDir::new().unwrap();
        list(
            &dir,
            "w.json",
            json!({"format": 1, "part": "w", "persona": "work", "harnesses": {}, "bring_ins": [{
                "name": "mem",
                "unit": {"standard": "mcp", "path": "/u/server.json"},
                "source": {"kind": "store", "at": "git@example.org:me/mem", "rev": "abc"},
                "tenure": {"kind": "borrowed", "from": "a friend", "since": "2026-10-01"},
                "route": "write",
                "values": {"store": "/s"},
                "later": "ignored"
            }]}),
        );
        let (parts, _) = read_all(dir.path()).unwrap();
        let b = parts[0].bring_ins[0].clone().unwrap();
        assert_eq!(parts[0].persona.as_deref(), Some("work"));
        assert_eq!(b.unit.unwrap().standard, Standard::Mcp);
        assert_eq!(b.source.unwrap().rev.as_deref(), Some("abc"));
        assert_eq!(
            b.tenure,
            Tenure::Borrowed {
                from: "a friend".into(),
                since: Some("2026-10-01".into())
            }
        );
        assert_eq!(b.values["store"], "/s");
    }

    #[test]
    fn route_defaults_from_the_source_and_borrowed_is_report_only() {
        let at = |kind| BringIn {
            name: "b".into(),
            unit: None,
            source: Some(Source {
                kind,
                at: None,
                rev: None,
            }),
            tenure: Tenure::Own,
            route: None,
            values: BTreeMap::new(),
        };
        assert_eq!(at(SourceKind::Project).route(), Route::Worktree);
        assert_eq!(at(SourceKind::Store).route(), Route::Write);
        assert_eq!(at(SourceKind::Dir).route(), Route::Write);
        assert_eq!(at(SourceKind::Vendor).route(), Route::Report);
        let recorded = BringIn {
            route: Some(Route::Backlog),
            ..at(SourceKind::Project)
        };
        assert_eq!(recorded.route(), Route::Backlog);
        let borrowed = BringIn {
            tenure: Tenure::Borrowed {
                from: "x".into(),
                since: None,
            },
            ..recorded
        };
        assert_eq!(borrowed.route(), Route::Report);
        assert_eq!(
            BringIn {
                source: None,
                ..at(SourceKind::Dir)
            }
            .route(),
            Route::Report
        );
    }

    #[test]
    fn a_bad_bring_in_is_reported_alone() {
        let dir = TempDir::new().unwrap();
        list(
            &dir,
            "a.json",
            json!({"format": 1, "part": "a", "harnesses": {}, "bring_ins": [
                {"name": "good", "tenure": null, "values": null, "route": null},
                {"name": "bad", "tenure": {"kind": "rented"}},
                {"route": "write"}
            ]}),
        );
        let (parts, _) = read_all(dir.path()).unwrap();
        let b = &parts[0].bring_ins;
        assert_eq!(b[0].as_ref().unwrap().name, "good");
        assert_eq!(b[1].as_ref().unwrap_err().0, "bad");
        assert_eq!(b[2].as_ref().unwrap_err().0, "#2");
    }

    #[test]
    fn lists_it_cannot_take_are_refused_and_others_are_not_read() {
        let dir = TempDir::new().unwrap();
        list(&dir, "a.json", json!({"format": 2, "part": "a"}));
        list(&dir, "b.json", json!({"format": 1, "part": "c"}));
        list(
            &dir,
            "d.json",
            json!({"format": 1, "part": "d", "harnesses": {}, "persona": 1}),
        );
        list(
            &dir,
            "e.json",
            json!({"format": 1, "part": "e", "harnesses": {}, "bring_ins": {}}),
        );
        list(&dir, "i.json", json!({"format": 1, "part": "i"}));
        list(
            &dir,
            "j.json",
            json!({"format": 1, "part": "j", "harnesses": {"claude": {}}}),
        );
        let bad_kind =
            json!({"format": 1, "part": "k", "harnesses": {"claude": [{"kind": 1, "name": "x"}]}});
        list(&dir, "k.json", bad_kind);
        let bad_scope = json!({"format": 1, "part": "l", "harnesses": {"claude": [{"kind": "skill", "name": "x", "scope": 2}]}});
        list(&dir, "l.json", bad_scope);
        list(&dir, ".f.json", json!("not read"));
        fs::write(dir.path().join("g.txt"), "not read").unwrap();
        fs::write(dir.path().join("h.json"), "not json").unwrap();
        let (parts, refused) = read_all(dir.path()).unwrap();
        assert!(parts.is_empty());
        let files: Vec<_> = refused.iter().map(|r| r.file.as_str()).collect();
        assert_eq!(
            files,
            [
                "a.json", "b.json", "d.json", "e.json", "h.json", "i.json", "j.json", "k.json",
                "l.json"
            ]
        );
        assert!(read_all(&dir.path().join("none")).unwrap().0.is_empty());
    }
}
