//! agent-orch — observation-only orchestrator over tmux + coding-agent panes.
//!
//! Script-style: one file. Read top-to-bottom. Layout:
//!   1. Constants + paths.
//!   2. `Session` model + `apply_event` impl.
//!   3. Pure helpers: read/write sessions, format, sort, filter,
//!      kiro-orphan classification.
//!   4. Locked-IO helpers (acquire flock, run a closure, atomic write).
//!   5. Subcommand handlers (`cmd_wrap`, `cmd_hook`, ... — all stubs in
//!      slice 1, filled in subsequent slices).
//!   6. `clap` dispatch in `main`.
//!   7. `#[cfg(test)] mod tests` at the bottom.
//!
//! Splitting this file is a v2 concern (soft threshold ~1000 LOC).

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::fs;
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

// ── 5 · subcommand handlers (stubs in slice 1) ─────────────────────

fn cmd_wrap(_kind: String, _cwd: Option<PathBuf>, _agent_argv: Vec<String>) -> Result<()> {
    anyhow::bail!("agent-orch wrap: not implemented yet (slice 2)")
}

fn cmd_hook(_event: String) -> Result<()> {
    anyhow::bail!("agent-orch hook: not implemented yet (slice 3)")
}

fn cmd_pick() -> Result<()> {
    anyhow::bail!("agent-orch pick: not implemented yet (slice 5)")
}

fn cmd_loop() -> Result<()> {
    anyhow::bail!("agent-orch loop: not implemented yet (slice 5)")
}

fn cmd_list() -> Result<()> {
    let state = state_dir()?;
    let sessions = read_sessions(&sessions_path(&state))?;
    if sessions.is_empty() {
        println!("(no registered sessions)");
        return Ok(());
    }
    let mut sessions = sessions;
    sort_sessions(&mut sessions);
    for s in &sessions {
        println!("{}\t{}", s.pane_id, format_row(s));
    }
    Ok(())
}

fn cmd_unregister(_pane_id: String) -> Result<()> {
    anyhow::bail!("agent-orch unregister: not implemented yet (slice 4)")
}

fn cmd_doctor() -> Result<()> {
    anyhow::bail!("agent-orch doctor: not implemented yet (slice 7)")
}

fn cmd_default() -> Result<()> {
    anyhow::bail!("agent-orch (bare): not implemented yet (slice 5)")
}

// ── 6 · clap dispatch ──────────────────────────────────────────────

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
    let cli = Cli::parse();
    match cli.cmd {
        None => cmd_default(),
        Some(Cmd::Wrap {
            kind,
            cwd,
            agent_argv,
        }) => cmd_wrap(kind, cwd, agent_argv),
        Some(Cmd::Hook { event }) => cmd_hook(event),
        Some(Cmd::Pick) => cmd_pick(),
        Some(Cmd::Loop) => cmd_loop(),
        Some(Cmd::List) => cmd_list(),
        Some(Cmd::Unregister { pane_id }) => cmd_unregister(pane_id),
        Some(Cmd::Doctor) => cmd_doctor(),
    }
}

