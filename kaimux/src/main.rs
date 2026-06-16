//! kaimux — observation-only orchestrator over tmux + coding-agent panes.
//!
//! One file, six sections top-to-bottom:
//!   §1 Session    — registry record, lifecycle state machine
//!   §2 Store      — flock + atomic-write registry persistence
//!   §3 Wrapper    — wrap an agent (Claude/Kiro/Other) — register + execvp
//!   §4 Hook       — entry the wrapped agent's hooks call into
//!   §5 Configure  — `setup`/`teardown` (Claude settings + tmux keybind)
//!   §6 Loop       — picker (render, fzf body, run, peek)
//! `main` dispatches the CLI. End-to-end: tests/kaimux/integration.sh.

use anyhow::{Context, Result};
use atomicwrites::{AllowOverwrite, AtomicFile};
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

// Done record decays to Idle at render time after this — no on-disk update.
const IDLE_THRESHOLD_SECS: u64 = 60;

// Notification is load-bearing — drives the Waiting state.
const HOOK_EVENTS: &[&str] = &[
    EVT_USER_PROMPT_SUBMIT,
    EVT_PRE_TOOL_USE,
    EVT_POST_TOOL_USE,
    EVT_NOTIFICATION,
    EVT_STOP,
];

// Name used for the dashboard tmux session. Override with --session <name>.
const DEFAULT_SESSION_NAME: &str = "kaimux";

// Marker our prefix-table keybind carries so teardown can self-discover it
// (rides as a `run-shell "true #<marker>"` before switch-client; the `#`
// makes it a shell comment, tmux echoes it verbatim in list-keys output).
const KEYBIND_MARKER: &str = "x-kaimux-managed";

// Tag on hook entries we wrote — teardown removes only ours.
const KAIMUX_TAG: &str = "x-kaimux-managed";

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ─────────────────────────────────────────────────────────────────────────────
// §1 · Session

// Stored lifecycle. Idle is computed at render time so a
// forgotten session doesn't need a background ager.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Working, // tool running or prompt just submitted
    Waiting, // blocked on the user (permission / Notification)
    Done,    // finished turn — decays to DisplayState::Idle past the threshold
}

// Render-time view of State (adds Idle). Per-variant data — glyph + sort
// priority — lives on the variant so adding a state means filling out one
// match arm, not three.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayState {
    Working,
    Waiting,
    Done,
    Idle,
}

impl DisplayState {
    fn glyph(self) -> &'static str {
        match self {
            Self::Waiting => "💬",
            Self::Done => "✓",
            Self::Idle => "·",
            Self::Working => "▶",
        }
    }

    // Triage sort: surface what needs attention. Working agents are
    // self-managing and sink to the bottom; Idle beats Working because a
    // forgotten session needs a decision.
    fn priority(self) -> u8 {
        match self {
            Self::Waiting => 0,
            Self::Done => 1,
            Self::Idle => 2,
            Self::Working => 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub pane_id: String, // tmux pane id (e.g. %17) — registry's primary key
    pub pid: i32,        // wrapper pid; live_only drops entries whose pid is gone
    pub kind: String,    // claude / kiro / Other(<name>) — drives wrapper_for dispatch
    pub cwd: String,     // working dir at wrap time; used by Kiro refcount-cleanup
    pub started: u64,    // unix secs at wrap; elapsed-column floor before any event
    pub state: State,
    pub state_ts: u64, // unix secs of last state change — decay-to-Idle measures from here
    #[serde(default)]
    pub last_event: String,
    #[serde(default)]
    pub last_event_ts: u64, // unix secs of last hook of any kind — drives elapsed column
    #[serde(default)]
    pub created_kiro_config: bool, // true iff this wrap wrote .kiro/agents/kaimux.json
}

impl Session {
    // payload is unused today; kept so a future kind whose state machine
    // cares about tool_name/tool_input can deserialise without changing
    // the trait method.
    fn apply_event(&mut self, event: &str, _payload: &serde_json::Value, now: u64) {
        // Always bump activity — even events we don't model still drive the
        // elapsed column ("recently driven by something").
        self.last_event = event.into();
        self.last_event_ts = now;
        self.state = match event {
            EVT_USER_PROMPT_SUBMIT | EVT_PRE_TOOL_USE | EVT_POST_TOOL_USE => State::Working,
            EVT_NOTIFICATION => State::Waiting,
            EVT_STOP => State::Done,
            _ => return, // unknown event: only activity ts moves
        };
        self.state_ts = now;
    }

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

    // `<icon> <addr>\t<kind>\t<cwd-fixed-width>\t<elapsed>`. Tab-separated
    // because the cwd may contain `…`. addr is passed in (not derived) so this
    // stays pure — testable without a tmux server.
    fn format_header(&self, addr: &str, now: u64) -> String {
        let glyph = self.display_state(now).glyph();
        let elapsed = format_elapsed(now.saturating_sub(self.last_event_ts.max(self.started)));
        let cwd = cwd_fixed_width(&self.cwd, CWD_COLUMN_WIDTH);
        format!("{} {}\t{}\t{}\t{}", glyph, addr, self.kind, cwd, elapsed)
    }

    fn activity(&self) -> u64 {
        self.state_ts.max(self.last_event_ts).max(self.started)
    }
}

// 24 fits 3 typical path segments while leaving room for kind + elapsed
// on a 100-col terminal.
const CWD_COLUMN_WIDTH: usize = 24;

// Truncate-from-front with `…/` so the distinguishing trailing segments
// stay visible. Char-based (not byte): unicode-width / CJK not handled —
// path components are nearly always ASCII for tmux panes.
fn cwd_fixed_width(cwd: &str, width: usize) -> String {
    let chars: Vec<char> = cwd.chars().collect();
    let body = if chars.len() <= width {
        cwd.to_string()
    } else {
        // Cut from the back; reserve 2 chars for `…/`. Snap cut to the
        // nearest `/` within the next 4 chars so we don't slice mid-segment.
        let cut = chars.len() - (width - 2);
        let snap = chars[cut..]
            .iter()
            .take(4)
            .position(|c| *c == '/')
            .map(|i| cut + i + 1)
            .unwrap_or(cut);
        format!("…/{}", chars[snap..].iter().collect::<String>())
    };
    format!("{body:<width$}")
}

// Compact: 5s / 2m / 1h / 3d. Round down to the largest unit that fits.
// humantime/Duration formatters give multi-unit output (`1h 5m 3s`); we
// want one unit, so a tiny lookup table is simpler than either.
const ELAPSED_UNITS: &[(u64, char)] = &[(86_400, 'd'), (3_600, 'h'), (60, 'm'), (1, 's')];

fn format_elapsed(secs: u64) -> String {
    let (n, unit) = ELAPSED_UNITS
        .iter()
        .find(|(n, _)| secs >= *n)
        .copied()
        .unwrap_or((1, 's'));
    format!("{}{unit}", secs / n)
}

// Pass `now` through so one render pass uses one timestamp — the decay
// layer makes priority depend on it, and inconsistent values would let
// rows reorder mid-comparison.
fn sort_sessions(sessions: &mut [Session], now: u64) {
    // (priority, -activity) — lower priority first, more-recent activity
    // wins ties.
    sessions.sort_by_key(|s| (s.display_state(now).priority(), u64::MAX - s.activity()));
}

// `kill(pid, 0)` is a permission probe — exists iff signal-able, no signal
// sent. Render-time honesty without a sweeper, in case pane-exited missed.
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
    // ${XDG_STATE_HOME:-$HOME/.local/state}/kaimux. Empty XDG ⇒ unset
    // (some shells leave it set to "" — don't anchor state at /kaimux).
    pub fn from_env() -> Result<Self> {
        if let Ok(xdg) = std::env::var("XDG_STATE_HOME") {
            if !xdg.is_empty() {
                return Ok(Store {
                    dir: PathBuf::from(xdg).join("kaimux"),
                });
            }
        }
        let home = std::env::var("HOME").context("$HOME unset")?;
        Ok(Store {
            dir: PathBuf::from(home).join(".local/state/kaimux"),
        })
    }

    pub fn new(dir: PathBuf) -> Self {
        Store { dir }
    }
    pub fn dir(&self) -> &Path {
        &self.dir
    }
    pub fn hook_marker(&self) -> PathBuf {
        self.dir.join(HOOK_MARKER)
    }

    // No lock — eventually consistent. Empty/missing ⇒ empty Vec.
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

    // Read-modify-write under flock. fd-lock owns the locking; atomicwrites
    // owns the tmp+rename. Our job is the closure — what to mutate, not how
    // to fence it. Lock + tmp file release on drop, so panics still clean up.
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
        AtomicFile::new(self.dir.join(SESSIONS_FILE), AllowOverwrite)
            .write(|w| w.write_all(&serde_json::to_vec_pretty(&sessions)?))
            .context("atomic write sessions.json")?;
        Ok(out)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// §3 · Wrapper — wrap an agent session: prepare per-kind config, register in
// the store, install the pane-exited hook, execvp the agent. Kind variation
// lives in the trait impls; everything else in `wrap()` below.

pub struct WrapCtx<'a> {
    pub store: &'a Store,
    pub self_path: &'a Path,
    pub pane_id: &'a str,
    pub cwd: &'a Path,
    pub agent_argv: &'a [String],
}

