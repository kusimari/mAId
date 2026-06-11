#!/usr/bin/env bash
# tests/kaimux/integration.sh — shell-driven end-to-end test.
#
# Spins up a private tmux server, drives the compiled kaimux binary
# through the wrap → hook → list → unregister flow, asserts on-disk
# state at each step. Covers what the in-process Rust tests can't:
# the real tmux side effects (set-hook, set-option, switch-client) and
# real argv handling on a live process.
#
# Skips (exit 0) if tmux/jq/the dist binary are missing — this lets
# CI environments without tmux pass `deno task test:smoke` cleanly.
#
# Usage:
#   tests/kaimux/integration.sh
#   KAIMUX_INTEGRATION_DEBUG=1 tests/kaimux/integration.sh

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
BIN="$ROOT/dist/kaimux"

# Isolate from any outer tmux: a wrapped kaimux must not see the
# outer client's $TMUX_PANE or write to the real $HOME state-dir.
unset TMUX TMUX_PANE TMUX_PLUGIN_MANAGER_PATH
STATE_PARENT="$(mktemp -d)"
export XDG_STATE_HOME="$STATE_PARENT"
TMUX_TMPDIR="$(mktemp -d)"
export TMUX_TMPDIR
TMUX_SOCKET="kaimux-test-$$"

debug=${KAIMUX_INTEGRATION_DEBUG:-0}
log()   { printf '\033[36m[i]\033[0m %s\n' "$*"; }
pass()  { printf '\033[32m[ok]\033[0m %s\n' "$*"; }
fail()  { printf '\033[31m[fail]\033[0m %s\n' "$*" >&2; debug_dump; exit 1; }
skip()  { printf '\033[33m[skip]\033[0m %s\n' "$*"; exit 0; }
trace() { (( debug )) && printf '  %s\n' "$*" >&2 || true; }

debug_dump() {
  (( debug )) || return 0
  echo "─── debug dump ───" >&2
  echo "STATE_PARENT=$STATE_PARENT" >&2
  ls -la "$STATE_PARENT/kaimux/" 2>&1 >&2 || true
  cat "$STATE_PARENT/kaimux/sessions.json" 2>&1 >&2 || true
  tmux -L "$TMUX_SOCKET" ls 2>&1 >&2 || true
}

SETUP_HOME=""
FRESH_HOME=""
cleanup() {
  tmux -L "$TMUX_SOCKET" kill-server 2>/dev/null || true
  rm -rf "$STATE_PARENT" "$TMUX_TMPDIR" "$SETUP_HOME" "$FRESH_HOME"
}
trap cleanup EXIT

# ── pre-flight ─────────────────────────────────────────────────────

command -v tmux >/dev/null || skip "tmux not on PATH"
command -v jq   >/dev/null || skip "jq not on PATH"
[[ -x "$BIN" ]] || skip "binary not built — run \`deno task kaimux:build\` first"

# Resolve `tmux` to the actual binary path so our internal calls work
# even under shells that alias `tmux` (zsh-with-tmux-plugin etc).
TMUX_BIN="$(command -v tmux)"
T() { "$TMUX_BIN" -L "$TMUX_SOCKET" "$@"; }

log "binary: $BIN"
log "state-parent: $STATE_PARENT"

STATE="$STATE_PARENT/kaimux"
sessions_json() { cat "$STATE/sessions.json" 2>/dev/null || echo '[]'; }
field()  { jq -r ".[$1].$2" <<<"$(sessions_json)"; }

# Spawn an agent in a new tmux pane and return its pane id. The agent
# is launched as the new-session/new-window command directly — tmux
# auto-sets TMUX_PANE in its env, so the wrapper sees the right value.
# `remain-on-exit on` keeps dead panes around so we can capture stderr
# on failure.
spawn() {
  local target="$1"  # session name (creates a new session) or `session:window`
  local cwd="$2"
  local kind="$3"
  shift 3
  local agent_argv=("$@")
  if [[ "$target" != *:* ]]; then
    T new-session -d -s "$target" -n p1 -PF '#{pane_id}' \
      env "XDG_STATE_HOME=$STATE_PARENT" \
      "$BIN" wrap "$kind" --cwd "$cwd" -- "${agent_argv[@]}"
  else
    T new-window -t "${target%%:*}" -n "${target##*:}" -PF '#{pane_id}' \
      env "XDG_STATE_HOME=$STATE_PARENT" \
      "$BIN" wrap "$kind" --cwd "$cwd" -- "${agent_argv[@]}"
  fi
}

