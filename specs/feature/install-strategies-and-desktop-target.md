# Feature: install-strategies-and-desktop-target

## Git Setup

- Branch: `feat/install-strategies-and-desktop-target`
- Base: `main`

## Feature Brief

mAId installs skills by symlinking `$HOME` paths back into the checkout.
That is one *strategy*, not the definition of install — and it has two
costs. A fresh clone installed on a machine stays coupled to that
checkout forever (move or delete it and the install breaks), and one
target refuses symlinks outright, so it cannot be reached at all: the
desktop agent app's plugin directory rejects a symlinked entry by design.

This feature separates **what** is installed from **how** it reaches a
destination. The registry gains a strategy per row — symlink for the
three coding agents (live edits stay valuable while authoring skills) and
copy for the desktop app — and the desktop app becomes a fourth
first-class target of the existing `<action>-<resource-kind>` verbs and
agent selector.

After it lands: `just resources::install-skills desktop` packages the
Cowork-appropriate skills as a plugin and copies it where the app reads
it; the other three targets behave exactly as before; and "install"
means a fresh clone can install and be deleted, for the target where
that is the honest contract.

Part of initiative: [[claude-desktop-cowork]] *(corporate spec tree; the
initiative itself is not tracked in this public repo)*

## Requirements

### The verb surface

The existing pattern extends by one selector value — no new verbs:

```
just resources::install-skills   [claude|kiro|codex|desktop]
just resources::uninstall-skills [claude|kiro|codex|desktop]
just resources::status-skills    [claude|kiro|codex|desktop]
```

Omitting the selector still means "every target", so a bare
`install-skills` now also reaches the desktop app.

### What the user observes

- `install-skills desktop` reports which skills were packaged and where
  they landed. A second run reports no change.
- `status-skills` shows, per target, whether the destination matches the
  checkout — and for the copied target, whether the copy is **stale**
  (source edited since the copy) rather than merely present.
- `uninstall-skills desktop` removes what was installed and nothing
  else. Hand-added entries alongside ours are preserved.
- Installing to the desktop app needs elevated permission, and the verb
  says so up front rather than failing partway.
- A target whose app is not installed is skipped with a reason, not an
  error — matching how the browser installable graceful-skips.
- Editing a skill after installing shows the change immediately on the
  three symlinked targets, and requires a re-run for the copied one. The
  status verb is what tells the user a re-run is due.

### Skills reaching the desktop app — document-shaped only

**The desktop target carries document-oriented skills only. Terminal-shaped
skills are deliberately left out.** This is a scoping rule, not a
temporary gap: the desktop app's surface is document work (drafting,
research, note-keeping), while skills built around a repo, a shell, or a
terminal session have nothing to act on there.

Today that means the note-taking and writing-style skills are in;
`kdevkit` (spec-driven development over a checkout) and `browser`
(drives a real browser through a terminal-registered MCP) are out.

The user-visible consequence: `install-skills desktop` installs a
*subset*, and says which skills it skipped and why, so the omission reads
as intent rather than as a bug. Adding a new skill does not
automatically reach the desktop target — it must be declared
document-shaped.

Because this rule outlives the feature, it is recorded in `project.md`
(see Implementation Plan), not only here.

## Test Strategy

Per `project.md`'s Testing section. The load-bearing layer is the Rust
unit tests with a tempfile fake `$HOME` — they already cover the symlink
state machine, and the copy strategy needs the same treatment.

### Unit (Rust, fake `$HOME` — load-bearing)

- Copy strategy: fresh destination gets the packaged plugin; contents
  match source.
- Copy is idempotent: second install reports no change, mtimes stable.
- Stale detection: touch a source file, status reports stale.
- Re-install over stale replaces content and clears stale.
- Uninstall removes only managed entries; a sibling hand-added directory
  survives.
- A real (non-symlink) file at a symlink destination is still preserved
  without `--force` — existing guarantee, must not regress.
- Selector filtering: `desktop` touches only desktop rows; `claude` only
  claude rows; no selector touches all.
- Packaging shape: the emitted plugin has a valid manifest and each
  selected skill present; a skill absent from source is an error, not a
  silent skip.

