//! agent-orch — observation-only orchestrator over tmux + coding-agent panes.
//!
//! Script-style: one file. Four small typeclasses, top-to-bottom:
//!
//!   §1 · `Session`   — registry record + apply_event + format. Read/write
//!                      under flock for mutators; reads are eventually
//!                      consistent, no lock.
//!   §2 · `Wrapper`   — `wrap <kind> -- <argv>`: register, install per-kind
//!                      hook config, exec the agent. tmux-only.
//!   §3 · `Hook`      — `hook <event>`: read stdin, mutate the matching
//!                      record under flock.
//!   §4 · `Loop`      — `<bare>` and `pick`: ensure orchestrator session,
//!                      drive an fzf picker, switch-client to the chosen
//!                      pane. `unregister` is the tmux pane-exited target.
//!
//! Tests sit at the bottom — a few unit tests on pure functions, a few
//! behavior tests on the typeclass entrypoints. End-to-end coverage is
//! the shell integration test (`tests/integration.sh`).

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

// ─────────────────────────────────────────────────────────────────────────────
// §1 · Session
//
// One record per registered agent pane. The state-dir contains:
//   sessions.json     — the registry, a top-level array of Session
//   sessions.lock     — flock target, no content
//   tmp/<pane>/       — per-pane scratch (claude --settings file lives here)
//   .tmux-hook-installed — marker so wrap installs the global hook once
//
// The pid recorded on register is our own; after `execvp` the kernel keeps
// the pid pointing at the same process, now running the agent. So the live
// agent's pid == sessions.json's `pid` field == liveness probe target.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Unknown,
    Running,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub pane_id: String,
    pub pid: i32,
    pub kind: String,
    pub cwd: String,
    pub started: u64,
    pub state: State,
    pub state_ts: u64,
    #[serde(default)]
    pub last_prompt: String,
    #[serde(default)]
    pub last_tool: String,
    #[serde(default)]
    pub last_event: String,
    #[serde(default)]
    pub last_event_ts: u64,
    #[serde(default)]
    pub created_kiro_config: bool,
}

const EVT_USER_PROMPT_SUBMIT: &str = "UserPromptSubmit";
const EVT_PRE_TOOL_USE: &str = "PreToolUse";
const EVT_POST_TOOL_USE: &str = "PostToolUse";
const EVT_STOP: &str = "Stop";

impl Session {
    /// Apply one hook event to this session. Bumps `last_event` /
    /// `last_event_ts` unconditionally; per-event updates below.
    fn apply_event(&mut self, event: &str, prompt: Option<&str>, tool: Option<&str>, now: u64) {
        self.last_event = event.into();
        self.last_event_ts = now;
        match event {
            EVT_USER_PROMPT_SUBMIT => {
                self.state = State::Running;
                self.state_ts = now;
                if let Some(p) = prompt {
                    self.last_prompt = p.chars().take(80).collect();
                }
            }
            EVT_PRE_TOOL_USE => {
                self.state = State::Running;
                self.state_ts = now;
                if let Some(t) = tool {
                    self.last_tool = t.into();
                }
            }
            EVT_POST_TOOL_USE => {
                if let Some(t) = tool {
                    self.last_tool = t.into();
                }
            }
            EVT_STOP => {
                self.state = State::Complete;
                self.state_ts = now;
            }
            _ => {} // unknown event: only the bumps above
        }
    }
}

fn state_dir() -> Result<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_STATE_HOME") {
        if !xdg.is_empty() {
            return Ok(PathBuf::from(xdg).join("agent-orch"));
        }
    }
    let home = std::env::var("HOME").context("$HOME unset")?;
    Ok(PathBuf::from(home).join(".local/state/agent-orch"))
}

fn sessions_path(state: &Path) -> PathBuf {
    state.join("sessions.json")
}

fn lock_path(state: &Path) -> PathBuf {
    state.join("sessions.lock")
}

fn tmp_dir_for_pane(state: &Path, pane_id: &str) -> PathBuf {
    state.join("tmp").join(pane_id)
}

fn hook_install_marker(state: &Path) -> PathBuf {
    state.join(".tmux-hook-installed")
}

/// Read sessions.json. Empty / missing → `Vec::new()`. Malformed errors loud.
fn read_sessions(state_dir: &Path) -> Result<Vec<Session>> {
    let path = sessions_path(state_dir);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))
}

/// Write sessions.json atomically (write to per-pid tmp, then rename).
/// Caller holds `with_lock` for read-modify-write atomicity.
fn write_sessions(state_dir: &Path, sessions: &[Session]) -> Result<()> {
    fs::create_dir_all(state_dir).with_context(|| format!("mkdir -p {}", state_dir.display()))?;
    let path = sessions_path(state_dir);
    let tmp = path.with_extension(format!("json.tmp.{}", std::process::id()));
    fs::write(&tmp, serde_json::to_vec_pretty(sessions)?)
        .with_context(|| format!("write {}", tmp.display()))?;
    fs::rename(&tmp, &path)
        .with_context(|| format!("rename {} → {}", tmp.display(), path.display()))?;
    Ok(())
}

