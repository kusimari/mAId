//! The shipped Claude Code hooks file, run as Claude Code runs it: each
//! command through `sh -c`, `@maid-core@` replaced with the built core, the
//! hook's JSON payload on stdin, in a bare environment with a temp HOME.

use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use tempfile::TempDir;

use ghost_in_shell_tests::core;
use ghost_in_shell_tests::{maid_plugin, ClaudeHome};

fn hooks() -> Value {
    let path = ghost_in_shell_tests::plugin().join("hooks.json");
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

/// The command for `event`, from the entry whose matcher names `source`
/// (or the one entry, for an event without matchers).
fn command(event: &str, source: Option<&str>) -> String {
    let entries = hooks()["hooks"][event].as_array().unwrap().clone();
    let entry = entries
        .iter()
        .find(|e| match (source, e["matcher"].as_str()) {
            (Some(s), Some(m)) => m.split('|').any(|x| x == s),
            (None, None) => true,
            _ => false,
        })
        .unwrap_or_else(|| panic!("no {event} entry for {source:?}"));
    let hook = &entry["hooks"][0];
    assert_eq!(hook["type"], "command");
    assert!(hook["timeout"].as_u64().is_some(), "{event} has a timeout");
    hook["command"].as_str().unwrap().to_string()
}

/// A HOME whose Claude Code has mAId's plugin and its memory installed.
fn home() -> TempDir {
    let home = TempDir::new().unwrap();
    let claude = ClaudeHome::at(home.path());
    claude.plugin("maid@maid", &maid_plugin(&home.path().join("plugin")));
    let store = home.path().join("store").display().to_string();
    claude.mcp("personal-memory", "/bin/pensieve", &["--store", &store]);
    home
}

fn run(home: &Path, command: &str, payload: &Value) -> Output {
    assert!(command.starts_with("@maid-core@ "), "{command}");
    let mut child = Command::new("/bin/sh")
        .args(["-c", &command.replacen("@maid-core@", core(), 1)])
        .env_clear()
        .env("HOME", home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload.to_string().as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

/// The hook's `additionalContext`, or `None` when it printed nothing.
fn context(out: &Output, event: &str) -> Option<String> {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout.clone()).unwrap();
    if stdout.trim().is_empty() {
        return None;
    }
    let v: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["hookSpecificOutput"]["hookEventName"], event);
    Some(
        v["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .to_string(),
    )
}

fn start(source: &str) -> Value {
    json!({"session_id": "s1", "transcript_path": "/t", "cwd": "/w",
        "hook_event_name": "SessionStart", "source": source})
}

#[test]
fn session_start_places_the_genome_once_and_again_after_a_drop() {
    let home = home();
    let h = home.path();
    let first = context(
        &run(
            h,
            &command("SessionStart", Some("startup")),
            &start("startup"),
        ),
        "SessionStart",
    );
    assert!(first.unwrap().starts_with("personas: personal"));
    assert_eq!(
        context(
            &run(
                h,
                &command("SessionStart", Some("startup")),
                &start("startup")
            ),
            "SessionStart"
        ),
        None,
        "the session id comes from the payload, so a second start sends nothing"
    );
    // A resume keeps its history and its session id, so the genome is
    // already there; a fork has a new id, so its own record sends it.
    let resumed = context(
        &run(
            h,
            &command("SessionStart", Some("resume")),
            &start("resume"),
        ),
        "SessionStart",
    );
    assert_eq!(resumed, None, "a resume does not place the genome again");
    for source in ["clear", "compact"] {
        let again = context(
            &run(h, &command("SessionStart", Some(source)), &start(source)),
            "SessionStart",
        );
        assert!(again.is_some(), "{source} places the genome again");
    }
    let mut fork = start("fork");
    fork["session_id"] = json!("s2");
    let forked = context(
        &run(h, &command("SessionStart", Some("fork")), &fork),
        "SessionStart",
    );
    assert!(forked.is_some(), "a fork is a new session");
}

#[test]
fn a_sub_agent_gets_its_own_genome() {
    let home = home();
    let h = home.path();
    run(
        h,
        &command("SessionStart", Some("startup")),
        &start("startup"),
    );
    let payload = json!({"session_id": "s1", "transcript_path": "/t", "cwd": "/w",
        "hook_event_name": "SubagentStart", "agent_id": "a1", "agent_type": "general-purpose"});
    let cmd = command("SubagentStart", None);
    let slice = context(&run(h, &cmd, &payload), "SubagentStart").unwrap();
    assert!(
        slice.starts_with("personas: personal") && !slice.contains("pending:"),
        "{slice}"
    );
    assert_eq!(context(&run(h, &cmd, &payload), "SubagentStart"), None);
}

#[test]
fn a_prompt_with_nothing_that_fits_gets_nothing() {
    let home = home();
    let payload = json!({"session_id": "s1", "transcript_path": "/t", "cwd": "/w",
        "hook_event_name": "UserPromptSubmit", "prompt": "hello"});
    let cmd = command("UserPromptSubmit", None);
    assert!(cmd.contains(" expression "), "{cmd}");
    for _ in 0..2 {
        assert_eq!(
            context(&run(home.path(), &cmd, &payload), "UserPromptSubmit"),
            None,
            "the prompt goes as it would without mAId"
        );
    }
}

#[test]
fn a_failing_hook_exits_0_with_nothing_on_stdout() {
    let home = home();
    let cmd = command("UserPromptSubmit", None);
    let out = run(
        home.path(),
        &cmd,
        &json!({"session_id": "../escape", "prompt": "x"}),
    );
    assert!(out.status.success());
    assert!(out.stdout.is_empty());
    let state = home.path().join(".local/state/my-harness");
    let _ = fs::remove_dir_all(&state);
    fs::create_dir_all(state.parent().unwrap()).unwrap();
    fs::write(&state, "not a dir").unwrap();
    let out = run(
        home.path(),
        &cmd,
        &json!({"session_id": "s9", "prompt": "x"}),
    );
    assert!(out.status.success(), "a state dir that cannot be written");
    assert!(out.stdout.is_empty());
    let renamed = cmd.replace("--carrier append", "--carrier-renamed append");
    let out = run(
        home.path(),
        &renamed,
        &json!({"session_id": "s9", "prompt": "x"}),
    );
    assert!(
        out.status.success(),
        "a flag the core does not know still never blocks"
    );
    assert!(out.stdout.is_empty());
    let out = run(
        home.path(),
        "@maid-core@ genome --carrier append --input -",
        &json!({"session_id": "../x"}),
    );
    assert!(
        !out.status.success(),
        "without --claude-event an error still fails"
    );
}

#[test]
fn the_shipped_mcp_config_starts_the_core_as_a_server_with_the_session() {
    let path = ghost_in_shell_tests::plugin().join("mcp.json");
    let v: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    let server = &v["mcpServers"]["my-harness"];
    assert_eq!(server["command"], "@maid-core@");
    let args: Vec<&str> = server["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap())
        .collect();
    let home = home();
    let mut child = Command::new(core())
        .args(&args)
        .env_clear()
        .env("HOME", home.path())
        .env("CLAUDE_CODE_SESSION_ID", "s1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let call = json!({"jsonrpc": "2.0", "id": 1, "method": "resources/read",
        "params": {"uri": "my-harness://genome"}});
    writeln!(child.stdin.take().unwrap(), "{call}").unwrap();
    let out = child.wait_with_output().unwrap();
    let reply: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(reply["result"]["contents"][0]["text"]
        .as_str()
        .unwrap()
        .starts_with("personas: personal"));
    assert!(
        home.path()
            .join(".local/state/my-harness/delivery/s1")
            .is_dir(),
        "the session comes from the variable Claude Code sets"
    );
}

#[test]
fn session_end_marks_the_session_for_sleep_and_says_nothing() {
    let home = home();
    let h = home.path();
    let t = h.join(".claude/projects/-w/s1.jsonl");
    fs::create_dir_all(t.parent().unwrap()).unwrap();
    fs::write(&t, "{\"type\":\"user\",\"message\":{\"content\":\"hi\"}}\n").unwrap();
    let end = json!({"session_id": "s1", "transcript_path": t, "cwd": "/w",
        "hook_event_name": "SessionEnd", "reason": "other"});
    let out = run(h, &command("SessionEnd", None), &end);
    assert_eq!(context(&out, "SessionEnd"), None);
    let mark: Value = serde_json::from_str(
        &fs::read_to_string(h.join(".local/state/my-harness/sleep/s1.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(mark["reasons"], json!(["end"]));
    let genome = context(
        &run(
            h,
            &command("SessionStart", Some("startup")),
            &start("startup"),
        ),
        "SessionStart",
    )
    .unwrap();
    assert!(genome.contains("to reflect on: 1 sessions"), "{genome}");
    // With no transcript to find, the hook fails quietly: an end event
    // must never hold up Claude Code.
    let lost = json!({"session_id": "s2", "transcript_path": "/nowhere/s2.jsonl",
        "hook_event_name": "SessionEnd"});
    let out = run(h, &command("SessionEnd", None), &lost);
    assert!(out.status.success() && out.stdout.is_empty());
}
