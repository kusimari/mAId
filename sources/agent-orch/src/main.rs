//! agent-orch — observation-only orchestrator over tmux + coding-agent panes.
//!
//! Script-style: one file. Four typeclasses, top-to-bottom.
//!
//!   §1 · `Session`  — registry record + per-record ops via `impl`.
//!   §2 · `Store`    — owns the state-dir, hides flock; exposes
//!                     `read()` (no lock) and `mutate(|v| ...)`.
//!   §3 · `Wrapper`  — trait with `Claude` / `Kiro` / `Other` impls.
//!                     `prepare` (per-kind config — Claude no-op,
//!                     Kiro project-scoped), `cleanup` (per-kind
//!                     unregister), and a default `hook` method body
//!                     shared across kinds. Claude hooks live
//!                     user-globally via `setup` / `teardown`.
//!   §4 · `Loop`     — picker (`render`, `render_to`, `run`, `body`).
//!
//! The CLI dispatch (`main`) wires these together. Tests at the
//! bottom drive each typeclass directly with a tempdir `Store`;
//! end-to-end coverage is `tests/agent-orch/integration.sh`.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SESSIONS_FILE: &str = "sessions.json";
const LOCK_FILE: &str = "sessions.lock";
const HOOK_MARKER: &str = ".tmux-hook-installed";

const EVT_USER_PROMPT_SUBMIT: &str = "UserPromptSubmit";
const EVT_PRE_TOOL_USE: &str = "PreToolUse";
const EVT_POST_TOOL_USE: &str = "PostToolUse";
const EVT_POST_TOOL_USE_FAILURE: &str = "PostToolUseFailure";
const EVT_NOTIFICATION: &str = "Notification";
const EVT_STOP: &str = "Stop";

/// All Claude hook events `setup` installs, in display order. Adding
/// an event means adding it here and to `apply_event`.
const HOOK_EVENTS: &[&str] = &[
    EVT_USER_PROMPT_SUBMIT,
    EVT_PRE_TOOL_USE,
    EVT_POST_TOOL_USE,
    EVT_POST_TOOL_USE_FAILURE,
    EVT_NOTIFICATION,
    EVT_STOP,
];

/// After this many seconds with no hook event, an `Active` session
/// is downgraded to `Stalled` at render time. Catches the case where
/// the hook reporter died, the agent crashed mid-tool, or a tool is
/// running for genuinely longer than expected. Render-only — never
/// written to the registry, so once an event arrives the row recovers.
const STALL_AFTER_SECS: u64 = 90;

const ORCHESTRATOR_SESSION: &str = "orchestrator";

/// Marker field on hook entries we wrote to a Claude settings file
/// via `setup`. `teardown` removes only entries carrying this tag,
/// leaving user-authored entries verbatim. The `x-` prefix is the
/// conventional "tool-private extension" signal in JSON config —
/// makes accidental collision with user-authored decoration
/// effectively zero.
const AGENT_ORCH_TAG: &str = "x-agent-orch-managed";

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ─────────────────────────────────────────────────────────────────────────────
// §1 · Session

/// Lifecycle state per session, derived from hook events and
/// downgraded at render time when activity goes silent. The
/// distinction that matters most: `Active` (a tool is in flight),
/// `Thinking` (the agent is deciding what to do — LLM round-trip
/// or between tools), `Waiting` (Claude raised a notification —
/// permission prompt, idle timer, etc.; the user needs to look),
/// `Idle` (Stop fired — last turn complete), `Cold` (registered
/// but no hooks ever fired), `Stalled` (was Active but no event
/// in `STALL_AFTER_SECS` — render-only, recovers on next event).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Cold,
    Thinking,
    Active,
    Waiting,
    Idle,
    Stalled,
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
    pub last_event: String,
    #[serde(default)]
    pub last_event_ts: u64,
    /// Timestamp `UserPromptSubmit` fired for the current turn. 0
    /// outside an active turn. Used to derive elapsed time and the
    /// completed-turn duration when `Stop` lands.
    #[serde(default)]
    pub prompt_started_at: u64,
    /// Timestamp `PreToolUse` fired for the current tool. 0 between
    /// tools. Used to render `0:04` next to a running tool.
    #[serde(default)]
    pub tool_started_at: u64,
    /// `Bash` / `Edit` / etc. — the bare tool name from the most
    /// recent Pre/PostToolUse payload.
    #[serde(default)]
    pub last_tool_name: String,
    /// Short preview of the tool's input, e.g. `cargo test` for a
    /// Bash call or `tests/foo.rs` for an Edit. Truncated.
    #[serde(default)]
    pub last_tool_preview: String,
    /// Tool-call count since the last `UserPromptSubmit`. Resets to
    /// 0 on the next prompt submit; final value is shown on Idle
    /// rows as "done in 2m31s · 6 tools".
    #[serde(default)]
    pub tools_this_turn: u32,
    /// Wall-clock duration of the most recently completed turn
    /// (Stop_ts - prompt_started_at). 0 if no turn has completed.
    #[serde(default)]
    pub last_turn_duration: u64,
    #[serde(default)]
    pub created_kiro_config: bool,
}

impl Session {
    /// Apply one hook event. Bumps `last_event` / `last_event_ts`
    /// unconditionally; per-event state transitions below. Hooks
    /// must never block the agent — this body's only failure mode
    /// is silent default of unknown event names.
    fn apply_event(&mut self, event: &str, payload: &serde_json::Value, now: u64) {
        self.last_event = event.into();
        self.last_event_ts = now;
        match event {
            EVT_USER_PROMPT_SUBMIT => {
                self.state = State::Thinking;
                self.state_ts = now;
                self.prompt_started_at = now;
                self.tool_started_at = 0;
                self.tools_this_turn = 0;
                self.last_tool_name.clear();
                self.last_tool_preview.clear();
                if let Some(p) = payload.get("prompt").and_then(|v| v.as_str()) {
                    // 80 scalar values, not graphemes — preview-only,
                    // splitting a flag emoji's components is fine.
                    self.last_prompt = p.chars().take(80).collect();
                }
            }
            EVT_PRE_TOOL_USE => {
                self.state = State::Active;
                self.state_ts = now;
                self.tool_started_at = now;
                if let Some(name) = payload.get("tool_name").and_then(|v| v.as_str()) {
                    self.last_tool_name = name.into();
                }
                self.last_tool_preview =
                    tool_input_preview(&self.last_tool_name, payload.get("tool_input"));
            }
            EVT_POST_TOOL_USE | EVT_POST_TOOL_USE_FAILURE => {
                self.state = State::Thinking;
                self.state_ts = now;
                self.tool_started_at = 0;
                self.tools_this_turn = self.tools_this_turn.saturating_add(1);
                if let Some(name) = payload.get("tool_name").and_then(|v| v.as_str()) {
                    self.last_tool_name = name.into();
                }
            }
            EVT_NOTIFICATION => {
                // Claude raised a user-visible notification —
                // commonly a permission prompt or an idle timer.
                // The agent is not making progress until the user
                // intervenes. Sticky until the next event clears it.
                self.state = State::Waiting;
                self.state_ts = now;
                self.tool_started_at = 0;
            }
            EVT_STOP => {
                self.state = State::Idle;
                self.state_ts = now;
                self.tool_started_at = 0;
                if self.prompt_started_at > 0 {
                    self.last_turn_duration = now.saturating_sub(self.prompt_started_at);
                }
            }
            _ => {} // unknown event: only the bumps above
        }
    }

    /// Render-time decoration: an `Active` session whose last event
    /// was more than `STALL_AFTER_SECS` ago is shown as `Stalled`.
    /// Never written to the registry — recovers as soon as the next
    /// hook event fires. Caller passes `now` so tests can be
    /// deterministic.
    fn effective_state(&self, now: u64) -> State {
        if matches!(self.state, State::Active)
            && now.saturating_sub(self.last_event_ts) > STALL_AFTER_SECS
        {
            State::Stalled
        } else {
            self.state.clone()
        }
    }

    /// Picker row content. Shape varies by state:
    ///   Active    `▶ <kind> <cwd> · <Tool(preview)> · 0:04`
    ///   Thinking  `◉ <kind> <cwd> · "<prompt>" · thinking · 1:02`
    ///   Waiting   `⚠ <kind> <cwd> · waiting (permission?) · 0:23`
    ///   Idle      `· <kind> <cwd> · done in 2m31s · 6 tools · 7m ago`
    ///   Cold      `· <kind> <cwd> · —`
    ///   Stalled   `⊘ <kind> <cwd> · stalled at <Tool(preview)> · 2m04s`
    /// Caller passes `now` so the elapsed/ago figures are
    /// rendered consistently across all rows in one snapshot.
    fn format_row(&self, now: u64) -> String {
        let st = self.effective_state(now);
        let glyph = state_glyph(&st);
        let head = format!("{} {} {}", glyph, self.kind, cwd_tail(&self.cwd));
        match st {
            State::Active => {
                let tool = self.tool_label();
                let elapsed = duration_short(now.saturating_sub(self.tool_started_at));
                format!("{head} · {tool} · {elapsed}")
            }
            State::Thinking => {
                let prompt = self.prompt_or_dash();
                let elapsed = duration_short(now.saturating_sub(self.prompt_started_at));
                format!("{head} · {prompt} · thinking · {elapsed}")
            }
            State::Waiting => {
                let elapsed = duration_short(now.saturating_sub(self.last_event_ts));
                format!("{head} · waiting · {elapsed}")
            }
            State::Idle => {
                let dur = duration_short(self.last_turn_duration);
                let ago = ago(now.saturating_sub(self.last_event_ts));
                format!(
                    "{head} · done in {dur} · {} tools · {ago}",
                    self.tools_this_turn
                )
            }
            State::Stalled => {
                let tool = self.tool_label();
                let since = duration_short(now.saturating_sub(self.last_event_ts));
                format!("{head} · stalled at {tool} · {since} silent")
            }
            State::Cold => format!("{head} · —"),
        }
    }

    fn tool_label(&self) -> String {
        if self.last_tool_name.is_empty() {
            "—".into()
        } else if self.last_tool_preview.is_empty() {
            self.last_tool_name.clone()
        } else {
            format!("{}({})", self.last_tool_name, self.last_tool_preview)
        }
    }

    fn prompt_or_dash(&self) -> &str {
        if self.last_prompt.is_empty() {
            "—"
        } else {
            self.last_prompt.as_str()
        }
    }

    /// "Active at" — max(state_ts, last_event_ts, started). Used by sort.
    fn activity(&self) -> u64 {
        self.state_ts.max(self.last_event_ts).max(self.started)
    }

