//! Install's claude memory step under a temp HOME, with a stub `claude`
//! that keeps `~/.claude.json` and logs each call. One test, run in
//! order, because the step finds claude through the process-wide PATH.

use build_tool::shared::Agent;
use build_tool::stages::{
    cmd_install_bring_ins, cmd_install_memory, cmd_uninstall_memory, Checkout,
};
use build_tool::whole;
use ghost::parts::{Source, SourceKind};
use shell_claude::config_dir::ConfigDir;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use tempfile::TempDir;

/// `mcp add -s user <name> -- <cmd> <args>...` writes one server;
/// `mcp remove` empties the list. A `fail-<verb>` file beside HOME fails it.
const STUB: &str = r#"#!/bin/sh
echo "$*" >> "$HOME/../calls"
if [ -e "$HOME/../fail-$2" ]; then echo "error: boom" >&2; exit 1; fi
case "$2" in
  add)
    name=$5; shift 6
    printf '{"mcpServers":{"%s":{"type":"stdio","command":"%s","args":["%s","%s"]}}}\n' "$name" "$1" "$2" "$3" > "$HOME/.claude.json" ;;
  remove)
    printf '{"mcpServers":{}}\n' > "$HOME/.claude.json" ;;
esac
"#;

struct Env {
    _tmp: TempDir,
    root: PathBuf,
}

impl Env {
    /// A temp root with HOME, a parts dir, a profile, the store and a stub
    /// claude first on PATH (or no claude at all).
    fn new(with_claude: bool) -> Env {
        let tmp = TempDir::new().unwrap();
        let root = fs::canonicalize(tmp.path()).unwrap();
        for d in [
            "home/.claude",
            "state",
            "profile/bin",
            "bin",
            "stores/personal",
        ] {
            fs::create_dir_all(root.join(d)).unwrap();
        }
        if with_claude {
            fs::write(root.join("bin/claude"), STUB).unwrap();
            fs::set_permissions(root.join("bin/claude"), fs::Permissions::from_mode(0o755))
                .unwrap();
        }
        std::env::set_var("PATH", root.join("bin"));
        fs::write(
            root.join("store-file"),
            format!("{}\n", root.join("stores/personal").display()),
        )
        .unwrap();
        Env { _tmp: tmp, root }
    }
    fn at(&self, p: &str) -> PathBuf {
        self.root.join(p)
    }
    /// Claude's config under HOME, the variable unset.
    fn cc(&self) -> ConfigDir {
        ConfigDir::new(&self.at("home"), None)
    }
    fn install(&self) -> u8 {
        cmd_install_memory(
            &self.cc(),
            &self.at("store-file"),
            &self.at("profile"),
            &self.at("state"),
            false,
            None,
        )
        .unwrap()
    }
    fn uninstall(&self) {
        cmd_uninstall_memory(
            &self.cc(),
            &self.at("profile"),
            &self.at("state"),
            false,
            None,
        )
        .unwrap();
    }
    fn calls(&self) -> String {
        fs::read_to_string(self.at("calls")).unwrap_or_default()
    }
    fn launcher(&self) -> String {
        self.at("profile/bin/pensieve").display().to_string()
    }
    fn json(&self, p: &str) -> serde_json::Value {
        serde_json::from_str(&fs::read_to_string(self.at(p)).unwrap()).unwrap()
    }
    fn list(&self) -> String {
        fs::read_to_string(self.at("state/maid-harness.json")).unwrap_or_default()
    }
    fn whole(&self) -> String {
        whole::check(
            &self.at("state"),
            &self.at("home"),
            &self.cc(),
            Some(self.at("bin").into_os_string()),
            Some(Agent::Claude),
        )
        .unwrap()
        .to_string()
    }
}

fn settings(e: &Env, text: &str) {
    fs::write(e.at("home/.claude/settings.json"), text).unwrap();
}

