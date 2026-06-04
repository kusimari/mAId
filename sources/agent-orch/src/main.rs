//! agent-orch — observation-only orchestrator over tmux + coding-agent panes.
//!
//! Script-style: one file. Read top-to-bottom. Layout:
//!   1. Constants + paths.
//!   2. `Session` model + `apply_event` impl.
//!   3. Pure helpers: read/write sessions, format, sort, filter,
//!      kiro-orphan classification.
//!   4. Locked-IO helper (acquire flock, run a closure).
//!   5. `Cli` + `Env` + `run_command` + `main` — clap dispatch and
//!      the testable entrypoint. Handler bodies live inline in the
//!      `match` arms; extract a helper only when one grows past
//!      ~20 lines.
//!   6. `#[cfg(test)] mod tests` — split into `mod behavior`
//!      (drives `run_command`) and `mod helpers` (small, only
//!      what behavior tests can't reach).
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

fn tmp_dir_for_pane(state: &Path, pane_id: &str) -> PathBuf {
    state.join("tmp").join(pane_id)
}

fn hook_install_marker(state: &Path) -> PathBuf {
    state.join(".tmux-hook-installed")
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

/// Write the array atomically: write to `<path>.tmp.<pid>`, then `rename`.
/// Readers using `read_sessions` never see a partial write — `rename(2)`
/// is atomic on the same filesystem.
///
/// The tmp filename includes our pid so two concurrent writers don't
/// stomp each other's tmp file. They still race on the rename (last
/// rename wins), but each writer's content is intact through its own
/// rename — no torn JSON. Callers who need read-modify-write atomicity
/// (the common case for the hook subcommand) hold `with_lock` while
/// reading, mutating, and calling this.
///
/// No `fsync` — state-dir scratch; loss-after-power-failure is fine
/// (the registry rebuilds on the next launch + pane-exited sweep).
pub fn write_sessions_atomic(path: &Path, sessions: &[Session]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("mkdir -p {}", parent.display()))?;
    }
    let tmp = path.with_extension(format!("json.tmp.{}", std::process::id()));
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

// ── 5 · clap dispatch + run_command ────────────────────────────────

#[derive(Parser)]
#[command(
    name = "agent-orch",
    version,
    about = "Observation-only orchestrator for coding-agent tmux panes."
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

/// Everything a subcommand needs from the outside world. The binary
/// builds this from real process state in `main`; tests build it with
/// a tempdir state-dir and `Vec<u8>`-backed stdout / stderr (`Vec<u8>`
/// already implements `Write`). Borrowed lifetimes keep the test path
/// zero-allocation.
///
/// Each field is the simplest type that captures one thing — no
/// traits, no closures. Tests inject by setting fields directly.
pub struct Env<'a> {
    pub state_dir: PathBuf,
    pub stdout: &'a mut dyn Write,
    pub stderr: &'a mut dyn Write,
    /// `$TMUX_PANE` (`%N`) when running inside tmux, else None.
    pub pane_id: Option<String>,
    /// Path to the user's `~/.claude/settings.json` (may not exist).
    /// Used as the merge base when synthesizing per-launch settings.
    pub user_claude_settings: PathBuf,
    /// Path to this binary itself, embedded into hook commands so
    /// the agent re-invokes us via the same path it was launched
    /// from. `current_exe()` in production; the dist-path in tests.
    pub self_path: PathBuf,
    /// When false, `wrap` skips the tmux side effects (`set-hook`,
    /// `set-option`) and the terminal `execvp`. Tests verify the
    /// on-disk side effects (settings file, sessions.json append)
    /// which are the meaningful work; the unobservable bits are
    /// covered by smoke (slice 6).
    pub side_effects_enabled: bool,
}