    /// Sort precedence: doing-something > waiting > idle > cold.
    /// Stalled sorts with idle so a stuck row doesn't squat at the
    /// top forever.
    fn state_group(&self, now: u64) -> u8 {
        match self.effective_state(now) {
            State::Active => 0,
            State::Thinking => 1,
            State::Waiting => 2,
            State::Idle => 3,
            State::Stalled => 4,
            State::Cold => 5,
        }
    }
}

/// Glyph prefix per state. Plain ASCII/Unicode without ANSI; fzf
/// renders the row on its own line so we don't need color escapes
/// for the picker's pre-built theme to look reasonable. Color
/// support lives in a follow-up if the user wants it.
fn state_glyph(state: &State) -> &'static str {
    match state {
        State::Active => "▶",
        State::Thinking => "◉",
        State::Waiting => "⚠",
        State::Idle => "·",
        State::Stalled => "⊘",
        State::Cold => "·",
    }
}

/// Short duration for in-progress timers: `0:04`, `1:23`, `2m04s`,
/// `1h17m`. Threshold flips at 100s and at 1h to keep the column
/// width under 6 chars.
fn duration_short(secs: u64) -> String {
    if secs < 100 {
        let m = secs / 60;
        let s = secs % 60;
        format!("{}:{:02}", m, s)
    } else if secs < 3600 {
        let m = secs / 60;
        let s = secs % 60;
        format!("{}m{:02}s", m, s)
    } else {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        format!("{}h{:02}m", h, m)
    }
}

/// Human "<n> ago" for stale events. `now`-relative. Rounds toward
/// the nearest unit: `just now` for <5s, `4s ago`, `7m ago`,
/// `2h ago`, `3d ago`.
fn ago(secs: u64) -> String {
    if secs < 5 {
        return "just now".into();
    }
    if secs < 60 {
        return format!("{}s ago", secs);
    }
    if secs < 3600 {
        return format!("{}m ago", secs / 60);
    }
    if secs < 86400 {
        return format!("{}h ago", secs / 3600);
    }
    format!("{}d ago", secs / 86400)
}

/// Short preview of a tool's input. Returns "" when we don't
/// know how to summarize this tool. Per-tool whitelist — keeps
/// the row predictable; an unknown tool prints just its name.
fn tool_input_preview(tool: &str, input: Option<&serde_json::Value>) -> String {
    let Some(v) = input else { return String::new() };
    let pick = |key: &str| -> Option<String> {
        v.get(key)
            .and_then(|x| x.as_str())
            .map(|s| s.chars().take(40).collect::<String>())
    };
    match tool {
        "Bash" => pick("command").unwrap_or_default(),
        "Edit" | "Write" | "Read" | "NotebookEdit" => pick("file_path").unwrap_or_default(),
        "Grep" | "Glob" => pick("pattern").unwrap_or_default(),
        "Agent" | "Task" => pick("description").unwrap_or_default(),
        "WebFetch" | "WebSearch" => pick("url").or_else(|| pick("query")).unwrap_or_default(),
        _ => String::new(),
    }
}

fn cwd_tail(cwd: &str) -> String {
    use std::path::Component;
    let last2: Vec<&str> = Path::new(cwd)
        .components()
        // Skip RootDir / CurDir / ParentDir / Prefix — only named
        // segments. Otherwise "/repo" yields ["/", "repo"] and joins
        // to "//repo".
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

/// Sort: active > thinking > waiting > idle > stalled > cold;
/// within group, most-recently-active first.
fn sort_sessions(sessions: &mut [Session], now: u64) {
    sessions.sort_by(|a, b| {
        let group = a.state_group(now).cmp(&b.state_group(now));
        if group.is_ne() {
            return group;
        }
        b.activity().cmp(&a.activity())
    });
}

/// Drop entries whose pid is no longer alive (kernel signal-0 probe).
fn live_only(sessions: Vec<Session>) -> Vec<Session> {
    use nix::sys::signal::kill;
    use nix::unistd::Pid;
    sessions
        .into_iter()
        .filter(|s| kill(Pid::from_raw(s.pid), None).is_ok())
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// §2 · Store
//
// One value owns the state-dir, hides flock, exposes a clean
// read/mutate surface. Every read-modify-write site collapses from
// the with_lock + read_sessions + write_sessions dance to a single
// `store.mutate(|v| ...)` call.

pub struct Store {
    dir: PathBuf,
}

impl Store {
    /// Resolve `${XDG_STATE_HOME:-$HOME/.local/state}/agent-orch`.
    pub fn from_env() -> Result<Self> {
        if let Ok(xdg) = std::env::var("XDG_STATE_HOME") {
            if !xdg.is_empty() {
                return Ok(Store {
                    dir: PathBuf::from(xdg).join("agent-orch"),
                });
            }
        }
        let home = std::env::var("HOME").context("$HOME unset")?;
        Ok(Store {
            dir: PathBuf::from(home).join(".local/state/agent-orch"),
        })
    }

    /// Tests pass a tempdir.
    pub fn new(dir: PathBuf) -> Self {
        Store { dir }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn hook_marker(&self) -> PathBuf {
        self.dir.join(HOOK_MARKER)
    }

    /// Eventually-consistent read. No lock. Empty / missing file →
    /// `Vec::new()`. Malformed errors loud.
    pub fn read(&self) -> Result<Vec<Session>> {
        let path = self.dir.join(SESSIONS_FILE);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        if bytes.is_empty() {
            return Ok(Vec::new());
        }
        serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))
    }

    /// Read-modify-write under flock. The closure mutates `v` in
    /// place and may return a value (e.g. a `Prepared` argv) that
    /// the caller wants to use after the lock releases. Store
    /// handles the lock + atomic write. Lock releases when the
    /// file handle drops, so panics in `f` still release.
    pub fn mutate<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut Vec<Session>) -> Result<T>,
    {
        fs::create_dir_all(&self.dir)
            .with_context(|| format!("mkdir -p {}", self.dir.display()))?;
        let lock_file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(self.dir.join(LOCK_FILE))
            .with_context(|| format!("open {}", self.dir.join(LOCK_FILE).display()))?;
        let mut lock = fd_lock::RwLock::new(lock_file);
        let _guard = lock.write().context("flock")?;
        let mut sessions = self.read()?;
        let out = f(&mut sessions)?;
        self.write_atomic(&sessions)?;
        Ok(out)
    }

    /// Atomic write: write to per-pid tmp, then rename. The tmp name
    /// includes our pid so concurrent writers don't stomp each other's
    /// tmp file. Callers always hold `mutate`'s lock for read-modify-
    /// write atomicity. No fsync — state-dir scratch.
    fn write_atomic(&self, sessions: &[Session]) -> Result<()> {
        fs::create_dir_all(&self.dir)
            .with_context(|| format!("mkdir -p {}", self.dir.display()))?;
        let path = self.dir.join(SESSIONS_FILE);
        let tmp = path.with_extension(format!("json.tmp.{}", std::process::id()));
        fs::write(&tmp, serde_json::to_vec_pretty(sessions)?)
            .with_context(|| format!("write {}", tmp.display()))?;
        fs::rename(&tmp, &path)
            .with_context(|| format!("rename {} → {}", tmp.display(), path.display()))?;
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// §3 · Wrapper
//
// trait + three impls. Per-kind variation lives in the impl blocks;
// the kind-agnostic bits (registration, tmux hook install, execvp)
// live in the free `wrap()` function below the trait.

pub struct WrapCtx<'a> {
    pub store: &'a Store,
    pub self_path: &'a Path,
    pub pane_id: &'a str,
    pub cwd: &'a Path,
    pub agent_argv: &'a [String],
}

#[derive(Debug)]
pub struct Prepared {
    pub program: String,
    pub argv: Vec<String>,
    pub created_kiro_config: bool,
}

pub trait Wrapper {
    fn kind(&self) -> &str;

    /// Synthesize/ensure per-kind hook config, return (program, argv)
    /// for execvp + any flag the session record needs to carry.
    fn prepare(&self, ctx: &WrapCtx) -> Result<Prepared>;

    /// Per-kind cleanup on unregister.
    fn cleanup(&self, store: &Store, removing: &Session, others: &[Session]) -> Result<()>;

    /// Hook handling — default method body, identical for all kinds
    /// today. A future kind whose stdin payload differs can override.
    /// Always exits Ok — a failing hook reporter must not block the
    /// agent's turn.
    ///
    /// Caller verifies pane ownership; the CLI dispatch (`Cmd::Hook`)
    /// filters on `$AGENT_ORCH_PANE` before reaching here. Direct
    /// callers must do the same — this body acts on whatever
    /// `pane_id` it was given.
    fn hook(
        &self,
        store: &Store,
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
        store.mutate(|sessions| {
            if let Some(s) = sessions.iter_mut().find(|s| s.pane_id == pane_id) {
                s.apply_event(event, &payload, now);
            }
            Ok(()) // stale fire after unregister: silent no-op.
        })
    }
}

pub struct Claude;
pub struct Kiro;
pub struct Other(pub String);

impl Wrapper for Claude {
    fn kind(&self) -> &str {
        "claude"
    }

    /// No-op. Claude hooks live user-globally via
    /// `agent-orch setup`, not per-launch. The wrapper just
    /// passes argv through unchanged.
    fn prepare(&self, ctx: &WrapCtx) -> Result<Prepared> {
        Ok(Prepared {
            program: ctx.agent_argv[0].clone(),
            argv: ctx.agent_argv.to_vec(),
            created_kiro_config: false,
        })
    }

    /// No-op. The user-global hooks installed by `setup` outlive
    /// any individual pane and are removed only by `teardown`.
    fn cleanup(&self, _store: &Store, _removing: &Session, _others: &[Session]) -> Result<()> {
        Ok(())
    }
}

impl Wrapper for Kiro {
    fn kind(&self) -> &str {
        "kiro"
    }

    fn prepare(&self, ctx: &WrapCtx) -> Result<Prepared> {
        let dir = ctx.cwd.join(".kiro").join("agents");
        let path = dir.join("agent-orch.json");
        let created = if path.exists() {
            false
        } else {
            fs::create_dir_all(&dir).with_context(|| format!("mkdir -p {}", dir.display()))?;
            fs::write(
                &path,
                serde_json::to_vec_pretty(&build_kiro_config(ctx.self_path))?,
            )?;
            true
        };
        Ok(Prepared {
            program: ctx.agent_argv[0].clone(),
            argv: ctx.agent_argv.to_vec(),
            created_kiro_config: created,
        })
    }

    /// Refcount-agnostic: if no other live `kind=kiro` session shares
    /// `removing.cwd`, remove the project-scoped config. Closing the
    /// creator first while reusers remain still keeps the file alive
    /// (a sibling exists). Closing the last reuser removes it even if
    /// its `created_kiro_config=false`.
    fn cleanup(&self, _store: &Store, removing: &Session, others: &[Session]) -> Result<()> {
        let has_sibling = others
            .iter()
            .any(|s| s.kind == "kiro" && s.cwd == removing.cwd);
        if has_sibling {
            return Ok(());
        }
        let cfg = PathBuf::from(&removing.cwd)
            .join(".kiro")
            .join("agents")
            .join("agent-orch.json");
        let _ = fs::remove_file(&cfg);
        if let Some(parent) = cfg.parent() {
            let _ = fs::remove_dir(parent);
            if let Some(grand) = parent.parent() {
                let _ = fs::remove_dir(grand);
            }
        }
        Ok(())
    }
}

impl Wrapper for Other {
    fn kind(&self) -> &str {
        &self.0
    }

    fn prepare(&self, ctx: &WrapCtx) -> Result<Prepared> {
        Ok(Prepared {
            program: ctx.agent_argv[0].clone(),
            argv: ctx.agent_argv.to_vec(),
            created_kiro_config: false,
        })
    }

    fn cleanup(&self, _store: &Store, _removing: &Session, _others: &[Session]) -> Result<()> {
        Ok(())
    }
}

fn wrapper_for(kind: &str) -> Box<dyn Wrapper> {
    match kind {
        "claude" => Box::new(Claude),
        "kiro" => Box::new(Kiro),
        other => Box::new(Other(other.to_string())),
    }
}

/// Add our four hook entries to a Claude-style settings JSON,
/// each tagged with `"x-agent-orch-managed": true` so `unmerge`
/// can find and remove only ours.
///
/// Claude's hooks schema is nested: each event maps to an array of
/// matcher-groups, where each matcher-group has a `matcher` string
/// (tool-name selector; "" matches all) and a `hooks` array of
/// command entries. Our commands fire unconditionally, so we use
/// `matcher: ""`.
///
/// ```json
/// "hooks": {
///   "Stop": [
///     { "matcher": "",
///       "hooks": [{ "type": "command", "command": "<self> hook Stop" }],
///       "x-agent-orch-managed": true }
///   ]
/// }
/// ```
///
/// **Idempotent.** If a tagged entry already exists for an event,
/// the existing command is rewritten to match `self_path` (handles
/// the binary-moved case). New events are appended; user-authored
/// entries (without the tag) are preserved verbatim.
///
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
    for ev in HOOK_EVENTS {
        let arr = hooks.entry((*ev).to_string()).or_insert_with(|| json!([]));
        let Value::Array(list) = arr else {
            anyhow::bail!("user claude settings.hooks.{} must be an array", ev);
        };
        let cmd = format!("{} hook {}", self_str, ev);
        // Idempotence + path refresh: if our tagged entry is
        // already there, rewrite its command (in case the binary
        // moved). Otherwise append a fresh tagged entry.
        if let Some(existing) = list.iter_mut().find(|e| {
            e.get(AGENT_ORCH_TAG)
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        }) {
            // Happy path: rewrite the command in slot 0 of the
            // nested hooks array. `continue` exits the per-event
            // loop so we don't fall through to the overwrite.
            if let Some(inner) = existing.get_mut("hooks").and_then(|h| h.as_array_mut()) {
                if let Some(first) = inner.first_mut() {
                    first["command"] = json!(cmd);
                    continue;
                }
            }
            // Fall-through: tagged entry has unexpected shape
            // (missing `hooks` array, empty array, wrong type).
            // Overwrite it cleanly.
            *existing = json!({
                "matcher": "",
                "hooks": [{ "type": "command", "command": cmd }],
                AGENT_ORCH_TAG: true,
            });
        } else {
            list.push(json!({
                "matcher": "",
                "hooks": [{ "type": "command", "command": cmd }],
                AGENT_ORCH_TAG: true,
            }));
        }
    }
    Ok(())
}