### Functional (the existing `resources/tests/run` harness)

- After a real `install-skills desktop`, the app loads the skills. This
  is attended and gated like `verify-browser-mcp` — it needs the app
  installed and elevated permission, so it is not in the default gate.

### Not tested

Whether the app *uses* a skill well — that is the skill's own fixture,
already covered per-skill for the three coding agents.

## Design

### Why this shape

The registry already varies *how* a destination is populated —
`Kind::Link` writes one symlink, `Kind::FanOut` writes one per child,
because codex owns its skills directory and ships its own entries there.
So "destinations differ in mechanism" is not a new idea; the enum has
carried it since the codex row landed. What is new is a mechanism that
is not a symlink at all.

That makes **strategy the third dimension of the registry**, alongside
destination and agent — the same move `resource-symmetry` made when it
added the agent tag rather than writing a per-agent function. The
alternative, a separate desktop-only code path beside the registry loop,
is exactly the hand-written divergence that let codex be silently
omitted from the MCP installer before; the registry is the single source
of truth precisely so a new target is a row, not a branch.

**Why the three coding agents keep symlinking.** Live edits are the
point while authoring a skill: change the markdown, next session sees it.
Converting them to copy would trade that for a uniformity nobody
benefits from, and the fresh-clone argument does not apply — those
targets read from `$HOME` paths mAId owns entirely. Copy is adopted
where it is *required*, not as a blanket policy.

**Why a plugin rather than loose skills.** The desktop app does not read
the coding-agent skills directory at all; its extension surface is a
plugin — a directory with a manifest that bundles skills (and, later,
agents, commands, hooks, MCP servers). A single-skill plugin may put
`SKILL.md` at its root, but the `skills/<name>/SKILL.md` layout is
correct for a plugin expected to carry more than one, which this is.

**Why copy and not a marketplace.** A marketplace is the vendor's
recommended fleet mechanism and would avoid elevated permission, but it
needs the plugin served from a reachable git or HTTPS origin and pins
auto-install to a commit SHA. For a local checkout on one machine that
is more moving parts than the job needs, and it makes the install
depend on network reachability. Copy from the checkout keeps the
install self-contained. A marketplace stays the clean upgrade path if
this is ever distributed to more than one machine — noted, not built.

**Stale is a first-class state, not a failure.** A symlinked
destination cannot go stale; a copy can, and silently. So the
comparison the planner already performs (`Match`, `Missing`,
`WrongTarget`, `BlockedByRealFile`, …) gains a stale variant for copied
rows. This is the honest cost of copy-install, and surfacing it in
`status` is what keeps it from being a footgun.

### Components

1. **Registry gains a strategy column.** Rows become
   `(home_subpath, source_subpath, strategy, agent)` where strategy is
   `Link | FanOut | Copy`. The desktop row names the app's plugin
   directory as its destination and the packaged plugin as its source.

2. **A packaging step for the copied row.** The desktop row's source is
   not `resources/content/skills` directly — it is a plugin assembled
   from a declared subset of it (manifest + `skills/<name>/` per
   selected skill). Assembly happens under the repo's existing build
   output, so the checkout is not mutated and the copy has a single
   source of truth.

3. **The planner learns copy comparison.** `plan_one` currently reads
   `symlink_metadata` and compares link targets. For copy rows it
   compares directory contents instead, yielding `Match`, `Missing`, or
   the new stale state. Install/uninstall/status all drive off the plan,
   so each verb gains copy support from this one change.

4. **Elevated-permission handling.** The app's plugin directory is
   system-wide. The verb checks writability first and reports what it
   needs before doing any work, so a partial install is not possible.
   This is the amended-constraint surface below.

5. **`project.md` amendments** — two hard constraints, scoped rather
   than deleted (see below).

### Constraint amendments

Two current hard constraints were written when symlink was the only
mechanism. Both are amended to say what they actually protect:

- *"Never write into registry destinations"* → the intent is **source is
  truth; never hand-edit an installed artefact.** That holds for both
  strategies. The literal reading (destinations must be symlinks) is
  replaced by: a destination is managed by its row's strategy, and
  copied destinations are overwritten by install, never edited in place.
