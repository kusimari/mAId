# Feature: kdevkit-compaction

## Git Setup

- Branch: `feat/kdevkit-compaction`
- Base: `main` at `4d0be91` (post-A + post-B merge)

## Feature Brief

Compact the kdevkit skill so each session loads only what it
needs into main context. Today the SKILL.md (post-A + post-B)
is ~964 lines; most of it is *create-and-setup* prose
(project.md template, six-section schema, four interviews,
feature/backlog/initiative templates, code-review setup prompt
long-form, §10 template + repo guidance) that fires only on
project genesis or feature genesis — rare events. The
operational content (§1 detect, §3 entry cues, §5 phase gates,
§7 dev loop, §8 closure, §9 cross-cutting) fires every session
and must stay always-on.

Two load primitives let main keep its context lean while
preserving correctness:

- **Verify-as-subagent.** A small structural check at session
  start tells main "setup is fine, carry on." On drift, main
  dispatches a fresh-context subagent that loads `setup.md`
  and `project.md`, validates against the canonical schema,
  returns a structured verdict. The setup narrative never
  enters main's context.
- **Inline-Read on demand.** When main needs to **write** —
  fresh `project.md`, four interviews, new template — it
  inline-Reads `setup.md` or `interviews.md` at the moment of
  need, executes, exits. Interactive flows don't round-trip
  through a subagent.

Functional tests assert **behavior**, not load order. Whether
main inline-Reads a deferred file or has the prose already in
context, the user-visible answer should be the same. The tests
also stress conversational resilience: kdevkit cues fire after
many turns of unrelated coding talk, the way they do in real
sessions.

## Requirements

### File split

- `sources/skills/kdevkit/SKILL.md` (always-on, target ~500
  lines): §1 (locate), §3 (entry cues), §4 (operational
  decisions), §5 (run-frame), §7 (dev loop), §8 (closure),
  §9 (cross-cutting). Keeps the §6 trigger ("when entering
  planning for a fresh feature, inline-Read `interviews.md`")
  but moves the four interviews + the feature-file template
  body out.
- `sources/skills/kdevkit/setup.md` (deferred): project.md
  six-section template, first-time detection prose, the
  full code-review setup prompt long-form (current §4 prose),
  `code_review:` schema and sticky-write rules, the optional
  `## Active initiatives` index format, the `## Agent
  Development > kdevkit` block schema. Loaded on session start
  by the verify subagent if drift is detected, and inline-Read
  by main when writing a fresh project.md.
- `sources/skills/kdevkit/interviews.md` (deferred): the four
  short interviews, feature file template body, backlog item
  template, initiative file template (currently in §10), and
  the §10 entry-verb walk-throughs ("start initiative" /
  "stream <n> for <initiative>" template-fill steps). Loaded
  inline by main when starting a fresh feature, a fresh
  backlog item, or a fresh initiative.

The split rule of thumb codified in SKILL.md for future
feature authors: **fires every session → SKILL.md; fires on
project / feature / initiative genesis → deferred file.**

### Verify-as-subagent primitive

- **Trigger.** Main runs a small structural check at session
  start — does `project.md` exist? does it have the six
  required headings (Mission, Architecture, Tech Stack,
  Layout, Testing, Deployment)? does it have an `## Agent
  Development > kdevkit > code_review:` block? does any
  `## Active initiatives` index match `initiative/`'s
  on-disk state?
- **Clean** → no further action. Steady-state path; near-zero
  context cost.
- **Drift** → main dispatches a fresh-context `kdevkit-verify`
  subagent. The subagent receives the user's session-start
  prompt, the path to `project.md`, the path to `setup.md`,
  and the on-disk listing of `$SPEC_ROOT/initiative/`. It
  loads `setup.md` and `project.md`, validates, returns a
  structured verdict:

  ```
  {
    "status": "clean" | "drift",
    "findings": [
      {
        "section": "<heading>",
        "issue": "<one-sentence>",
        "suggestion": "<one-sentence remedy>"
      }
    ]
  }
  ```

- **Apply.** Main reads the verdict. If `status: drift`, main
  surfaces findings to the user with one-line suggestions and
  applies accepted edits via Edit. The setup narrative never
  enters main's context — only the structured verdict.
- **Fallback.** Host without subagent dispatch → main
  inline-Reads `setup.md`, runs the same checks itself, and
  proceeds. Behavior degrades to today's footprint; no
  breakage. The §7 Code Review Gate's "fresh-context agent
  call" precedent (Claude Code's Agent tool, Kiro's
  equivalent) is the same primitive — kdevkit phrases verify
  generically and lets the host translate.

