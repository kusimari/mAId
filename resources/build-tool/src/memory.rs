//! mAId's memory on Claude Code: the personal store, the `personal-memory`
//! server bound to it, and Claude's own automatic memory turned off. See
//! project.md "My harness", Memory.

use crate::part::{Item, What, ANOTHER_PART};
use crate::settings::{
    claude_settings, read_settings, write_atomic, write_settings, Outcome, NO_CLI,
};
use anyhow::{anyhow, bail, Context, Result};
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The name mAId registers its memory server under.
pub const SERVER: &str = "personal-memory";
/// Its declaration in the profile (project.md "My harness", Memory).
pub const MANIFEST: &str = "share/maid/servers/personal-memory/server.json";
/// pensieve's index, relative to the store: the value install binds to the
/// declaration's `index` key, so the selector reads it from the map.
pub const INDEX: &str = "memory/MEMORY.md";
/// Claude's automatic memory, off. A string, so rule 14 can check it.
pub const SETTING: &str = "env.CLAUDE_CODE_DISABLE_AUTO_MEMORY";
const ENV: &str = "env";
const VAR: &str = "CLAUDE_CODE_DISABLE_AUTO_MEMORY";
const OFF: &str = "1";
/// What an older mAId pointed Claude's memory at; only removed now.
pub const LEGACY_SETTING: &str = "autoMemoryDirectory";

/// Why install did not place something. Fixed phrases, no paths.
pub const OTHER_VALUE: &str = "not mAId's";
pub const STORE_MISSING: &str = "personal store missing";
pub const NO_STORE: &str = "no personal store set";
pub const NO_SERVER: &str = "no personal-memory server";
pub const ADD_FAILED: &str = "mcp add failed";
pub const REMOVE_FAILED: &str = "mcp remove failed";
const ENV_NOT_OBJECT: &str = "env is not an object";

// ─────────────────────────────────────────────────────────────────
// The personal store: one path in mAId's own config file.
// ─────────────────────────────────────────────────────────────────

/// The configured store; none when the file is absent or empty. Whether
/// it exists is the caller's to check.
pub fn store(file: &Path) -> Result<Option<PathBuf>> {
    match fs::read_to_string(file) {
        Ok(text) if !text.trim().is_empty() => Ok(Some(PathBuf::from(text.trim()))),
        Ok(_) => Ok(None),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("cannot read {}", file.display())),
    }
}

/// Set the store to `path`, an existing directory.
pub fn set_store(file: &Path, path: &Path) -> Result<String> {
    let path = fs::canonicalize(path)
        .ok()
        .filter(|p| p.is_dir())
        .ok_or_else(|| {
            anyhow!(
                "{} is not a directory; clone the store first",
                path.display()
            )
        })?;
    write_atomic(file, format!("{}\n", path.display()).as_bytes())?;
    Ok(format!(
        "store    {}\nrun `just install` to apply to claude",
        path.display()
    ))
}

// ─────────────────────────────────────────────────────────────────
// The server: `personal-memory` in claude's user MCP config.
//
// The entry is mAId's while its command is the profile's pensieve; any
// other same-named entry, or one another part's list records, is left.
// claude is changed only through its own `mcp add/remove`.
// ─────────────────────────────────────────────────────────────────

fn claude_json(home: &Path) -> PathBuf {
    home.join(".claude.json")
}

/// The registered entry, if any. A config that cannot be read stops the
/// step: whether mAId's entry is there is unknown.
fn registered(home: &Path) -> Result<Option<Value>> {
    let at = claude_json(home);
    let map = read_settings(&at)?;
    match map.get("mcpServers") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Object(servers)) => Ok(servers.get(SERVER).cloned()),
        Some(_) => bail!("{} mcpServers is not an object", at.display()),
    }
}

fn is_ours(entry: &Value, launcher: &Path) -> bool {
    entry["command"].as_str() == Some(&launcher.display().to_string())
}

fn args_for(store: &Path) -> Vec<String> {
    vec!["--store".into(), store.display().to_string()]
}

fn same_args(entry: &Value, want: &[String]) -> bool {
    entry["args"].as_array().is_some_and(|a| {
        a.iter()
            .map(Value::as_str)
            .eq(want.iter().map(|s| Some(s.as_str())))
    })
}

