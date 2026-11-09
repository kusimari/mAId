//! The binary, as a hook or a person calls it: a bare environment holding
//! only HOME, a temp state dir with mAId's list and a fixture second part.

use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

fn setup() -> TempDir {
    let home = TempDir::new().unwrap();
    let parts = home.path().join(".local/state/my-harness/parts");
    fs::create_dir_all(&parts).unwrap();
    let skill = home.path().join("notes/SKILL.md");
    fs::create_dir_all(skill.parent().unwrap()).unwrap();
    fs::write(
        &skill,
        "---\nname: notes\ndescription: capture notes\nmetadata:\n  my-harness.scope: turn\n  my-harness.job: capture a note\n---\n",
    )
    .unwrap();
    let store = home.path().join("work-store");
    fs::create_dir_all(&store).unwrap();
    let server = home.path().join("server.json");
    let decl = json!({"scope": "session", "job": "memory for work",
        "config": {"store": {"about": "the store"}}, "state": {"local": "store"}});
    fs::write(
        &server,
        json!({"name": "work-memory", "_meta": {"my-harness": decl}}).to_string(),
    )
    .unwrap();
    let mine = json!({"format": 1, "part": "maid-harness", "persona": "personal",
        "harnesses": {"claude": [{"kind": "plugin", "name": "maid@maid"}]},
        "bring_ins": [{"name": "notes", "unit": {"standard": "skill", "path": skill},
            "source": {"kind": "project", "rev": "abc"}}]});
    let theirs = json!({"format": 1, "part": "work-part", "harnesses": {}, "persona": "work",
        "bring_ins": [{"name": "work-memory", "unit": {"standard": "mcp", "path": server},
            "values": {"store": store}}]});
    fs::write(parts.join("maid-harness.json"), mine.to_string()).unwrap();
    fs::write(parts.join("work-part.json"), theirs.to_string()).unwrap();
    home
}

fn core(home: &Path, args: &[&str]) -> Output {
    let out = Command::new(ghost_in_shell_tests::core())
        .args(args)
        .env_clear()
        .env("HOME", home)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn text(home: &Path, args: &[&str]) -> String {
    String::from_utf8(core(home, args).stdout).unwrap()
}

#[test]
fn genome_on_demand_shows_both_parts_with_no_checkout_and_a_bare_environment() {
    let home = setup();
    let out = text(home.path(), &["genome"]);
    for want in [
        "personas: personal (maid-harness), work (work-part)",
        "  work-memory: memory for work [work, own]; state: ",
        "  notes: capture a note [personal, own]",
        "environment: 2 of 2 bring-ins ready",
        "pending: 0 decisions",
    ] {
        assert!(out.contains(want), "{want:?} in:\n{out}");
    }
    assert_eq!(
        text(home.path(), &["genome"]),
        out,
        "on demand answers every time"
    );
}

#[test]
fn json_carries_the_same_picture() {
    let home = setup();
    let v: Value = serde_json::from_str(&text(home.path(), &["genome", "--json"])).unwrap();
    let item = &v["items"][0];
    assert_eq!(item["id"], "genome");
    assert_eq!(item["text"], text(home.path(), &["genome"]));
    assert_eq!(item["data"]["personas"][1]["persona"], "work");
    assert_eq!(item["data"]["environment"][0]["readiness"]["is"], "ready");
}

#[test]
fn an_append_carrier_gets_the_picture_once_per_session_until_reset() {
    let home = setup();
    let append = |session: &str, reset: bool| {
        let mut args = vec!["genome", "--carrier", "append", "--session", session];
        if reset {
            args.push("--reset");
        }
        text(home.path(), &args)
    };
    assert!(append("s1", false).starts_with("personas:"));
    assert_eq!(append("s1", false), "", "nothing changed, nothing sent");
    assert!(
        append("s1", true).starts_with("personas:"),
        "--reset sends it again"
    );
    assert_eq!(append("s1", false), "");
    assert!(
        append("s1.agent-1", false).starts_with("personas:"),
        "a sub-session gets its own picture"
    );
    assert_eq!(append("s1.agent-1", false), "");
}

#[test]
fn the_unfilled_entry_points_succeed_with_nothing() {
    let home = setup();
    let e = "expression";
    assert_eq!(
        text(
            home.path(),
            &[e, "--carrier", "append", "--session", "s", "--input", "{}"]
        ),
        ""
    );
    assert_eq!(text(home.path(), &[e, "--json"]).trim(), r#"{"items":[]}"#);
}

#[test]
fn map_and_environment_print_their_views() {
    let home = setup();
    let map: Value = serde_json::from_str(&text(home.path(), &["map"])).unwrap();
    assert_eq!(map["bring_ins"][0]["route"], "worktree");
    assert_eq!(map["bring_ins"][0]["declared"]["is"], "declared");
    let env = text(home.path(), &["environment"]);
    assert!(env.contains("ready       notes (maid-harness)"), "{env}");
}

#[test]
fn a_bad_session_id_fails_and_writes_nothing_outside_the_record() {
    let home = setup();
    let out = Command::new(ghost_in_shell_tests::core())
        .args(["genome", "--carrier", "append", "--session", "../escape"])
        .env_clear()
        .env("HOME", home.path())
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(!home.path().join(".local/state/my-harness/escape").exists());
}

#[test]
fn input_that_is_not_json_fails() {
    let home = setup();
    let out = Command::new(ghost_in_shell_tests::core())
        .args(["expression", "--input", "{not json"])
        .env_clear()
        .env("HOME", home.path())
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("not JSON"));
}
