//! Writing a persona's memory through its own server: the core starts the
//! command the part binds as `serve`, speaks MCP over stdio (one JSON-RPC
//! message per line), calls the memory tool, and stops it. The server owns
//! where and how the store holds a memory, so the store is left exactly as
//! the server's own write leaves it.

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

/// How long a server gets to answer every call before it is stopped.
pub const TIMEOUT: Duration = Duration::from_secs(30);

/// The root path of the memory tool's commands (`memory_create`, ...),
/// which pensieve serves the store's memory dir under.
pub const ROOT: &str = "/memories";

/// The `serve` value: a JSON array of strings, the command and its
/// arguments.
pub fn argv(serve: &str) -> Result<Vec<String>> {
    let v: Vec<String> = serde_json::from_str(serve)
        .map_err(|_| anyhow!("serve is not a JSON array of strings: {serve}"))?;
    match v.first() {
        Some(cmd) if std::path::Path::new(cmd).is_absolute() => Ok(v),
        _ => bail!("serve must start with an absolute command: {serve}"),
    }
}

/// Call each tool in order, in one run of the server, waiting for each
/// answer before the next request and stopping at the first that fails:
/// a later call (an index line) must not run when the one before it (the
/// memory it points at) did not. Answers the text of each call made, the
/// last one an error when one failed.
///
/// `serve` is the command Claude Code registered for the memory server at
/// the user's scope, which the core runs as Claude Code would, with the
/// core's environment.
pub fn call(argv: &[String], calls: &[(&str, Value)]) -> Result<Vec<Result<String, String>>> {
    let (cmd, args) = argv
        .split_first()
        .ok_or_else(|| anyhow!("serve is empty"))?;
    let mut child = spawn(cmd, args).with_context(|| format!("cannot start {cmd}"))?;
    let mut stdin = child.stdin.take().expect("piped");
    let stdout = child.stdout.take().expect("piped");
    let mut stderr = child.stderr.take().expect("piped");
    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let errs = std::thread::spawn(move || {
        let mut out = String::new();
        let _ = stderr.read_to_string(&mut out);
        out
    });
    let deadline = Instant::now() + TIMEOUT;
    // One request, then lines until its answer; `None` when the server
    // ended or ran out of time first.
    let ask = |stdin: &mut std::process::ChildStdin, id: usize, msg: Value| -> Option<Value> {
        writeln!(stdin, "{msg}").ok()?;
        loop {
            let left = deadline.checked_duration_since(Instant::now())?;
            let line = match rx.recv_timeout(left) {
                Ok(l) => l,
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => return None,
            };
            let Ok(v) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if v["id"].as_u64() == Some(id as u64) {
                return Some(v);
            }
        }
    };
    let init = json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {
        "protocolVersion": "2025-06-18", "capabilities": {},
        "clientInfo": {"name": "ghost-claude", "version": env!("CARGO_PKG_VERSION")}}});
    let mut answers = vec![];
    let ready = ask(&mut stdin, 0, init).is_some_and(|v| v["error"].is_null());
    if ready {
        let _ = writeln!(
            stdin,
            "{}",
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
        );
        for (i, (name, arguments)) in calls.iter().enumerate() {
            let msg = json!({"jsonrpc": "2.0", "id": i + 1, "method": "tools/call",
                "params": {"name": name, "arguments": arguments}});
            let answer = match ask(&mut stdin, i + 1, msg) {
                None => Err("no answer".to_string()),
                Some(v) => {
                    let text = v["result"]["content"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|c| c["text"].as_str())
                        .collect::<Vec<_>>()
                        .join("\n");
                    match (&v["error"], v["result"]["isError"] == true) {
                        (Value::Null, false) => Ok(text),
                        (Value::Null, true) => Err(text),
                        (e, _) => Err(e["message"].as_str().unwrap_or("error").to_string()),
                    }
                }
            };
            let failed = answer.is_err();
            answers.push(answer);
            if failed {
                break;
            }
        }
    }
    // Closing stdin ends the server; it gets a moment to finish what it
    // does on the way out, and is stopped after that so a hung one never
    // holds the hook.
    drop(stdin);
    let stop = Instant::now() + Duration::from_secs(2);
    let status = loop {
        if let Some(st) = child.try_wait()? {
            break st;
        }
        if Instant::now() > stop {
            let _ = child.kill();
            break child.wait()?;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let err = errs.join().unwrap_or_default();
    if !ready {
        bail!("{cmd} did not start as a server ({status}{})", tail(&err));
    }
    if let Some(Err(e)) = answers.last_mut() {
        if e == "no answer" {
            *e = format!("no answer ({status}{})", tail(&err));
        }
    }
    Ok(answers)
}

/// Start the server. A command written a moment ago (a fresh install, a
/// test's stub) can be busy while another thread's fork still holds it
/// open for writing, so that is tried again, briefly.
fn spawn(cmd: &str, args: &[String]) -> std::io::Result<std::process::Child> {
    let mut tries = 0;
    loop {
        match Command::new(cmd)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy && tries < 50 => {
                tries += 1;
                std::thread::sleep(Duration::from_millis(20));
            }
            other => return other,
        }
    }
}

