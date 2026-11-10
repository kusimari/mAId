//! The environment: the ghost as resolved where the session runs.
//! Derived, never authored: each bring-in is ready when its config is
//! bound, its state locations and secrets are there, and the commands it
//! requires are on PATH.

use crate::ghost::{BringIn, Declared, Ghost};
use serde::Serialize;
use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::path::{Component, Path, PathBuf};

/// What the environment offers: a PATH, and which variables are set. Only
/// the names are kept, so no secret's value is held or printed.
#[derive(Debug, Clone, Default)]
pub struct Host {
    pub path: Option<OsString>,
    pub set: BTreeSet<String>,
}

impl Host {
    pub fn from_env() -> Host {
        Host {
            path: std::env::var_os("PATH"),
            set: std::env::vars_os()
                .filter(|(_, v)| !v.is_empty())
                .filter_map(|(k, _)| k.into_string().ok())
                .collect(),
        }
    }
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "kebab-case", tag = "is", content = "why")]
pub enum Readiness {
    Ready,
    NotReady(Vec<String>),
    /// Registered, with no declaration to check readiness against.
    Undeclared,
    Unreadable(String),
}

impl Readiness {
    pub fn label(&self) -> &'static str {
        match self {
            Readiness::Ready => "ready",
            Readiness::NotReady(_) => "not ready",
            Readiness::Undeclared => "undeclared",
            Readiness::Unreadable(_) => "unreadable",
        }
    }

    pub fn why(&self) -> Option<String> {
        match self {
            Readiness::NotReady(w) => Some(w.join("; ")),
            Readiness::Unreadable(w) => Some(w.clone()),
            Readiness::Ready | Readiness::Undeclared => None,
        }
    }
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Row {
    pub part: String,
    pub name: String,
    pub readiness: Readiness,
}

/// Every bring-in in the ghost, in its order, with its readiness here.
pub fn derive(ghost: &Ghost, host: &Host) -> Vec<Row> {
    ghost
        .bring_ins
        .iter()
        .map(|b| Row {
            part: b.part.clone(),
            name: b.name.clone(),
            readiness: readiness(b, host),
        })
        .collect()
}

pub fn readiness(b: &BringIn, host: &Host) -> Readiness {
    let d = match &b.declared {
        Declared::Unreadable { why } => return Readiness::Unreadable(why.clone()),
        Declared::Undeclared { .. } => return Readiness::Undeclared,
        Declared::Declared(d) => d,
    };
    let values = b.record.as_ref().map(|r| &r.values);
    let value = |k: &str| values.and_then(|v| v.get(k));
    let mut why = vec![];
    for (key, spec) in &d.config {
        match value(key) {
            None if !spec.optional => why.push(format!("config {key} not bound")),
            None => {}
            Some(v) => match v.strip_prefix("env:") {
                Some(var) if !host.set.contains(var) => why.push(format!("{var} not set")),
                _ => {}
            },
        }
    }
    // An optional key's location may not be written yet: an empty tier.
    let state_keys = d
        .state
        .iter()
        .flat_map(|s| s.keys())
        .filter(|k| !d.config.get(*k).is_some_and(|spec| spec.optional));
    for key in state_keys {
        // A location that is not a path (a URL, say) is not checked here.
        if let Some(v) = value(key).filter(|v| Path::new(v).is_absolute()) {
            if !Path::new(v).exists() {
                why.push(format!("{key} at {v} not there"));
            }
        }
    }
    if let Some(Err(e)) = index_of(b) {
        why.push(e);
    }
    for cmd in &d.requires {
        if find_on(host.path.as_deref(), cmd).is_none() {
            why.push(format!("{cmd} not on PATH"));
        }
    }
    match why.is_empty() {
        true => Readiness::Ready,
        false => Readiness::NotReady(why),
    }
}