#[test]
fn claude_memory_step() {
    registers_turns_memory_off_and_whole_status_sees_both();
    the_memory_bring_in_is_declared_and_ready_by_its_store();
    a_moved_store_is_registered_again();
    a_missing_store_is_skipped_never_created_and_maids_entry_goes();
    no_store_set_registers_nothing_and_leaves_claudes_memory_on();
    another_entry_or_claim_is_left_alone();
    without_claude_nothing_is_written();
    an_unreadable_claude_json_fails_install();
    a_failed_add_fails_install_and_is_recorded();
    a_failed_remove_keeps_the_entry_recorded_and_fails_the_run();
    the_old_memory_dir_goes_only_when_it_is_maids();
}

fn registers_turns_memory_off_and_whole_status_sees_both() {
    let e = Env::new(true);
    let before = "{\n  \"theme\": \"dark\"\n}\n";
    settings(&e, before);
    assert_eq!(e.install(), 0);

    let store = e.at("stores/personal").display().to_string();
    assert_eq!(
        e.calls(),
        format!(
            "mcp add -s user personal-memory -- {} --store {store}\n",
            e.launcher()
        )
    );
    let s = e.json("home/.claude/settings.json");
    assert_eq!(s["env"]["CLAUDE_CODE_DISABLE_AUTO_MEMORY"], "1");
    assert_eq!(s["theme"], "dark");
    let list = e.list();
    assert!(list.contains("\"name\": \"personal-memory\""), "{list}");
    assert!(
        list.contains("\"name\": \"env.CLAUDE_CODE_DISABLE_AUTO_MEMORY\"")
            && list.contains("\"value\": \"1\""),
        "{list}"
    );
    let report = e.whole();
    assert!(
        report.contains("present        mcp personal-memory"),
        "{report}"
    );
    assert!(
        report.contains("present        setting env.CLAUDE_CODE_DISABLE_AUTO_MEMORY"),
        "{report}"
    );

    // A second install changes nothing.
    let calls = e.calls();
    assert_eq!(e.install(), 0);
    assert_eq!(e.calls(), calls);

    // Nothing is written into the store.
    assert_eq!(fs::read_dir(e.at("stores/personal")).unwrap().count(), 0);

    e.uninstall();
    assert!(e.calls().ends_with("mcp remove -s user personal-memory\n"));
    assert_eq!(
        fs::read_to_string(e.at("home/.claude/settings.json")).unwrap(),
        before
    );
    assert!(!e.at("state/maid-harness.json").exists());
}

fn the_memory_bring_in_is_declared_and_ready_by_its_store() {
    let e = Env::new(true);
    assert_eq!(e.install(), 0);
    // The profile as the flake builds it: the skills dir and the shipped
    // server manifest.
    fs::create_dir_all(e.at("profile/share/maid/skills")).unwrap();
    let unit = e.at("profile/share/maid/servers/personal-memory/server.json");
    fs::create_dir_all(unit.parent().unwrap()).unwrap();
    let shipped = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../content/servers/personal-memory/server.json");
    fs::copy(shipped, &unit).unwrap();
    let pensieve = Source {
        kind: SourceKind::Project,
        at: Some("https://github.com/me/pensieve".into()),
        rev: Some("p1".into()),
    };
    let checkout = Checkout {
        pensieve: Some(pensieve.clone()),
        ..Checkout::default()
    };
    let rc = cmd_install_bring_ins(
        &e.at("state"),
        &e.at("profile"),
        &e.at("config"),
        checkout,
        &e.at("store-file"),
        &[],
        false,
    )
    .unwrap();
    assert_eq!(rc, 0);
    let list = e.json("state/maid-harness.json");
    let memory = list["bring_ins"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["name"] == "personal-memory")
        .unwrap();
    assert_eq!(memory["unit"]["path"], unit.display().to_string());
    assert_eq!(memory["source"], serde_json::to_value(&pensieve).unwrap());
    assert_eq!(memory["values"]["index"], "memory/MEMORY.md");
    let report = e.whole();
    assert!(
        report.contains("    ready          personal-memory\n"),
        "{report}"
    );

    fs::remove_dir(e.at("stores/personal")).unwrap();
    let report = e.whole();
    let store = e.at("stores/personal").display().to_string();
    assert!(
        report.contains(&format!(
            "    not ready      personal-memory (store at {store} not there)"
        )),
        "{report}"
    );
}

