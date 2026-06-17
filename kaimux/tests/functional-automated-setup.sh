#!/usr/bin/env bash
# tests/kaimux/functional-automated-setup.sh — spin up the functional fixture.
#
# Usage: functional-automated-setup.sh <KEY>
#
# <KEY> is the tmux prefix-table suffix to bind for "switch back
# to the orchestrator session" — e.g. `O` makes
# `<your-tmux-prefix> O` jump to the dashboard session. Pick any
# unbound key in your prefix table; pass `none` to install hooks
# only with no keybind.
#
# All sessions and cwds carry the `kaimux-test-` prefix so the
# fixture can never collide with the user's real sessions or
# real project directories. Re-running the script unconditionally
# kills any pre-existing kaimux-test-* sessions first — safe
# because the prefix is private to this fixture.
#
# Spawns four sessions on the user's running tmux server:
#
#   kaimux-test-proj-a    1 window, 1 pane: claude wrapped
#   kaimux-test-proj-b    2 windows. window 2 ("code") has a horizontal
#                          split with TWO claudes side-by-side, both
#                          wrapped, both rooted at /tmp/kaimux-test-proj-b
#                          — exercises multiple agents in one window
#                          sharing a cwd
#   kaimux-test-proj-c    1 window, vertical split — kiro on top,
#                          claude on bottom (both wrapped)
#   kaimux-test-dashboard the orchestrator session hosting the fzf
#                          picker. Bootstrapped detached; you attach
#                          with `tmux attach -t kaimux-test-dashboard`.
#
# Then runs `kaimux setup --session kaimux-test-dashboard --key <KEY>`
# (or just `--session …` if <KEY> is `none`) to install the
# user-global Claude hooks and the dashboard-switch keybind.
#
# Why launch agents via `tmux send-keys` into a fresh login shell
# (rather than as the new-session command directly): the user's
# zsh login shell is what promotes `~/.toolbox/bin` to the front
# of $PATH. Running the wrap as a non-login shell would resolve
# `claude` to whatever's first in the bare-PATH ordering (often
# `~/.local/bin/claude`, the no-Bedrock-auth standalone). By
# letting zsh -l finish initializing first, then sending the wrap
# invocation, the wrapper's execvp resolves the toolbox shim.
#
# Hand-off:
#
#   $ tmux attach -t kaimux-test-dashboard
#   $ <prefix> <KEY>      # from any wrapped pane, jumps to dashboard
#
# Tear down with: tests/kaimux/functional-automated-teardown.sh

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
BIN="$ROOT/dist/kaimux"

# Hardcoded prefix scopes every session + cwd this fixture creates,
# so re-running unconditionally kills only ours, never the user's.
PREFIX="kaimux-test-"
DASHBOARD="${PREFIX}dashboard"
PROJ_A="${PREFIX}proj-a"
PROJ_B="${PREFIX}proj-b"
PROJ_C="${PREFIX}proj-c"
CWD_A="/tmp/$PROJ_A"
CWD_B="/tmp/$PROJ_B"
CWD_C="/tmp/$PROJ_C"

# ── argv ───────────────────────────────────────────────────────────

