//! Claude reads every file of mAId's skills without asking: install adds
//! mAId's plugin dir to `permissions.additionalDirectories` in Claude's
//! user settings. See project.md "Skill reads".
//!
//! One element of a list another part or the user also writes: mAId owns
//! the element its part list records, and an identical element it found
//! there (adopted); any other element is left alone.

use crate::part::{Item, What, ANOTHER_PART};
use crate::settings::{claude_settings, read_settings, write_settings, Outcome, NO_CLI};
use crate::shared::{marketplace_root, PLUGIN_ID};
use anyhow::Result;
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub const SETTING: &str = "permissions.additionalDirectories";
const PERMISSIONS: &str = "permissions";
const DIRS: &str = "additionalDirectories";
const NOT_A_LIST: &str = "not a list";
const NO_PLUGIN: &str = "no mAId plugin in the profile";

fn plugin() -> &'static str {
    PLUGIN_ID.split('@').next().unwrap_or(PLUGIN_ID)
}

/// mAId's plugin dir on the profile side of the marketplace. Claude reports
/// it as each skill's base dir, so a grant here covers every module.
pub fn plugin_dir(profile: &Path) -> PathBuf {
    marketplace_root(profile).join("plugins").join(plugin())
}

/// What the part list recorded, while it has the shape `plugin_dir` gives:
/// a hand-edited list cannot make mAId remove another grant.
fn recorded(own: Option<&Item>) -> Option<&str> {
    let tail = Path::new("marketplace/plugins").join(plugin());
    own.and_then(|i| i.value.as_deref()).filter(|r| {
        let r = Path::new(r);
        r.is_absolute()
            && r.ends_with(&tail)
            && r.components().all(|c| c != std::path::Component::ParentDir)
    })
}

/// The list, or `Err` when something that is not one stands there.
fn dirs(map: &Map<String, Value>) -> std::result::Result<Vec<Value>, ()> {
    match map.get(PERMISSIONS) {
        None | Some(Value::Null) => Ok(vec![]),
        Some(Value::Object(p)) => match p.get(DIRS) {
            None | Some(Value::Null) => Ok(vec![]),
            Some(Value::Array(a)) => Ok(a.clone()),
            Some(_) => Err(()),
        },
        Some(_) => Err(()),
    }
}

/// Write the list back, dropping it (then `permissions`) when empty.
fn set_dirs(map: &mut Map<String, Value>, list: Vec<Value>) {
    let perms = map
        .entry(PERMISSIONS)
        .or_insert_with(|| Value::Object(Map::new()));
    if perms.is_null() {
        *perms = Value::Object(Map::new());
    }
    if let Value::Object(p) = perms {
        match list.is_empty() {
            true => p.remove(DIRS),
            false => p.insert(DIRS.into(), Value::Array(list)),
        };
        if p.is_empty() {
            map.remove(PERMISSIONS);
        }
    }
}

fn holds(list: &[Value], dir: &str) -> bool {
    list.iter().any(|v| v.as_str() == Some(dir))
}

