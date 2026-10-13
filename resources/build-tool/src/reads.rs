//! Claude reads every file of mAId's skills, and reads and writes the
//! user's skill state, without asking. Install adds mAId's plugin dir to
//! `permissions.additionalDirectories` and an `Edit` rule for each form of
//! each skill's state dir to `permissions.allow`, in Claude's user settings.
//! See project.md "Skill reads" and "Skill state".
//!
//! Each is an element of a list another part or the user also writes: mAId
//! owns the elements its part list records, and an identical element it
//! found there (adopted); any other element is left alone.

use crate::part::{Item, What, ANOTHER_PART};
use crate::settings::{claude_settings, read_settings, write_settings, NO_CLI};
use crate::shared::{marketplace_root, PLUGIN_ID};
use anyhow::Result;
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

const PERMISSIONS: &str = "permissions";
const NOT_A_LIST: &str = "not a list";
const NO_PLUGIN: &str = "no mAId plugin in the profile";
const NOT_OURS: &str = "config path not of mAId's shape";

/// The skills that keep user state, each in `<config>/<skill>/`. Only these
/// dirs are granted: the config dir also holds the browser allowlist, which
/// an agent must never write (project.md "Browser-enforced allowlist").
pub const STATEFUL_SKILLS: [&str; 1] = ["writing-style"];

/// A list under `permissions` that mAId keeps elements in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum List {
    /// `additionalDirectories`: the plugin dir, read-only.
    Dirs,
    /// `allow`: `Edit` rules on skill state dirs, which also grant reads.
    Allow,
}

impl List {
    pub const ALL: [List; 2] = [List::Dirs, List::Allow];

    pub fn setting(self) -> &'static str {
        match self {
            List::Dirs => "permissions.additionalDirectories",
            List::Allow => "permissions.allow",
        }
    }

    fn key(self) -> &'static str {
        match self {
            List::Dirs => "additionalDirectories",
            List::Allow => "allow",
        }
    }

    /// Whether a recorded value has the shape mAId writes here: a
    /// hand-edited part list cannot make mAId remove another grant.
    fn ours(self, value: &str) -> bool {
        let path = match self {
            List::Dirs => Some(value).filter(|v| {
                Path::new(v).ends_with(Path::new("marketplace/plugins").join(plugin()))
            }),
            List::Allow => value
                .strip_prefix("Edit(/")
                .and_then(|v| v.strip_suffix("/**)"))
                // A glob or rule character would widen or break the rule.
                .filter(|v| !v.contains(['*', '?', '[', ']', '(', ')', '{', '}', '\\', '!']))
                .filter(|v| {
                    Path::new(v)
                        .file_name()
                        .is_some_and(|n| STATEFUL_SKILLS.iter().any(|s| n == *s))
                }),
        };
        path.map(Path::new)
            .is_some_and(|p| p.is_absolute() && p.components().all(|c| c != Component::ParentDir))
    }
}

/// What install wants in one list now, or why it wants nothing.
pub struct Grant {
    pub list: List,
    want: Vec<String>,
    skip: Option<&'static str>,
}

fn plugin() -> &'static str {
    PLUGIN_ID.split('@').next().unwrap_or(PLUGIN_ID)
}

/// mAId's plugin dir on the profile side of the marketplace. Claude reports
/// it as each skill's base dir, so a grant here covers every module.
pub fn plugin_dir(profile: &Path) -> PathBuf {
    marketplace_root(profile).join("plugins").join(plugin())
}

pub fn plugin_grant(profile: &Path) -> Grant {
    let dir = plugin_dir(profile);
    Grant {
        list: List::Dirs,
        skip: (!dir.is_dir()).then_some(NO_PLUGIN),
        want: vec![dir.display().to_string()],
    }
}