if [[ $# -ne 1 ]]; then
  echo "usage: $0 <KEY>" >&2
  echo "       $0 O          # bind <prefix> O to switch-client" >&2
  echo "       $0 none       # install hooks only, no keybind" >&2
  exit 2
fi
KEY="$1"

# Resolve the real tmux on PATH (avoids zsh-tmux-plugin alias clobbering).
TMUX_BIN="$(command -v tmux)"

# Sessions this script creates. Teardown reads the same list.
SESSIONS=("$PROJ_A" "$PROJ_B" "$PROJ_C" "$DASHBOARD")

log()  { printf '\033[36m[i]\033[0m %s\n' "$*"; }
ok()   { printf '\033[32m[ok]\033[0m %s\n' "$*"; }
skip() { printf '\033[33m[skip]\033[0m %s\n' "$*"; }
fail() { printf '\033[31m[fail]\033[0m %s\n' "$*" >&2; exit 1; }

T() { "$TMUX_BIN" "$@"; }

# ── pre-flight ─────────────────────────────────────────────────────

[[ -n "$TMUX_BIN" ]]      || fail "tmux not on PATH"
command -v fzf >/dev/null || fail "fzf not on PATH"
[[ -x "$BIN" ]]           || fail "binary not built — run \`just kaimux::build\` first"
T list-sessions >/dev/null 2>&1 || fail "no tmux server running — start one first"

# Refuse to run inside tmux: send-keys timing into a pane the user
# might be looking at is asking for trouble, and the orchestrator
# bootstrap step needs an *outside* client to switch.
[[ -z "${TMUX:-}" ]] || fail "do not run this from inside tmux — run it from a plain terminal"

# ── pre-kill any leftover fixture sessions ─────────────────────────
#
# Idempotent re-run: kill anything matching our private prefix.
# Safe because real user sessions can't legitimately use the
# `kaimux-test-` prefix.

for s in "${SESSIONS[@]}"; do
  if T has-session -t "$s" 2>/dev/null; then
    T kill-session -t "$s" 2>/dev/null && log "killed pre-existing session: $s"
  fi
done

# Wipe any leftover registry entries for the previous fixture so
# F1.2's count assertion starts from zero.
REG="$HOME/.local/state/kaimux/sessions.json"
if [[ -f "$REG" ]]; then
  printf '[]' > "$REG"
  log "cleared registry: $REG"
fi

# ── helpers ────────────────────────────────────────────────────────

# Always launch panes with `zsh -l` so the user's login PATH (the
# one that promotes ~/.toolbox/bin to the front) is in effect. The
# wrapper's execvp then resolves `claude` / `kiro-cli` via the
# toolbox shims rather than whatever the bare-PATH ordering picks.
ensure_session() {
  local name="$1" cwd="$2"
  if T has-session -t "$name" 2>/dev/null; then
    return 0
  fi
  mkdir -p "$cwd"
  T new-session -d -s "$name" -c "$cwd" zsh -l
}

# True if the registry has a record for the given pane id.
already_wrapped() {
  local pane="$1" reg="$HOME/.local/state/kaimux/sessions.json"
  [[ -f "$reg" ]] || return 1
  command grep -q "\"pane_id\": \"$pane\"" "$reg" 2>/dev/null
}

# Send the wrap invocation to a target pane. The pane's zsh login
# shell does its full PATH setup before we type, so `claude` /
# `kiro-cli` resolve via toolbox shims.
send_wrap() {
  local target="$1" cwd="$2" kind="$3"
  shift 3
  local agent_argv=("$@")
  local cmd
  printf -v cmd '%q wrap %q --cwd %q --' "$BIN" "$kind" "$cwd"
  for a in "${agent_argv[@]}"; do
    printf -v cmd '%s %q' "$cmd" "$a"
  done
  # Wait for shell prompt to settle before typing. ~250ms suffices
  # in practice; bump if you see commands lost on slow machines.
  sleep 0.3
  T send-keys -t "$target" "$cmd" Enter
}

pane_of() { T display-message -p -t "$1" '#{pane_id}'; }

# ── install user-global hooks + (optional) keybind FIRST ──────────
#
# Claude reads ~/.claude/settings.json at startup, so hooks have
# to be in place before we spawn any wrapped claudes. Install the
# Notification + the four lifecycle hooks here so each claude
# inherits them on first invocation.

if [[ "$KEY" == "none" ]]; then
  log "installing user-global Claude hooks targeting $DASHBOARD (no keybind)"
  "$BIN" --session "$DASHBOARD" setup
else
  log "installing user-global Claude hooks + prefix '$KEY' keybind → $DASHBOARD"
  "$BIN" --session "$DASHBOARD" setup --key "$KEY"
fi
ok "setup complete (hooks installed before agents start)"

# ── proj-a — single-pane Claude session ────────────────────────────

log "$PROJ_A (single-pane Claude)"
ensure_session "$PROJ_A" "$CWD_A"
PANE_A="$(pane_of "$PROJ_A")"
if already_wrapped "$PANE_A"; then
  skip "$PROJ_A · pane $PANE_A already wrapped — leaving alone"
else
  send_wrap "$PANE_A" "$CWD_A" claude claude
  ok "$PROJ_A · claude wrapped (pane $PANE_A)"
fi

# ── proj-b — two windows; window 2 = two claudes side-by-side ──────
#
# Window 1 ("notes") is just a plain shell. Window 2 ("code") has a
# horizontal split: claude on the left, claude on the right, BOTH
# wrapped, BOTH rooted at $CWD_B. Exercises:
#   - two separate claude lifecycles per pane (the registry must
#     keep both rows distinct via $TMUX_PANE / $KAIMUX_PANE);
#   - two claudes sharing one cwd (no per-cwd refcount logic for
#     claude — kiro is the only kind with that pattern, so this
#     should be a no-op aside from the two registry rows);
#   - hook events fire independently per pane → picker shows each
#     row advancing on its own.

log "$PROJ_B (two windows; window 2 = two claudes side-by-side)"
ensure_session "$PROJ_B" "$CWD_B"
PROJB_WIN1="$(T list-windows -t "$PROJ_B" -F '#{window_id}' | head -1)"
PROJB_WIN_COUNT="$(T list-windows -t "$PROJ_B" | wc -l)"
if [[ "$PROJB_WIN_COUNT" -lt 2 ]]; then
  T rename-window -t "$PROJB_WIN1" notes
  T new-window -t "$PROJ_B" -n code -c "$CWD_B"
  T split-window -h -t "$PROJ_B:code" -c "$CWD_B"
fi
# Resolve left and right panes by `pane_left` (column position) —
# don't trust pane index ordering after splits.
PROJB_CODE_LEFT="$(T list-panes -t "$PROJ_B:code" -F '#{pane_id} #{pane_left}' \
  | sort -k2 -n | awk 'NR==1{print $1}')"
PROJB_CODE_RIGHT="$(T list-panes -t "$PROJ_B:code" -F '#{pane_id} #{pane_left}' \
  | sort -k2 -n | awk 'NR==2{print $1}')"
if already_wrapped "$PROJB_CODE_LEFT"; then
  skip "$PROJ_B · left pane $PROJB_CODE_LEFT already wrapped"
else
  send_wrap "$PROJB_CODE_LEFT" "$CWD_B" claude claude
  ok "$PROJ_B · claude wrapped (pane $PROJB_CODE_LEFT, window 'code', left)"
fi
if already_wrapped "$PROJB_CODE_RIGHT"; then
  skip "$PROJ_B · right pane $PROJB_CODE_RIGHT already wrapped"
else
  send_wrap "$PROJB_CODE_RIGHT" "$CWD_B" claude claude
  ok "$PROJ_B · claude wrapped (pane $PROJB_CODE_RIGHT, window 'code', right)"
fi

# ── proj-c — vertical split, Kiro top + Claude bottom ──────────────

log "$PROJ_C (Kiro top, Claude bottom)"
ensure_session "$PROJ_C" "$CWD_C"
PROJC_PANE_COUNT="$(T list-panes -t "$PROJ_C" | wc -l)"
if [[ "$PROJC_PANE_COUNT" -lt 2 ]]; then
  T split-window -v -t "$PROJ_C" -c "$CWD_C"
fi
# Sort panes by `pane_top` so we get top→bottom regardless of how
# tmux numbered them after the split.
PROJC_TOP="$(T list-panes -t "$PROJ_C" -F '#{pane_id} #{pane_top}' \
  | sort -k2 -n | awk 'NR==1{print $1}')"
PROJC_BOTTOM="$(T list-panes -t "$PROJ_C" -F '#{pane_id} #{pane_top}' \
  | sort -k2 -n | awk 'NR==2{print $1}')"
if already_wrapped "$PROJC_BOTTOM"; then
  skip "$PROJ_C · bottom pane $PROJC_BOTTOM already wrapped"
else
  send_wrap "$PROJC_BOTTOM" "$CWD_C" claude claude
  ok "$PROJ_C · claude wrapped (pane $PROJC_BOTTOM, bottom)"
fi
if already_wrapped "$PROJC_TOP"; then
  skip "$PROJ_C · top pane $PROJC_TOP already wrapped"
else
  send_wrap "$PROJC_TOP" "$CWD_C" kiro kiro-cli
  ok "$PROJ_C · kiro wrapped (pane $PROJC_TOP, top)"
fi

# ── dashboard — bootstrap the orchestrator session itself ──────────
#
# Bare `kaimux` from a non-tmux shell creates the dashboard session
# detached, then tries `switch-client -t <name>` — which fails with
# "no current client" since we're outside tmux. That's fine: the
# session is created and running the body, the user attaches later.
# Replicate just the session-create half here without the failing
# switch-client call. Pass --session through so the session's
# startup command (a child kaimux) self-identifies as the dashboard.

if T has-session -t "$DASHBOARD" 2>/dev/null; then
  skip "$DASHBOARD already exists"
else
  log "$DASHBOARD (the orchestrator session itself)"
  T new-session -d -s "$DASHBOARD" "$BIN --session $DASHBOARD"
  ok "$DASHBOARD ready (running fzf picker)"
fi

# ── wait for all 5 wraps to register ───────────────────────────────
#
# `send_wrap` types the wrap command into a fresh login shell — claude
# / kiro-cli launch + register asynchronously. Setup's contract is
# "fixture is ready on exit", so block here until the registry has 5
# rows or we hit a timeout. Without this the test script can race
# ahead and resolve panes from a partial registry.

REG="$HOME/.local/state/kaimux/sessions.json"
log "waiting for all 5 wraps to register (up to 30s)"
deadline=$(( $(date +%s) + 30 ))
while [[ "$(date +%s)" -lt "$deadline" ]]; do
  count="$(jq 'length' "$REG" 2>/dev/null || echo 0)"
  if [[ "$count" -ge 5 ]]; then
    ok "registry has $count wrapped panes"
    break
  fi
  sleep 0.5
done
count="$(jq 'length' "$REG" 2>/dev/null || echo 0)"
if [[ "$count" -lt 5 ]]; then
  fail "fixture incomplete: registry has $count panes after 30s, expected 5"
fi

# ── done ───────────────────────────────────────────────────────────

if [[ "$KEY" == "none" ]]; then
  switch_hint="no keybind installed — use \`tmux switch-client -t $DASHBOARD\` directly"
else
  switch_hint="<tmux-prefix> $KEY from any wrapped pane → $DASHBOARD"
fi

cat <<EOF

  Sessions ready: ${SESSIONS[*]}

    Attach the orchestrator picker:
      tmux attach -t $DASHBOARD

    Switch back from any wrapped pane:
      $switch_hint

  Tear down with:
    $HERE/functional-automated-teardown.sh

EOF
