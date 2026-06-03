# Backlog: kdevkit-compaction

## What

Restructure the kdevkit skill so that **only operational
content** is loaded into every session, while one-time setup
content and template prose live in deferred files that load on
demand or via fresh-context subagent.

Concretely, split the current single-file skill:

```
sources/skills/kdevkit/
├── SKILL.md         always-on, operational only
│                    §1 (locate), §3 (entry cues),
│                    §4 (operational decisions),
│                    §5, §7, §8, §9
├── setup.md         deferred. project.md template, six-section
│                    schema, first-time detection prose,
│                    code_review block schema + sticky-write
│                    rules, the long-form code-review setup prompt.
└── interviews.md    deferred. four interviews, feature file
                     template, backlog template, initiative
                     template (post-B).
```

Two load primitives the always-on SKILL.md uses:

- **Verify-as-subagent.** SKILL.md carries a tiny structural
  check at session start ("does `project.md` exist? does it have
  the six headings? is `## Agent Development > kdevkit >
  code_review:` present?"). Cheap, narrative-free.
  - Clean → nothing happens. Steady-state path; ~zero overhead.
  - Dirty → dispatch a fresh-context `kdevkit-verify` subagent.
    The subagent loads `setup.md` + `project.md`, validates
    against the canonical schema, returns
    `{ status, missing, suggested_edits: [{file, anchor, content}] }`.
    Main applies edits via Edit. The setup narrative never
    enters main's context.

  This reuses the same fresh-context primitive that §7 Code
  Review Gate already invokes — same generic phrasing, same
  host-portability story (Claude Code Agent tool, Kiro
  equivalent, host-specific translation).

- **Inline-Read on demand.** When main needs to **write** —
  fresh `project.md`, four interviews, new backlog/feature
  template — main inline-Reads `setup.md` or `interviews.md`
  at the moment of need, executes, exits. Interactive flows
  don't round-trip through a subagent; that overhead isn't
  worth it for one-shot creates.

**Fallback.** Host without subagent dispatch → main falls back
to inline-Read of `setup.md` for the verify check too.
Behavior degrades to today's footprint; no breakage.

## Why

The skill is 693 lines (post-§7 Code Review Gate, post-three-
phase, post-grouped-Reading-order). Most of it is *setup-and-
create* narrative — project.md template, code_review schema,
four interviews, file templates — which is dead weight on every
steady-state session. When project.md is already populated and
the agent is mid-feature, none of that prose fires; it's pure
context overhead.

But the prose can't be *removed* either: to know "is the setup
correct?", you need the schema. That's the deferral problem.

Three real costs of the current shape:

1. **Token cost per session.** Every Claude Code / Kiro session
   that loads kdevkit pays for the full SKILL.md, regardless of
   whether the project is fresh or steady-state. mAId's own
   project.md is fully populated; loading the project.md
   template every time we open this repo is waste.
2. **Context-budget pressure on the main agent.** As kdevkit
   grows (initiative tier, comment-prefix convention, future
   features), the always-on cost compounds. Compaction now
   prevents the skill from quietly becoming a context hog later.
3. **Verify needs the schema, but main doesn't.** The current
   shape conflates "agent doing feature work" with "agent
   verifying the setup is correct" — both contexts inherit the
   full template prose. A fresh-context subagent can verify
   without contaminating main.

The realization that drove this: the dev-loop content (§5/§7/§8/§9)
fires every session and must stay always-on. The setup content
(§2 templates, §6 interviews, §3 backlog template) fires only on
project genesis or feature genesis — rare events. Splitting on
that frequency line is the right cut.

## How it composes with existing kdevkit sections

The split is reorganizational, not behavior-changing. The
user-visible side of every gate stays identical.

- **§1 Locate the spec tree** — stays in SKILL.md.
  Always-on, fires every session.
- **§2 Load project context** — *splits*. The trigger ("read
  project.md if it exists; ask the question if not") stays in
  SKILL.md. The template, six-section schema, first-time
  detection logic, and `code_review` block schema move to
  `setup.md`. SKILL.md gains a verify-handoff: "if project.md
  is present but malformed, dispatch the verify subagent."
- **§3 Load feature context** — entry cues stay in SKILL.md.
  The backlog and feature templates move to `interviews.md`.
  SKILL.md says "when writing a fresh feature spec, inline-Read
  `interviews.md`."
- **§4 Start feature session** — operational decisions
  (worktree, planning-phase opt-out) stay. The long-form
  code-review setup prompt moves to `setup.md`; SKILL.md keeps
  a one-line trigger ("if `code_review:` is missing from
  project.md, inline-Read `setup.md` and run the setup prompt").
- **§5 / §7 / §8 / §9** — stay in SKILL.md verbatim. These are
  every-session operational content.
- **§6 Feature planning** — *splits*. The Plan-commit rule and
  Planning Review Gate stay (operational; fires on every plan).
  The four interviews and the feature file template move to
  `interviews.md`. SKILL.md says "when entering planning for a
  fresh feature, inline-Read `interviews.md`."

The split rule of thumb: **fires every session → SKILL.md;
fires on project / feature genesis → deferred file.**

## Where it lands relative to A and B

This compaction runs **after** A (`kdevkit-agent-comment-prefix`)
and B (`kdevkit-initiative-tier`) ship. Reasoning:

- A adds ~30–60 lines of operational content (the prefix rule
  + two example commands). Lands in SKILL.md (operational).
- B adds an initiative template (deferred — moves to
  `interviews.md` or its own `templates/initiative.md` shard)
  plus operational changes to §1, §3, §6, §8 (stays in
  SKILL.md). B is the larger surface; doing it before D means
  D compacts the *post-B* shape and we don't have to redo the
  cut later.
- Doing D before A or B would mean two compaction passes (one
  now, one after each later feature drops new template prose
  back into the always-on file). Net waste.

## How to ship it (likely shape)

1. **Inventory the cut.** Walk SKILL.md section-by-section.
   For each prose block, classify: operational (every session)
   vs. setup (project / feature genesis). Output: a per-block
   classification table in the feature spec.
2. **Carve the deferred files.** Move the setup-class prose
   into `setup.md` and `interviews.md`, preserving wording.
   Don't paraphrase — the SKILL.md prose is already
   load-bearing for current behavior; rewording risks drift.
3. **Rewrite SKILL.md trigger points.** §2, §3, §4, §6 each
   gain a one-line handoff to the deferred file at the place
   the moved content used to live. Phrasing is generic
   ("inline-Read `setup.md`"); host-specific implementation
   notes go in a §10-style reference section if needed.
4. **Add the verify primitive.** New short subsection in §2
   describing the structural check + subagent dispatch.
   Generic phrasing — same shape as §7's existing
   fresh-context dispatch.
5. **Three new functional smoke fixtures.** Judge-mode (§9
   testing layer):
   - `kdevkit-steady-state.smoke` — populated project.md,
     mid-feature → judge: "does the agent reference setup
     narrative? (should not)"
   - `kdevkit-setup-drift.smoke` — project.md missing
     `code_review:` → judge: "does the agent dispatch verify
     and apply the suggested edit?"
   - `kdevkit-fresh-feature.smoke` — no spec → judge:
     "does the agent run the four interviews?"
6. **Existing fixtures** (`kdevkit.smoke`, `-feature-loop`,
   `-feature-planning`, `-feature-closure`, `-dev-loop`,
   `-review-gate`, `-review-config-setup`) must continue to
   pass — they're the regression net.
7. **Skill version bump.** This is a structural change to how
   the skill loads, but user-visible behavior is unchanged.
   Probably v3.X (post-B's v3.0) — minor, since no behavior
   contracts shift.

Estimated diff size: SKILL.md drops ~150–200 lines (target
~500 line always-on file from a post-B starting point near
~700–750), two new files of ~100–150 lines each, three new
smoke fixtures.

## Open questions

1. **Granularity of `setup.md` vs. `interviews.md`.** Is the
   two-file split right, or do we want a single
   `deferred.md`? Lean two-file: project.md setup and feature
   interviews fire at different events, and a fresh-feature
   session shouldn't pay the project.md template tax. Confirm
   by counting the reads: a session that hits both is rare
   (only on a brand-new project's first feature).

2. **Verify subagent's structured return shape.** What's the
   contract — `{ status, missing, suggested_edits }` is the
   sketch, but should `suggested_edits` be diff hunks the main
   applies via Edit, or full-file replacements? Diff hunks are
   smaller but more brittle if the file shifted between
   subagent dispatch and main's apply. Lean diff hunks with a
   "regenerate if sha mismatches" fallback.

3. **Where do cross-cutting templates live?** Initiative
   template (post-B), feature template, backlog template — all
   templates. Could shard into `templates/` subdirectory or
   group into one `templates.md`. Lean group-into-one until a
   template grows to standalone size.

4. **Host fallback testing.** The fallback path
   ("subagent dispatch unavailable → inline-Read `setup.md`")
   is a behavior we'd ideally exercise in CI. mAId's
   functional tests run against real `claude` and `kiro-cli`
   binaries, which both support subagent dispatch — so the
   fallback path is essentially untested. Punt to the feature
   spec; document the limitation explicitly.

5. **Re-promotion of features that touch SKILL.md.** A future
   feature that adds operational content to SKILL.md will
   ride on the post-D shape (lean SKILL.md). A future feature
   that adds setup or template content has to land it in the
   right deferred file. The skill text needs to make this
   distinction explicit so a future agent doesn't dump new
   templates into SKILL.md by reflex.

## Trigger to promote

- A and B have shipped (squash-merged to `main`). D is the
  natural next feature in the kdevkit sequence.
- Any session that loads kdevkit hits a noticeable
  context-budget ceiling — currently theoretical, but a real
  signal once it happens.
- A second always-on skill in mAId starts to face the same
  problem (e.g. notes or writing-style grows past 500 lines)
  and we want to validate the split-load pattern on kdevkit
  first before generalizing.
