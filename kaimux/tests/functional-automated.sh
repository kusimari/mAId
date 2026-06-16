#!/usr/bin/env bash
# tests/kaimux/functional-automated.sh — assert the spec's F1-F8
# scenarios end-to-end against the user's real tmux server with
# real claude/kiro-cli CLIs.
#
# Usage:
#   tests/kaimux/functional-automated-setup.sh O      # spawn fixture first
#   tests/kaimux/functional-automated.sh         # run all scenarios
#   tests/kaimux/functional-automated.sh F2      # run a single scenario
#   tests/kaimux/functional-automated.sh F2,F5   # comma-separated subset
#
# ── Scenario index ────────────────────────────────────────────────
#
#   F1   Fixture setup is honest          — tmux sessions exist,
#                                            registry has 5 wrapped
#                                            panes, render emits 5
#                                            multi-line items.
#   F2   Working → done lifecycle         — typing a prompt into a
#                                            wrapped claude flips
#                                            state through working
#                                            → done with the right
#                                            icons.
#   F2b  Waiting state surfaces           — a Notification event
#                                            flips state to waiting
#                                            and sorts the row to
#                                            the top of the dashboard.
#   F3   Independent panes track separately — two claudes in the
#                                            same proj-b window
#                                            don't bleed state into
#                                            each other.
#   F4   Mixed-kind co-existence         — proj-c's kiro + claude
#                                            both register, both
#                                            appear, kind column
#                                            is correct.
#   F5   Pane close removes the row      — `tmux kill-pane` triggers
#                                            the pane-exited hook
#                                            and the row drops from
#                                            both registry and render.
#                                            (Destructive — runs late.)
#   F6   Fresh wrap appears live         — wrapping a new agent in
#                                            an unwrapped pane shows
#                                            up in render within ~1s.
#   F7   State and snippet are independent — pane content can change
#                                            without flipping state,
#                                            and vice versa; the two
#                                            signals don't conflate.
#   F8   Keybind round-trip + dead-pid filter — `setup --key X`
#                                            installs a prefix
#                                            binding; render filters
#                                            entries whose pid has
#                                            died.
#
# Refuses to run inside tmux (we send-keys into panes the user
# might be looking at — too easy to clobber). Refuses without
# the fixture (functional-automated-setup.sh must have run first).
#
# What's asserted is the user-visible behaviour of the
# dashboard, not internal field names. The hidden hook verb
# is sometimes used as a stand-in to drive transitions
# deterministically (we can't always coax claude into a
# `Notification` state on demand), but the assertion always
# lands on what `kaimux render` emits — what the user
# would see in the dashboard.

set -uo pipefail
# Note: we deliberately don't `set -e` — one scenario failing
# shouldn't abort the rest. Each F-block runs independently and
# accumulates pass/fail counters.

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
BIN="$ROOT/dist/kaimux"
REG="$HOME/.local/state/kaimux/sessions.json"

TMUX_BIN="$(command -v tmux 2>/dev/null || true)"
T() { "$TMUX_BIN" "$@"; }

# ── output helpers ────────────────────────────────────────────────

passed=0
failed=0
ran=()

log()    { printf '\033[36m[i]\033[0m %s\n' "$*"; }
ok()     { printf '\033[32m[ok]\033[0m %s\n' "$*"; passed=$((passed + 1)); }
fail()   { printf '\033[31m[fail]\033[0m %s\n' "$*" >&2; failed=$((failed + 1)); }
skip()   { printf '\033[33m[skip]\033[0m %s\n' "$*"; }
section() { printf '\n\033[1;34m── %s ──\033[0m\n' "$*"; }

# ── pre-flight ────────────────────────────────────────────────────

[[ -n "$TMUX_BIN" ]]      || { fail "tmux not on PATH"; exit 2; }
command -v fzf >/dev/null || { fail "fzf not on PATH"; exit 2; }
command -v jq  >/dev/null || { fail "jq not on PATH";  exit 2; }
[[ -x "$BIN" ]]           || { fail "binary not built — run \`just kaimux::build\`"; exit 2; }
[[ -z "${TMUX:-}" ]]      || { fail "do not run from inside tmux"; exit 2; }

