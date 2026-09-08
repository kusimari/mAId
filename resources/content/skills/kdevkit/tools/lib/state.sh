#!/bin/sh
# kdevkit feature loop — WHAT THE REPOSITORY SHOWS.
#
# This file only reads. It computes facts and answers whether a stage change
# is legal; it never writes, never installs, and never decides what to do
# about an answer. Sourced by two callers who must agree on the facts:
#
#   ../driver              what a coding agent invokes
#   ../hooks/*             what git invokes
#
# Read this first: everything else is a consumer of these answers.

# shellcheck disable=SC2034  # constants are read by the files that source this
STAGES="research planning dev review closure closed"
TRAILER_STAGE="Kdevkit-Feature-Stage"
TRAILER_RETURN="Kdevkit-Feature-Return"
TRAILER_ACK="Kdevkit-Feature-Ack"
TRAILER_EXCEPTION="Kdevkit-Feature-Exception"
# Sentinel identifying a hook as ours, so install can recognise a copy of
# itself at any path rather than only at the one it knows about.
MARKER="kdevkit-feature-loop-hook"

die() { echo "feature-loop: $*" >&2; exit 2; }
in_repo() { git rev-parse --git-dir >/dev/null 2>&1 || die "not a git repository"; }
git_path() { git rev-parse --git-path "$1"; }
intent_file() { git_path kdevkit-intent; }
verified_file() { git rev-parse --git-path kdevkit-verified; }

branch() { git rev-parse --abbrev-ref HEAD 2>/dev/null || echo ""; }

default_branch() {
    git config --get kdevkit.defaultBranch 2>/dev/null && return 0
    for c in main master; do
        git show-ref --verify --quiet "refs/heads/$c" && { echo "$c"; return 0; }
    done
    echo main
}

default_branch() {
    git config --get kdevkit.defaultBranch 2>/dev/null && return 0
    for c in main master; do
        git show-ref --verify --quiet "refs/heads/$c" && { echo "$c"; return 0; }
    done
    echo main
}

# The spec naming this branch. Searched in the working tree, so the very
# first commit of a feature (which creates the spec) still counts.
#
# Specs write the branch line in several shapes, and a literal match on one
# of them made the whole mechanism silently inert on every real spec in this
# project. Match the branch NAME with the surrounding punctuation optional:
# `Branch: x`, `- Branch: \`x\``, `**Branch:** x` all count. Anchored on the
# name so a branch mentioned in prose elsewhere does not qualify a spec.
spec_file() {
    b=$(branch)
    [ -n "$b" ] || return 1
    [ -d specs/feature ] || return 1
    grep -rlE "^[-*[:space:]]*(\*\*)?Branch:?(\*\*)?:?[[:space:]]*.?${b}.?[[:space:]]*$" \
        specs/feature 2>/dev/null | head -1
}

# Is this branch under kdevkit's feature loop? Anything else — the
# default branch, an unrelated branch, initiative-level work — must
# leave every commit untouched.
applies() {
    b=$(branch)
    [ -n "$b" ] || return 1
    [ "$b" != "$(default_branch)" ] || return 1
    [ -n "$(spec_file 2>/dev/null)" ] || [ -n "$(stage_recorded)" ]
}