/// Run `claude mcp <args>` on `home`; `Err` carries claude's first line.
fn claude_mcp(home: &Path, args: &[&str]) -> std::result::Result<(), String> {
    let out = Command::new("claude")
        .env("HOME", home)
        .arg("mcp")
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    match out.status.success() {
        true => Ok(()),
        false => {
            let text = String::from_utf8_lossy(&out.stderr).to_string()
                + &String::from_utf8_lossy(&out.stdout);
            Err(text.lines().next().unwrap_or("no output").to_string())
        }
    }
}

fn remove_server(home: &Path) -> std::result::Result<(), String> {
    claude_mcp(home, &["remove", "-s", "user", SERVER])
}

/// Remove the entry, or on a dry run report that it would be.
fn remove_unless(home: &Path, dry_run: bool) -> std::result::Result<(), String> {
    match dry_run {
        true => Ok(()),
        false => remove_server(home),
    }
}

/// What install meets on claude for the server.
pub struct ServerPlan<'a> {
    /// The profile's `bin/pensieve`.
    pub launcher: &'a Path,
    /// None when mAId's config names no store.
    pub store: Option<&'a Path>,
    /// Another part's list records `personal-memory` on claude as placed.
    pub claimed: bool,
    pub cli_present: bool,
}

/// One pass over the server: what to print and record, and whether a
/// claude command failed (the run then exits non-zero).
pub struct ServerOutcome {
    pub out: Outcome,
    pub failed: bool,
}

fn ok(line: String, item: Option<Item>) -> ServerOutcome {
    ServerOutcome {
        out: Outcome { line, item },
        failed: false,
    }
}

fn failed(line: String, item: Option<Item>) -> ServerOutcome {
    ServerOutcome {
        out: Outcome { line, item },
        failed: true,
    }
}

pub fn install_server(home: &Path, plan: &ServerPlan, dry_run: bool) -> Result<ServerOutcome> {
    let tag = if dry_run { "(dry-run) " } else { "" };
    let placed = || Some(Item::placed(What::Mcp, SERVER));
    let skipped = |why: &str| Some(Item::skipped(What::Mcp, SERVER, why));
    let skip = |why: &str| format!("{tag}skip          claude mcp {SERVER} ({why})");
    if !plan.cli_present {
        // Nothing can change; an entry mAId placed stays recorded as placed.
        let kept = registered(home)
            .ok()
            .flatten()
            .is_some_and(|e| is_ours(&e, plan.launcher));
        return Ok(ok(
            skip(NO_CLI),
            if kept { placed() } else { skipped(NO_CLI) },
        ));
    }
    let entry = registered(home)?;
    let ours = entry.as_ref().is_some_and(|e| is_ours(e, plan.launcher));
    if plan.claimed {
        return Ok(ok(skip(ANOTHER_PART), skipped(ANOTHER_PART)));
    }
    if entry.is_some() && !ours {
        return Ok(ok(skip(OTHER_VALUE), skipped(OTHER_VALUE)));
    }
    let store = match plan.store {
        Some(s) if s.is_dir() => s,
        _ => {
            let (why, said) = match plan.store {
                None => (
                    NO_STORE,
                    format!("{NO_STORE}; run `just memory-store <path>`"),
                ),
                Some(_) => (STORE_MISSING, STORE_MISSING.to_string()),
            };
            if !ours {
                return Ok(ok(skip(&said), skipped(why)));
            }
            if let Err(first) = remove_unless(home, dry_run) {
                return Ok(failed(
                    skip(&format!("{said}; {REMOVE_FAILED}: {first}")),
                    placed(),
                ));
            }
            return Ok(ok(
                format!("{tag}removed       claude mcp {SERVER} ({said})"),
                skipped(why),
            ));
        }
    };
    let want = args_for(store);
    let done = format!(
        "{tag}done          claude mcp {SERVER} -> {}",
        store.display()
    );
    if dry_run || entry.as_ref().is_some_and(|e| ours && same_args(e, &want)) {
        return Ok(ok(done, placed()));
    }
    // `mcp add` refuses a name that is there: an older store path goes first.
    if ours {
        if let Err(first) = remove_server(home) {
            return Ok(failed(skip(&format!("{REMOVE_FAILED}: {first}")), placed()));
        }
    }
    let launcher = plan.launcher.display().to_string();
    let mut add = vec!["add", "-s", "user", SERVER, "--", launcher.as_str()];
    add.extend(want.iter().map(String::as_str));
    Ok(match claude_mcp(home, &add) {
        Ok(()) => ok(done, placed()),
        Err(first) => failed(skip(&format!("{ADD_FAILED}: {first}")), skipped(ADD_FAILED)),
    })
}