/// Reverse of `merge_claude_hooks`: remove every tagged entry from
/// the settings JSON. Prunes empty containers — if an event's array
/// becomes empty, drop the key; if `hooks` becomes empty, drop the
/// `hooks` key. Caller decides whether to delete the file when the
/// result is `{}`.
fn unmerge_claude_hooks(settings: &mut serde_json::Value) {
    let Some(root) = settings.as_object_mut() else {
        return;
    };
    let Some(hooks) = root.get_mut("hooks").and_then(|v| v.as_object_mut()) else {
        return;
    };
    let event_names: Vec<String> = hooks.keys().cloned().collect();
    for ev in event_names {
        let Some(arr) = hooks.get_mut(&ev).and_then(|v| v.as_array_mut()) else {
            continue;
        };
        arr.retain(|e| {
            !e.get(AGENT_ORCH_TAG)
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        });
        if arr.is_empty() {
            hooks.remove(&ev);
        }
    }
    if hooks.is_empty() {
        root.remove("hooks");
    }
}

/// `agent-orch setup` — install our four hook entries into the
/// `agent-orch setup` — install our four hook entries into the
/// Claude settings file at `path` (typically
/// `~/.claude/settings.json`). Creates the file (and parent dir)
/// if absent. Idempotent: re-running rewrites our tagged entries'
/// command paths in case the binary moved, but doesn't duplicate.
///
/// `key`: optional tmux prefix-table suffix to bind for
/// switching back to the orchestrator session (e.g. `Some("O")`
/// → `<prefix> O` runs `switch-client -t orchestrator`). When
/// `None`, no keybind is installed. When `Some`, any pre-existing
/// switch-to-orchestrator binding (from a previous `setup` with
/// a different key) is removed first so re-keying is clean.
fn run_setup(path: &Path, self_path: &Path, key: Option<&str>) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("mkdir -p {}", parent.display()))?;
    }
    let mut settings: serde_json::Value = if path.exists() {
        let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
        if bytes.is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?
        }
    } else {
        serde_json::json!({})
    };
    merge_claude_hooks(&mut settings, self_path)?;
    write_json_atomic(path, &settings)?;
    if let Some(suffix) = key {
        // Remove any prior binding (might be a different key from
        // an earlier setup) before installing the new one.
        uninstall_tmux_keybind();
        install_tmux_keybind(suffix);
    }
    Ok(())
}

/// `agent-orch teardown` — remove our tagged hook entries from
/// the Claude settings file at `path`. Prunes empty containers;
/// removes the file entirely if the result is `{}`. No-op if the
/// file is absent. Always self-discovers and removes any prefix
/// binding whose action is `switch-client -t orchestrator` —
/// no key argument needed.
fn run_teardown(path: &Path) -> Result<()> {
    uninstall_tmux_keybind();
    if !path.exists() {
        return Ok(());
    }
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    if bytes.is_empty() {
        // Empty file → nothing to do; leave it alone.
        return Ok(());
    }
    let mut settings: serde_json::Value =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;
    unmerge_claude_hooks(&mut settings);
    let is_empty_obj = settings.as_object().map(|o| o.is_empty()).unwrap_or(false);
    if is_empty_obj {
        fs::remove_file(path).with_context(|| format!("rm {}", path.display()))?;
        Ok(())
    } else {
        write_json_atomic(path, &settings)
    }
}

/// Run a tmux command, honoring `$AGENT_ORCH_TMUX_SOCKET` as
/// `-L <name>` (used by the integration script to target a
/// private server) and suppressing stderr (best-effort: a
/// "no server" message during first-run is normal).
fn tmux_cmd(args: &[&str]) -> std::process::Command {
    let mut cmd = std::process::Command::new("tmux");
    if let Ok(name) = std::env::var("AGENT_ORCH_TMUX_SOCKET") {
        if !name.is_empty() {
            cmd.arg("-L").arg(name);
        }
    }
    cmd.args(args).stderr(std::process::Stdio::null());
    cmd
}

/// Best-effort: bind `<prefix> <suffix>` to
/// `switch-client -t orchestrator` on the running tmux server.
/// Prefix-bound (default `C-b`) rather than root-bound so inner
/// TUIs (claude/kiro) never see the keystroke and can't race
/// with tmux for it — same idiom as every other tmux command
/// (`C-b c`, `C-b "`, `C-b d`).
///
/// Live-only — survives until the server exits. If tmux isn't
/// running (no server, no $TMUX, not on PATH), silently no-op;
/// the user can re-run `setup` after starting tmux. Persistence
/// across server restarts is the user's job (bake the
/// equivalent line into `~/.tmux.conf` via home-manager /
/// chezmoi / yadm / ansible-pull / your dotfiles setup — see
/// the spec's Decision Log).
fn install_tmux_keybind(suffix: &str) {
    let _ = tmux_cmd(&[
        "bind-key",
        "-T",
        "prefix",
        suffix,
        "switch-client",
        "-t",
        ORCHESTRATOR_SESSION,
    ])
    .status();
}

/// Self-discovering reverse: scan the prefix table for any
/// binding whose action is `switch-client -t orchestrator` and
/// unbind it. Lets the user re-key without remembering the old
/// suffix and lets `teardown` work without a `--key` flag.
fn uninstall_tmux_keybind() {
    let out = tmux_cmd(&["list-keys", "-T", "prefix"])
        .stderr(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .output();
    let Ok(out) = out else { return };
    if !out.status.success() {
        return;
    }
    // Each line looks like:
    //   bind-key    -T prefix O       switch-client -t orchestrator
    // Capture the suffix (the column after `-T prefix`) on lines
    // ending in our action string.
    let stdout = String::from_utf8_lossy(&out.stdout);
    let target_action = format!("switch-client -t {}", ORCHESTRATOR_SESSION);
    for line in stdout.lines() {
        if !line.contains(&target_action) {
            continue;
        }
        let Some(after) = line.split("-T prefix").nth(1) else {
            continue;
        };
        let Some(suffix) = after.split_whitespace().next() else {
            continue;
        };
        let _ = tmux_cmd(&["unbind-key", "-T", "prefix", suffix]).status();
    }
}

/// Atomic write for arbitrary JSON files: per-pid tmp + rename.
/// Used by setup/teardown writing the user's settings file.
fn write_json_atomic(path: &Path, value: &serde_json::Value) -> Result<()> {
    let tmp = path.with_extension(format!("json.tmp.{}", std::process::id()));
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(&tmp, &bytes).with_context(|| format!("write {}", tmp.display()))?;
    fs::rename(&tmp, path)
        .with_context(|| format!("rename {} → {}", tmp.display(), path.display()))?;
    Ok(())
}

/// Resolve `~/.claude/settings.json`. Used by `main`'s setup /
/// teardown dispatch. Tests use injected paths.
fn user_claude_settings_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("$HOME unset")?;
    Ok(PathBuf::from(home).join(".claude/settings.json"))
}

