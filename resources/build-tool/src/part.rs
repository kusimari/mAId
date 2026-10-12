//! mAId's part list: what this repo placed on each base harness, at
//! `<parts dir>/maid-harness.json`. The format is a contract with other
//! repos' parts and the whole-status that reads them all; see project.md
//! "My harness".
//!
//! This module is the only writer. It writes its own file and nothing
//! else in the parts dir; it reads the others only for what they claim.

use crate::shared::Agent;
use anyhow::{anyhow, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub const PART: &str = "maid-harness";
/// Why a value is not adopted: another part's list records it.
pub const ANOTHER_PART: &str = "another part's";
const FORMAT: u32 = 1;

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

/// One thing a part placed, or meant to place and skipped.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Item {
    pub kind: What,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped: Option<String>,
    /// For a setting: the value the part set, so it can tell its own value
    /// from another part's later.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// The part found this exact value already there and took it over: it
    /// owns it as if it had written it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub adopted: bool,
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
        }
    }

    pub fn skipped(kind: What, name: impl Into<String>, why: impl Into<String>) -> Item {
        Item {
            skipped: Some(why.into()),
            ..Item::placed(kind, name)
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct PartList {
    format: u32,
    part: String,
    harnesses: BTreeMap<String, Vec<Item>>,
}

/// The file mAId's part list lives in.
pub fn path(dir: &Path) -> PathBuf {
    dir.join(format!("{PART}.json"))
}

/// mAId's list, if there is one, refusing a format or part it does not
/// know.
fn read(dir: &Path) -> Result<Option<PartList>> {
    let at = path(dir);
    let list: PartList = match fs::read_to_string(&at) {
        Ok(text) => serde_json::from_str(&text)
            .with_context(|| format!("{} is not a part list", at.display()))?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).with_context(|| format!("cannot read {}", at.display())),
    };
    ensure!(
        list.format == FORMAT && list.part == PART,
        "{} is format {} of part {:?}; refusing to read or rewrite it",
        at.display(),
        list.format,
        list.part
    );
    Ok(Some(list))
}

/// What mAId's list says it placed on `agent`, skips left out: what an
/// earlier install added and so may remove. No list owns nothing.
pub fn placed(dir: &Path, agent: Agent) -> Result<Vec<Item>> {
    let Some(mut list) = read(dir)? else {
        return Ok(vec![]);
    };
    let mut items = list.harnesses.remove(agent.cli()).unwrap_or_default();
    items.retain(|i| i.skipped.is_none());
    Ok(items)
}

/// The values of setting `name` on `agent` that another part's list
/// records as placed: theirs, so never adopted or written (rule 2). A list
/// that cannot be read, or of a format other than 1, claims nothing.
pub fn claimed_by_others(dir: &Path, agent: Agent, name: &str) -> BTreeSet<String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return BTreeSet::new();
    };
    let own = format!("{PART}.json");
    let lists = entries.flatten().filter_map(|e| {
        let file = e.file_name().into_string().ok()?;
        let stem = file.strip_suffix(".json")?;
        if stem.is_empty() || stem.starts_with('.') || file == own {
            return None;
        }
        serde_json::from_str::<serde_json::Value>(&fs::read_to_string(e.path()).ok()?)
            .ok()
            .filter(|l| l["format"].as_f64() == Some(1.0))
    });
    lists
        .flat_map(|list| match &list["harnesses"][agent.cli()] {
            serde_json::Value::Array(items) => items.clone(),
            _ => vec![],
        })
        .filter(|i| i["kind"] == "setting" && i["name"] == name && i["skipped"].is_null())
        .filter_map(|i| i["value"].as_str().map(str::to_string))
        .collect()
}

/// Replace `agent`'s entries of `kinds` in `scope` with `items`, keeping
/// every other entry. Recording in a scope also drops an unscoped skip of
/// those kinds: it was skipped for want of a scope. The file goes when
/// nothing is left in it.
pub fn record(
    dir: &Path,
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
    rewrite(dir, agent, replaced, items)
}

/// Replace `agent`'s entry for the setting `name` with `item` (none clears
/// it), keeping every other setting: each step owns its own setting.
pub fn record_setting(dir: &Path, agent: Agent, name: &str, item: Option<Item>) -> Result<()> {
    let replaced = |i: &Item| i.kind == What::Setting && i.name == name;
    rewrite(dir, agent, replaced, item.into_iter().collect())
}

