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
/// builds this from real process state (`Env::from_process`); tests
/// build it with a tempdir state-dir and `Vec<u8>`-backed stdout /
/// stderr (`Vec<u8>` already implements `Write`). Borrowed lifetimes
/// keep the test path zero-allocation.
pub struct Env<'a> {
    pub state_dir: PathBuf,
    pub stdout: &'a mut dyn Write,
    pub stderr: &'a mut dyn Write,
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
        Some(Cmd::Wrap { .. }) => anyhow::bail!("agent-orch wrap: not implemented yet (slice 2)"),
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
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    let mut env = Env {
        state_dir: state,
        stdout: &mut stdout,
        stderr: &mut stderr,
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

    fn drive(state: &Path, stdout: &mut Vec<u8>, stderr: &mut Vec<u8>, args: &[&str]) -> i32 {
        let mut env = Env {
            state_dir: state.to_path_buf(),
            stdout,
            stderr,
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

        // The bail!() stubs verify dispatch reaches the right arm.
        // Each becomes a real behavior test in its slice.

        #[test]
        fn wrap_dispatch_currently_bails_with_slice_2_marker() {
            let (dir, mut so, mut se) = fixtures();
            let code = drive(
                dir.path(),
                &mut so,
                &mut se,
                &["wrap", "claude", "--", "echo"],
            );
            assert_eq!(code, 1);
            assert!(String::from_utf8_lossy(&se).contains("slice 2"));
        }

        #[test]
        fn hook_dispatch_currently_bails_with_slice_3_marker() {
            let (dir, mut so, mut se) = fixtures();
            let code = drive(dir.path(), &mut so, &mut se, &["hook", "Stop"]);
            assert_eq!(code, 1);
            assert!(String::from_utf8_lossy(&se).contains("slice 3"));
        }

        // Placeholders — flip on as each slice lands.

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
    }
}
