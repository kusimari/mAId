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
//!   §4 · `Loop`     — picker (`render`, `render_to`, `peek`,
//!                     `run`, `body`).
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
const EVT_NOTIFICATION: &str = "Notification";
const EVT_STOP: &str = "Stop";

/// Seconds a `Done` record can sit before the picker renders it
/// as `Idle`. Pure render-time decay — never written back to
/// disk, so a forgotten session in a tmux pane doesn't need a
/// background process to age its state file.
const IDLE_THRESHOLD_SECS: u64 = 60;

/// All Claude hook events `setup` installs. Each fires its own
/// `agent-orch hook <event>` invocation; `apply_event` decides
/// which mapped to a state transition.
///
/// `Notification` is load-bearing for the four-state machine —
/// without it the picker can't surface "agent waiting on the
/// user" as a distinct state, so it would never sort to the
/// top of the dashboard. `teardown` is tag-scoped, so legacy
/// entries from earlier installs (without `Notification`) get
/// cleaned up on the next teardown without needing a migration
/// shim.
const HOOK_EVENTS: &[&str] = &[
    EVT_USER_PROMPT_SUBMIT,
    EVT_PRE_TOOL_USE,
    EVT_POST_TOOL_USE,
    EVT_NOTIFICATION,
    EVT_STOP,
];

/// Name of the tmux session that hosts the orchestrator picker.
/// Matches the binary name so a fresh user can find it via
/// `tmux ls`. Anywhere we write `switch-client -t agent-orch`
/// in tmux state (the keybind, the new-session bootstrap, the
/// teardown self-discovery probe) flows from this constant.
const ORCHESTRATOR_SESSION: &str = "agent-orch";

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

/// Lifecycle state stored in `sessions.json`. Three discrete
/// states cover what the hook reporter can decide about an
/// agent at the moment of an event:
///
/// - `Working` — agent is in motion (a tool is running, or
///   a prompt was just submitted).
/// - `Waiting` — agent is blocked on the user (permission
///   prompt or notification).
/// - `Done` — agent finished its turn. May decay to the
///   user-visible `Idle` after `IDLE_THRESHOLD_SECS`,
///   purely at render time.
///
/// `Idle` deliberately is *not* in this enum — keeping the
/// stored-state shape three-variant means a forgotten session
/// doesn't need a background process to flip its state file
/// every minute. The render layer computes `Idle` from
/// `Done + state_ts` against the current time; see
/// [`Session::display_state`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Working,
    Waiting,
    Done,
}

/// User-visible state in the dashboard. A function of the
/// stored [`State`] plus elapsed time. `Done` records past
/// the idle threshold render as `Idle`; everything else
/// passes through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayState {
    Working,
    Waiting,
    Done,
    Idle,
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
    pub last_event: String,
    #[serde(default)]
    pub last_event_ts: u64,
    #[serde(default)]
    pub created_kiro_config: bool,
}

impl Session {
    /// Apply one hook event to this session. Always bumps
    /// `last_event` / `last_event_ts` so the elapsed-time
    /// column reflects the most recent activity, even for
    /// events that don't trigger a state change.
    ///
    /// Mapping (see spec → "How the lifecycle states are
    /// derived"):
    ///
    /// - `UserPromptSubmit` → `Working` (transitional —
    ///   the user just handed the agent a task; if no
    ///   `PreToolUse` follows, the next `Stop` flips to
    ///   `Done`).
    /// - `PreToolUse`       → `Working`
    /// - `PostToolUse`      → `Working` (more tools may
    ///   follow in the same turn — only `Stop` ends the
    ///   turn).
    /// - `Notification`     → `Waiting` (highest priority
    ///   in the dashboard sort — agent is blocked on the
    ///   user).
    /// - `Stop`             → `Done`.
    ///
    /// Unknown events bump `last_event_ts` only — silent
    /// no-op for state. Hooks must never block the agent's
    /// turn, so this body has no fallible paths.
    ///
    /// Payload is unused today but kept in the signature so
    /// a future kind whose state machine cares about
    /// `tool_name` / `tool_input` can deserialise without
    /// changing the trait method.
    fn apply_event(&mut self, event: &str, _payload: &serde_json::Value, now: u64) {
        self.last_event = event.into();
        self.last_event_ts = now;
        match event {
            EVT_USER_PROMPT_SUBMIT | EVT_PRE_TOOL_USE | EVT_POST_TOOL_USE => {
                self.state = State::Working;
                self.state_ts = now;
            }
            EVT_NOTIFICATION => {
                self.state = State::Waiting;
                self.state_ts = now;
            }
            EVT_STOP => {
                self.state = State::Done;
                self.state_ts = now;
            }
            // Unknown event: only the activity timestamp moves.
            // Lets sort track "recently driven by something" even
            // for events we don't model yet (e.g. a future
            // PostToolUseFailure variant we haven't wired up).
            _ => {}
        }
    }