/// `Edit` on each skill's state dir as given and as it resolves: Claude
/// checks a path both ways, so behind a symlinked HOME one rule alone is
/// denied. Granted before the dir exists, since a skill creates it. A rule
/// mAId could not later recognise as its own is never written.
pub fn config_grant(config: &Path) -> Grant {
    let config: PathBuf = config.components().collect();
    let want: Vec<String> = STATEFUL_SKILLS
        .iter()
        .flat_map(|skill| {
            let dir = config.join(skill);
            let resolved = resolve(&dir);
            match resolved == dir {
                true => vec![dir],
                false => vec![dir, resolved],
            }
        })
        .map(|p| format!("Edit(/{}/**)", p.display()))
        .collect();
    let shaped = want.iter().all(|r| List::Allow.ours(r));
    Grant {
        list: List::Allow,
        skip: (!shaped).then_some(NOT_OURS),
        want: if shaped { want } else { vec![] },
    }
}

/// `path` with its deepest existing ancestor resolved: the dir itself need
/// not exist yet. A link that leads nowhere cannot be resolved, so the path
/// is kept as given.
fn resolve(path: &Path) -> PathBuf {
    for a in path.ancestors() {
        match fs::canonicalize(a) {
            Ok(c) => return c.join(path.strip_prefix(a).unwrap_or(Path::new(""))),
            Err(_) if fs::symlink_metadata(a).is_ok() => break,
            Err(_) => continue,
        }
    }
    path.to_path_buf()
}

/// The lines to print, and the part-list entries for the setting (none once
/// mAId holds nothing there).
pub struct Placed {
    pub lines: Vec<String>,
    pub items: Vec<Item>,
}

/// The list, or `Err` when something that is not one stands there.
fn elements(map: &Map<String, Value>, key: &str) -> std::result::Result<Vec<Value>, ()> {
    match map.get(PERMISSIONS) {
        None | Some(Value::Null) => Ok(vec![]),
        Some(Value::Object(p)) => match p.get(key) {
            None | Some(Value::Null) => Ok(vec![]),
            Some(Value::Array(a)) => Ok(a.clone()),
            Some(_) => Err(()),
        },
        Some(_) => Err(()),
    }
}

/// Write the list back, dropping it (then `permissions`) when empty.
fn set_elements(map: &mut Map<String, Value>, key: &str, list: Vec<Value>) {
    let perms = map
        .entry(PERMISSIONS)
        .or_insert_with(|| Value::Object(Map::new()));
    if perms.is_null() {
        *perms = Value::Object(Map::new());
    }
    if let Value::Object(p) = perms {
        match list.is_empty() {
            true => p.remove(key),
            false => p.insert(key.into(), Value::Array(list)),
        };
        if p.is_empty() {
            map.remove(PERMISSIONS);
        }
    }
}

fn holds(list: &[Value], element: &str) -> bool {
    list.iter().any(|v| v.as_str() == Some(element))
}

/// What mAId's part list recorded in `list`, while it has mAId's shape.
fn recorded(list: List, own: &[Item]) -> Vec<&str> {
    own.iter()
        .filter_map(|i| i.value.as_deref())
        .filter(|v| list.ours(v))
        .collect()
}

