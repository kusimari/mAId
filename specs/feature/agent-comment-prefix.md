# Feature: agent-comment-prefix

## Git Setup

- Branch: `feat/agent-comment-prefix`
- Base: `main` at `8f922c6` (post-backlog-record)

## Feature Brief

Add an `[agent]:` comment-prefix convention to the kdevkit skill
so that when a coding agent operates the CR/PR tooling on the
human's behalf, the audit trail in the CR is disambiguable even
though both parties post under the same identity. The agent
prefixes every comment body it posts on the CR/PR with a literal
`[agent]:` on the first line. The human's comments stay
unprefixed by default; bare comments read as human.

The convention applies to comment bodies only — not the diff,
not commit messages (already attributable via Conventional
Commits subjects), and not the CR/PR description (no thread to
disambiguate; it's structurally the agent's artefact, generated
from the commit log). The rule is generic across whatever the
agent posts: replies to reviewer threads, free-form follow-ups,
status/ack-style comments — all `[agent]:`-prefixed without
carve-outs.

## Requirements

- **Prefix shape: bare `[agent]:`.** Literal string, first line
  of the comment body, followed by a colon. The value being
  encoded is "not human" — qualifying the prefix with skill name
  or model name is brittle (skill stack changes; host changes
  mean the prefix would lie) and adds noise without information
  a future reader would use.
- **Scope: every comment body the agent posts on the CR/PR.**
  No carve-outs. The skill does not enumerate AutoSDE,
  `/code-review`, or any other downstream skill that may post on
  the agent's behalf — kdevkit's rule is "if the agent is
  posting, prefix it." Other skills inherit by being invoked by
  the agent that already operates under this rule. The convention
  travels with the actor, not with the tool.
- **Out of scope: CR/PR description.** Description has no thread
  to disambiguate, and is structurally the agent's artefact
  (generated from the Conventional Commits log). Prefixing it
  would add noise without information.
- **Out of scope: commit messages.** Already attributable via
  the `feat(<scope>):` / `plan(<feature>):` / `close(<feature>):`
  subject; prefixing would break Conventional Commits parsers.
- **Out of scope: the diff.** The diff is the artefact, not a
  comment.
- **Human side: no prefix required.** The default is bare comment
  = human. A human MAY use `[human]:` to mark a comment as a
  steer rather than a review note, but the skill does not
  require it. The prefix is the agent's responsibility, not a
  symmetric convention.
- **Forward-only.** Convention applies to comments posted after
  the rule lands. Existing CRs at the time the convention
  ships do not get backfilled. Earlier comments stay
  un-prefixed and read by their context. If the rule lands
  mid-feature, the agent on the next loop in that feature's CR
  starts prefixing; earlier comments in the same CR stay
  un-prefixed.