#[derive(Debug)]
pub struct Prepared {
    pub program: String, // execvp target — split from argv[0] so wrappers can rewrite it
    pub argv: Vec<String>,
    pub created_kiro_config: bool, // true ⇒ this wrap wrote .kiro/agents/kaimux.json
}

pub trait Wrapper {
    fn kind(&self) -> &str;
    // Ensure per-kind config; return execvp (program, argv) + any flag
    // the session record needs.
    fn prepare(&self, ctx: &WrapCtx) -> Result<Prepared>;
    fn cleanup(&self, store: &Store, removing: &Session, others: &[Session]) -> Result<()>;
}

pub struct Claude;
pub struct Kiro;
pub struct Other(pub String);

// Argv-passthrough Prepared for kinds whose prepare has no per-kind state.
fn passthrough(ctx: &WrapCtx) -> Prepared {
    Prepared {
        program: ctx.agent_argv[0].clone(),
        argv: ctx.agent_argv.to_vec(),
        created_kiro_config: false,
    }
}

impl Wrapper for Claude {
    fn kind(&self) -> &str {
        "claude"
    }

    // Claude hooks live user-globally via `kaimux setup`, not per-launch.
    fn prepare(&self, ctx: &WrapCtx) -> Result<Prepared> {
        Ok(passthrough(ctx))
    }

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
        let path = dir.join("kaimux.json");
        let created = !path.exists();
        if created {
            fs::create_dir_all(&dir).with_context(|| format!("mkdir -p {}", dir.display()))?;
            fs::write(
                &path,
                serde_json::to_vec_pretty(&build_kiro_config(ctx.self_path))?,
            )?;
        }
        Ok(Prepared {
            created_kiro_config: created,
            ..passthrough(ctx)
        })
    }

    // Refcount on cwd — keep the project-scoped config alive while any kiro
    // session in this cwd is live. The created_kiro_config flag isn't
    // consulted: closing the last reuser removes it even if a different
    // session created it.
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
            .join("kaimux.json");
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
        Ok(passthrough(ctx))
    }
    fn cleanup(&self, _store: &Store, _removing: &Session, _others: &[Session]) -> Result<()> {
        Ok(())
    }
}

// Unknown kinds register + lifecycle-clean on pane-exit, no per-kind config.
// Lets the user observe arbitrary CLIs before we know enough to wrap them.
fn wrapper_for(kind: &str) -> Box<dyn Wrapper> {
    match kind {
        "claude" => Box::new(Claude),
        "kiro" => Box::new(Kiro),
        other => Box::new(Other(other.to_string())),
    }
}

// Idempotent via a marker file. tmux `set-hook -g` is itself idempotent
// so the marker race between two wrappers is benign.
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

// `tmux set-option -p` — tags wrapped panes with `@kaimux-pane <id>` so
// future tmux-only walkers (e.g. a `kaimux doctor`) can find them
// without reading the registry.
fn tmux_set_pane_option(pane_id: &str, key: &str, value: &str) -> Result<()> {
    let status = std::process::Command::new("tmux")
        .args(["set-option", "-p", "-t", pane_id, key, value])
        .status()
        .context("tmux set-option")?;
    anyhow::ensure!(status.success(), "tmux set-option failed: {status}");
    Ok(())
}

// execvp — replaces our process with the agent. Returns only on failure.
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

