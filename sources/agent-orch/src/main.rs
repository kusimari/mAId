//! agent-orch — observation-only orchestrator over tmux + coding-agent panes.
//!
//! Script-style: one file. Read top-to-bottom. Layout:
//!   1. Constants + paths.
//!   2. `Session` model + `apply_event` impl.
//!   3. Pure helpers: read/write sessions, format, sort, filter,
//!      kiro-orphan classification.
//!   4. Locked-IO helper (acquire flock, run a closure).
//!   5. `Env` (testable I/O surface — state-dir, stdout, stderr).
//!   6. Subcommand handlers (`cmd_wrap`, `cmd_hook`, ... — all
//!      stubs in slice 1, filled in subsequent slices).
//!      `run_command(env, args)` → exit code: behavior-test entrypoint.
//!   7. `clap` dispatch in `main` (delegates to `run_command`).
//!   8. `#[cfg(test)] mod tests` at the bottom — split into
//!      `mod behavior` (drives `run_command`) and `mod helpers`
//!      (small, only what behavior tests can't reach).
//!
//! Splitting this file is a v2 concern (soft threshold ~1000 LOC).

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

// ── 1 · paths ──────────────────────────────────────────────────────

/// State directory: `${XDG_STATE_HOME:-$HOME/.local/state}/agent-orch`.
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

// Used by slice 2 (wrap) and slice 4 (unregister cleanup); the
// allow lifts the slice-1-only "unused" lint without hiding it.
#[allow(dead_code)]
fn tmp_dir_for_pane(state: &Path, pane_id: &str) -> PathBuf {
    state.join("tmp").join(pane_id)
}

// ── 2 · session model ──────────────────────────────────────────────

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

/// Hook event names from Claude Code / Kiro CLI. Kept as &str
/// to keep clap parsing trivial; converted at the apply site.
pub const EVT_USER_PROMPT_SUBMIT: &str = "UserPromptSubmit";
pub const EVT_PRE_TOOL_USE: &str = "PreToolUse";
pub const EVT_POST_TOOL_USE: &str = "PostToolUse";
pub const EVT_STOP: &str = "Stop";

impl Session {
    /// Apply one hook event to this session. Returns `true` if any
    /// observable field changed (caller decides whether to write).
    ///
    /// In practice this almost always returns `true`, since
    /// `last_event_ts` is bumped to `now` on every call. The bool
    /// is useful only if the caller passes a `now` that may equal
    /// the previous `last_event_ts` (e.g. coarse-grained clock,
    /// replay). Treat it as a write-elision hint, not a contract.
    ///
    /// `prompt` and `tool` carry the relevant payload field where
    /// the event provides one; both empty/None for events that don't.
    pub fn apply_event(
        &mut self,
        event: &str,
        prompt: Option<&str>,
        tool: Option<&str>,
        now: u64,
    ) -> bool {
        let before = self.clone();
        self.last_event = event.to_string();
        self.last_event_ts = now;
        match event {
            EVT_USER_PROMPT_SUBMIT => {
                self.state = State::Running;
                self.state_ts = now;
                if let Some(p) = prompt {
                    self.last_prompt = truncate(p, 80);
                }
            }
            EVT_PRE_TOOL_USE => {
                self.state = State::Running;
                self.state_ts = now;
                if let Some(t) = tool {
                    self.last_tool = t.to_string();
                }
            }
            EVT_POST_TOOL_USE => {
                if let Some(t) = tool {
                    self.last_tool = t.to_string();
                }
            }
            EVT_STOP => {
                self.state = State::Complete;
                self.state_ts = now;
            }
            _ => {} // unknown events: just record last_event_ts, no state change
        }
        before != *self
    }
}

// 80 scalar values, not graphemes — this is preview text shown
// in a one-line picker row; clipping a flag emoji's components is
// acceptable. Don't "fix" to byte-truncation, that breaks UTF-8.
fn truncate(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

// ── 3 · pure helpers (parse / format / sort / filter / kiro) ───────

/// Read sessions.json into a Vec. Empty/missing file → empty Vec.
/// A malformed file errors loudly — silent skip would mask data
/// corruption.
pub fn read_sessions(path: &Path) -> Result<Vec<Session>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    let v: Vec<Session> =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;
    Ok(v)
}

