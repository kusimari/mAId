#!/usr/bin/env bash
# tests/kaimux/functional-automated-setup.sh — spin up the functional fixture.
#
# Usage: functional-automated-setup.sh <KEY>
#
# <KEY> is the tmux prefix-table suffix to bind for "switch back
# to the orchestrator session" — e.g. `O` makes
# `<your-tmux-prefix> O` jump to the kaimux session. Pick any
# unbound key in your prefix table; pass `none` to install hooks
# only with no keybind.
#
# Spawns four sessions on the user's running tmux server:
#
#   proj-a       1 window, 1 pane: claude wrapped
#   proj-b       2 windows. window 2 ("code") has a horizontal split
#                with TWO claudes side-by-side, both wrapped, both
#                rooted at /tmp/proj-b — exercises multiple agents
#                in one window sharing a cwd
#   proj-c       1 window, vertical split — kiro on top, claude
#                bottom (both wrapped)
#   kaimux   the orchestrator session itself, hosting the fzf
#                picker. Bootstrapped detached; you attach with
#                `tmux attach -t kaimux`.
#
# Then runs `kaimux setup --key <KEY>` (or `setup` if <KEY> is
# `none`) to install the user-global Claude hooks and the
# orchestrator-switch keybind.
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
#   $ tmux attach -t kaimux        # already running, picker visible
#   $ <prefix> <KEY>                   # from any wrapped pane, jumps back here
#
# Tear down with: tests/kaimux/functional-automated-teardown.sh

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
BIN="$ROOT/dist/kaimux"

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
SESSIONS=(proj-a proj-b proj-c kaimux)

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

# ── helpers ────────────────────────────────────────────────────────

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

# ── proj-a — single-pane Claude session ────────────────────────────

log "proj-a (single-pane Claude)"
ensure_session proj-a /tmp/proj-a
PANE_A="$(pane_of proj-a)"
if already_wrapped "$PANE_A"; then
  skip "proj-a · pane $PANE_A already wrapped — leaving alone"
else
  send_wrap "$PANE_A" /tmp/proj-a claude claude
  ok "proj-a · claude wrapped (pane $PANE_A)"
fi

# ── proj-b — two windows; window 2 = two claudes side-by-side ──────
#
# Window 1 ("notes") is just a plain shell. Window 2 ("code") has a
# horizontal split: claude on the left, claude on the right, BOTH
# wrapped, BOTH rooted at /tmp/proj-b. Exercises:
#   - two separate claude lifecycles per pane (the registry must
#     keep both rows distinct via $TMUX_PANE / $KAIMUX_PANE);
#   - two claudes sharing one cwd (no per-cwd refcount logic for
#     claude — kiro is the only kind with that pattern, so this
#     should be a no-op aside from the two registry rows);
#   - hook events fire independently per pane → picker shows each
#     row advancing on its own.

log "proj-b (two windows; window 2 = two claudes side-by-side)"
ensure_session proj-b /tmp/proj-b
PROJB_WIN1="$(T list-windows -t proj-b -F '#{window_id}' | head -1)"
PROJB_WIN_COUNT="$(T list-windows -t proj-b | wc -l)"
if [[ "$PROJB_WIN_COUNT" -lt 2 ]]; then
  T rename-window -t "$PROJB_WIN1" notes
  T new-window -t proj-b -n code -c /tmp/proj-b
  T split-window -h -t proj-b:code -c /tmp/proj-b
fi
# Resolve left and right panes by `pane_left` (column position) —
# don't trust pane index ordering after splits.
PROJB_CODE_LEFT="$(T list-panes -t proj-b:code -F '#{pane_id} #{pane_left}' \
  | sort -k2 -n | awk 'NR==1{print $1}')"
PROJB_CODE_RIGHT="$(T list-panes -t proj-b:code -F '#{pane_id} #{pane_left}' \
  | sort -k2 -n | awk 'NR==2{print $1}')"
if already_wrapped "$PROJB_CODE_LEFT"; then
  skip "proj-b · left pane $PROJB_CODE_LEFT already wrapped"
else
  send_wrap "$PROJB_CODE_LEFT" /tmp/proj-b claude claude
  ok "proj-b · claude wrapped (pane $PROJB_CODE_LEFT, window 'code', left)"
fi
if already_wrapped "$PROJB_CODE_RIGHT"; then
  skip "proj-b · right pane $PROJB_CODE_RIGHT already wrapped"
else
  send_wrap "$PROJB_CODE_RIGHT" /tmp/proj-b claude claude
  ok "proj-b · claude wrapped (pane $PROJB_CODE_RIGHT, window 'code', right)"
fi

# ── proj-c — vertical split, Kiro top + Claude bottom ──────────────

log "proj-c (Kiro top, Claude bottom)"
ensure_session proj-c /tmp/proj-c
PROJC_PANE_COUNT="$(T list-panes -t proj-c | wc -l)"
if [[ "$PROJC_PANE_COUNT" -lt 2 ]]; then
  T split-window -v -t proj-c -c /tmp/proj-c
fi
# Sort panes by `pane_top` so we get top→bottom regardless of how
# tmux numbered them after the split.
PROJC_TOP="$(T list-panes -t proj-c -F '#{pane_id} #{pane_top}' \
  | sort -k2 -n | awk 'NR==1{print $1}')"
PROJC_BOTTOM="$(T list-panes -t proj-c -F '#{pane_id} #{pane_top}' \
  | sort -k2 -n | awk 'NR==2{print $1}')"
if already_wrapped "$PROJC_BOTTOM"; then
  skip "proj-c · bottom pane $PROJC_BOTTOM already wrapped"
else
  send_wrap "$PROJC_BOTTOM" /tmp/proj-c claude claude
  ok "proj-c · claude wrapped (pane $PROJC_BOTTOM, bottom)"
fi
if already_wrapped "$PROJC_TOP"; then
  skip "proj-c · top pane $PROJC_TOP already wrapped"
else
  send_wrap "$PROJC_TOP" /tmp/proj-c kiro kiro-cli
  ok "proj-c · kiro wrapped (pane $PROJC_TOP, top)"
fi

# ── install user-global hooks + (optional) keybind ─────────────────

if [[ "$KEY" == "none" ]]; then
  log "installing user-global Claude hooks (no keybind)"
  "$BIN" setup
else
  log "installing user-global Claude hooks + prefix '$KEY' keybind"
  "$BIN" setup --key "$KEY"
fi
ok "setup complete"

# ── kaimux — bootstrap the orchestrator session itself ─────────
#
# Bare `kaimux` from a non-tmux shell creates the orchestrator
# session detached, then tries `switch-client -t kaimux` —
# which fails with "no current client" since we're outside tmux.
# That's fine: the session is created and running the body, the
# user attaches later. Replicate just the session-create half here
# without the failing switch-client call.

if T has-session -t kaimux 2>/dev/null; then
  skip "kaimux session already exists"
else
  log "kaimux (the orchestrator session itself)"
  T new-session -d -s kaimux "$BIN"
  ok "kaimux session ready (running fzf picker)"
fi

# ── done ───────────────────────────────────────────────────────────

if [[ "$KEY" == "none" ]]; then
  switch_hint="no keybind installed — use \`tmux switch-client -t kaimux\` directly"
else
  switch_hint="<tmux-prefix> $KEY from any wrapped pane → kaimux"
fi

cat <<EOF

  Sessions ready: ${SESSIONS[*]}

    Attach the orchestrator picker:
      tmux attach -t kaimux

    Switch back from any wrapped pane:
      $switch_hint

  Tear down with:
    $HERE/functional-automated-teardown.sh

EOF