/// Install: add each element mAId wants, or adopt the identical one already
/// there unless another part's list records it (`claimed`). `own` is what
/// mAId's part list recorded for this setting.
pub fn install_claude(
    home: &Path,
    grant: &Grant,
    own: &[Item],
    claimed: &BTreeSet<String>,
    cli_present: bool,
    dry_run: bool,
) -> Result<Placed> {
    let tag = if dry_run { "(dry-run) " } else { "" };
    let (list, setting) = (grant.list, grant.list.setting());
    let at = claude_settings(home);
    let recorded = recorded(list, own);
    let skipped = |why: &str, items: Vec<Item>| Placed {
        lines: vec![format!("{tag}skip          claude {setting} ({why})")],
        items: match items.is_empty() {
            true => vec![Item::skipped(What::Setting, setting, why)],
            false => items,
        },
    };
    // A config path of the wrong shape still sweeps mAId's old rules below:
    // only a missing claude or plugin leaves the file untouched.
    let untouched = grant.skip.filter(|w| *w != NOT_OURS);
    if let Some(why) = (!cli_present).then_some(NO_CLI).or(untouched) {
        // Nothing is written, so an unreadable file is no reason to fail; an
        // element mAId placed before stays recorded.
        let map = read_settings(&at).unwrap_or_default();
        let found = elements(&map, list.key()).unwrap_or_default();
        let kept = own
            .iter()
            .filter(|i| {
                i.value
                    .as_deref()
                    .is_some_and(|v| list.ours(v) && holds(&found, v))
            })
            .cloned()
            .collect();
        return Ok(skipped(why, kept));
    }
    let mut map = read_settings(&at)?;
    let Ok(mut found) = elements(&map, list.key()) else {
        return Ok(skipped(NOT_A_LIST, vec![]));
    };
    let before = found.clone();
    // mAId's elements from an older path are stale, and still mAId's.
    found.retain(|v| {
        v.as_str()
            .is_none_or(|v| !recorded.contains(&v) || grant.want.iter().any(|w| w == v))
    });
    let mut out = Placed {
        lines: vec![],
        items: vec![],
    };
    for want in &grant.want {
        let mine = recorded.contains(&want.as_str());
        let placed = |adopted| Item {
            value: Some(want.clone()),
            adopted,
            ..Item::placed(What::Setting, setting)
        };
        let theirs = || {
            (
                format!("{tag}skip          claude {setting} {want} ({ANOTHER_PART})"),
                Item::skipped(What::Setting, setting, ANOTHER_PART),
            )
        };
        let (line, item) = match (holds(&found, want), mine) {
            (true, true) => (
                format!("{tag}done          claude {setting} {want}"),
                placed(
                    own.iter()
                        .any(|i| i.adopted && i.value.as_ref() == Some(want)),
                ),
            ),
            (true, false) if claimed.contains(want) => theirs(),
            (true, false) => (
                format!("{tag}adopted       claude {setting} {want} (already present, adopted)"),
                placed(true),
            ),
            // Never newly claim what another part's list records (rule 2).
            (false, false) if claimed.contains(want) => theirs(),
            (false, _) => {
                found.push(Value::String(want.clone()));
                (
                    format!("{tag}done          claude {setting} + {want}"),
                    placed(false),
                )
            }
        };
        out.lines.push(line);
        out.items.push(item);
    }
    if let Some(why) = grant.skip {
        out.lines
            .push(format!("{tag}skip          claude {setting} ({why})"));
        out.items.push(Item::skipped(What::Setting, setting, why));
    }
    if found != before && !dry_run {
        set_elements(&mut map, list.key(), found);
        write_settings(&at, &map)?;
    }
    Ok(out)
}