/// Write the array atomically: write to `<path>.tmp`, then `rename`.
/// Readers using `read_sessions` never see a partial write because
/// `rename(2)` is atomic on the same filesystem.
///
/// No `fsync` — this is state-dir scratch; loss-after-power-failure
/// is acceptable (the registry rebuilds on the next agent launch +
/// pane-exited sweep).
pub fn write_sessions_atomic(path: &Path, sessions: &[Session]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("mkdir -p {}", parent.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(sessions)?;
    fs::write(&tmp, &bytes).with_context(|| format!("write {}", tmp.display()))?;
    fs::rename(&tmp, path)
        .with_context(|| format!("rename {} → {}", tmp.display(), path.display()))?;
    Ok(())
}

/// Picker row format.
/// `<glyph> <kind> <cwd-tail> · <last_prompt> [· <last_tool>]`
pub fn format_row(s: &Session) -> String {
    let glyph = match s.state {
        State::Running => "▶",
        State::Complete => "✓",
        State::Unknown => "·",
    };
    let cwd_tail = cwd_tail(&s.cwd);
    let prompt = if s.last_prompt.is_empty() {
        "—"
    } else {
        s.last_prompt.as_str()
    };
    let mut row = format!("{} {} {} · {}", glyph, s.kind, cwd_tail, prompt);
    if !s.last_tool.is_empty() {
        row.push_str(" · ");
        row.push_str(&s.last_tool);
    }
    row
}

fn cwd_tail(cwd: &str) -> String {
    use std::path::Component;
    let p = Path::new(cwd);
    let last2: Vec<&str> = p
        .components()
        // Skip RootDir / CurDir / ParentDir / Prefix — only keep
        // named segments. Otherwise "/repo" yields ["/", "repo"]
        // and joins to "//repo".
        .filter_map(|c| match c {
            Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .rev()
        .take(2)
        .collect();
    if last2.is_empty() {
        cwd.to_string()
    } else {
        last2.into_iter().rev().collect::<Vec<_>>().join("/")
    }
}

/// Sort: running > complete > unknown; within group, most-recently-active first.
/// "Active" = max(state_ts, last_event_ts, started).
pub fn sort_sessions(sessions: &mut [Session]) {
    sessions.sort_by(|a, b| {
        let group = state_group(&a.state).cmp(&state_group(&b.state));
        if group.is_ne() {
            return group;
        }
        let a_act = activity(a);
        let b_act = activity(b);
        b_act.cmp(&a_act) // descending
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

/// Drop entries whose pid is no longer alive. `is_alive` is a
/// callback so tests can inject deterministic liveness.
pub fn live_filter<F>(sessions: Vec<Session>, mut is_alive: F) -> Vec<Session>
where
    F: FnMut(i32) -> bool,
{
    sessions.into_iter().filter(|s| is_alive(s.pid)).collect()
}

/// Returns paths to `.kiro/agents/agent-orch.json` files that
/// should be removed when `removing` exits, given the live
/// sibling set (excluding `removing` itself).
///
/// Rule: if `removing.kind == "kiro"` AND no other live `kind=kiro`
/// session shares its `cwd`, remove that cwd's
/// `.kiro/agents/agent-orch.json`. Creation-flag-agnostic
/// (avoids the order-dependent leak when reusers outlive the
/// creator — see spec Decision Log).
pub fn kiro_orphan_paths(removing: &Session, others: &[Session]) -> Vec<PathBuf> {
    if removing.kind != "kiro" {
        return Vec::new();
    }
    let has_sibling = others
        .iter()
        .any(|s| s.kind == "kiro" && s.cwd == removing.cwd);
    if has_sibling {
        Vec::new()
    } else {
        vec![PathBuf::from(&removing.cwd)
            .join(".kiro")
            .join("agents")
            .join("agent-orch.json")]
    }
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ── 4 · locked IO ──────────────────────────────────────────────────

/// Acquire an exclusive POSIX advisory lock on `state/sessions.lock`
/// for the duration of `f`. The lock file is created if absent.
/// Holders block (no timeout); contention is sub-millisecond at our
/// scale.
///
/// The `RwLockWriteGuard` returned by `fd-lock` releases the lock on
/// drop, so `f` is free to panic — the close-fd-releases-advisory-lock
/// path is the same on Unix.
pub fn with_lock<F, R>(state: &Path, f: F) -> Result<R>
where
    F: FnOnce() -> Result<R>,
{
    fs::create_dir_all(state).with_context(|| format!("mkdir -p {}", state.display()))?;
    let file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(lock_path(state))
        .with_context(|| format!("open {}", lock_path(state).display()))?;
    let mut lock = fd_lock::RwLock::new(file);
    let _guard = lock.write().context("flock")?;
    f()
}

// ── 5 · Env (testable entrypoint surface) ──────────────────────────

/// Everything a subcommand needs from the outside world. Production
/// constructs from process state (`Env::from_process`); tests
/// construct with an injected state-dir and Vec-backed stdout/stderr
/// (`Env::for_test`). Each handler takes `&mut Env` so behavior tests
/// can drive the same dispatch path the binary uses.
///
/// Kept small on purpose — only what slice 1's surface needs. Future
/// slices grow this struct (now-injection for clock-pinned tests,
/// stdin for `hook`, exec-spawner shim for `wrap`) as their tests
/// demand it.
pub struct Env {
    pub state_dir: PathBuf,
    pub stdout: Box<dyn Write>,
    pub stderr: Box<dyn Write>,
}

impl Env {
    pub fn from_process() -> Result<Self> {
        Ok(Env {
            state_dir: state_dir()?,
            stdout: Box::new(std::io::stdout()),
            stderr: Box::new(std::io::stderr()),
        })
    }
}

// ── 6 · subcommand handlers ────────────────────────────────────────

fn cmd_wrap(
    _env: &mut Env,
    _kind: String,
    _cwd: Option<PathBuf>,
    _agent_argv: Vec<String>,
) -> Result<i32> {
    anyhow::bail!("agent-orch wrap: not implemented yet (slice 2)")
}

fn cmd_hook(_env: &mut Env, _event: String) -> Result<i32> {
    anyhow::bail!("agent-orch hook: not implemented yet (slice 3)")
}

fn cmd_pick(_env: &mut Env) -> Result<i32> {
    anyhow::bail!("agent-orch pick: not implemented yet (slice 5)")
}

fn cmd_loop(_env: &mut Env) -> Result<i32> {
    anyhow::bail!("agent-orch loop: not implemented yet (slice 5)")
}

fn cmd_list(env: &mut Env) -> Result<i32> {
    let sessions = read_sessions(&sessions_path(&env.state_dir))?;
    if sessions.is_empty() {
        writeln!(env.stdout, "(no registered sessions)")?;
        return Ok(0);
    }
    let mut sessions = sessions;
    sort_sessions(&mut sessions);
    for s in &sessions {
        writeln!(env.stdout, "{}\t{}", s.pane_id, format_row(s))?;
    }
    Ok(0)
}

fn cmd_unregister(_env: &mut Env, _pane_id: String) -> Result<i32> {
    anyhow::bail!("agent-orch unregister: not implemented yet (slice 4)")
}

fn cmd_doctor(_env: &mut Env) -> Result<i32> {
    anyhow::bail!("agent-orch doctor: not implemented yet (slice 7)")
}

fn cmd_default(_env: &mut Env) -> Result<i32> {
    anyhow::bail!("agent-orch (bare): not implemented yet (slice 5)")
}

/// Behavior-test entrypoint. Drives the same clap dispatch the
/// binary uses, but with all I/O routed through `env`. Returns the
/// process-style exit code (0 = ok, non-zero = failure surfaced via
/// the handler's `Result`). On parse error, the error's message is
/// written to `env.stderr` and exit code 2 is returned.
pub fn run_command<I, S>(env: &mut Env, args: I) -> i32
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let argv: Vec<String> = std::iter::once("agent-orch".to_string())
        .chain(args.into_iter().map(Into::into))
        .collect();
    let cli = match Cli::try_parse_from(&argv) {
        Ok(c) => c,
        Err(e) => {
            let _ = writeln!(env.stderr, "{}", e);
            return 2;
        }
    };
    let result = match cli.cmd {
        None => cmd_default(env),
        Some(Cmd::Wrap {
            kind,
            cwd,
            agent_argv,
        }) => cmd_wrap(env, kind, cwd, agent_argv),
        Some(Cmd::Hook { event }) => cmd_hook(env, event),
        Some(Cmd::Pick) => cmd_pick(env),
        Some(Cmd::Loop) => cmd_loop(env),
        Some(Cmd::List) => cmd_list(env),
        Some(Cmd::Unregister { pane_id }) => cmd_unregister(env, pane_id),
        Some(Cmd::Doctor) => cmd_doctor(env),
    };
    match result {
        Ok(code) => code,
        Err(e) => {
            let _ = writeln!(env.stderr, "Error: {:#}", e);
            1
        }
    }
}

// ── 7 · clap dispatch ──────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "agent-orch",
    version,
    about = "Observation-only orchestrator for coding-agent tmux panes.",
    arg_required_else_help = false
)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Wrap a coding agent: register, inject hooks, execvp.
    Wrap {
        /// Agent kind: claude | kiro | other (registers only).
        kind: String,
        /// Override the recorded cwd (default: current dir).
        #[arg(long)]
        cwd: Option<PathBuf>,
        /// Agent argv after `--`.
        #[arg(last = true)]
        agent_argv: Vec<String>,
    },
    /// Hook reporter; called by Claude/Kiro on each lifecycle event.
    Hook {
        /// Event name (UserPromptSubmit, PreToolUse, PostToolUse, Stop).
        event: String,
    },
    /// Print one selected pane id from the registry to stdout.
    Pick,
    /// Picker loop (runs inside the orchestrator session).
    #[command(name = "loop")]
    Loop,
    /// Print the live registry as a table.
    List,
    /// Remove an entry from the registry; called by tmux pane-exited.
    Unregister { pane_id: String },
    /// Sanity-check tmux, fzf, agent CLIs, state dir.
    Doctor,
}

fn main() -> Result<()> {
    let mut env = Env::from_process()?;
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let code = run_command(&mut env, argv);
    std::process::exit(code);
}

// ── 8 · tests ──────────────────────────────────────────────────────
//
// Two-tier shape:
//
//   `mod behavior` drives `run_command` against a tempdir state-dir
//   and asserts what a user would observe — stdout, on-disk
//   sessions.json, side-effect files. These are the tests that
//   answer "does the system do the right thing?" and survive
//   refactors of the helpers underneath.
//
//   `mod helpers` is small. It carries only the pure-function
//   invariants the behavior tests can't reach (e.g. cwd_tail's
//   handling of root-anchored paths, kiro_orphan_paths'
//   creation-flag-agnostic rule). Each entry is justified by a
//   bug the behavior tests would have missed.
//
// Behavior tests for slices 2-5 (wrap, hook, unregister, pick) are
// authored as `#[ignore]`d placeholders so the discipline is
// visible from the start — those tests come alive as their slices
// land.

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    // ── shared fixtures ────────────────────────────────────────────

    fn mk_session(pane: &str, kind: &str, cwd: &str, started: u64) -> Session {
        Session {
            pane_id: pane.to_string(),
            pid: 12345,
            kind: kind.to_string(),
            cwd: cwd.to_string(),
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

    /// Build an `Env` whose state-dir is the given tempdir and whose
    /// stdout/stderr capture into `Vec<u8>`s the caller can read back.
    /// Returns `(env, stdout_buf, stderr_buf)` — the buffers are
    /// `Arc<Mutex<Vec<u8>>>` so the caller can drop the env (which
    /// closes the writers) and then read the captured bytes.
    fn test_env(state_dir: PathBuf) -> (Env, SharedBuf, SharedBuf) {
        let stdout = SharedBuf::new();
        let stderr = SharedBuf::new();
        let env = Env {
            state_dir,
            stdout: Box::new(stdout.clone()),
            stderr: Box::new(stderr.clone()),
        };
        (env, stdout, stderr)
    }

    /// Tiny `Write` impl backed by `Arc<Mutex<Vec<u8>>>` so tests can
    /// pull the captured bytes back out after `run_command` returns.
    #[derive(Clone)]
    struct SharedBuf(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl SharedBuf {
        fn new() -> Self {
            SharedBuf(std::sync::Arc::new(std::sync::Mutex::new(Vec::new())))
        }
        fn as_string(&self) -> String {
            String::from_utf8(self.0.lock().unwrap().clone()).expect("utf-8")
        }
    }

    impl Write for SharedBuf {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Seed a state-dir with a sessions.json containing the given
    /// records. Used by behavior tests to set up a registry shape
    /// without going through the (slice-2) `wrap` subcommand.
    fn seed_sessions(state_dir: &Path, sessions: &[Session]) {
        write_sessions_atomic(&sessions_path(state_dir), sessions).unwrap();
    }

    // ── behavior — exercise run_command end-to-end ─────────────────

    mod behavior {
        use super::*;

        // ---- list ----

        #[test]
        fn list_with_no_sessions_prints_marker() {
            let dir = tempdir().unwrap();
            let (mut env, stdout, _stderr) = test_env(dir.path().to_path_buf());
            let code = run_command(&mut env, ["list"]);
            drop(env); // flush
            assert_eq!(code, 0);
            assert_eq!(stdout.as_string(), "(no registered sessions)\n");
        }

        #[test]
        fn list_returns_sessions_after_seed() {
            let dir = tempdir().unwrap();
            let mut s = mk_session("%42", "claude", "/repo/foo", 1000);
            s.state = State::Running;
            s.last_prompt = "fix tests".to_string();
            seed_sessions(dir.path(), std::slice::from_ref(&s));

            let (mut env, stdout, _stderr) = test_env(dir.path().to_path_buf());
            let code = run_command(&mut env, ["list"]);
            drop(env);
            assert_eq!(code, 0);
            let out = stdout.as_string();
            assert!(out.contains("%42"), "stdout missing pane id: {:?}", out);
            assert!(out.contains("claude"), "stdout missing kind: {:?}", out);
            assert!(
                out.contains("fix tests"),
                "stdout missing prompt: {:?}",
                out
            );
        }

        #[test]
        fn list_orders_running_before_complete_before_unknown() {
            let dir = tempdir().unwrap();
            let mut a = mk_session("%a", "claude", "/x", 100);
            a.state = State::Complete;
            a.state_ts = 200;
            let mut b = mk_session("%b", "claude", "/y", 100);
            b.state = State::Running;
            b.state_ts = 300;
            let c = mk_session("%c", "kiro", "/z", 100); // unknown
            seed_sessions(dir.path(), &[a, b, c]);

            let (mut env, stdout, _stderr) = test_env(dir.path().to_path_buf());
            run_command(&mut env, ["list"]);
            drop(env);
            let out = stdout.as_string();
            let pos_b = out.find("%b").expect("running row missing");
            let pos_a = out.find("%a").expect("complete row missing");
            let pos_c = out.find("%c").expect("unknown row missing");
            assert!(pos_b < pos_a, "running must come before complete");
            assert!(pos_a < pos_c, "complete must come before unknown");
        }

        // ---- parse error path ----

        #[test]
        fn unknown_subcommand_writes_to_stderr_and_exits_2() {
            let dir = tempdir().unwrap();
            let (mut env, _stdout, stderr) = test_env(dir.path().to_path_buf());
            let code = run_command(&mut env, ["nope-not-a-subcommand"]);
            drop(env);
            assert_eq!(code, 2);
            assert!(
                !stderr.as_string().is_empty(),
                "expected clap error on stderr"
            );
        }

        // ---- handlers stubbed in slice 1; tests exit 1 with bail!() ----
        //
        // These verify the dispatch path reaches the right handler,
        // not the (yet-to-be-built) handler logic. They get rewritten
        // into real assertions when each slice lands.

        #[test]
        fn wrap_dispatch_reaches_handler_and_currently_bails() {
            let dir = tempdir().unwrap();
            let (mut env, _stdout, stderr) = test_env(dir.path().to_path_buf());
            let code = run_command(&mut env, ["wrap", "claude", "--", "echo"]);
            drop(env);
            assert_eq!(code, 1, "stub bails with Err");
            assert!(stderr.as_string().contains("slice 2"));
        }

        #[test]
        fn hook_dispatch_reaches_handler_and_currently_bails() {
            let dir = tempdir().unwrap();
            let (mut env, _stdout, stderr) = test_env(dir.path().to_path_buf());
            let code = run_command(&mut env, ["hook", "Stop"]);
            drop(env);
            assert_eq!(code, 1);
            assert!(stderr.as_string().contains("slice 3"));
        }

        // ---- placeholders for slice 2-5 behaviors ----
        //
        // These name the behavior tests that load-bear when their
        // slices land. They're #[ignore]'d so cargo test runs green
        // today; `cargo test -- --ignored` (or removing the attr in
        // the relevant slice) flips them on.

        #[test]
        #[ignore = "slice 2 — wrap appends a sessions.json record"]
        fn wrap_appends_session_record() {}

        #[test]
        #[ignore = "slice 2 — wrap installs the global tmux pane-exited hook"]
        fn wrap_installs_pane_exited_hook() {}

        #[test]
        #[ignore = "slice 2 — wrap synthesizes claude --settings tempfile"]
        fn wrap_synthesizes_claude_settings_with_user_base_merged() {}

        #[test]
        #[ignore = "slice 3 — hook UserPromptSubmit flips state to running and stores prompt"]
        fn hook_user_prompt_submit_marks_running_and_stores_prompt() {}

        #[test]
        #[ignore = "slice 3 — hook Stop flips state to complete"]
        fn hook_stop_marks_complete() {}

        #[test]
        #[ignore = "slice 3 — list reflects state changes after a hook fires"]
        fn list_reflects_hook_updates() {}

        #[test]
        #[ignore = "slice 4 — wrap then unregister: register/cleanup round-trip"]
        fn wrap_then_unregister_cleans_record_and_tempdir() {}

        #[test]
        #[ignore = "slice 4 — kiro reuser closing last removes shared .kiro/agents/agent-orch.json"]
        fn kiro_unregister_removes_orphan_when_creator_already_left() {}

        #[test]
        #[ignore = "slice 5 — pick prints selected pane id to stdout, exit 0"]
        fn pick_emits_selected_pane_id() {}
    }

    // ── helpers — invariants behavior tests can't see ──────────────
    //
    // Keep this list short. Each entry must justify itself by
    // catching a class of bug the behavior tests can't surface.

    mod helpers {
        use super::*;

        /// `cwd_tail("/repo")` once returned `"//repo"` because the
        /// `RootDir` `Component` joined as `"/"`. Behavior tests
        /// catch this only via the visible row, where the bad output
        /// hides as a cosmetic glitch. This pure test pins the rule.
        #[test]
        fn cwd_tail_handles_root_anchored_paths() {
            assert_eq!(cwd_tail("/home/me/repo/foo"), "repo/foo");
            assert_eq!(cwd_tail("/repo"), "repo");
            assert_eq!(cwd_tail(""), "");
        }

        /// The Kiro refcount cleanup rule is creation-flag-agnostic
        /// to avoid a leak: when a reuser closes last (its
        /// `created_kiro_config=false`), it must still remove the
        /// shared file. This test pins that rule directly because
        /// the slice-1 surface (`list`) doesn't exercise unregister.
        /// Once slice 4's `kiro_unregister_removes_orphan_when_
        /// creator_already_left` behavior test goes green, this
        /// helper test is redundant and can be deleted.
        #[test]
        fn kiro_orphan_creation_flag_agnostic() {
            let mut reuser = mk_session("%2", "kiro", "/repo", 2);
            reuser.created_kiro_config = false;
            let out = kiro_orphan_paths(&reuser, &[]);
            assert_eq!(
                out,
                vec![PathBuf::from("/repo/.kiro/agents/agent-orch.json")]
            );
        }
    }
}