// ── 7 · tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

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

    // ── apply_event ────────────────────────────────────────────────

    #[test]
    fn apply_user_prompt_submit_sets_running_and_prompt() {
        let mut s = mk_session("%1", "claude", "/repo", 1000);
        let changed = s.apply_event(EVT_USER_PROMPT_SUBMIT, Some("hello world"), None, 1100);
        assert!(changed);
        assert_eq!(s.state, State::Running);
        assert_eq!(s.state_ts, 1100);
        assert_eq!(s.last_prompt, "hello world");
        assert_eq!(s.last_event, EVT_USER_PROMPT_SUBMIT);
        assert_eq!(s.last_event_ts, 1100);
    }

    #[test]
    fn apply_user_prompt_submit_truncates_to_80_chars() {
        let mut s = mk_session("%1", "claude", "/repo", 1000);
        let long = "a".repeat(120);
        s.apply_event(EVT_USER_PROMPT_SUBMIT, Some(&long), None, 1100);
        assert_eq!(s.last_prompt.chars().count(), 80);
    }

    #[test]
    fn apply_pre_tool_use_sets_running_and_tool() {
        let mut s = mk_session("%1", "claude", "/repo", 1000);
        s.apply_event(EVT_PRE_TOOL_USE, None, Some("Bash"), 1100);
        assert_eq!(s.state, State::Running);
        assert_eq!(s.last_tool, "Bash");
    }

    #[test]
    fn apply_post_tool_use_keeps_state_refreshes_tool() {
        let mut s = mk_session("%1", "claude", "/repo", 1000);
        s.state = State::Running;
        s.state_ts = 1100;
        s.apply_event(EVT_POST_TOOL_USE, None, Some("Edit"), 1200);
        assert_eq!(s.state, State::Running, "state must not change");
        assert_eq!(s.state_ts, 1100, "state_ts must not change on PostToolUse");
        assert_eq!(s.last_tool, "Edit");
    }

    #[test]
    fn apply_stop_sets_complete() {
        let mut s = mk_session("%1", "claude", "/repo", 1000);
        s.state = State::Running;
        s.apply_event(EVT_STOP, None, None, 1300);
        assert_eq!(s.state, State::Complete);
        assert_eq!(s.state_ts, 1300);
    }

    #[test]
    fn apply_unknown_event_records_event_only() {
        let mut s = mk_session("%1", "claude", "/repo", 1000);
        let changed = s.apply_event("Notification", None, None, 1500);
        // last_event_ts bump → bool is true even for unknown events.
        // Documented as a write-elision hint, not a contract.
        assert!(changed);
        assert_eq!(s.state, State::Unknown);
        assert_eq!(s.last_event, "Notification");
        assert_eq!(s.last_event_ts, 1500);
    }

    // ── read_sessions / write_sessions_atomic ──────────────────────

    #[test]
    fn read_missing_file_returns_empty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("does-not-exist.json");
        assert_eq!(read_sessions(&path).unwrap(), vec![]);
    }

    #[test]
    fn read_empty_file_returns_empty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("empty.json");
        fs::write(&path, "").unwrap();
        assert_eq!(read_sessions(&path).unwrap(), vec![]);
    }

    #[test]
    fn write_then_read_round_trips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("sessions.json");
        let s = mk_session("%42", "claude", "/repo/foo", 1000);
        write_sessions_atomic(&path, std::slice::from_ref(&s)).unwrap();
        let read = read_sessions(&path).unwrap();
        assert_eq!(read, vec![s]);
    }

    #[test]
    fn write_creates_parent_dir() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested/dir/sessions.json");
        let s = mk_session("%1", "claude", "/x", 1);
        write_sessions_atomic(&path, std::slice::from_ref(&s)).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn write_atomic_uses_tmp_then_rename() {
        // Verify the .tmp file does not linger after a successful write.
        let dir = tempdir().unwrap();
        let path = dir.path().join("sessions.json");
        let s = mk_session("%1", "claude", "/x", 1);
        write_sessions_atomic(&path, std::slice::from_ref(&s)).unwrap();
        assert!(path.exists());
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn read_malformed_errors() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bad.json");
        fs::write(&path, "not json").unwrap();
        assert!(read_sessions(&path).is_err());
    }

    // ── format_row ─────────────────────────────────────────────────

    #[test]
    fn format_row_running_with_tool() {
        let mut s = mk_session("%1", "claude", "/home/me/repo/foo", 1000);
        s.state = State::Running;
        s.last_prompt = "fix tests".to_string();
        s.last_tool = "Bash".to_string();
        let row = format_row(&s);
        assert!(row.contains('▶'));
        assert!(row.contains("claude"));
        assert!(row.contains("repo/foo"));
        assert!(row.contains("fix tests"));
        assert!(row.contains("Bash"));
    }

    #[test]
    fn format_row_complete_no_tool_no_prompt() {
        let mut s = mk_session("%1", "kiro", "/x", 1000);
        s.state = State::Complete;
        let row = format_row(&s);
        assert!(row.contains('✓'));
        assert!(row.contains("kiro"));
        assert!(!row.ends_with(" · "), "trailing separator when tool empty");
    }

    #[test]
    fn format_row_unknown_glyph() {
        let s = mk_session("%1", "claude", "/x", 1000);
        let row = format_row(&s);
        assert!(row.contains('·'), "unknown state glyph");
    }

    #[test]
    fn cwd_tail_takes_last_two_components() {
        assert_eq!(cwd_tail("/home/me/repo/foo"), "repo/foo");
        assert_eq!(cwd_tail("/repo"), "repo");
        assert_eq!(cwd_tail(""), "");
    }

    // ── sort_sessions ──────────────────────────────────────────────

    #[test]
    fn sort_by_state_group_then_recency() {
        let mut a = mk_session("%a", "claude", "/x", 100);
        a.state = State::Complete;
        a.state_ts = 200;

        let mut b = mk_session("%b", "claude", "/y", 100);
        b.state = State::Running;
        b.state_ts = 150;

        let mut c = mk_session("%c", "claude", "/z", 100);
        c.state = State::Running;
        c.state_ts = 300;

        let mut d = mk_session("%d", "kiro", "/w", 100);
        d.state = State::Unknown;

        let mut v = vec![a, b, c, d];
        sort_sessions(&mut v);
        let pane_order: Vec<&str> = v.iter().map(|s| s.pane_id.as_str()).collect();
        // running (most-recent first), then complete, then unknown
        assert_eq!(pane_order, vec!["%c", "%b", "%a", "%d"]);
    }

    // ── live_filter ────────────────────────────────────────────────

    #[test]
    fn live_filter_drops_dead_pids() {
        let mut s1 = mk_session("%1", "claude", "/x", 1);
        s1.pid = 1001;
        let mut s2 = mk_session("%2", "claude", "/y", 1);
        s2.pid = 2002;
        let alive = [s1.pid];
        let v = vec![s1.clone(), s2];
        let live = live_filter(v, |pid| alive.contains(&pid));
        assert_eq!(live, vec![s1]);
    }

    // ── kiro_orphan_paths ──────────────────────────────────────────

    #[test]
    fn kiro_orphan_returns_empty_for_non_kiro() {
        let claude = mk_session("%1", "claude", "/repo", 1);
        let out = kiro_orphan_paths(&claude, &[]);
        assert!(out.is_empty());
    }

    #[test]
    fn kiro_orphan_returns_empty_when_sibling_present() {
        let mut a = mk_session("%1", "kiro", "/repo", 1);
        a.created_kiro_config = true;
        let b = mk_session("%2", "kiro", "/repo", 2);
        // Closing `a` while `b` still lives → no removal.
        let out = kiro_orphan_paths(&a, &[b]);
        assert!(out.is_empty());
    }

    #[test]
    fn kiro_orphan_returns_path_when_alone() {
        let a = mk_session("%1", "kiro", "/repo", 1);
        let out = kiro_orphan_paths(&a, &[]);
        assert_eq!(
            out,
            vec![PathBuf::from("/repo/.kiro/agents/agent-orch.json")]
        );
    }

    #[test]
    fn kiro_orphan_creation_flag_agnostic() {
        // The reuser has created_kiro_config=false — but if it's
        // the last live record in cwd, cleanup must still happen.
        // This is the order-(a) leak the spec calls out and that
        // the creation-flag-agnostic rule fixes.
        let mut reuser = mk_session("%2", "kiro", "/repo", 2);
        reuser.created_kiro_config = false;
        let out = kiro_orphan_paths(&reuser, &[]);
        assert_eq!(
            out,
            vec![PathBuf::from("/repo/.kiro/agents/agent-orch.json")]
        );
    }

    #[test]
    fn kiro_orphan_separates_by_cwd() {
        let a = mk_session("%1", "kiro", "/repo-a", 1);
        let b = mk_session("%2", "kiro", "/repo-b", 2);
        // Closing a while b lives in a different cwd → still remove a's config.
        let out = kiro_orphan_paths(&a, &[b]);
        assert_eq!(
            out,
            vec![PathBuf::from("/repo-a/.kiro/agents/agent-orch.json")]
        );
    }

    // ── with_lock smoke (in-process) ───────────────────────────────

    #[test]
    fn with_lock_runs_closure_and_releases() {
        let dir = tempdir().unwrap();
        let v = with_lock(dir.path(), || Ok(42_u32)).unwrap();
        assert_eq!(v, 42);
        // A subsequent acquire must not block.
        let v = with_lock(dir.path(), || Ok(7_u32)).unwrap();
        assert_eq!(v, 7);
    }

    // ── state_dir / sessions_path / tmp_dir_for_pane ───────────────

    #[test]
    fn tmp_dir_namespaces_by_pane() {
        let p = tmp_dir_for_pane(Path::new("/state"), "%42");
        assert_eq!(p, PathBuf::from("/state/tmp/%42"));
    }

    #[test]
    fn sessions_path_is_under_state() {
        let p = sessions_path(Path::new("/state"));
        assert_eq!(p, PathBuf::from("/state/sessions.json"));
    }
}