/// Uninstall: remove the entry only while it is mAId's and no other part's
/// list records it. Without claude, or when claude fails to remove it, it
/// stays recorded.
pub fn uninstall_server(
    home: &Path,
    launcher: &Path,
    claimed: bool,
    cli_present: bool,
    dry_run: bool,
) -> Result<ServerOutcome> {
    let tag = if dry_run { "(dry-run) " } else { "" };
    let entry = registered(home)?;
    let ours = entry.as_ref().is_some_and(|e| is_ours(e, launcher)) && !claimed;
    let kept = |why: &str| format!("{tag}kept          claude mcp {SERVER} ({why})");
    Ok(match (entry.is_some(), ours, cli_present) {
        (false, _, _) => ok(
            format!("{tag}done          claude mcp {SERVER} (not registered)"),
            None,
        ),
        (true, false, _) if claimed => ok(kept(ANOTHER_PART), None),
        (true, false, _) => ok(kept(OTHER_VALUE), None),
        (true, true, false) => ok(kept(NO_CLI), Some(Item::placed(What::Mcp, SERVER))),
        (true, true, true) => match remove_unless(home, dry_run) {
            Ok(()) => ok(format!("{tag}removed       claude mcp {SERVER}"), None),
            Err(first) => failed(
                kept(&format!("{REMOVE_FAILED}: {first}")),
                Some(Item::placed(What::Mcp, SERVER)),
            ),
        },
    })
}

// ─────────────────────────────────────────────────────────────────
// Claude's automatic memory, off: one value (rules 2 and 3).
// ─────────────────────────────────────────────────────────────────

/// What mAId owns of the setting: what its part list says it set or
/// adopted, and the values other parts' lists record (never taken).
#[derive(Debug, Default)]
pub struct Ownership {
    pub recorded: Option<String>,
    pub adopted: bool,
    pub claimed: BTreeSet<String>,
}

/// The `env` object, or `Err` when something that is not one stands there.
fn env_of(map: &Map<String, Value>) -> std::result::Result<Option<&Map<String, Value>>, ()> {
    match map.get(ENV) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Object(env)) => Ok(Some(env)),
        Some(_) => Err(()),
    }
}

/// An absent key, `null` and `""` all count as empty.
fn current(map: &Map<String, Value>) -> Option<Value> {
    env_of(map)
        .ok()
        .flatten()
        .and_then(|env| env.get(VAR))
        .filter(|v| !v.is_null() && v.as_str() != Some(""))
        .cloned()
}

fn set_var(map: &mut Map<String, Value>, value: Option<Value>) {
    let env = map.entry(ENV).or_insert_with(|| Value::Object(Map::new()));
    if env.is_null() {
        *env = Value::Object(Map::new());
    }
    if let Value::Object(e) = env {
        match value {
            Some(v) => e.insert(VAR.into(), v),
            None => e.remove(VAR),
        };
        if e.is_empty() {
            map.remove(ENV);
        }
    }
}

