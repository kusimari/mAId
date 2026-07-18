---
name: pr-review-tui-across-hosts
description: A first-class, ergonomic way to review code changes from the terminal across both GitHub PRs and enterprise/self-hosted code-review platforms — generalizing the throwaway gh-review-tmux wrapper experiment into a supported mAId capability.
metadata:
  type: backlog
---

# Terminal code-review across GitHub and enterprise review hosts

<!-- Public-repo note: this repo forbids internal product/team/repo/tool
     names (project.md Hard constraints). The specific enterprise
     code-review platform the author uses is referred to here only
     generically ("enterprise/self-hosted review host"). File the
     host-specific integration details in a corporate spec tree, not
     here. -->

## What

A supported way to review code changes from the terminal — the diff,
the file tree, inline comments, approve/request-changes — that works
across **both**:

- **GitHub pull requests** (public + private repos), and
- **enterprise / self-hosted code-review platforms** (the review
  surface that isn't GitHub — reached via its own CLI/API).

Today mAId has nothing for this. Reviewing PR #30 was done ad-hoc.
The goal is one consistent review workflow regardless of which host a
given change lives on, ideally surfaced the way other capabilities are
(a skill and/or a `resources::*` verb), rather than a personal
throwaway script.

## Why

- Review happens on two hosts with two totally different tools; the
  context-switch and the missing ergonomics (scrolling, selection,
  inline-comment flow) make terminal review worse than the web UI, so
  it doesn't get used.
- mAId already drives coding agents across three tools uniformly
  (claude/kiro/codex) and installs skills to each; a matching
  "review anywhere" capability fits the same "one canonical workflow,
  many surfaces" mission.
- The kdevkit dev loop leans on a Code Review Gate; a good local review
  surface makes acting on that gate's findings faster.

## The gh-review experiment (captured before discarding)

An untracked `gh-review-tmux.d/` directory sat in the repo root (dated
2026-06-22) — a personal spike, never committed. What it did, worth
keeping as prior art:

- **`run`** — a bash wrapper that launched **`gh-review`** (a
  third-party terminal TUI for GitHub PR review) under an ephemeral
  `nix shell` (`cargo`, `rustc`, `gh`, `pkg-config`, `openssl`).
  It `cargo install`ed gh-review into a `mktemp -d` root each run and
  exec'd it — nothing permanently installed. Invocation:
  `./run <PR-number-or-url>`.
- **tmux mouse-wheel remap** — while gh-review ran, it sourced a tmux
  snippet rebinding `WheelUpPane`/`WheelDownPane` to send arrow keys
  (3 at a time), so mouse-wheel moved the selection inside the TUI;
  an `EXIT` trap unbound them and re-sourced `~/.tmux.conf` to restore
  normal mouse behavior.
- **`wheel-on.conf`** — the tmux `bind -T root Wheel{Up,Down}Pane …`
  snippet, guarded for copy-mode (`mouse_any_flag`) and
  alternate-screen apps (`alternate_on`).

What the experiment proved / its limits:
- Ephemeral-nix-install + exec is a clean way to run a Rust review TUI
  with no persistent footprint — reusable pattern.
- The wheel→arrows tmux dance is the fiddly part; a real capability
  should decide whether to own that or leave scrolling to the TUI.
- It was **GitHub-only** (built on `gh` + gh-review). It did nothing
  for the enterprise/self-hosted review host — which is the whole
  reason this needs to be a real, two-host feature rather than a
  one-host script.

## Open questions

- **One tool or two adapters?** Is there a review TUI that abstracts
  both hosts, or does this become a thin mAId layer with a
  GitHub adapter (`gh`/gh-review) and a separate enterprise-host
  adapter (that host's CLI/API)? The latter keeps host-specific
  (and internal) details out of this public repo.
- **Skill vs. verb vs. runnable resource.** Is "review this change" a
  skill (agent-driven, reads the diff and comments), a `resources::*`
  verb (launches a TUI), or — like the browser-control MCP — a
  runnable resource? The browser-MCP precedent (registration + Just
  verbs, not the registry) may be the closest shape if a server is
  involved.
- **Public-repo boundary.** The enterprise-host adapter will reference
  internal endpoints/tooling. Those must live in a corporate spec tree,
  not here; this feature spec stays generic and the integration is
  filed off-repo. Confirm that split before building.
- **Agent-in-the-loop?** Should the review capability just present the
  change, or also let a coding agent pre-review it (à la the kdevkit
  Code Review Gate) and surface findings inline?
