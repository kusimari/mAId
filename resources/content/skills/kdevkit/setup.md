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

<!-- Test layers + commands; load-bearing vs. nice-to-have. -->

## Deployment

<!-- Build / release / install path, or how it's consumed. -->
```

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
  SKILL.md §5/§6/§7/§8. Set `false` to skip §6 Planning and
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
share an identity (see SKILL.md §7); your own comments don't
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

When in-flight initiatives exist (see SKILL.md §10),
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
3. **`## Active initiatives` index, if present, matches
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