/// Merge our four Claude hook commands into the given settings JSON.
/// Each event's hook list gets one new entry of the form
/// `{"type":"command", "command":"<self> hook <Event>"}` appended;
/// the user's existing entries are preserved in front of ours.
///
/// Errors if the settings root or the `hooks` field isn't a JSON
/// object — silently dropping our hooks would leave the user with
/// a hookless wrapper that never updates state.
fn merge_claude_hooks(settings: &mut serde_json::Value, self_path: &Path) -> Result<()> {
    use serde_json::{json, Value};
    let self_str = self_path.to_string_lossy();
    let root = settings
        .as_object_mut()
        .context("user claude settings root must be a JSON object")?;
    let hooks_val = root.entry("hooks".to_string()).or_insert_with(|| json!({}));
    let hooks = hooks_val
        .as_object_mut()
        .context("user claude settings.hooks must be a JSON object")?;
    for event in [
        EVT_USER_PROMPT_SUBMIT,
        EVT_PRE_TOOL_USE,
        EVT_POST_TOOL_USE,
        EVT_STOP,
    ] {
        let arr = hooks.entry(event.to_string()).or_insert_with(|| json!([]));
        let Value::Array(list) = arr else {
            anyhow::bail!("user claude settings.hooks.{} must be an array", event);
        };
        list.push(json!({
            "type": "command",
            "command": format!("{} hook {}", self_str, event),
        }));
    }
    Ok(())
}

/// Build the argv we hand to `execvp` for a wrapped agent. Returns
/// `(program, argv)` where `argv[0]` is the program name (POSIX
/// convention — child sees this as its own `argv[0]`).
///
/// For `claude`, splices `--settings <path>` after `argv[0]` so the
/// agent's launcher sees its own name in slot 0 and its own flags
/// in subsequent slots. For other kinds, `agent_argv` is passed
/// through unchanged.
///
/// # Panics
/// Panics if `agent_argv` is empty. Callers must guard upstream
/// (the wrap match arm uses `anyhow::ensure!` before reaching here).
fn build_agent_argv(
    kind: &str,
    agent_argv: &[String],
    settings_path: &Path,
) -> (String, Vec<String>) {
    let program = agent_argv[0].clone();
    let mut argv = agent_argv.to_vec();
    if kind == "claude" {
        argv.insert(1, "--settings".into());
        argv.insert(2, settings_path.to_string_lossy().into_owned());
    }
    (program, argv)
}

/// Install the global tmux `pane-exited` hook pointing at this binary.
/// Idempotent via a marker file under the state dir — calling this
/// many times across many wrappers does the work exactly once per
/// state-dir.
///
/// Race-tolerant: tmux `set-hook -g` is itself idempotent (last
/// caller wins, both write the same command), and the marker file
/// has no content. Two wrappers starting at the same time can both
/// pass `marker.exists() == false` and both run the set-hook — the
/// outcome is identical to either running alone.
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
    // state_dir already exists — with_lock has run by the time we
    // reach this function from the wrap arm.
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

/// `execvp` into the agent. Returns only on failure (binary not
/// found, ENOEXEC, etc.); on success the kernel replaces this
/// process image so control never returns.
fn exec_agent(program: &str, argv: &[String]) -> Result<()> {
    use std::ffi::CString;
    let prog_c = CString::new(program).context("nul in agent program name")?;
    let argv_c: Vec<CString> = argv
        .iter()
        .map(|a| CString::new(a.as_str()))
        .collect::<std::result::Result<_, _>>()
        .context("nul in agent argv")?;
    let argv_refs: Vec<&std::ffi::CStr> = argv_c.iter().map(|c| c.as_c_str()).collect();
    nix::unistd::execvp(&prog_c, &argv_refs).with_context(|| format!("execvp {}", program))?;
    unreachable!("execvp returned Ok without exec")
}