/// Uninstall: remove the elements mAId recorded (placed or adopted), only.
pub fn uninstall_claude(
    home: &Path,
    list: List,
    own: &[Item],
    dry_run: bool,
) -> Result<Vec<String>> {
    let tag = if dry_run { "(dry-run) " } else { "" };
    let setting = list.setting();
    let at = claude_settings(home);
    let mut map = read_settings(&at)?;
    let recorded = recorded(list, own);
    let Ok(mut found) = elements(&map, list.key()) else {
        return Ok(vec![format!(
            "{tag}done          claude {setting} (none of mAId's)"
        )]);
    };
    let gone: Vec<String> = recorded
        .iter()
        .filter(|r| holds(&found, r))
        .map(|r| format!("{tag}removed       claude {setting} {r}"))
        .collect();
    if gone.is_empty() {
        return Ok(vec![format!(
            "{tag}done          claude {setting} (none of mAId's)"
        )]);
    }
    found.retain(|v| v.as_str().is_none_or(|v| !recorded.contains(&v)));
    set_elements(&mut map, list.key(), found);
    if !dry_run {
        write_settings(&at, &map)?;
    }
    Ok(gone)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    const DIRS: &str = "additionalDirectories";

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

    /// One pass of the plugin grant, which holds one element.
    struct One {
        line: String,
        item: Option<Item>,
    }

    fn grant_plugin(
        e: &Env,
        own: Option<&Item>,
        claimed: &BTreeSet<String>,
        cli: bool,
        dry_run: bool,
    ) -> One {
        let own: Vec<Item> = own.into_iter().cloned().collect();
        let p = install_claude(
            &e.home,
            &plugin_grant(&e.profile),
            &own,
            claimed,
            cli,
            dry_run,
        )
        .unwrap();
        assert_eq!(p.items.len(), 1, "{:?}", p.items);
        One {
            line: p.lines.join("\n"),
            item: p.items.into_iter().next(),
        }
    }

    fn install(e: &Env, own: Option<&Item>) -> One {
        grant_plugin(e, own, &BTreeSet::new(), true, false)
    }

    fn unplug(e: &Env, own: Option<&Item>, dry_run: bool) {
        let own: Vec<Item> = own.into_iter().cloned().collect();
        uninstall_claude(&e.home, List::Dirs, &own, dry_run).unwrap();
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
        assert_eq!(item.name, List::Dirs.setting());
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
        unplug(&e, Some(&item), false);
        assert_eq!(settings(&e), serde_json::json!({}));
    }

    #[test]
    fn an_identical_element_another_part_records_is_not_adopted() {
        let e = env();
        seed(&e, serde_json::json!({"permissions": {DIRS: [mine(&e)]}}));
        let theirs = BTreeSet::from([mine(&e)]);
        let out = grant_plugin(&e, None, &theirs, true, false);
        let item = out.item.unwrap();
        assert_eq!(
            (item.skipped.as_deref(), item.adopted),
            (Some(ANOTHER_PART), false)
        );
        unplug(&e, Some(&item), false);
        assert_eq!(
            settings(&e),
            serde_json::json!({"permissions": {DIRS: [mine(&e)]}})
        );
    }

    #[test]
    fn an_element_another_part_records_is_never_added_and_maids_own_stays() {
        let e = env();
        let theirs = BTreeSet::from([mine(&e)]);
        let out = grant_plugin(&e, None, &theirs, true, false);
        assert_eq!(out.item.unwrap().skipped.as_deref(), Some(ANOTHER_PART));
        assert!(!claude_settings(&e.home).exists());
        let item = install(&e, None).item.unwrap();
        let again = grant_plugin(&e, Some(&item), &theirs, true, false);
        assert_eq!(again.item, Some(item));
    }

    #[test]
    fn a_different_element_is_never_touched() {
        let e = env();
        let other = format!("{}/", mine(&e));
        seed(&e, serde_json::json!({"permissions": {DIRS: [other]}}));
        let item = install(&e, None).item.unwrap();
        assert!(!item.adopted);
        unplug(&e, Some(&item), false);
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
        unplug(&e, Some(&item), false);
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
        unplug(&e, None, false);
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
            ..Item::placed(What::Setting, List::Dirs.setting())
        };
        unplug(&e, Some(&forged), false);
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
                ..Item::placed(What::Setting, List::Dirs.setting())
            };
            assert!(recorded(List::Dirs, &[item]).is_empty(), "{r}");
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
        let out = grant_plugin(&e, Some(&item), &BTreeSet::new(), false, false);
        assert_eq!(out.item.unwrap().skipped.as_deref(), Some(NO_CLI));
    }

    #[test]
    fn its_element_from_an_older_profile_path_is_replaced() {
        let e = env();
        let old = Item {
            value: Some("/old/state/maid/marketplace/plugins/maid".into()),
            ..Item::placed(What::Setting, List::Dirs.setting())
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
        let none = grant_plugin(&e, None, &BTreeSet::new(), false, false);
        assert_eq!(none.item.unwrap().skipped.as_deref(), Some(NO_CLI));
        assert!(!claude_settings(&e.home).exists());
        let item = install(&e, None).item.unwrap();
        let bytes = fs::read(claude_settings(&e.home)).unwrap();
        let out = grant_plugin(&e, Some(&item), &BTreeSet::new(), false, false);
        assert_eq!(out.item, Some(item));
        assert_eq!(fs::read(claude_settings(&e.home)).unwrap(), bytes);
    }

    #[test]
    fn dry_run_writes_nothing() {
        let e = env();
        grant_plugin(&e, None, &BTreeSet::new(), true, true);
        assert!(!claude_settings(&e.home).exists());
        let item = install(&e, None).item.unwrap();
        let bytes = fs::read(claude_settings(&e.home)).unwrap();
        unplug(&e, Some(&item), true);
        assert_eq!(fs::read(claude_settings(&e.home)).unwrap(), bytes);
    }

    // ── the config grant ────────────────────────────────────────

    fn rule(p: &Path) -> String {
        format!("Edit(/{}/writing-style/**)", p.display())
    }

    fn config(e: &Env) -> PathBuf {
        fs::canonicalize(e.home.parent().unwrap())
            .unwrap()
            .join("config/maid")
    }

    fn grant_config(e: &Env, own: &[Item], claimed: &BTreeSet<String>) -> Placed {
        install_claude(
            &e.home,
            &config_grant(&config(e)),
            own,
            claimed,
            true,
            false,
        )
        .unwrap()
    }

    #[test]
    fn a_plain_config_path_gets_one_edit_rule_beside_the_users_own() {
        let e = env();
        seed(
            &e,
            serde_json::json!({"permissions": {"allow": ["Bash(ls)"], DIRS: ["/d"]}}),
        );
        let out = grant_config(&e, &[], &BTreeSet::new());
        let want = rule(&config(&e));
        assert!(want.starts_with("Edit(//") && want.ends_with("/config/maid/writing-style/**)"));
        assert_eq!(
            settings(&e),
            serde_json::json!({"permissions": {"allow": ["Bash(ls)", want], DIRS: ["/d"]}})
        );
        assert_eq!(out.items.len(), 1);
        assert_eq!(out.items[0].value.as_deref(), Some(want.as_str()));
        assert_eq!(out.items[0].name, "permissions.allow");
    }

    #[test]
    fn a_config_path_behind_a_symlink_gets_a_rule_for_each_form() {
        let e = env();
        let real = config(&e).parent().unwrap().to_path_buf();
        fs::create_dir_all(&real).unwrap();
        let link = real.parent().unwrap().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        // The dir itself need not exist: its parent resolves.
        let given = link.join("maid");
        let g = config_grant(&given);
        assert_eq!(g.want, vec![rule(&given), rule(&real.join("maid"))]);
        let out = install_claude(&e.home, &g, &[], &BTreeSet::new(), true, false).unwrap();
        assert_eq!(out.items.len(), 2);
        assert_eq!(
            settings(&e)["permissions"]["allow"],
            serde_json::json!([rule(&given), rule(&real.join("maid"))])
        );
        uninstall_claude(&e.home, List::Allow, &out.items, false).unwrap();
        assert_eq!(settings(&e), serde_json::json!({}));
    }

    #[test]
    fn the_config_rules_rerun_byte_identical_and_uninstall_removes_only_them() {
        let e = env();
        seed(
            &e,
            serde_json::json!({"permissions": {"allow": ["Bash(ls)"]}}),
        );
        let first = grant_config(&e, &[], &BTreeSet::new()).items;
        let bytes = fs::read(claude_settings(&e.home)).unwrap();
        let again = grant_config(&e, &first, &BTreeSet::new());
        assert_eq!(fs::read(claude_settings(&e.home)).unwrap(), bytes);
        assert_eq!(again.items, first);
        uninstall_claude(&e.home, List::Allow, &first, false).unwrap();
        assert_eq!(
            settings(&e),
            serde_json::json!({"permissions": {"allow": ["Bash(ls)"]}})
        );
    }

    #[test]
    fn an_identical_rule_is_adopted_unless_another_part_records_it() {
        let e = env();
        let want = rule(&config(&e));
        seed(&e, serde_json::json!({"permissions": {"allow": [want]}}));
        let out = grant_config(&e, &[], &BTreeSet::new());
        assert!(out.items[0].adopted, "{:?}", out.items);
        let theirs = BTreeSet::from([want.clone()]);
        let out = grant_config(&e, &[], &theirs);
        assert_eq!(out.items[0].skipped.as_deref(), Some(ANOTHER_PART));
        uninstall_claude(&e.home, List::Allow, &out.items, false).unwrap();
        assert_eq!(
            settings(&e),
            serde_json::json!({"permissions": {"allow": [want]}})
        );
    }

    #[test]
    fn a_stale_config_rule_is_replaced() {
        let e = env();
        let old = "Edit(//old/config/maid/writing-style/**)".to_string();
        seed(
            &e,
            serde_json::json!({"permissions": {"allow": [old, "Bash(ls)"]}}),
        );
        let recorded = Item {
            value: Some(old),
            ..Item::placed(What::Setting, List::Allow.setting())
        };
        grant_config(&e, &[recorded], &BTreeSet::new());
        assert_eq!(
            settings(&e)["permissions"]["allow"],
            serde_json::json!(["Bash(ls)", rule(&config(&e))])
        );
    }

    #[test]
    fn a_recorded_rule_not_of_maids_shape_is_never_removed() {
        for forged in [
            "Edit(//home/me/notes/**)",
            "Edit(//c/maid/**)",
            "Read(//x/maid/writing-style/**)",
            "Edit(/rel/maid/writing-style/**)",
            "Edit(//a/../maid/writing-style/**)",
            "Edit(//a/*/writing-style/**)",
            "Bash(rm -rf /)",
        ] {
            assert!(!List::Allow.ours(forged), "{forged}");
            let e = env();
            let theirs = serde_json::json!({"permissions": {"allow": [forged]}});
            seed(&e, theirs.clone());
            let item = Item {
                value: Some(forged.into()),
                ..Item::placed(What::Setting, List::Allow.setting())
            };
            uninstall_claude(&e.home, List::Allow, &[item], false).unwrap();
            assert_eq!(settings(&e), theirs);
        }
        assert!(List::Allow.ours("Edit(//c/maid/writing-style/**)"));
    }

    #[test]
    fn with_no_plugin_in_the_profile_the_config_is_still_granted() {
        let e = env();
        fs::remove_dir(plugin_dir(&e.profile)).unwrap();
        assert_eq!(
            install(&e, None).item.unwrap().skipped.as_deref(),
            Some(NO_PLUGIN)
        );
        grant_config(&e, &[], &BTreeSet::new());
        assert_eq!(
            settings(&e),
            serde_json::json!({"permissions": {"allow": [rule(&config(&e))]}})
        );
    }

    #[test]
    fn without_claude_the_config_rules_stay_recorded_while_present() {
        let e = env();
        let items = grant_config(&e, &[], &BTreeSet::new()).items;
        let g = config_grant(&config(&e));
        let out = install_claude(&e.home, &g, &items, &BTreeSet::new(), false, false).unwrap();
        assert_eq!(out.items, items);
        seed(&e, serde_json::json!({}));
        let out = install_claude(&e.home, &g, &items, &BTreeSet::new(), false, false).unwrap();
        assert_eq!(out.items[0].skipped.as_deref(), Some(NO_CLI));
    }

    #[test]
    fn the_browser_allowlist_and_memory_store_are_outside_every_rule() {
        let e = env();
        let config = config(&e);
        for rule in config_grant(&config).want {
            let dir = rule
                .strip_prefix("Edit(/")
                .and_then(|r| r.strip_suffix("/**)"))
                .unwrap()
                .to_string();
            for file in ["browser-allowlist", "memory-store"] {
                assert!(!config.join(file).starts_with(&dir), "{rule} covers {file}");
            }
        }
    }

    #[test]
    fn a_config_path_maid_could_not_recognise_writes_nothing() {
        for bad in [
            "/a/../maid",
            "/a/b*/maid",
            "/a/(x)/maid",
            "/a/{b,c}/maid",
            "/a/!b/maid",
        ] {
            let e = env();
            let g = config_grant(Path::new(bad));
            let out = install_claude(&e.home, &g, &[], &BTreeSet::new(), true, false).unwrap();
            assert_eq!(out.items[0].skipped.as_deref(), Some(NOT_OURS), "{bad}");
            assert!(!claude_settings(&e.home).exists(), "{bad}");
        }
    }

    #[test]
    fn a_trailing_slash_or_dot_in_the_config_path_is_normalised() {
        let g = config_grant(Path::new("/nowhere/./c/maid/"));
        assert_eq!(g.want, vec!["Edit(//nowhere/c/maid/writing-style/**)"]);
    }

    #[test]
    fn a_config_path_of_the_wrong_shape_still_removes_maids_old_rules() {
        let e = env();
        let old = rule(&config(&e));
        let items = grant_config(&e, &[], &BTreeSet::new()).items;
        let g = config_grant(Path::new("/a/b*/maid"));
        let out = install_claude(&e.home, &g, &items, &BTreeSet::new(), true, false).unwrap();
        assert_eq!(out.items.len(), 1);
        assert_eq!(out.items[0].skipped.as_deref(), Some(NOT_OURS));
        assert!(!settings(&e).to_string().contains(&old));
    }
}