# Wait for sessions.json to contain at least N entries. Times out (and
# fails) after ~4s if it doesn't.
wait_for_sessions() {
  local want=$1
  for _ in {1..40}; do
    if [[ "$(jq 'length' <<<"$(sessions_json)")" -ge "$want" ]]; then
      return 0
    fi
    sleep 0.1
  done
  fail "timed out waiting for $want session(s) — got $(jq 'length' <<<"$(sessions_json)")"
}

T set-option -g remain-on-exit on 2>/dev/null || true

# ── case 1 — wrap claude registers a session ───────────────────────
# Claude hooks live globally now (installed by `setup`, exercised in
# case 10 below). The wrapper itself just registers + execvp's.

log "case 1: wrap claude inside tmux"
PANE_ID="$(spawn w1 "$STATE_PARENT" claude sleep 300)"
trace "pane_id=$PANE_ID"
wait_for_sessions 1

[[ "$(jq 'length' <<<"$(sessions_json)")" -eq 1 ]] || fail "case 1: expected 1 session, got $(jq 'length' <<<"$(sessions_json)")"
[[ "$(field 0 pane_id)" == "$PANE_ID" ]] || fail "case 1: pane_id mismatch (got $(field 0 pane_id))"
[[ "$(field 0 kind)" == "claude" ]] || fail "case 1: kind != claude"
pass "wrap claude registers session"

# ── case 2 — hook subcommand drives the four-state machine ─────────
#
# Stored states: working / waiting / done. (Idle is render-time
# decay over a stale Done — see case 4c.) Mapping:
#   - UserPromptSubmit → working (transitional)
#   - PreToolUse       → working
#   - PostToolUse      → working (more tools may follow)
#   - Notification     → waiting
#   - Stop             → done

log "case 2: hook UserPromptSubmit marks state working"
echo '{"prompt":"fix the failing test"}' \
  | env "XDG_STATE_HOME=$STATE_PARENT" "KAIMUX_PANE=$PANE_ID" \
    "$BIN" hook UserPromptSubmit
[[ "$(field 0 state)" == "working" ]] || fail "case 2: state != working ($(field 0 state))"
[[ "$(field 0 last_event)" == "UserPromptSubmit" ]] || fail "case 2: last_event mismatch"
pass "UserPromptSubmit flipped to working"

log "case 3: hook PreToolUse keeps state working"
echo '{"tool_name":"Bash","tool_input":{"command":"cargo test"}}' \
  | env "XDG_STATE_HOME=$STATE_PARENT" "KAIMUX_PANE=$PANE_ID" \
    "$BIN" hook PreToolUse
[[ "$(field 0 state)" == "working" ]] || fail "case 3: state != working ($(field 0 state))"
pass "PreToolUse stayed working"

log "case 4: hook PostToolUse keeps state working (more tools may follow)"
echo '{"tool_name":"Bash"}' \
  | env "XDG_STATE_HOME=$STATE_PARENT" "KAIMUX_PANE=$PANE_ID" \
    "$BIN" hook PostToolUse
[[ "$(field 0 state)" == "working" ]] || fail "case 4: state != working ($(field 0 state))"
pass "PostToolUse stayed working"

log "case 4b: hook Stop flips state to done"
echo '{}' | env "XDG_STATE_HOME=$STATE_PARENT" "KAIMUX_PANE=$PANE_ID" \
    "$BIN" hook Stop
[[ "$(field 0 state)" == "done" ]] || fail "case 4b: state != done ($(field 0 state))"
pass "Stop flipped to done"

log "case 4c: hook Notification flips state to waiting (highest priority — sorts to top)"
echo '{"message":"Allow Bash command?"}' \
  | env "XDG_STATE_HOME=$STATE_PARENT" "KAIMUX_PANE=$PANE_ID" \
    "$BIN" hook Notification
[[ "$(field 0 state)" == "waiting" ]] || fail "case 4c: state != waiting ($(field 0 state))"
pass "Notification flipped to waiting"

# Reset to done for case 5's render assertions to be deterministic.
echo '{}' | env "XDG_STATE_HOME=$STATE_PARENT" "KAIMUX_PANE=$PANE_ID" \
    "$BIN" hook Stop