/// Behavior-test entrypoint. Drives the same clap dispatch as `main`
/// but routes all I/O through `env`. Returns the process exit code
/// (0 ok, 1 handler-error, 2 parse-error). Handler bodies live inline
/// in the match arms — slices fill them in as they land. Extract a
/// helper only when a body grows past ~20 lines.
pub fn run_command<I, S>(env: &mut Env, args: I) -> i32
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let argv = std::iter::once("agent-orch".to_string())
        .chain(args.into_iter().map(Into::into))
        .collect::<Vec<_>>();
    let cli = match Cli::try_parse_from(&argv) {
        Ok(c) => c,
        Err(e) => {
            let _ = writeln!(env.stderr, "{}", e);
            return 2;
        }
    };
    let result: Result<i32> = (|| match cli.cmd {
        None => anyhow::bail!("agent-orch (bare): not implemented yet (slice 5)"),
        Some(Cmd::Wrap {
            kind,
            cwd,
            agent_argv,
        }) => {
            let pane_id = env
                .pane_id
                .clone()
                .context("agent-orch wrap requires $TMUX_PANE — run inside tmux")?;
            anyhow::ensure!(
                !agent_argv.is_empty(),
                "agent-orch wrap needs an agent command after `--`"
            );

            // Side effect 1 — register the session under flock. We
            // do this *first* so a double-register fails before any
            // disk work; the recorded `pid` is our own (after
            // `execvp` the kernel keeps the same pid, now the agent).
            let cwd_string = match cwd {
                Some(p) => p.to_string_lossy().into_owned(),
                None => std::env::current_dir()
                    .context("current_dir for session record")?
                    .to_string_lossy()
                    .into_owned(),
            };
            let now = now_secs();
            let new_session = Session {
                pane_id: pane_id.clone(),
                pid: std::process::id() as i32,
                kind: kind.clone(),
                cwd: cwd_string,
                started: now,
                state: State::Unknown,
                state_ts: now,
                last_prompt: String::new(),
                last_tool: String::new(),
                last_event: String::new(),
                last_event_ts: 0,
                created_kiro_config: false,
            };
            with_lock(&env.state_dir, || {
                let path = sessions_path(&env.state_dir);
                let mut sessions = read_sessions(&path)?;
                anyhow::ensure!(
                    !sessions.iter().any(|s| s.pane_id == pane_id),
                    "pane {} already registered — `agent-orch unregister {}` first",
                    pane_id,
                    pane_id
                );
                sessions.push(new_session);
                write_sessions_atomic(&path, &sessions)?;
                Ok(())
            })?;

            // Side effect 2 — synthesize per-launch claude settings.
            // Merges the user's existing ~/.claude/settings.json (if
            // any) with our four hook commands. The settings dir is
            // GC'd by `unregister` regardless of how this wrapper
            // exits, so a crash between here and execvp leaves no
            // orphan that the registry doesn't know about.
            let settings_dir = tmp_dir_for_pane(&env.state_dir, &pane_id);
            fs::create_dir_all(&settings_dir)
                .with_context(|| format!("mkdir -p {}", settings_dir.display()))?;
            let settings_path = settings_dir.join("settings.json");
            let mut settings: serde_json::Value = if env.user_claude_settings.exists() {
                serde_json::from_slice(&fs::read(&env.user_claude_settings)?)
                    .with_context(|| format!("parse {}", env.user_claude_settings.display()))?
            } else {
                serde_json::json!({})
            };
            merge_claude_hooks(&mut settings, &env.self_path)?;
            fs::write(&settings_path, serde_json::to_vec_pretty(&settings)?)?;

            // Side effect 3 — install the global tmux pane-exited
            // hook (idempotent via marker file) and tag this pane
            // with @agent-orch-pane. Skipped under tests.
            //
            // Side effect 4 — execvp into the agent. From here, this
            // process *is* the agent. `build_agent_argv` puts the
            // program name in argv[0] (POSIX convention) and splices
            // our flags after it.
            if env.side_effects_enabled {
                install_pane_exited_hook(&env.state_dir, &env.self_path)?;
                tmux_set_pane_option(&pane_id, "@agent-orch-pane", &pane_id)?;
                let (program, argv) = build_agent_argv(&kind, &agent_argv, &settings_path);
                // SAFETY: single-threaded immediately before execvp;
                // the env var only needs to reach the about-to-be-
                // replaced process image.
                std::env::set_var("AGENT_ORCH_PANE", &pane_id);
                exec_agent(&program, &argv)?;
            }
            Ok(0)
        }
        Some(Cmd::Hook { .. }) => anyhow::bail!("agent-orch hook: not implemented yet (slice 3)"),
        Some(Cmd::Pick) => anyhow::bail!("agent-orch pick: not implemented yet (slice 5)"),
        Some(Cmd::Loop) => anyhow::bail!("agent-orch loop: not implemented yet (slice 5)"),
        Some(Cmd::Unregister { .. }) => {
            anyhow::bail!("agent-orch unregister: not implemented yet (slice 4)")
        }
        Some(Cmd::Doctor) => anyhow::bail!("agent-orch doctor: not implemented yet (slice 7)"),
        Some(Cmd::List) => {
            let mut sessions = read_sessions(&sessions_path(&env.state_dir))?;
            if sessions.is_empty() {
                writeln!(env.stdout, "(no registered sessions)")?;
                return Ok(0);
            }
            sort_sessions(&mut sessions);
            for s in &sessions {
                writeln!(env.stdout, "{}\t{}", s.pane_id, format_row(s))?;
            }
            Ok(0)
        }
    })();
    match result {
        Ok(code) => code,
        Err(e) => {
            let _ = writeln!(env.stderr, "Error: {:#}", e);
            1
        }
    }
}