fn a_moved_store_is_registered_again() {
    let e = Env::new(true);
    e.install();
    fs::create_dir_all(e.at("stores/other")).unwrap();
    fs::write(
        e.at("store-file"),
        e.at("stores/other").display().to_string(),
    )
    .unwrap();
    e.install();
    let calls = e.calls();
    let lines: Vec<&str> = calls.lines().collect();
    assert_eq!(lines.len(), 3, "{calls}");
    assert_eq!(lines[1], "mcp remove -s user personal-memory");
    assert!(lines[2].ends_with(&format!("--store {}", e.at("stores/other").display())));
}

fn a_missing_store_is_skipped_never_created_and_maids_entry_goes() {
    let e = Env::new(true);
    e.install();
    fs::remove_dir(e.at("stores/personal")).unwrap();
    assert_eq!(e.install(), 0);
    assert!(e.calls().ends_with("mcp remove -s user personal-memory\n"));
    assert!(!e.at("stores/personal").exists());
    // mAId's server went, so mAId's setting goes back too.
    let s = e.json("home/.claude/settings.json");
    assert!(s.get("env").is_none(), "{s}");
    let list = e.list();
    assert!(
        list.contains("\"skipped\": \"personal store missing\""),
        "{list}"
    );
    assert!(e.whole().contains("skipped        mcp personal-memory"));
}

fn no_store_set_registers_nothing_and_leaves_claudes_memory_on() {
    let e = Env::new(true);
    fs::remove_file(e.at("store-file")).unwrap();
    settings(&e, "{}\n");
    assert_eq!(e.install(), 0);
    assert_eq!(e.calls(), "");
    assert_eq!(
        fs::read_to_string(e.at("home/.claude/settings.json")).unwrap(),
        "{}\n"
    );
    let list = e.list();
    assert!(
        list.contains("\"skipped\": \"no personal store set\""),
        "{list}"
    );
    assert!(
        list.contains("\"skipped\": \"no personal-memory server\""),
        "{list}"
    );
}

fn another_entry_or_claim_is_left_alone() {
    let e = Env::new(true);
    let theirs =
        r#"{"mcpServers":{"personal-memory":{"command":"/elsewhere/pensieve","args":[]}}}"#;
    fs::write(e.at("home/.claude.json"), theirs).unwrap();
    e.install();
    assert_eq!(e.calls(), "");
    assert!(e.list().contains("\"skipped\": \"not mAId's\""));
    e.uninstall();
    assert_eq!(
        fs::read_to_string(e.at("home/.claude.json")).unwrap(),
        theirs
    );

    // Another part's list records the server and the setting: neither taken.
    let e = Env::new(true);
    settings(
        &e,
        "{\"env\": {\"CLAUDE_CODE_DISABLE_AUTO_MEMORY\": \"1\"}}\n",
    );
    fs::write(
        e.at("state/other.json"),
        serde_json::json!({"format": 1, "part": "other", "harnesses": {"claude": [
            {"kind": "mcp", "name": "personal-memory"},
            {"kind": "setting", "name": "env.CLAUDE_CODE_DISABLE_AUTO_MEMORY", "value": "1"}]}})
        .to_string(),
    )
    .unwrap();
    e.install();
    assert_eq!(e.calls(), "");
    let list = e.list();
    // The server is theirs, so mAId has none and leaves their setting alone.
    assert!(list.contains("\"skipped\": \"another part's\""), "{list}");
    assert!(
        list.contains("\"skipped\": \"no personal-memory server\""),
        "{list}"
    );
    assert_eq!(
        e.json("home/.claude/settings.json")["env"]["CLAUDE_CODE_DISABLE_AUTO_MEMORY"],
        "1"
    );

    // mAId's own entry, then claimed by another part: install and uninstall
    // agree it is theirs, so neither removes it.
    let e = Env::new(true);
    e.install();
    fs::write(
        e.at("state/other.json"),
        serde_json::json!({"format": 1, "part": "other", "harnesses": {"claude": [
            {"kind": "mcp", "name": "personal-memory"}]}})
        .to_string(),
    )
    .unwrap();
    e.install();
    e.uninstall();
    assert!(!e.calls().contains("remove"), "{}", e.calls());
}