/// Install: add mAId's element, or adopt the identical one already there
/// unless another part's list records it (`claimed`). `own` is what mAId's
/// part list recorded for this setting.
pub fn install_claude(
    home: &Path,
    profile: &Path,
    own: Option<&Item>,
    claimed: &BTreeSet<String>,
    cli_present: bool,
    dry_run: bool,
) -> Result<Outcome> {
    let tag = if dry_run { "(dry-run) " } else { "" };
    let at = claude_settings(home);
    let recorded = recorded(own);
    let want = plugin_dir(profile);
    let skip = match (cli_present, want.is_dir()) {
        (false, _) => Some(NO_CLI),
        (true, false) => Some(NO_PLUGIN),
        (true, true) => None,
    };
    if let Some(why) = skip {
        // Nothing is written, so an unreadable file is no reason to fail; an
        // element mAId placed before stays recorded.
        let map = read_settings(&at).unwrap_or_default();
        let kept = dirs(&map)
            .ok()
            .zip(recorded)
            .is_some_and(|(l, r)| holds(&l, r));
        return Ok(Outcome {
            line: format!("{tag}skip          claude {SETTING} ({why})"),
            item: Some(match kept {
                true => own.cloned().expect("a recorded value has its item"),
                false => Item::skipped(What::Setting, SETTING, why),
            }),
        });
    }
    let mut map = read_settings(&at)?;
    let Ok(mut list) = dirs(&map) else {
        return Ok(Outcome {
            line: format!("{tag}skip          claude {SETTING} ({NOT_A_LIST})"),
            item: Some(Item::skipped(What::Setting, SETTING, NOT_A_LIST)),
        });
    };
    let want = want.display().to_string();
    let before = list.clone();
    // mAId's element from an older profile path is stale, and still mAId's.
    if let Some(old) = recorded.filter(|r| *r != want) {
        list.retain(|v| v.as_str() != Some(old));
    }
    let placed = |adopted| Item {
        value: Some(want.clone()),
        adopted,
        ..Item::placed(What::Setting, SETTING)
    };
    let (line, item) = match (holds(&list, &want), recorded == Some(want.as_str())) {
        (true, true) => (
            format!("{tag}done          claude {SETTING} {want}"),
            placed(own.is_some_and(|i| i.adopted)),
        ),
        (true, false) if claimed.contains(&want) => (
            format!("{tag}skip          claude {SETTING} {want} ({ANOTHER_PART})"),
            Item::skipped(What::Setting, SETTING, ANOTHER_PART),
        ),
        (true, false) => (
            format!("{tag}adopted       claude {SETTING} {want} (already present, adopted)"),
            placed(true),
        ),
        // Never newly claim what another part's list records (rule 2).
        (false, _) if claimed.contains(&want) && recorded != Some(want.as_str()) => (
            format!("{tag}skip          claude {SETTING} {want} ({ANOTHER_PART})"),
            Item::skipped(What::Setting, SETTING, ANOTHER_PART),
        ),
        (false, _) => {
            list.push(Value::String(want.clone()));
            (
                format!("{tag}done          claude {SETTING} + {want}"),
                placed(false),
            )
        }
    };
    if list != before && !dry_run {
        set_dirs(&mut map, list);
        write_settings(&at, &map)?;
    }
    Ok(Outcome {
        line,
        item: Some(item),
    })
}

