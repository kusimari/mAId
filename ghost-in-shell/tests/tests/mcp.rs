//! `maid-core mcp` as a shell runs it: JSON-RPC lines on stdio, a bare
//! environment, a temp HOME, the session id in a variable it is told of.

use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use tempfile::TempDir;

use ghost_in_shell_tests::{core, ClaudeHome};

/// A HOME whose Claude Code has a skill put in by hand that declares itself.
fn home() -> TempDir {
    let home = TempDir::new().unwrap();
    ClaudeHome::at(home.path()).skill("deployer", "---\nname: deployer\ndescription: d\nmetadata:\n  my-harness.contract: \"1\"\n  my-harness.repository: https://example.org/me/kit\n  my-harness.scope: turn\n  my-harness.job: deploys\n  my-harness.signals.jobs: deploy\n  my-harness.rules: SKILL.md#rules\n---\n## Rules\n\nTAG FIRST\n");
    home
}

/// Every reply to `messages`, in order.
fn talk(home: &Path, messages: &[Value]) -> Vec<Value> {
    let mut child = Command::new(core())
        .args(["mcp", "--session-env", "SID"])
        .env_clear()
        .env("HOME", home)
        .env("SID", "s1")
        .current_dir(home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for m in messages {
        writeln!(stdin, "{m}").unwrap();
    }
    writeln!(stdin, "{{not json").unwrap();
    drop(stdin);
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn req(id: u64, method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
}

#[test]
fn the_server_answers_the_genome_and_expression_on_demand() {
    let home = home();
    let replies = talk(
        home.path(),
        &[
            req(
                1,
                "initialize",
                json!({"protocolVersion": "2025-06-18", "capabilities": {}}),
            ),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            req(2, "resources/list", json!({})),
            req(3, "resources/read", json!({"uri": "my-harness://genome"})),
            req(4, "prompts/get", json!({"name": "genome"})),
            req(5, "tools/list", json!({})),
            req(
                6,
                "tools/call",
                json!({"name": "expression", "arguments": {"prompt": "deploy it"}}),
            ),
            req(
                7,
                "tools/call",
                json!({"name": "expression", "arguments": {"prompt": "deploy it"}}),
            ),
            req(8, "no/such", json!({})),
        ],
    );
    assert_eq!(
        replies.len(),
        9,
        "one per request, none for the notification: {replies:?}"
    );
    assert_eq!(replies[0]["result"]["serverInfo"]["name"], "my-harness");
    assert_eq!(replies[0]["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(
        replies[1]["result"]["resources"][0]["uri"],
        "my-harness://genome"
    );
    let resource = replies[2]["result"]["contents"][0]["text"]
        .as_str()
        .unwrap();
    let cli = Command::new(core())
        .arg("genome")
        .env_clear()
        .env("HOME", home.path())
        .output()
        .unwrap();
    assert_eq!(
        resource,
        String::from_utf8(cli.stdout).unwrap(),
        "the same genome as the CLI's"
    );
    assert_eq!(
        replies[3]["result"]["messages"][0]["content"]["text"],
        resource
    );
    assert_eq!(replies[4]["result"]["tools"][0]["name"], "expression");
    for r in [&replies[5], &replies[6]] {
        let text = r["result"]["content"][0]["text"].as_str().unwrap();
        assert!(
            text.contains("fits this turn: deployer") && text.contains("TAG FIRST"),
            "on demand answers every time: {text}"
        );
    }
    assert_eq!(replies[7]["error"]["code"], -32601);
    assert_eq!(replies[8]["error"]["code"], -32700);
    assert!(
        home.path()
            .join(".local/state/my-harness/delivery/s1")
            .is_dir(),
        "on demand records under the session it was told of"
    );
}

#[test]
fn the_tool_answers_only_for_folders_under_its_own() {
    let home = home();
    let outside = TempDir::new().unwrap();
    fs::write(
        outside.path().join("notes.md"),
        "# Elsewhere\n\n## Handoff\n\n- **Stage:** SECRET\n",
    )
    .unwrap();
    ClaudeHome::at(home.path()).skill("kit", "---\nname: kit\ndescription: d\nmetadata:\n  my-harness.contract: \"1\"\n  my-harness.repository: https://example.org/me/kit\n  my-harness.scope: turn\n  my-harness.job: k\n  my-harness.record: notes.md#handoff\n---\n");
    std::os::unix::fs::symlink(outside.path(), home.path().join("link")).unwrap();
    let up = format!(
        "{}/../{}",
        home.path().display(),
        outside
            .path()
            .strip_prefix(home.path().parent().unwrap())
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    );
    let tries = [
        outside.path().display().to_string(),
        home.path().join("link").display().to_string(),
        up,
    ];
    let calls: Vec<Value> = tries
        .iter()
        .enumerate()
        .map(|(i, cwd)| {
            req(
                i as u64 + 1,
                "tools/call",
                json!({"name": "expression", "arguments": {"prompt": "x", "cwd": cwd}}),
            )
        })
        .collect();
    for r in &talk(home.path(), &calls)[..3] {
        let text = r["result"]["content"][0]["text"].as_str().unwrap();
        assert!(
            !text.contains("SECRET"),
            "answered for a folder outside its own: {text}"
        );
    }
}