- *"No global state mutation on install"* → retained for the three
  coding-agent targets and for the toolchain (still no `cargo install`,
  still no `~/.local/bin` shim). The desktop target is declared an
  explicit exception: its plugin directory is system-wide by the app's
  design, the verb is opt-in, and it announces the permission it needs.

Amending rather than silently contradicting is the point; the diff
should show a reviewer exactly which guarantee changed and why.

## Implementation Plan

- [ ] Add the strategy column to the registry with `Copy`; keep every
      existing row's behaviour byte-identical (pure refactor, tests green).
- [ ] Teach the planner copy comparison, including the stale state, with
      unit tests over a fake `$HOME`.
- [ ] Implement copy install/uninstall driven off the plan; preserve the
      real-file guarantee and managed-only uninstall.
- [ ] Add the plugin packaging step and the declared skill subset, with
      the skipped skills reported by name.
- [ ] Add the desktop registry row, the selector value, and the
      writability precheck.
- [ ] Amend the two `project.md` hard constraints.
- [ ] Record the **document-shaped-only** rule in `project.md` as a
      standing constraint: which skills may reach the desktop target and
      why terminal-shaped ones are excluded. This outlives the feature —
      a future skill must be declared document-shaped to be included, so
      the rule cannot live only in this spec. (mAId has no repo-root
      `AGENTS.md` and deploys no global instruction file by design, so
      `project.md` is the home; it is project-knowledge, not operational
      instruction.)
- [ ] Update `README.md` verb docs and `project.md` Architecture /
      Deployment for the second strategy.
- [ ] Attended functional check: install for real, confirm the app loads
      the skills.

- *Risk note:* the packaging step is new build output; keep it inside the
  existing build directory so `status`/`uninstall` have one source of
  truth and nothing lands in the content tree.
- *Risk note:* the elevated-permission path is the one step that cannot
  be exercised in the unit tests' fake `$HOME`. Keep it a thin,
  well-isolated precheck so the untested surface is minimal.
- *Risk note:* stale detection by content comparison is portable;
  mtime comparison is not (checkout order, clock skew). Prefer content.

## Session Log

- 2026-08-25 · Spec drafted. Grounded in `resource-symmetry` (the prior
  feature that made skills and the browser installable symmetric behind
  one selector) and in the desktop app's own bundle: its 3P code path
  logs `Refusing symlink at <path>` for a symlinked plugin entry, and
  hardcodes the system plugin directory per platform. That refusal is
  what rules out extending the symlink registry to this target.

## Decision Log

- **Strategy per registry row, not a parallel desktop code path.**
  The registry is already the single source of truth for deployment and
  already varies mechanism (`Link` vs `FanOut`). A separate path would
  repeat the hand-written divergence that previously let a target be
  silently omitted. Considered a desktop-specific installer script
  (mirroring the browser installable's shell layer) and rejected: the
  browser MCP is shell because *registration is a runnable command a
  symlink cannot express*; copying files is exactly what the build-tool
  already does.
- **Symlink retained for claude/kiro/codex.** Live editing while
  authoring skills is worth more than mechanism uniformity, and the
  fresh-clone problem does not apply to destinations mAId fully owns.
- **Copy over marketplace for now.** Marketplace needs a served origin
  and a SHA pin; copy from the checkout is self-contained and offline.
  Marketplace recorded as the upgrade path if this ships to more than
  one machine.
- **Stale as an explicit plan state.** The real cost of copy-install is
  silent drift. Surfacing it in `status` converts a footgun into a
  reported condition.
- **Document-shaped skills only on the desktop target; terminal-shaped
  ones excluded.** Confirmed as the scope for this feature. The split is
  by what the surface can act on, not by preference: the desktop app is
  a document workspace, so a skill built around a checkout or a shell
  session has nothing to operate on there. Considered installing
  everything and letting the app ignore what does not apply, and
  rejected — a skill that cannot work is worse than an absent one,
  because it advertises a capability the surface cannot honour. Recorded
  in `project.md` rather than only here, since a future skill has to be
  declared document-shaped to be included.