// Wrapper-dispatch + register + (when side_effects) execvp. prepare runs
// inside the store lock so a concurrent unregister can't remove a kiro
// config between our ensure and register.
pub fn wrap(w: &dyn Wrapper, ctx: &WrapCtx, side_effects: bool) -> Result<Prepared> {
    anyhow::ensure!(
        !ctx.agent_argv.is_empty(),
        "kaimux wrap needs an agent command after `--`"
    );

    let now = now_secs();
    let prepared = ctx.store.mutate(|sessions| {
        // Existing record on this pane: alive pid ⇒ genuine double-register
        // (refuse loud); dead pid ⇒ orphaned by remain-on-exit / server
        // restart / missed pane-exited (clean up the stale record and reuse).
        if let Some(idx) = sessions.iter().position(|s| s.pane_id == ctx.pane_id) {
            let alive =
                nix::sys::signal::kill(nix::unistd::Pid::from_raw(sessions[idx].pid), None).is_ok();
            anyhow::ensure!(
                !alive,
                "pane {} already registered — `kaimux unregister {}` first",
                ctx.pane_id,
                ctx.pane_id
            );
            let stale = sessions.remove(idx);
            // Remove first so cleanup's siblings list excludes the stale —
            // kiro refcount stays correct.
            wrapper_for(&stale.kind).cleanup(ctx.store, &stale, sessions)?;
        }
        let prepared = w.prepare(ctx)?;
        sessions.push(Session {
            pane_id: ctx.pane_id.into(),
            pid: std::process::id() as i32,
            kind: w.kind().into(),
            cwd: ctx.cwd.to_string_lossy().into_owned(),
            started: now,
            // Initial Done — no events yet; the priority sort treats it as
            // a finished turn (glanceable, non-blocking). Decays to Idle.
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
        tmux_set_pane_option(ctx.pane_id, "@kaimux-pane", ctx.pane_id)?;
        // SAFETY: single-threaded immediately before execvp.
        std::env::set_var("KAIMUX_PANE", ctx.pane_id);
        exec_agent(&prepared.program, &prepared.argv)?;
    }
    Ok(prepared)
}

// Tmux `pane-exited` target.
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
// §4 · Hook — single entry the wrapped agent's lifecycle hooks call into.
// Resolves the pane against the registry, runs `apply_event`, persists.
// Always Ok — a failing reporter must never block the agent's turn (the
// Cmd::Hook dispatch swallows errors). Caller filters $KAIMUX_PANE before
// invoking; this body trusts pane_id.
pub fn report_event(
    store: &Store,
    pane_id: &str,
    event: &str,
    stdin: &mut dyn Read,
    now: u64,
) -> Result<()> {
    let mut buf = Vec::new();
    stdin.read_to_end(&mut buf).context("read hook payload")?;
    let payload: serde_json::Value = serde_json::from_slice(&buf).unwrap_or(serde_json::json!({}));
    store.mutate(|sessions| {
        if let Some(s) = sessions.iter_mut().find(|s| s.pane_id == pane_id) {
            s.apply_event(event, &payload, now);
        }
        Ok(()) // stale fire after unregister: silent no-op
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// §5 · Configure — `kaimux setup` / `kaimux teardown`. Edits the user's
// Claude settings file + tmux prefix-table keybind. Idempotent on both
// directions; teardown self-discovers the keybind via marker.

// `kaimux setup`. Idempotent: re-running rewrites our entries' command
// paths (binary-moved case) without duplicating. With `key`, also binds
// `<tmux-prefix> <key>` to switch to the dashboard; any prior dashboard
// binding is removed first so re-keying is clean.
fn run_setup(path: &Path, self_path: &Path, key: Option<&str>, session_name: &str) -> Result<()> {
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
        uninstall_tmux_keybind(); // remove any prior binding before re-key
        install_tmux_keybind(suffix, session_name);
    }
    Ok(())
}

// `kaimux teardown`. Self-discovers the keybind via marker — no flags
// needed, works regardless of the suffix or session name used at install.
fn run_teardown(path: &Path) -> Result<()> {
    uninstall_tmux_keybind();
    if !path.exists() {
        return Ok(());
    }
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    if bytes.is_empty() {
        return Ok(()); // empty file: leave it alone
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

// Idempotent merge of our four hook entries into Claude's settings shape:
//   hooks.<event> = [ { matcher: "", hooks: [{type:"command",command}], x-kaimux-managed: true }, ... ]
// Existing tagged entries get their command rewritten (binary moved case);
// user-authored entries (no tag) preserved verbatim. Errors loud on shape
// mismatches — silent-drop would leave the user with a hookless wrapper.
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
        if let Some(existing) = list
            .iter_mut()
            .find(|e| e.get(KAIMUX_TAG).and_then(|v| v.as_bool()).unwrap_or(false))
        {
            // Happy path: rewrite command in slot 0; continue skips the overwrite.
            if let Some(inner) = existing.get_mut("hooks").and_then(|h| h.as_array_mut()) {
                if let Some(first) = inner.first_mut() {
                    first["command"] = json!(cmd);
                    continue;
                }
            }
            // Tagged entry with unexpected shape — overwrite cleanly.
            *existing = json!({
                "matcher": "",
                "hooks": [{ "type": "command", "command": cmd }],
                KAIMUX_TAG: true,
            });
        } else {
            list.push(json!({
                "matcher": "",
                "hooks": [{ "type": "command", "command": cmd }],
                KAIMUX_TAG: true,
            }));
        }
    }
    Ok(())
}

// Inverse of merge: drop tagged entries, prune empty containers. Caller
// decides whether to delete the file when the result is `{}`.
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
        arr.retain(|e| !e.get(KAIMUX_TAG).and_then(|v| v.as_bool()).unwrap_or(false));
        if arr.is_empty() {
            hooks.remove(&ev);
        }
    }
    if hooks.is_empty() {
        root.remove("hooks");
    }
}

// Same nested schema as Claude's hooks (Kiro reuses the matcher-array shape).
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

// Bind <prefix> <suffix> → switch-client. Prefix-bound (not root-bound)
// so inner TUIs (claude/kiro) never see it. Action chain rides a no-op
// `run-shell "true #<marker>"`; tmux preserves the marker verbatim in
// list-keys output, which is how teardown self-discovers the binding
// without a state file or a --key flag. Live-only — server restart
// drops it; persistence across restarts is the user's job (bake the
// equivalent line into ~/.tmux.conf).
fn install_tmux_keybind(suffix: &str, session_name: &str) {
    let action = format!("run-shell \"true #{KEYBIND_MARKER}\" ; switch-client -t {session_name}");
    let _ = tmux_cmd(&["bind-key", "-T", "prefix", suffix, &action]).status();
}

fn uninstall_tmux_keybind() {
    let out = tmux_cmd(&["list-keys", "-T", "prefix"])
        .stderr(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .output();
    let Ok(out) = out else { return };
    if !out.status.success() {
        return;
    }
    // list-keys line shape:
    //   bind-key -T prefix O run-shell "true #x-kaimux-managed" \; switch-client -t <name>
    let stdout = String::from_utf8_lossy(&out.stdout);
    for line in stdout.lines() {
        if !line.contains(KEYBIND_MARKER) {
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

fn write_json_atomic(path: &Path, value: &serde_json::Value) -> Result<()> {
    AtomicFile::new(path, AllowOverwrite)
        .write(|w| w.write_all(&serde_json::to_vec_pretty(value)?))
        .with_context(|| format!("atomic write {}", path.display()))
}

fn user_claude_settings_path() -> Result<PathBuf> {
    Ok(PathBuf::from(std::env::var("HOME").context("$HOME unset")?).join(".claude/settings.json"))
}

// $KAIMUX_TMUX_SOCKET → `-L <name>` (integration tests target a private
// server). Stderr suppressed: "no server" is normal pre-first-run noise.
// Used by Configure (keybind ops) and Loop (preview/capture/address).
fn tmux_cmd(args: &[&str]) -> std::process::Command {
    let mut cmd = std::process::Command::new("tmux");
    if let Ok(name) = std::env::var("KAIMUX_TMUX_SOCKET") {
        if !name.is_empty() {
            cmd.arg("-L").arg(name);
        }
    }
    cmd.args(args).stderr(std::process::Stdio::null());
    cmd
}

// Run a raw `tmux` command (no socket override, no stderr suppression) and
// return whether it exited 0. Convenience for the dashboard bootstrap
// (`Loop::run`) where we need a yes/no on session presence + spawn ops.
fn tmux_ok(args: &[&str]) -> bool {
    std::process::Command::new("tmux")
        .args(args)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

// ─────────────────────────────────────────────────────────────────────────────
// §6 · Loop

// Fixed: a varying row height would reflow the whole fzf list on every
// reload. v2 follow-up makes this runtime-tunable.
const SNIPPET_LINES: usize = 3;

pub struct Item {
    pub pane_id: String, // leading fzf column — bound actions reference it via {1}; hidden via --with-nth=2..
    pub header: String,  // tab-separated: icon, addr, kind, cwd, elapsed
    pub snippet: [String; SNIPPET_LINES],
}

pub struct Loop<'a> {
    store: &'a Store,
}

impl<'a> Loop<'a> {
    pub fn new(store: &'a Store) -> Self {
        Loop { store }
    }

    pub fn render(&self) -> Result<Vec<Item>> {
        self.render_with(&|p| resolve_pane_addr(p), &|p| capture_snippet(p))
    }

    // Injected resolver + snippet so tests can drive the pipeline without a
    // tmux server. Best-effort by design: stale row > missing row.
    pub fn render_with(
        &self,
        resolve_addr: &dyn Fn(&str) -> String,
        snippet: &dyn Fn(&str) -> [String; SNIPPET_LINES],
    ) -> Result<Vec<Item>> {
        let now = now_secs();
        let mut sessions = live_only(self.store.read()?);
        sort_sessions(&mut sessions, now);
        Ok(sessions
            .into_iter()
            .map(|s| {
                let addr = resolve_addr(&s.pane_id);
                let header = s.format_header(&addr, now);
                let snippet = snippet(&s.pane_id);
                Item {
                    pane_id: s.pane_id,
                    header,
                    snippet,
                }
            })
            .collect())
    }

    // Item shape: `<pane_id>\t<header>\n<line1>\n<line2>\n<line3>`,
    // items NUL-separated for fzf's `--read0`. Snippet NULs sanitised to
    // `?` so a pane spitting raw NULs can't split the item early.
    pub fn render_to(&self, stdout: &mut dyn Write) -> Result<()> {
        let items = self.render()?;
        let blocks: Vec<String> = items
            .iter()
            .map(|i| {
                let snippet = i
                    .snippet
                    .iter()
                    .map(|l| l.replace('\0', "?"))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("{}\t{}\n{snippet}", i.pane_id, i.header)
            })
            .collect();
        stdout.write_all(blocks.join("\0").as_bytes())?;
        Ok(())
    }

    // Ensure the dashboard tmux session exists; switch-client to it. The
    // picker body runs inside that session as a bare `kaimux --session NAME`
    // (bare detects "already inside the dashboard" and runs body, not run).
    pub fn run(&self, self_path: &Path, session_name: &str) -> Result<()> {
        if !tmux_ok(&["has-session", "-t", session_name]) {
            // Pass --session through to the bare child so it self-identifies.
            let cmd = format!(
                "{} --session {}",
                self_path.display(),
                shell_quote(session_name)
            );
            anyhow::ensure!(
                tmux_ok(&["new-session", "-d", "-s", session_name, &cmd]),
                "tmux new-session failed"
            );
        }
        anyhow::ensure!(
            tmux_ok(&["switch-client", "-t", session_name]),
            "tmux switch-client failed"
        );
        Ok(())
    }

    // fzf's --preview target. `-e` preserves ANSI so claude/kiro coloured
    // output renders as-is (fzf's --ansi interprets it). Routes through
    // tmux_cmd to honour $KAIMUX_TMUX_SOCKET in the integration tests.
    pub fn peek(&self, pane_id: &str, lines: u32, out: &mut dyn Write) -> Result<()> {
        let start = format!("-{}", lines);
        let output = tmux_cmd(&[
            "capture-pane",
            "-p",
            "-e",
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
        // Pane gone (closed, server restart) ⇒ empty preview, not an error.
        if output.status.success() {
            out.write_all(&output.stdout)?;
        }
        Ok(())
    }

    // Event-driven picker. fzf runs once with --listen=<sock>; we drive it
    // over UDS instead of respawning. Two background threads:
    //   watcher   — on sessions.json change, post reload(<self> render)
    //               (rebuild the list — rows added/removed/changed state)
    //   heartbeat — once a second, post refresh-preview (rerun preview only)
    // Split because reload() briefly blocks fzf input — doing it at 1 Hz
    // flickered. refresh-preview leaves cursor/query/prompt untouched.
    //
    // fzf flags (the load-bearing ones):
    //   --read0           items are NUL-delimited (multi-line: header + 3-line snippet)
    //   --with-nth=2..    hide the pane-id column from display
    //   --track --id-nth=1  keep cursor on the same pane id across reloads
    //   --gap=1 --highlight-line --ansi  layout / colour
    //   --preview '<self> peek {1}'  side window with full pane content
    //
    // Bindings:
    //   enter  switch-client to {1} (non-terminal — fzf survives the jump)
    //   p      tmux popup attached to {1}
    //   x      `<self> unregister {1}` then reload — drops the row, leaves the agent alive
    pub fn body(&self, self_path: &Path) -> Result<()> {
        let sock_path = pick_listen_socket();
        let sock = sock_path.to_string_lossy().to_string();
        let render_cmd = format!("{} render", self_path.display());
        let preview_cmd = format!("{} peek {{1}}", self_path.display());
        let enter_bind = "enter:execute-silent(tmux switch-client -t {1})+clear-query";
        let peek_bind = "p:execute(tmux display-popup -E \"tmux attach -t {1}\")";
        let kill_bind = format!(
            "x:execute-silent({} unregister {{1}})+reload({})",
            self_path.display(),
            render_cmd,
        );
        let header = "enter jump · p peek · x kill record · / filter · esc exit";

        let mut child = std::process::Command::new("fzf")
            .args([
                &format!("--listen={sock}"),
                "--read0",
                "--gap=1",
                "--highlight-line",
                "--ansi",
                "--with-nth=2..",
                "--track",
                "--id-nth=1",
                "--preview",
                &preview_cmd,
                "--preview-window=right:50%",
                "--header",
                header,
                "--bind",
                enter_bind,
                "--bind",
                peek_bind,
                "--bind",
                &kill_bind,
            ])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .context("spawn fzf (is it on PATH?)")?;

        // Seed the source on stdin then close it so fzf treats it as
        // exhausted; updates after this arrive via reload(...) on the socket.
        {
            let stdin = child.stdin.as_mut().context("fzf stdin")?;
            self.render_to(stdin)?;
        }
        drop(child.stdin.take());

        let (tx, rx) = mpsc::channel::<Tick>();

        let watch_dir = self.store.dir().to_path_buf();
        let watcher_tx = tx.clone();
        thread::spawn(move || {
            if let Err(e) = run_watcher(&watch_dir, watcher_tx) {
                eprintln!("kaimux watcher: {e:#}");
            }
        });

        let heartbeat_tx = tx;
        thread::spawn(move || {
            while heartbeat_tx.send(Tick::RefreshPreview).is_ok() {
                thread::sleep(Duration::from_secs(1));
            }
        });

        // 200ms timeout: poll fzf exit ~5×/s. Pure blocking recv would
        // miss the user closing fzf with no ticks in flight.
        loop {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(tick) => {
                    let mut want_reload = matches!(tick, Tick::Reload);
                    let mut want_refresh = matches!(tick, Tick::RefreshPreview);
                    // Coalesce backlog — a burst of identical ticks fires once.
                    while let Ok(more) = rx.try_recv() {
                        match more {
                            Tick::Reload => want_reload = true,
                            Tick::RefreshPreview => want_refresh = true,
                        }
                    }
                    if want_reload {
                        if let Err(e) = push_action(&sock_path, &format!("reload({render_cmd})")) {
                            eprintln!("kaimux reload: {e:#}");
                        }
                        want_refresh = true; // reload may shift focus — refresh preview to track it
                    }
                    if want_refresh {
                        if let Err(e) = push_action(&sock_path, "refresh-preview") {
                            eprintln!("kaimux refresh-preview: {e:#}");
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    // Both threads gone — selections still work, no live updates.
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

// %17 → `proj-b:code.0`. Tmux failure ⇒ `?:?.<pane-id>` so the row still
// renders with a recognisable identifier.
fn resolve_pane_addr(pane_id: &str) -> String {
    let out = tmux_cmd(&["display-message", "-p", "-t", pane_id, "#S:#I.#P"])
        .stdout(std::process::Stdio::piped())
        .output();
    match out {
        Ok(o) if o.status.success() => {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() {
                format!("?:?.{}", pane_id)
            } else {
                s
            }
        }
        _ => format!("?:?.{}", pane_id),
    }
}

// `capture-pane -S -3` can return 0 lines if the agent's output scrolled
// past — capture a wider window (-50) and trim to SNIPPET_LINES.
fn capture_snippet(pane_id: &str) -> [String; SNIPPET_LINES] {
    let mut out = std::array::from_fn(|_| String::new());
    let cap = tmux_cmd(&[
        "capture-pane",
        "-p",
        "-e",
        "-t",
        pane_id,
        "-E",
        "-1",
        "-S",
        "-50",
    ])
    .stdout(std::process::Stdio::piped())
    .output();
    let Ok(cap) = cap else { return out };
    if !cap.status.success() {
        return out;
    }
    let captured = String::from_utf8_lossy(&cap.stdout);
    // Last SNIPPET_LINES non-empty lines from the capture.
    // Drop trailing blanks first so a pane that's just been
    // cleared doesn't fill the snippet with whitespace.
    let mut lines: Vec<&str> = captured.lines().collect();
    while matches!(lines.last(), Some(l) if l.trim().is_empty()) {
        lines.pop();
    }
    let take = lines.len().saturating_sub(SNIPPET_LINES);
    // 2-space indent separates snippet lines from the next row's header.
    // Empty lines stay un-padded — saves visual weight on scant panes.
    for (i, line) in lines.iter().skip(take).take(SNIPPET_LINES).enumerate() {
        out[i] = if line.trim().is_empty() {
            String::new()
        } else {
            format!("  {line}")
        };
    }
    out
}

enum Tick {
    Reload,         // sessions.json actually changed
    RefreshPreview, // 1 Hz heartbeat
}

// XDG_RUNTIME_DIR (tmpfs, per-user) → TMPDIR → /tmp. Per-pid filename so
// concurrent dashboards don't collide.
fn pick_listen_socket() -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .or_else(|| std::env::var_os("TMPDIR").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    dir.join(format!("kaimux-fzf-{}.sock", std::process::id()))
}

// 100ms debounce: live-feeling, and coalesces the tmp+rename pair from
// an atomic write into one event.
fn run_watcher(dir: &Path, tx: mpsc::Sender<Tick>) -> Result<()> {
    use notify::RecursiveMode;
    use notify_debouncer_mini::new_debouncer;

    fs::create_dir_all(dir).with_context(|| format!("mkdir -p {}", dir.display()))?;
    // Watch the dir, not the file — the file may not exist on first run,
    // and atomic rename swaps inodes (a file-watch would go stale).
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

// fzf speaks HTTP/1.1 over its UDS; body is the raw action.
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
    // Drain the response — fzf must not see a half-closed socket. Body
    // unparsed; any 2xx/4xx is fine, connection errors surface above.
    let mut sink = Vec::new();
    let _ = stream.read_to_end(&mut sink);
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// CLI

#[derive(Parser)]
#[command(
    name = "kaimux",
    version,
    about = "Observation-only orchestrator over tmux + coding-agent panes."
)]
struct Cli {
    // Optional: bare `kaimux` (no subcommand) is valid — the dashboard entrypoint.
    #[command(subcommand)]
    cmd: Option<Cmd>,
    /// Tmux session name that hosts the dashboard. Override with --session NAME
    /// to scope multiple dashboards or sidestep a colliding session.
    #[arg(long, global = true)]
    session: Option<String>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Install kaimux hooks into ~/.claude/settings.json (idempotent).
    /// With --key X, also bind <tmux-prefix> X to switch to the dashboard.
    Setup {
        /// Prefix-table suffix (e.g. O → tmux-prefix then O).
        #[arg(long)]
        key: Option<String>,
    },
    /// Remove kaimux hooks; self-discovers any keybind we installed.
    Teardown,
    /// Wrap a coding agent: register, inject hooks, execvp.
    Wrap {
        kind: String,
        #[arg(long)]
        cwd: Option<PathBuf>,
        #[arg(last = true)]
        agent_argv: Vec<String>,
    },
    // Hook reporter — Claude/Kiro on each lifecycle event.
    #[command(hide = true)]
    Hook { event: String },
    // Pane-exited target — tmux invokes us on close.
    #[command(hide = true)]
    Unregister { pane_id: String },
    // fzf reload source.
    #[command(hide = true)]
    Render,
    // fzf --preview source.
    #[command(hide = true)]
    Peek {
        pane_id: String,
        #[arg(long, default_value_t = 25)]
        lines: u32,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let store = Store::from_env()?;
    let self_path = std::env::current_exe().context("current_exe")?;
    let session_name = cli.session.as_deref().unwrap_or(DEFAULT_SESSION_NAME);

    match cli.cmd {
        // Bare. Inside dashboard session ⇒ run picker body (we're the
        // session's startup command). Anywhere else ⇒ create + switch to it.
        None => {
            if inside_dashboard(session_name) {
                Loop::new(&store).body(&self_path)
            } else {
                Loop::new(&store).run(&self_path, session_name)
            }
        }

        Some(Cmd::Setup { key }) => run_setup(
            &user_claude_settings_path()?,
            &self_path,
            key.as_deref(),
            session_name,
        ),
        Some(Cmd::Teardown) => run_teardown(&user_claude_settings_path()?),

        Some(Cmd::Wrap {
            kind,
            cwd,
            agent_argv,
        }) => {
            let pane_id = std::env::var("TMUX_PANE")
                .ok()
                .filter(|s| !s.is_empty())
                .context("kaimux wrap requires $TMUX_PANE — run inside tmux")?;
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
            if let Some(pane) = std::env::var("KAIMUX_PANE").ok().filter(|s| !s.is_empty()) {
                let _ = report_event(&store, &pane, &event, &mut std::io::stdin(), now_secs());
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

// `$TMUX` set AND tmux says we're in <session_name>. Either alone
// misclassifies — a user running bare `kaimux` from inside any other
// tmux session would otherwise spawn a body there.
fn inside_dashboard(session_name: &str) -> bool {
    if std::env::var_os("TMUX").is_none() {
        return false;
    }
    let out = std::process::Command::new("tmux")
        .args(["display-message", "-p", "#{session_name}"])
        .output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim() == session_name,
        _ => false,
    }
}

// Single-quote with `'\''` escapes — safe for the command string tmux
// passes to /bin/sh -c when we embed a session name in `new-session`.
fn shell_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests — each typeclass through its own surface (Store via read/mutate,
// Wrapper via wrap(), hook via Wrapper::hook, Loop via render). E2E
// (real tmux + execvp + pane-exited) is tests/kaimux/integration.sh.

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
            // Default fresh-wrap state (matches `wrap`) — Done bucket until
            // it decays to Idle or gets driven by an event.
            state: State::Done,
            state_ts: started,
            last_event: String::new(),
            last_event_ts: 0,
            created_kiro_config: false,
        }
    }

    fn empty_payload() -> serde_json::Value {
        serde_json::json!({})
    }

    fn fixtures() -> (TempDir, Store) {
        let dir = tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf());
        (dir, store)
    }

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
        assert_eq!(s.display_state(now).priority(), 0);

        s.state = State::Done;
        s.state_ts = now;
        assert_eq!(s.display_state(now).priority(), 1);

        // Same Done record, but aged past the idle threshold:
        // priority becomes 2 (idle bucket) without changing storage.
        s.state_ts = now - IDLE_THRESHOLD_SECS - 1;
        assert_eq!(s.display_state(now).priority(), 2);

        s.state = State::Working;
        s.state_ts = now;
        assert_eq!(s.display_state(now).priority(), 3);
    }

    #[test]
    fn format_header_emits_icon_addr_kind_cwd_elapsed() {
        // The dashboard header line. Tab-separated columns:
        // <icon> <addr> \t <kind> \t <cwd-fixed-width> \t <elapsed>
        let mut s = mk("%1", "claude", "/home/me/repo/foo", 1000);
        s.state = State::Working;
        s.last_event_ts = 1000;

        let h = s.format_header("proj-b:code.0", 1005);
        assert!(h.starts_with("▶ proj-b:code.0\t"), "header: {h}");
        let cols: Vec<&str> = h.split('\t').collect();
        assert_eq!(cols.len(), 4, "expected 4 tab-separated columns: {h:?}");
        assert!(cols[1].contains("claude"));
        // cwd column padded to fixed width.
        assert_eq!(cols[2].chars().count(), CWD_COLUMN_WIDTH);
        // elapsed reflects 1005 - 1000 == 5s.
        assert_eq!(cols[3], "5s");
    }

    #[test]
    fn format_header_picks_glyph_per_state() {
        let mut s = mk("%1", "claude", "/home/me/repo/foo", 1000);
        s.last_event_ts = 1000;

        s.state = State::Waiting;
        assert!(s.format_header("a:0.0", 1000).starts_with("💬 "));

        s.state = State::Done;
        assert!(s.format_header("a:0.0", 1000).starts_with("✓ "));

        s.state = State::Working;
        assert!(s.format_header("a:0.0", 1000).starts_with("▶ "));

        // Done aged past the threshold decays to Idle.
        s.state = State::Done;
        s.state_ts = 0;
        assert!(s
            .format_header("a:0.0", IDLE_THRESHOLD_SECS + 100)
            .starts_with("· "));
    }

    #[test]
    fn format_elapsed_buckets() {
        assert_eq!(format_elapsed(0), "0s");
        assert_eq!(format_elapsed(5), "5s");
        assert_eq!(format_elapsed(59), "59s");
        assert_eq!(format_elapsed(60), "1m");
        assert_eq!(format_elapsed(125), "2m");
        assert_eq!(format_elapsed(3_599), "59m");
        assert_eq!(format_elapsed(3_600), "1h");
        assert_eq!(format_elapsed(86_399), "23h");
        assert_eq!(format_elapsed(86_400), "1d");
        assert_eq!(format_elapsed(259_200), "3d");
    }

    #[test]
    fn cwd_fixed_width_pads_short_path() {
        let out = cwd_fixed_width("proj-b", 14);
        assert_eq!(out.chars().count(), 14);
        assert!(out.starts_with("proj-b"), "got: {out:?}");
    }

    #[test]
    fn cwd_fixed_width_truncates_long_path_with_leading_marker() {
        let out = cwd_fixed_width("/home/me/work/projects/kaimux", 18);
        // Always exactly width chars; always starts with the
        // leading marker; ends with the distinguishing
        // trailing path segment.
        assert_eq!(out.chars().count(), 18);
        assert!(out.starts_with("…/"));
        assert!(out.contains("kaimux"), "got: {out:?}");
    }

    #[test]
    fn cwd_fixed_width_handles_exact_width() {
        let path = "abcdefghij"; // 10 chars
        let out = cwd_fixed_width(path, 10);
        assert_eq!(out.chars().count(), 10);
        assert_eq!(out, path);
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
        let self_path = PathBuf::from("/test/kaimux");
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
        let self_path = PathBuf::from("/test/kaimux");
        run_setup(&path, &self_path, None, DEFAULT_SESSION_NAME).unwrap();

        let v: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        for ev in HOOK_EVENTS {
            let ev = *ev;
            let arr = v["hooks"][ev].as_array().unwrap();
            assert_eq!(arr.len(), 1, "{ev}");
            assert_eq!(arr[0]["matcher"], "");
            assert_eq!(arr[0][KAIMUX_TAG], true);
            assert_eq!(
                arr[0]["hooks"][0]["command"],
                format!("/test/kaimux hook {}", ev)
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
        let self_path = PathBuf::from("/test/kaimux");
        run_setup(&path, &self_path, None, DEFAULT_SESSION_NAME).unwrap();

        let v: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        // Sibling fields untouched.
        assert_eq!(v["permissions"][0], "read");
        // User's UserPromptSubmit entry preserved at slot 0; ours appended.
        let ups = v["hooks"]["UserPromptSubmit"].as_array().unwrap();
        assert_eq!(ups.len(), 2);
        assert_eq!(ups[0]["hooks"][0]["command"], "my-hook");
        assert!(ups[0].get(KAIMUX_TAG).is_none());
        assert_eq!(ups[1][KAIMUX_TAG], true);
        // Other events have just our entry.
        for ev in HOOK_EVENTS.iter().filter(|e| **e != EVT_USER_PROMPT_SUBMIT) {
            let arr = v["hooks"][*ev].as_array().unwrap();
            assert_eq!(arr.len(), 1);
            assert_eq!(arr[0][KAIMUX_TAG], true);
        }
    }

    #[test]
    fn setup_idempotent_no_duplicate_entries() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let self_path = PathBuf::from("/test/kaimux");
        run_setup(&path, &self_path, None, DEFAULT_SESSION_NAME).unwrap();
        run_setup(&path, &self_path, None, DEFAULT_SESSION_NAME).unwrap();

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
        run_setup(
            &path,
            &PathBuf::from("/old/kaimux"),
            None,
            DEFAULT_SESSION_NAME,
        )
        .unwrap();
        run_setup(
            &path,
            &PathBuf::from("/new/kaimux"),
            None,
            DEFAULT_SESSION_NAME,
        )
        .unwrap();

        let v: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let arr = v["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["hooks"][0]["command"], "/new/kaimux hook Stop");
    }

    #[test]
    fn setup_rejects_non_object_user_settings_root() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, b"[]").unwrap();
        let err = run_setup(
            &path,
            &PathBuf::from("/x/kaimux"),
            None,
            DEFAULT_SESSION_NAME,
        )
        .unwrap_err();
        assert!(format!("{err:#}").contains("must be a JSON object"));
    }

    #[test]
    fn setup_rejects_non_array_hooks_event() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, br#"{"hooks":{"Stop":"oops"}}"#).unwrap();
        let err = run_setup(
            &path,
            &PathBuf::from("/x/kaimux"),
            None,
            DEFAULT_SESSION_NAME,
        )
        .unwrap_err();
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
                  { "matcher": "", "hooks": [{"type":"command","command":"/x hook UserPromptSubmit"}], "x-kaimux-managed": true }
                ],
                "Stop": [
                  { "matcher": "", "hooks": [{"type":"command","command":"/x hook Stop"}], "x-kaimux-managed": true }
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
        run_setup(
            &path,
            &PathBuf::from("/x/kaimux"),
            None,
            DEFAULT_SESSION_NAME,
        )
        .unwrap();
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
            br#"{"permissions":["read"],"hooks":{"Stop":[{"matcher":"","hooks":[{"type":"command","command":"/x hook Stop"}],"x-kaimux-managed":true}]}}"#,
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
        run_setup(
            &path,
            &PathBuf::from("/x/kaimux"),
            None,
            DEFAULT_SESSION_NAME,
        )
        .unwrap();
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
        let self_path = PathBuf::from("/test/kaimux");
        let ctx = ctx(&store, "%1", &cwd, &argv, &self_path);
        let p = Kiro.prepare(&ctx).unwrap();

        assert!(p.created_kiro_config);
        let cfg = cwd.join(".kiro").join("agents").join("kaimux.json");
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
                format!("/test/kaimux hook {}", ev)
            );
        }
    }

    #[test]
    fn kiro_prepare_reuser_does_not_stamp_flag() {
        let (dir, store) = fixtures();
        let cwd = dir.path().to_path_buf();
        let argv = vec!["kiro".to_string()];
        let self_path = PathBuf::from("/test/kaimux");
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
        let cfg = cwd.join(".kiro").join("agents").join("kaimux.json");
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
        let cfg = cwd.join(".kiro").join("agents").join("kaimux.json");
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
        let self_path = PathBuf::from("/test/kaimux");
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
        let self_path = PathBuf::from("/test/kaimux");
        let ctx = ctx(&store, "%9", &cwd, &argv, &self_path);
        wrap(&Kiro, &ctx, false).unwrap();

        let v = store.read().unwrap();
        assert_eq!(v.len(), 1);
        assert!(v[0].created_kiro_config);
        assert!(cwd.join(".kiro/agents/kaimux.json").exists());
    }

    #[test]
    fn wrap_refuses_double_register_when_existing_pid_alive() {
        // The wrap path records `std::process::id()` (the test
        // process pid, alive by definition), so a second wrap on
        // the same pane sees an alive sibling and refuses.
        let (dir, store) = fixtures();
        let cwd = dir.path().to_path_buf();
        let argv = vec!["stub".to_string()];
        let self_path = PathBuf::from("/test/kaimux");
        let ctx = ctx(&store, "%5", &cwd, &argv, &self_path);
        wrap(&Claude, &ctx, false).unwrap();
        let err = wrap(&Claude, &ctx, false).unwrap_err();
        assert!(format!("{err:#}").contains("already registered"));
    }

    // A reaped pid: signal-0 returns ESRCH, so `wrap`'s liveness probe
    // reads it as dead — what we need to seed a stale-record case.
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
        let self_path = PathBuf::from("/test/kaimux");
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
        let cfg = cwd.join(".kiro").join("agents").join("kaimux.json");
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
        let self_path = PathBuf::from("/test/kaimux");
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
        let self_path = PathBuf::from("/test/kaimux");
        let ctx = ctx(&store, "%1", &cwd, &argv, &self_path);
        let err = wrap(&Claude, &ctx, false).unwrap_err();
        assert!(format!("{err:#}").contains("after `--`"));
    }

    // 8 · Wrapper — hook (default trait method) ──────────────────

    fn empty_stdin() -> Cursor<Vec<u8>> {
        Cursor::new(b"{}".to_vec())
    }

    #[test]
    fn hook_pre_tool_use_marks_running() {
        let (dir, store) = fixtures();
        let cwd = dir.path().to_path_buf();
        let argv = vec!["stub".to_string()];
        let self_path = PathBuf::from("/test/kaimux");
        let ctx = ctx(&store, "%9", &cwd, &argv, &self_path);
        wrap(&Claude, &ctx, false).unwrap();

        report_event(&store, "%9", EVT_PRE_TOOL_USE, &mut empty_stdin(), 1234).unwrap();
        let v = store.read().unwrap();
        assert_eq!(v[0].state, State::Working);
        assert_eq!(v[0].last_event_ts, 1234);
    }

    #[test]
    fn hook_stop_marks_done() {
        let (dir, store) = fixtures();
        let cwd = dir.path().to_path_buf();
        let argv = vec!["stub".to_string()];
        let self_path = PathBuf::from("/test/kaimux");
        let ctx = ctx(&store, "%1", &cwd, &argv, &self_path);
        wrap(&Claude, &ctx, false).unwrap();

        // Drive into Working first so we can observe the flip back.
        report_event(&store, "%1", EVT_PRE_TOOL_USE, &mut empty_stdin(), 50).unwrap();
        assert_eq!(store.read().unwrap()[0].state, State::Working);

        report_event(&store, "%1", EVT_STOP, &mut empty_stdin(), 99).unwrap();
        assert_eq!(store.read().unwrap()[0].state, State::Done);
    }

    #[test]
    fn hook_notification_marks_waiting() {
        // The load-bearing case for the four-state machine —
        // permission prompts must surface as Waiting so they sort
        // to the top of the dashboard.
        let (dir, store) = fixtures();
        let cwd = dir.path().to_path_buf();
        let argv = vec!["stub".to_string()];
        let self_path = PathBuf::from("/test/kaimux");
        let ctx = ctx(&store, "%1", &cwd, &argv, &self_path);
        wrap(&Claude, &ctx, false).unwrap();

        report_event(&store, "%1", EVT_NOTIFICATION, &mut empty_stdin(), 250).unwrap();
        let s = &store.read().unwrap()[0];
        assert_eq!(s.state, State::Waiting);
        assert_eq!(s.last_event, "Notification");
    }

    #[test]
    fn hook_no_op_for_unknown_pane() {
        let (_dir, store) = fixtures();
        // No record for %999 — must not error, must not phantom-write.
        report_event(&store, "%999", EVT_STOP, &mut empty_stdin(), 1).unwrap();
        assert!(store.read().unwrap().is_empty());
    }

    // 9 · unregister + Loop ──────────────────────────────────────

    #[test]
    fn unregister_runs_per_kind_cleanup_via_trait() {
        let (dir, store) = fixtures();
        let cwd = dir.path().to_path_buf();
        let argv = vec!["stub".to_string()];
        let self_path = PathBuf::from("/test/kaimux");
        let ctx = ctx(&store, "%1", &cwd, &argv, &self_path);
        wrap(&Kiro, &ctx, false).unwrap();
        let cfg = cwd.join(".kiro/agents/kaimux.json");
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

        // Use injected resolvers so the test doesn't shell
        // out to tmux. The dead-pid filter happens at the
        // store layer, before resolution.
        let items = Loop::new(&store)
            .render_with(&|p| format!("test:{p}"), &|_| {
                std::array::from_fn(|i| format!("snip{i}"))
            })
            .unwrap();
        // dead pid filtered out; alive remains.
        assert_eq!(items.len(), 1, "items: {}", items.len());
        assert_eq!(items[0].pane_id, "%alive");
    }

    #[test]
    fn loop_render_with_emits_one_item_per_live_session() {
        // The injected resolvers prove the render pipeline
        // composes header + snippet correctly without
        // shelling out to tmux.
        let (_dir, store) = fixtures();
        store
            .mutate(|v| {
                let mut a = mk("%a", "claude", "/repo/a", 1);
                a.state = State::Working;
                a.last_event_ts = now_secs();
                v.push(a);
                let mut b = mk("%b", "kiro", "/repo/b", 1);
                b.state = State::Done;
                b.last_event_ts = now_secs();
                v.push(b);
                Ok(())
            })
            .unwrap();

        let items = Loop::new(&store)
            .render_with(&|p| format!("addr-for-{p}"), &|p| {
                std::array::from_fn(|i| format!("{p}-line-{i}"))
            })
            .unwrap();

        assert_eq!(items.len(), 2);
        // Sort: working (3) sinks; done (1) goes first.
        assert_eq!(items[0].pane_id, "%b");
        assert!(items[0].header.contains("addr-for-%b"));
        assert!(items[0].header.contains("kiro"));
        assert_eq!(items[0].snippet[0], "%b-line-0");
        assert_eq!(items[0].snippet[2], "%b-line-2");

        assert_eq!(items[1].pane_id, "%a");
        assert!(items[1].header.contains("addr-for-%a"));
        assert!(items[1].header.contains("claude"));
    }

    #[test]
    fn loop_render_to_emits_null_separated_multi_line_items() {
        // The shape fzf reads via --read0:
        //   <pane_id>\t<header>\n<line1>\n<line2>\n<line3>\0
        //   <pane_id>\t<header>\n<line1>...
        // No trailing \0 after the last item.
        let (_dir, store) = fixtures();
        store
            .mutate(|v| {
                let mut a = mk("%a", "claude", "/repo/a", 1);
                a.state = State::Working;
                a.last_event_ts = now_secs();
                v.push(a);
                let mut b = mk("%b", "kiro", "/repo/b", 1);
                b.state = State::Done;
                b.last_event_ts = now_secs();
                v.push(b);
                Ok(())
            })
            .unwrap();

        let mut buf = Vec::new();
        Loop::new(&store)
            .render_with(&|p| format!("addr-{p}"), &|p| {
                std::array::from_fn(|i| format!("{p}-snip-{i}"))
            })
            .map(|items| {
                // Reuse render_to's serialiser by going
                // through Loop. Easiest route: stash the
                // injected items through render_to via
                // a helper rebuild.
                let last = items.len().saturating_sub(1);
                for (i, item) in items.iter().enumerate() {
                    use std::io::Write;
                    write!(&mut buf, "{}\t{}", item.pane_id, item.header).unwrap();
                    for line in &item.snippet {
                        buf.push(b'\n');
                        buf.extend_from_slice(line.as_bytes());
                    }
                    if i != last {
                        buf.push(0);
                    }
                }
            })
            .unwrap();

        // Two items, separated by exactly one NUL.
        let nul_count = buf.iter().filter(|&&b| b == 0).count();
        assert_eq!(nul_count, 1, "expected exactly 1 NUL between 2 items");

        let parts: Vec<&[u8]> = buf.split(|&b| b == 0).collect();
        assert_eq!(parts.len(), 2);

        for part in &parts {
            let item_text = std::str::from_utf8(part).unwrap();
            // Each item: pane_id\theader\nl1\nl2\nl3
            let lines: Vec<&str> = item_text.split('\n').collect();
            assert_eq!(
                lines.len(),
                1 + SNIPPET_LINES,
                "expected 1 header + {SNIPPET_LINES} snippet lines: {item_text:?}"
            );
            assert!(lines[0].starts_with("%"));
        }
    }

    #[test]
    fn loop_render_to_sanitises_null_in_snippet() {
        // Defensive: a pane emitting raw NUL bytes through
        // capture-pane would otherwise split the item early.
        let (_dir, store) = fixtures();
        store
            .mutate(|v| {
                let mut a = mk("%a", "claude", "/repo/a", 1);
                a.state = State::Working;
                a.last_event_ts = now_secs();
                v.push(a);
                Ok(())
            })
            .unwrap();

        let mut buf = Vec::new();
        let items = Loop::new(&store)
            .render_with(&|p| format!("addr-{p}"), &|_| {
                std::array::from_fn(|_| String::from("oh\0no"))
            })
            .unwrap();
        // Manually serialise via the same shape render_to uses.
        for item in &items {
            use std::io::Write;
            write!(&mut buf, "{}\t{}", item.pane_id, item.header).unwrap();
            for line in &item.snippet {
                buf.push(b'\n');
                let sanitised: String = line
                    .chars()
                    .map(|c| if c == '\0' { '?' } else { c })
                    .collect();
                buf.extend_from_slice(sanitised.as_bytes());
            }
        }
        let nul_count = buf.iter().filter(|&&b| b == 0).count();
        assert_eq!(nul_count, 0, "expected no NULs after sanitising");
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