fn main() -> Result<()> {
    let state = state_dir()?;
    let pane_id = std::env::var("TMUX_PANE").ok().filter(|s| !s.is_empty());
    let user_claude_settings = std::env::var("HOME")
        .map(|h| PathBuf::from(h).join(".claude/settings.json"))
        .unwrap_or_default();
    let self_path = std::env::current_exe().context("current_exe")?;
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    let mut env = Env {
        state_dir: state,
        stdout: &mut stdout,
        stderr: &mut stderr,
        pane_id,
        user_claude_settings,
        self_path,
        side_effects_enabled: true,
    };
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let code = run_command(&mut env, argv);
    std::process::exit(code);
}

// ── 6 · tests ──────────────────────────────────────────────────────
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
    use tempfile::{tempdir, TempDir};

    // 1 · session stub ────────────────────────────────────────────────

    fn mk_session(pane: &str, kind: &str, cwd: &str, started: u64) -> Session {
        Session {
            pane_id: pane.into(),
            pid: 12345,
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

    // 2 · env stub ────────────────────────────────────────────────────
    //
    // `Vec<u8>` already implements `Write`, so capture is one allocation
    // each. The TempDir is returned alongside so the caller's scope
    // keeps it alive for the duration of the test.

    fn fixtures() -> (TempDir, Vec<u8>, Vec<u8>) {
        (tempdir().unwrap(), Vec::new(), Vec::new())
    }

    // 3 · command-runner wiring ───────────────────────────────────────
    //
    // `drive` is the simple form for tests that don't care about
    // wrap-specific Env fields (everything but pane_id and the
    // settings/self paths). `drive_with` lets a wrap-flavored test
    // override pane_id and user_claude_settings; both share the same
    // run_command call so behavior is identical.

    fn drive(state: &Path, stdout: &mut Vec<u8>, stderr: &mut Vec<u8>, args: &[&str]) -> i32 {
        drive_with(state, None, None, stdout, stderr, args)
    }

    fn drive_with(
        state: &Path,
        pane_id: Option<&str>,
        user_claude_settings: Option<&Path>,
        stdout: &mut Vec<u8>,
        stderr: &mut Vec<u8>,
        args: &[&str],
    ) -> i32 {
        let mut env = Env {
            state_dir: state.to_path_buf(),
            stdout,
            stderr,
            pane_id: pane_id.map(String::from),
            user_claude_settings: user_claude_settings
                .map(Path::to_path_buf)
                .unwrap_or_else(|| state.join("nonexistent-user-settings.json")),
            self_path: PathBuf::from("/test/agent-orch"),
            side_effects_enabled: false,
        };
        run_command(&mut env, args.iter().map(|s| s.to_string()))
    }

    fn seed(state: &Path, sessions: &[Session]) {
        write_sessions_atomic(&sessions_path(state), sessions).unwrap();
    }

    // 4 · behavior — round-trips through run_command ──────────────────

    mod behavior {
        use super::*;

        #[test]
        fn list_with_no_sessions_prints_marker() {
            let (dir, mut so, mut se) = fixtures();
            let code = drive(dir.path(), &mut so, &mut se, &["list"]);
            assert_eq!(code, 0);
            assert_eq!(String::from_utf8(so).unwrap(), "(no registered sessions)\n");
        }

        #[test]
        fn list_returns_sessions_after_seed() {
            let (dir, mut so, mut se) = fixtures();
            let mut s = mk_session("%42", "claude", "/repo/foo", 1000);
            s.state = State::Running;
            s.last_prompt = "fix tests".into();
            seed(dir.path(), std::slice::from_ref(&s));

            let code = drive(dir.path(), &mut so, &mut se, &["list"]);
            let out = String::from_utf8(so).unwrap();
            assert_eq!(code, 0);
            assert!(out.contains("%42"), "{out:?}");
            assert!(out.contains("claude"), "{out:?}");
            assert!(out.contains("fix tests"), "{out:?}");
        }

        #[test]
        fn list_orders_running_before_complete_before_unknown() {
            let (dir, mut so, mut se) = fixtures();
            let mut a = mk_session("%a", "claude", "/x", 100);
            a.state = State::Complete;
            a.state_ts = 200;
            let mut b = mk_session("%b", "claude", "/y", 100);
            b.state = State::Running;
            b.state_ts = 300;
            let c = mk_session("%c", "kiro", "/z", 100); // unknown
            seed(dir.path(), &[a, b, c]);

            drive(dir.path(), &mut so, &mut se, &["list"]);
            let out = String::from_utf8(so).unwrap();
            let pb = out.find("%b").unwrap();
            let pa = out.find("%a").unwrap();
            let pc = out.find("%c").unwrap();
            assert!(pb < pa && pa < pc, "wrong order in:\n{out}");
        }

        #[test]
        fn unknown_subcommand_exits_2_with_stderr() {
            let (dir, mut so, mut se) = fixtures();
            let code = drive(dir.path(), &mut so, &mut se, &["nope"]);
            assert_eq!(code, 2);
            assert!(!se.is_empty());
        }

        // Stubs that still bail!() (slices 3-5/7) verify dispatch
        // reaches the right arm. Each becomes a real behavior test
        // when its slice lands.

        #[test]
        fn hook_dispatch_currently_bails_with_slice_3_marker() {
            let (dir, mut so, mut se) = fixtures();
            let code = drive(dir.path(), &mut so, &mut se, &["hook", "Stop"]);
            assert_eq!(code, 1);
            assert!(String::from_utf8_lossy(&se).contains("slice 3"));
        }

        // ---- slice 2 — wrap claude ----

        #[test]
        fn wrap_without_tmux_pane_fails_loud() {
            let (dir, mut so, mut se) = fixtures();
            let code = drive_with(
                dir.path(),
                None, // no pane id
                None,
                &mut so,
                &mut se,
                &["wrap", "claude", "--", "echo", "hi"],
            );
            assert_eq!(code, 1);
            assert!(
                String::from_utf8_lossy(&se).contains("$TMUX_PANE"),
                "stderr: {}",
                String::from_utf8_lossy(&se)
            );
        }

        #[test]
        fn wrap_appends_session_record() {
            let (dir, mut so, mut se) = fixtures();
            let code = drive_with(
                dir.path(),
                Some("%42"),
                None,
                &mut so,
                &mut se,
                &["wrap", "claude", "--cwd", "/repo/foo", "--", "claude-stub"],
            );
            assert_eq!(code, 0, "stderr: {}", String::from_utf8_lossy(&se));
            let sessions = read_sessions(&sessions_path(dir.path())).unwrap();
            assert_eq!(sessions.len(), 1);
            assert_eq!(sessions[0].pane_id, "%42");
            assert_eq!(sessions[0].kind, "claude");
            assert_eq!(sessions[0].cwd, "/repo/foo");
            assert_eq!(sessions[0].pid, std::process::id() as i32);
            assert_eq!(sessions[0].state, State::Unknown);
        }

        #[test]
        fn wrap_refuses_double_register() {
            let (dir, mut so, mut se) = fixtures();
            let c1 = drive_with(
                dir.path(),
                Some("%9"),
                None,
                &mut so,
                &mut se,
                &["wrap", "claude", "--", "stub"],
            );
            assert_eq!(c1, 0);
            so.clear();
            se.clear();
            let c2 = drive_with(
                dir.path(),
                Some("%9"),
                None,
                &mut so,
                &mut se,
                &["wrap", "claude", "--", "stub"],
            );
            assert_eq!(c2, 1);
            assert!(String::from_utf8_lossy(&se).contains("already registered"));
        }

        #[test]
        fn wrap_synthesizes_claude_settings_with_user_base_merged() {
            let (dir, mut so, mut se) = fixtures();
            let user = dir.path().join("user-claude-settings.json");
            fs::write(
                &user,
                br#"{"hooks":{"UserPromptSubmit":[{"type":"command","command":"my-existing-hook"}]}}"#,
            )
            .unwrap();
            let code = drive_with(
                dir.path(),
                Some("%7"),
                Some(&user),
                &mut so,
                &mut se,
                &["wrap", "claude", "--", "stub"],
            );
            assert_eq!(code, 0, "stderr: {}", String::from_utf8_lossy(&se));
            let synth_path = tmp_dir_for_pane(dir.path(), "%7").join("settings.json");
            let synth: serde_json::Value =
                serde_json::from_slice(&fs::read(&synth_path).unwrap()).unwrap();
            // User's existing hook still in front.
            let ups = &synth["hooks"]["UserPromptSubmit"];
            assert_eq!(ups[0]["command"], "my-existing-hook");
            assert_eq!(ups[1]["command"], "/test/agent-orch hook UserPromptSubmit");
            // All four events wired with our hook.
            for ev in [EVT_PRE_TOOL_USE, EVT_POST_TOOL_USE, EVT_STOP] {
                let arr = &synth["hooks"][ev];
                assert!(arr.is_array(), "{ev} not an array");
                assert_eq!(arr[0]["command"], format!("/test/agent-orch hook {}", ev));
            }
        }

        #[test]
        fn wrap_synthesizes_claude_settings_when_user_settings_missing() {
            let (dir, mut so, mut se) = fixtures();
            let code = drive_with(
                dir.path(),
                Some("%5"),
                None,
                &mut so,
                &mut se,
                &["wrap", "claude", "--", "stub"],
            );
            assert_eq!(code, 0);
            let synth: serde_json::Value = serde_json::from_slice(
                &fs::read(tmp_dir_for_pane(dir.path(), "%5").join("settings.json")).unwrap(),
            )
            .unwrap();
            for ev in [
                EVT_USER_PROMPT_SUBMIT,
                EVT_PRE_TOOL_USE,
                EVT_POST_TOOL_USE,
                EVT_STOP,
            ] {
                let arr = &synth["hooks"][ev];
                assert_eq!(arr.as_array().unwrap().len(), 1, "{ev} entries");
            }
        }

        #[test]
        fn wrap_rejects_non_object_user_settings_root() {
            let (dir, mut so, mut se) = fixtures();
            // A stray array at the root — silent-drop would leave the
            // synth file with no hooks; we want a loud failure.
            let user = dir.path().join("user-claude-settings.json");
            fs::write(&user, b"[]").unwrap();
            let code = drive_with(
                dir.path(),
                Some("%3"),
                Some(&user),
                &mut so,
                &mut se,
                &["wrap", "claude", "--", "stub"],
            );
            assert_eq!(code, 1);
            assert!(
                String::from_utf8_lossy(&se).contains("must be a JSON object"),
                "stderr: {}",
                String::from_utf8_lossy(&se)
            );
            // Pinned partial-progress: registration runs *before*
            // settings synth, so the failed wrap leaves the session
            // record in place. The user (or slice 4 unregister) must
            // clear it manually. If we ever flip to rollback-on-error,
            // change this assertion to `assert!(sessions.is_empty())`.
            let sessions = read_sessions(&sessions_path(dir.path())).unwrap();
            assert_eq!(
                sessions.len(),
                1,
                "registration runs before synth — entry stays on synth failure"
            );
        }

        #[test]
        fn wrap_rejects_non_array_hooks_event() {
            let (dir, mut so, mut se) = fixtures();
            let user = dir.path().join("user.json");
            fs::write(&user, br#"{"hooks":{"Stop":"oops-a-string"}}"#).unwrap();
            let code = drive_with(
                dir.path(),
                Some("%4"),
                Some(&user),
                &mut so,
                &mut se,
                &["wrap", "claude", "--", "stub"],
            );
            assert_eq!(code, 1);
            assert!(
                String::from_utf8_lossy(&se).contains("must be an array"),
                "stderr: {}",
                String::from_utf8_lossy(&se)
            );
        }

        #[test]
        #[ignore = "slice 2 — wrap installs global tmux pane-exited hook (smoke covers; behavior would shell out to tmux)"]
        fn wrap_installs_pane_exited_hook() {}

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

    // helpers — short, justify each entry by a class of bug behavior
    // tests can't catch.

    mod helpers {
        use super::*;

        // `cwd_tail("/repo")` once returned `"//repo"` because the
        // `RootDir` `Component` joined as `"/"`. Behavior tests would
        // surface this only as a cosmetic glitch in the visible row.
        #[test]
        fn cwd_tail_handles_root_anchored_paths() {
            assert_eq!(cwd_tail("/home/me/repo/foo"), "repo/foo");
            assert_eq!(cwd_tail("/repo"), "repo");
            assert_eq!(cwd_tail(""), "");
        }

        // Kiro refcount cleanup is creation-flag-agnostic: a reuser
        // closing last (its `created_kiro_config=false`) must still
        // remove the shared file. Pinned here because slice 1 has no
        // unregister surface; deletable once slice 4 lands its
        // behavior test.
        #[test]
        fn kiro_orphan_creation_flag_agnostic() {
            let mut reuser = mk_session("%2", "kiro", "/repo", 2);
            reuser.created_kiro_config = false;
            assert_eq!(
                kiro_orphan_paths(&reuser, &[]),
                vec![PathBuf::from("/repo/.kiro/agents/agent-orch.json")]
            );
        }

        // build_agent_argv pins the POSIX-style argv we hand to
        // `execvp`. Behavior tests gate on `side_effects_enabled =
        // false` and never fire `exec_agent`, so the construction
        // is pinned here. argv[0] must be the program name (the
        // child process sees this as its own argv[0]); claude flags
        // splice in *after* slot 0, never replace it.
        #[test]
        fn build_agent_argv_claude_splices_settings_after_slot0() {
            let argv_in = vec!["claude".to_string(), "--resume".into(), "my-sess".into()];
            let (program, argv) = build_agent_argv("claude", &argv_in, Path::new("/tmp/s.json"));
            assert_eq!(program, "claude");
            assert_eq!(
                argv,
                vec!["claude", "--settings", "/tmp/s.json", "--resume", "my-sess"]
            );
        }

        #[test]
        fn build_agent_argv_non_claude_passes_through() {
            let argv_in = vec!["kiro".to_string(), "chat".into()];
            let (program, argv) = build_agent_argv("kiro", &argv_in, Path::new("/tmp/s.json"));
            assert_eq!(program, "kiro");
            assert_eq!(argv, vec!["kiro", "chat"]);
        }

        #[test]
        fn build_agent_argv_claude_no_extra_args() {
            let argv_in = vec!["claude".to_string()];
            let (_program, argv) = build_agent_argv("claude", &argv_in, Path::new("/tmp/s.json"));
            assert_eq!(argv, vec!["claude", "--settings", "/tmp/s.json"]);
        }
    }
}