fn rewrite(
    dir: &Path,
    agent: Agent,
    replaced: impl Fn(&Item) -> bool,
    items: Vec<Item>,
) -> Result<()> {
    let at = path(dir);
    let mut list = read(dir)?.unwrap_or_else(|| PartList {
        format: FORMAT,
        part: PART.into(),
        harnesses: BTreeMap::new(),
    });
    let entries = list.harnesses.entry(agent.cli().into()).or_default();
    entries.retain(|i| !replaced(i));
    entries.extend(items);
    entries.sort();
    entries.dedup();
    list.harnesses.retain(|_, entries| !entries.is_empty());

    if list.harnesses.is_empty() {
        return match fs::remove_file(&at) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        };
    }
    fs::create_dir_all(dir)?;
    // Atomic, so a reader never sees half a list.
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    writeln!(tmp, "{}", serde_json::to_string_pretty(&list)?)?;
    tmp.persist(&at)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const MCP: &[What] = &[What::Mcp];
    const SKILL: &[What] = &[What::Skill];

    fn read(dir: &Path) -> PartList {
        serde_json::from_str(&fs::read_to_string(path(dir)).unwrap()).unwrap()
    }

    fn entries(dir: &Path, agent: Agent) -> Vec<Item> {
        read(dir).harnesses[agent.cli()].clone()
    }

    #[test]
    fn first_record_writes_the_part_and_its_entries() {
        let dir = TempDir::new().unwrap();
        let skill = Item::placed(What::Skill, "notes");
        record(dir.path(), Agent::Kiro, SKILL, None, vec![skill.clone()]).unwrap();
        let list = read(dir.path());
        assert_eq!((list.format, list.part.as_str()), (1, "maid-harness"));
        assert_eq!(list.harnesses["kiro-cli"], vec![skill]);
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
            vec![Item::skipped(What::Mcp, "browser", "no sub-agent")],
        )
        .unwrap();
        record(dir.path(), Agent::Kiro, MCP, None, vec![]).unwrap();
        assert_eq!(entries(dir.path(), Agent::Kiro), vec![mcp]);
    }

    /// Skipped for want of a kiro sub-agent, then installed into one, then
    /// uninstalled from it: nothing is left.
    #[test]
    fn recording_in_a_scope_drops_the_unscoped_skip() {
        let dir = TempDir::new().unwrap();
        let skip = Item::skipped(What::Mcp, "browser", "no sub-agent named");
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
        let text = r#"{"format":2,"part":"maid-harness","harnesses":{},"more":1}"#;
        fs::write(path(dir.path()), text).unwrap();
        let err = record(dir.path(), Agent::Kiro, SKILL, None, vec![]).unwrap_err();
        assert!(err.to_string().contains("format 2"), "{err}");
        assert_eq!(fs::read_to_string(path(dir.path())).unwrap(), text);
    }

    #[test]
    fn another_parts_list_is_left_alone() {
        let dir = TempDir::new().unwrap();
        let text = r#"{"format":1,"part":"other","harnesses":{}}"#;
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
        let other = dir.path().join("another-part.json");
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
    fn other_parts_claim_only_their_placed_values_of_that_setting() {
        let dir = TempDir::new().unwrap();
        let write = |file: &str, text: &str| fs::write(dir.path().join(file), text).unwrap();
        write(
            "other.json",
            r#"{"format":1,"part":"other","harnesses":{"claude":[
                {"kind":"setting","name":"s","value":"/theirs"},
                {"kind":"setting","name":"s","value":"/skipped","skipped":"why"},
                {"kind":"setting","name":"t","value":"/t"},
                {"kind":"new-kind","name":"s","value":"/kind"}],
              "codex":[{"kind":"setting","name":"s","value":"/codex"}]}}"#,
        );
        write(
            ".hidden.json",
            r#"{"format":1,"harnesses":{"claude":[{"kind":"setting","name":"s","value":"/dot"}]}}"#,
        );
        write("broken.json", "not json");
        write(
            "future.json",
            r#"{"format":2,"harnesses":{"claude":[{"kind":"setting","name":"s","value":"/future"}]}}"#,
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
        let text = r#"{"format":2,"part":"maid-harness","harnesses":{"claude":[{"kind":"plugin","name":"p"}]}}"#;
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

    #[test]
    fn kinds_parse_by_their_format_name() {
        assert_eq!(What::parse("mcp").unwrap(), What::Mcp);
        assert!(What::parse("server").is_err());
    }
}
