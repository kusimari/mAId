#!/usr/bin/env bash
# tests/agent-orch/integration.sh — shell-driven end-to-end test.
#
# Spins up a private tmux server, drives the compiled agent-orch binary
# through the wrap → hook → list → unregister flow, asserts on-disk
# state at each step. Covers what the in-process Rust tests can't:
# the real tmux side effects (set-hook, set-option, switch-client) and
# real argv handling on a live process.
#
# Skips (exit 0) if tmux/jq/the dist binary are missing — this lets
# CI environments without tmux pass `deno task test:smoke` cleanly.
#
# Usage:
#   tests/agent-orch/integration.sh
#   AGENT_ORCH_INTEGRATION_DEBUG=1 tests/agent-orch/integration.sh

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
BIN="$ROOT/dist/agent-orch/agent-orch"

# Isolate from any outer tmux: a wrapped agent-orch must not see the
# outer client's $TMUX_PANE or write to the real $HOME state-dir.
unset TMUX TMUX_PANE TMUX_PLUGIN_MANAGER_PATH
STATE_PARENT="$(mktemp -d)"
export XDG_STATE_HOME="$STATE_PARENT"
TMUX_TMPDIR="$(mktemp -d)"
export TMUX_TMPDIR
TMUX_SOCKET="agent-orch-test-$$"

debug=${AGENT_ORCH_INTEGRATION_DEBUG:-0}
log()   { printf '\033[36m[i]\033[0m %s\n' "$*"; }
pass()  { printf '\033[32m[ok]\033[0m %s\n' "$*"; }
fail()  { printf '\033[31m[fail]\033[0m %s\n' "$*" >&2; debug_dump; exit 1; }
skip()  { printf '\033[33m[skip]\033[0m %s\n' "$*"; exit 0; }
trace() { (( debug )) && printf '  %s\n' "$*" >&2 || true; }