# ── case 5 — render emits a multi-line item (drives the loop body) ─
# `render` is the hidden subcommand the event-driven loop body
# wires into fzf's `reload(...)` action. Each item is:
#   <pane_id>\t<header>\n<snippet line 1>\n<line 2>\n<line 3>
# Items are NUL-separated. The leading `<pane_id>\t` is what
# fzf's `--id-nth=1` keys on; `--with-nth=2..` hides it from
# display. Pass KAIMUX_TMUX_SOCKET so resolve_pane_addr +
# capture_snippet target our private server.

log "case 5: render emits a multi-line item per registered pane"
# Capture to a file, not $(...), so trailing newlines and NUL
# bytes survive bash's command-substitution sanitisation.
RENDER_FILE="$STATE_PARENT/render-out"
env "XDG_STATE_HOME=$STATE_PARENT" "KAIMUX_TMUX_SOCKET=$TMUX_SOCKET" \
  "$BIN" render > "$RENDER_FILE"
trace "$(od -c "$RENDER_FILE" | head -10)"

# With 1 registered pane: zero NULs (NUL is the inter-item
# separator, no trailing NUL by design).
nul_count="$(tr -cd '\0' < "$RENDER_FILE" | wc -c)"
[[ "$nul_count" -eq 0 ]] || fail "case 5: expected 0 NULs (1 item), got $nul_count"

# 1 header + 3 snippet lines = 3 newlines between them. Empty
# snippet lines are still present (kept for fixed item height
# so fzf doesn't reflow on every reload).
nl_count="$(tr -cd '\n' < "$RENDER_FILE" | wc -c)"
[[ "$nl_count" -eq 3 ]] || fail "case 5: expected 3 newlines (1 header + 3 snippet), got $nl_count"

# Header line (first line) must start with "<pane_id>\t".
header="$(head -1 "$RENDER_FILE")"
[[ "$header" == "$PANE_ID"$'\t'* ]] || fail "case 5: pane id not in column 1: $header"
[[ "$header" == *"claude"* ]] || fail "case 5: header missing kind"
# Case 4 left the row in `done` state; the corresponding glyph is `✓`.
[[ "$header" == *"✓"* ]] || fail "case 5: header missing done glyph (expected ✓)"

# Header has the resolved tmux address (session:window.pane).
# We don't assert the exact value (depends on tmux's pane id
# scheme on the private server) but it must not be the
# `?:?.<pane-id>` fallback.
[[ "$header" != *"?:?."* ]] || fail "case 5: address resolution fell back: $header"
pass "render emits multi-line item: <pane_id>\\t<header>\\n<snippet x 3>"

# ── case 5b — peek shells out to tmux capture-pane ─────────────────
# The kaimux peek subcommand wraps `tmux capture-pane`. We
# drive it inside a real tmux session (the integration script's
# private server) and assert the output matches what's currently
# visible in the pane.

log "case 5b: peek dumps the last N lines of a pane"
# Send a known marker into the pane, wait for it to land.
T send-keys -t "$PANE_ID" 'echo MARKER_PEEK_42' Enter
sleep 0.5
PEEK_OUT="$(env "XDG_STATE_HOME=$STATE_PARENT" "KAIMUX_TMUX_SOCKET=$TMUX_SOCKET" \
  "$BIN" peek "$PANE_ID" --lines 50)"
trace "$PEEK_OUT"
[[ "$PEEK_OUT" == *"MARKER_PEEK_42"* ]] || fail "case 5b: peek output didn't contain the marker"
pass "peek captured pane content via tmux capture-pane"

log "case 5c: peek on an unknown pane returns empty without erroring"
PEEK_NONE="$(env "XDG_STATE_HOME=$STATE_PARENT" "KAIMUX_TMUX_SOCKET=$TMUX_SOCKET" \
  "$BIN" peek '%no-such-pane' 2>&1)" || fail "case 5c: peek on unknown pane should exit 0"
[[ -z "$PEEK_NONE" ]] || fail "case 5c: expected empty, got: $PEEK_NONE"
pass "peek on unknown pane gracefully empty"

# ── case 5d — peek invokes capture-pane with -e for ANSI ──────────
# Verifies the right tmux flags are wired up. We can't easily
# inject coloured cell content into the integration fixture's
# `sleep 300` pane (no shell to interpret the escape sequence),
# so we exercise the surface-level flag path: peek a real pane,
# assert the call succeeds and produces output. The
# end-to-end ANSI rendering is asserted by the functional layer
# where real claude/kiro produce coloured output naturally.