Free-form `findings` (not diff hunks) intentionally: the
subagent doesn't see the live file post-context, so the safest
contract is "describe the issue and the remedy"; main applies
the edit against the actual file. Diff hunks would be brittle
if the file shifted.

### Inline-Read on demand

- **Fresh project.md.** When project.md is missing/empty and
  feature work begins, main inline-Reads `setup.md`, executes
  the six-section template + first-time detection + code-review
  setup prompt, writes the file, exits.
- **Fresh feature.** When entering a feature with no spec on
  disk (start mode, neither `feature/` nor `backlog/` has the
  file), main inline-Reads `interviews.md`, runs the four
  short interviews, writes the feature spec, exits.
- **Fresh initiative.** When the user says "start initiative
  `<name>`", main inline-Reads `interviews.md`, runs the
  initiative interview shape (Why → Streams → initiative-level
  Decisions), writes `$SPEC_ROOT/initiative/<name>.md`,
  updates the project.md index, exits.
- **Backlog capture.** When the user describes a "we should
  eventually" item, main inline-Reads `interviews.md` for the
  backlog template if it doesn't already remember it; the
  template is short enough that it may already be inferred
  from the SKILL.md cross-reference.

### SKILL.md trigger points

The always-on SKILL.md gains explicit triggers where the moved
content used to live. Phrasing is generic
(`inline-Read sources/skills/kdevkit/setup.md`); the host
translates. Specifically:

- **§1**: keep "locate $SPEC_ROOT" prose; no deferred-load
  trigger here.
- **§2** (Load project context): replace the project.md
  template + first-time detection + code-review setup prompt
  with a one-line trigger: "if project.md is missing, empty,
  or fails the structural verify (next subsection),
  inline-Read `setup.md` and follow its template + detection
  prose." A new short subsection describes the verify-as-
  subagent primitive.
- **§3** (Load feature context): keep entry cues and the
  populated-spec rule. Replace the backlog template body with
  a trigger: "for the backlog item template, inline-Read
  `interviews.md` if needed."
- **§4** (Start feature session): keep operational decisions
  (worktree, planning-phase opt-out). Replace the long-form
  code-review setup prompt with a one-line trigger and a
  short summary: "if `kdevkit.code_review:` is missing,
  inline-Read `setup.md` and run the setup prompt; the
  resulting block sticky-writes to project.md."
- **§5**: keep the run-frame. The §6 four interviews trigger
  lives in §6.
- **§6** (Feature planning): keep the Plan-commit rule (always-
  on; fires every plan). Replace the four-interview body
  description and the feature-file template with a trigger:
  "for the four interviews and the feature-file template,
  inline-Read `interviews.md`." Keep the auto-link rule (§6
  always-on).
- **§7 / §8 / §9**: stay verbatim. Operational every session.
- **§10** (Initiative tier): keep the entry verbs (operational
  triggers fire every session) and the cross-stream rebase
  mechanics (operational when a parent stream re-ships).
  Replace the initiative file template + entry-verb template-
  fill steps with a trigger: "for the initiative template and
  entry-verb template-fill steps, inline-Read
  `interviews.md`." Keep the working-across-repo-shapes
  guidance (read once and absorbed; small enough to stay).

### Frontmatter / version

- SKILL.md `version` bumps from 3.0.0 to 3.1.0 (signpost — the
  compaction landed; user-visible behavior unchanged).
- Frontmatter `description` updated to mention the
  on-demand-load shape so a future agent inspecting the
  registry knows the skill is multi-file.

### Future-feature placement rule

A new short subsection in SKILL.md (anchored in §9 or just
before §10) codifies the rule for future contributors:
**fires every session → SKILL.md; fires on project / feature
/ initiative genesis → `setup.md` or `interviews.md`.** This
prevents future features from quietly dropping new templates
into SKILL.md by reflex.

## Test Strategy

Test strategy follows your direction. Three principles:

1. **Behavior-based, not shape-based.** Smokes assert what
   the agent *does* in response to a user ask. They do **not**
   inspect which file got loaded, what the agent's context
   contains, or what tool the agent dispatched. The skill
   does the right thing whether or not compaction landed.
2. **Independent of compaction internals.** A v3.0 (pre-D)
   kdevkit and a v3.1 (post-D) kdevkit both pass the same
   functional smokes — the only difference is the load
   pattern, which is invisible to the user. The smokes form
   the regression net for kdevkit's *contract*, not for
   kdevkit's *compaction*.
3. **Conversational-stream stress for every fixture.** Real
   sessions have long coding back-and-forths before a
   kdevkit cue fires; closure cues in particular often fire
   after dozens of turns. Every fixture (existing + new)
   runs in two modes — *bare* (current shape) and *stressed*
   (the same prompt with a synthetic conversational-stream
   prefix injected). The same `expected_narrative` evaluates
   both modes; passing both proves the contract survives
   compaction-window pressure.

