# kdevkit — setup (deferred)

This file carries the schemas, templates, and one-time setup
prompts that fire only on **project genesis** or when an
on-disk `project.md` is found to have drifted from the kdevkit
schema. Loaded by main on demand (inline-Read for create flows,
fresh-context subagent for verify) — never always-on.

## `project.md` template

Six sections, fixed order. The HTML comments are prompts —
keep them in place so future sessions re-read the intent.

```markdown
# Project: <name>

## Mission

<!-- Purpose + who it serves. One paragraph. -->

## Architecture

<!-- Logical shape: components + responsibilities. Words mandatory. -->

## Tech Stack

<!-- Languages, runtimes, frameworks. Versions where they matter. -->

## Layout

<!-- Directory tree, one-line annotation per entry. -->

## Testing

<!-- Test layers + which is load-bearing. Command strings live in
     a repo-root AGENTS.md where one exists (SKILL.md §2 Context
     layers); here, carry the layer semantics, not duplicated
     commands. -->

## Deployment

<!-- Build / release / install path, or how it's consumed. -->
```

**Context layers (SKILL.md §2).** `project.md` is the
project-knowledge layer — the persistent *why* and *shape*. It is
not a repo-root `AGENTS.md`: operational command strings belong in
`AGENTS.md` where the repo keeps one, and kdevkit never writes its
own scaffold (these headers, HTML prompts, logs, the initiatives
index) into `AGENTS.md`. Keep both persistent files lean — exact
commands and explicit boundaries over prose; over-stuffed context
files degrade agent performance.

## First-time `project.md` detection

When creating `project.md`, probe ecosystem markers
(`package.json`, `pyproject.toml`, `Cargo.toml`, `go.mod`,
`Makefile`, `deno.json`) and CI files (`.github/workflows/*`,
`.gitlab-ci.yml` — verbatim) for the toolchain; confirm in
one batch; write Testing as prose — §7 reads commands from it
at run time.

Append the **Code-review setup prompt** (below) as a one-liner
to the same setup batch so the new project lands with a
`code_review:` block already declared.

## Optional `## Agent Development` section

`project.md` may carry an `## Agent Development` section
organised by skill. Keys under `kdevkit`:

- `prefer_worktree: true|false` — feature-start worktree
  recommendation (see SKILL.md §4).
- `planning_phase: true|false` (default `true`) — three-phase
  feature branch (planning → dev → closure) per
  SKILL.md §5 and the phase modules. Set `false` to skip §6 Planning (`phases/plan.md`) and
  let spec edits ride with the first dev commit.
- `code_review:` — nested block configuring the §7 Code Review
  Gate. All keys optional; defaults below.

  ```yaml
  code_review:
    reviewer: host-native       # default; alternative: skill:<name>
                                # / mcp:<server>.<tool> / agent:<name>
    threshold: 70               # 0–100 score floor for Push
    authority: hard-stop        # alternative: soft
    retry_budget: 2             # total fix-and-retry cycles (incl. first review)
  ```

  - **`reviewer`** — who runs the review. Prefix-tagged so the
    orchestrator knows what to dispatch:
    - `host-native` (default) — the host coding agent's built-in
      code review.
    - `skill:<name>` — a skill in the registry (bare strings
      without a prefix default to `skill:`).
    - `mcp:<server>.<tool>` — an MCP server's tool.
    - `agent:<name>` — a named project-configured agent.
  - **`threshold`** — score floor; sub-threshold loops back to
    Quality. Default `70`.
  - **`authority`** — `hard-stop` blocks Push when retries
    exhaust; `soft` allows Push with residuals appended to
    Session Log.
  - **`retry_budget`** — total review attempts including the
    first (a budget of 2 = up to 2 outer review cycles, not 2
    retries on top of an initial pass). Default `2`. The Test
    Gate uses the same "attempts including first" semantics.

  Omitting the block entirely triggers the Code-review setup
  prompt below. Once written (even with all defaults), the
  block sticks — the question doesn't re-fire next session.

