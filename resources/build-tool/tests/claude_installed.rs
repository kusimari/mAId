//! Claude Code on mAId's plugin as install places it: profile generations
//! outside the state dir, as nix keeps them, the plugin installed by
//! `Plugins::install` with the real claude CLI, then a `claude -p` session
//! against a loopback stub of the model API. No model and no network; a
//! temp HOME, so nothing real is read or written.
//!
//! Ignored by default: it needs `claude` (`CLAUDE_BIN`, else on PATH) and
//! builds `maid-core`. Run after any change to where install puts the
//! plugin, or to Claude Code:
//!
//!   cargo test -p build-tool --test claude_installed -- --ignored

use build_tool::deploy::{Deploy, Plugins};
use build_tool::shared::Agent;
use serde_json::{json, Value};
use shell_claude::config_dir::ConfigDir;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

type Log = Arc<Mutex<Vec<Value>>>;

// The stub after ghost-in-shell/tests/tests/claude_live.rs, answering text only.
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
        return ("application/json", br#"{"input_tokens": 100}"#.to_vec());
    }
    if body["stream"] != true {
        let msg = json!({"id": "msg_stub", "type": "message", "role": "assistant", "model": "stub",
            "content": [{"type": "text", "text": "stub reply"}], "stop_reason": "end_turn",
            "stop_sequence": null, "usage": {"input_tokens": 10, "output_tokens": 5}});
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
    ev(
        "content_block_start",
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}),
    );
    ev(
        "content_block_delta",
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "stub reply"}}),
    );
    ev(
        "content_block_stop",
        json!({"type": "content_block_stop", "index": 0}),
    );
    ev(
        "message_delta",
        json!({"type": "message_delta", "delta": {"stop_reason": "end_turn", "stop_sequence": null},
        "usage": {"output_tokens": 5}}),
    );
    ev("message_stop", json!({"type": "message_stop"}));
    ("text/event-stream", s.into_bytes())
}