/// Hold an exclusive POSIX advisory lock on the state dir's lock file
/// for the duration of `f`. Lock releases when the file handle drops,
/// so panics in `f` still release.
fn with_lock<R>(state_dir: &Path, f: impl FnOnce() -> Result<R>) -> Result<R> {
    fs::create_dir_all(state_dir).with_context(|| format!("mkdir -p {}", state_dir.display()))?;
    let file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(lock_path(state_dir))
        .with_context(|| format!("open {}", lock_path(state_dir).display()))?;
    let mut lock = fd_lock::RwLock::new(file);
    let _guard = lock.write().context("flock")?;
    f()
}

/// Sort: running > complete > unknown; within group, most-recently-active first.
fn sort_sessions(sessions: &mut [Session]) {
    sessions.sort_by(|a, b| {
        let group = state_group(&a.state).cmp(&state_group(&b.state));
        if group.is_ne() {
            return group;
        }
        activity(b).cmp(&activity(a))
    });
}

fn state_group(s: &State) -> u8 {
    match s {
        State::Running => 0,
        State::Complete => 1,
        State::Unknown => 2,
    }
}

fn activity(s: &Session) -> u64 {
    s.state_ts.max(s.last_event_ts).max(s.started)
}

/// Picker row: `<glyph> <kind> <cwd-tail> · <prompt> [· <tool>]`.
fn format_row(s: &Session) -> String {
    let glyph = match s.state {
        State::Running => "▶",
        State::Complete => "✓",
        State::Unknown => "·",
    };
    let prompt = if s.last_prompt.is_empty() {
        "—"
    } else {
        s.last_prompt.as_str()
    };
    let mut row = format!("{} {} {} · {}", glyph, s.kind, cwd_tail(&s.cwd), prompt);
    if !s.last_tool.is_empty() {
        row.push_str(" · ");
        row.push_str(&s.last_tool);
    }
    row
}

fn cwd_tail(cwd: &str) -> String {
    use std::path::Component;
    let last2: Vec<&str> = Path::new(cwd)
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .rev()
        .take(2)
        .collect();
    if last2.is_empty() {
        cwd.into()
    } else {
        last2.into_iter().rev().collect::<Vec<_>>().join("/")
    }
}