    /// Resolve the user-visible state at render time. Stored
    /// `Done` records age into `Idle` once they've sat past
    /// [`IDLE_THRESHOLD_SECS`] since their last state change.
    /// Pure function — same input ⇒ same output.
    fn display_state(&self, now: u64) -> DisplayState {
        match self.state {
            State::Working => DisplayState::Working,
            State::Waiting => DisplayState::Waiting,
            State::Done => {
                if now.saturating_sub(self.state_ts) > IDLE_THRESHOLD_SECS {
                    DisplayState::Idle
                } else {
                    DisplayState::Done
                }
            }
        }
    }

    /// Single-line picker row. **Slice 1 placeholder shape** —
    /// Slice 2 of the fix-branch replaces this with a multi-
    /// line item carrying the tmux address, elapsed time, and
    /// pane snippet. Kept here so existing call sites compile
    /// while the four-state machine lands. Format:
    /// `<glyph> <kind> <cwd-tail>`.
    fn format_row(&self, now: u64) -> String {
        let glyph = state_glyph(self.display_state(now));
        format!("{} {} {}", glyph, self.kind, cwd_tail(&self.cwd))
    }

    /// "Active at" — max(state_ts, last_event_ts, started). Used by sort.
    fn activity(&self) -> u64 {
        self.state_ts.max(self.last_event_ts).max(self.started)
    }

    /// Sort priority. Top of the dashboard down to the
    /// bottom: waiting (0) → done (1) → idle (2) → working
    /// (3). The dashboard is a triage queue; sort by "does
    /// this agent need the user's attention?" not "is this
    /// agent active?". Working agents are self-managing
    /// and sink to the bottom; idle agents are forgotten
    /// assets that beat working because they need a
    /// decision (give a task or close the pane).
    fn priority(&self, now: u64) -> u8 {
        match self.display_state(now) {
            DisplayState::Waiting => 0,
            DisplayState::Done => 1,
            DisplayState::Idle => 2,
            DisplayState::Working => 3,
        }
    }
}