/// Build a fresh Kiro agent config wiring our four hook commands.
/// Same nested matcher+hooks schema as Claude.
fn build_kiro_config(self_path: &Path) -> serde_json::Value {
    use serde_json::json;
    let s = self_path.to_string_lossy();
    let entry = |ev: &str| {
        json!([{
            "matcher": "",
            "hooks": [{ "type": "command", "command": format!("{} hook {}", s, ev) }],
        }])
    };
    json!({
        "hooks": {
            EVT_USER_PROMPT_SUBMIT: entry(EVT_USER_PROMPT_SUBMIT),
            EVT_PRE_TOOL_USE:       entry(EVT_PRE_TOOL_USE),
            EVT_POST_TOOL_USE:      entry(EVT_POST_TOOL_USE),
            EVT_STOP:               entry(EVT_STOP),
        }
    })
}

/// Install the global tmux `pane-exited` hook. Idempotent via a
/// marker file. tmux `set-hook -g` is itself idempotent so the
/// race between two wrappers passing `marker.exists() == false` is
/// benign.
fn install_pane_exited_hook(store: &Store, self_path: &Path) -> Result<()> {
    let marker = store.hook_marker();
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

/// Top-level wrap: dispatch to the right `Wrapper`, mutate the store,
/// then (when side effects are enabled) install the global tmux hook
/// + tag the pane + execvp the agent.
///
/// `prepare` and the registry write share one critical section: the
/// store mutation calls `w.prepare(ctx)` *inside* the lock. Closes the
/// race where a concurrent `unregister` could remove a kiro config in
/// the gap between `ensure` and `register`.
pub fn wrap(w: &dyn Wrapper, ctx: &WrapCtx, side_effects: bool) -> Result<Prepared> {
    anyhow::ensure!(
        !ctx.agent_argv.is_empty(),
        "agent-orch wrap needs an agent command after `--`"
    );

    let now = now_secs();
    let prepared = ctx.store.mutate(|sessions| {
        // If a record exists for this pane, two cases:
        //  - the recorded pid is alive → genuine double-register, refuse loud.
        //  - the recorded pid is dead → an earlier wrap exited but the pane
        //    stayed alive (interactive shell, remain-on-exit, or the
        //    pane-exited hook didn't fire — server restart, etc.). Run the
        //    kind-specific cleanup for the stale record and replace it.
        if let Some(idx) = sessions.iter().position(|s| s.pane_id == ctx.pane_id) {
            let alive =
                nix::sys::signal::kill(nix::unistd::Pid::from_raw(sessions[idx].pid), None).is_ok();
            anyhow::ensure!(
                !alive,
                "pane {} already registered — `agent-orch unregister {}` first",
                ctx.pane_id,
                ctx.pane_id
            );
            let stale = sessions.remove(idx);
            // Cleanup uses the *remaining* sessions as siblings — the kiro
            // refcount logic stays correct because `stale` is no longer in
            // the list.
            wrapper_for(&stale.kind).cleanup(ctx.store, &stale, sessions)?;
        }
        let prepared = w.prepare(ctx)?;
        sessions.push(Session {
            pane_id: ctx.pane_id.into(),
            pid: std::process::id() as i32,
            kind: w.kind().into(),
            cwd: ctx.cwd.to_string_lossy().into_owned(),
            started: now,
            state: State::Cold,
            state_ts: now,
            last_prompt: String::new(),
            last_event: String::new(),
            last_event_ts: 0,
            prompt_started_at: 0,
            tool_started_at: 0,
            last_tool_name: String::new(),
            last_tool_preview: String::new(),
            tools_this_turn: 0,
            last_turn_duration: 0,
            created_kiro_config: prepared.created_kiro_config,
        });
        Ok(prepared)
    })?;

    if side_effects {
        install_pane_exited_hook(ctx.store, ctx.self_path)?;
        tmux_set_pane_option(ctx.pane_id, "@agent-orch-pane", ctx.pane_id)?;
        // SAFETY: single-threaded immediately before execvp.
        std::env::set_var("AGENT_ORCH_PANE", ctx.pane_id);
        exec_agent(&prepared.program, &prepared.argv)?;
    }
    Ok(prepared)
}

/// `unregister`: remove the record, run per-kind cleanup via the
/// matching `Wrapper` impl. Tmux `pane-exited` target.
pub fn unregister(store: &Store, pane_id: &str) -> Result<()> {
    store.mutate(|sessions| {
        let Some(idx) = sessions.iter().position(|s| s.pane_id == pane_id) else {
            return Ok(()); // already gone
        };
        let removing = sessions.remove(idx);
        wrapper_for(&removing.kind).cleanup(store, &removing, sessions)
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// §4 · Loop

pub struct Loop<'a> {
    store: &'a Store,
}

impl<'a> Loop<'a> {
    pub fn new(store: &'a Store) -> Self {
        Loop { store }
    }

    /// Read sessions, filter live pids, sort, format rows.
    /// Returns `(pane_id, formatted_row)` pairs. `now` is captured
    /// once so all rows in one snapshot share the same time
    /// reference (otherwise a "5s ago" / "0:04" reading drifts
    /// across rows in a slow render).
    pub fn render(&self) -> Result<Vec<(String, String)>> {
        let now = now_secs();
        let mut sessions = live_only(self.store.read()?);
        sort_sessions(&mut sessions, now);
        Ok(sessions
            .into_iter()
            .map(|s| (s.pane_id.clone(), s.format_row(now)))
            .collect())
    }

    /// Print the rendered rows to `stdout`, one tab-separated
    /// `<pane_id>\t<formatted-row>` line each. Used by the
    /// hidden `agent-orch render` subcommand which the
    /// event-driven `body` invokes via fzf's `reload(...)` action.
    pub fn render_to(&self, stdout: &mut dyn Write) -> Result<()> {
        for (id, row) in self.render()? {
            writeln!(stdout, "{}\t{}", id, row)?;
        }
        Ok(())
    }

    /// Ensure the orchestrator tmux session exists, switch the
    /// client to it. The picker loop body runs inside that
    /// session via a bare `agent-orch` invocation — bare
    /// detects whether it's already inside the orchestrator
    /// session and, if so, runs `body` instead of recursing.
    pub fn run(&self, self_path: &Path) -> Result<()> {
        let has = std::process::Command::new("tmux")
            .args(["has-session", "-t", ORCHESTRATOR_SESSION])
            .status()
            .context("tmux has-session")?
            .success();
        if !has {
            let cmd = self_path.display().to_string();
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

    /// Event-driven picker loop body. Runs inside the
    /// orchestrator session.
    ///
    /// `fzf --listen` opens a Unix socket for control commands.
    /// `--with-nth=2..` shows every column except the pane id;
    /// `--track --id-nth=1` sticks the highlight to the same
    /// pane id across reloads (so a state change to the focused
    /// row doesn't lose the highlight). `enter:execute-silent
    /// (tmux switch-client -t {1})` is non-terminal — fzf stays
    /// alive across selections. We push a `reload(self render)`
    /// command to the listen socket whenever sessions.json
    /// changes; debounced 100ms by `notify-debouncer-mini`.
    pub fn body(&self, self_path: &Path) -> Result<()> {
        let sock_path = pick_listen_socket();
        let sock = sock_path.to_string_lossy().to_string();
        let render_cmd = format!("{} render", self_path.display());
        let bind = "enter:execute-silent(tmux switch-client -t {1})+clear-query";

        let mut child = std::process::Command::new("fzf")
            .args([
                &format!("--listen={sock}"),
                "--with-nth=2..",
                "--track",
                "--id-nth=1",
                "--bind",
                bind,
            ])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .context("spawn fzf (is it on PATH?)")?;

        // Seed the initial row set on stdin, then close it so
        // fzf treats the source as exhausted; subsequent updates
        // arrive via reload commands on the listen socket.
        {
            let stdin = child.stdin.as_mut().context("fzf stdin")?;
            for (id, row) in self.render()? {
                writeln!(stdin, "{}\t{}", id, row)?;
            }
        }
        drop(child.stdin.take());

        // Watcher → main thread channel. Watcher errors are
        // surfaced once on stderr, then we keep going (fzf
        // selections still work without auto-reload).
        let (tx, rx) = mpsc::channel::<()>();
        let watch_dir = self.store.dir().to_path_buf();
        let watcher_tx = tx.clone();
        thread::spawn(move || {
            if let Err(e) = run_watcher(&watch_dir, watcher_tx) {
                eprintln!("agent-orch watcher: {e:#}");
            }
        });

        // Heartbeat thread: emit a tick once a second so the
        // picker re-renders even when no hook event has fired.
        // This keeps elapsed-time columns moving (`0:04` → `0:05`)
        // and lets `effective_state` demote a silent Active row to
        // Stalled live, without waiting on the next hook write.
        let heartbeat_tx = tx;
        thread::spawn(move || {
            while heartbeat_tx.send(()).is_ok() {
                thread::sleep(Duration::from_secs(1));
            }
        });

        loop {
            // Block up to 200ms on a watcher tick. If one
            // arrives, push reload to fzf. Either way, poll the
            // child between iterations so we can exit when fzf
            // does (Esc / Ctrl-C / kill).
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(()) => {
                    // Drain any backlog so we coalesce bursts
                    // into one reload.
                    while rx.try_recv().is_ok() {}
                    if let Err(e) = push_reload(&sock_path, &render_cmd) {
                        eprintln!("agent-orch reload: {e:#}");
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    // Watcher died; loop continues without
                    // auto-reload. fzf selections still work.
                }
            }
            if let Some(_status) = child.try_wait().context("fzf try_wait")? {
                break;
            }
        }
        let _ = fs::remove_file(&sock_path);
        Ok(())
    }
}

/// Pick a path for fzf's listen socket. Prefer
/// `${XDG_RUNTIME_DIR}` (typically tmpfs, per-user, cleaned at
/// logout); fall back to `$TMPDIR` / `/tmp`. Per-pid filename so
/// concurrent orchestrator sessions don't collide.
fn pick_listen_socket() -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .or_else(|| std::env::var_os("TMPDIR").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    dir.join(format!("agent-orch-fzf-{}.sock", std::process::id()))
}

/// Watch the store dir for sessions.json changes; emit one
/// `()` per debounced batch. Debounce window is 100ms — short
/// enough that the picker feels live, long enough to coalesce
/// the tmp+rename pair an atomic write produces.
fn run_watcher(dir: &Path, tx: mpsc::Sender<()>) -> Result<()> {
    use notify::RecursiveMode;
    use notify_debouncer_mini::new_debouncer;

    fs::create_dir_all(dir).with_context(|| format!("mkdir -p {}", dir.display()))?;
    // Watch the directory (not the file) — the file may not
    // exist yet on first run, and atomic rename swaps inodes
    // anyway, which would invalidate a file-level watch.
    let (notify_tx, notify_rx) = mpsc::channel();
    let mut debouncer =
        new_debouncer(Duration::from_millis(100), notify_tx).context("notify debouncer")?;
    debouncer
        .watcher()
        .watch(dir, RecursiveMode::NonRecursive)
        .with_context(|| format!("watch {}", dir.display()))?;

    let target = SESSIONS_FILE;
    for batch in notify_rx {
        let events = batch.unwrap_or_default();
        let interesting = events.iter().any(|e| {
            e.path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n == target)
                .unwrap_or(false)
        });
        if interesting && tx.send(()).is_err() {
            // main loop dropped the receiver — fzf exited
            break;
        }
    }
    Ok(())
}

/// Push `reload(<cmd>)` to fzf's listen socket. fzf speaks
/// HTTP/1.1 over the UDS — the request body is the action
/// string, no special framing beyond Content-Length.
fn push_reload(sock: &Path, render_cmd: &str) -> Result<()> {
    let body = format!("reload({render_cmd})");
    let mut stream =
        UnixStream::connect(sock).with_context(|| format!("connect {}", sock.display()))?;
    stream
        .set_write_timeout(Some(Duration::from_millis(500)))
        .ok();
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .ok();
    let request = format!(
        "POST / HTTP/1.1\r\nHost: fzf\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream
        .write_all(request.as_bytes())
        .context("write reload")?;
    // Drain the response so fzf doesn't see a half-closed
    // socket. We don't parse it — any 2xx/4xx is acceptable;
    // a connection error already surfaces above.
    let mut sink = Vec::new();
    let _ = stream.read_to_end(&mut sink);
    Ok(())
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
    /// Install agent-orch hooks into ~/.claude/settings.json (idempotent).
    /// With `--key X`, also bind `<tmux-prefix> X` to `switch-client -t
    /// orchestrator` on the running tmux server. Key is whatever you
    /// want to type after your tmux prefix (`O`, `a`, `F1`, etc.).
    Setup {
        /// Tmux prefix-table key suffix to bind for "switch back to
        /// orchestrator" (e.g. `O` → press your tmux prefix then `O`).
        /// Omit to install hooks only, no keybind.
        #[arg(long)]
        key: Option<String>,
    },
    /// Remove agent-orch hooks from ~/.claude/settings.json.
    /// Self-discovers and removes any prefix binding whose action is
    /// `switch-client -t orchestrator` — no `--key` argument needed.
    Teardown,
    /// Wrap a coding agent: register, inject hooks, execvp.
    Wrap {
        kind: String,
        #[arg(long)]
        cwd: Option<PathBuf>,
        #[arg(last = true)]
        agent_argv: Vec<String>,
    },
    /// Hook reporter; called by Claude/Kiro on each lifecycle event.
    #[command(hide = true)]
    Hook { event: String },
    /// Remove a record (called by tmux pane-exited).
    #[command(hide = true)]
    Unregister { pane_id: String },
    /// Print one tab-separated <pane_id>\t<row> per session,
    /// in picker-sort order, dead pids filtered out. Used by
    /// the loop body's fzf reload action.
    #[command(hide = true)]
    Render,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let store = Store::from_env()?;
    let self_path = std::env::current_exe().context("current_exe")?;

    match cli.cmd {
        // Bare invocation. Self-detects:
        //  - inside the `orchestrator` tmux session → run the
        //    event-driven picker `body` (this is what tmux
        //    spawned when the session was created);
        //  - anywhere else → ensure the orchestrator session
        //    exists and switch-client to it.
        None => {
            if inside_orchestrator() {
                Loop::new(&store).body(&self_path)
            } else {
                Loop::new(&store).run(&self_path)
            }
        }

        Some(Cmd::Setup { key }) => {
            run_setup(&user_claude_settings_path()?, &self_path, key.as_deref())
        }
        Some(Cmd::Teardown) => run_teardown(&user_claude_settings_path()?),

        Some(Cmd::Wrap {
            kind,
            cwd,
            agent_argv,
        }) => {
            let pane_id = std::env::var("TMUX_PANE")
                .ok()
                .filter(|s| !s.is_empty())
                .context("agent-orch wrap requires $TMUX_PANE — run inside tmux")?;
            let resolved_cwd = match cwd {
                Some(p) => p,
                None => std::env::current_dir().context("current_dir")?,
            };
            let ctx = WrapCtx {
                store: &store,
                self_path: &self_path,
                pane_id: &pane_id,
                cwd: &resolved_cwd,
                agent_argv: &agent_argv,
            };
            wrap(&*wrapper_for(&kind), &ctx, true).map(|_| ())
        }

        Some(Cmd::Hook { event }) => {
            // Hooks must never block the agent's turn — fail-soft.
            // The unlocked read can fail (corrupt registry); silently
            // skip rather than surfacing the error to the agent.
            if let Some(pane) = std::env::var("AGENT_ORCH_PANE")
                .ok()
                .filter(|s| !s.is_empty())
            {
                let kind = store
                    .read()
                    .ok()
                    .and_then(|s| s.iter().find(|x| x.pane_id == pane).map(|x| x.kind.clone()));
                if let Some(kind) = kind {
                    let mut stdin = std::io::stdin();
                    let _ = wrapper_for(&kind).hook(&store, &pane, &event, &mut stdin, now_secs());
                }
            }
            Ok(())
        }

        Some(Cmd::Unregister { pane_id }) => unregister(&store, &pane_id),

        Some(Cmd::Render) => {
            let mut stdout = std::io::stdout();
            Loop::new(&store).render_to(&mut stdout)
        }
    }
}

/// True iff the current process is running inside the
/// `orchestrator` tmux session. Used by bare `agent-orch` to
/// decide between `run` (bootstrap from outside) and `body`
/// (the picker tmux spawned from `run`). `$TMUX` set + tmux's
/// `display-message #{session_name}` returning `orchestrator`
/// is the load-bearing check; either alone misclassifies (a
/// user inside any tmux session running bare `agent-orch` would
/// otherwise spawn a body in their own session).
fn inside_orchestrator() -> bool {
    if std::env::var_os("TMUX").is_none() {
        return false;
    }
    let out = std::process::Command::new("tmux")
        .args(["display-message", "-p", "#{session_name}"])
        .output();
    match out {
        Ok(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout).trim() == ORCHESTRATOR_SESSION
        }
        _ => false,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
//
// Each typeclass is exercised through its own surface: `Store` via
// read/mutate, `Wrapper` via `wrap()` driving Claude/Kiro/Other,
// hook via `Wrapper::hook(&store, ...)`, `Loop` via `render`.
// End-to-end (real tmux, real argv preserved across execvp, real
// pane-exited hook) is `tests/agent-orch/integration.sh`.

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tempfile::{tempdir, TempDir};

    // 1 · Session stub ────────────────────────────────────────────

    fn mk(pane: &str, kind: &str, cwd: &str, started: u64) -> Session {
        Session {
            pane_id: pane.into(),
            pid: std::process::id() as i32,
            kind: kind.into(),
            cwd: cwd.into(),
            started,
            state: State::Cold,
            state_ts: started,
            last_prompt: String::new(),
            last_event: String::new(),
            last_event_ts: 0,
            prompt_started_at: 0,
            tool_started_at: 0,
            last_tool_name: String::new(),
            last_tool_preview: String::new(),
            tools_this_turn: 0,
            last_turn_duration: 0,
            created_kiro_config: false,
        }
    }

    /// Helper: build the JSON payload an apply_event call expects.
    fn payload(prompt: Option<&str>, tool: Option<&str>) -> serde_json::Value {
        use serde_json::json;
        let mut obj = serde_json::Map::new();
        if let Some(p) = prompt {
            obj.insert("prompt".into(), json!(p));
        }
        if let Some(t) = tool {
            obj.insert("tool_name".into(), json!(t));
        }
        json!(obj)
    }

    // 2 · Store + WrapCtx fixtures ───────────────────────────────

    fn fixtures() -> (TempDir, Store) {
        let dir = tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf());
        (dir, store)
    }

    /// Build a WrapCtx for a test. `store` and `cwd` are inside the
    /// tempdir so any per-kind side effects (Kiro project config)
    /// land where the test expects. `self_path` is a stable string
    /// the test asserts on.
    fn ctx<'a>(
        store: &'a Store,
        pane_id: &'a str,
        cwd: &'a Path,
        argv: &'a [String],
        self_path: &'a Path,
    ) -> WrapCtx<'a> {
        WrapCtx {
            store,
            self_path,
            pane_id,
            cwd,
            agent_argv: argv,
        }
    }

    // 3 · Session — apply_event + format_row + sort ──────────────

    #[test]
    fn session_apply_event_walks_thinking_active_thinking_idle() {
        let mut s = mk("%1", "claude", "/repo", 1000);

        // Submit prompt → Thinking, prompt_started_at recorded.
        s.apply_event(EVT_USER_PROMPT_SUBMIT, &payload(Some("hello"), None), 1100);
        assert_eq!(s.state, State::Thinking);
        assert_eq!(s.last_prompt, "hello");
        assert_eq!(s.prompt_started_at, 1100);
        assert_eq!(s.tools_this_turn, 0);

        // Tool starts → Active, tool_started_at recorded.
        s.apply_event(EVT_PRE_TOOL_USE, &payload(None, Some("Bash")), 1200);
        assert_eq!(s.state, State::Active);
        assert_eq!(s.tool_started_at, 1200);
        assert_eq!(s.last_tool_name, "Bash");

        // Tool ends → back to Thinking, count bumped.
        s.apply_event(EVT_POST_TOOL_USE, &payload(None, Some("Bash")), 1250);
        assert_eq!(s.state, State::Thinking);
        assert_eq!(s.tool_started_at, 0);
        assert_eq!(s.tools_this_turn, 1);

        // Stop → Idle, last_turn_duration recorded.
        s.apply_event(EVT_STOP, &payload(None, None), 1300);
        assert_eq!(s.state, State::Idle);
        assert_eq!(s.last_turn_duration, 200); // 1300 - 1100
    }

    #[test]
    fn session_apply_event_notification_sets_waiting() {
        let mut s = mk("%1", "claude", "/repo", 1);
        s.apply_event(EVT_USER_PROMPT_SUBMIT, &payload(Some("ok"), None), 100);
        s.apply_event(EVT_NOTIFICATION, &payload(None, None), 200);
        assert_eq!(s.state, State::Waiting);
    }

    #[test]
    fn session_apply_event_post_tool_failure_counts_and_thinks() {
        // PostToolUseFailure should be treated like PostToolUse:
        // count it as a completed tool call and return to Thinking.
        let mut s = mk("%1", "claude", "/repo", 1);
        s.apply_event(EVT_USER_PROMPT_SUBMIT, &payload(Some("x"), None), 100);
        s.apply_event(EVT_PRE_TOOL_USE, &payload(None, Some("Bash")), 110);
        s.apply_event(EVT_POST_TOOL_USE_FAILURE, &payload(None, Some("Bash")), 120);
        assert_eq!(s.state, State::Thinking);
        assert_eq!(s.tools_this_turn, 1);
    }

    #[test]
    fn session_apply_event_truncates_long_prompt() {
        let mut s = mk("%1", "claude", "/repo", 1);
        s.apply_event(
            EVT_USER_PROMPT_SUBMIT,
            &payload(Some(&"x".repeat(200)), None),
            2,
        );
        assert_eq!(s.last_prompt.chars().count(), 80);
    }

    #[test]
    fn session_format_row_active_shows_tool_and_elapsed() {
        let mut s = mk("%1", "claude", "/home/me/repo/foo", 1000);
        s.state = State::Active;
        s.tool_started_at = 1100;
        s.last_event_ts = 1100; // recent — not stalled
        s.last_tool_name = "Bash".into();
        s.last_tool_preview = "cargo test".into();
        let row = s.format_row(1104);
        assert!(row.contains('▶'), "row: {row}");
        assert!(row.contains("repo/foo"));
        assert!(row.contains("Bash(cargo test)"));
        assert!(row.contains("0:04"));
    }

    #[test]
    fn session_format_row_thinking_shows_prompt_and_thinking() {
        let mut s = mk("%1", "claude", "/repo", 1000);
        s.state = State::Thinking;
        s.last_prompt = "fix tests".into();
        s.prompt_started_at = 1100;
        let row = s.format_row(1162);
        assert!(row.contains('◉'), "row: {row}");
        assert!(row.contains("fix tests"));
        assert!(row.contains("thinking"));
        assert!(row.contains("1:02"));
    }

    #[test]
    fn session_format_row_waiting_shows_glyph_and_age() {
        let mut s = mk("%1", "claude", "/repo", 1);
        s.state = State::Waiting;
        s.last_event_ts = 100;
        let row = s.format_row(123);
        assert!(row.contains('⚠'), "row: {row}");
        assert!(row.contains("waiting"));
    }

    #[test]
    fn session_format_row_idle_shows_done_in_and_tools_and_ago() {
        let mut s = mk("%1", "claude", "/repo", 1);
        s.state = State::Idle;
        s.last_event_ts = 1000; // Stop fired here
        s.last_turn_duration = 151; // 2m31s
        s.tools_this_turn = 6;
        let row = s.format_row(1420); // 7m later
        assert!(row.starts_with("· "), "row: {row}");
        assert!(row.contains("done in 2m31s"));
        assert!(row.contains("6 tools"));
        assert!(row.contains("7m ago"));
    }

    #[test]
    fn session_format_row_cold_shows_em_dash() {
        let s = mk("%1", "claude", "/repo", 1);
        assert!(s.format_row(2).contains("·"));
        assert!(s.format_row(2).contains("—"));
    }

    #[test]
    fn session_effective_state_demotes_active_to_stalled_after_silence() {
        let mut s = mk("%1", "claude", "/repo", 1);
        s.state = State::Active;
        s.last_event_ts = 100;
        // Within window: stays Active.
        assert_eq!(s.effective_state(100 + STALL_AFTER_SECS), State::Active);
        // Past window: demoted.
        assert_eq!(
            s.effective_state(100 + STALL_AFTER_SECS + 1),
            State::Stalled
        );
    }

    #[test]
    fn session_state_group_orders_doing_first_idle_last() {
        let mut s = mk("%1", "claude", "/x", 0);
        let now = 1_000_000;
        s.state = State::Active;
        s.last_event_ts = now;
        assert_eq!(s.state_group(now), 0);
        s.state = State::Thinking;
        assert_eq!(s.state_group(now), 1);
        s.state = State::Waiting;
        assert_eq!(s.state_group(now), 2);
        s.state = State::Idle;
        assert_eq!(s.state_group(now), 3);
        s.state = State::Cold;
        assert_eq!(s.state_group(now), 5);
    }

    #[test]
    fn cwd_tail_handles_root_anchored() {
        // Once produced "//repo" because RootDir Component joined as "/".
        assert_eq!(cwd_tail("/home/me/repo/foo"), "repo/foo");
        assert_eq!(cwd_tail("/repo"), "repo");
        assert_eq!(cwd_tail(""), "");
    }

    #[test]
    fn duration_short_formats_across_thresholds() {
        assert_eq!(duration_short(0), "0:00");
        assert_eq!(duration_short(4), "0:04");
        assert_eq!(duration_short(99), "1:39");
        assert_eq!(duration_short(151), "2m31s");
        assert_eq!(duration_short(3600), "1h00m");
        assert_eq!(duration_short(4577), "1h16m");
    }

    #[test]
    fn ago_formats_across_thresholds() {
        assert_eq!(ago(0), "just now");
        assert_eq!(ago(3), "just now");
        assert_eq!(ago(7), "7s ago");
        assert_eq!(ago(120), "2m ago");
        assert_eq!(ago(7200), "2h ago");
    }

    #[test]
    fn tool_input_preview_picks_per_tool_field() {
        use serde_json::json;
        assert_eq!(
            tool_input_preview("Bash", Some(&json!({"command": "cargo test"}))),
            "cargo test"
        );
        assert_eq!(
            tool_input_preview("Edit", Some(&json!({"file_path": "/x/y.rs"}))),
            "/x/y.rs"
        );
        assert_eq!(
            tool_input_preview("UnknownTool", Some(&json!({"command": "x"}))),
            ""
        );
    }

    #[test]
    fn sort_sessions_active_first_thinking_next_idle_last_recent_first() {
        let now = 1_000_000;
        let mut a = mk("%a", "claude", "/x", 100);
        a.state = State::Idle;
        a.state_ts = 200;
        a.last_event_ts = 200;
        let mut b = mk("%b", "claude", "/y", 100);
        b.state = State::Active;
        b.state_ts = 150;
        b.last_event_ts = now - 5; // recent so not stalled, but older than c
        let mut c = mk("%c", "claude", "/z", 100);
        c.state = State::Active;
        c.state_ts = 300;
        c.last_event_ts = now; // most recent activity
        let d = mk("%d", "kiro", "/w", 100); // Cold
        let mut v = vec![a, b, c, d];
        sort_sessions(&mut v, now);
        assert_eq!(
            v.iter().map(|s| s.pane_id.as_str()).collect::<Vec<_>>(),
            vec!["%c", "%b", "%a", "%d"]
        );
    }

    // 4 · Store — read / mutate ──────────────────────────────────

    #[test]
    fn store_read_missing_returns_empty() {
        let (_dir, store) = fixtures();
        assert!(store.read().unwrap().is_empty());
    }

    #[test]
    fn store_mutate_round_trips() {
        let (_dir, store) = fixtures();
        store
            .mutate(|v| {
                v.push(mk("%1", "claude", "/repo", 1000));
                Ok(())
            })
            .unwrap();
        let v = store.read().unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].pane_id, "%1");
    }

    #[test]
    fn store_mutate_observes_prior_state() {
        let (_dir, store) = fixtures();
        store
            .mutate(|v| {
                v.push(mk("%1", "claude", "/x", 1));
                Ok(())
            })
            .unwrap();
        store
            .mutate(|v| {
                assert_eq!(v.len(), 1);
                v.push(mk("%2", "kiro", "/y", 2));
                Ok(())
            })
            .unwrap();
        assert_eq!(store.read().unwrap().len(), 2);
    }

    // 5 · Wrapper — Claude prepare + cleanup ─────────────────────

    #[test]
    fn claude_prepare_passes_argv_through_unchanged() {
        // Claude::prepare is now a no-op — hooks live globally
        // via setup. argv is whatever the user passed after `--`.
        let (dir, store) = fixtures();
        let argv = vec!["claude".to_string(), "--resume".into(), "abc".into()];
        let self_path = PathBuf::from("/test/agent-orch");
        let cwd = dir.path().to_path_buf();
        let ctx = ctx(&store, "%7", &cwd, &argv, &self_path);
        let p = Claude.prepare(&ctx).unwrap();
        assert_eq!(p.program, "claude");
        assert_eq!(p.argv, vec!["claude", "--resume", "abc"]);
        assert!(!p.created_kiro_config);
    }

    #[test]
    fn claude_cleanup_is_a_noop() {
        let (dir, store) = fixtures();
        let removing = mk("%9", "claude", dir.path().to_str().unwrap(), 1);
        // No assertions on the filesystem; the contract is "do nothing,
        // succeed". Just verify it returns Ok.
        Claude.cleanup(&store, &removing, &[]).unwrap();
    }

    // 6a · setup / teardown — Claude user-global hook install ────

    #[test]
    fn setup_creates_settings_with_tagged_entries() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let self_path = PathBuf::from("/test/agent-orch");
        run_setup(&path, &self_path, None).unwrap();

        let v: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        for ev in HOOK_EVENTS {
            let ev = *ev;
            let arr = v["hooks"][ev].as_array().unwrap();
            assert_eq!(arr.len(), 1, "{ev}");
            assert_eq!(arr[0]["matcher"], "");
            assert_eq!(arr[0][AGENT_ORCH_TAG], true);
            assert_eq!(
                arr[0]["hooks"][0]["command"],
                format!("/test/agent-orch hook {}", ev)
            );
            assert_eq!(arr[0]["hooks"][0]["type"], "command");
        }
    }

    #[test]
    fn setup_preserves_user_existing_entries_and_appends_ours() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        // User has their own UserPromptSubmit hook (no tag).
        fs::write(
            &path,
            br#"{
              "permissions": ["read"],
              "hooks": {
                "UserPromptSubmit": [{
                  "matcher": "",
                  "hooks": [{"type":"command","command":"my-hook"}]
                }]
              }
            }"#,
        )
        .unwrap();
        let self_path = PathBuf::from("/test/agent-orch");
        run_setup(&path, &self_path, None).unwrap();

        let v: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        // Sibling fields untouched.
        assert_eq!(v["permissions"][0], "read");
        // User's UserPromptSubmit entry preserved at slot 0; ours appended.
        let ups = v["hooks"]["UserPromptSubmit"].as_array().unwrap();
        assert_eq!(ups.len(), 2);
        assert_eq!(ups[0]["hooks"][0]["command"], "my-hook");
        assert!(ups[0].get(AGENT_ORCH_TAG).is_none());
        assert_eq!(ups[1][AGENT_ORCH_TAG], true);
        // Other events have just our entry.
        for ev in HOOK_EVENTS.iter().filter(|e| **e != EVT_USER_PROMPT_SUBMIT) {
            let arr = v["hooks"][*ev].as_array().unwrap();
            assert_eq!(arr.len(), 1);
            assert_eq!(arr[0][AGENT_ORCH_TAG], true);
        }
    }

    #[test]
    fn setup_idempotent_no_duplicate_entries() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let self_path = PathBuf::from("/test/agent-orch");
        run_setup(&path, &self_path, None).unwrap();
        run_setup(&path, &self_path, None).unwrap();

        let v: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        for ev in HOOK_EVENTS {
            let ev = *ev;
            assert_eq!(
                v["hooks"][ev].as_array().unwrap().len(),
                1,
                "{ev} duplicated"
            );
        }
    }

    #[test]
    fn setup_refreshes_command_path_on_rerun() {
        // Path-refresh case: user moved/rebuilt the binary. Re-running
        // setup with a new self_path rewrites the tagged entry's
        // command, doesn't add a second.
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        run_setup(&path, &PathBuf::from("/old/agent-orch"), None).unwrap();
        run_setup(&path, &PathBuf::from("/new/agent-orch"), None).unwrap();

        let v: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let arr = v["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["hooks"][0]["command"], "/new/agent-orch hook Stop");
    }

    #[test]
    fn setup_rejects_non_object_user_settings_root() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, b"[]").unwrap();
        let err = run_setup(&path, &PathBuf::from("/x/agent-orch"), None).unwrap_err();
        assert!(format!("{err:#}").contains("must be a JSON object"));
    }

    #[test]
    fn setup_rejects_non_array_hooks_event() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, br#"{"hooks":{"Stop":"oops"}}"#).unwrap();
        let err = run_setup(&path, &PathBuf::from("/x/agent-orch"), None).unwrap_err();
        assert!(format!("{err:#}").contains("must be an array"));
    }

    #[test]
    fn teardown_removes_only_tagged_entries_and_preserves_user_ones() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        // Mixed: user entry + ours.
        fs::write(
            &path,
            br#"{
              "permissions": ["read"],
              "hooks": {
                "UserPromptSubmit": [
                  { "matcher": "", "hooks": [{"type":"command","command":"my-hook"}] },
                  { "matcher": "", "hooks": [{"type":"command","command":"/x hook UserPromptSubmit"}], "x-agent-orch-managed": true }
                ],
                "Stop": [
                  { "matcher": "", "hooks": [{"type":"command","command":"/x hook Stop"}], "x-agent-orch-managed": true }
                ]
              }
            }"#,
        )
        .unwrap();
        run_teardown(&path).unwrap();

        let v: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(v["permissions"][0], "read");
        // UserPromptSubmit kept the user's entry, dropped ours.
        let ups = v["hooks"]["UserPromptSubmit"].as_array().unwrap();
        assert_eq!(ups.len(), 1);
        assert_eq!(ups[0]["hooks"][0]["command"], "my-hook");
        // Stop's array became empty → key removed entirely.
        assert!(v["hooks"].get("Stop").is_none());
    }

    #[test]
    fn teardown_removes_file_when_only_our_content_remains() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        run_setup(&path, &PathBuf::from("/x/agent-orch"), None).unwrap();
        assert!(path.exists());
        run_teardown(&path).unwrap();
        assert!(!path.exists(), "file should be removed when result is {{}}");
    }

    #[test]
    fn teardown_keeps_file_when_user_content_remains() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            br#"{"permissions":["read"],"hooks":{"Stop":[{"matcher":"","hooks":[{"type":"command","command":"/x hook Stop"}],"x-agent-orch-managed":true}]}}"#,
        )
        .unwrap();
        run_teardown(&path).unwrap();
        assert!(path.exists());
        let v: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(v["permissions"][0], "read");
        assert!(v.get("hooks").is_none(), "hooks should be pruned");
    }

    #[test]
    fn teardown_noop_on_missing_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("does-not-exist.json");
        run_teardown(&path).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn setup_then_teardown_full_round_trip_restores_pre_state() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let original = br#"{"permissions":["read"],"hooks":{"UserPromptSubmit":[{"matcher":"","hooks":[{"type":"command","command":"my-hook"}]}]}}"#;
        fs::write(&path, original).unwrap();
        run_setup(&path, &PathBuf::from("/x/agent-orch"), None).unwrap();
        run_teardown(&path).unwrap();

        // Re-read and re-parse to compare structurally (formatting may differ).
        let after: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let before: serde_json::Value = serde_json::from_slice(original).unwrap();
        assert_eq!(after, before);
    }

    // 6 · Wrapper — Kiro prepare + cleanup (refcount) ────────────

    #[test]
    fn kiro_prepare_writes_project_config_first_time() {
        let (dir, store) = fixtures();
        let cwd = dir.path().to_path_buf();
        let argv = vec!["kiro".to_string(), "chat".into()];
        let self_path = PathBuf::from("/test/agent-orch");
        let ctx = ctx(&store, "%1", &cwd, &argv, &self_path);
        let p = Kiro.prepare(&ctx).unwrap();

        assert!(p.created_kiro_config);
        let cfg = cwd.join(".kiro").join("agents").join("agent-orch.json");
        assert!(cfg.exists());
        // argv unchanged for kiro
        assert_eq!(p.program, "kiro");
        assert_eq!(p.argv, vec!["kiro", "chat"]);

        let parsed: serde_json::Value = serde_json::from_slice(&fs::read(&cfg).unwrap()).unwrap();
        // Kiro's project config currently mirrors the original Claude
        // 4-event set (the new Notification / PostToolUseFailure events
        // ship to ~/.claude/settings.json only). The wider Kiro hook
        // integration is a separate decision — see project memory.
        for ev in [
            EVT_USER_PROMPT_SUBMIT,
            EVT_PRE_TOOL_USE,
            EVT_POST_TOOL_USE,
            EVT_STOP,
        ] {
            let entry = &parsed["hooks"][ev][0];
            assert_eq!(entry["matcher"], "");
            assert_eq!(
                entry["hooks"][0]["command"],
                format!("/test/agent-orch hook {}", ev)
            );
        }
    }

    #[test]
    fn kiro_prepare_reuser_does_not_stamp_flag() {
        let (dir, store) = fixtures();
        let cwd = dir.path().to_path_buf();
        let argv = vec!["kiro".to_string()];
        let self_path = PathBuf::from("/test/agent-orch");
        // First creates.
        let ctx_a = ctx(&store, "%1", &cwd, &argv, &self_path);
        let pa = Kiro.prepare(&ctx_a).unwrap();
        assert!(pa.created_kiro_config);
        // Second reuses.
        let ctx_b = ctx(&store, "%2", &cwd, &argv, &self_path);
        let pb = Kiro.prepare(&ctx_b).unwrap();
        assert!(!pb.created_kiro_config);
    }

    #[test]
    fn kiro_cleanup_keeps_config_while_sibling_alive() {
        let (dir, store) = fixtures();
        let cwd = dir.path().to_path_buf();
        let cfg = cwd.join(".kiro").join("agents").join("agent-orch.json");
        // Pretend two sessions registered, file exists.
        fs::create_dir_all(cfg.parent().unwrap()).unwrap();
        fs::write(&cfg, b"{}").unwrap();

        let removing = mk("%1", "kiro", cwd.to_str().unwrap(), 1);
        let sibling = mk("%2", "kiro", cwd.to_str().unwrap(), 2);
        Kiro.cleanup(&store, &removing, &[sibling]).unwrap();
        assert!(cfg.exists(), "sibling alive — config must remain");
    }

    #[test]
    fn kiro_cleanup_removes_config_when_last_session_closes_creator_first_ordering() {
        // The load-bearing case: the creator closes first while a
        // reuser remains, so the file stays. Then the reuser closes;
        // its created_kiro_config=false, but cleanup is refcount-
        // agnostic so the file must be removed.
        let (dir, store) = fixtures();
        let cwd = dir.path().to_path_buf();
        let cfg = cwd.join(".kiro").join("agents").join("agent-orch.json");
        fs::create_dir_all(cfg.parent().unwrap()).unwrap();
        fs::write(&cfg, b"{}").unwrap();

        let creator = {
            let mut s = mk("%1", "kiro", cwd.to_str().unwrap(), 1);
            s.created_kiro_config = true;
            s
        };
        let reuser = mk("%2", "kiro", cwd.to_str().unwrap(), 2); // flag false
                                                                 // Creator closes first while reuser is still alive.
        Kiro.cleanup(&store, &creator, std::slice::from_ref(&reuser))
            .unwrap();
        assert!(cfg.exists(), "creator closed but reuser alive — keep file");
        // Reuser closes last.
        Kiro.cleanup(&store, &reuser, &[]).unwrap();
        assert!(
            !cfg.exists(),
            "reuser was last — file must go (refcount-agnostic)"
        );
    }

    // 7 · Wrapper — top-level wrap() integration  ────────────────

    #[test]
    fn wrap_claude_appends_session_record_with_correct_fields() {
        let (dir, store) = fixtures();
        let cwd = dir.path().to_path_buf();
        let argv = vec!["claude-stub".to_string()];
        let self_path = PathBuf::from("/test/agent-orch");
        let ctx = ctx(&store, "%42", &cwd, &argv, &self_path);
        wrap(&Claude, &ctx, false).unwrap();

        let v = store.read().unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].pane_id, "%42");
        assert_eq!(v[0].kind, "claude");
        assert_eq!(v[0].pid, std::process::id() as i32);
        assert_eq!(v[0].state, State::Cold);
        assert!(!v[0].created_kiro_config);
    }

    #[test]
    fn wrap_kiro_stamps_created_flag_and_writes_project_config() {
        let (dir, store) = fixtures();
        let cwd = dir.path().to_path_buf();
        let argv = vec!["kiro-stub".to_string()];
        let self_path = PathBuf::from("/test/agent-orch");
        let ctx = ctx(&store, "%9", &cwd, &argv, &self_path);
        wrap(&Kiro, &ctx, false).unwrap();

        let v = store.read().unwrap();
        assert_eq!(v.len(), 1);
        assert!(v[0].created_kiro_config);
        assert!(cwd.join(".kiro/agents/agent-orch.json").exists());
    }

    #[test]
    fn wrap_refuses_double_register_when_existing_pid_alive() {
        // The wrap path records `std::process::id()` (the test
        // process pid, alive by definition), so a second wrap on
        // the same pane sees an alive sibling and refuses.
        let (dir, store) = fixtures();
        let cwd = dir.path().to_path_buf();
        let argv = vec!["stub".to_string()];
        let self_path = PathBuf::from("/test/agent-orch");
        let ctx = ctx(&store, "%5", &cwd, &argv, &self_path);
        wrap(&Claude, &ctx, false).unwrap();
        let err = wrap(&Claude, &ctx, false).unwrap_err();
        assert!(format!("{err:#}").contains("already registered"));
    }

    /// Spawn `true`, wait for it to exit, return its (now-reaped)
    /// pid. signal-0 against a reaped pid returns ESRCH, so our
    /// `kill(_, None).is_ok()` liveness probe reads it as dead —
    /// exactly what we need to seed a stale record.
    fn dead_pid() -> i32 {
        let mut child = std::process::Command::new("true").spawn().unwrap();
        let pid = child.id() as i32;
        let _ = child.wait();
        // Sanity check: kill(0) must say it's gone.
        assert!(
            nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid), None).is_err(),
            "selected pid {pid} should be dead after wait"
        );
        pid
    }

    #[test]
    fn wrap_replaces_stale_record_with_dead_pid() {
        // An earlier wrap exited (agent crashed, pane stayed alive
        // as an interactive shell, etc.) but the pane-exited hook
        // didn't fire. The next wrap on the same pane should
        // auto-cleanup the stale record and proceed, not refuse.
        let (dir, store) = fixtures();
        let cwd = dir.path().to_path_buf();
        let dp = dead_pid();
        store
            .mutate(|v| {
                let mut s = mk("%5", "claude", cwd.to_str().unwrap(), 1);
                s.pid = dp;
                v.push(s);
                Ok(())
            })
            .unwrap();
        // Pre-condition: stale record exists.
        assert_eq!(store.read().unwrap().len(), 1);

        // Now wrap on the same pane — should silently replace.
        let argv = vec!["stub".to_string()];
        let self_path = PathBuf::from("/test/agent-orch");
        let ctx = ctx(&store, "%5", &cwd, &argv, &self_path);
        wrap(&Claude, &ctx, false).unwrap();

        let v = store.read().unwrap();
        assert_eq!(v.len(), 1, "old record replaced, not duplicated");
        assert_eq!(v[0].pid, std::process::id() as i32);
    }

    #[test]
    fn wrap_replaces_stale_kiro_runs_per_kind_cleanup() {
        // Stale-record replacement also runs the prior kind's
        // cleanup. For Kiro, that's the refcount-agnostic check —
        // and since the stale record was the only kiro session in
        // that cwd, the project config gets removed.
        let (dir, store) = fixtures();
        let cwd = dir.path().to_path_buf();
        let cfg = cwd.join(".kiro").join("agents").join("agent-orch.json");
        fs::create_dir_all(cfg.parent().unwrap()).unwrap();
        fs::write(&cfg, b"{}").unwrap();

        let dp = dead_pid();
        store
            .mutate(|v| {
                let mut s = mk("%9", "kiro", cwd.to_str().unwrap(), 1);
                s.pid = dp;
                s.created_kiro_config = true;
                v.push(s);
                Ok(())
            })
            .unwrap();

        // Wrap a fresh kiro on the same pane. Stale cleanup removes
        // the old config; prepare immediately writes a new one.
        let argv = vec!["kiro".to_string()];
        let self_path = PathBuf::from("/test/agent-orch");
        let ctx = ctx(&store, "%9", &cwd, &argv, &self_path);
        wrap(&Kiro, &ctx, false).unwrap();

        // The new wrap re-created the kiro config (its prepare runs
        // ensure_kiro_config and sees the file absent after stale
        // cleanup ran).
        assert!(cfg.exists());
        let v = store.read().unwrap();
        assert_eq!(v.len(), 1);
        assert!(
            v[0].created_kiro_config,
            "fresh wrap should be the creator since stale cleanup removed the file"
        );
    }

    #[test]
    fn wrap_refuses_empty_agent_argv() {
        let (dir, store) = fixtures();
        let cwd = dir.path().to_path_buf();
        let argv: Vec<String> = vec![];
        let self_path = PathBuf::from("/test/agent-orch");
        let ctx = ctx(&store, "%1", &cwd, &argv, &self_path);
        let err = wrap(&Claude, &ctx, false).unwrap_err();
        assert!(format!("{err:#}").contains("after `--`"));
    }

    // 8 · Wrapper — hook (default trait method) ──────────────────

    #[test]
    fn hook_user_prompt_submit_marks_running_and_stores_prompt() {
        let (dir, store) = fixtures();
        let cwd = dir.path().to_path_buf();
        let argv = vec!["stub".to_string()];
        let self_path = PathBuf::from("/test/agent-orch");
        let ctx = ctx(&store, "%9", &cwd, &argv, &self_path);
        wrap(&Claude, &ctx, false).unwrap();

        let payload = br#"{"prompt":"fix the test"}"#.to_vec();
        Claude
            .hook(
                &store,
                "%9",
                EVT_USER_PROMPT_SUBMIT,
                &mut Cursor::new(payload),
                1234,
            )
            .unwrap();
        let v = store.read().unwrap();
        assert_eq!(v[0].state, State::Thinking);
        assert_eq!(v[0].last_prompt, "fix the test");
    }

    #[test]
    fn hook_stop_marks_idle() {
        let (dir, store) = fixtures();
        let cwd = dir.path().to_path_buf();
        let argv = vec!["stub".to_string()];
        let self_path = PathBuf::from("/test/agent-orch");
        let ctx = ctx(&store, "%1", &cwd, &argv, &self_path);
        wrap(&Claude, &ctx, false).unwrap();

        Claude
            .hook(&store, "%1", EVT_STOP, &mut Cursor::new(b"{}".to_vec()), 99)
            .unwrap();
        assert_eq!(store.read().unwrap()[0].state, State::Idle);
    }

    #[test]
    fn hook_no_op_for_unknown_pane() {
        let (_dir, store) = fixtures();
        // No record for %999 — must not error, must not phantom-write.
        Claude
            .hook(
                &store,
                "%999",
                EVT_STOP,
                &mut Cursor::new(b"{}".to_vec()),
                1,
            )
            .unwrap();
        assert!(store.read().unwrap().is_empty());
    }

    #[test]
    fn hook_default_method_works_via_kiro_impl_too() {
        // Both impls inherit the default method body; verify by
        // dispatching through Kiro on a kiro-registered session.
        let (dir, store) = fixtures();
        let cwd = dir.path().to_path_buf();
        let argv = vec!["stub".to_string()];
        let self_path = PathBuf::from("/test/agent-orch");
        let ctx = ctx(&store, "%7", &cwd, &argv, &self_path);
        wrap(&Kiro, &ctx, false).unwrap();

        Kiro.hook(
            &store,
            "%7",
            EVT_USER_PROMPT_SUBMIT,
            &mut Cursor::new(br#"{"prompt":"hi"}"#.to_vec()),
            42,
        )
        .unwrap();
        assert_eq!(store.read().unwrap()[0].state, State::Thinking);
        assert_eq!(store.read().unwrap()[0].last_prompt, "hi");
    }

    // 9 · unregister + Loop ──────────────────────────────────────

    #[test]
    fn unregister_runs_per_kind_cleanup_via_trait() {
        let (dir, store) = fixtures();
        let cwd = dir.path().to_path_buf();
        let argv = vec!["stub".to_string()];
        let self_path = PathBuf::from("/test/agent-orch");
        let ctx = ctx(&store, "%1", &cwd, &argv, &self_path);
        wrap(&Kiro, &ctx, false).unwrap();
        let cfg = cwd.join(".kiro/agents/agent-orch.json");
        assert!(cfg.exists());

        unregister(&store, "%1").unwrap();
        assert!(store.read().unwrap().is_empty());
        assert!(!cfg.exists(), "Kiro::cleanup should have removed config");
    }

    #[test]
    fn unregister_idempotent_on_unknown_pane() {
        let (_dir, store) = fixtures();
        unregister(&store, "%does-not-exist").unwrap();
    }

    #[test]
    fn loop_render_filters_dead_pids_and_sorts() {
        let (_dir, store) = fixtures();
        store
            .mutate(|v| {
                let mut alive = mk("%alive", "claude", "/x", 1);
                alive.state = State::Active;
                alive.state_ts = 100;
                alive.last_event_ts = now_secs();
                let mut dead = mk("%dead", "claude", "/y", 2);
                dead.pid = 1; // overwhelmingly likely dead/inaccessible
                dead.state = State::Active;
                v.push(alive);
                v.push(dead);
                Ok(())
            })
            .unwrap();

        let rows = Loop::new(&store).render().unwrap();
        // dead pid filtered out; alive remains
        assert_eq!(rows.len(), 1, "rows: {rows:?}");
        assert_eq!(rows[0].0, "%alive");
    }

    #[test]
    fn loop_render_to_emits_tab_separated_rows() {
        // Drives the surface backing `agent-orch render`: each
        // session becomes one `<pane_id>\t<formatted-row>` line.
        // The `\t` split is load-bearing — fzf's `--with-nth=2..`
        // skips the pane id column in display while `--id-nth=1`
        // tracks selection by pane id across reloads.
        let (_dir, store) = fixtures();
        store
            .mutate(|v| {
                let mut s = mk("%alive", "claude", "/repo/foo", 1);
                s.state = State::Thinking;
                s.last_prompt = "do a thing".into();
                s.prompt_started_at = 100;
                s.last_event_ts = now_secs();
                v.push(s);
                Ok(())
            })
            .unwrap();

        let mut buf = Vec::new();
        Loop::new(&store).render_to(&mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 1);
        let mut parts = lines[0].splitn(2, '\t');
        assert_eq!(parts.next().unwrap(), "%alive");
        let row = parts.next().unwrap();
        assert!(row.contains("claude"));
        assert!(row.contains("repo/foo"));
        assert!(row.contains("do a thing"));
    }
}