debug_dump() {
  (( debug )) || return 0
  echo "─── debug dump ───" >&2
  echo "STATE_PARENT=$STATE_PARENT" >&2
  ls -la "$STATE_PARENT/agent-orch/" 2>&1 >&2 || true
  cat "$STATE_PARENT/agent-orch/sessions.json" 2>&1 >&2 || true
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
[[ -x "$BIN" ]] || skip "binary not built — run \`deno task agent-orch:build\` first"

# Resolve `tmux` to the actual binary path so our internal calls work
# even under shells that alias `tmux` (zsh-with-tmux-plugin etc).
TMUX_BIN="$(command -v tmux)"
T() { "$TMUX_BIN" -L "$TMUX_SOCKET" "$@"; }

log "binary: $BIN"
log "state-parent: $STATE_PARENT"

STATE="$STATE_PARENT/agent-orch"
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

# ── case 2 — hook subcommand updates state ─────────────────────────

log "case 2: hook UserPromptSubmit flips state to running"
echo '{"prompt":"fix the failing test"}' \
  | env "XDG_STATE_HOME=$STATE_PARENT" "AGENT_ORCH_PANE=$PANE_ID" \
    "$BIN" hook UserPromptSubmit
[[ "$(field 0 state)" == "running" ]] || fail "case 2: state != running ($(field 0 state))"
[[ "$(field 0 last_prompt)" == "fix the failing test" ]] || fail "case 2: last_prompt mismatch"
pass "hook UserPromptSubmit marked running and stored prompt"

log "case 3: hook PreToolUse updates last_tool"
echo '{"tool_name":"Bash"}' \
  | env "XDG_STATE_HOME=$STATE_PARENT" "AGENT_ORCH_PANE=$PANE_ID" \
    "$BIN" hook PreToolUse
[[ "$(field 0 last_tool)" == "Bash" ]] || fail "case 3: last_tool != Bash"
pass "hook PreToolUse stored tool name"

log "case 4: hook Stop flips state to complete"
echo '{}' | env "XDG_STATE_HOME=$STATE_PARENT" "AGENT_ORCH_PANE=$PANE_ID" \
    "$BIN" hook Stop
[[ "$(field 0 state)" == "complete" ]] || fail "case 4: state != complete"
pass "hook Stop marked complete"

# ── case 5 — render emits the live row (drives the loop body) ─────
# `render` is the hidden subcommand the event-driven loop body
# wires into fzf's `reload(...)` action. Each row is
# `<pane_id>\t<formatted-row>`; pane id sits in column 1 so fzf's
# `--id-nth=1 --with-nth=2..` can track and hide it respectively.

log "case 5: render emits a tab-separated row per registered pane"
RENDER_OUT="$(env "XDG_STATE_HOME=$STATE_PARENT" "$BIN" render)"
trace "$RENDER_OUT"
[[ "$(printf '%s\n' "$RENDER_OUT" | wc -l)" -eq 1 ]] || fail "case 5: expected 1 row"
[[ "$RENDER_OUT" == "$PANE_ID"$'\t'* ]] || fail "case 5: pane id not in column 1"
[[ "$RENDER_OUT" == *"claude"* ]] || fail "case 5: render missing kind"
[[ "$RENDER_OUT" == *"fix the failing test"* ]] || fail "case 5: render missing prompt"
pass "render emits tab-separated rows in picker-sort order"

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
[[ -f "$KIRO_CWD/.kiro/agents/agent-orch.json" ]] || fail "case 7: kiro config missing"

env "XDG_STATE_HOME=$STATE_PARENT" "$BIN" unregister "$P1"
[[ -f "$KIRO_CWD/.kiro/agents/agent-orch.json" ]] || fail "case 7: kiro config removed too early"

env "XDG_STATE_HOME=$STATE_PARENT" "$BIN" unregister "$P2"
[[ ! -f "$KIRO_CWD/.kiro/agents/agent-orch.json" ]] || fail "case 7: kiro config not removed after last close"
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
[[ "$hook_cmd" == *"agent-orch"* ]] \
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
tagged="$(jq '.hooks.UserPromptSubmit[1]."x-agent-orch-managed"' < "$SETUP_FILE")"
[[ "$tagged" == "true" ]] || fail "case 10: x-agent-orch-managed tag missing"

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
# `switch-client -t orchestrator` on the running tmux server.
# `teardown` should unbind it without needing the key argument
# (it self-discovers any prefix binding routing to the
# orchestrator). Re-running `setup --key Y` should swap cleanly
# — old binding gone, new one in place. We point the binary at
# our private tmux socket via `$AGENT_ORCH_TMUX_SOCKET` so the
# assertions don't touch the user's real server.

log "case 11a: setup --key X installs the prefix X keybind"
KEY_HOME="$(mktemp -d)"
mkdir -p "$KEY_HOME/.claude"

# Pre-condition: no orchestrator binding in the prefix table.
T list-keys -T prefix 2>/dev/null | grep -q "switch-client -t orchestrator" \
  && fail "case 11a: prefix already routes somewhere to orchestrator before setup"

env "HOME=$KEY_HOME" "AGENT_ORCH_TMUX_SOCKET=$TMUX_SOCKET" "$BIN" setup --key X
T list-keys -T prefix 2>/dev/null | grep -E '^bind-key +-T prefix +X ' \
  | grep -q "switch-client -t orchestrator" \
  || fail "case 11a: setup --key X did not install prefix X → orchestrator"
pass "setup --key X installed prefix X keybind"

# Without --key on setup, no keybind is touched. To prove that, first
# confirm we can re-key cleanly: setup --key Y should remove X and add Y.
log "case 11b: setup --key Y swaps the keybind (X removed, Y added)"
env "HOME=$KEY_HOME" "AGENT_ORCH_TMUX_SOCKET=$TMUX_SOCKET" "$BIN" setup --key Y
T list-keys -T prefix 2>/dev/null | grep -E '^bind-key +-T prefix +X ' >/dev/null \
  && fail "case 11b: re-keying did not remove the old X binding"
T list-keys -T prefix 2>/dev/null | grep -E '^bind-key +-T prefix +Y ' \
  | grep -q "switch-client -t orchestrator" \
  || fail "case 11b: re-keying did not install the new Y binding"
pass "setup --key Y swapped X→Y cleanly"

log "case 11c: teardown self-discovers and removes the keybind"
env "HOME=$KEY_HOME" "AGENT_ORCH_TMUX_SOCKET=$TMUX_SOCKET" "$BIN" teardown
T list-keys -T prefix 2>/dev/null | grep -q "switch-client -t orchestrator" \
  && fail "case 11c: teardown did not remove the orchestrator binding"
rm -rf "$KEY_HOME"
pass "teardown self-discovered and removed the keybind"

log "case 11d: setup without --key installs hooks but no keybind"
KEY_HOME="$(mktemp -d)"
mkdir -p "$KEY_HOME/.claude"
env "HOME=$KEY_HOME" "AGENT_ORCH_TMUX_SOCKET=$TMUX_SOCKET" "$BIN" setup
T list-keys -T prefix 2>/dev/null | grep -q "switch-client -t orchestrator" \
  && fail "case 11d: setup without --key installed a keybind anyway"
[[ -f "$KEY_HOME/.claude/settings.json" ]] \
  || fail "case 11d: setup without --key skipped the Claude hook install"
env "HOME=$KEY_HOME" "AGENT_ORCH_TMUX_SOCKET=$TMUX_SOCKET" "$BIN" teardown
rm -rf "$KEY_HOME"
pass "setup without --key installed hooks only, teardown was a clean no-op for the keybind"

log "all cases passed (1-10 + 11a/b/c/d)"
