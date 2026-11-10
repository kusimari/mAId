//! Claude Code on mAId's two plugins as their installs place them: plain's
//! package staged by `plain::stage` and placed by plain's `Plugins`, the
//! ghost's staged by `shell_claude::package` and placed by the ghost's own
//! install step, each in a profile whose generations sit outside the state
//! dir as nix keeps them; then `claude -p` sessions against a loopback stub
//! of the model API. No model and no network; a temp HOME, so nothing real
//! is read or written.
//!
//! Ignored by default: it needs `claude` (`CLAUDE_BIN`, else on PATH) and
//! builds `ghost-claude`. Run after any change to where either install puts
//! its plugin, or to Claude Code:
//!
//!   cargo test -p ghost-in-shell-tests --test claude_installed -- --ignored

use ghost_in_shell_tests::{content, core, plugin};
use plain::claude_config::ConfigDir;
use plain::place::{Deploy, Plugins};
use plain::shared::Agent;
use serde_json::{json, Value};
use shell_claude::install::Setup;
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

/// A ghost generation at `dir`: the ghost's plugin, its adapter's job
/// naming `tag`, so a session shows which generation ran.
fn ghost_generation(d: &Path, dir: &Path, tag: &str) {
    let src = d.join(format!("plugin-{tag}"));
    plain::stage::copy_tree(&plugin(), &src).unwrap();
    let manifest = src.join(".claude-plugin/plugin.json");
    let mut m: Value = serde_json::from_str(&fs::read_to_string(&manifest).unwrap()).unwrap();
    m["extensions"]["my-harness"]["job"] = format!("the adapter, generation {tag}").into();
    fs::write(&manifest, m.to_string()).unwrap();
    shell_claude::package::stage(&src, Path::new(core()), dir, dir).unwrap();
}

/// Point a profile link at a generation, as nix and a rollback do.
fn switch(profile: &Path, to: &Path) {
    let _ = fs::remove_file(profile);
    std::os::unix::fs::symlink(to, profile).unwrap();
}

/// One `claude -p` session in `home`, with no `--plugin-dir`: the plugins
/// are only what the installs registered. Returns its stdout.
fn session(claude: &Path, home: &Path, work: &Path, url: &str, extra: &[&str]) -> String {
    let mut child = Command::new(claude)
        .args([
            "-p",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            "--verbose",
        ])
        .args(extra)
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

/// A session's first prompt, and its stdout.
fn run(claude: &Path, home: &Path, work: &Path, extra: &[&str]) -> (String, String) {
    let (url, log) = stub();
    let out = session(claude, home, work, &url, extra);
    (first_prompt(&log), out)
}

#[test]
#[ignore = "needs claude and builds ghost-claude; run with --ignored"]
fn claude_code_runs_plain_alone_then_the_ghost_on_off_and_across_generations() {
    let claude = std::env::var("CLAUDE_BIN").unwrap_or_else(|_| "claude".into());
    let claude = plain::shared::find_on(std::env::var_os("PATH").as_deref(), &claude)
        .unwrap_or_else(|| PathBuf::from(&claude));
    assert!(claude.is_file(), "claude on PATH, or CLAUDE_BIN");
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    // The installs run `claude` from PATH: this one, first.
    let bin = d.join("bin");
    fs::create_dir_all(&bin).unwrap();
    std::os::unix::fs::symlink(&claude, bin.join("claude")).unwrap();
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    std::env::set_var("PATH", &path);
    // The installs' CLI calls set only HOME: this would point them at real config.
    std::env::remove_var("CLAUDE_CONFIG_DIR");
    let home = d.join("home");
    let work = d.join("work");
    fs::create_dir_all(&work).unwrap();
    let state = home.join(".local/state/maid");
    fs::create_dir_all(&state).unwrap();
    let config = ConfigDir::new(&home, None);

    // Plain alone, staged by plain and placed by plain.
    let plain_gen = d.join("store/maid");
    plain::stage::plain(&content(), &plain_gen, &[]).unwrap();
    let plain_profile = state.join("profile");
    switch(&plain_profile, &plain_gen);
    Plugins {
        home: home.clone(),
        claude: config.clone(),
        profile: plain_profile,
    }
    .install(Some(Agent::Claude), false, false)
    .unwrap();
    let (first, out) = run(&claude, &home, &work, &[]);
    assert!(
        !first.contains("personas:"),
        "plain alone has no genome:\n{first}"
    );
    assert!(
        !out.contains("plugin:ghost"),
        "no ghost hook or server:\n{out}"
    );

    // The ghost over it, generation alpha, placed by its own install step.
    let (a, b) = (d.join("store/ghost-a"), d.join("store/ghost-b"));
    ghost_generation(d, &a, "alpha");
    ghost_generation(d, &b, "beta");
    let profile = state.join("ghost/profile");
    fs::create_dir_all(profile.parent().unwrap()).unwrap();
    switch(&profile, &a);
    let s = Setup {
        root: ghost_in_shell_tests::repo(),
        profile: profile.clone(),
        claude: config.clone(),
        records: d.join("records"),
        state: d.join("ghost-state"),
        exe: PathBuf::from(core()),
        path: Some(path.clone().into()),
    };
    shell_claude::install::place(&s).unwrap();
    let (first, out) = run(&claude, &home, &work, &[]);
    assert!(
        out.contains(r#"{"name":"plugin:ghost:my-harness","status":"connected""#),
        "the ghost's MCP server connects:\n{out}"
    );
    assert_eq!(
        first.matches("generation alpha").count(),
        1,
        "the start hook placed the genome once:\n{first}"
    );

    // Off for every session, on again; off for one launch.
    shell_claude::switch::switch(&s.cli(), false, "user", &work).unwrap();
    let (first, _) = run(&claude, &home, &work, &[]);
    assert!(
        !first.contains("generation alpha"),
        "off: no genome:\n{first}"
    );
    shell_claude::switch::switch(&s.cli(), true, "user", &work).unwrap();
    let off = r#"{"enabledPlugins":{"ghost@ghost-in-shell":false}}"#;
    let (first, _) = run(&claude, &home, &work, &["--settings", off]);
    assert!(
        !first.contains("generation alpha"),
        "off for one launch:\n{first}"
    );
    let (first, _) = run(&claude, &home, &work, &[]);
    assert!(first.contains("generation alpha"), "on again:\n{first}");

    // A new generation, placed; then back by the link alone.
    switch(&profile, &b);
    shell_claude::install::place(&s).unwrap();
    let market = || -> Value {
        let v: Value = serde_json::from_str(
            &fs::read_to_string(home.join(".claude/plugins/known_marketplaces.json")).unwrap(),
        )
        .unwrap();
        v["ghost-in-shell"].clone()
    };
    let registered = market();
    assert_eq!(
        registered["installLocation"],
        json!(profile.join("share/ghost/marketplace")),
        "claude is on the ghost profile's own marketplace"
    );
    let (first, _) = run(&claude, &home, &work, &[]);
    assert_eq!(first.matches("generation beta").count(), 1, "{first}");
    switch(&profile, &a);
    let (first, _) = run(&claude, &home, &work, &[]);
    assert_eq!(
        first.matches("generation alpha").count(),
        1,
        "after the link moves back, the previous generation's hooks run:\n{first}"
    );
    assert!(!first.contains("generation beta"), "{first}");
    assert_eq!(market(), registered, "no marketplace registered again");
}
