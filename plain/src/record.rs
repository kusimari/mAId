//! mAId's install record: what plain install placed on each agent, from
//! which repo and revision, with which settings, at
//! `<install-records>/maid.json`, so uninstall removes exactly that and
//! status checks it. Plain's own business: the ghost never reads it. The
//! format is shared with other packages' installers and the whole-status
//! that reads them all; see project.md "My harness" (the install record).
//!
//! This module is the only writer. It writes its own file and nothing
//! else in the folder; it reads the others only for what they claim. For
//! one release it also reads, and on its next write removes, mAId's old
//! part list (`maid-harness.json` in the old parts folder), and reads the
//! old lists of other packages beside it.

use crate::shared::Agent;
use anyhow::{anyhow, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// mAId's package name, and its install record's stem.
pub const PACKAGE: &str = "maid";
/// mAId's old part list's stem, migrated on the next write.
pub const OLD_PART: &str = "maid-harness";
/// Why a value is not adopted: another package's manifest records it.
pub const ANOTHER_PART: &str = "another part's";
const FORMAT: u32 = 1;

/// Where install records live, and for one release the old parts folder.
pub trait Dirs {
    fn records(&self) -> &Path;
    fn old(&self) -> Option<&Path> {
        None
    }
    /// Whose record this is: mAId's unless another package names itself.
    fn package(&self) -> &str {
        PACKAGE
    }
}

/// Another package's own record in the folder, written in the same format
/// through this module. It has no old part list.
#[derive(Debug, Clone)]
pub struct Own {
    pub records: PathBuf,
    pub package: String,
}

impl Dirs for Own {
    fn records(&self) -> &Path {
        &self.records
    }
    fn package(&self) -> &str {
        &self.package
    }
}

/// A folder alone: install records and no old parts folder.
impl Dirs for Path {
    fn records(&self) -> &Path {
        self
    }
}

impl Dirs for PathBuf {
    fn records(&self) -> &Path {
        self
    }
}

/// Both folders, as install and uninstall use them.
#[derive(Debug, Clone)]
pub struct Lists {
    pub records: PathBuf,
    pub old: PathBuf,
}

impl Dirs for Lists {
    fn records(&self) -> &Path {
        &self.records
    }
    fn old(&self) -> Option<&Path> {
        Some(&self.old)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(rename_all = "lowercase")]
pub enum What {
    Skill,
    Mcp,
    Agent,
    Plugin,
    Marketplace,
    Setting,
}

impl What {
    pub fn parse(token: &str) -> Result<What> {
        serde_json::from_value(serde_json::Value::String(token.into()))
            .map_err(|_| anyhow!("unknown kind {token:?}"))
    }
}

/// A repo and the revision installed from it; or, for a piece found on
/// PATH rather than pinned, where it was found and the version it reports.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct Repo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rev: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
}

/// One thing a package placed, or meant to place and skipped.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Item {
    pub kind: What,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped: Option<String>,
    /// For a setting: the value the package set, so it can tell its own
    /// value from another's later.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// The package found this exact value already there and took it over:
    /// it owns it as if it had written it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub adopted: bool,
    /// The dir its files are in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<PathBuf>,
    /// Where it came from, when not this repo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Repo>,
    /// The settings it was bound with. A secret's value is `env:<NAME>`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub values: BTreeMap<String, String>,
}

impl Item {
    pub fn placed(kind: What, name: impl Into<String>) -> Item {
        Item {
            kind,
            name: name.into(),
            scope: None,
            skipped: None,
            value: None,
            adopted: false,
            at: None,
            source: None,
            values: BTreeMap::new(),
        }
    }

    pub fn skipped(kind: What, name: impl Into<String>, why: impl Into<String>) -> Item {
        Item {
            skipped: Some(why.into()),
            ..Item::placed(kind, name)
        }
    }
}

/// What install records about the package as a whole.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Package {
    pub source: Option<Repo>,
    /// Where the package's files were installed.
    pub root: Option<PathBuf>,
    pub config: Option<PathBuf>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct InstallRecord {
    format: u32,
    package: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source: Option<Repo>,
    /// When the package was first installed here; a reinstall keeps it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    installed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    root: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    config: Option<PathBuf>,
    agents: BTreeMap<String, Vec<Item>>,
}