/// Uninstall: remove the element mAId recorded (placed or adopted), only.
pub fn uninstall_claude(home: &Path, own: Option<&Item>, dry_run: bool) -> Result<Outcome> {
    let tag = if dry_run { "(dry-run) " } else { "" };
    let at = claude_settings(home);
    let mut map = read_settings(&at)?;
    let line = match (dirs(&map), recorded(own)) {
        (Ok(mut list), Some(r)) if holds(&list, r) => {
            list.retain(|v| v.as_str() != Some(r));
            set_dirs(&mut map, list);
            if !dry_run {
                write_settings(&at, &map)?;
            }
            format!("{tag}removed       claude {SETTING} {r}")
        }
        _ => format!("{tag}done          claude {SETTING} (none of mAId's)"),
    };
    Ok(Outcome { line, item: None })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    struct Env {
        _tmp: TempDir,
        home: PathBuf,
        profile: PathBuf,
    }

    fn env() -> Env {
        let tmp = TempDir::new().unwrap();
        let profile = tmp.path().join("state/maid/profile");
        fs::create_dir_all(plugin_dir(&profile)).unwrap();
        Env {
            home: tmp.path().join("home"),
            profile,
            _tmp: tmp,
        }
    }

    fn mine(e: &Env) -> String {
        plugin_dir(&e.profile).display().to_string()
    }

    fn seed(e: &Env, v: Value) {
        fs::create_dir_all(e.home.join(".claude")).unwrap();
        fs::write(claude_settings(&e.home), v.to_string()).unwrap();
    }

    fn settings(e: &Env) -> Value {
        serde_json::from_str(&fs::read_to_string(claude_settings(&e.home)).unwrap()).unwrap()
    }

    fn install(e: &Env, own: Option<&Item>) -> Outcome {
        install_claude(&e.home, &e.profile, own, &BTreeSet::new(), true, false).unwrap()
    }

    #[test]
    fn the_element_is_the_plugin_dir_beside_the_profile() {
        let e = env();
        assert!(mine(&e).ends_with("state/maid/marketplace/plugins/maid"));
    }

    #[test]
    fn install_adds_its_element_to_no_settings_file() {
        let e = env();
        let out = install(&e, None);
        assert_eq!(
            settings(&e),
            serde_json::json!({"permissions": {DIRS: [mine(&e)]}})
        );
        let item = out.item.unwrap();
        assert_eq!((item.value, item.adopted), (Some(mine(&e)), false));
        assert_eq!(item.name, SETTING);
    }

    #[test]
    fn install_appends_and_keeps_every_other_key_and_element() {
        let e = env();
        let seeded = serde_json::json!({
            "z": 1,
            "permissions": {"allow": ["Read(//x/**)"], DIRS: ["/other"], "defaultMode": "auto"}
        });
        seed(&e, seeded);
        install(&e, None);
        assert_eq!(
            settings(&e),
            serde_json::json!({
                "z": 1,
                "permissions": {"allow": ["Read(//x/**)"], DIRS: ["/other", mine(&e)], "defaultMode": "auto"}
            })
        );
    }

    #[test]
    fn a_rerun_leaves_the_bytes_alone() {
        let e = env();
        let first = install(&e, None).item;
        let bytes = fs::read(claude_settings(&e.home)).unwrap();
        let again = install(&e, first.as_ref());
        assert_eq!(fs::read(claude_settings(&e.home)).unwrap(), bytes);
        assert_eq!(again.item, first);
    }

    #[test]
    fn an_identical_element_found_there_is_adopted_and_uninstall_removes_it() {
        let e = env();
        seed(&e, serde_json::json!({"permissions": {DIRS: [mine(&e)]}}));
        let bytes = fs::read(claude_settings(&e.home)).unwrap();
        let out = install(&e, None);
        assert!(
            out.line.contains("already present, adopted"),
            "{}",
            out.line
        );
        assert_eq!(fs::read(claude_settings(&e.home)).unwrap(), bytes);
        let item = out.item.unwrap();
        assert!(item.adopted && item.skipped.is_none());
        // Stays adopted on the next install.
        assert!(install(&e, Some(&item)).item.unwrap().adopted);
        uninstall_claude(&e.home, Some(&item), false).unwrap();
        assert_eq!(settings(&e), serde_json::json!({}));
    }

    #[test]
    fn an_identical_element_another_part_records_is_not_adopted() {
        let e = env();
        seed(&e, serde_json::json!({"permissions": {DIRS: [mine(&e)]}}));
        let theirs = BTreeSet::from([mine(&e)]);
        let out = install_claude(&e.home, &e.profile, None, &theirs, true, false).unwrap();
        let item = out.item.unwrap();
        assert_eq!(
            (item.skipped.as_deref(), item.adopted),
            (Some(ANOTHER_PART), false)
        );
        uninstall_claude(&e.home, Some(&item), false).unwrap();
        assert_eq!(
            settings(&e),
            serde_json::json!({"permissions": {DIRS: [mine(&e)]}})
        );
    }

    #[test]
    fn an_element_another_part_records_is_never_added_and_maids_own_stays() {
        let e = env();
        let theirs = BTreeSet::from([mine(&e)]);
        let out = install_claude(&e.home, &e.profile, None, &theirs, true, false).unwrap();
        assert_eq!(out.item.unwrap().skipped.as_deref(), Some(ANOTHER_PART));
        assert!(!claude_settings(&e.home).exists());
        let item = install(&e, None).item.unwrap();
        let again = install_claude(&e.home, &e.profile, Some(&item), &theirs, true, false).unwrap();
        assert_eq!(again.item, Some(item));
    }

    #[test]
    fn a_different_element_is_never_touched() {
        let e = env();
        let other = format!("{}/", mine(&e));
        seed(&e, serde_json::json!({"permissions": {DIRS: [other]}}));
        let item = install(&e, None).item.unwrap();
        assert!(!item.adopted);
        uninstall_claude(&e.home, Some(&item), false).unwrap();
        assert_eq!(
            settings(&e),
            serde_json::json!({"permissions": {DIRS: [other]}})
        );
    }

    #[test]
    fn uninstall_removes_only_the_recorded_element() {
        let e = env();
        seed(
            &e,
            serde_json::json!({"permissions": {DIRS: ["/a"], "allow": []}}),
        );
        let item = install(&e, None).item.unwrap();
        uninstall_claude(&e.home, Some(&item), false).unwrap();
        assert_eq!(
            settings(&e),
            serde_json::json!({"permissions": {DIRS: ["/a"], "allow": []}})
        );
    }

    #[test]
    fn uninstall_with_nothing_recorded_changes_nothing() {
        let e = env();
        seed(&e, serde_json::json!({"permissions": {DIRS: [mine(&e)]}}));
        let bytes = fs::read(claude_settings(&e.home)).unwrap();
        uninstall_claude(&e.home, None, false).unwrap();
        assert_eq!(fs::read(claude_settings(&e.home)).unwrap(), bytes);
    }

    #[test]
    fn an_empty_list_gets_the_element() {
        let e = env();
        seed(&e, serde_json::json!({"permissions": {DIRS: []}}));
        install(&e, None);
        assert_eq!(
            settings(&e),
            serde_json::json!({"permissions": {DIRS: [mine(&e)]}})
        );
    }

    #[test]
    fn a_null_permissions_gets_the_element() {
        let e = env();
        seed(&e, serde_json::json!({"permissions": null}));
        install(&e, None);
        assert_eq!(
            settings(&e),
            serde_json::json!({"permissions": {DIRS: [mine(&e)]}})
        );
    }

    #[test]
    fn a_recorded_value_of_another_shape_is_not_maids() {
        let e = env();
        let theirs = serde_json::json!({"permissions": {DIRS: ["/home/me/notes"]}});
        seed(&e, theirs.clone());
        let forged = Item {
            value: Some("/home/me/notes".into()),
            ..Item::placed(What::Setting, SETTING)
        };
        uninstall_claude(&e.home, Some(&forged), false).unwrap();
        assert_eq!(settings(&e), theirs);
        let out = install(&e, Some(&forged));
        assert!(!out.item.unwrap().adopted);
        assert_eq!(
            settings(&e),
            serde_json::json!({"permissions": {DIRS: ["/home/me/notes", mine(&e)]}})
        );
    }

    #[test]
    fn a_relative_or_dotted_recorded_value_is_not_maids() {
        for r in ["marketplace/plugins/maid", "/a/../marketplace/plugins/maid"] {
            let item = Item {
                value: Some(r.into()),
                ..Item::placed(What::Setting, SETTING)
            };
            assert_eq!(recorded(Some(&item)), None, "{r}");
        }
    }

    #[test]
    fn with_no_plugin_in_the_profile_nothing_is_written() {
        let e = env();
        fs::remove_dir(plugin_dir(&e.profile)).unwrap();
        let item = install(&e, None).item.unwrap();
        assert_eq!(item.skipped.as_deref(), Some(NO_PLUGIN));
        assert!(!claude_settings(&e.home).exists());
    }

    #[test]
    fn without_claude_an_element_no_longer_there_is_recorded_skipped() {
        let e = env();
        let item = install(&e, None).item.unwrap();
        seed(&e, serde_json::json!({}));
        let out = install_claude(
            &e.home,
            &e.profile,
            Some(&item),
            &BTreeSet::new(),
            false,
            false,
        )
        .unwrap();
        assert_eq!(out.item.unwrap().skipped.as_deref(), Some(NO_CLI));
    }

    #[test]
    fn its_element_from_an_older_profile_path_is_replaced() {
        let e = env();
        let old = Item {
            value: Some("/old/state/maid/marketplace/plugins/maid".into()),
            ..Item::placed(What::Setting, SETTING)
        };
        seed(
            &e,
            serde_json::json!({"permissions": {DIRS: ["/old/state/maid/marketplace/plugins/maid", "/b"]}}),
        );
        install(&e, Some(&old));
        assert_eq!(
            settings(&e),
            serde_json::json!({"permissions": {DIRS: ["/b", mine(&e)]}})
        );
    }

    #[test]
    fn a_value_that_is_not_a_list_is_left_alone_and_reported() {
        for text in [
            r#"{"permissions": {"additionalDirectories": "/x"}}"#,
            r#"{"permissions": []}"#,
        ] {
            let e = env();
            fs::create_dir_all(e.home.join(".claude")).unwrap();
            fs::write(claude_settings(&e.home), text).unwrap();
            let item = install(&e, None).item.unwrap();
            assert_eq!(item.skipped.as_deref(), Some(NOT_A_LIST));
            assert_eq!(fs::read_to_string(claude_settings(&e.home)).unwrap(), text);
        }
    }

    #[test]
    fn without_claude_nothing_is_written_and_a_placed_element_stays_recorded() {
        let e = env();
        let none =
            install_claude(&e.home, &e.profile, None, &BTreeSet::new(), false, false).unwrap();
        assert_eq!(none.item.unwrap().skipped.as_deref(), Some(NO_CLI));
        assert!(!claude_settings(&e.home).exists());
        let item = install(&e, None).item.unwrap();
        let bytes = fs::read(claude_settings(&e.home)).unwrap();
        let out = install_claude(
            &e.home,
            &e.profile,
            Some(&item),
            &BTreeSet::new(),
            false,
            false,
        )
        .unwrap();
        assert_eq!(out.item, Some(item));
        assert_eq!(fs::read(claude_settings(&e.home)).unwrap(), bytes);
    }

    #[test]
    fn dry_run_writes_nothing() {
        let e = env();
        install_claude(&e.home, &e.profile, None, &BTreeSet::new(), true, true).unwrap();
        assert!(!claude_settings(&e.home).exists());
        let item = install(&e, None).item.unwrap();
        let bytes = fs::read(claude_settings(&e.home)).unwrap();
        uninstall_claude(&e.home, Some(&item), true).unwrap();
        assert_eq!(fs::read(claude_settings(&e.home)).unwrap(), bytes);
    }
}