- `review_brief:` — nested block naming the briefing generator
  for §7's Review Briefing. All keys optional; the block itself
  is optional and **absent means off**, so unlike `code_review:`
  it fires no setup prompt.

  ```yaml
  review_brief:
    enabled: false              # default; true opts the briefing in
    generator: <ref>            # optional; omit to auto-resolve the
                                # installed review-briefing role
  ```

  - **`enabled`** — `false` (default) means no briefing; the dev
    loop is exactly as it was. `true` generates one and uses it as
    the PR/CR body.
  - **`generator`** — *which* tool fills the review-briefing role,
    using the same prefix-tagged `<ref>` grammar as
    `code_review.reviewer` above. Omit it in the common case:
    kdevkit resolves the single installed tool advertising the
    role, and asks once (persisting the answer here) when that is
    ambiguous or finds nothing. This key is the only place a
    specific briefing tool is ever named — kdevkit itself
    dispatches a role.

  **What the generator needs, how it runs, and what a briefing
  contains are the generator's contract, not kdevkit's** — so
  there are no keys here for inputs, context isolation, or
  section shape. kdevkit reads the generator's own definition and
  honours what it asks for, which is what lets a project swap in a
  briefing tool with an entirely different contract without
  touching kdevkit.

  Note the asymmetry with `code_review:`: a missing `code_review:`
  block means "not yet decided" (prompt the user), whereas a
  missing `review_brief:` block means "not wanted" (stay silent).
  A step that costs an extra agent call should be opted into, not
  prompted for.

## Code-review setup prompt

When `kdevkit.code_review:` is missing from `project.md`,
fire a one-line prompt on session entry — the §7 Code Review
Gate is the only gate that reads the config, but firing on
entry (regardless of fresh / continue / pick-up mode) keeps
the prompt out of the dev loop:

> _"This project doesn't declare a code reviewer. Use the
> host's native review (default), or point to a project-specific
> one (`skill:<name>` / `mcp:<server>.<tool>` / `agent:<name>`)?
> Reply 'default', paste a reference, or 'skip'."_

Alongside the prompt, surface a one-line note (outside the
blockquote so it isn't read as a reply option): _"This skill
prefixes agent-authored CR/PR comments with `[agent]:` so
review threads stay disambiguable when builder and reviewer
share an identity (see `phases/review.md` §7); your own comments don't
need a prefix."_

Then **sticky-write** the answer to `project.md`'s `## Agent
Development > kdevkit` block so the question doesn't re-fire
next session:

- Reply `'default'` → write
  `code_review: { reviewer: host-native }`.
- Reply with a `<ref>` → write
  `code_review: { reviewer: <ref> }`. Threshold / authority /
  retry_budget inherit defaults.
- Reply `'skip'` → no write; question re-fires next session.
  (Lets a user defer the decision without committing.)

The same prompt fires from the first-time `project.md` flow
above as the appended one-liner.

## Optional `## Active initiatives` index

When in-flight initiatives exist (see `tiers/initiative.md` §10),
`project.md` MAY carry an `## Active initiatives` index near
the bottom — one line per initiative, removed at last-stream
close:

```markdown
## Active initiatives

- **<name>** (`initiative/<name>.md`) — <one-line intent>
```

The index lets the agent skip loading every initiative file
unconditionally; only the initiative(s) referenced by the
current entry cue or the current feature load.

## Verify schema (for the verify-as-subagent primitive)

The verify subagent (dispatched by SKILL.md §2 when drift is
detected) validates `project.md` against the canonical schema
above. Returns:

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

Validation rules the subagent applies, in order:

1. **Six required headings present**, in fixed order: Mission,
   Architecture, Tech Stack, Layout, Testing, Deployment. Out
   of order, missing, or duplicated → `drift`.
2. **`## Agent Development > kdevkit > code_review:` block
   present** with at least the `reviewer:` key, OR the block is
   entirely absent (in which case main fires the Code-review
   setup prompt). Block present-but-malformed → `drift`.
3. **`review_brief:` block, if present, parses with no unknown
   keys** — only `enabled` and `generator` are recognized. The
   block is optional and its absence is **not** drift (absent
   means the gate is off; no setup prompt fires for it).
4. **`## Active initiatives` index, if present, matches
   `$SPEC_ROOT/initiative/`** — every line in the index has a
   matching `initiative/<name>.md` on disk; every on-disk
   initiative either has an index line or is archived. Drift
   in either direction → `drift`.

`findings` are free-form (one issue + one suggestion per row).
Main applies accepted findings via Edit against the live
`project.md`. The subagent does not return diff hunks — it
doesn't see the live file post-context, so the safest contract
is "describe the issue and the remedy"; main applies the edit
against the actual file.

If the host doesn't support fresh-context subagent dispatch,
main inline-Reads this file and runs the validation itself.
Behavior degrades to today's footprint; no breakage.