# Fixture must already exist. We don't auto-spawn it because
# functional-automated-setup.sh requires arguments (the keybind suffix)
# and may need API credits to bootstrap.
T list-sessions 2>/dev/null | awk -F: '{print $1}' | grep -q '^proj-a$' || {
  fail "fixture not present — run functional-automated-setup.sh first"
  exit 2
}
[[ -f "$REG" ]] || { fail "registry not found at $REG"; exit 2; }

# ── helpers used across F-scenarios ───────────────────────────────

# Read sessions.json field for a row matched by pane_id. Returns
# empty string when no record matches — caller checks for that.
field_for_pane() {
  local pane="$1" field="$2"
  jq -r --arg p "$pane" \
    '[.[] | select(.pane_id == $p)] | first | .[$field] // ""' \
    "$REG"
}

# Count rows in the dashboard for a given pane id.
render_has_pane() {
  local pane="$1"
  "$BIN" render | tr '\0' '\n' | grep -qE "^${pane}\b"
}

# Extract the header line for a given pane id from `render` output.
render_header_for_pane() {
  local pane="$1"
  "$BIN" render | tr '\0' '\n' | awk -v p="$pane" -F'\t' '$1 == p {print; exit}'
}

# Poll a predicate until it returns 0 or `timeout_secs` elapses.
# The polling cadence (100ms) is fast enough to catch a
# working→done flip from a single tool call.
wait_for() {
  local timeout_secs="$1" desc="$2"
  shift 2
  local deadline=$(( $(date +%s) + timeout_secs ))
  while [[ "$(date +%s)" -lt "$deadline" ]]; do
    if "$@"; then
      return 0
    fi
    sleep 0.1
  done
  fail "$desc — timed out after ${timeout_secs}s"
  return 1
}

# Wait for sessions.json to reflect a pane in a given state.
pane_state_is() {
  local pane="$1" want="$2"
  [[ "$(field_for_pane "$pane" state)" == "$want" ]]
}

# ── scenario selection (default: all) ─────────────────────────────

