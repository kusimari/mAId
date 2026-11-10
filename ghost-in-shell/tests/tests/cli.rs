//! The binary, as a hook or a person calls it: a bare environment holding
//! only HOME, whose Claude Code has mAId's part and a second part installed.

use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

/// A plugin at `<home>/<name>` of the repo `repo`, with a skill `notes`
/// when given and the declaration of a memory server for `persona`.
fn plugin(home: &Path, name: &str, repo: &str, persona: &str, notes: bool) -> std::path::PathBuf {
    let at = home.join(name);
    fs::create_dir_all(at.join(".claude-plugin")).unwrap();
    fs::write(
        at.join(".claude-plugin/plugin.json"),
        json!({"name": name, "repository": repo}).to_string(),
    )
    .unwrap();
    if notes {
        fs::create_dir_all(at.join("skills/notes")).unwrap();
        fs::write(
            at.join("skills/notes/SKILL.md"),
            "---\nname: notes\ndescription: capture notes\nmetadata:\n  my-harness.contract: \"1\"\n  my-harness.scope: turn\n  my-harness.job: capture a note\n---\n",
        )
        .unwrap();
    }
    let decl = json!({"contract": 1, "scope": "session", "job": format!("memory for {persona}"),
        "persona": persona,
        "config": {"store": {"about": "the store", "bind": "arg:--store"}}, "state": {"permanent": "store", "promote": "off"}});
    let server = at.join(format!("servers/{persona}-memory/server.json"));
    fs::create_dir_all(server.parent().unwrap()).unwrap();
    fs::write(
        &server,
        json!({"name": format!("{persona}-memory"), "_meta": {"my-harness": decl}}).to_string(),
    )
    .unwrap();
    at
}

/// A HOME whose Claude Code has mAId's part (its notes and memory) and a
/// second part with its own memory.
fn setup() -> TempDir {
    let home = TempDir::new().unwrap();
    let claude = ghost_in_shell_tests::ClaudeHome::at(home.path());
    let h = home.path();
    claude.plugin(
        "maid@maid",
        &plugin(h, "maid", "https://example.org/me/mAId", "personal", true),
    );
    claude.plugin(
        "work@work",
        &plugin(h, "work", "https://example.org/me/work-part", "work", false),
    );
    for (server, store) in [("personal-memory", "store"), ("work-memory", "work-store")] {
        fs::create_dir_all(h.join(store)).unwrap();
        let store = h.join(store).display().to_string();
        claude.mcp(server, "/bin/pensieve", &["--store", &store]);
    }
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
        "personas: personal (mAId), work (work-part)",
        "  work-memory: memory for work [work, own]; state: ",
        "  notes: capture a note [personal, own]",
        "environment: 3 of 3 bring-ins ready",
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
    let map: Value = serde_json::from_str(&text(home.path(), &["ghost"])).unwrap();
    assert_eq!(map["bring_ins"][0]["route"], "worktree");
    assert_eq!(map["bring_ins"][0]["declared"]["is"], "declared");
    let env = text(home.path(), &["environment"]);
    assert!(env.contains("ready       notes (mAId)"), "{env}");
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

/// A hook still answers when the session's working dir is gone: only the
/// install verbs that read a project ask for it.
#[test]
fn the_genome_answers_from_a_working_dir_that_is_gone() {
    let home = TempDir::new().unwrap();
    let gone = home.path().join("gone");
    fs::create_dir_all(&gone).unwrap();
    let out = Command::new("/bin/sh")
        .arg("-c")
        .arg("cd \"$1\" && rmdir \"$1\" && exec \"$2\" genome")
        .arg("sh")
        .arg(&gone)
        .arg(ghost_in_shell_tests::core())
        .env_clear()
        .env("HOME", home.path())
        .output()
        .unwrap();
    assert!(
        out.status.success() && String::from_utf8_lossy(&out.stdout).contains("personas:"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
