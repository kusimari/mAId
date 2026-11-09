//! Claude Code itself, with mAId's Claude adapter, against a stub of the
//! model API: no model and no network. The stub listens on 127.0.0.1 at a
//! port the OS picks, records every request body and answers from a
//! script, so what Claude Code places in a session is read as data.
//!
//! Ignored by default: it needs `claude` (`CLAUDE_BIN`, else on PATH). Run
//! `cargo test -p ghost-in-shell-tests --test claude_live -- --ignored` after any
//! change to the adapter, the hooks, the plugin manifest or Claude Code.
//! Claude Code runs in a temp HOME, so nothing real is read or written.

use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

use ghost_in_shell_tests::core;

type Log = Arc<Mutex<Vec<Value>>>;

/// The stub model API. Its answers: a sub-agent call for a prompt saying
/// SPAWN, a call of the expression tool for CALLTOOL, a read of the genome
/// resource for READRES, a summary for a compaction, else a line of text.
fn stub() -> (String, Log) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let log: Log = Arc::default();
    let l = log.clone();
    std::thread::spawn(move || {
        for conn in listener.incoming().flatten() {
            let l = l.clone();
            std::thread::spawn(move || serve(conn, &l));
        }
    });
    (url, log)
}

fn serve(conn: TcpStream, log: &Log) {
    let mut out = conn.try_clone().unwrap();
    let mut r = BufReader::new(conn);
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
        let mut len = 0;
        loop {
            let mut h = String::new();
            r.read_line(&mut h).unwrap();
            if h.trim().is_empty() {
                break;
            }
            if let Some((k, v)) = h.split_once(':') {
                if k.eq_ignore_ascii_case("content-length") {
                    len = v.trim().parse().unwrap_or(0);
                }
            }
        }
        let mut body = vec![0; len];
        r.read_exact(&mut body).unwrap();
        let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
        let (ctype, data) = answer(&path, &body);
        log.lock()
            .unwrap()
            .push(json!({"path": path, "body": body}));
        let head = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: {ctype}\r\ncontent-length: {}\r\n\r\n",
            data.len()
        );
        if out
            .write_all(head.as_bytes())
            .and_then(|_| out.write_all(&data))
            .is_err()
        {
            return;
        }
    }
}

fn answer(path: &str, body: &Value) -> (&'static str, Vec<u8>) {
    if path.contains("count_tokens") {
        return (
            "application/json",
            json!({"input_tokens": 100}).to_string().into_bytes(),
        );
    }
    let last = last_user(body);
    let blocks = if last.contains("Your task is to create a detailed summary") {
        vec![json!({"type": "text", "text": "<summary>stub summary</summary>"})]
    } else if last.contains("SPAWN") && !last.contains("tool_result") {
        let agent = ["Agent", "Task"]
            .into_iter()
            .find(|n| {
                body["tools"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|t| t["name"] == *n)
            })
            .unwrap_or("Agent");
        vec![json!({"type": "tool_use", "id": "toolu_1", "name": agent,
            "input": {"description": "probe", "prompt": "SUBTASK say hi", "subagent_type": "general-purpose"}})]
    } else if last.contains("READRES") && !last.contains("tool_result") {
        vec![
            json!({"type": "tool_use", "id": "toolu_3", "name": "ReadMcpResourceTool",
            "input": {"server": "plugin:maid:my-harness", "uri": "my-harness://genome"}}),
        ]
    } else if last.contains("CALLTOOL") && !last.contains("tool_result") {
        vec![
            json!({"type": "tool_use", "id": "toolu_2", "name": "mcp__plugin_maid_my-harness__expression",
            "input": {"prompt": "deploy the release"}}),
        ]
    } else {
        vec![json!({"type": "text", "text": "stub reply"})]
    };
    let stop = match blocks[0]["type"] == "tool_use" {
        true => "tool_use",
        false => "end_turn",
    };
    if body["stream"] != true {
        let msg = json!({"id": "msg_stub", "type": "message", "role": "assistant", "model": "stub",
            "content": blocks, "stop_reason": stop, "stop_sequence": null,
            "usage": {"input_tokens": 10, "output_tokens": 5}});
        return ("application/json", msg.to_string().into_bytes());
    }
    let mut s = String::new();
    let mut ev = |t: &str, d: Value| s.push_str(&format!("event: {t}\ndata: {d}\n\n"));
    ev(
        "message_start",
        json!({"type": "message_start", "message": {"id": "msg_stub", "type": "message",
        "role": "assistant", "model": "stub", "content": [], "stop_reason": null, "stop_sequence": null,
        "usage": {"input_tokens": 10, "output_tokens": 1}}}),
    );
    for (i, b) in blocks.iter().enumerate() {
        let (start, delta) = match b["type"] == "text" {
            true => (
                json!({"type": "text", "text": ""}),
                json!({"type": "text_delta", "text": b["text"]}),
            ),
            false => (
                json!({"type": "tool_use", "id": b["id"], "name": b["name"], "input": {}}),
                json!({"type": "input_json_delta", "partial_json": b["input"].to_string()}),
            ),
        };
        ev(
            "content_block_start",
            json!({"type": "content_block_start", "index": i, "content_block": start}),
        );
        ev(
            "content_block_delta",
            json!({"type": "content_block_delta", "index": i, "delta": delta}),
        );
        ev(
            "content_block_stop",
            json!({"type": "content_block_stop", "index": i}),
        );
    }
    ev(
        "message_delta",
        json!({"type": "message_delta", "delta": {"stop_reason": stop, "stop_sequence": null},
        "usage": {"output_tokens": 5}}),
    );
    ev("message_stop", json!({"type": "message_stop"}));
    ("text/event-stream", s.into_bytes())
}