/// mAId's old part list, as far as migration reads it.
#[derive(Deserialize)]
struct OldList {
    format: u32,
    part: String,
    harnesses: BTreeMap<String, Vec<Item>>,
    #[serde(default)]
    bring_ins: Vec<Value>,
}

/// The file mAId's install record lives in.
pub fn path(dirs: &(impl Dirs + ?Sized)) -> PathBuf {
    dirs.records().join(format!("{}.json", dirs.package()))
}

fn old_path(dirs: &(impl Dirs + ?Sized)) -> Option<PathBuf> {
    dirs.old().map(|d| d.join(format!("{OLD_PART}.json")))
}

/// mAId's manifest, if there is one, refusing a format or package it does
/// not know; else its old part list, read as one.
fn read(dirs: &(impl Dirs + ?Sized)) -> Result<Option<InstallRecord>> {
    let at = path(dirs);
    match fs::read_to_string(&at) {
        Ok(text) => {
            let m: InstallRecord = serde_json::from_str(&text)
                .with_context(|| format!("{} is not an install record", at.display()))?;
            ensure!(
                m.format == FORMAT && m.package == dirs.package(),
                "{} is format {} of package {:?}; refusing to read or rewrite it",
                at.display(),
                m.format,
                m.package
            );
            return Ok(Some(m));
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e).with_context(|| format!("cannot read {}", at.display())),
    }
    let Some(old) = old_path(dirs) else {
        return Ok(None);
    };
    let list: OldList = match fs::read_to_string(&old) {
        Ok(text) => serde_json::from_str(&text)
            .with_context(|| format!("{} is not a part list", old.display()))?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).with_context(|| format!("cannot read {}", old.display())),
    };
    ensure!(
        list.format == FORMAT && list.part == OLD_PART,
        "{} is format {} of part {:?}; refusing to read or rewrite it",
        old.display(),
        list.format,
        list.part
    );
    // The earliest `since` an old list records stands in for when mAId was
    // first installed.
    let installed = list
        .bring_ins
        .iter()
        .filter_map(|b| b["tenure"]["since"].as_str())
        .min()
        .map(str::to_string);
    Ok(Some(InstallRecord {
        format: FORMAT,
        package: PACKAGE.into(),
        source: None,
        installed,
        root: None,
        config: None,
        agents: list.harnesses,
    }))
}

/// What mAId's manifest says it placed on `agent`, skips left out: what an
/// earlier install added and so may remove. No manifest owns nothing.
pub fn placed(dirs: &(impl Dirs + ?Sized), agent: Agent) -> Result<Vec<Item>> {
    let Some(mut m) = read(dirs)? else {
        return Ok(vec![]);
    };
    let mut items = m.agents.remove(agent.cli()).unwrap_or_default();
    items.retain(|i| i.skipped.is_none());
    Ok(items)
}

/// mAId's entry of `kind` named `name` on `agent`, placed or skipped.
pub fn recorded(
    dirs: &(impl Dirs + ?Sized),
    agent: Agent,
    kind: What,
    name: &str,
) -> Result<Option<Item>> {
    Ok(read(dirs)?.and_then(|mut m| {
        m.agents
            .remove(agent.cli())
            .unwrap_or_default()
            .into_iter()
            .find(|i| i.kind == kind && i.name == name)
    }))
}

/// The values of setting `name` on `agent` that another package records as
/// placed: theirs, so never adopted or written (rule 2). A file that cannot
/// be read, or of a format other than 1, claims nothing.
pub fn claimed_by_others(
    dirs: &(impl Dirs + ?Sized),
    agent: Agent,
    name: &str,
) -> BTreeSet<String> {
    other_entries(dirs, agent)
        .filter(|i| i["kind"] == "setting" && i["name"] == name && i["skipped"].is_null())
        .filter_map(|i| i["value"].as_str().map(str::to_string))
        .collect()
}

/// Whether another package records an entry of `kind` named `name` on
/// `agent` as placed: theirs, so never taken over (rule 2). A file that
/// cannot be read, or of a format other than 1, records nothing.
pub fn placed_by_others(dirs: &(impl Dirs + ?Sized), agent: Agent, kind: What, name: &str) -> bool {
    let kind = serde_json::to_value(kind).unwrap_or_default();
    other_entries(dirs, agent)
        .any(|i| i["kind"] == kind && i["name"] == name && i["skipped"].is_null())
}