/// The memory index a bring-in has, if it has one: its permanent tier's
/// bound location joined with its bound `index`. `Err` when the index
/// would leave that location.
pub fn index_of(b: &BringIn) -> Option<Result<(PathBuf, PathBuf), String>> {
    let d = b.declaration()?;
    let key = d.state.as_ref()?.permanent.as_ref()?;
    if !d.config.contains_key("index") {
        return None;
    }
    let values = &b.record.as_ref()?.values;
    let (store, index) = (values.get(key)?, values.get("index")?);
    Some(
        contained(Path::new(store), Path::new(index))
            .map(|i| (PathBuf::from(store), i))
            .map_err(|e| format!("index: {e}")),
    )
}

/// `rel` under `store`, when it is relative, has no `..`, and still lies
/// under the store once links are followed.
pub fn contained(store: &Path, rel: &Path) -> Result<PathBuf, String> {
    if !store.is_absolute() {
        return Err(format!("store {} is not an absolute path", store.display()));
    }
    let plain = rel
        .components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
    if !plain || rel.as_os_str().is_empty() {
        return Err(format!("{} leaves its store", rel.display()));
    }
    let joined = store.join(rel);
    let Ok(root) = store.canonicalize() else {
        // No store yet: nothing to read in it or through it.
        return Ok(joined);
    };
    // The deepest part of the path that is there, links followed, must
    // still lie under the store.
    match joined.ancestors().find_map(|a| a.canonicalize().ok()) {
        Some(there) if there.starts_with(&root) => Ok(joined),
        _ => Err(format!("{} leaves its store", rel.display())),
    }
}