### Functional smoke fixtures (judge mode)

The total fixture surface is 11: 7 existing + 4 new.

Existing fixtures (regression net):
`kdevkit.smoke`, `kdevkit-feature-loop.smoke`,
`kdevkit-feature-planning.smoke`, `kdevkit-feature-closure.smoke`,
`kdevkit-dev-loop.smoke`, `kdevkit-review-gate.smoke`,
`kdevkit-review-config-setup.smoke`.

Before re-running these, audit each for fragility against the
on-demand-load shape. The dev-loop / review-gate / closure
fixtures are operational and should pass unchanged. The
feature-planning fixture asks about the four interviews — must
still pass: the four interviews remain part of the kdevkit
contract; whether their text lives in SKILL.md or
`interviews.md` is invisible to the user. Patch any wording in
existing fixtures that asserts file location rather than
behavior.

**New fixtures (4).** Each is judge-mode
(`prompt: ` + `expected_narrative: `):

1. **`kdevkit-initiative-recognition.smoke`** — user describes
   coupled, multi-CR work in conversation ("we need to refactor
   the auth middleware *and* migrate the session store *and*
   update the API contract; these need to land in order"); the
   agent should (a) offer to create an initiative rather than
   one feature or three backlog items, (b) walk through the
   §10 entry-verb shape (`start initiative <name>`), (c) ask
   the initiative-level interview questions (Why → Streams →
   initiative-level Decisions), and (d) describe what the
   downstream `stream <n> for <initiative>` cue would do
   (auto-link, separate feature spec). One fixture covers
   recognition + entry verbs + auto-link description.
   Wrong answers: collapsing into one feature; splitting
   into three independent backlog items; jumping to
   spec-write without offering the initiative shape;
   skipping the auto-link description.
2. **`kdevkit-stream-closure.smoke`** — closing a feature
   whose spec carries `Part of initiative: [[X]]`; the
   `expected_narrative` covers both branches:
   (a) **Normal stream case**: walk §8.1–§8.3 plus §8.3.5
   (update X's Status table row with branch / CR /
   status=shipped / ship date / one-line learning), then §8.4
   commit + push + Closure Review Gate + squash-merge — all
   in the same `close(<feature>):` commit.
   (b) **Last-stream branch**: if every other Status table row
   were already `shipped`, the same `close(<feature>):` commit
   also archives the spec (`git rm
   $SPEC_ROOT/initiative/<name>.md`) and removes the line from
   `project.md`'s `## Active initiatives` index — no separate
   `close(<initiative>):` ceremony.
   Wrong answers: skipping §8.3.5; staging the Status table
   update separately from the close-commit; archiving the
   spec but forgetting the index entry; creating
   `close(<initiative>):`.
3. **`kdevkit-cross-stream-rebase.smoke`** — Stream 2 is in
   dev; Stream 1's CR re-ships to `main` after review; the
   user mentions "Stream 1 just merged"; the agent should
   describe the §10 cross-stream rebase mechanics in order:
   `git fetch origin && git rebase origin/main` → re-run §7
   Quality + Test + Code Review Gates → push with
   `git push --force-with-lease` (never plain `--force`) →
   update the open PR/CR body if the diff substantially
   changed. Procedural and easy to get wrong; standalone
   fixture justified.
   Wrong answers: ignoring Stream 1's merge; plain `--force`;
   skipping §7 re-run after rebase; merge commit instead of
   rebase; rebasing without re-running gates.
4. **`kdevkit-closure-after-long-session.smoke`** — the
   killer test for D. Closure is the highest-stakes event
   (squash-merge invariant lives in §8). The `expected_narrative`
   covers the full §8 cycle: §8.1 reconcile, §8.2 soft
   project.md verify, §8.3 backlog ask (interactive —
   *asking is the artifact*; must ask even if the answer is
   "none"), §8.3.5 if applicable, §8.4 commit + push, §8.5
   Closure Review Gate (title rewritten to `feat(...)` from
   `close(...)`), §8.6 squash-merge.
   Wrong answers: any §8 step skipped; treating the long
   conversational stream as evidence closure is redundant;
   auto-merging without §8.3 backlog ask; leaving the title
   as `close(...)` for the squash subject.

### Cuts and rationale

Five originally-listed fixtures were cut from this round:

- *initiative-start* — subsumed by `initiative-recognition`'s
  expected_narrative (entry-verb walk-through is part of
  recognition).
- *stream-auto-link* — implicitly tested by `stream-closure`:
  closure can't find the parent initiative and fire §8.3.5
  unless auto-link populated correctly upstream.
- *last-stream-archive* — folded into `stream-closure` as the
  second branch of the same `expected_narrative`.
- *initiative-after-long-session* — redundant with
  `closure-after-long-session` once every fixture runs
  stressed by default. Once the harness flag exists, every
  fixture is a long-session test for free.
- *planning-after-long-session* — same. The existing
  `kdevkit-feature-planning.smoke` already covers the §6
  plan-commit ordering; running it stressed is the
  long-session test.

The trade-off: cutting from 9 → 4 loses some isolated assertion
granularity. If `closure-after-long-session` fails, the
failure narrative says "closure broke under stress" but not
necessarily "the agent forgot the planning order
specifically." Acceptable — narrow tests get added when a real
failure exposes a gap, not preemptively.

### Conversational-stream stress: the shared prefix

A new file at `tests/functional/conversational-stream.txt`
carries the synthetic prefix authored once and reused across
every fixture. Shape: ~30–50 simulated turns of mixed coding
chatter — file edits, test runs, debugging tangents, some
kdevkit-relevant turns mixed in but not focal — followed by
the literal cue "Now the user says:" so the harness can
splice the per-fixture prompt after.

Authoring discipline for the prefix:

- **Mixed kdevkit relevance.** Some turns mention specs,
  feature branches, or commits but not the *current* feature.
  Stress comes from "did the agent track the context that
  matters here, despite all this nearby noise."
- **No internal markers.** Public-repo hygiene applies — the
  prefix lives in the same `specs/`-adjacent test surface
  the §9 internal-marker grep guards.
- **One file, edited centrally.** Drift between fixtures
  comes from divergent prefix content; a shared file
  prevents that drift.

### Two-mode harness

`tests/functional/run` gains a `--stressed` flag. Both modes
must pass the same `expected_narrative`:

- **Bare (default)** — same as today; the per-fixture
  `prompt:` is sent verbatim. Fast pre-flight; cheaper API
  cost; first signal that the contract holds *at all*.
- **Stressed (`--stressed`)** — the harness reads
  `tests/functional/conversational-stream.txt`, prepends
  it to each fixture's `prompt:` (separated by a clear
  marker like `\n\nNow the user says: `), then sends. Slower,
  costs more API credits, but proves the contract survives
  compaction-window pressure.