# Per-project settings live in specs/project.md under `### kdevkit`, the
# same place kdevkit already declares things like the code reviewer —
# not in git config, which is per-clone and invisible to the repo.
#
#   ### kdevkit
#   - `gates:`
#     - `dev:`
#       - `quality: just lint`
#       - `tests: just test`
#
# Read as: the value on the first `key: value` line under the named
# stage's block. Absent configuration means nothing to verify.
gate_command() {
    stage=$1; key=$2
    f=specs/project.md
    [ -f "$f" ] || return 0
    awk -v stage="$stage:" -v key="$key:" '
        /^#/            { in_kdevkit = ($0 ~ /kdevkit/) }
        !in_kdevkit     { next }
        $0 ~ stage      { in_stage = 1; next }
        in_stage && /`[a-z_]+:`[[:space:]]*$/ { in_stage = 0 }
        in_stage {
            line = $0
            gsub(/`/, "", line)
            sub(/^[[:space:]]*-[[:space:]]*/, "", line)
            if (index(line, key) == 1) {
                sub(/^[^:]*:[[:space:]]*/, "", line)
                print line
                exit
            }
        }
    ' "$f"
}

stage_recorded() {
    git log --format="%(trailers:key=$TRAILER_STAGE,valueonly=true,unfold=true)" 2>/dev/null \
        | tr -d '\r' | grep . | head -1 | sed 's/[[:space:]]*$//'
}

return_count() {
    git log --format="%(trailers:key=$TRAILER_RETURN,valueonly=true,unfold=true)" 2>/dev/null \
        | grep -c . || true
}

# A work commit is a conventional type that means implementation, as
# distinct from the plan() commit that opens a feature.
has_work_commit() {
    rng=$(range)
    [ -n "$rng" ] || return 1
    git log --no-decorate --format='%s' "$rng" 2>/dev/null \
        | grep -qE '^(feat|fix|refactor|perf|test)(\(|!|:)'
}

has_plan_commit() {
    rng=$(range)
    [ -n "$rng" ] || return 1
    git log --no-decorate --format='%s' "$rng" 2>/dev/null | grep -qE '^(plan|docs)(\(|!|:)'
}

# The commit carrying the newest `Return-To`, if any. A return resets what
# counts as evidence: work done before going back was work on the old
# understanding, and must not imply the feature is still where it was.
last_return_commit() {
    git log --format="%H%x09%(trailers:key=$TRAILER_RETURN,valueonly=true,unfold=true)" 2>/dev/null \
        | awk -F'\t' 'NF > 1 && $2 != "" { print $1; exit }'
}

# Commits whose evidence counts. Bounded below by the default branch, so a
# long-lived main does not make every feature look like it has work in it,
# and by the newest return, so going back genuinely rewinds the evidence.
range() {
    since=$(last_return_commit)
    if [ -n "$since" ]; then
        echo "$since..HEAD"
        return
    fi
    d=$(default_branch)
    if git show-ref --verify --quiet "refs/heads/$d" && [ "$(branch)" != "$d" ]; then
        echo "$d..HEAD"
    else
        echo "HEAD"
    fi
}

plan_items_open() {
    f=$(spec_file 2>/dev/null) || { echo 0; return; }
    [ -n "$f" ] || { echo 0; return; }
    grep -cE '^[[:space:]]*-[[:space:]]*\[[[:space:]]\]' "$f" 2>/dev/null || true
}

plan_items_done() {
    f=$(spec_file 2>/dev/null) || { echo 0; return; }
    [ -n "$f" ] || { echo 0; return; }
    grep -cE '^[[:space:]]*-[[:space:]]*\[[xX]\]' "$f" 2>/dev/null || true
}

handoff_blocks() {
    f=$(spec_file 2>/dev/null) || { echo 0; return; }
    [ -n "$f" ] || { echo 0; return; }
    grep -c '^## Handoff' "$f" 2>/dev/null || true
}

remote_branch_exists() {
    b=$(branch)
    [ -n "$b" ] || return 1
    [ -n "$(git ls-remote --heads origin "$b" 2>/dev/null)" ]
}

tree_hash() { git rev-parse "HEAD^{tree}" 2>/dev/null || echo ""; }

# Evidence recorded against a tree other than the current one is stale:
# something was edited after it was gathered.
#
# Kept in the worktree's own git directory, NOT in git config: config is
# shared across worktrees, so two features in flight would overwrite each
# other's record. Never committed either — it describes a working tree.
verified_file() { git rev-parse --git-path kdevkit-verified; }

verified_tree() {
    f=$(verified_file)
    [ -f "$f" ] && cat "$f" || true
}

verified_tree() {
    f=$(verified_file)
    [ -f "$f" ] && cat "$f" || true
}

checks_verified() {
    v=$(verified_tree)
    [ -n "$v" ] && [ "$v" = "$(tree_hash)" ]
}

# A return is open while the commit that recorded it is still the newest
# one: nothing has been done about it yet. The fix commit discharges it.
open_return() {
    git log -1 --format="%(trailers:key=$TRAILER_RETURN,valueonly=true,unfold=true)" 2>/dev/null \
        | tr -d '\r' | grep . | head -1 | sed 's/[[:space:]]*$//'
}

yn() { if "$@" >/dev/null 2>&1; then echo yes; else echo no; fi; }

cmd_facts() {
    in_repo
    echo "branch=$(branch)"
    echo "default_branch=$(default_branch)"
    echo "applies=$(yn applies)"
    echo "spec_file=$(spec_file 2>/dev/null || true)"
    echo "stage_recorded=$(stage_recorded)"
    echo "return_count=$(return_count)"
    echo "open_return=$(open_return)"
    echo "has_work_commit=$(yn has_work_commit)"
    echo "has_plan_commit=$(yn has_plan_commit)"
    echo "plan_items_open=$(plan_items_open)"
    echo "plan_items_done=$(plan_items_done)"
    echo "handoff_blocks=$(handoff_blocks)"
    echo "remote_branch=$(yn remote_branch_exists)"
    echo "tree=$(tree_hash)"
    echo "verified_tree=$(verified_tree)"
    echo "checks_verified=$(yn checks_verified)"
}

# How many commits have been made in the current stage. Repeated failure at
# one stage is evidence about a different one.
attempts_in_stage() {
    st=$(stage_recorded)
    [ -n "$st" ] || { echo 0; return; }
    git log --format="%(trailers:key=$TRAILER_STAGE,valueonly=true,unfold=true)" 2>/dev/null \
        | awk -v s="$st" 'BEGIN{n=0} {gsub(/[[:space:]]/,"")} $0==s{n++; next} $0!=""{exit} END{print n}'
}

# ── the closed table of moves ────────────────────────────────────
# Forward edges only. Going back is handled by `return`, which is
# always permitted and always recorded.
legal_forward() {
    from=$1; to=$2
    case "${from:-unrecorded}:$to" in
        unrecorded:research|unrecorded:planning) return 0 ;;
        research:planning) return 0 ;;
        planning:dev) return 0 ;;
        dev:review) return 0 ;;
        review:closure) return 0 ;;
        closure:closed) return 0 ;;
        *) return 1 ;;
    esac
}

# The onward stage, from the table rather than from any phase module's
# prose. A module states what it must achieve and asks where that leads,
# so adding a stage never means editing the module before it.
next_stage() {
    from=${1:-}
    case "${from:-unrecorded}" in
        unrecorded) echo planning ;;
        research)   echo planning ;;
        planning)   echo dev ;;
        dev)        echo review ;;
        review)     echo closure ;;
        closure)    echo closed ;;
        *)          return 3 ;;
    esac
}

valid_stage() {
    for s in $STAGES; do [ "$1" = "$s" ] && return 0; done
    return 1
}

# Conditions beyond edge legality. Returns 0 permit, 1 refuse, 3 cannot
# determine. Prints the reason on refusal.
preconditions() {
    to=$1
    if [ -z "$(spec_file 2>/dev/null)" ]; then
        echo "cannot determine: no spec in specs/feature names branch $(branch)"
        return 3
    fi
    hb=$(handoff_blocks)
    if [ "$hb" != "1" ]; then
        echo "refused: the spec must have exactly one '## Handoff' section, found $hb"
        echo "  to resolve: leave one block and delete the others"
        return 1
    fi
    r=$(open_return)
    if [ -n "$r" ]; then
        echo "refused: a return to '$r' is the newest thing on this branch"
        echo "  to resolve: do the work it asks for and commit it; that discharges the return"
        return 1
    fi
    case "$to" in
        dev)
            if ! has_plan_commit; then
                echo "refused: the spec is not committed yet"
                echo "  to resolve: commit the spec as plan(<feature>): ..."
                return 1
            fi
            ;;
        review)
            if ! has_work_commit; then
                echo "refused: no implementation commit on this branch"
                echo "  to resolve: commit the work, or record an exception if there is genuinely none"
                return 1
            fi
            open=$(plan_items_open)
            if [ "$open" != "0" ]; then
                echo "refused: $open implementation plan item(s) still unticked"
                echo "  to resolve: tick them, or run 'except --skipping \"plan items\" --why ...'"
                return 1
            fi
            if ! checks_verified; then
                echo "refused: the gates have not been observed passing on this tree"
                echo "  to resolve: run 'feature-loop verify', or record an exception"
                return 1
            fi
            ;;
        closure)
            if ! remote_branch_exists; then
                echo "refused: branch $(branch) is not on the remote, so it cannot have been reviewed"
                echo "  to resolve: push it, or record an exception if review happened elsewhere"
                return 1
            fi
            ;;
    esac
    return 0
}

# The stage the repository itself implies, regardless of what was last
# recorded or asked for. This is what stops a forgotten `advance` from
# leaving the record behind reality: an agent that commits implementation
# work is in dev whether or not it said so.
#
# Only stages with an observable signature are derived: planning from a
# plan commit, dev from an implementation commit, closure from a close
# commit. Review has no signature — nothing a commit contains distinguishes
# "a human has reviewed this" — so review is never inferred and must be
# recorded deliberately.
# `$1` is the subject of the commit being prepared, which is not in
# history yet — the stamp runs before the commit exists, so without this
# a commit's own evidence could not count toward its own stage.
implied_stage() {
    pending=${1:-}
    # kdevkit's own convention gives closure an observable signature: a
    # `close(...)` commit is what closing a feature produces. Found by an
    # agent that did the closure work correctly without calling `advance`,
    # leaving the record behind reality — the same class of bug the
    # derivation exists to prevent, one stage further along.
    if printf '%s' "$pending" | grep -qE '^close(\(|!|:)' || has_close_commit; then
        echo closure
    elif printf '%s' "$pending" | grep -qE '^(feat|fix|refactor|perf|test)(\(|!|:)' || has_work_commit; then
        echo dev
    elif printf '%s' "$pending" | grep -qE '^(plan|docs)(\(|!|:)' || has_plan_commit; then
        echo planning
    else
        echo ""
    fi
}

has_close_commit() {
    rng=$(range)
    [ -n "$rng" ] || return 1
    git log --no-decorate --format='%s' "$rng" 2>/dev/null | grep -qE '^close(\(|!|:)'
}

# Stage ordering, for taking the later of two. A record may move forward
# on its own evidence; it must never slide backwards without a `return`.
stage_rank() {
    case "${1:-}" in
        research) echo 1 ;;
        planning) echo 2 ;;
        dev)      echo 3 ;;
        review)   echo 4 ;;
        closure)  echo 5 ;;
        closed)   echo 6 ;;
        *)        echo 0 ;;
    esac
}

later_stage() {
    a=$1; b=$2
    [ "$(stage_rank "$a")" -ge "$(stage_rank "$b")" ] && { echo "$a"; return; }
    echo "$b"
}

exception_count() {
    git log --format="%(trailers:key=$TRAILER_EXCEPTION,valueonly=true,unfold=true)" 2>/dev/null \
        | grep -c . || true
}