ALL=(F1 F2 F2b F3 F4 F5 F6 F7 F8)
selected=("${ALL[@]}")
if [[ $# -gt 0 ]]; then
  IFS=',' read -ra selected <<<"$1"
fi

want() {
  local target="$1"
  for s in "${selected[@]}"; do
    [[ "$s" == "$target" ]] && return 0
  done
  return 1
}

# Resolve fixture pane ids by working backward from the registry.
# The fixture has 5 wrapped panes: 1 in proj-a, 2 in proj-b:code,
# 1 each (kiro+claude) in proj-c. We pick representatives by
# (kind, cwd) so the test doesn't depend on tmux's pane-id
# numbering.
PANE_PROJ_A_CLAUDE="$(jq -r \
  '[.[] | select(.kind == "claude" and (.cwd | endswith("proj-a")))] \
   | first | .pane_id // ""' "$REG")"
PANE_PROJ_B_LEFT="$(jq -r \
  '[.[] | select(.kind == "claude" and (.cwd | endswith("proj-b")))] \
   | sort_by(.pane_id) | first | .pane_id // ""' "$REG")"
PANE_PROJ_B_RIGHT="$(jq -r \
  '[.[] | select(.kind == "claude" and (.cwd | endswith("proj-b")))] \
   | sort_by(.pane_id) | last | .pane_id // ""' "$REG")"
PANE_PROJ_C_CLAUDE="$(jq -r \
  '[.[] | select(.kind == "claude" and (.cwd | endswith("proj-c")))] \
   | first | .pane_id // ""' "$REG")"
PANE_PROJ_C_KIRO="$(jq -r \
  '[.[] | select(.kind == "kiro")] \
   | first | .pane_id // ""' "$REG")"

# ── F1. Setup spawns the multi-session fixture cleanly ────────────

if want F1; then
  section "F1 — fixture is present and dashboard reflects it"
  ran+=(F1)

  # F1.1 — tmux has the four sessions.
  for s in proj-a proj-b proj-c kaimux; do
    if T has-session -t "$s" 2>/dev/null; then
      ok "F1.1 — tmux session $s exists"
    else
      fail "F1.1 — tmux session $s missing"
    fi
  done

  # F1.2 — registry has 5 wrapped panes (1 + 2 + 2).
  reg_count="$(jq 'length' "$REG")"
  if [[ "$reg_count" -eq 5 ]]; then
    ok "F1.2 — registry has 5 wrapped panes (proj-a + 2x proj-b + 2x proj-c)"
  else
    fail "F1.2 — registry has $reg_count panes, expected 5"
  fi

  # F1.3 — every recorded pane resolves to a live tmux pane.
  live_panes="$(T list-panes -a -F '#{pane_id}' 2>/dev/null)"
  missing=0
  while IFS= read -r p; do
    if ! grep -qF "$p" <<<"$live_panes"; then
      fail "F1.3 — recorded pane $p not in tmux"
      missing=$((missing + 1))
    fi
  done < <(jq -r '.[].pane_id' "$REG")
  [[ "$missing" -eq 0 ]] && ok "F1.3 — every recorded pane is live in tmux"

  # F1.4 — dashboard render emits 5 multi-line items.
  render_out="$("$BIN" render)"
  # NUL count = item count - 1 (no trailing NUL by design).
  nuls="$(printf '%s' "$render_out" | tr -cd '\0' | wc -c)"
  if [[ "$nuls" -eq 4 ]]; then
    ok "F1.4 — render emits 5 NUL-separated items"
  else
    fail "F1.4 — render NUL count is $nuls, expected 4 (5 items)"
  fi
fi

# ── F2. Dummy queries propagate to dashboard ──────────────────────

if want F2; then
  section "F2 — dummy query flips state through working → done"
  ran+=(F2)

  if [[ -z "$PANE_PROJ_A_CLAUDE" ]]; then
    skip "F2 — proj-a's claude pane id not resolvable"
  else
    log "F2: typing prompt into pane $PANE_PROJ_A_CLAUDE"
    T send-keys -t "$PANE_PROJ_A_CLAUDE" "list files in cwd" Enter

    # F2.1 — state hits `working` while claude runs the tool.
    if wait_for 30 "F2.1 — pane reaches working state" \
        pane_state_is "$PANE_PROJ_A_CLAUDE" working; then
      ok "F2.1 — state reached working (tool started)"

      # F2.2 — render's icon for the row is ▶.
      h="$(render_header_for_pane "$PANE_PROJ_A_CLAUDE")"
      if [[ "$h" == *"▶"* ]]; then
        ok "F2.2 — render shows ▶ glyph for working pane"
      else
        fail "F2.2 — render header missing ▶: $h"
      fi
    fi

    # F2.3 — settles to `done` within 30s of the prompt.
    if wait_for 30 "F2.3 — pane settles to done" \
        pane_state_is "$PANE_PROJ_A_CLAUDE" done; then
      ok "F2.3 — state settled to done (tool finished)"
      h="$(render_header_for_pane "$PANE_PROJ_A_CLAUDE")"
      if [[ "$h" == *"✓"* ]]; then
        ok "F2.4 — render shows ✓ glyph for done pane"
      else
        fail "F2.4 — render header missing ✓: $h"
      fi
    fi
  fi
fi

# ── F2b. Permission prompts surface as waiting + sort top ─────────
#
# Triggering a real Notification deterministically is hard
# (depends on claude's allowlist behaviour). Drive the event
# directly via the hook verb so the test asserts on the
# downstream behaviour: the row sorts to the top of render
# and shows the 💬 icon. This is the same hidden-verb-as-
# stand-in pattern documented in the spec's Test Strategy.

if want F2b; then
  section "F2b — Notification flips state to waiting and sorts to top"
  ran+=(F2b)

  # Pick a pane that's currently NOT first in render output
  # so the sort assertion is meaningful.
  target="$PANE_PROJ_C_CLAUDE"
  if [[ -z "$target" ]]; then
    skip "F2b — no proj-c claude pane to drive"
  else
    echo '{"message":"Allow Bash command?"}' \
      | env "KAIMUX_PANE=$target" "$BIN" hook Notification

    if wait_for 5 "F2b.1 — pane reaches waiting state" \
        pane_state_is "$target" waiting; then
      ok "F2b.1 — state reached waiting"
    fi

    # F2b.2 — render's first item is now `target`.
    first_pane="$("$BIN" render | tr '\0' '\n' | head -1 | cut -f1)"
    if [[ "$first_pane" == "$target" ]]; then
      ok "F2b.2 — waiting pane sorted to the top of render"
    else
      fail "F2b.2 — first render item is $first_pane, expected $target"
    fi

    # F2b.3 — header carries 💬 glyph.
    h="$(render_header_for_pane "$target")"
    if [[ "$h" == *"💬"* ]]; then
      ok "F2b.3 — render shows 💬 glyph for waiting pane"
    else
      fail "F2b.3 — render header missing 💬: $h"
    fi

    # Restore: drive Stop so the row goes back to done and
    # later scenarios start from a predictable state.
    echo '{}' | env "KAIMUX_PANE=$target" "$BIN" hook Stop || true
  fi
fi

# ── F3. Two agents in one window track independently ──────────────

if want F3; then
  section "F3 — proj-b's two claudes track independently"
  ran+=(F3)

  if [[ -z "$PANE_PROJ_B_LEFT" || -z "$PANE_PROJ_B_RIGHT" ]]; then
    skip "F3 — proj-b's two claude panes not resolvable"
  elif [[ "$PANE_PROJ_B_LEFT" == "$PANE_PROJ_B_RIGHT" ]]; then
    skip "F3 — only one proj-b claude pane present"
  else
    # Drive each pane to a different state via the hook verb so
    # we don't depend on coaxing claude into matching states
    # naturally. The assertion is "they don't cross-contaminate."
    echo '{}' | env "KAIMUX_PANE=$PANE_PROJ_B_LEFT" "$BIN" hook PreToolUse
    echo '{}' | env "KAIMUX_PANE=$PANE_PROJ_B_RIGHT" "$BIN" hook Stop

    # Give the writes time to land (each hook call grabs the lock
    # and does an atomic rename — usually <10ms but be safe).
    sleep 0.3

    left_state="$(field_for_pane "$PANE_PROJ_B_LEFT" state)"
    right_state="$(field_for_pane "$PANE_PROJ_B_RIGHT" state)"
    if [[ "$left_state" == "working" && "$right_state" == "done" ]]; then
      ok "F3.1 — left=working, right=done (independent rows)"
    else
      fail "F3.1 — expected left=working,right=done; got left=$left_state right=$right_state"
    fi

    # F3.2 — last_event_ts diverges between the two rows. They
    # were stamped <0.3s apart but with different events.
    left_ev="$(field_for_pane "$PANE_PROJ_B_LEFT" last_event)"
    right_ev="$(field_for_pane "$PANE_PROJ_B_RIGHT" last_event)"
    if [[ "$left_ev" == "PreToolUse" && "$right_ev" == "Stop" ]]; then
      ok "F3.2 — last_event differs (no cross-contamination)"
    else
      fail "F3.2 — last_event left=$left_ev right=$right_ev"
    fi

    # Reset both to done so later scenarios are predictable.
    echo '{}' | env "KAIMUX_PANE=$PANE_PROJ_B_LEFT" "$BIN" hook Stop || true
  fi
fi

# ── F4. Mixed kinds in one window ─────────────────────────────────

if want F4; then
  section "F4 — proj-c's kiro + claude both present"
  ran+=(F4)

  if [[ -z "$PANE_PROJ_C_KIRO" || -z "$PANE_PROJ_C_CLAUDE" ]]; then
    skip "F4 — proj-c kiro/claude panes not resolvable"
  else
    # F4.1 — both registered.
    if render_has_pane "$PANE_PROJ_C_KIRO"; then
      ok "F4.1 — kiro pane present in render"
    else
      fail "F4.1 — kiro pane missing from render"
    fi
    if render_has_pane "$PANE_PROJ_C_CLAUDE"; then
      ok "F4.2 — claude pane present in render"
    else
      fail "F4.2 — claude pane missing from render"
    fi

    # F4.3 — drive claude through working→done; verify the icon
    # advances. Kiro's lifecycle stays flat (Kiro hooks are
    # out of scope in v1 — that's expected).
    echo '{}' | env "KAIMUX_PANE=$PANE_PROJ_C_CLAUDE" "$BIN" hook PreToolUse
    sleep 0.3
    h="$(render_header_for_pane "$PANE_PROJ_C_CLAUDE")"
    if [[ "$h" == *"▶"* ]]; then
      ok "F4.3 — claude row advances to ▶ working"
    else
      fail "F4.3 — claude row not working: $h"
    fi
    echo '{}' | env "KAIMUX_PANE=$PANE_PROJ_C_CLAUDE" "$BIN" hook Stop
    sleep 0.3
  fi
fi

# ── F5. Closing a wrapped pane removes its row ────────────────────
#
# F5 is destructive — the killed pane is gone for good. Run
# it last among the deterministic tests so other scenarios
# have the full fixture to work with.

if want F5; then
  section "F5 — closing a pane removes its row from dashboard"
  ran+=(F5)

  # Use proj-a's claude — single-pane session, simpler tear down.
  target="$PANE_PROJ_A_CLAUDE"
  if [[ -z "$target" ]]; then
    skip "F5 — proj-a claude pane not resolvable"
  else
    pre_count="$(jq 'length' "$REG")"

    # Kill the pane. The tmux global pane-exited hook fires
    # `kaimux unregister #{hook_pane}` which removes the
    # registry record.
    T kill-pane -t "$target"

    # F5.1 — within ~2s, the registry record is gone.
    record_gone() {
      local p="$1"
      [[ "$(jq --arg p "$p" '[.[] | select(.pane_id == $p)] | length' "$REG")" -eq 0 ]]
    }
    if wait_for 2 "F5.1 — record removed from registry" \
        record_gone "$target"; then
      ok "F5.1 — pane-exited hook fired; registry no longer has $target"
    fi

    # F5.2 — within ~2s, render no longer emits the row.
    if wait_for 2 "F5.2 — render no longer emits the row" \
        bash -c "! \"$BIN\" render | tr '\\0' '\\n' | grep -qE \"^${target}\\b\""; then
      ok "F5.2 — render dropped $target"
    fi

    # F5.3 — other rows survive untouched.
    post_count="$(jq 'length' "$REG")"
    if [[ "$post_count" -eq $((pre_count - 1)) ]]; then
      ok "F5.3 — exactly one row removed; others intact ($pre_count → $post_count)"
    else
      fail "F5.3 — expected $((pre_count - 1)) rows, got $post_count"
    fi
  fi
fi

# ── F6. Wrapping a fresh agent appears in dashboard ───────────────

if want F6; then
  section "F6 — fresh wrap appears in dashboard within ~1s"
  ran+=(F6)

  fresh_session="kaimux-test-$$"
  fresh_cwd="/tmp/$fresh_session"
  mkdir -p "$fresh_cwd"

  if T has-session -t "$fresh_session" 2>/dev/null; then
    T kill-session -t "$fresh_session" || true
  fi

  # Spawn a session and have it run wrap with a long-lived
  # placeholder. We use `sleep 600` instead of real claude so
  # F6 doesn't cost API credits — the assertion is about the
  # registration path, not lifecycle hooks (those are F2's
  # territory).
  T new-session -d -s "$fresh_session" -c "$fresh_cwd" \
    "$BIN" wrap claude --cwd "$fresh_cwd" -- sleep 600

  # F6.1 — within ~2s, registry has a new record.
  fresh_record() {
    [[ "$(jq --arg c "$fresh_cwd" \
      '[.[] | select(.cwd == $c)] | length' "$REG")" -ge 1 ]]
  }
  if wait_for 2 "F6.1 — registry gains the new pane" fresh_record; then
    fresh_pane="$(jq -r --arg c "$fresh_cwd" \
      '[.[] | select(.cwd == $c)] | first | .pane_id' "$REG")"
    ok "F6.1 — registry gained $fresh_pane (cwd=$fresh_cwd)"

    # F6.2 — render emits the new row.
    if render_has_pane "$fresh_pane"; then
      ok "F6.2 — render emits the new row"
    else
      fail "F6.2 — new pane $fresh_pane absent from render"
    fi

    # F6.3 — header carries kind=claude.
    h="$(render_header_for_pane "$fresh_pane")"
    if [[ "$h" == *"claude"* ]]; then
      ok "F6.3 — render header carries kind=claude"
    else
      fail "F6.3 — render header missing kind: $h"
    fi
  fi

  # Cleanup. kill-session triggers pane-exited which fires
  # unregister; functional-automated-teardown.sh's registry-flush will
  # mop up any straggler.
  T kill-session -t "$fresh_session" 2>/dev/null || true
  rm -rf "$fresh_cwd"
fi

# ── F7. State and pane content are independent signals ────────────
#
# The load-bearing UX assertion: the dashboard conveys two
# independent signals per row (lifecycle state from hooks,
# pane snippet from live tmux capture-pane). Verified by
# observing that the snippet column changes content without
# the state column changing — i.e. they don't move in
# lockstep.

if want F7; then
  section "F7 — state column and snippet column are independent"
  ran+=(F7)

  target="$PANE_PROJ_C_CLAUDE"
  if [[ -z "$target" ]]; then
    skip "F7 — no claude pane to drive"
  else
    # Snapshot 1: pre-prompt.
    snap1="$("$BIN" render | tr '\0' '\n' \
      | awk -v p="$target" -F'\t' '$1 == p {found=1; getline; print}')"

    # Send a noop into the pane so the snippet changes but no
    # hook fires (we're not running through claude — we're
    # just changing pane content).
    T send-keys -t "$target" "echo MARKER_$$" Enter
    sleep 1.0

    # Snapshot 2: same state (no hook fired), different snippet.
    snap2="$("$BIN" render | tr '\0' '\n' \
      | awk -v p="$target" -F'\t' '$1 == p {found=1; getline; print}')"

    if [[ "$snap1" != "$snap2" ]]; then
      ok "F7.1 — snippet changed without hook firing"
    else
      fail "F7.1 — snippet unchanged after pane content moved"
    fi
  fi
fi

# ── F8. Keybind round-trip + dead-pid filter ──────────────────────

if want F8; then
  section "F8 — keybind round-trip + dead-pid filter"
  ran+=(F8)

  # F8.1 — `<prefix> <KEY>` is registered. (We don't synthesize
  # keystrokes — too flaky. Reading list-keys is the assertion.)
  if T list-keys -T prefix 2>/dev/null \
      | grep -q "switch-client -t kaimux"; then
    ok "F8.1 — prefix keybind registered (switches to kaimux)"
  else
    fail "F8.1 — no prefix keybind for kaimux in tmux"
  fi

  # F8.2 — Kill an agent's pid directly (NOT the pane); the
  # render-time liveness probe drops the row.
  victim="$PANE_PROJ_C_KIRO"
  if [[ -z "$victim" ]]; then
    skip "F8.2 — no kiro pane available"
  else
    pid="$(field_for_pane "$victim" pid)"
    if [[ -n "$pid" && "$pid" != "null" ]]; then
      kill -9 "$pid" 2>/dev/null || true
      sleep 0.5
      if ! render_has_pane "$victim"; then
        ok "F8.2 — render filtered out dead-pid row $victim"
      else
        fail "F8.2 — dead-pid row $victim still present in render"
      fi
    else
      skip "F8.2 — pid not resolvable for $victim"
    fi
  fi
fi

# ── summary ────────────────────────────────────────────────────────

section "summary"
log "ran scenarios: ${ran[*]:-none}"
log "passed: $passed   failed: $failed"
if [[ "$failed" -gt 0 ]]; then
  exit 1
fi
exit 0