/// The `maid-core` this checkout builds, beside build-tool in the target dir.
/// It builds the dev profile, so the test runs under the dev profile too.
fn core() -> PathBuf {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let built = Command::new(cargo)
        .args([
            "build",
            "--quiet",
            "-p",
            "shell-claude",
            "--bin",
            "maid-core",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .status()
        .unwrap();
    assert!(built.success(), "cargo build -p shell-claude");
    let core = Path::new(env!("CARGO_BIN_EXE_build-tool")).with_file_name("maid-core");
    assert!(core.is_file(), "no {}", core.display());
    core
}

/// A profile generation laid out as flake.nix builds it, from the repo's
/// Claude adapter files, and the declaration of a memory for the persona
/// `tag`, so a session shows which generation Claude Code has by the
/// persona its genome names.
fn generation(dir: &Path, tag: &str, core: &Path) {
    let m = dir.join("share/maid/marketplace");
    let p = m.join("plugins/maid");
    let put = |at: &Path, text: &str| {
        fs::create_dir_all(at.parent().unwrap()).unwrap();
        fs::write(at, text).unwrap();
    };
    put(
        &m.join(".claude-plugin/marketplace.json"),
        r#"{"name":"maid","owner":{"name":"mAId"},"plugins":[{"name":"maid","source":"./plugins/maid"}]}"#,
    );
    put(
        &m.join(".agents/plugins/marketplace.json"),
        r#"{"name":"maid","plugins":[{"name":"maid","source":{"source":"local","path":"./plugins/maid"},"policy":{"installation":"AVAILABLE"}}]}"#,
    );
    let adapter = build_tool::shared::repo_root()
        .unwrap()
        .join(build_tool::shared::GHOST_PLUGIN_DIR);
    let decl: Value =
        serde_json::from_str(&fs::read_to_string(adapter.join("my-harness.json")).unwrap())
            .unwrap();
    let version = format!("1.0.0-{tag}");
    let manifest = json!({"name": "maid", "version": version, "description": "mAId's skills",
        "author": {"name": "mAId"}, "repository": "https://example.org/me/maid",
        "extensions": {"my-harness": decl},
        "hooks": "./hooks/claude.json", "mcpServers": "./claude-mcp.json"});
    put(&p.join(".claude-plugin/plugin.json"), &manifest.to_string());
    put(
        &p.join(".codex-plugin/plugin.json"),
        &json!({"name": "maid", "version": version}).to_string(),
    );
    let hook_core = core.display().to_string();
    put(
        &p.join("hooks/claude.json"),
        &fs::read_to_string(adapter.join("hooks.json"))
            .unwrap()
            .replace("@maid-core@", &hook_core),
    );
    put(
        &p.join("claude-mcp.json"),
        &fs::read_to_string(adapter.join("mcp.json"))
            .unwrap()
            .replace("@maid-core@", &core.display().to_string()),
    );
    let memory = json!({"name": "mem", "_meta": {"my-harness": {"contract": 1, "scope": "session",
        "job": "memory", "persona": tag, "config": {"store": {"about": "s", "bind": "arg:--store"}},
        "state": {"permanent": "store", "promote": "off"}}}});
    put(&p.join("servers/mem/server.json"), &memory.to_string());
}

/// What `just rollback` and an install do to the profile: move its link.
fn switch(profile: &Path, to: &Path) {
    let _ = fs::remove_file(profile);
    std::os::unix::fs::symlink(to, profile).unwrap();
}

/// One `claude -p` session in `home`, with no `--plugin-dir`: the plugin is
/// only what install registered. Returns its stdout.
fn session(claude: &Path, home: &Path, work: &Path, url: &str) -> String {
    let mut child = Command::new(claude)
        .args([
            "-p",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            "--verbose",
        ])
        .current_dir(work)
        .env_clear()
        .env("HOME", home)
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("ANTHROPIC_BASE_URL", url)
        .env("ANTHROPIC_API_KEY", "sk-stub-not-a-key")
        .env("DISABLE_AUTOUPDATER", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("claude");
    let m = json!({"type": "user", "message": {"role": "user", "content": "hello there"}});
    writeln!(child.stdin.take().unwrap(), "{m}").unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "claude: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

/// The messages of the first main-loop request of the session.
fn first_prompt(log: &Log) -> String {
    let log = log.lock().unwrap();
    log.iter()
        .filter(|r| r["path"].as_str().unwrap_or("").starts_with("/v1/messages"))
        .map(|r| r["body"]["messages"].to_string())
        .find(|m| m.contains("hello there"))
        .unwrap_or_default()
}

#[test]
#[ignore = "needs claude and builds maid-core; run with --ignored"]
fn claude_code_runs_the_installed_plugins_hooks_and_follows_a_rollback() {
    let claude = std::env::var("CLAUDE_BIN").unwrap_or_else(|_| "claude".into());
    let claude = build_tool::shared::find_on(std::env::var_os("PATH").as_deref(), &claude)
        .unwrap_or_else(|| PathBuf::from(&claude));
    assert!(claude.is_file(), "claude on PATH, or CLAUDE_BIN");
    let core = core();
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    // install runs `claude` from PATH: this one, first.
    let bin = d.join("bin");
    fs::create_dir_all(&bin).unwrap();
    std::os::unix::fs::symlink(&claude, bin.join("claude")).unwrap();
    let path = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("{}:{path}", bin.display()));
    // install's CLI calls set only HOME: this would point them at real config.
    std::env::remove_var("CLAUDE_CONFIG_DIR");

    let home = d.join("home");
    let work = d.join("work");
    fs::create_dir_all(&work).unwrap();
    // Generations outside the state dir, as the nix store holds them.
    let (a, b) = (d.join("store/gen-a"), d.join("store/gen-b"));
    generation(&a, "alpha", &core);
    generation(&b, "beta", &core);
    let state = home.join(".local/state/maid");
    fs::create_dir_all(&state).unwrap();
    let profile = state.join("profile");
    switch(&profile, &a);
    let p = Plugins {
        home: home.clone(),
        claude: ConfigDir::new(&home, None),
        profile: profile.clone(),
    };
    let run = |args: &[&str]| {
        let out = Command::new(&claude)
            .args(args)
            .env("HOME", &home)
            .output()
            .unwrap();
        assert!(out.status.success(), "claude {args:?}: {out:?}");
    };
    // An older mAId's install: claude on a dir of links into the profile.
    let links = state.join("marketplace");
    fs::create_dir_all(&links).unwrap();
    for entry in [".claude-plugin", ".agents", "plugins"] {
        std::os::unix::fs::symlink(
            profile.join("share/maid/marketplace").join(entry),
            links.join(entry),
        )
        .unwrap();
    }
    run(&["mcp", "add", "-s", "user", "mem", "--", "/bin/true"]);
    run(&["plugin", "marketplace", "add", links.to_str().unwrap()]);
    run(&["plugin", "install", "maid@maid", "--scope", "user"]);

    // This install, on the same generation: the marketplace only moves.
    p.install(Some(Agent::Claude), false, false).unwrap();
    let (url, log) = stub();
    let out = session(&claude, &home, &work, &url);
    assert!(
        out.contains(r#"{"name":"plugin:maid:my-harness","status":"connected""#),
        "the plugin's MCP server connects:\n{out}"
    );
    let first = first_prompt(&log);
    assert_eq!(
        first.matches("personas: alpha (maid)").count(),
        1,
        "the start hook placed the genome once:\n{first}"
    );

    switch(&profile, &b);
    p.install(Some(Agent::Claude), false, false).unwrap();
    let maid = || {
        let v: Value = serde_json::from_str(
            &fs::read_to_string(home.join(".claude/plugins/known_marketplaces.json")).unwrap(),
        )
        .unwrap();
        (
            v["maid"]["source"].clone(),
            v["maid"]["installLocation"].clone(),
        )
    };
    let registered = maid();
    assert_eq!(
        registered.1,
        json!(profile.join("share/maid/marketplace")),
        "claude is on the profile's own marketplace"
    );
    let (url, log) = stub();
    session(&claude, &home, &work, &url);
    let first = first_prompt(&log);
    assert_eq!(
        first.matches("personas: beta (maid)").count(),
        1,
        "the new generation's hooks run:\n{first}"
    );

    // Roll back: the link moves and nothing is registered again.
    switch(&profile, &a);
    let (url, log) = stub();
    session(&claude, &home, &work, &url);
    let first = first_prompt(&log);
    assert_eq!(
        first.matches("personas: alpha (maid)").count(),
        1,
        "after the link moves back, the previous generation's hooks run:\n{first}"
    );
    assert!(!first.contains("personas: beta"), "{first}");
    assert_eq!(maid(), registered, "no marketplace registered again");
}