/// Sessions whose pid is no longer alive (kernel signal-0 probe).
fn live_only(sessions: Vec<Session>) -> Vec<Session> {
    use nix::sys::signal::kill;
    use nix::unistd::Pid;
    sessions
        .into_iter()
        .filter(|s| kill(Pid::from_raw(s.pid), None).is_ok())
        .collect()
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ─────────────────────────────────────────────────────────────────────────────
// §2 · Wrapper
//
// `wrap <kind> -- <argv>` registers the launch and execs the agent.
//
// Per-kind hook config:
//   claude — synthesize `$STATE_DIR/tmp/<pane>/settings.json` (merge
//            user's ~/.claude/settings.json under our four hook entries)
//            and pass `--settings <path>` to the agent.
//   kiro   — write `<cwd>/.kiro/agents/agent-orch.json` if absent (refcount
//            cleanup on unregister); set the agent config so kiro picks it up.
//   other  — register only, no hook injection. Picker shows `unknown` state.

/// Build the (program, argv) we hand to execvp. argv[0] is the program
/// name (POSIX convention — child sees this as its own argv[0]).
fn build_agent_argv(
    kind: &str,
    user_argv: &[String],
    settings_path: &Path,
) -> (String, Vec<String>) {
    let program = user_argv[0].clone();
    let mut argv = user_argv.to_vec();
    if kind == "claude" {
        argv.insert(1, "--settings".into());
        argv.insert(2, settings_path.to_string_lossy().into_owned());
    }
    (program, argv)
}

/// Merge our four hook commands into a Claude-style settings JSON.
/// Errors loud on a non-object root or non-array event entries —
/// silent-drop would leave the user with a hookless wrapper.
fn merge_claude_hooks(settings: &mut serde_json::Value, self_path: &Path) -> Result<()> {
    use serde_json::{json, Value};
    let self_str = self_path.to_string_lossy();
    let root = settings
        .as_object_mut()
        .context("user claude settings root must be a JSON object")?;
    let hooks = root
        .entry("hooks".to_string())
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .context("user claude settings.hooks must be a JSON object")?;
    for ev in [
        EVT_USER_PROMPT_SUBMIT,
        EVT_PRE_TOOL_USE,
        EVT_POST_TOOL_USE,
        EVT_STOP,
    ] {
        let arr = hooks.entry(ev.to_string()).or_insert_with(|| json!([]));
        let Value::Array(list) = arr else {
            anyhow::bail!("user claude settings.hooks.{} must be an array", ev);
        };
        list.push(json!({
            "type": "command",
            "command": format!("{} hook {}", self_str, ev),
        }));
    }
    Ok(())
}

/// Build a fresh Kiro agent config wiring our four hook commands.
fn build_kiro_config(self_path: &Path) -> serde_json::Value {
    use serde_json::json;
    let s = self_path.to_string_lossy();
    json!({
        "hooks": {
            EVT_USER_PROMPT_SUBMIT: [{ "command": format!("{} hook {}", s, EVT_USER_PROMPT_SUBMIT) }],
            EVT_PRE_TOOL_USE:       [{ "command": format!("{} hook {}", s, EVT_PRE_TOOL_USE) }],
            EVT_POST_TOOL_USE:      [{ "command": format!("{} hook {}", s, EVT_POST_TOOL_USE) }],
            EVT_STOP:               [{ "command": format!("{} hook {}", s, EVT_STOP) }],
        }
    })
}

/// Install the global tmux `pane-exited` hook (idempotent via marker file).
/// Race-tolerant: tmux `set-hook -g` is itself idempotent.
fn install_pane_exited_hook(state_dir: &Path, self_path: &Path) -> Result<()> {
    let marker = hook_install_marker(state_dir);
    if marker.exists() {
        return Ok(());
    }
    let cmd = format!(
        "run-shell \"{} unregister #{{hook_pane}}\"",
        self_path.display()
    );
    let status = std::process::Command::new("tmux")
        .args(["set-hook", "-g", "pane-exited", &cmd])
        .status()
        .context("tmux set-hook (is tmux on PATH?)")?;
    anyhow::ensure!(status.success(), "tmux set-hook failed: {status}");
    fs::write(&marker, b"")?;
    Ok(())
}

fn tmux_set_pane_option(pane_id: &str, key: &str, value: &str) -> Result<()> {
    let status = std::process::Command::new("tmux")
        .args(["set-option", "-p", "-t", pane_id, key, value])
        .status()
        .context("tmux set-option")?;
    anyhow::ensure!(status.success(), "tmux set-option failed: {status}");
    Ok(())
}

/// `execvp` into the agent. Returns only on failure.
fn exec_agent(program: &str, argv: &[String]) -> Result<()> {
    use std::ffi::CString;
    let prog = CString::new(program).context("nul in agent program")?;
    let argv_c: Vec<CString> = argv
        .iter()
        .map(|a| CString::new(a.as_str()))
        .collect::<std::result::Result<_, _>>()
        .context("nul in agent argv")?;
    let argv_refs: Vec<&std::ffi::CStr> = argv_c.iter().map(|c| c.as_c_str()).collect();
    nix::unistd::execvp(&prog, &argv_refs).with_context(|| format!("execvp {}", program))?;
    unreachable!("execvp returned Ok")
}

/// Synthesize and write the per-pane Claude settings file. Returns its path.
fn synth_claude_settings(
    state_dir: &Path,
    pane_id: &str,
    user_settings: &Path,
    self_path: &Path,
) -> Result<PathBuf> {
    let dir = tmp_dir_for_pane(state_dir, pane_id);
    fs::create_dir_all(&dir).with_context(|| format!("mkdir -p {}", dir.display()))?;
    let path = dir.join("settings.json");
    let mut settings: serde_json::Value = if user_settings.exists() {
        serde_json::from_slice(&fs::read(user_settings)?)
            .with_context(|| format!("parse {}", user_settings.display()))?
    } else {
        serde_json::json!({})
    };
    merge_claude_hooks(&mut settings, self_path)?;
    fs::write(&path, serde_json::to_vec_pretty(&settings)?)?;
    Ok(path)
}

/// Write `<cwd>/.kiro/agents/agent-orch.json` if absent. Returns `true`
/// when this call created the file (caller stamps the flag on the record
/// for observability; cleanup on unregister is creation-flag-agnostic).
fn ensure_kiro_config(cwd: &Path, self_path: &Path) -> Result<bool> {
    let dir = cwd.join(".kiro").join("agents");
    let path = dir.join("agent-orch.json");
    if path.exists() {
        return Ok(false);
    }
    fs::create_dir_all(&dir).with_context(|| format!("mkdir -p {}", dir.display()))?;
    fs::write(
        &path,
        serde_json::to_vec_pretty(&build_kiro_config(self_path))?,
    )?;
    Ok(true)
}

#[derive(Debug)]
struct WrapInputs {
    state_dir: PathBuf,
    self_path: PathBuf,
    user_claude_settings: PathBuf,
    pane_id: Option<String>,
    cwd_override: Option<PathBuf>,
    side_effects: bool,
}

/// `wrap` typeclass entrypoint. Performs disk-side effects (register +
/// per-kind config); then if `side_effects=true`, installs the tmux hook
/// and execvps the agent. Tests pass `side_effects=false` and verify
/// the disk state directly. Smoke / integration covers the exec path.
fn wrap(inputs: &WrapInputs, kind: &str, agent_argv: &[String]) -> Result<()> {
    let pane_id = inputs
        .pane_id
        .clone()
        .context("agent-orch wrap requires $TMUX_PANE — run inside tmux")?;
    anyhow::ensure!(
        !agent_argv.is_empty(),
        "agent-orch wrap needs an agent command after `--`"
    );

    let cwd = match &inputs.cwd_override {
        Some(p) => p.clone(),
        None => std::env::current_dir().context("current_dir")?,
    };

    // Claude settings are pane-scoped — outside the lock is fine, no
    // concurrent writer can target the same per-pane tmpdir.
    let claude_settings = if kind == "claude" {
        Some(synth_claude_settings(
            &inputs.state_dir,
            &pane_id,
            &inputs.user_claude_settings,
            &inputs.self_path,
        )?)
    } else {
        None
    };

    // Kiro config + registry write happen in one critical section.
    // Race we're closing: without the lock, ensure_kiro_config could
    // see "file exists" right before another pane's unregister sees
    // "no live kiro siblings" and removes it — leaving us live with a
    // missing config. Inside the lock, unregister can't observe the
    // gap.
    let now = now_secs();
    with_lock(&inputs.state_dir, || {
        let created_kiro_config = if kind == "kiro" {
            ensure_kiro_config(&cwd, &inputs.self_path)?
        } else {
            false
        };
        let mut sessions = read_sessions(&inputs.state_dir)?;
        anyhow::ensure!(
            !sessions.iter().any(|x| x.pane_id == pane_id),
            "pane {} already registered — `agent-orch unregister {}` first",
            pane_id,
            pane_id
        );
        sessions.push(Session {
            pane_id: pane_id.clone(),
            pid: std::process::id() as i32,
            kind: kind.into(),
            cwd: cwd.to_string_lossy().into_owned(),
            started: now,
            state: State::Unknown,
            state_ts: now,
            last_prompt: String::new(),
            last_tool: String::new(),
            last_event: String::new(),
            last_event_ts: 0,
            created_kiro_config,
        });
        write_sessions(&inputs.state_dir, &sessions)
    })?;

    if inputs.side_effects {
        install_pane_exited_hook(&inputs.state_dir, &inputs.self_path)?;
        tmux_set_pane_option(&pane_id, "@agent-orch-pane", &pane_id)?;
        let settings_for_argv = claude_settings.as_deref().unwrap_or_else(|| Path::new(""));
        let (program, argv) = build_agent_argv(kind, agent_argv, settings_for_argv);
        // SAFETY: single-threaded immediately before execvp.
        std::env::set_var("AGENT_ORCH_PANE", &pane_id);
        exec_agent(&program, &argv)?;
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// §3 · Hook
//
// `hook <event> < <stdin-json>` mutates the matching record's state
// under flock. Always exits 0 — a failing reporter must not block the
// agent's turn.

/// Apply one hook event to the matching record. No-op if the record
/// is gone (stale fire after unregister). `pane_id` is the wrapper-set
/// `$AGENT_ORCH_PANE` value; passed in explicitly so tests don't fight
/// process-global env state under cargo's parallel runner.
fn hook(
    state_dir: &Path,
    pane_id: &str,
    event: &str,
    stdin: &mut dyn Read,
    now: u64,
) -> Result<()> {
    let mut buf = Vec::new();
    stdin.read_to_end(&mut buf).context("read hook payload")?;
    let payload: serde_json::Value = if buf.is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_slice(&buf).unwrap_or(serde_json::json!({}))
    };
    let prompt = payload.get("prompt").and_then(|v| v.as_str());
    let tool = payload.get("tool_name").and_then(|v| v.as_str());

    with_lock(state_dir, || {
        let mut sessions = read_sessions(state_dir)?;
        let Some(s) = sessions.iter_mut().find(|s| s.pane_id == pane_id) else {
            return Ok(()); // stale fire after unregister; drop.
        };
        s.apply_event(event, prompt, tool, now);
        write_sessions(state_dir, &sessions)
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// §4 · Loop
//
// Two surfaces:
//   `pick`  — prints one selected pane id to stdout (used by the loop body).
//   `<bare>` and `loop` — ensure the orchestrator session exists, switch
//                          client, run the picker loop.
//
// The picker reads sessions.json + filters by pid liveness, runs `fzf`
// over the formatted rows, prints the chosen pane id. The loop body
// invokes `pick` and shells out to `tmux switch-client -t <pane>`.

const ORCHESTRATOR_SESSION: &str = "orchestrator";

/// Render rows for the picker, filtered to live sessions, sorted.
fn render_rows(state_dir: &Path) -> Result<Vec<(String, String)>> {
    let mut sessions = live_only(read_sessions(state_dir)?);
    sort_sessions(&mut sessions);
    Ok(sessions
        .into_iter()
        .map(|s| (s.pane_id.clone(), format_row(&s)))
        .collect())
}

/// Run fzf over the rendered rows. Selection → pane id on stdout.
/// Empty registry → exit 0 with no output. fzf cancel → exit 0 silently.
fn pick(state_dir: &Path, stdout: &mut dyn std::io::Write) -> Result<()> {
    let rows = render_rows(state_dir)?;
    if rows.is_empty() {
        return Ok(());
    }
    let mut child = std::process::Command::new("fzf")
        .arg("--with-nth=2..")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .context("spawn fzf (is it on PATH?)")?;
    {
        use std::io::Write as _;
        let stdin = child.stdin.as_mut().context("fzf stdin")?;
        for (id, row) in &rows {
            writeln!(stdin, "{}\t{}", id, row)?;
        }
    }
    let out = child.wait_with_output().context("fzf wait")?;
    if !out.status.success() {
        return Ok(()); // fzf cancelled (Esc/^C) — silent.
    }
    let line = String::from_utf8_lossy(&out.stdout);
    if let Some(pane) = line.split('\t').next() {
        writeln!(stdout, "{}", pane.trim())?;
    }
    Ok(())
}

/// `unregister <pane>`: tmux pane-exited target. Removes the record,
/// runs Kiro orphan-config cleanup (refcount-agnostic), and rm-rf's
/// the per-pane tmp dir.
fn unregister(state_dir: &Path, pane_id: &str) -> Result<()> {
    with_lock(state_dir, || {
        let sessions = read_sessions(state_dir)?;
        let Some(idx) = sessions.iter().position(|s| s.pane_id == pane_id) else {
            return Ok(()); // already gone.
        };
        let removing = sessions[idx].clone();
        let mut remaining = sessions.clone();
        remaining.remove(idx);

        // Kiro: if no live kiro session remains in the same cwd, remove
        // the project-scoped agent config. Creation-flag-agnostic so the
        // close-creator-first ordering doesn't leak (see spec).
        if removing.kind == "kiro"
            && !remaining
                .iter()
                .any(|s| s.kind == "kiro" && s.cwd == removing.cwd)
        {
            let cfg = PathBuf::from(&removing.cwd)
                .join(".kiro")
                .join("agents")
                .join("agent-orch.json");
            let _ = fs::remove_file(&cfg);
            let _ = fs::remove_dir(cfg.parent().unwrap());
            let _ = fs::remove_dir(cfg.parent().and_then(|p| p.parent()).unwrap());
        }

        // Per-pane scratch (Claude --settings file).
        let _ = fs::remove_dir_all(tmp_dir_for_pane(state_dir, pane_id));

        write_sessions(state_dir, &remaining)
    })
}

/// Ensure the orchestrator tmux session exists, switch the client to it,
/// and run the picker loop in its window. The loop body itself runs as
/// `agent-orch loop-body` inside the new session.
fn run_loop(self_path: &Path) -> Result<()> {
    // ensure-session
    let has = std::process::Command::new("tmux")
        .args(["has-session", "-t", ORCHESTRATOR_SESSION])
        .status()
        .context("tmux has-session")?
        .success();
    if !has {
        let cmd = format!("{} loop-body", self_path.display());
        let status = std::process::Command::new("tmux")
            .args(["new-session", "-d", "-s", ORCHESTRATOR_SESSION, &cmd])
            .status()
            .context("tmux new-session")?;
        anyhow::ensure!(status.success(), "tmux new-session failed");
    }
    let status = std::process::Command::new("tmux")
        .args(["switch-client", "-t", ORCHESTRATOR_SESSION])
        .status()
        .context("tmux switch-client")?;
    anyhow::ensure!(status.success(), "tmux switch-client failed");
    Ok(())
}

/// Picker loop body. Runs inside the orchestrator session.
/// pick → switch-client → repeat. fzf-cancel sleeps briefly and re-renders.
fn loop_body(state_dir: &Path) -> Result<()> {
    use std::io::Write as _;
    loop {
        let mut buf = Vec::new();
        if let Err(e) = pick(state_dir, &mut buf) {
            eprintln!("pick: {e:#}");
            std::thread::sleep(std::time::Duration::from_millis(500));
            continue;
        }
        let line = String::from_utf8_lossy(&buf);
        let pane = line.trim();
        if pane.is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(200));
            continue;
        }
        let _ = std::process::Command::new("tmux")
            .args(["switch-client", "-t", pane])
            .status();
        // After switch-client returns, the user is on the agent's pane.
        // The loop iterates so when they come back to the orchestrator
        // (M-o keybind), the picker is fresh.
        std::io::stderr().flush().ok();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CLI

#[derive(Parser)]
#[command(
    name = "agent-orch",
    version,
    about = "Observation-only orchestrator over tmux + coding-agent panes."
)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Wrap a coding agent: register, inject hooks, execvp.
    Wrap {
        kind: String,
        #[arg(long)]
        cwd: Option<PathBuf>,
        #[arg(last = true)]
        agent_argv: Vec<String>,
    },
    /// Hook reporter; called by Claude/Kiro on each lifecycle event.
    Hook { event: String },
    /// Print one selected pane id from the registry to stdout.
    Pick,
    /// Print the live registry, one pane per line.
    List,
    /// Remove a record (called by tmux pane-exited and on demand).
    Unregister { pane_id: String },
    /// Picker loop body — runs inside the orchestrator session.
    #[command(name = "loop-body")]
    LoopBody,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let state = state_dir()?;
    let self_path = std::env::current_exe().context("current_exe")?;

    match cli.cmd {
        // Bare invocation: ensure-session + switch-client. The loop body
        // (which consumes `state`) runs in the spawned orchestrator session.
        None => run_loop(&self_path),

        Some(Cmd::Wrap {
            kind,
            cwd,
            agent_argv,
        }) => {
            let inputs = WrapInputs {
                state_dir: state,
                self_path,
                user_claude_settings: std::env::var("HOME")
                    .map(|h| PathBuf::from(h).join(".claude/settings.json"))
                    .unwrap_or_default(),
                pane_id: std::env::var("TMUX_PANE").ok().filter(|s| !s.is_empty()),
                cwd_override: cwd,
                side_effects: true,
            };
            wrap(&inputs, &kind, &agent_argv)
        }

        Some(Cmd::Hook { event }) => {
            // Hooks must never block the agent's turn — fail-soft.
            let pane_id = std::env::var("AGENT_ORCH_PANE")
                .ok()
                .filter(|s| !s.is_empty());
            if let Some(pane) = pane_id {
                let mut stdin = std::io::stdin();
                let _ = hook(&state, &pane, &event, &mut stdin, now_secs());
            }
            Ok(())
        }

        Some(Cmd::Pick) => {
            let mut stdout = std::io::stdout();
            pick(&state, &mut stdout)
        }

        Some(Cmd::List) => {
            let mut sessions = read_sessions(&state)?;
            if sessions.is_empty() {
                println!("(no registered sessions)");
                return Ok(());
            }
            sort_sessions(&mut sessions);
            for s in &sessions {
                println!("{}\t{}", s.pane_id, format_row(s));
            }
            Ok(())
        }

        Some(Cmd::Unregister { pane_id }) => unregister(&state, &pane_id),

        Some(Cmd::LoopBody) => loop_body(&state),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
//
// Each typeclass entrypoint is tested directly with a tempdir state-dir.
// No Env/run_command indirection — tests call the functions the same way
// `main` does. End-to-end coverage (wrapper exec, tmux hooks, picker UX)
// is the shell integration test (`tests/integration.sh`).

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tempfile::tempdir;

    fn mk(pane: &str, kind: &str, cwd: &str, started: u64) -> Session {
        Session {
            pane_id: pane.into(),
            pid: std::process::id() as i32,
            kind: kind.into(),
            cwd: cwd.into(),
            started,
            state: State::Unknown,
            state_ts: started,
            last_prompt: String::new(),
            last_tool: String::new(),
            last_event: String::new(),
            last_event_ts: 0,
            created_kiro_config: false,
        }
    }

    // §1 · Session

    #[test]
    fn session_round_trip() {
        let dir = tempdir().unwrap();
        let s = mk("%1", "claude", "/repo", 1000);
        write_sessions(dir.path(), std::slice::from_ref(&s)).unwrap();
        assert_eq!(read_sessions(dir.path()).unwrap(), vec![s]);
    }

    #[test]
    fn session_apply_event_transitions() {
        let mut s = mk("%1", "claude", "/repo", 1000);
        s.apply_event(EVT_USER_PROMPT_SUBMIT, Some("hello"), None, 1100);
        assert_eq!(s.state, State::Running);
        assert_eq!(s.last_prompt, "hello");

        s.apply_event(EVT_PRE_TOOL_USE, None, Some("Bash"), 1200);
        assert_eq!(s.last_tool, "Bash");

        s.apply_event(EVT_STOP, None, None, 1300);
        assert_eq!(s.state, State::Complete);
    }

    #[test]
    fn session_apply_event_truncates_long_prompt() {
        let mut s = mk("%1", "claude", "/repo", 1);
        s.apply_event(EVT_USER_PROMPT_SUBMIT, Some(&"x".repeat(200)), None, 2);
        assert_eq!(s.last_prompt.chars().count(), 80);
    }

    #[test]
    fn cwd_tail_handles_root_anchored() {
        // Once produced "//repo" because RootDir Component joined as "/".
        assert_eq!(cwd_tail("/home/me/repo/foo"), "repo/foo");
        assert_eq!(cwd_tail("/repo"), "repo");
        assert_eq!(cwd_tail(""), "");
    }

    #[test]
    fn sort_running_first_then_complete_then_unknown_recent_first() {
        let mut a = mk("%a", "claude", "/x", 100);
        a.state = State::Complete;
        a.state_ts = 200;
        let mut b = mk("%b", "claude", "/y", 100);
        b.state = State::Running;
        b.state_ts = 150;
        let mut c = mk("%c", "claude", "/z", 100);
        c.state = State::Running;
        c.state_ts = 300;
        let d = mk("%d", "kiro", "/w", 100);
        let mut v = vec![a, b, c, d];
        sort_sessions(&mut v);
        assert_eq!(
            v.iter().map(|s| s.pane_id.as_str()).collect::<Vec<_>>(),
            vec!["%c", "%b", "%a", "%d"]
        );
    }

    // §2 · Wrapper

    fn wrap_inputs(state: &Path, pane: Option<&str>, user: Option<&Path>) -> WrapInputs {
        WrapInputs {
            state_dir: state.to_path_buf(),
            self_path: PathBuf::from("/test/agent-orch"),
            user_claude_settings: user
                .map(Path::to_path_buf)
                .unwrap_or_else(|| state.join("nonexistent.json")),
            pane_id: pane.map(String::from),
            cwd_override: Some(state.to_path_buf()), // tests run with cwd=tempdir
            side_effects: false,
        }
    }

    #[test]
    fn build_argv_claude_splices_settings_after_slot0() {
        let argv = vec!["claude".into(), "--resume".into(), "abc".into()];
        let (prog, out) = build_agent_argv("claude", &argv, Path::new("/tmp/s.json"));
        assert_eq!(prog, "claude");
        assert_eq!(
            out,
            vec!["claude", "--settings", "/tmp/s.json", "--resume", "abc"]
        );
    }

    #[test]
    fn build_argv_non_claude_passes_through() {
        let argv = vec!["kiro".into(), "chat".into()];
        let (prog, out) = build_agent_argv("kiro", &argv, Path::new("/tmp/s.json"));
        assert_eq!(prog, "kiro");
        assert_eq!(out, vec!["kiro", "chat"]);
    }

    #[test]
    fn wrap_claude_appends_session_and_synthesizes_settings() {
        let dir = tempdir().unwrap();
        let inputs = wrap_inputs(dir.path(), Some("%42"), None);
        wrap(&inputs, "claude", &["claude-stub".into()]).unwrap();

        let sessions = read_sessions(dir.path()).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].pane_id, "%42");
        assert_eq!(sessions[0].kind, "claude");
        assert!(!sessions[0].created_kiro_config);

        let synth: serde_json::Value = serde_json::from_slice(
            &fs::read(tmp_dir_for_pane(dir.path(), "%42").join("settings.json")).unwrap(),
        )
        .unwrap();
        for ev in [
            EVT_USER_PROMPT_SUBMIT,
            EVT_PRE_TOOL_USE,
            EVT_POST_TOOL_USE,
            EVT_STOP,
        ] {
            assert_eq!(
                synth["hooks"][ev][0]["command"],
                format!("/test/agent-orch hook {}", ev)
            );
        }
    }

    #[test]
    fn wrap_claude_merges_user_existing_hook() {
        let dir = tempdir().unwrap();
        let user = dir.path().join("user.json");
        fs::write(
            &user,
            br#"{"hooks":{"UserPromptSubmit":[{"type":"command","command":"my-hook"}]}}"#,
        )
        .unwrap();
        let inputs = wrap_inputs(dir.path(), Some("%7"), Some(&user));
        wrap(&inputs, "claude", &["stub".into()]).unwrap();
        let synth: serde_json::Value = serde_json::from_slice(
            &fs::read(tmp_dir_for_pane(dir.path(), "%7").join("settings.json")).unwrap(),
        )
        .unwrap();
        let ups = &synth["hooks"]["UserPromptSubmit"];
        assert_eq!(ups[0]["command"], "my-hook");
        assert_eq!(ups[1]["command"], "/test/agent-orch hook UserPromptSubmit");
    }

    #[test]
    fn wrap_claude_rejects_non_object_user_settings() {
        let dir = tempdir().unwrap();
        let user = dir.path().join("user.json");
        fs::write(&user, b"[]").unwrap();
        let inputs = wrap_inputs(dir.path(), Some("%3"), Some(&user));
        let err = wrap(&inputs, "claude", &["stub".into()]).unwrap_err();
        assert!(format!("{err:#}").contains("must be a JSON object"));
    }

    #[test]
    fn wrap_kiro_writes_project_config_and_stamps_flag() {
        let dir = tempdir().unwrap();
        let inputs = wrap_inputs(dir.path(), Some("%9"), None);
        wrap(&inputs, "kiro", &["kiro-stub".into()]).unwrap();

        let sessions = read_sessions(dir.path()).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].kind, "kiro");
        assert!(sessions[0].created_kiro_config);
        let cfg = dir
            .path()
            .join(".kiro")
            .join("agents")
            .join("agent-orch.json");
        assert!(cfg.exists());
        let parsed: serde_json::Value = serde_json::from_slice(&fs::read(&cfg).unwrap()).unwrap();
        for ev in [
            EVT_USER_PROMPT_SUBMIT,
            EVT_PRE_TOOL_USE,
            EVT_POST_TOOL_USE,
            EVT_STOP,
        ] {
            assert_eq!(
                parsed["hooks"][ev][0]["command"],
                format!("/test/agent-orch hook {}", ev)
            );
        }
    }

    #[test]
    fn wrap_kiro_reuser_does_not_stamp_flag() {
        let dir = tempdir().unwrap();
        // First launch in cwd creates the file.
        let inputs = wrap_inputs(dir.path(), Some("%1"), None);
        wrap(&inputs, "kiro", &["stub".into()]).unwrap();
        // Second launch in the same cwd reuses; flag must be false.
        let inputs2 = wrap_inputs(dir.path(), Some("%2"), None);
        wrap(&inputs2, "kiro", &["stub".into()]).unwrap();
        let sessions = read_sessions(dir.path()).unwrap();
        let s2 = sessions.iter().find(|s| s.pane_id == "%2").unwrap();
        assert!(!s2.created_kiro_config);
    }

    #[test]
    fn wrap_refuses_double_register_on_same_pane() {
        let dir = tempdir().unwrap();
        let inputs = wrap_inputs(dir.path(), Some("%5"), None);
        wrap(&inputs, "claude", &["stub".into()]).unwrap();
        let err = wrap(&inputs, "claude", &["stub".into()]).unwrap_err();
        assert!(format!("{err:#}").contains("already registered"));
    }

    #[test]
    fn wrap_refuses_without_pane_id() {
        let dir = tempdir().unwrap();
        let inputs = wrap_inputs(dir.path(), None, None);
        let err = wrap(&inputs, "claude", &["stub".into()]).unwrap_err();
        assert!(format!("{err:#}").contains("$TMUX_PANE"));
    }

    // §3 · Hook

    #[test]
    fn hook_user_prompt_submit_marks_running_and_stores_prompt() {
        let dir = tempdir().unwrap();
        let inputs = wrap_inputs(dir.path(), Some("%9"), None);
        wrap(&inputs, "claude", &["stub".into()]).unwrap();
        let payload = br#"{"prompt":"fix the test"}"#.to_vec();
        hook(
            dir.path(),
            "%9",
            EVT_USER_PROMPT_SUBMIT,
            &mut Cursor::new(payload),
            1234,
        )
        .unwrap();
        let sessions = read_sessions(dir.path()).unwrap();
        assert_eq!(sessions[0].state, State::Running);
        assert_eq!(sessions[0].last_prompt, "fix the test");
    }

    #[test]
    fn hook_stop_marks_complete() {
        let dir = tempdir().unwrap();
        let inputs = wrap_inputs(dir.path(), Some("%1"), None);
        wrap(&inputs, "claude", &["stub".into()]).unwrap();
        hook(
            dir.path(),
            "%1",
            EVT_STOP,
            &mut Cursor::new(b"{}".to_vec()),
            99,
        )
        .unwrap();
        let sessions = read_sessions(dir.path()).unwrap();
        assert_eq!(sessions[0].state, State::Complete);
    }

    #[test]
    fn hook_no_op_for_unknown_pane() {
        let dir = tempdir().unwrap();
        // No record for %999 — must not error, must not write a phantom.
        hook(
            dir.path(),
            "%999",
            EVT_STOP,
            &mut Cursor::new(b"{}".to_vec()),
            1,
        )
        .unwrap();
        assert!(read_sessions(dir.path()).unwrap().is_empty());
    }

    // §4 · Loop / unregister

    #[test]
    fn unregister_removes_record_and_kiro_config() {
        let dir = tempdir().unwrap();
        let inputs = wrap_inputs(dir.path(), Some("%1"), None);
        wrap(&inputs, "kiro", &["stub".into()]).unwrap();
        let cfg = dir
            .path()
            .join(".kiro")
            .join("agents")
            .join("agent-orch.json");
        assert!(cfg.exists());
        unregister(dir.path(), "%1").unwrap();
        assert!(read_sessions(dir.path()).unwrap().is_empty());
        assert!(!cfg.exists());
    }

    #[test]
    fn unregister_kiro_keeps_config_while_sibling_alive() {
        let dir = tempdir().unwrap();
        // Two kiro sessions in the same cwd.
        wrap(
            &wrap_inputs(dir.path(), Some("%1"), None),
            "kiro",
            &["stub".into()],
        )
        .unwrap();
        wrap(
            &wrap_inputs(dir.path(), Some("%2"), None),
            "kiro",
            &["stub".into()],
        )
        .unwrap();
        let cfg = dir
            .path()
            .join(".kiro")
            .join("agents")
            .join("agent-orch.json");
        assert!(cfg.exists());

        // Close creator first (the original concern). File must stay.
        unregister(dir.path(), "%1").unwrap();
        assert!(
            cfg.exists(),
            "%1 closed but %2 still alive — config must remain"
        );

        // Close the reuser. Now no kiro sessions in cwd → file removed,
        // even though %2's flag was false. Refcount-agnostic.
        unregister(dir.path(), "%2").unwrap();
        assert!(
            !cfg.exists(),
            "all kiro sessions closed — config must be gone"
        );
    }

    #[test]
    fn unregister_idempotent_on_unknown_pane() {
        let dir = tempdir().unwrap();
        unregister(dir.path(), "%does-not-exist").unwrap();
    }

    #[test]
    fn unregister_removes_per_pane_tmp_dir() {
        let dir = tempdir().unwrap();
        wrap(
            &wrap_inputs(dir.path(), Some("%4"), None),
            "claude",
            &["stub".into()],
        )
        .unwrap();
        let pane_tmp = tmp_dir_for_pane(dir.path(), "%4");
        assert!(pane_tmp.join("settings.json").exists());
        unregister(dir.path(), "%4").unwrap();
        assert!(!pane_tmp.exists());
    }

    #[test]
    fn render_rows_filters_dead_pids_and_sorts() {
        let dir = tempdir().unwrap();
        let mut alive = mk("%alive", "claude", "/x", 1);
        alive.state = State::Running;
        alive.state_ts = 100;
        // pid that is almost certainly dead — skip the test if it
        // happens to be alive (extremely unlikely on a real machine).
        let mut dead = mk("%dead", "claude", "/y", 2);
        dead.pid = 1;
        dead.state = State::Running;
        write_sessions(dir.path(), &[alive, dead]).unwrap();
        let rows = render_rows(dir.path()).unwrap();
        assert_eq!(rows.len(), 1, "dead pid must be filtered: {rows:?}");
        assert_eq!(rows[0].0, "%alive");
    }
}