fn tail(err: &str) -> String {
    match err.trim().lines().last() {
        Some(l) => format!(": {l}"),
        None => String::new(),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use tempfile::TempDir;

    /// A stand-in memory server: logs each message it is sent to `log`, and
    /// answers every call ok, or as an error when its text holds `FAIL`.
    pub fn stub(dir: &Path) -> (String, std::path::PathBuf) {
        let log = dir.join("server.log");
        let bin = dir.join("stub-server");
        fs::write(
            &bin,
            format!(
                "#!/bin/sh\nwhile IFS= read -r line; do\n  printf '%s\\n' \"$line\" >> '{}'\n  id=$(printf '%s' \"$line\" | sed -n 's/^{{\"jsonrpc\":\"2.0\",\"id\":\\([0-9]*\\).*/\\1/p')\n  [ -z \"$id\" ] && continue\n  err=false; case \"$line\" in *FAIL*) err=true;; esac\n  printf '{{\"jsonrpc\":\"2.0\",\"id\":%s,\"result\":{{\"content\":[{{\"type\":\"text\",\"text\":\"done %s\"}}],\"isError\":%s}}}}\\n' \"$id\" \"$id\" \"$err\"\ndone\n",
                log.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
        (
            serde_json::to_string(&[bin.display().to_string()]).unwrap(),
            log,
        )
    }

    #[test]
    fn each_call_gets_its_own_answer_and_errors_stay_with_their_call() {
        let d = TempDir::new().unwrap();
        let (serve, log) = stub(d.path());
        let got = call(
            &argv(&serve).unwrap(),
            &[
                ("memory_create", json!({"path": "/memories/a.md"})),
                ("memory_create", json!({"path": "/memories/FAIL.md"})),
            ],
        )
        .unwrap();
        assert_eq!(got, [Ok("done 1".into()), Err("done 2".into())]);
        let sent = fs::read_to_string(&log).unwrap();
        assert!(sent.lines().next().unwrap().contains("\"initialize\""));
        assert_eq!(sent.matches("tools/call").count(), 2);
        fs::remove_file(&log).unwrap();
        let got = call(
            &argv(&serve).unwrap(),
            &[
                ("memory_create", json!({"path": "/memories/FAIL.md"})),
                ("memory_insert", json!({"path": "/memories/MEMORY.md"})),
            ],
        )
        .unwrap();
        assert_eq!(
            got,
            [Err("done 1".into())],
            "the call after a failure is not made"
        );
        assert_eq!(
            fs::read_to_string(&log)
                .unwrap()
                .matches("tools/call")
                .count(),
            1
        );
    }

    #[test]
    fn serve_must_be_an_array_starting_with_an_absolute_command() {
        assert!(argv("pensieve --store /s").is_err());
        assert!(argv(r#"["pensieve"]"#).is_err());
        assert!(argv("[]").is_err());
        assert_eq!(argv(r#"["/b/p", "--store", "/s"]"#).unwrap().len(), 3);
    }

    #[test]
    fn a_server_that_answers_nothing_fails_with_why() {
        let d = TempDir::new().unwrap();
        let e = call(&["/bin/true".to_string()], &[("memory_view", json!({}))]).unwrap_err();
        assert!(e.to_string().contains("did not start as a server"), "{e}");
        assert!(call(&[d.path().join("none").display().to_string()], &[]).is_err());
    }
}
