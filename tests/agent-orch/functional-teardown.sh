#!/usr/bin/env bash
# tests/agent-orch/functional-teardown.sh — undo functional-setup.sh.
#
# Kills the four sessions the setup script creates (proj-a, proj-b,
# proj-c, viewer) plus the orchestrator session if you bootstrapped
# one, runs `agent-orch teardown` to remove the user-global Claude
# hooks and M-o keybind, and clears the registry file. Idempotent
# — safe to run when nothing is set up.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
BIN="$ROOT/dist/agent-orch/agent-orch"

TMUX_BIN="$(command -v tmux)"

SESSIONS=(orchestrator proj-a proj-b proj-c viewer)
REG="$HOME/.local/state/agent-orch/sessions.json"

ok()   { printf '\033[32m[ok]\033[0m %s\n' "$*"; }
skip() { printf '\033[33m[skip]\033[0m %s\n' "$*"; }
warn() { printf '\033[33m[warn]\033[0m %s\n' "$*" >&2; }

T() { "$TMUX_BIN" "$@"; }

# ── kill the sessions we created ───────────────────────────────────

if [[ -z "$TMUX_BIN" ]]; then
  warn "tmux not on PATH — skipping session cleanup"
elif ! T list-sessions >/dev/null 2>&1; then
  skip "no tmux server running"
else
  for s in "${SESSIONS[@]}"; do
    if T has-session -t "$s" 2>/dev/null; then
      T kill-session -t "$s" && ok "killed session $s"
    else
      skip "session $s not present"
    fi
  done
fi

# ── undo setup (Claude hooks + M-o keybind) ────────────────────────

if [[ -x "$BIN" ]]; then
  "$BIN" teardown
  ok "removed Claude hooks + M-o keybind"
else
  warn "binary at $BIN missing — skipping teardown"
fi

# ── clear the registry ────────────────────────────────────────────
#
# The pane-exited tmux hook normally drives `agent-orch unregister`
# for each closing pane. But we just kill-session'd everything in
# one go on the live server; depending on tmux version + timing
# some of those events might not have fired before the server-side
# state for those panes was gone. Truncate the registry to be sure
# the next setup starts fresh.

if [[ -f "$REG" ]]; then
  printf '[]' > "$REG"
  ok "cleared registry: $REG"
fi

# Also clean up any project-scoped Kiro configs the wrapper wrote
# during the test cycle. Refcount-agnostic cleanup normally handles
# this on unregister, but again, we may have raced ahead.
for d in /tmp/proj-a /tmp/proj-b /tmp/proj-c; do
  if [[ -f "$d/.kiro/agents/agent-orch.json" ]]; then
    rm -f "$d/.kiro/agents/agent-orch.json"
    rmdir "$d/.kiro/agents" 2>/dev/null || true
    rmdir "$d/.kiro" 2>/dev/null || true
    ok "cleared kiro config under $d"
  fi
done

ok "teardown complete"