Both modes evaluate the same `expected_narrative`. A pass
in bare + fail in stressed = a real bug surfaced by
compaction. A fail in both = the change broke the contract
fundamentally; bare is the cheaper signal to debug from.

### Functional gate runs inside D's dev loop

**Project.md override scoped to D**: the agent runs
`deno task test:functional --stressed` (and the bare mode if
needed) **as part of D's §7 Test Gate**, not as a user-driven
follow-up. project.md's "Functional tests are user-driven"
rule is a sensible default for most features but blocks the
feedback loop that makes compaction tractable — D's whole
point is "did the contract survive?" and only the smokes
answer that. The agent reads judge feedback, patches
SKILL.md / setup.md / interviews.md, re-runs, until green.

This is a per-feature override, not a project.md change. After
D ships, future features default back to user-driven
functional. If a future feature also benefits from agent-side
functional runs, it carries its own override in its spec.

The override extends the §7 Test Gate's `retry_budget`: bare
+ stressed counts as one Test Gate invocation per cycle; if
either fails, the cycle fix-and-retries up to the default
budget of 2 cycles before stopping and reporting per §7.

### Quality gate

`deno task fmt && deno task lint && deno task check` after the
SKILL.md / setup.md / interviews.md edit slice. SKILL.md is
markdown — fmt may rewrap; lint and check are no-ops for `.md`.

### Test gate

`deno task test:unit` (default §7 Test Gate). 22 unit tests
must remain green. The deploy logic is unchanged: skill
directories deploy as-is whether they contain one file or
three. Schema validator runs against frontmatter — only
SKILL.md has frontmatter; the deferred files do not.
Verify by adding one schema unit test asserting the
deferred files (no frontmatter) deploy without warning.

## Design

### Diff shape

- **`sources/skills/kdevkit/SKILL.md`** — drops ~150–200
  lines (target ~500–550 lines from a current 964). The
  drops are the long-form prose blocks moved to deferred
  files; the **operational rules and triggers stay**.
- **`sources/skills/kdevkit/setup.md`** — new file,
  ~150–200 lines. project.md template + first-time detection
  + code-review setup prompt + `code_review:` schema +
  sticky-write rules + `## Agent Development` block schema +
  `## Active initiatives` index format.