fn without_claude_nothing_is_written() {
    let e = Env::new(false);
    settings(&e, "{}\n");
    assert_eq!(e.install(), 0);
    assert_eq!(
        fs::read_to_string(e.at("home/.claude/settings.json")).unwrap(),
        "{}\n"
    );
    assert!(!e.at("home/.claude.json").exists());
    assert_eq!(
        e.list()
            .matches("\"skipped\": \"claude not on PATH\"")
            .count(),
        2
    );
}

fn an_unreadable_claude_json_fails_install() {
    let e = Env::new(true);
    fs::write(e.at("home/.claude.json"), "not json").unwrap();
    let err = cmd_install_memory(
        &e.cc(),
        &e.at("store-file"),
        &e.at("profile"),
        &e.at("state"),
        false,
        None,
    )
    .unwrap_err();
    assert!(format!("{err:#}").contains(".claude.json"), "{err:#}");
    assert_eq!(e.calls(), "");
}

fn a_failed_add_fails_install_and_is_recorded() {
    let e = Env::new(true);
    fs::write(e.at("fail-add"), "").unwrap();
    assert_eq!(e.install(), 1);
    assert!(e.list().contains("\"skipped\": \"mcp add failed\""));
}

fn a_failed_remove_keeps_the_entry_recorded_and_fails_the_run() {
    let e = Env::new(true);
    e.install();
    fs::write(e.at("fail-remove"), "").unwrap();
    let rc = cmd_uninstall_memory(&e.cc(), &e.at("profile"), &e.at("state"), false, None).unwrap();
    assert_eq!(rc, 1);
    assert!(e.list().contains("\"name\": \"personal-memory\""));
    // A store move that cannot remove the old entry keeps it, and says so.
    fs::create_dir_all(e.at("stores/other")).unwrap();
    fs::write(
        e.at("store-file"),
        e.at("stores/other").display().to_string(),
    )
    .unwrap();
    assert_eq!(e.install(), 1);
    let list = e.list();
    assert!(
        list.contains("\"name\": \"personal-memory\"") && !list.contains("skipped\": \"mcp"),
        "{list}"
    );
}

fn the_old_memory_dir_goes_only_when_it_is_maids() {
    let e = Env::new(true);
    let old = e.at("stores/personal/memory").display().to_string();
    settings(
        &e,
        &serde_json::json!({"autoMemoryDirectory": old}).to_string(),
    );
    fs::write(
        e.at("state/maid-harness.json"),
        serde_json::json!({"format": 1, "part": "maid-harness", "harnesses": {"claude": [
            {"kind": "setting", "name": "autoMemoryDirectory", "value": old}]}})
        .to_string(),
    )
    .unwrap();
    e.install();
    let s = e.json("home/.claude/settings.json");
    assert!(s.get("autoMemoryDirectory").is_none(), "{s}");
    assert!(!e.list().contains("autoMemoryDirectory"));

    // One an older install recorded as skipped goes from the list too.
    let e = Env::new(true);
    fs::write(
        e.at("state/maid-harness.json"),
        serde_json::json!({"format": 1, "part": "maid-harness", "harnesses": {"claude": [
            {"kind": "setting", "name": "autoMemoryDirectory",
             "skipped": "mAId names no default memory store"}]}})
        .to_string(),
    )
    .unwrap();
    e.install();
    assert!(!e.list().contains("autoMemoryDirectory"));
    e.uninstall();
    assert!(!e.at("state/maid-harness.json").exists());

    // A hand-set one stays.
    let e = Env::new(true);
    settings(&e, "{\"autoMemoryDirectory\": \"/mine\"}\n");
    e.install();
    assert_eq!(
        e.json("home/.claude/settings.json")["autoMemoryDirectory"],
        "/mine"
    );
}