- **Tool-agnostic phrasing in SKILL.md prose.** The rule must
  be stated in terms of "CR/PR comment bodies", not tied to
  any specific review tool. Tool-specific examples (e.g. the
  internal-review CLI's `reply -m '[agent]: ...'` vs. `gh pr
  comment --body '[agent]: ...'`) go inline with the rule or
  in the Reference shape, not in the rule statement.
- **Composes with §7 Agent-dev Review Gate, §8 Closure Review
  Gate, §4 Code-review setup prompt.** The cycle each gate
  drives — agent reads CR comment, agent replies / fixes — is
  where the rule fires. §4's setup prompt mentions the
  convention so a human seeing kdevkit for the first time
  understands what `[agent]:` means.

## Test Strategy

The convention is prose-only — no code, no executable behavior
to assert beyond "kdevkit prose contains the rule."

- **`test:unit`** — irrelevant. No schema or deploy logic
  touched. Stays green by default.
- **`test:smoke`** — irrelevant for behavior, but the
  symlink-resolution check still runs as the regression net
  (asserts kdevkit is reachable through `~/.claude/skills/` and
  `~/.kiro/steering/skills/` after deploy).
- **`test:functional` (judge mode)** — *deferred to D
  (kdevkit-compaction)*. Per the planning conversation, the
  three judge fixtures D plans to add already cover steady-state
  behavior, including the agent's CR-comment behavior. Adding a
  comment-prefix-specific fixture now would land twice: once
  here, once when D restructures the test surface. Bundling all
  new functional fixtures into D is the cleaner cut.
- **Dogfood evidence.** Every comment the agent posts on this
  feature's own PR/CR is the live demo. If the convention is
  correctly applied, every agent-authored comment on the PR
  starts with `[agent]:`. Human-verifiable in review.

The post-D work that D's spec inherits from this feature:

- One judge fixture asserting that an agent operating under
  kdevkit prefixes its CR comments with `[agent]:` when given a
  mock CR-reply scenario. Land it as part of D's three new
  judge fixtures, not as a standalone. (Recorded in D's backlog
  for follow-through.)

Existing kdevkit fixtures stay green:
`kdevkit.smoke`, `kdevkit-feature-loop.smoke`,
`kdevkit-feature-planning.smoke`, `kdevkit-feature-closure.smoke`,
`kdevkit-dev-loop.smoke`, `kdevkit-review-gate.smoke`,
`kdevkit-review-config-setup.smoke`. Regression net.

Quality gate per project.md:
`deno task fmt && deno task lint && deno task check`. Run
after the SKILL.md edit slice.

## Design

The diff lands in `sources/skills/kdevkit/SKILL.md` only. No
code, no template files, no new artefacts. Touch points by
section:

- **§7 Agent-dev Review Gate.** Add the prefix rule as a
  short subsection ("Comment-prefix convention"). Two example
  command shapes inline — one internal-review-CLI style, one
  GitHub `gh pr` style — framed as illustrative, not
  normative. The rule statement itself is tool-agnostic.
- **§8 Closure Review Gate.** One-line cross-reference back to
  §7's rule, since the closure cycle reuses the same comment
  discipline. No re-statement.
- **§4 Code-review setup prompt.** Append a sentence to the
  first-time-declaring-`code_review:` setup-prompt blurb so the
  human sees the convention at project genesis: _"This skill
  prefixes agent-authored CR comments with `[agent]:`; you
  don't need to prefix yours."_
- **§9 Cross-cutting rules.** Considered, rejected as the
  landing site. The convention is operational (fires inside
  the dev / closure cycles) rather than an always-on hygiene
  rule (commit author, public-repo grep). §9 stays for the
  rules that fire at every phase regardless of CR/PR posture;
  the prefix rule fires only when the agent is posting to a
  CR/PR thread, which §7 / §8 already scope.

The trade-off considered: a single source-of-truth subsection
in §9 with cross-refs from §7 and §8, vs. the rule landing in
§7 with a back-ref from §8. Picked §7 as the source-of-truth
because §7 is where the comment cycle lives; §8 inherits by
reusing §7's cycle. §9's threshold for "always-on cross-cutting"
stays unchanged.

The prefix string `[agent]:` is verbatim — no variation,
no parameter. A future "qualified prefix" (per-skill,
per-model) can ship as a v3.X feature if a real need surfaces;
not blocked, not preempted.

## Implementation Plan

One slice. Non-trivial only because the SKILL.md prose has
multiple insertion points and the tool-agnostic phrasing
needs care.

1. **Edit `sources/skills/kdevkit/SKILL.md`.**
   - Add new subsection in §7 between **Push Gate** and
     **Agent-dev Review Gate**, titled
     `### Comment-prefix convention`. Body covers: the rule
     (every agent-authored CR/PR comment body starts with
     `[agent]:` on the first line, no carve-outs, scope is
     "comment bodies on the CR/PR" — not description, not
     commit messages, not the diff), the human side
     (unprefixed by default; `[human]:` optional), forward-only
     migration, two illustrative example command shapes (one
     internal-review-CLI style, one `gh pr` style), and a
     one-line note pointing at §4 for the setup-prompt
     mention.
   - In §8 (Closure), add a one-line cross-reference at the
     top of the section: "Closure cycle reuses §7's
     comment-prefix convention for any agent-authored CR
     comments."
   - In §4 (Code-review setup prompt), append the convention
     mention to the prose-block describing what the prompt
     does. One sentence.
   - Bump frontmatter `version` from `2.6.0` to `2.7.0` (minor
     — additive convention, no behavior change to existing
     gates).
   - Update frontmatter `description` minimally if the new
     subsection's keyword should be discoverable; otherwise
     leave. Lean leave — the description already mentions
     "Quality/Test/Code-Review/Push/Review gates" which
     covers the §7 surface.
2. **Run Quality Gate.** `deno task fmt && deno task lint
   && deno task check`. The SKILL.md edit is markdown — fmt
   may rewrap; lint and check are no-ops for `.md` content.
3. **Run Test Gate.** `deno task test:unit`. Should be green
   (no schema or deploy code touched). Existing kdevkit smoke
   fixtures (loaded after deploy) verify SKILL.md still
   parses.
4. **Run Code Review Gate.** Per `code_review.reviewer:
   host-native` in `project.md`. Threshold 70, hard-stop,
   retry-budget 2. The reviewer sees `project.md` + the diff;
   no feature-spec context. Findings ≥ threshold → Push.
5. **Push.** Open Agent-dev Review Gate (PR) per §7. Body
   carries Approach + Reading order.
6. **Closure.** On `"feature done"` cue: §8 reconciles spec
   markers, soft project.md verify (likely no edits), backlog
   cleanup (the promoted backlog file is already gone — `git
   mv` happened on branch), commit + push closure edits, open
   Closure Review Gate, squash-merge.

Risk notes:

- *Phrasing drift.* The rule needs to be tool-agnostic in the
  rule statement and only show tool-specific examples after.
  Reviewer should flag if the rule statement names a specific
  review tool (it should say "CR/PR comment bodies", not name
  the CLI).
- *Discoverability.* The rule must be findable from §4 (where a
  human encounters kdevkit setup) and §7/§8 (where the agent
  encounters the comment cycle). Three landing points are
  enough; more would be redundant.
- *Forward-only edge case.* This very PR is the live demo —
  every agent comment on this PR's review thread should be
  `[agent]:`-prefixed. If the rule lands and the agent's first
  reply on the same PR is unprefixed, the dogfood is broken
  before it starts. Apply the rule in this PR's review cycle
  starting from the next agent comment after the rule is
  committed.

## Session Log

<!-- append: date · what was done · decisions made -->

- 2026-06-03 · backlog → feature promotion · ran §6 four
  interviews around existing What/Why · resolved 5 open
  questions (prefix=`[agent]:`, no description prefix, no
  scope carve-outs incl. acks, no per-skill enumeration of
  AutoSDE/etc., forward-only migration) · test strategy
  bundled into D (kdevkit-compaction) per user direction.

## Decision Log

<!-- append: decision · rationale · alternatives rejected -->

- **Prefix string is bare `[agent]:`.** Rationale: the value
  encoded is "not human" — the future reader cares whether the
  comment came from a model, not which model. Qualifying the
  prefix would be brittle across host changes and add noise.
  Alternatives rejected: `[agent kdevkit]:` (skill-scoped, but
  kdevkit's rule fires across other skills' posts too;
  qualifier would lie); `[agent claude]:` (host-coupled;
  switching hosts breaks the prefix's truthfulness).
- **No prefix on CR/PR description.** Rationale: description
  has no thread to disambiguate; structurally the agent's
  artefact (generated from commit log). Alternative rejected:
  symmetric "anything the agent writes gets the prefix" —
  cleanly stated but adds noise to the description without
  information value.
- **No carve-outs by comment type.** Free-form replies and
  short status acks both get prefixed; the agent doesn't have
  to categorise. Rationale: the rule is "agent posted →
  prefix"; categorising adds branches that don't help a future
  reader and risk drift. Alternative rejected: "only freeform
  replies, not status acks" — the user's framing was that
  responses to comments are free-form anyway; ack-vs-reply is
  not a useful distinction.
- **No per-skill enumeration of AutoSDE / `/code-review`.**
  The kdevkit rule is "the agent operating the CR posts with
  the prefix"; skills the agent invokes inherit by being
  invoked by an agent already under the rule. Rationale: the
  convention travels with the actor, not the tool.
  Alternative rejected: cross-skill follow-up backlog item to
  modify AutoSDE / `/code-review` separately — adds churn
  without changing the user-visible CR timeline.
- **Forward-only migration.** Rationale: backfilling existing
  CR comments costs API churn and is arguably revisionist;
  earlier comments read by context. Alternative rejected:
  sweep-and-edit existing comments to add the prefix.
- **Test fixture deferred to D (kdevkit-compaction).**
  Rationale: D plans three new judge fixtures for the
  steady-state / setup-drift / fresh-feature paths; bundling
  the comment-prefix fixture there means one functional-test
  surface change instead of two. Recorded as a follow-through
  item in D's spec. Alternative rejected: ship a comment-prefix
  judge fixture on this feature's branch — duplicate test
  surface change, costs API credits twice.
- **Source-of-truth in §7, cross-ref from §8.** Rationale:
  §7 is where the agent enters the CR-comment cycle; §8
  inherits by reusing the same cycle. §9 reserved for
  always-on hygiene. Alternative rejected: lift the rule to
  §9 with cross-refs from §7 and §8 — would dilute §9's
  always-on shape.