- **`sources/skills/kdevkit/interviews.md`** — new file,
  ~200–250 lines. Four short interviews + feature file
  template + backlog item template + initiative file
  template + initiative entry-verb walk-throughs.

### Verify subagent dispatch

The subagent dispatch primitive uses the same generic phrasing
as §7 Code Review Gate. SKILL.md's §2 verify subsection:

> When the structural check (project.md exists + six
> headings + `code_review:` block + active-initiatives
> index matches `initiative/` listing) reports drift,
> dispatch a fresh-context agent call with these inputs:
> the path to `project.md`, the path to `setup.md`, and the
> on-disk listing of `$SPEC_ROOT/initiative/`. The agent
> loads `setup.md` and `project.md`, validates against the
> canonical schema, and returns
> `{ status, findings: [...] }`.
>
> Main applies any accepted findings via Edit. The setup
> narrative never enters main's context.
>
> Host translation: Claude Code's Agent tool, Kiro's
> equivalent, the host's fresh-context primitive of choice.
> Where unavailable, fall back to inline-Read of `setup.md`
> and run the validation in main.

### Inline-Read primitive

SKILL.md uses generic phrasing for inline-Read:
"inline-Read `setup.md`" / "inline-Read `interviews.md`". The
host translates — for Claude Code, this is the Read tool. The
deferred files are part of the same skill directory, deployed
as the `~/.claude/skills/kdevkit/` symlink (existing registry
entry — no registry change needed since the registry deploys
the whole directory as a symlink).

The deferred files do **not** carry frontmatter. mAId's
schema validator loads SKILL.md's frontmatter; the validator
should not trip on `setup.md` / `interviews.md` lacking it.
Add a schema unit test (or audit the existing parser) to
confirm only `SKILL.md` is required to have frontmatter
inside a skill directory; sibling files are free-form.

### Future-feature rule placement

The single-source-of-truth rule for placement
(operational → SKILL.md; setup/template → deferred) lands in
SKILL.md's introduction near the top, with a one-line
back-reference from §9. Adding it once at the top means
every future-feature author sees it before reaching §-rules
that drop new templates.

### Trade-offs considered

- **Two deferred files vs. one `deferred.md`.** Two-file
  split chosen. project.md setup and feature interviews fire
  on different events (project genesis vs. feature genesis);
  combining them means a fresh-feature session pays the
  project.md template tax. The marginal complexity (two
  deferred files instead of one) is justified by the load
  asymmetry.
- **Verify subagent return shape.** Free-form `findings`
  chosen over structured diff hunks. The subagent doesn't see
  the live file post-context; describing the issue lets main
  apply the edit against the current state. Diff hunks would
  be brittle to file-shift between subagent dispatch and
  main's apply.
- **Templates as inline-Read vs. separate files.** Single
  `interviews.md` chosen over per-template files
  (`feature-template.md`, `backlog-template.md`,
  `initiative-template.md`). One file is simpler; if it grows
  past ~300 lines we shard. Templates are short enough that
  the all-in-one shape is fine.
- **Skill version bump.** 3.0.0 → 3.1.0 chosen (signpost
  minor — restructure with no user-visible behavior change).
  3.0.0 was the initiative tier; 3.1.0 is the compaction.
  Keeping the major version stable is honest about what
  the user sees.

### Composes with v2.7.0 (A) and v3.0.0 (B)

- **A's §7 comment-prefix convention** — operational, fires
  every dev/closure cycle. Stays in SKILL.md verbatim.
