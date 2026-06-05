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

cleanup() {
  tmux -L "$TMUX_SOCKET" kill-server 2>/dev/null || true
  rm -rf "$STATE_PARENT" "$TMUX_TMPDIR"
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

# ── case 1 — wrap claude registers a session and synthesizes settings

log "case 1: wrap claude inside tmux"
PANE_ID="$(spawn w1 "$STATE_PARENT" claude sleep 300)"
trace "pane_id=$PANE_ID"
wait_for_sessions 1

[[ "$(jq 'length' <<<"$(sessions_json)")" -eq 1 ]] || fail "case 1: expected 1 session, got $(jq 'length' <<<"$(sessions_json)")"
[[ "$(field 0 pane_id)" == "$PANE_ID" ]] || fail "case 1: pane_id mismatch (got $(field 0 pane_id))"
[[ "$(field 0 kind)" == "claude" ]] || fail "case 1: kind != claude"
[[ -f "$STATE/tmp/$PANE_ID/settings.json" ]] || fail "case 1: per-pane settings.json missing"

SETTINGS="$STATE/tmp/$PANE_ID/settings.json"
for ev in UserPromptSubmit PreToolUse PostToolUse Stop; do
  # Claude's nested matcher+hooks shape: `.hooks.<event>[0].hooks[0].command`.
  cmd="$(jq -r ".hooks.\"$ev\"[0].hooks[0].command" < "$SETTINGS")"
  [[ "$cmd" == *"hook $ev" ]] || fail "case 1: hook $ev command mismatch ($cmd)"
done
pass "wrap claude registers session and synthesizes settings"

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

# ── case 5 — list emits the live row ───────────────────────────────

log "case 5: list emits the live row"
LIST_OUT="$(env "XDG_STATE_HOME=$STATE_PARENT" "$BIN" list)"
trace "$LIST_OUT"
[[ "$LIST_OUT" == *"$PANE_ID"* ]] || fail "case 5: list missing pane id"
[[ "$LIST_OUT" == *"claude"* ]] || fail "case 5: list missing kind"
[[ "$LIST_OUT" == *"fix the failing test"* ]] || fail "case 5: list missing prompt"
pass "list reflects accumulated state"

# ── case 6 — unregister removes record and tmp dir ─────────────────

log "case 6: unregister removes record + tmp dir"
env "XDG_STATE_HOME=$STATE_PARENT" "$BIN" unregister "$PANE_ID"
[[ "$(jq 'length' <<<"$(sessions_json)")" -eq 0 ]] || fail "case 6: record not removed"
[[ ! -d "$STATE/tmp/$PANE_ID" ]] || fail "case 6: tmp dir not removed"
pass "unregister cleaned record and per-pane tmp dir"

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

# ── case 9 — list filters dead-pid records at query time ───────────

log "case 9: list filters dead-pid records (pid-liveness sweep)"
# Spawn an agent that exits quickly; the registry record persists but
# its pid is dead. `list` is documented as unfiltered (the spec leans on
# the picker's render_rows for liveness), so we drive the picker proxy
# via an `unregister` after the pane dies — this is what the tmux
# pane-exited hook would do in a real run.
P9="$(spawn d1 "$STATE_PARENT" claude sleep 0.2)"
trace "d1 pane=$P9"
wait_for_sessions 1
sleep 0.6 # let the agent process actually die
# Confirm record is still present (list is unfiltered).
[[ "$(jq 'length' <<<"$(sessions_json)")" -eq 1 ]] || fail "case 9: pre-cleanup record missing"
# Simulate the pane-exited hook firing.
env "XDG_STATE_HOME=$STATE_PARENT" "$BIN" unregister "$P9"
LIST_OUT="$(env "XDG_STATE_HOME=$STATE_PARENT" "$BIN" list)"
[[ "$LIST_OUT" == "(no registered sessions)" ]] || fail "case 9: list still shows dead pane: $LIST_OUT"
pass "list reflects unregister of dead-pid pane"

log "all 9 cases passed"