/// Install: turn Claude's own memory off, only while mAId's own server is
/// registered (`server`); otherwise give back mAId's value if it holds one.
/// With no claude on PATH nothing is written, and mAId's value stays its.
/// A part with a server that follows the same rules takes the value on at
/// its next install.
pub fn install_off(
    home: &Path,
    own: &Ownership,
    server: bool,
    cli_present: bool,
    dry_run: bool,
) -> Result<Outcome> {
    let tag = if dry_run { "(dry-run) " } else { "" };
    let at = claude_settings(home);
    let off = Value::String(OFF.into());
    let mine = |v: &Value| own.recorded.is_some() && v.as_str() == own.recorded.as_deref();
    let placed = |adopted| Item {
        value: Some(OFF.into()),
        adopted,
        ..Item::placed(What::Setting, SETTING)
    };
    let skip = |why: &str| Outcome {
        line: format!("{tag}skip          claude {SETTING} ({why})"),
        item: Some(Item::skipped(What::Setting, SETTING, why)),
    };
    if !cli_present {
        // Nothing is written, so an unreadable file is no reason to fail.
        let map = read_settings(&at).unwrap_or_default();
        return Ok(match current(&map).filter(|v| mine(v)) {
            Some(_) => Outcome {
                line: format!("{tag}skip          claude {SETTING} ({NO_CLI})"),
                item: Some(placed(own.adopted)),
            },
            None => skip(NO_CLI),
        });
    }
    let mut map = read_settings(&at)?;
    if env_of(&map).is_err() {
        return Ok(skip(ENV_NOT_OBJECT));
    }
    if !server {
        if !current(&map).is_some_and(|v| mine(&v)) {
            return Ok(skip(NO_SERVER));
        }
        set_var(&mut map, None);
        if !dry_run {
            write_settings(&at, &map)?;
        }
        return Ok(Outcome {
            line: format!("{tag}removed       claude {SETTING} ({NO_SERVER})"),
            item: Some(Item::skipped(What::Setting, SETTING, NO_SERVER)),
        });
    }
    let claimed = own.claimed.contains(OFF);
    Ok(match current(&map) {
        Some(v) if mine(&v) => Outcome {
            line: format!("{tag}done          claude {SETTING} = {OFF}"),
            item: Some(placed(own.adopted)),
        },
        Some(v) if v == off && claimed => skip(ANOTHER_PART),
        Some(v) if v == off => Outcome {
            line: format!("{tag}adopted       claude {SETTING} = {OFF} (already present, adopted)"),
            item: Some(placed(true)),
        },
        Some(_) => skip(OTHER_VALUE),
        // Never newly write what another part's list records (rule 2).
        None if claimed => skip(ANOTHER_PART),
        None => {
            set_var(&mut map, Some(off));
            if !dry_run {
                write_settings(&at, &map)?;
            }
            Outcome {
                line: format!("{tag}done          claude {SETTING} = {OFF}"),
                item: Some(placed(false)),
            }
        }
    })
}

/// Uninstall: remove the value only while it is mAId's.
pub fn uninstall_off(home: &Path, own: &Ownership, dry_run: bool) -> Result<Outcome> {
    let tag = if dry_run { "(dry-run) " } else { "" };
    let at = claude_settings(home);
    let mut map = read_settings(&at)?;
    let line = match current(&map) {
        Some(v) if own.recorded.is_some() && v.as_str() == own.recorded.as_deref() => {
            set_var(&mut map, None);
            if !dry_run {
                write_settings(&at, &map)?;
            }
            format!("{tag}removed       claude {SETTING}")
        }
        Some(_) => format!("{tag}kept          claude {SETTING} ({OTHER_VALUE})"),
        None => format!("{tag}done          claude {SETTING} (not set)"),
    };
    Ok(Outcome { line, item: None })
}

/// Remove the memory dir an older mAId set, when it is still exactly the
/// value mAId recorded; anything else is left. `None` when nothing to say.
pub fn remove_legacy(home: &Path, recorded: Option<&str>, dry_run: bool) -> Result<Option<String>> {
    let Some(recorded) = recorded else {
        return Ok(None);
    };
    let tag = if dry_run { "(dry-run) " } else { "" };
    let at = claude_settings(home);
    let mut map = read_settings(&at)?;
    Ok(Some(
        match map.get(LEGACY_SETTING).and_then(Value::as_str) {
            Some(v) if v == recorded => {
                map.remove(LEGACY_SETTING);
                if !dry_run {
                    write_settings(&at, &map)?;
                }
                format!("{tag}removed       claude {LEGACY_SETTING}")
            }
            Some(_) => format!("{tag}kept          claude {LEGACY_SETTING} ({OTHER_VALUE})"),
            None => format!("{tag}done          claude {LEGACY_SETTING} (not set)"),
        },
    ))
}

// ─────────────────────────────────────────────────────────────────
// Status.
// ─────────────────────────────────────────────────────────────────