/// Every other package's entries on `agent`: every other install record's
/// `agents`, and every old list's `harnesses` but mAId's own.
fn other_entries(dirs: &(impl Dirs + ?Sized), agent: Agent) -> impl Iterator<Item = Value> {
    let folders = [
        Some((dirs.records().to_path_buf(), dirs.package(), "agents")),
        dirs.old().map(|d| (d.to_path_buf(), OLD_PART, "harnesses")),
    ];
    let agent = agent.cli();
    folders
        .into_iter()
        .flatten()
        .flat_map(|(dir, own, key)| {
            lists(&dir, own)
                .into_iter()
                .flat_map(move |l| match &l[key][agent] {
                    Value::Array(items) => items.clone(),
                    _ => vec![],
                })
        })
        .collect::<Vec<_>>()
        .into_iter()
}

/// Every file of format 1 in `dir` but `own`, as JSON.
fn lists(dir: &Path, own: &str) -> Vec<Value> {
    let own = format!("{own}.json");
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let file = e.file_name().into_string().ok()?;
            let stem = file.strip_suffix(".json")?;
            if stem.is_empty() || stem.starts_with('.') || file == own {
                return None;
            }
            serde_json::from_str::<Value>(&fs::read_to_string(e.path()).ok()?)
                .ok()
                .filter(|l| l["format"].as_f64() == Some(1.0))
        })
        .collect()
}

/// Replace `agent`'s entries of `kinds` in `scope` with `items`, keeping
/// every other entry. Recording in a scope also drops an unscoped skip of
/// those kinds: it was skipped for want of a scope. The file goes when
/// nothing is left in it.
pub fn record(
    dirs: &(impl Dirs + ?Sized),
    agent: Agent,
    kinds: &[What],
    scope: Option<&str>,
    items: Vec<Item>,
) -> Result<()> {
    let replaced = |i: &Item| {
        kinds.contains(&i.kind)
            && match (i.scope.as_deref(), scope) {
                (found, want) if found == want => true,
                (None, Some(_)) => i.skipped.is_some(),
                _ => false,
            }
    };
    rewrite(dirs, agent, replaced, items)
}

/// Replace `agent`'s entries of `kind` named `name` in `scope` with
/// `items`, keeping every other entry of that kind: each step owns its own
/// MCP server or setting.
pub fn record_named(
    dirs: &(impl Dirs + ?Sized),
    agent: Agent,
    kind: What,
    scope: Option<&str>,
    name: &str,
    items: Vec<Item>,
) -> Result<()> {
    let replaced = |i: &Item| i.kind == kind && i.name == name && i.scope.as_deref() == scope;
    rewrite(dirs, agent, replaced, items)
}

/// Replace `agent`'s entry for the setting `name` with `item` (none clears
/// it), keeping every other setting: each step owns its own setting.
pub fn record_setting(
    dirs: &(impl Dirs + ?Sized),
    agent: Agent,
    name: &str,
    item: Option<Item>,
) -> Result<()> {
    record_named(
        dirs,
        agent,
        What::Setting,
        None,
        name,
        item.into_iter().collect(),
    )
}

/// Record what install knows of the package as a whole, keeping every
/// entry. With nothing placed there is no manifest to write it in.
pub fn record_package(dirs: &(impl Dirs + ?Sized), package: Package) -> Result<()> {
    change(dirs, |m| {
        m.source = package.source;
        m.root = package.root;
        m.config = package.config;
    })
}

/// Change every entry in place, keeping which entries there are.
pub fn amend(dirs: &(impl Dirs + ?Sized), each: impl Fn(&mut Item)) -> Result<()> {
    change(dirs, |m| m.agents.values_mut().flatten().for_each(each))
}

fn rewrite(
    dirs: &(impl Dirs + ?Sized),
    agent: Agent,
    replaced: impl Fn(&Item) -> bool,
    items: Vec<Item>,
) -> Result<()> {
    change(dirs, |m| {
        let entries = m.agents.entry(agent.cli().into()).or_default();
        let (gone, kept): (Vec<Item>, Vec<Item>) = entries.drain(..).partition(&replaced);
        *entries = kept;
        // A step that reads back what it placed knows its name, not its
        // files or source: those stay as install last recorded them.
        entries.extend(items.into_iter().map(|mut i| {
            let same = gone.iter().find(|g| {
                (g.kind, &g.name, &g.scope) == (i.kind, &i.name, &i.scope) && g.skipped.is_none()
            });
            if let (Some(g), None) = (same, &i.skipped) {
                i.at = i.at.take().or_else(|| g.at.clone());
                i.source = i.source.take().or_else(|| g.source.clone());
                if i.values.is_empty() {
                    i.values = g.values.clone();
                }
            }
            i
        }));
        entries.sort();
        entries.dedup();
        m.agents.retain(|_, entries| !entries.is_empty());
    })
}