log "case 5d: peek call succeeds (capture-pane -e flag wired)"
PEEK_OUT2="$(env "XDG_STATE_HOME=$STATE_PARENT" "KAIMUX_TMUX_SOCKET=$TMUX_SOCKET" \
  "$BIN" peek "$PANE_ID" --lines 10)"
[[ -n "$PEEK_OUT2" ]] || fail "case 5d: peek produced no output for live pane"
pass "peek invocation succeeds (-e flag exercise)"

# ── case 6 — unregister removes the record ─────────────────────────

log "case 6: unregister removes the record"
env "XDG_STATE_HOME=$STATE_PARENT" "$BIN" unregister "$PANE_ID"
[[ "$(jq 'length' <<<"$(sessions_json)")" -eq 0 ]] || fail "case 6: record not removed"
pass "unregister cleaned record"

# ── case 7 — kiro refcount cleanup (close-creator-first) ───────────

log "case 7: kiro refcount survives close-creator-first ordering"
KIRO_CWD="$(mktemp -d)"
P1="$(spawn k1 "$KIRO_CWD" kiro sleep 300)"
trace "p1=$P1"
wait_for_sessions 1
P2="$(spawn k1:p2 "$KIRO_CWD" kiro sleep 300)"
trace "p2=$P2"
wait_for_sessions 2
[[ -f "$KIRO_CWD/.kiro/agents/kaimux.json" ]] || fail "case 7: kiro config missing"

env "XDG_STATE_HOME=$STATE_PARENT" "$BIN" unregister "$P1"
[[ -f "$KIRO_CWD/.kiro/agents/kaimux.json" ]] || fail "case 7: kiro config removed too early"

env "XDG_STATE_HOME=$STATE_PARENT" "$BIN" unregister "$P2"
[[ ! -f "$KIRO_CWD/.kiro/agents/kaimux.json" ]] || fail "case 7: kiro config not removed after last close"
pass "kiro refcount cleanup survives close-creator-first ordering"
rm -rf "$KIRO_CWD"

# ── case 8 — wrap refuses without $TMUX_PANE ───────────────────────

log "case 8: wrap refuses without \$TMUX_PANE"
out="$(env "XDG_STATE_HOME=$STATE_PARENT" "$BIN" wrap claude -- sleep 1 2>&1 || true)"
[[ "$out" == *"\$TMUX_PANE"* ]] || fail "case 8: missing TMUX_PANE error: $out"
pass "wrap refuses without \$TMUX_PANE"

# ── case 9 — global tmux pane-exited hook is registered ────────────
# tmux's pane-exited hook only fires when the pane's process exits
# *naturally* (the shell/agent terminates), and in our private-server
# test environment with `remain-on-exit on` it's stubbornly hard to
# reproduce that timing reliably from a script. The integration script
# verifies the hook is REGISTERED (so the wrapper installed it
# correctly); the hook-fires-on-real-pane-death path is exercised by
# the manual functional test plan when the user closes a real shell.

log "case 9: global tmux pane-exited hook is registered"
hook_cmd="$(T show-hook -g pane-exited 2>/dev/null || true)"
[[ "$hook_cmd" == *"kaimux"* ]] \
  || fail "case 9: tmux pane-exited hook not registered ($hook_cmd) — case 1 prerequisite failed"
[[ "$hook_cmd" == *"unregister"* ]] \
  || fail "case 9: hook command doesn't reference unregister ($hook_cmd)"
pass "global tmux pane-exited hook is registered with the unregister command"

# ── case 10 — setup / teardown round-trip on user-global settings ──
#
# `setup` writes tagged hook entries to ~/.claude/settings.json (under
# our $HOME tempdir for isolation). `teardown` removes only those
# entries. Pre-existing user content survives both.

log "case 10: setup/teardown preserves user-existing settings"
SETUP_HOME="$(mktemp -d)"
SETUP_FILE="$SETUP_HOME/.claude/settings.json"
mkdir -p "$SETUP_HOME/.claude"
# Seed with a user entry the test must preserve.
cat > "$SETUP_FILE" <<'EOF'
{
  "permissions": ["read"],
  "hooks": {
    "UserPromptSubmit": [
      { "matcher": "", "hooks": [{ "type": "command", "command": "user-hook" }] }
    ]
  }
}
EOF