/// Where `program` is in a PATH value: the first regular file with any
/// execute bit in an absolute entry. Empty and relative entries are
/// skipped: what is found gets run, and a `claude` planted in a checked-out
/// repo must not be.
pub fn find_on(path: Option<&OsStr>, program: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    // A name, never a path: joined onto a PATH dir it could reach anywhere.
    if program.is_empty() || program.contains('/') {
        return None;
    }
    std::env::split_paths(path?)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(program))
        .find(|p| {
            p.metadata()
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    /// A part with one declared bring-in, bound with `values`.
    fn one(dir: &Path, values: serde_json::Value) -> Ghost {
        let skill = dir.join("SKILL.md");
        fs::write(
            &skill,
            "---\nname: m\ndescription: d\nmetadata:\n  my-harness.contract: \"1\"\n  my-harness.scope: session\n  my-harness.job: memory\n  my-harness.config.store: the store\n  my-harness.config.token: \"[secret] [optional] a token\"\n  my-harness.config.colour: \"[optional] a colour\"\n  my-harness.state.local: store\n  my-harness.requires: tool\n---\n",
        )
        .unwrap();
        of(json!({"part": "a", "persona": "p", "bring_ins": [
            {"name": "m", "unit": {"standard": "skill", "path": skill}, "values": values}
        ]}))
    }

    fn of(part: serde_json::Value) -> Ghost {
        Ghost::of(&[crate::parts::Part::from_json(&part).unwrap()])
    }

    fn host(dir: &Path, with_tool: bool, set: &[&str]) -> Host {
        let bin = dir.join("bin");
        fs::create_dir_all(&bin).unwrap();
        if with_tool {
            fs::write(bin.join("tool"), "").unwrap();
            fs::set_permissions(bin.join("tool"), fs::Permissions::from_mode(0o755)).unwrap();
        }
        Host {
            path: Some(bin.into_os_string()),
            set: set.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn why(ghost: &Ghost, host: &Host) -> Vec<String> {
        match readiness(&ghost.bring_ins[0], host) {
            Readiness::NotReady(w) => w,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn all_clear_is_ready() {
        let dir = TempDir::new().unwrap();
        let store = dir.path().display().to_string();
        let ghost = one(dir.path(), json!({"store": store, "token": "env:TOKEN"}));
        let h = host(dir.path(), true, &["TOKEN"]);
        assert_eq!(derive(&ghost, &h)[0].readiness, Readiness::Ready);
    }

    #[test]
    fn each_reason_stands_on_its_own() {
        let dir = TempDir::new().unwrap();
        let store = dir.path().display().to_string();
        let ok = host(dir.path(), true, &["TOKEN"]);
        let unbound = one(dir.path(), json!({}));
        assert_eq!(why(&unbound, &ok), ["config store not bound"]);
        let gone = one(dir.path(), json!({"store": "/nowhere/store"}));
        assert_eq!(why(&gone, &ok), ["store at /nowhere/store not there"]);
        let unset = one(dir.path(), json!({"store": store, "token": "env:TOKEN"}));
        assert_eq!(why(&unset, &host(dir.path(), true, &[])), ["TOKEN not set"]);
        let bound = one(dir.path(), json!({"store": store}));
        let no_tool = Host {
            path: Some(dir.path().join("empty").into_os_string()),
            ..ok.clone()
        };
        assert_eq!(why(&bound, &no_tool), ["tool not on PATH"]);
    }

    #[test]
    fn an_optional_tier_not_written_yet_is_empty_not_missing() {
        let dir = TempDir::new().unwrap();
        let skill = dir.path().join("SKILL.md");
        fs::write(
            &skill,
            "---\nname: w\ndescription: d\nmetadata:\n  my-harness.contract: \"1\"\n  my-harness.scope: turn\n  my-harness.job: write\n  my-harness.config.learned: \"[optional] pending\"\n  my-harness.config.kept: kept\n  my-harness.state.local: learned\n  my-harness.state.permanent: kept\n---\n",
        )
        .unwrap();
        let ghost = of(json!({"part": "a", "bring_ins": [
            {"name": "w", "unit": {"standard": "skill", "path": skill},
             "values": {"learned": "/nowhere/learned.md", "kept": "/nowhere/kept.md"}}
        ]}));
        assert_eq!(
            why(&ghost, &Host::default()),
            ["kept at /nowhere/kept.md not there"]
        );
    }

    #[test]
    fn undeclared_and_unreadable_are_their_own_states() {
        let rows = derive(
            &of(json!({"part": "a", "bring_ins": [
                {"name": "plain"},
                {"name": "gone", "unit": {"standard": "mcp", "path": "/nowhere/server.json"}}
            ]})),
            &Host::default(),
        );
        assert_eq!(rows[0].readiness, Readiness::Undeclared);
        assert!(
            matches!(&rows[1].readiness, Readiness::Unreadable(w) if w.contains("cannot read"))
        );
    }

    #[test]
    fn find_on_takes_the_first_executable_file_in_path_order() {
        use std::os::unix::fs::PermissionsExt;
        let dirs: Vec<_> = (0..3).map(|_| tempfile::TempDir::new().unwrap()).collect();
        let put = |i: usize, mode: u32| {
            let at = dirs[i].path().join("tool");
            std::fs::write(&at, "").unwrap();
            std::fs::set_permissions(&at, std::fs::Permissions::from_mode(mode)).unwrap();
            at
        };
        put(0, 0o644);
        std::fs::create_dir(dirs[1].path().join("tool")).unwrap();
        let later = put(2, 0o755);
        let path = std::env::join_paths(dirs.iter().map(|d| d.path())).unwrap();
        assert_eq!(find_on(Some(&path), "tool"), Some(later));
        assert_eq!(find_on(None, "tool"), None);
    }

    #[test]
    fn empty_and_relative_path_entries_are_skipped() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::TempDir::new().unwrap();
        let at = dir.path().join("tool");
        std::fs::write(&at, "").unwrap();
        std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o755)).unwrap();
        // The same dir, spelled relative to this process's current dir.
        let cwd = std::env::current_dir().unwrap();
        let up = "../".repeat(cwd.components().count() - 1);
        let relative = format!("{up}{}", dir.path().strip_prefix("/").unwrap().display());
        assert!(PathBuf::from(&relative).join("tool").is_file());
        for path in ["", ":", &relative, &format!(":{relative}:")] {
            assert_eq!(find_on(Some(path.as_ref()), "tool"), None, "{path:?}");
        }
        assert_eq!(
            find_on(Some(dir.path().as_os_str()), "tool"),
            Some(at.clone())
        );
        let named = format!(
            "../{}/tool",
            dir.path().file_name().unwrap().to_string_lossy()
        );
        for program in ["", "/bin/sh", named.as_str(), at.to_str().unwrap()] {
            assert_eq!(
                find_on(Some(dir.path().as_os_str()), program),
                None,
                "{program:?}"
            );
        }
    }
}