fn change(dirs: &(impl Dirs + ?Sized), edit: impl FnOnce(&mut InstallRecord)) -> Result<()> {
    let at = path(dirs);
    let mut m = read(dirs)?.unwrap_or_else(|| InstallRecord {
        format: FORMAT,
        package: dirs.package().into(),
        source: None,
        installed: None,
        root: None,
        config: None,
        agents: BTreeMap::new(),
    });
    edit(&mut m);
    let remove = |p: &Path| match fs::remove_file(p) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    };
    if m.agents.is_empty() {
        remove(&at)?;
        if let Some(old) = old_path(dirs) {
            remove(&old)?;
        }
        return Ok(());
    }
    m.installed.get_or_insert_with(crate::time::today);
    let dir = dirs.records();
    fs::create_dir_all(dir)?;
    // Atomic, so a reader never sees half a manifest.
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    writeln!(tmp, "{}", serde_json::to_string_pretty(&m)?)?;
    tmp.persist(&at)?;
    // The old list is migrated once the new file holds its entries.
    if let Some(old) = old_path(dirs) {
        remove(&old)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const MCP: &[What] = &[What::Mcp];
    const SKILL: &[What] = &[What::Skill];

    fn read(dir: &Path) -> InstallRecord {
        serde_json::from_str(&fs::read_to_string(path(dir)).unwrap()).unwrap()
    }

    fn entries(dir: &Path, agent: Agent) -> Vec<Item> {
        read(dir).agents[agent.cli()].clone()
    }

    #[test]
    fn first_record_writes_the_part_and_its_entries() {
        let dir = TempDir::new().unwrap();
        let skill = Item::placed(What::Skill, "notes");
        record(dir.path(), Agent::Kiro, SKILL, None, vec![skill.clone()]).unwrap();
        let list = read(dir.path());
        assert_eq!((list.format, list.package.as_str()), (1, "maid"));
        assert_eq!(list.agents["kiro-cli"], vec![skill]);
        assert_eq!(list.installed, Some(crate::time::today()));
    }

    #[test]
    fn recording_the_same_entries_again_leaves_the_bytes_alone() {
        let dir = TempDir::new().unwrap();
        let items = || {
            vec![
                Item::placed(What::Skill, "b"),
                Item::placed(What::Skill, "a"),
            ]
        };
        record(dir.path(), Agent::Agy, SKILL, None, items()).unwrap();
        let first = fs::read(path(dir.path())).unwrap();
        record(dir.path(), Agent::Agy, SKILL, None, items()).unwrap();
        assert_eq!(fs::read(path(dir.path())).unwrap(), first);
    }

    #[test]
    fn recording_one_harness_keeps_the_others() {
        let dir = TempDir::new().unwrap();
        let skill = Item::placed(What::Skill, "notes");
        record(dir.path(), Agent::Kiro, SKILL, None, vec![skill.clone()]).unwrap();
        record(dir.path(), Agent::Agy, SKILL, None, vec![]).unwrap();
        assert_eq!(entries(dir.path(), Agent::Kiro), vec![skill]);
    }

    #[test]
    fn recording_one_kind_keeps_the_other_kinds() {
        let dir = TempDir::new().unwrap();
        let mcp = Item::placed(What::Mcp, "browser");
        record(dir.path(), Agent::Claude, MCP, None, vec![mcp.clone()]).unwrap();
        record(
            dir.path(),
            Agent::Claude,
            &[What::Plugin],
            None,
            vec![Item::placed(What::Plugin, "p")],
        )
        .unwrap();
        record(dir.path(), Agent::Claude, &[What::Plugin], None, vec![]).unwrap();
        assert_eq!(entries(dir.path(), Agent::Claude), vec![mcp]);
    }

    #[test]
    fn recording_one_scope_keeps_the_other_scopes() {
        let dir = TempDir::new().unwrap();
        let mcp = Item {
            scope: Some("one".into()),
            ..Item::placed(What::Mcp, "browser")
        };
        record(dir.path(), Agent::Kiro, MCP, Some("one"), vec![mcp.clone()]).unwrap();
        record(
            dir.path(),
            Agent::Kiro,
            MCP,
            None,
            vec![Item::skipped(What::Mcp, "browser", "not placed")],
        )
        .unwrap();
        record(dir.path(), Agent::Kiro, MCP, None, vec![]).unwrap();
        assert_eq!(entries(dir.path(), Agent::Kiro), vec![mcp]);
    }

    /// Skipped with no scope, then placed in a scope, then removed from
    /// it: nothing is left.
    #[test]
    fn recording_in_a_scope_drops_the_unscoped_skip() {
        let dir = TempDir::new().unwrap();
        let skip = Item::skipped(What::Mcp, "browser", "not placed");
        record(dir.path(), Agent::Kiro, MCP, None, vec![skip]).unwrap();
        let scoped = Item {
            scope: Some("a".into()),
            ..Item::placed(What::Mcp, "browser")
        };
        record(dir.path(), Agent::Kiro, MCP, Some("a"), vec![scoped]).unwrap();
        record(dir.path(), Agent::Kiro, MCP, Some("a"), vec![]).unwrap();
        assert!(!path(dir.path()).exists());
    }

    #[test]
    fn a_list_in_another_format_is_left_alone() {
        let dir = TempDir::new().unwrap();
        let text = r#"{"format":2,"package":"maid","agents":{},"more":1}"#;
        fs::write(path(dir.path()), text).unwrap();
        let err = record(dir.path(), Agent::Kiro, SKILL, None, vec![]).unwrap_err();
        assert!(err.to_string().contains("format 2"), "{err}");
        assert_eq!(fs::read_to_string(path(dir.path())).unwrap(), text);
    }

    #[test]
    fn another_packages_manifest_is_left_alone() {
        let dir = TempDir::new().unwrap();
        let text = r#"{"format":1,"package":"other","agents":{}}"#;
        fs::write(path(dir.path()), text).unwrap();
        assert!(record(dir.path(), Agent::Kiro, SKILL, None, vec![]).is_err());
        assert_eq!(fs::read_to_string(path(dir.path())).unwrap(), text);
    }

    #[test]
    fn a_skipped_entry_carries_its_reason() {
        let dir = TempDir::new().unwrap();
        let item = Item::skipped(What::Plugin, "p", "claude not on PATH");
        record(dir.path(), Agent::Claude, &[What::Plugin], None, vec![item]).unwrap();
        let text = fs::read_to_string(path(dir.path())).unwrap();
        assert!(
            text.contains(r#""skipped": "claude not on PATH""#),
            "{text}"
        );
    }

    #[test]
    fn the_file_goes_with_its_last_entry_and_nothing_else_does() {
        let dir = TempDir::new().unwrap();
        let other = dir.path().join("another-package.json");
        fs::write(&other, "{}").unwrap();
        let skill = vec![Item::placed(What::Skill, "notes")];
        record(dir.path(), Agent::Kiro, SKILL, None, skill).unwrap();
        record(dir.path(), Agent::Kiro, SKILL, None, vec![]).unwrap();
        assert!(!path(dir.path()).exists());
        assert_eq!(fs::read_to_string(&other).unwrap(), "{}");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn recording_nothing_with_no_file_writes_nothing() {
        let dir = TempDir::new().unwrap();
        let parts = dir.path().join("parts");
        record(&parts, Agent::Kiro, SKILL, None, vec![]).unwrap();
        assert!(!parts.exists());
    }

    #[test]
    fn a_settings_value_is_read_back() {
        let dir = TempDir::new().unwrap();
        let item = Item {
            value: Some("/s/memory".into()),
            ..Item::placed(What::Setting, "k")
        };
        record(
            dir.path(),
            Agent::Claude,
            &[What::Setting],
            None,
            vec![item.clone()],
        )
        .unwrap();
        assert_eq!(placed(dir.path(), Agent::Claude).unwrap(), vec![item]);
    }

    #[test]
    fn another_parts_placed_server_is_theirs_and_a_skipped_one_is_not() {
        let dir = TempDir::new().unwrap();
        let other = |items: serde_json::Value| {
            fs::write(
                dir.path().join("other.json"),
                serde_json::json!({"format": 1, "package": "other", "agents": {"claude": items}})
                    .to_string(),
            )
            .unwrap()
        };
        other(serde_json::json!([{"kind": "mcp", "name": "s", "skipped": "x"}]));
        assert!(!placed_by_others(dir.path(), Agent::Claude, What::Mcp, "s"));
        other(serde_json::json!([{"kind": "mcp", "name": "s"}]));
        assert!(placed_by_others(dir.path(), Agent::Claude, What::Mcp, "s"));
        assert!(!placed_by_others(
            dir.path(),
            Agent::Claude,
            What::Setting,
            "s"
        ));
        assert!(!placed_by_others(dir.path(), Agent::Codex, What::Mcp, "s"));
    }

    #[test]
    fn other_parts_claim_only_their_placed_values_of_that_setting() {
        let dir = TempDir::new().unwrap();
        let write = |file: &str, text: &str| fs::write(dir.path().join(file), text).unwrap();
        write(
            "other.json",
            r#"{"format":1,"package":"other","agents":{"claude":[
                {"kind":"setting","name":"s","value":"/theirs"},
                {"kind":"setting","name":"s","value":"/skipped","skipped":"why"},
                {"kind":"setting","name":"t","value":"/t"},
                {"kind":"new-kind","name":"s","value":"/kind"}],
              "codex":[{"kind":"setting","name":"s","value":"/codex"}]}}"#,
        );
        write(
            ".hidden.json",
            r#"{"format":1,"agents":{"claude":[{"kind":"setting","name":"s","value":"/dot"}]}}"#,
        );
        write("broken.json", "not json");
        write(
            "future.json",
            r#"{"format":2,"agents":{"claude":[{"kind":"setting","name":"s","value":"/future"}]}}"#,
        );
        let mine = Item {
            value: Some("/mine".into()),
            ..Item::placed(What::Setting, "s")
        };
        record_setting(dir.path(), Agent::Claude, "s", Some(mine)).unwrap();
        assert_eq!(
            claimed_by_others(dir.path(), Agent::Claude, "s"),
            BTreeSet::from(["/theirs".to_string()])
        );
        assert!(claimed_by_others(&dir.path().join("none"), Agent::Claude, "s").is_empty());
    }

    #[test]
    fn placed_reads_one_harness_and_leaves_out_skips() {
        let dir = TempDir::new().unwrap();
        assert_eq!(placed(dir.path(), Agent::Claude).unwrap(), vec![]);
        let plugin = Item::placed(What::Plugin, "p@m");
        let skip = Item::skipped(What::Marketplace, "m", "not mAId's");
        let kinds = &[What::Plugin, What::Marketplace];
        record(
            dir.path(),
            Agent::Claude,
            kinds,
            None,
            vec![plugin.clone(), skip],
        )
        .unwrap();
        let other = vec![Item::placed(What::Plugin, "q@m")];
        record(dir.path(), Agent::Codex, kinds, None, other).unwrap();
        assert_eq!(placed(dir.path(), Agent::Claude).unwrap(), vec![plugin]);
        assert_eq!(placed(dir.path(), Agent::Kiro).unwrap(), vec![]);
    }

    /// Ownership decides what uninstall removes, so a list mAId cannot
    /// read as its own owns nothing and stops the run.
    #[test]
    fn placed_refuses_a_list_in_another_format() {
        let dir = TempDir::new().unwrap();
        let text =
            r#"{"format":2,"package":"maid","agents":{"claude":[{"kind":"plugin","name":"p"}]}}"#;
        fs::write(path(dir.path()), text).unwrap();
        assert!(placed(dir.path(), Agent::Claude).is_err());
    }

    #[test]
    fn each_setting_is_recorded_by_name_and_keeps_the_others() {
        let dir = TempDir::new().unwrap();
        let setting = |name: &str| Item {
            value: Some("v".into()),
            ..Item::placed(What::Setting, name)
        };
        let (a, b) = (setting("a"), setting("b.c"));
        record_setting(dir.path(), Agent::Claude, "a", Some(a.clone())).unwrap();
        record_setting(dir.path(), Agent::Claude, "b.c", Some(b.clone())).unwrap();
        assert_eq!(entries(dir.path(), Agent::Claude), vec![a.clone(), b]);
        record_setting(dir.path(), Agent::Claude, "b.c", None).unwrap();
        assert_eq!(entries(dir.path(), Agent::Claude), vec![a]);
    }

    #[test]
    fn adopted_is_written_only_when_set_and_read_back() {
        let dir = TempDir::new().unwrap();
        let mine = Item::placed(What::Setting, "a");
        let found = Item {
            adopted: true,
            ..Item::placed(What::Setting, "b")
        };
        record_setting(dir.path(), Agent::Claude, "a", Some(mine)).unwrap();
        record_setting(dir.path(), Agent::Claude, "b", Some(found.clone())).unwrap();
        let text = fs::read_to_string(path(dir.path())).unwrap();
        assert_eq!(text.matches("adopted").count(), 1, "{text}");
        assert!(placed(dir.path(), Agent::Claude).unwrap().contains(&found));
    }

    fn lists(dir: &TempDir) -> Lists {
        Lists {
            records: dir.path().join("install-records"),
            old: dir.path().join("parts"),
        }
    }

    #[test]
    fn the_old_part_list_is_read_as_the_manifest_and_gone_on_the_next_write() {
        let dir = TempDir::new().unwrap();
        let l = lists(&dir);
        fs::create_dir_all(&l.old).unwrap();
        let old = l.old.join("maid-harness.json");
        fs::write(
            &old,
            r#"{"format":1,"part":"maid-harness","persona":"personal",
              "harnesses":{"claude":[{"kind":"plugin","name":"maid@maid"},
                {"kind":"setting","name":"s","value":"/v","adopted":true}]},
              "bring_ins":[{"name":"d","tenure":{"kind":"borrowed","from":"x","since":"2026-10-31"}},
                {"name":"n"}]}"#,
        )
        .unwrap();
        let plugin = Item::placed(What::Plugin, "maid@maid");
        assert!(placed(&l, Agent::Claude).unwrap().contains(&plugin));
        record(
            &l,
            Agent::Kiro,
            SKILL,
            None,
            vec![Item::placed(What::Skill, "n")],
        )
        .unwrap();
        assert!(!old.exists());
        let m = read(&l.records);
        assert_eq!(m.installed.as_deref(), Some("2026-10-31"));
        assert_eq!(m.agents["claude"][0], plugin);
        assert!(m.agents["claude"][1].adopted);
        let text = fs::read_to_string(path(&l)).unwrap();
        assert!(
            !text.contains("persona") && !text.contains("bring_ins"),
            "{text}"
        );
    }

    #[test]
    fn a_whole_uninstall_takes_the_old_list_too() {
        let dir = TempDir::new().unwrap();
        let l = lists(&dir);
        fs::create_dir_all(&l.old).unwrap();
        let old = l.old.join("maid-harness.json");
        fs::write(&old, r#"{"format":1,"part":"maid-harness","harnesses":{"agy":[{"kind":"skill","name":"n"}]}}"#).unwrap();
        record(&l, Agent::Agy, SKILL, None, vec![]).unwrap();
        assert!(!old.exists() && !path(&l).exists());
    }

    #[test]
    fn an_old_list_of_another_format_stops_the_run_and_is_left() {
        let dir = TempDir::new().unwrap();
        let l = lists(&dir);
        fs::create_dir_all(&l.old).unwrap();
        let text = r#"{"format":2,"part":"maid-harness","harnesses":{}}"#;
        fs::write(l.old.join("maid-harness.json"), text).unwrap();
        assert!(placed(&l, Agent::Claude).is_err());
        assert!(record(&l, Agent::Kiro, SKILL, None, vec![]).is_err());
        assert_eq!(
            fs::read_to_string(l.old.join("maid-harness.json")).unwrap(),
            text
        );
    }

    #[test]
    fn other_packages_are_read_from_both_folders_and_maids_old_list_is_not_theirs() {
        let dir = TempDir::new().unwrap();
        let l = lists(&dir);
        fs::create_dir_all(&l.old).unwrap();
        fs::create_dir_all(&l.records).unwrap();
        let entry = r#"[{"kind":"mcp","name":"s"}]"#;
        fs::write(
            l.old.join("work.json"),
            format!(r#"{{"format":1,"part":"work","harnesses":{{"claude":{entry}}}}}"#),
        )
        .unwrap();
        assert!(placed_by_others(&l, Agent::Claude, What::Mcp, "s"));
        fs::remove_file(l.old.join("work.json")).unwrap();
        fs::write(
            l.old.join("maid-harness.json"),
            format!(r#"{{"format":1,"part":"maid-harness","harnesses":{{"claude":{entry}}}}}"#),
        )
        .unwrap();
        assert!(!placed_by_others(&l, Agent::Claude, What::Mcp, "s"));
        fs::write(
            l.records.join("ghost.json"),
            format!(r#"{{"format":1,"package":"ghost","agents":{{"claude":{entry}}}}}"#),
        )
        .unwrap();
        assert!(placed_by_others(&l, Agent::Claude, What::Mcp, "s"));
    }

    #[test]
    fn the_package_facts_are_kept_beside_the_entries_and_installed_stays() {
        let dir = TempDir::new().unwrap();
        let package = Package {
            source: Some(Repo {
                repo: Some("github.com:me/maid.git".into()),
                rev: Some("abc".into()),
                path: None,
            }),
            root: Some("/p/share/maid".into()),
            config: Some("/c".into()),
        };
        record_package(dir.path(), package.clone()).unwrap();
        assert!(
            !path(dir.path()).exists(),
            "nothing placed, nothing written"
        );
        record(
            dir.path(),
            Agent::Agy,
            SKILL,
            None,
            vec![Item::placed(What::Skill, "n")],
        )
        .unwrap();
        let mut text: Value =
            serde_json::from_str(&fs::read_to_string(path(dir.path())).unwrap()).unwrap();
        text["installed"] = "2026-01-01".into();
        fs::write(path(dir.path()), text.to_string()).unwrap();
        record_package(dir.path(), package.clone()).unwrap();
        let first = fs::read(path(dir.path())).unwrap();
        record_package(dir.path(), package).unwrap();
        assert_eq!(fs::read(path(dir.path())).unwrap(), first);
        let m = read(dir.path());
        assert_eq!(m.installed.as_deref(), Some("2026-01-01"));
        assert_eq!(m.root, Some("/p/share/maid".into()));
        assert_eq!(m.agents["agy"].len(), 1);
    }

    #[test]
    fn an_items_place_source_and_values_are_written_only_when_set() {
        let dir = TempDir::new().unwrap();
        let item = Item {
            at: Some("/p/servers/m".into()),
            source: Some(Repo {
                repo: Some("r".into()),
                rev: None,
                path: None,
            }),
            values: [("store".to_string(), "/s".to_string())].into(),
            ..Item::placed(What::Mcp, "m")
        };
        record_named(
            dir.path(),
            Agent::Claude,
            What::Mcp,
            None,
            "m",
            vec![item.clone()],
        )
        .unwrap();
        record_named(
            dir.path(),
            Agent::Claude,
            What::Mcp,
            None,
            "n",
            vec![Item::placed(What::Mcp, "n")],
        )
        .unwrap();
        let text = fs::read_to_string(path(dir.path())).unwrap();
        assert_eq!(text.matches("\"at\"").count(), 1, "{text}");
        assert_eq!(placed(dir.path(), Agent::Claude).unwrap()[0], item);
    }

    #[test]
    fn a_rerecorded_entry_keeps_its_files_and_source() {
        let dir = TempDir::new().unwrap();
        let placed_at = Item {
            at: Some("/p/plugin".into()),
            source: Some(Repo {
                repo: Some("r".into()),
                rev: None,
                path: None,
            }),
            ..Item::placed(What::Plugin, "p@m")
        };
        record(
            dir.path(),
            Agent::Claude,
            &[What::Plugin],
            None,
            vec![placed_at.clone()],
        )
        .unwrap();
        let plain = Item::placed(What::Plugin, "p@m");
        record(
            dir.path(),
            Agent::Claude,
            &[What::Plugin],
            None,
            vec![plain],
        )
        .unwrap();
        assert_eq!(placed(dir.path(), Agent::Claude).unwrap(), vec![placed_at]);
        let skip = Item::skipped(What::Plugin, "p@m", "gone");
        record(
            dir.path(),
            Agent::Claude,
            &[What::Plugin],
            None,
            vec![skip.clone()],
        )
        .unwrap();
        assert_eq!(
            recorded(dir.path(), Agent::Claude, What::Plugin, "p@m").unwrap(),
            Some(skip)
        );
    }

    #[test]
    fn kinds_parse_by_their_format_name() {
        assert_eq!(What::parse("mcp").unwrap(), What::Mcp);
        assert!(What::parse("server").is_err());
    }
}