/// The last user message's text, tool results included as JSON.
fn last_user(body: &Value) -> String {
    let Some(m) = body["messages"]
        .as_array()
        .and_then(|ms| ms.iter().rev().find(|m| m["role"] == "user"))
    else {
        return String::new();
    };
    match &m["content"] {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .map(|p| match p["type"] == "text" {
                true => p["text"].as_str().unwrap_or_default().to_string(),
                false => p.to_string(),
            })
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

/// A temp HOME with mAId's part (a turn skill that fits "deploy", a memory
/// whose index holds a deploy memory) and a fixture second part; a plugin
/// dir built from the repo's Claude adapter files as the flake builds it,
/// with the cargo-built core; and a work folder.
struct World {
    dir: TempDir,
}

impl World {
    fn new() -> World {
        World::build(false)
    }

    /// The same, with the second part's persona acting in the work folder:
    /// its turn skill names the folder as a location.
    fn acting() -> World {
        World::build(true)
    }

    fn build(acting: bool) -> World {
        let dir = TempDir::new().unwrap();
        let d = dir.path();
        let parts = d.join("home/.local/state/my-harness/parts");
        fs::create_dir_all(&parts).unwrap();
        fs::create_dir_all(d.join("work")).unwrap();
        let unit = d.join("units/deployer/SKILL.md");
        fs::create_dir_all(unit.parent().unwrap()).unwrap();
        fs::write(&unit, "---\nname: deployer\ndescription: deploy things\nmetadata:\n  my-harness.scope: turn\n  my-harness.job: deploy a release\n  my-harness.signals.jobs: deploy\n  my-harness.rules: SKILL.md#rules\n---\n## Rules\n\nTAG FIRST, THEN DEPLOY\n").unwrap();
        let store = d.join("store");
        fs::create_dir_all(store.join("memory")).unwrap();
        fs::write(
            store.join("memory/MEMORY.md"),
            "- [Deploy steps](deploy.md) - how a release is deployed\n",
        )
        .unwrap();
        let server = ghost_in_shell_tests::content().join("servers/personal-memory/server.json");
        let mine = json!({"format": 1, "part": "maid-harness", "persona": "personal", "harnesses": {},
            "bring_ins": [
                {"name": "deployer", "unit": {"standard": "skill", "path": unit}},
                {"name": "personal-memory", "unit": {"standard": "mcp", "path": server.canonicalize().unwrap()},
                    "values": {"store": store, "index": "memory/MEMORY.md"}}]});
        let mut theirs = json!({"format": 1, "part": "work-part", "persona": "work", "harnesses": {}, "bring_ins": []});
        if acting {
            let planner = d.join("units/planner/SKILL.md");
            fs::create_dir_all(planner.parent().unwrap()).unwrap();
            fs::write(&planner, "---\nname: planner\ndescription: plan work\nmetadata:\n  my-harness.scope: turn\n  my-harness.job: plan the work\n  my-harness.signals.locations: work\n---\n").unwrap();
            theirs["bring_ins"] =
                json!([{"name": "planner", "unit": {"standard": "skill", "path": planner}}]);
        }
        fs::write(parts.join("maid-harness.json"), mine.to_string()).unwrap();
        fs::write(parts.join("work-part.json"), theirs.to_string()).unwrap();
        let adapter = ghost_in_shell_tests::plugin();
        let plugin = d.join("plugin");
        fs::create_dir_all(plugin.join(".claude-plugin")).unwrap();
        fs::create_dir_all(plugin.join("hooks")).unwrap();
        let decl: Value =
            serde_json::from_str(&fs::read_to_string(adapter.join("my-harness.json")).unwrap())
                .unwrap();
        let manifest = json!({"name": "maid", "version": "1.0.0", "description": "mAId's skills",
            "author": {"name": "mAId"}, "extensions": {"my-harness": decl},
            "hooks": "./hooks/claude.json", "mcpServers": "./claude-mcp.json"});
        fs::write(
            plugin.join(".claude-plugin/plugin.json"),
            manifest.to_string(),
        )
        .unwrap();
        for (from, to) in [
            ("hooks.json", "hooks/claude.json"),
            ("mcp.json", "claude-mcp.json"),
        ] {
            let text = fs::read_to_string(adapter.join(from))
                .unwrap()
                .replace("@maid-core@", core());
            fs::write(plugin.join(to), text).unwrap();
        }
        World { dir }
    }

    /// One `claude -p` session fed `prompts` as stream-json, against `url`.
    fn session(&self, url: &str, prompts: &[&str]) -> String {
        let d = self.dir.path();
        let claude = std::env::var("CLAUDE_BIN").unwrap_or_else(|_| "claude".into());
        let mut child = Command::new(claude)
            .args([
                "-p",
                "--input-format",
                "stream-json",
                "--output-format",
                "stream-json",
                "--verbose",
            ])
            .args(["--allowedTools", "mcp__plugin_maid_my-harness__expression"])
            .arg("--plugin-dir")
            .arg(d.join("plugin"))
            .current_dir(d.join("work"))
            .env_clear()
            .env("HOME", d.join("home"))
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("ANTHROPIC_BASE_URL", url)
            .env("ANTHROPIC_API_KEY", "sk-stub-not-a-key")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("claude on PATH, or CLAUDE_BIN");
        let mut stdin = child.stdin.take().unwrap();
        for p in prompts {
            let m = json!({"type": "user", "message": {"role": "user", "content": p}});
            writeln!(stdin, "{m}").unwrap();
        }
        drop(stdin);
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "claude: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }
}

/// The main loop's requests, in order: those that carry the session's
/// prompts, not a sub-agent's or a side request's.
fn requests(log: &Log, has: &str) -> Vec<String> {
    log.lock()
        .unwrap()
        .iter()
        .filter(|r| {
            r["path"]
                .as_str()
                .unwrap_or("")
                .starts_with("/v1/messages?")
                || r["path"] == "/v1/messages"
        })
        .map(|r| r["body"]["messages"].to_string())
        .filter(|m| m.contains(has))
        .collect()
}

fn count(text: &str, what: &str) -> usize {
    text.matches(what).count()
}

const GENOME: &str = "personas: personal (maid-harness), work (work-part)";

#[test]
#[ignore = "needs claude; run with --ignored"]
fn a_session_starts_with_the_genome_and_each_turn_gets_only_what_changed() {
    let (url, log) = stub();
    let world = World::new();
    let out = world.session(
        &url,
        &[
            "deploy the release",
            "deploy the release again",
            "SPAWN a helper",
        ],
    );
    assert!(
        out.contains(r#"{"name":"plugin:maid:my-harness","status":"connected""#),
        "the plugin, `extensions` and all, loads and its MCP server connects:\n{out}"
    );

    let main = requests(&log, "deploy the release");
    let first = main.iter().find(|m| !m.contains("again")).unwrap();
    assert_eq!(
        count(first, GENOME),
        1,
        "Macro step 1: the genome, once:\n{first}"
    );
    assert_eq!(count(first, "pending: 0 decisions"), 1);
    assert_eq!(
        count(first, "TAG FIRST, THEN DEPLOY"),
        1,
        "Macro step 2: its first rules"
    );
    assert_eq!(count(first, "[Deploy steps]("), 1, "and a memory pointer");

    let second = main.iter().find(|m| m.contains("again")).unwrap();
    assert_eq!(count(second, GENOME), 1, "the genome is not placed again");
    assert_eq!(
        count(second, "TAG FIRST, THEN DEPLOY"),
        1,
        "a second prompt gets nothing new"
    );
    assert_eq!(count(second, "[Deploy steps]("), 1);
    assert_eq!(count(second, "[my-harness]"), count(first, "[my-harness]"));

    let sub = requests(&log, "SUBTASK");
    let sub = sub
        .iter()
        .find(|m| !m.contains("SPAWN"))
        .expect("a sub-agent request");
    assert_eq!(
        count(sub, "personas: personal"),
        1,
        "a sub-agent gets its slice:\n{sub}"
    );
    assert_eq!(
        count(sub, "pending:"),
        0,
        "without what waits for the person"
    );
}

#[test]
#[ignore = "needs claude; run with --ignored"]
fn with_a_persona_acting_a_second_prompt_with_nothing_changed_gets_nothing() {
    let (url, log) = stub();
    World::acting().session(&url, &["plan the beds", "plan the beds once more"]);
    let main = requests(&log, "plan the beds");
    let first = main.iter().find(|m| !m.contains("once more")).unwrap();
    assert_eq!(
        count(first, "acting through work"),
        1,
        "the acting persona, once:\n{first}"
    );
    let second = main.iter().find(|m| m.contains("once more")).unwrap();
    assert_eq!(
        count(second, "[my-harness]"),
        count(first, "[my-harness]"),
        "a second prompt with nothing changed gets nothing new:\n{second}"
    );
    assert_eq!(
        count(second, "none here"),
        0,
        "never told a record, never told there is none:\n{second}"
    );
}

#[test]
#[ignore = "needs claude; run with --ignored"]
fn the_on_demand_tool_answers_in_a_session() {
    let (url, log) = stub();
    World::new().session(&url, &["CALLTOOL"]);
    let called = requests(&log, "tool_result");
    assert!(
        called
            .iter()
            .any(|m| m.contains("fits this turn: deployer") && m.contains("TAG FIRST")),
        "the expression tool answers on demand: {called:?}"
    );
}

#[test]
#[ignore = "needs claude; run with --ignored"]
fn a_compaction_places_the_genome_again_once() {
    let (url, log) = stub();
    World::new().session(
        &url,
        &[
            "deploy the release",
            "/compact",
            "deploy the release after compact",
        ],
    );
    let after = requests(&log, "after compact");
    let last = after.last().expect("a request after the compaction");
    assert!(last.contains("stub summary"), "the compaction ran:\n{last}");
    assert_eq!(count(last, GENOME), 1, "the genome again, once:\n{last}");
    assert_eq!(
        count(last, "TAG FIRST, THEN DEPLOY"),
        1,
        "the reset clears the turn's record too, so a fit is sent again, once"
    );
}

/// Every string in the requests' messages: what a hook placed, a tool's
/// result, a prompt.
fn texts(log: &Log) -> Vec<String> {
    fn walk(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::String(s) => out.push(s.clone()),
            Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            Value::Object(o) => o.values().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let mut out = vec![];
    for r in log.lock().unwrap().iter() {
        walk(&r["body"]["messages"], &mut out);
    }
    out
}

#[test]
#[ignore = "needs claude; run with --ignored"]
fn the_on_demand_resource_reads_the_genome_the_session_started_with() {
    let (url, log) = stub();
    World::new().session(&url, &["READRES"]);
    let all = texts(&log);
    let placed = all
        .iter()
        .find_map(|t| t.strip_prefix("SessionStart hook additional context: "))
        .expect("the genome placed at start")
        .to_string();
    // Claude Code hands the tool's result back as the resource's JSON.
    let read = all
        .iter()
        .filter_map(|t| serde_json::from_str::<Value>(t).ok())
        .find_map(|v| v["contents"][0]["text"].as_str().map(str::to_string))
        .expect("the resource's text in a tool result");
    // The hook's context may be followed by Claude Code's own text.
    assert!(
        placed.starts_with(read.trim_end()),
        "the resource returns what the session started with:\nplaced:\n{placed}\nread:\n{read}"
    );
}

#[test]
#[ignore = "needs claude; run with --ignored"]
fn a_session_that_ends_is_marked_for_sleep() {
    let (url, _log) = stub();
    let world = World::new();
    world.session(&url, &["deploy the release"]);
    let sleep = world.dir.path().join("home/.local/state/my-harness/sleep");
    let marks: Vec<Value> = fs::read_dir(&sleep)
        .unwrap_or_else(|e| panic!("no marks at {}: {e}", sleep.display()))
        .map(|e| serde_json::from_str(&fs::read_to_string(e.unwrap().path()).unwrap()).unwrap())
        .collect();
    assert_eq!(marks.len(), 1, "{marks:?}");
    assert_eq!(marks[0]["reasons"], json!(["end"]));
    assert!(marks[0]["transcript"].as_str().unwrap().ends_with(".jsonl"));
}
