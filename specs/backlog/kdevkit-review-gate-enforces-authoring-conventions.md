# Backlog: kdevkit — Code Review Gate should enforce always-on authoring conventions

## What

Extend the §7 Code Review Gate **dispatch contract** so the
reviewer receives the skill's own always-on *authoring*
conventions, not just `project.md` + the diff. Today those
conventions (e.g. "Comments carry intent, not history",
"Write for intent", "Reach for what exists") live in `SKILL.md`
and are given to *no* gate — so nothing but the author's
in-the-moment memory enforces them.

Concretely, the contract at `SKILL.md` (~line 581, "Dispatch
contract" / "Receives:" / "Excluded:") currently passes the
reviewer:

- `project.md`
- the diff vs. base
- reviewer reference + threshold + authority + retry_budget

…and deliberately excludes the feature spec, session/decision
logs, and conversation history. That exclusion is correct (it
keeps the review honest about the diff-vs-project, not
diff-vs-plan). But it means the reviewer also never sees the
always-on authoring rules, which are neither in `project.md`
nor in the diff.

Proposed change: add a fourth item to "Receives:" — a small,
fixed extract of the always-on **authoring** conventions from
§6/§7 (the ones that judge *how code and comments are
written*, distinct from design/planning rules). Candidates:

- "Comments carry intent, not history" (§7 Write for intent).
- "Write for intent" (frame functions around caller intent;
  reach first for what's in reach; match surrounding style).
- "Reach for what exists" (§6, design-time — the library/idiom
  survey) insofar as it's checkable from the diff.

Keep the feature spec / logs / history exclusions exactly as
they are — this adds only the skill's own authoring rubric, not
feature context.

## Why

- **Observed live (2026-07-15).** In a kdevkit session the agent
  wrote comments that narrated the code ("keeps set -e happy"
  next to `|| true`; an `if`-guard comment restating the guard),
  violating the always-on "Comments carry intent, not history"
  rule — *even though the skill was loaded and the rule was in
  context.* The Code Review Gate ran and scored 90/100 without
  flagging it, because the reviewer's dispatch contract never
  included that rule. The user caught it manually.
- **The convention was orphaned from every gate.** Quality Gate
  is deterministic-only (fmt/lint/type) by definition. Test Gate
  is behavioral. Code Review is the *only* gate that can catch a
  subjective authoring-convention miss — and it's dispatched
  without the document that defines those conventions. So an
  always-on `SKILL.md` authoring rule is currently enforceable
  *only* by the author remembering it mid-write, which is the
  weakest possible enforcement and the exact failure mode
  observed.
- **Right-sized, not ceremony.** This is deliberately narrow: it
  reuses the existing gate and adds one input, rather than adding
  a new "verify all standing instructions" phase (which would be
  the ceremony creep kdevkit is otherwise trying to shed). The
  broad checklist version is explicitly *not* wanted; the sharp
  fix targets the specific gap (reviewer can't see the rules it's
  meant to apply).

## Open questions

- **Which rules qualify as "authoring" vs. "planning."** The
  reviewer should get rules that judge the *diff as written*
  (comments, function shape, reuse-in-diff), not planning-phase
  rules (four interviews, requirements smell test) that have no
  diff to check against. Needs a bright line so the extract
  stays small and stable.
- **How to pass it without bloating the dispatch.** Options: (a)
  a short inlined rubric string in the dispatch; (b) a pointer to
  a specific `SKILL.md` section the reviewer inline-Reads; (c) a
  dedicated deferred `review-rubric.md` the reviewer loads. (a)
  keeps the reviewer's context lean; (c) centralizes the list but
  adds a file. Lean toward (a) or (b).
- **Score interaction.** Should an authoring-convention miss be a
  hard-stop (< threshold) or advisory? Comment phrasing is lower
  severity than a correctness bug; maybe these findings are
  reported but weighted so they don't alone sink the score below
  threshold. Define the weighting.
- **Does an author-side pre-commit self-check also belong?** A
  cheap "re-read the diff against the authoring rules before
  dispatch" step was considered and rejected as the *primary*
  fix (it relies on author memory — the failure mode itself). It
  could still be a lightweight secondary. Decide whether to
  include it or rely solely on the gate.
- **Interaction with `code-review` skill / host-native reviewer.**
  If the reviewer is a separate skill or host-native tool, confirm
  the extra input threads through its interface, not just the
  in-skill dispatch description.

## Trigger to promote

- Another authoring-convention miss survives the Code Review Gate
  (this observed case is the first).
- The `code-review` skill or dispatch contract is being edited
  anyway — bundle this in.
- A decision to formalize an authoring rubric (its own file)
  independently.

## Note on editing the skill

`resources/content/skills/kdevkit/SKILL.md` is the source behind
the managed skills symlink — edit it here in the repo, not under
`~/.claude/skills/kdevkit/`. Changes land in the next session.
The dispatch-contract change is always-on operational content, so
it belongs in `SKILL.md` (§7 Code Review Gate), per the skill's
own multi-file placement rule.