/// Glyph for a [`DisplayState`]. Centralised so the picker
/// row, the unit tests, and any future status-bar exporter
/// agree on what each state looks like.
fn state_glyph(state: DisplayState) -> &'static str {
    match state {
        DisplayState::Waiting => "💬",
        DisplayState::Done => "✓",
        DisplayState::Idle => "·",
        DisplayState::Working => "▶",
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

/// Sort by `Session::priority(now)` (waiting, then done,
/// then idle, then working), with ties broken by
/// most-recently-active first. The decay layer means a
/// `Done` record's effective sort position depends on `now`;
/// pass `now` so a single render pass uses one consistent
/// timestamp for every comparison.
fn sort_sessions(sessions: &mut [Session], now: u64) {
    sessions.sort_by(|a, b| {
        let prio = a.priority(now).cmp(&b.priority(now));
        if prio.is_ne() {
            return prio;
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
/// → `<prefix> O` runs `switch-client -t agent-orch`). When
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
/// binding whose action is `switch-client -t agent-orch` —
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
/// `switch-client -t agent-orch` on the running tmux server.
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
/// binding whose action is `switch-client -t agent-orch` and
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
    //   bind-key    -T prefix O       switch-client -t agent-orch
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
            // Initial state: `Done`. No events have fired
            // yet, so the agent isn't actively working;
            // putting it in `Done` lets the picker's
            // priority sort treat it the same as a finished
            // turn (worth glancing at, not blocking).
            // Decays to `Idle` after the threshold passes.
            state: State::Done,
            state_ts: now,
            last_event: String::new(),
            last_event_ts: 0,
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

    /// Read sessions, filter live pids, sort by priority, format rows.
    /// Returns `(pane_id, formatted_row)` pairs. The single
    /// `now` snapshot is passed through the sort and format
    /// calls so `Done` records' decay-to-`Idle` boundary is
    /// consistent across all rows in one render pass.
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

    /// Capture the last `lines` of pane `pane_id` via
    /// `tmux capture-pane` and write the raw output to `out`.
    /// Used by fzf's `--preview` action — the preview window
    /// shows the agent's actual screen content, which is the
    /// load-bearing UX signal (state machine is just a glyph).
    /// Routes through `tmux_cmd` so `$AGENT_ORCH_TMUX_SOCKET`
    /// (set by the integration script) targets the test server.
    pub fn peek(&self, pane_id: &str, lines: u32, out: &mut dyn Write) -> Result<()> {
        let start = format!("-{}", lines);
        let output = tmux_cmd(&[
            "capture-pane",
            "-p",
            "-t",
            pane_id,
            "-E",
            "-1",
            "-S",
            &start,
        ])
        .stdout(std::process::Stdio::piped())
        .output()
        .context("tmux capture-pane")?;
        // Pane went away (closed, server restart) → empty output,
        // not an error. fzf still gets a clean (empty) preview.
        if output.status.success() {
            out.write_all(&output.stdout)?;
        }
        Ok(())
    }

    /// Event-driven picker loop body. Runs inside the
    /// orchestrator session.
    ///
    /// fzf is configured with:
    /// - `--listen=<sock>` — accept control commands over a UDS.
    /// - `--with-nth=2..` — show every column except the pane id.
    /// - `--track --id-nth=1` — keep the cursor on the same pane
    ///   id across reloads.
    /// - `--preview '<self> peek {1}'` — show the focused agent's
    ///   tmux pane content in a side window. This is the truth
    ///   signal: glyph at-a-glance, preview for everything else.
    /// - `enter:execute-silent(tmux switch-client -t {1})+clear-query`
    ///   — non-terminal binding. fzf stays alive across selections.
    ///
    /// Two background threads drive updates over the listen
    /// socket:
    /// - **Watcher** — `notify-debouncer-mini` on the store dir
    ///   posts `reload(<self> render)` only when sessions.json
    ///   actually changes. The list refresh is needed because
    ///   rows can appear / disappear / change state.
    /// - **Heartbeat** — every 1 second, posts `refresh-preview`.
    ///   This re-runs the preview command for the focused row but
    ///   does **not** touch the list — the cursor stays put, the
    ///   query stays put, the prompt stays bright. `reload(...)`
    ///   blocks fzf's input briefly while it re-runs the source;
    ///   doing that at 1 Hz produced continuous flicker. Splitting
    ///   the two actions kills the flicker without losing the
    ///   live-preview feel.
    pub fn body(&self, self_path: &Path) -> Result<()> {
        let sock_path = pick_listen_socket();
        let sock = sock_path.to_string_lossy().to_string();
        let render_cmd = format!("{} render", self_path.display());
        let preview_cmd = format!("{} peek {{1}}", self_path.display());
        let bind = "enter:execute-silent(tmux switch-client -t {1})+clear-query";

        let mut child = std::process::Command::new("fzf")
            .args([
                &format!("--listen={sock}"),
                "--with-nth=2..",
                "--track",
                "--id-nth=1",
                "--preview",
                &preview_cmd,
                "--preview-window=right:50%",
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

        // Single mpsc channel carries `Tick` enums. The watcher
        // sends `Tick::Reload` on real registry change; the
        // heartbeat sends `Tick::RefreshPreview` once a second.
        // Main thread routes each to the corresponding fzf action.
        let (tx, rx) = mpsc::channel::<Tick>();

        let watch_dir = self.store.dir().to_path_buf();
        let watcher_tx = tx.clone();
        thread::spawn(move || {
            if let Err(e) = run_watcher(&watch_dir, watcher_tx) {
                eprintln!("agent-orch watcher: {e:#}");
            }
        });

        let heartbeat_tx = tx;
        thread::spawn(move || {
            while heartbeat_tx.send(Tick::RefreshPreview).is_ok() {
                thread::sleep(Duration::from_secs(1));
            }
        });

        loop {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(tick) => {
                    let mut want_reload = matches!(tick, Tick::Reload);
                    let mut want_refresh = matches!(tick, Tick::RefreshPreview);
                    // Drain any backlog so a burst of identical
                    // ticks coalesces into one outgoing action.
                    while let Ok(more) = rx.try_recv() {
                        match more {
                            Tick::Reload => want_reload = true,
                            Tick::RefreshPreview => want_refresh = true,
                        }
                    }
                    if want_reload {
                        if let Err(e) = push_action(&sock_path, &format!("reload({render_cmd})")) {
                            eprintln!("agent-orch reload: {e:#}");
                        }
                        // After a reload, the focused row may have
                        // shifted; refresh the preview too so it
                        // tracks the new selection.
                        want_refresh = true;
                    }
                    if want_refresh {
                        if let Err(e) = push_action(&sock_path, "refresh-preview") {
                            eprintln!("agent-orch refresh-preview: {e:#}");
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    // Watcher AND heartbeat both gone — fzf
                    // selections still work, just no live updates.
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

/// Tick reasons the picker main loop receives. Drives whether
/// we send fzf a `reload(...)` (rebuild the list) or a
/// `refresh-preview` (rerun preview only — no flicker).
enum Tick {
    /// Real registry change observed by the notify watcher.
    Reload,
    /// 1 Hz heartbeat from the preview-refresh thread.
    RefreshPreview,
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
/// `Tick::Reload` per debounced batch. Debounce window is 100ms
/// — short enough that the picker feels live, long enough to
/// coalesce the tmp+rename pair an atomic write produces.
fn run_watcher(dir: &Path, tx: mpsc::Sender<Tick>) -> Result<()> {
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
        if interesting && tx.send(Tick::Reload).is_err() {
            // main loop dropped the receiver — fzf exited
            break;
        }
    }
    Ok(())
}

/// Push an action string to fzf's listen socket. fzf speaks
/// HTTP/1.1 over the UDS; the request body is the action,
/// no special framing beyond Content-Length. Used for both
/// `reload(...)` and `refresh-preview`.
fn push_action(sock: &Path, action: &str) -> Result<()> {
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
        action.len(),
        action
    );
    stream
        .write_all(request.as_bytes())
        .context("write fzf action")?;
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
    /// `switch-client -t agent-orch` — no `--key` argument needed.
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
    /// Print the last N lines of pane <pane-id> via
    /// `tmux capture-pane`. Used by the loop body's
    /// fzf --preview action.
    #[command(hide = true)]
    Peek {
        pane_id: String,
        #[arg(long, default_value_t = 10)]
        lines: u32,
    },
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

        Some(Cmd::Peek { pane_id, lines }) => {
            let mut stdout = std::io::stdout();
            Loop::new(&store).peek(&pane_id, lines, &mut stdout)
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
            // Default fresh-wrap state — the picker sorts these
            // into the middle bucket (Done) until they decay to
            // Idle or get driven into Working/Waiting.
            state: State::Done,
            state_ts: started,
            last_event: String::new(),
            last_event_ts: 0,
            created_kiro_config: false,
        }
    }

    /// Empty payload — apply_event ignores the fields today, but
    /// the signature still takes a `&serde_json::Value`.
    fn empty_payload() -> serde_json::Value {
        serde_json::json!({})
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

    // 3 · Session — apply_event + display_state + sort ──────────

    #[test]
    fn apply_event_pre_tool_use_marks_working() {
        let mut s = mk("%1", "claude", "/repo", 1000);
        assert_eq!(s.state, State::Done); // default fresh-wrap state
        s.apply_event(EVT_PRE_TOOL_USE, &empty_payload(), 1200);
        assert_eq!(s.state, State::Working);
        assert_eq!(s.last_event, "PreToolUse");
        assert_eq!(s.last_event_ts, 1200);
        assert_eq!(s.state_ts, 1200);
    }

    #[test]
    fn apply_event_post_tool_use_keeps_working() {
        // PostToolUse keeps Working — more tools may follow in
        // the same turn. Only Stop ends the turn.
        let mut s = mk("%1", "claude", "/repo", 1000);
        s.apply_event(EVT_PRE_TOOL_USE, &empty_payload(), 1200);
        assert_eq!(s.state, State::Working);
        s.apply_event(EVT_POST_TOOL_USE, &empty_payload(), 1250);
        assert_eq!(s.state, State::Working);
    }

    #[test]
    fn apply_event_user_prompt_submit_marks_working() {
        // The user just handed Claude a task — agent is in
        // motion until something tells us otherwise.
        let mut s = mk("%1", "claude", "/repo", 1000);
        s.state = State::Done;
        s.apply_event(EVT_USER_PROMPT_SUBMIT, &empty_payload(), 1100);
        assert_eq!(s.state, State::Working);
        assert_eq!(s.last_event, "UserPromptSubmit");
    }

    #[test]
    fn apply_event_notification_marks_waiting() {
        // The load-bearing case for the four-state machine —
        // permission prompts have to flip to Waiting, which is
        // what gets sorted to the top of the dashboard.
        let mut s = mk("%1", "claude", "/repo", 1000);
        s.apply_event(EVT_NOTIFICATION, &empty_payload(), 1500);
        assert_eq!(s.state, State::Waiting);
        assert_eq!(s.last_event, "Notification");
    }

    #[test]
    fn apply_event_stop_marks_done() {
        let mut s = mk("%1", "claude", "/repo", 1000);
        s.apply_event(EVT_PRE_TOOL_USE, &empty_payload(), 1100);
        s.apply_event(EVT_STOP, &empty_payload(), 1300);
        assert_eq!(s.state, State::Done);
    }

    #[test]
    fn apply_event_unknown_event_bumps_event_ts_only() {
        // Unknown events still update last_event/last_event_ts (so
        // sort sees the activity), but don't change state.
        let mut s = mk("%1", "claude", "/repo", 1000);
        let prior_state = s.state.clone();
        let prior_state_ts = s.state_ts;
        s.apply_event("SomethingWeDontKnow", &empty_payload(), 9999);
        assert_eq!(s.state, prior_state);
        assert_eq!(s.state_ts, prior_state_ts);
        assert_eq!(s.last_event, "SomethingWeDontKnow");
        assert_eq!(s.last_event_ts, 9999);
    }

    #[test]
    fn display_state_decays_done_to_idle_after_threshold() {
        // The user-visible Idle state is render-time decay over
        // a stored Done. Past the threshold, display flips to
        // Idle without touching the stored state.
        let mut s = mk("%1", "claude", "/repo", 1000);
        s.state = State::Done;
        s.state_ts = 1000;

        // Inside the threshold: still Done.
        assert_eq!(
            s.display_state(1000 + IDLE_THRESHOLD_SECS),
            DisplayState::Done
        );
        // Past the threshold: flips to Idle.
        assert_eq!(
            s.display_state(1000 + IDLE_THRESHOLD_SECS + 1),
            DisplayState::Idle
        );
    }

    #[test]
    fn display_state_does_not_decay_working_or_waiting() {
        // Only Done decays. Working and Waiting are explicit
        // signals; no time-based reinterpretation.
        let mut s = mk("%1", "claude", "/repo", 1000);
        s.state = State::Working;
        s.state_ts = 1000;
        assert_eq!(s.display_state(99_999_999), DisplayState::Working);

        s.state = State::Waiting;
        assert_eq!(s.display_state(99_999_999), DisplayState::Waiting);
    }

    #[test]
    fn priority_order_is_waiting_done_idle_working() {
        // Sort by attention-needed: waiting > done > idle > working.
        // Working sinks to the bottom because actively-progressing
        // agents are most self-managing; idle beats working because
        // a forgotten agent needs a decision.
        let now = 100_000;
        let mut s = mk("%1", "claude", "/repo", 0);

        s.state = State::Waiting;
        s.state_ts = now;
        assert_eq!(s.priority(now), 0);

        s.state = State::Done;
        s.state_ts = now;
        assert_eq!(s.priority(now), 1);

        // Same Done record, but aged past the idle threshold:
        // priority becomes 2 (idle bucket) without changing storage.
        s.state_ts = now - IDLE_THRESHOLD_SECS - 1;
        assert_eq!(s.priority(now), 2);

        s.state = State::Working;
        s.state_ts = now;
        assert_eq!(s.priority(now), 3);
    }

    #[test]
    fn format_row_emits_state_glyph_kind_and_cwd_tail() {
        // Slice 1 keeps the placeholder single-line shape;
        // Slice 2 will replace this with a multi-line item.
        let mut s = mk("%1", "claude", "/home/me/repo/foo", 1000);
        s.state = State::Working;
        let row = s.format_row(1000);
        assert!(row.starts_with("▶ "), "row: {row}");
        assert!(row.contains("claude"));
        assert!(row.contains("repo/foo"));

        s.state = State::Waiting;
        assert!(s.format_row(1000).starts_with("💬 "));

        s.state = State::Done;
        assert!(s.format_row(1000).starts_with("✓ "));

        // Done aged past the threshold renders as Idle.
        s.state_ts = 0;
        assert!(s.format_row(IDLE_THRESHOLD_SECS + 100).starts_with("· "));
    }

    #[test]
    fn cwd_tail_handles_root_anchored() {
        // Once produced "//repo" because RootDir Component joined as "/".
        assert_eq!(cwd_tail("/home/me/repo/foo"), "repo/foo");
        assert_eq!(cwd_tail("/repo"), "repo");
        assert_eq!(cwd_tail(""), "");
    }

    #[test]
    fn sort_sessions_priority_first_recency_within_bucket() {
        let now = 1_000_000;

        // a — Done (recent), bucket 1
        let mut a = mk("%a", "claude", "/x", 100);
        a.state = State::Done;
        a.state_ts = now - 10;
        a.last_event_ts = now - 10;

        // b — Working (most recent), bucket 3
        let mut b = mk("%b", "claude", "/y", 100);
        b.state = State::Working;
        b.state_ts = now;
        b.last_event_ts = now;

        // c — Waiting, bucket 0 (top)
        let mut c = mk("%c", "claude", "/z", 100);
        c.state = State::Waiting;
        c.state_ts = now - 5;
        c.last_event_ts = now - 5;

        // d — Done aged past threshold → Idle, bucket 2
        let mut d = mk("%d", "kiro", "/w", 100);
        d.state = State::Done;
        d.state_ts = now - IDLE_THRESHOLD_SECS - 100;
        d.last_event_ts = now - IDLE_THRESHOLD_SECS - 100;

        // e — Working but older than b, still bucket 3 (after d).
        let mut e = mk("%e", "claude", "/v", 100);
        e.state = State::Working;
        e.state_ts = now - 50;
        e.last_event_ts = now - 50;

        let mut v = vec![a, b, c, d, e];
        sort_sessions(&mut v, now);
        assert_eq!(
            v.iter().map(|s| s.pane_id.as_str()).collect::<Vec<_>>(),
            vec!["%c", "%a", "%d", "%b", "%e"],
            "expected waiting > done > idle > working, recency within"
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
        // Fresh wrap with no events fired yet sits in Done.
        assert_eq!(v[0].state, State::Done);
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
    fn hook_pre_tool_use_marks_running() {
        let (dir, store) = fixtures();
        let cwd = dir.path().to_path_buf();
        let argv = vec!["stub".to_string()];
        let self_path = PathBuf::from("/test/agent-orch");
        let ctx = ctx(&store, "%9", &cwd, &argv, &self_path);
        wrap(&Claude, &ctx, false).unwrap();

        Claude
            .hook(
                &store,
                "%9",
                EVT_PRE_TOOL_USE,
                &mut Cursor::new(b"{}".to_vec()),
                1234,
            )
            .unwrap();
        let v = store.read().unwrap();
        assert_eq!(v[0].state, State::Working);
        assert_eq!(v[0].last_event_ts, 1234);
    }

    #[test]
    fn hook_stop_marks_done() {
        let (dir, store) = fixtures();
        let cwd = dir.path().to_path_buf();
        let argv = vec!["stub".to_string()];
        let self_path = PathBuf::from("/test/agent-orch");
        let ctx = ctx(&store, "%1", &cwd, &argv, &self_path);
        wrap(&Claude, &ctx, false).unwrap();
        // Drive into Working first so we can observe the flip back.
        Claude
            .hook(
                &store,
                "%1",
                EVT_PRE_TOOL_USE,
                &mut Cursor::new(b"{}".to_vec()),
                50,
            )
            .unwrap();
        assert_eq!(store.read().unwrap()[0].state, State::Working);

        Claude
            .hook(&store, "%1", EVT_STOP, &mut Cursor::new(b"{}".to_vec()), 99)
            .unwrap();
        assert_eq!(store.read().unwrap()[0].state, State::Done);
    }

    #[test]
    fn hook_notification_marks_waiting() {
        // The load-bearing case for the four-state machine —
        // permission prompts must surface via the hook reporter
        // as Waiting so they sort to the top of the dashboard.
        let (dir, store) = fixtures();
        let cwd = dir.path().to_path_buf();
        let argv = vec!["stub".to_string()];
        let self_path = PathBuf::from("/test/agent-orch");
        let ctx = ctx(&store, "%1", &cwd, &argv, &self_path);
        wrap(&Claude, &ctx, false).unwrap();

        Claude
            .hook(
                &store,
                "%1",
                EVT_NOTIFICATION,
                &mut Cursor::new(b"{}".to_vec()),
                250,
            )
            .unwrap();
        let s = &store.read().unwrap()[0];
        assert_eq!(s.state, State::Waiting);
        assert_eq!(s.last_event, "Notification");
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
            EVT_PRE_TOOL_USE,
            &mut Cursor::new(b"{}".to_vec()),
            42,
        )
        .unwrap();
        assert_eq!(store.read().unwrap()[0].state, State::Working);
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
                alive.state = State::Working;
                alive.state_ts = 100;
                alive.last_event_ts = now_secs();
                let mut dead = mk("%dead", "claude", "/y", 2);
                dead.pid = 1; // overwhelmingly likely dead/inaccessible
                dead.state = State::Working;
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
                s.state = State::Working;
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
        assert!(row.contains("▶"));
        assert!(row.contains("claude"));
        assert!(row.contains("repo/foo"));
    }

    #[test]
    fn loop_peek_emits_empty_for_unknown_pane() {
        // peek shells out to `tmux capture-pane`. With a bogus pane
        // id, tmux exits non-zero; peek's contract is "graceful empty
        // output, no error". (The real-pane path is exercised by the
        // integration script.)
        let (_dir, store) = fixtures();
        let mut buf = Vec::new();
        Loop::new(&store)
            .peek("%not-a-real-pane", 5, &mut buf)
            .unwrap();
        assert!(
            buf.is_empty(),
            "expected empty output, got {} bytes",
            buf.len()
        );
    }
}