pub fn status(
    home: &Path,
    store: Option<&Path>,
    launcher: &Path,
    own: &Ownership,
) -> Result<Vec<String>> {
    let mut lines = vec![match store {
        None => format!("memory   {NO_STORE} (just memory-store <path>)"),
        Some(s) => format!(
            "memory   store {}{}",
            s.display(),
            if s.is_dir() { "" } else { " (MISSING)" }
        ),
    }];
    lines.push(match registered(home)? {
        None => format!("memory   claude mcp {SERVER} not registered"),
        Some(e) => {
            let whose = if is_ours(&e, launcher) {
                "mAId's"
            } else {
                OTHER_VALUE
            };
            let run: Vec<&str> = std::iter::once(&e["command"])
                .chain(e["args"].as_array().into_iter().flatten())
                .filter_map(Value::as_str)
                .collect();
            format!(
                "memory   claude mcp {SERVER} runs {} ({whose})",
                run.join(" ")
            )
        }
    });
    let map = read_settings(&claude_settings(home))?;
    lines.push(match current(&map) {
        None => format!("memory   claude {SETTING} not set (Claude's automatic memory is on)"),
        Some(v) => {
            let shown = v.as_str().map_or_else(|| v.to_string(), str::to_string);
            let whose = match own.recorded.as_deref() == v.as_str() && own.recorded.is_some() {
                true => "mAId's",
                false => OTHER_VALUE,
            };
            format!("memory   claude {SETTING} = {shown} ({whose})")
        }
    });
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn home() -> (TempDir, PathBuf) {
        let tmp = TempDir::new().unwrap();
        let home = fs::canonicalize(tmp.path()).unwrap();
        fs::create_dir_all(home.join(".claude")).unwrap();
        (tmp, home)
    }

    fn seed(home: &Path, text: &str) {
        fs::write(claude_settings(home), text).unwrap();
    }

    fn read(home: &Path) -> Value {
        serde_json::from_str(&fs::read_to_string(claude_settings(home)).unwrap()).unwrap()
    }

    fn own(recorded: Option<&str>, claimed: &[&str]) -> Ownership {
        Ownership {
            recorded: recorded.map(Into::into),
            adopted: false,
            claimed: claimed.iter().map(|c| c.to_string()).collect(),
        }
    }

    fn why(out: &Outcome) -> Option<&str> {
        out.item.as_ref().and_then(|i| i.skipped.as_deref())
    }

    // ── store ───────────────────────────────────────────────────

    #[test]
    fn no_store_file_or_an_empty_one_means_no_store() {
        let (tmp, _home) = home();
        let file = tmp.path().join("memory-store");
        assert_eq!(store(&file).unwrap(), None);
        fs::write(&file, "  \n").unwrap();
        assert_eq!(store(&file).unwrap(), None);
    }

    #[test]
    fn set_store_refuses_a_missing_dir_and_writes_an_existing_one() {
        let (tmp, home) = home();
        let file = tmp.path().join("cfg/memory-store");
        assert!(set_store(&file, &home.join("absent")).is_err());
        assert!(!file.exists());
        set_store(&file, &home).unwrap();
        assert_eq!(store(&file).unwrap(), Some(home.clone()));
    }

    // ── automatic memory off ────────────────────────────────────

    #[test]
    fn off_is_set_keeping_other_keys_and_env_vars() {
        let (_t, home) = home();
        seed(&home, r#"{"theme":"dark","env":{"FOO":"x"}}"#);
        let out = install_off(&home, &own(None, &[]), true, true, false).unwrap();
        let s = read(&home);
        assert_eq!(s["env"], serde_json::json!({"FOO": "x", VAR: "1"}));
        assert_eq!(s["theme"], "dark");
        let item = out.item.unwrap();
        assert_eq!((item.value.as_deref(), item.adopted), (Some("1"), false));
    }

    #[test]
    fn an_identical_value_is_adopted_unless_another_part_records_it() {
        let (_t, home) = home();
        seed(&home, r#"{"env":{"CLAUDE_CODE_DISABLE_AUTO_MEMORY":"1"}}"#);
        let out = install_off(&home, &own(None, &[]), true, true, false).unwrap();
        assert!(out.item.unwrap().adopted);
        let out = install_off(&home, &own(None, &["1"]), true, true, false).unwrap();
        assert_eq!(why(&out), Some(ANOTHER_PART));
    }

    #[test]
    fn a_value_another_part_records_is_never_written() {
        let (_t, home) = home();
        seed(&home, "{}");
        let out = install_off(&home, &own(None, &["1"]), true, true, false).unwrap();
        assert_eq!(why(&out), Some(ANOTHER_PART));
        assert_eq!(read(&home), serde_json::json!({}));
    }

    #[test]
    fn another_value_or_a_non_object_env_is_left_alone() {
        let (_t, home) = home();
        seed(&home, r#"{"env":{"CLAUDE_CODE_DISABLE_AUTO_MEMORY":"0"}}"#);
        let out = install_off(&home, &own(None, &[]), true, true, false).unwrap();
        assert_eq!(why(&out), Some(OTHER_VALUE));
        assert_eq!(read(&home)["env"][VAR], "0");
        seed(&home, r#"{"env":"x"}"#);
        let out = install_off(&home, &own(None, &[]), true, true, false).unwrap();
        assert_eq!(why(&out), Some(ENV_NOT_OBJECT));
        assert_eq!(read(&home)["env"], "x");
    }

    #[test]
    fn without_claude_or_on_a_dry_run_nothing_is_written() {
        let (_t, home) = home();
        seed(&home, "{}");
        let out = install_off(&home, &own(None, &[]), true, false, false).unwrap();
        assert_eq!(why(&out), Some(NO_CLI));
        install_off(&home, &own(None, &[]), true, true, true).unwrap();
        assert_eq!(fs::read_to_string(claude_settings(&home)).unwrap(), "{}");
    }

    #[test]
    fn without_maids_server_the_setting_is_left_or_given_back() {
        let (_t, home) = home();
        seed(&home, r#"{"env":{"CLAUDE_CODE_DISABLE_AUTO_MEMORY":"1"}}"#);
        let out = install_off(&home, &own(None, &[]), false, true, false).unwrap();
        assert_eq!(why(&out), Some(NO_SERVER));
        assert_eq!(read(&home)["env"][VAR], "1");
        let out = install_off(&home, &own(Some("1"), &[]), false, true, false).unwrap();
        assert_eq!(why(&out), Some(NO_SERVER));
        assert_eq!(read(&home), serde_json::json!({}));
        seed(&home, "{}");
        install_off(&home, &own(None, &[]), false, true, false).unwrap();
        assert_eq!(read(&home), serde_json::json!({}));
    }

    #[test]
    fn uninstall_removes_only_maids_value_and_an_env_it_leaves_empty() {
        let (_t, home) = home();
        seed(
            &home,
            r#"{"theme":"dark","env":{"CLAUDE_CODE_DISABLE_AUTO_MEMORY":"1"}}"#,
        );
        uninstall_off(&home, &own(None, &[]), false).unwrap();
        assert_eq!(read(&home)["env"][VAR], "1");
        uninstall_off(&home, &own(Some("1"), &[]), false).unwrap();
        assert_eq!(read(&home), serde_json::json!({"theme": "dark"}));
    }

    // ── the older memory dir ────────────────────────────────────

    #[test]
    fn the_old_memory_dir_goes_only_while_it_is_the_recorded_value() {
        let (_t, home) = home();
        seed(&home, r#"{"autoMemoryDirectory":"/s/memory"}"#);
        assert_eq!(remove_legacy(&home, None, false).unwrap(), None);
        remove_legacy(&home, Some("/other"), false).unwrap();
        assert_eq!(read(&home)["autoMemoryDirectory"], "/s/memory");
        remove_legacy(&home, Some("/s/memory"), false).unwrap();
        assert_eq!(read(&home), serde_json::json!({}));
    }

    // ── the server ──────────────────────────────────────────────

    #[test]
    fn the_entry_is_maids_only_while_it_runs_the_profiles_pensieve() {
        let me = Path::new("/p/bin/pensieve");
        assert!(is_ours(
            &serde_json::json!({"command": "/p/bin/pensieve"}),
            me
        ));
        assert!(!is_ours(&serde_json::json!({"command": "pensieve"}), me));
        assert!(!is_ours(&serde_json::json!({}), me));
    }

    #[test]
    fn a_non_object_server_list_stops_the_step() {
        let (_t, home) = home();
        fs::write(claude_json(&home), r#"{"mcpServers":[]}"#).unwrap();
        assert!(registered(&home).is_err());
        fs::write(claude_json(&home), r#"{"mcpServers":null}"#).unwrap();
        assert_eq!(registered(&home).unwrap(), None);
    }
}