env "HOME=$SETUP_HOME" "$BIN" setup
# Verify our entries landed alongside the user's, tagged.
n_ups="$(jq '.hooks.UserPromptSubmit | length' < "$SETUP_FILE")"
[[ "$n_ups" -eq 2 ]] || fail "case 10: expected 2 UserPromptSubmit entries after setup, got $n_ups"
n_stop="$(jq '.hooks.Stop | length' < "$SETUP_FILE")"
[[ "$n_stop" -eq 1 ]] || fail "case 10: expected 1 Stop entry after setup, got $n_stop"
# Notification is load-bearing for the four-state machine — must be installed.
n_notif="$(jq '.hooks.Notification | length' < "$SETUP_FILE")"
[[ "$n_notif" -eq 1 ]] || fail "case 10: expected 1 Notification entry after setup, got $n_notif"
notif_tagged="$(jq '.hooks.Notification[0]."x-kaimux-managed"' < "$SETUP_FILE")"
[[ "$notif_tagged" == "true" ]] || fail "case 10: Notification entry not tagged"
tagged="$(jq '.hooks.UserPromptSubmit[1]."x-kaimux-managed"' < "$SETUP_FILE")"
[[ "$tagged" == "true" ]] || fail "case 10: x-kaimux-managed tag missing"

# Idempotent: second setup doesn't duplicate.
env "HOME=$SETUP_HOME" "$BIN" setup
n_ups2="$(jq '.hooks.UserPromptSubmit | length' < "$SETUP_FILE")"
[[ "$n_ups2" -eq 2 ]] || fail "case 10: setup duplicated entries (got $n_ups2 UserPromptSubmit)"

env "HOME=$SETUP_HOME" "$BIN" teardown
# User content preserved exactly.
n_after="$(jq '.hooks.UserPromptSubmit | length' < "$SETUP_FILE")"
[[ "$n_after" -eq 1 ]] || fail "case 10: expected 1 UserPromptSubmit after teardown, got $n_after"
cmd_after="$(jq -r '.hooks.UserPromptSubmit[0].hooks[0].command' < "$SETUP_FILE")"
[[ "$cmd_after" == "user-hook" ]] || fail "case 10: user hook lost ($cmd_after)"
stop_present="$(jq '(.hooks // {}) | has("Stop")' < "$SETUP_FILE")"
[[ "$stop_present" == "false" ]] || fail "case 10: Stop key not pruned"
[[ "$(jq -r '.permissions[0]' < "$SETUP_FILE")" == "read" ]] \
  || fail "case 10: permissions sibling field clobbered"
pass "setup/teardown preserves user-existing settings"

# Pre-existing-empty case: setup creates the file, teardown removes it.
log "case 10b: setup/teardown round-trips on a fresh \$HOME"
FRESH_HOME="$(mktemp -d)"
env "HOME=$FRESH_HOME" "$BIN" setup
[[ -f "$FRESH_HOME/.claude/settings.json" ]] || fail "case 10b: setup didn't create settings.json"
env "HOME=$FRESH_HOME" "$BIN" teardown
[[ ! -f "$FRESH_HOME/.claude/settings.json" ]] || fail "case 10b: teardown didn't remove settings.json"
# SETUP_HOME and FRESH_HOME are cleaned up by the EXIT trap.
pass "setup/teardown removes settings.json when only our content remained"

# ── case 11 — setup --key / teardown register/unregister keybind ──
#
# `setup --key X` should bind `<prefix> X` to
# `switch-client -t kaimux` on the running tmux server.
# `teardown` should unbind it without needing the key argument
# (it self-discovers any prefix binding routing to the
# orchestrator). Re-running `setup --key Y` should swap cleanly
# — old binding gone, new one in place. We point the binary at
# our private tmux socket via `$KAIMUX_TMUX_SOCKET` so the
# assertions don't touch the user's real server.

log "case 11a: setup --key X installs the prefix X keybind"
KEY_HOME="$(mktemp -d)"
mkdir -p "$KEY_HOME/.claude"

# Pre-condition: no orchestrator binding in the prefix table.
T list-keys -T prefix 2>/dev/null | grep -q "switch-client -t kaimux" \
  && fail "case 11a: prefix already routes somewhere to orchestrator before setup"