- **B's §10 initiative tier** — operational sections (entry
  verbs, cross-stream rebase, repo-shape guidance) stay in
  SKILL.md. Template + entry-verb template-fill steps move to
  `interviews.md`. The `## Active initiatives` index format
  moves to `setup.md` (it's a project.md schema element).
- **§8.3.5 closure step (B)** — operational, fires every
  closure when the feature is part of an initiative. Stays in
  SKILL.md.
- **§9 cross-stream rebase carve-out (B)** — already a
  one-line pointer to §10 (per B's review); §10 carries the
  procedure. Procedure stays in SKILL.md.

## Implementation Plan

One slice. The diff is structurally larger than A or B because
two new files are added and SKILL.md is rewritten in place,
but the work is mechanical: identify operational vs. setup
prose, move setup prose to the right deferred file, leave
triggers behind in SKILL.md. No semantic invention.

1. **Inventory the cut.** Walk the post-A + post-B SKILL.md
   section by section. Classify each prose block:
   *operational* (every session) vs. *setup* (project / feature
   / initiative genesis). Output the classification table at
   the top of the implementation slice as a Decision Log
   entry — that table is the source of truth for the move.
2. **Carve `setup.md`.** Move setup-class prose into
   `setup.md`, preserving wording. Sections in order:
   project.md template → first-time detection prose →
   `## Agent Development` block schema (including
   `code_review:`, `prefer_worktree`, `planning_phase`) →
   code-review setup prompt long-form → sticky-write rules →
   `## Active initiatives` index format. No paraphrase.
3. **Carve `interviews.md`.** Move interview-class prose into
   `interviews.md`, preserving wording. Sections in order:
   four short interviews → feature file template body →
   backlog item template → initiative file template →
   initiative entry-verb walk-throughs. No paraphrase.
4. **Rewrite SKILL.md trigger points.** §2, §3, §4, §6, §10
   each gain a one-line inline-Read trigger where the moved
   content used to live. Add a verify-as-subagent subsection
   to §2 immediately after "If `$SPEC_ROOT/project.md` exists,
   read it silently."
5. **Add the future-feature placement rule.** One-line
   subsection in SKILL.md introduction (after the
   `Three nested loops` block, before §1) plus a one-line
   back-reference from §9.
6. **Bump SKILL.md frontmatter** version 3.0.0 → 3.1.0; update
   description to mention the on-demand-load shape.
7. **Schema unit test.** Add one unit test in
   `tests/schema_test.ts` asserting that a skill directory
   with a `SKILL.md` plus sibling files (no frontmatter)
   parses cleanly. Confirms the parser doesn't trip on the
   deferred files.
8. **Run Quality Gate.** `deno task fmt && deno task lint
   && deno task check`.
9. **Run Test Gate.** `deno task test:unit`. 22 unit tests
   must stay green; the new schema test must pass (count
   becomes 23).
10. **Audit existing functional smokes.** For each of the 7
    existing fixtures, re-read the `expected_narrative` and
    confirm it asserts behavior (not file location). Any
    fixture that says "the SKILL.md file says X" gets patched
    to "the agent says X" or equivalent. Stage edits.
11. **Add the shared conversational-stream prefix.** Author
    `tests/functional/conversational-stream.txt` with ~30–50
    simulated turns of mixed coding chatter (file edits,
    test runs, debugging tangents, some kdevkit-relevant
    turns mixed in but not focal). One file, edited
    centrally; the §9 internal-marker grep applies.
12. **Add the `--stressed` harness flag.** Extend
    `tests/functional/run` so `--stressed` reads
    `conversational-stream.txt`, prepends it to each
    fixture's `prompt:` (separated by `\n\nNow the user
    says: `), and sends the combined prompt. Both modes
    evaluate the same `expected_narrative`. Update the
    `--help` block and the harness's top-of-file usage
    comment.
13. **Add 4 new judge-mode fixtures**:
    - `kdevkit-initiative-recognition.smoke`
    - `kdevkit-stream-closure.smoke`
    - `kdevkit-cross-stream-rebase.smoke`
    - `kdevkit-closure-after-long-session.smoke`
    All `prompt: ` + `expected_narrative: ` shape; no
    fixture-local conversational-stream prefix (the harness
    flag handles stress).
14. **Run §7 Test Gate with functional smokes (D-scoped
    override).** `deno task test:functional` (bare) +
    `deno task test:functional -- --stressed` (or
    `tests/functional/run --stressed`, depending on the
    deno-task flag-passthrough shape). Read judge feedback;
    patch SKILL.md / setup.md / interviews.md; re-run.
    Within the §7 default `retry_budget: 2`. The override
    is scoped to D — future features default back to
    project.md's "user-driven" rule.
15. **Run Code Review Gate.** Per `code_review.reviewer:
    host-native`, threshold 70, hard-stop, retry-budget 2.
    Reviewer sees `project.md` + the diff (which now spans
    SKILL.md + new files + harness change + new fixtures);
    no feature spec.
16. **Push.** Open Agent-dev Review Gate per §7. Body:
    Approach + Reading order grouped by phase. Include
    test-gate evidence (bare + stressed pass counts).
17. **Closure** (per session override): §8.1 reconcile, §8.2
    soft project.md verify (likely no edits — mAId's project.md
    is already clean), §8.3 backlog ask (this feature closes
    the kdevkit-compaction backlog item, which was `git mv`'d
    at branch start), §8.3.5 not applicable (D is not part of
    an initiative), §8.4 commit + push closure edits if any,
    §8.5 Closure Review Gate, §8.6 squash-merge.

Risk notes:

- *Wording drift in moved prose.* The classification table
  in step 1 is meant to be exact source-of-truth; mechanical
  move only. Reviewer flags any rewording that changes
  meaning.
- *Inline-Read trigger phrasing.* The triggers must be tool-
  agnostic. "inline-Read `setup.md`" is generic; "use the
  Read tool to load `setup.md`" is Claude-Code-specific. The
  generic phrasing is the contract.
- *Behavior fidelity under compaction.* The whole point of
  this feature is that user-visible behavior is unchanged.
  The 9 new smokes plus the 7 existing smokes are the
  evidence that the contract holds. If a smoke regresses,
  the compaction broke a behavior contract — fix forward
  before pushing.
- *Conversational-stream prefix realism.* The prefix is
  synthetic; it injects content but not literal multi-turn
  delivery. Sufficient for compaction-window stress (since
  main-loop compaction summarizes prior turns into one
  block — content survives, turn count doesn't), but it
  doesn't fully emulate "the agent's last summary was
  truncated mid-thought." Acceptable trade-off; flag for D+1
  if a real-case regression exposes the gap.
- *Frontmatter parser.* The schema validator must not
  require frontmatter on `setup.md` / `interviews.md`. The
  step-7 unit test pins this; if the existing parser already
  enforces frontmatter on every `.md` file in a skill
  directory, the parser needs a small tweak to scope the
  requirement to `SKILL.md` only.
- *Host fallback for verify.* Subagent dispatch is host-
  capability; the inline-Read fallback covers absence. Not
  CI-coverable on mAId's current functional suite (which
  uses real `claude` and `kiro-cli` binaries that both
  support the primitive).

## Session Log

<!-- append: date · what was done · decisions made -->

- 2026-06-03 · backlog → feature promotion · §6 four
  interviews completed autonomously per user direction:
  Requirements (file split scope, verify primitive, inline-Read
  primitive, SKILL.md trigger points, frontmatter rules,
  future-feature placement rule); Test Strategy (behavior-based
  smokes; conversational-stream prefix on three of the new
  fixtures; existing 7 fixtures audited for shape-vs-behavior);
  Design (single-deferred-file vs. two-file → two-file;
  free-form vs. diff-hunk findings → free-form; verify dispatch
  generic phrasing); Implementation Plan (14 ordered steps).
  Open questions from the backlog all resolved autonomously
  (granularity, return shape, template grouping, fallback
  testing punted, future-feature rule).

- 2026-06-03 · plan rev 2 · cut new fixtures 9 → 4
  (initiative-recognition, stream-closure, cross-stream-rebase,
  closure-after-long-session); fold initiative-start /
  stream-auto-link / last-stream-archive into other fixtures'
  expected_narrative; drop initiative-after-long-session and
  planning-after-long-session as redundant once stress is
  applied to every fixture. Add shared
  `tests/functional/conversational-stream.txt` + harness
  `--stressed` flag; every fixture (existing 7 + new 4) runs
  in both bare and stressed modes against the same
  expected_narrative. Add D-scoped override: §7 Test Gate
  runs functional smokes in-loop so the agent can read judge
  feedback and patch SKILL.md / setup.md / interviews.md
  within retry_budget instead of waiting for user-driven
  runs. Implementation Plan grew 14 → 17 steps.

## Decision Log

<!-- append: decision · rationale · alternatives rejected -->

- **Two deferred files (`setup.md`, `interviews.md`), not
  one `deferred.md`.** Rationale: project.md setup and
  feature interviews fire on different events (project genesis
  vs. feature genesis); combining them means a fresh-feature
  session pays the project.md template tax. Two-file split
  honors the load asymmetry. Alternative rejected: single
  `deferred.md` for "simpler structure" — saves one file at
  the cost of always-loaded irrelevant prose during partial
  loads.
- **Verify subagent returns free-form `findings`, not diff
  hunks.** Rationale: the subagent doesn't see the live file
  post-context; describing the issue lets main apply the
  edit against the current state. Diff hunks would be
  brittle if the file shifted between subagent dispatch and
  main's apply. Alternative rejected: structured diff hunks
  for atomic apply — too brittle.
- **Verify dispatch reuses §7 Code Review Gate's generic
  fresh-context primitive.** Rationale: same shape, same
  host-portability story. The host translates "fresh-context
  agent call" (Claude Code Agent tool, Kiro equivalent).
  Inline-Read fallback covers hosts without subagent
  dispatch. Alternative rejected: bake Claude Code Agent
  tool into SKILL.md prose — would break Kiro / Codex
  portability.
- **Inline-Read for create flows; subagent for verify
  only.** Rationale: create flows (write project.md, run
  four interviews, write initiative) are interactive,
  multi-turn, conversational; round-tripping through a
  subagent every turn is awkward and slow. Verify is one-shot
  with a structured return — fits the subagent shape.
  Alternative rejected: subagent for both — more isolation
  but hostile to interactive flows.
- **Behavior-based smokes, not shape-based.** Rationale:
  per user direction. The skill should do the right thing
  whether or not compaction landed. Smokes assert
  user-visible behavior; whether main inline-Reads a
  deferred file or has the prose in context is invisible
  and irrelevant to correctness. Alternative rejected:
  shape-checking smokes ("did the agent reference setup
  narrative?") — couples the test to the implementation,
  breaks every time the load pattern changes.
- **Four new fixtures, not nine.** Rationale: at nine
  fixtures the spec became un-reviewable. Cutting to four
  preserves the broad coverage (initiative recognition,
  stream closure including last-stream branch, cross-stream
  rebase, closure-after-long-session) and folds isolated
  assertions into shared `expected_narrative` text where
  they were redundant. Each prompt + narrative is now
  short enough to read end-to-end. Alternative rejected:
  nine narrowly-scoped fixtures — over-specification when
  four broad-narrative fixtures cover the same ground via
  expected_narrative discipline; failure isolation is what
  you do *after* a real failure exposes a gap.
- **Shared `conversational-stream.txt` prefix; harness
  `--stressed` flag; every fixture runs in both modes.**
  Rationale: per user direction. Authoring 11 inline
  prefixes (one per fixture) would make stress vary by
  fixture and "did this fail because of stress content X"
  becomes a real question. One shared prefix = consistent
  stress; one place to evolve. The bare → stressed split
  is a CLI flag, not 11 separate fixture files. Both modes
  evaluate the same `expected_narrative` — passing both
  proves the contract survives compaction-window pressure;
  passing bare and failing stressed = a real bug surfaced
  by compaction; failing both = a fundamental contract
  break (debug bare first, it's cheaper). Alternative
  rejected: per-fixture inline prefix — drift risk;
  fragmented stress content; harder to evolve.
- **Conversational-stream prefix is synthetic
  (single-shot prompt with embedded content), not literal
  multi-turn.** Rationale: main-loop compaction summarizes
  prior turns into one block — content survives, turn count
  doesn't. A synthetic prefix carries the content stress
  without requiring multi-turn API delivery, keeping the
  harness simple. Alternative rejected: literal multi-turn
  via a session-aware harness — would require a new test
  scaffolding effort; not justified by the marginal fidelity
  gain for "did the agent retain the contract."
- **D's §7 Test Gate runs functional smokes in-loop
  (per-feature override of project.md's "user-driven"
  rule).** Rationale: D's whole point is "did the contract
  survive compaction?" and only the smokes answer that.
  Running them user-driven means a slow human-mediated
  feedback loop (agent edits, user runs, agent reads
  feedback, repeats); running them in-loop lets the agent
  fix-and-retry within the existing §7 retry_budget. The
  override is scoped to D's spec — future features default
  back to project.md's "user-driven" rule unless their own
  spec declares an override. Alternative rejected: change
  project.md's rule globally — over-broad; most features
  don't need agent-side functional runs and would pay the
  API-credit cost without benefit.
- **`setup.md` and `interviews.md` carry no frontmatter.**
  Rationale: only SKILL.md is the discoverable entrypoint;
  sibling files are inline-Read'd by reference. Frontmatter
  on the deferred files would imply they're independently
  discoverable, which they're not. Alternative rejected:
  frontmatter on every deferred file — implies discoverability
  that doesn't exist; misleads schema parsers.
- **Existing 7 functional smokes audited, not rewritten.**
  Rationale: most assert behavior already; any that assert
  file location get patched in line. Wholesale rewrite would
  risk losing regression coverage. Alternative rejected:
  rewrite all fixtures from scratch under the new behavior-
  based shape — too much churn; existing fixtures already
  work.
- **Version bump 3.0.0 → 3.1.0 (signpost minor).**
  Rationale: restructure with no user-visible behavior
  change; minor bump is honest. 3.0.0 was the initiative
  tier (visible new tier); 3.1.0 is compaction (invisible
  to users). Keeps the major version stable for what users
  actually see. Alternative rejected: 4.0.0 (major) — would
  signal a breaking change that doesn't exist.
- **Future-feature placement rule lands in SKILL.md
  introduction.** Rationale: every future-feature author
  reads SKILL.md before adding sections. Putting the rule
  at the top means it's seen before any §-section that
  could tempt new template prose. Alternative rejected:
  rule in §9 only — gets read after the whole skill, by
  which point new sections may already be drafted.
- **Schema unit test added in step 7 (sole code change in
  this feature).** Rationale: the deferred files lack
  frontmatter; pin the parser's tolerance. Without this
  test, a future schema tightening could silently break
  the on-demand-load shape. Alternative rejected: skip the
  unit test — relies on the parser's current behavior
  staying friendly.