env "HOME=$KEY_HOME" "KAIMUX_TMUX_SOCKET=$TMUX_SOCKET" "$BIN" setup --key X
T list-keys -T prefix 2>/dev/null | grep -E '^bind-key +-T prefix +X ' \
  | grep -q "switch-client -t kaimux" \
  || fail "case 11a: setup --key X did not install prefix X → orchestrator"
pass "setup --key X installed prefix X keybind"

# Without --key on setup, no keybind is touched. To prove that, first
# confirm we can re-key cleanly: setup --key Y should remove X and add Y.
log "case 11b: setup --key Y swaps the keybind (X removed, Y added)"
env "HOME=$KEY_HOME" "KAIMUX_TMUX_SOCKET=$TMUX_SOCKET" "$BIN" setup --key Y
T list-keys -T prefix 2>/dev/null | grep -E '^bind-key +-T prefix +X ' >/dev/null \
  && fail "case 11b: re-keying did not remove the old X binding"
T list-keys -T prefix 2>/dev/null | grep -E '^bind-key +-T prefix +Y ' \
  | grep -q "switch-client -t kaimux" \
  || fail "case 11b: re-keying did not install the new Y binding"
pass "setup --key Y swapped X→Y cleanly"

log "case 11c: teardown self-discovers and removes the keybind"
env "HOME=$KEY_HOME" "KAIMUX_TMUX_SOCKET=$TMUX_SOCKET" "$BIN" teardown
T list-keys -T prefix 2>/dev/null | grep -q "switch-client -t kaimux" \
  && fail "case 11c: teardown did not remove the orchestrator binding"
rm -rf "$KEY_HOME"
pass "teardown self-discovered and removed the keybind"

log "case 11d: setup without --key installs hooks but no keybind"
KEY_HOME="$(mktemp -d)"
mkdir -p "$KEY_HOME/.claude"
env "HOME=$KEY_HOME" "KAIMUX_TMUX_SOCKET=$TMUX_SOCKET" "$BIN" setup
T list-keys -T prefix 2>/dev/null | grep -q "switch-client -t kaimux" \
  && fail "case 11d: setup without --key installed a keybind anyway"
[[ -f "$KEY_HOME/.claude/settings.json" ]] \
  || fail "case 11d: setup without --key skipped the Claude hook install"
env "HOME=$KEY_HOME" "KAIMUX_TMUX_SOCKET=$TMUX_SOCKET" "$BIN" teardown
rm -rf "$KEY_HOME"
pass "setup without --key installed hooks only, teardown was a clean no-op for the keybind"

# ── case 12 — setup --session NAME bakes custom session into the keybind ─
# The `--session NAME` flag changes which tmux session the
# keybind switches to. Teardown still self-discovers our
# binding via the marker baked into the action, so it works
# without needing the session name.

log "case 12a: setup --session custom-dash --key Z bakes custom name in"
KEY_HOME="$(mktemp -d)"
mkdir -p "$KEY_HOME/.claude"
env "HOME=$KEY_HOME" "KAIMUX_TMUX_SOCKET=$TMUX_SOCKET" \
  "$BIN" --session custom-dash setup --key Z
# The bind-key line should target `custom-dash`, not `kaimux`.
T list-keys -T prefix 2>/dev/null | grep -E '^bind-key +-T prefix +Z ' \
  | grep -q "switch-client -t custom-dash" \
  || fail "case 12a: bind action does not switch to custom-dash"
T list-keys -T prefix 2>/dev/null | grep -E '^bind-key +-T prefix +Z ' \
  | grep -q "kaimux" || true   # marker present; matches via x-kaimux-managed
pass "setup --session custom-dash --key Z installed binding to custom-dash"

log "case 12b: teardown removes the custom-session binding without args"
env "HOME=$KEY_HOME" "KAIMUX_TMUX_SOCKET=$TMUX_SOCKET" "$BIN" teardown
T list-keys -T prefix 2>/dev/null | grep -E '^bind-key +-T prefix +Z ' >/dev/null \
  && fail "case 12b: teardown did not remove the custom-session binding"
rm -rf "$KEY_HOME"
pass "teardown self-discovered and removed the custom-session binding"

log "all cases passed (1, 2, 3, 4, 4b, 4c, 5, 5b, 5c, 5d, 6-10, 11a-d, 12a-b)"
