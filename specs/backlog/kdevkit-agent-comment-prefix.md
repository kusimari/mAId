# Backlog: kdevkit-agent-comment-prefix

## What

Add an `[agent]:` / `[human]:` comment-prefix convention to the
kdevkit skill so that when a coding agent operates the CR/PR
tooling on the human's behalf, the audit trail in the CR is
disambiguable even though both parties post under the same
identity.

The convention applies to **anything the agent writes into the
CR/PR surface that isn't the diff itself**:

- Reply comments authored by the agent in response to reviewer
  feedback.
- Description / summary edits the agent makes after the initial
  submission.
- Resolved-thread acknowledgements.

Shape:

- Agent-authored: every comment body starts with a literal
  `[agent]:` prefix on the first line, followed by the actual
  content. No prefix on the diff or commit messages — those are
  already attributable via the `feat(<scope>):` etc. subject
  line, and adding noise there breaks Conventional Commits
  parsers.
- Human-authored: prefix optional. The default is **the human
  does not prefix** — bare comments are read as human. If the
  human wants to be explicit (e.g. to mark a comment as a steer
  rather than a review note), they MAY use `[human]:`, but the
  skill does not require it.

## Why

When kdevkit runs with a coding agent as the builder and the
human as the instructor + reviewer, the CR tool becomes kludgy:

1. **The agent submits the CR as the human.** At Amazon, CRUX is
   kerberos-bound — there is no separate bot identity for an
   individual builder. GitHub has the same shape for non-org
   repos: the agent operates the human's account.
2. **The human comments on the CR as themselves.** Same
   identity.
3. **The agent replies to those comments — also as the human.**
4. CRUX/GitHub thread the comments by author, not by content. So
   the human's review note and the agent's reply land flat in
   the timeline, indistinguishable, no inline pairing.
5. "Ship it" has no closure path on the agent side — submitter
   and reviewer are the same identity, so the agent can't mark
   the CR shipped on behalf of the human. The human has to do
   it.

Two paths exist. Per the discussion that produced this backlog
item, we are taking **path B**:

- **Path A — stop using the CR as the dev-loop medium.** Iterate
  locally on the feature branch via `git diff` / AutoSDE /
  `/code-review`; submit the CR only once the human is already
  happy. Cleaner for downstream human reviewers, but loses the
  threaded conversation as a record.
- **Path B (this backlog item) — keep using the CR, accept the
  kludge, fix it minimally with a comment-prefix convention.**
  Threading is still broken (the tool can't fix that), but at
  least each comment self-identifies as agent or human, which is
  enough for a future reader (or the agent itself in a later
  session) to reconstruct the dev conversation.

The realization that drove this: **the dev-loop conversation is
load-bearing context** — closure commits and the next session's
prompt both reference "the comment thread on the CR" — and
discarding the threading means losing that context. The prefix
gives us back a 90%-good substitute (sequential, prefix-tagged,
grep-able) without forcing a workflow change on either party.

## How it composes with existing kdevkit sections

- **§7 Agent-dev Review Gate.** The gate already cycles
  CR-comment → agent-fix → re-push. Add a sub-rule: every comment
  the agent posts during this cycle prefixes its body with
  `[agent]:`. Applies whether the comment is a reply to a human
  thread, a "fixed in <sha>" note, or an acknowledgement.
- **§8 Closure Review Gate.** Same rule extends to the closure
  cycle. The agent's "ready for ship-it" comment, if it posts
  one, is `[agent]: ready for ship-it`.
- **§4 Code-review setup prompt.** The setup prompt that runs
  when a project first declares `kdevkit.code_review:` should
  surface the convention as part of the explainer — "this skill
  prefixes agent-authored CR comments with `[agent]:`; you don't
  need to prefix yours."
- **Conventional Commits** — explicitly NOT touched. Commit
  subjects stay `feat(<scope>):` / `fix(<scope>):` / etc., with
  no prefix. The convention applies only to CR/PR comment
  bodies.

## Where the convention has to land in the dual-target shape

kdevkit ships into two surfaces with different review tools:

1. **Internal (Amazon) projects** — CRUX (`cr` CLI,
   code.amazon.com/reviews/). Comments via `cr reply` / web UI.
   Bot-account workaround does not exist for individual
   builders. The prefix is the only mechanism.
2. **External (public GitHub) projects** — `gh pr` CLI, GitHub
   PR comments. The same convention applies; GitHub *does*
   support bot accounts via personal access tokens, but
   requiring users to set one up is friction we don't want
   default kdevkit to impose. So default = same prefix
   convention; users who want a bot identity can wire one up
   themselves and the prefix becomes redundant for them (still
   harmless).

This means the SKILL.md text has to be tool-agnostic: phrase the
rule in terms of "CR/PR comment bodies", not "CRUX" or "GitHub".
Any tool-specific examples (e.g. `cr reply -m '[agent]: ...'`
vs. `gh pr comment --body '[agent]: ...'`) go in §10 Reference
or as inline examples next to the rule.

## How to ship it (likely shape of the implementing feature)

This is a small change to `sources/skills/kdevkit/SKILL.md`:

1. **§7 Agent-dev Review Gate** — add the prefix rule as a
   numbered bullet. Two examples (one CRUX command, one `gh`
   command) inline.
2. **§8 Closure Review Gate** — one-line cross-reference back to
   §7's rule, since the closure cycle reuses the same comment
   discipline.
3. **§4 Code-review setup prompt** — add a sentence to the
   first-time-declaring-`code_review:` blurb so the human sees
   the convention up front.
4. **§10 Reference (or wherever the tool-CLI cheatsheet lives)**
   — two example commands (`cr` and `gh`) with the prefix.
5. **Skill version bump** — patch-level (additive convention,
   no breaking shape change). Probably v2.X+1.

Estimated diff size: 30-60 lines of SKILL.md, no code.

## Open questions

1. **Should the prefix be `[agent]:` or `[agent kdevkit]:` or
   `[agent claude]:`?** Lean toward bare `[agent]:` — the value
   is "not human", not "which agent". A human reading the CR
   six months later cares whether the comment came from a model
   or from them; they don't usually care which model.

2. **Do we prefix the CR description / summary edits the agent
   makes?** A description rewrite isn't a "comment" in the same
   sense — it has no thread to disambiguate. Probably the
   description should NOT be prefixed; the agent's contribution
   to the description is implicit (the description is generated
   from the commit log, which the agent authored). Worth a
   single line in the spec to make this explicit.

3. **What about resolved-thread "done" / "fixed" auto-replies?**
   Some agents auto-acknowledge resolved threads. These should
   absolutely be prefixed — they're the highest-volume comment
   type and the most likely to be confused with a human "lgtm".

4. **Interaction with AutoSDE / `/code-review` integrations.**
   When the agent uses AutoSDE to iterate on the CR, does
   AutoSDE post its own comments? If so, those should also be
   `[agent]:`-prefixed. This may need a small change to the
   AutoSDE skill or a wrapper rule.

5. **Migration for in-flight CRs.** Existing CRs at the time
   the convention lands won't retroactively have prefixes.
   Don't try to backfill — the convention applies forward only.
   If the SKILL.md change ships mid-feature, the agent on the
   next loop in that feature's CR starts prefixing; earlier
   comments in the same CR stay un-prefixed and are read by
   their context.

## Trigger to promote

- Any time we hit a CR where the human-vs-agent timeline becomes
  unrecoverable (i.e. someone has to ask "which one of us
  posted this?"). One real recurrence is enough — the convention
  is cheap to ship and the recurrence is the proof we need it.
- A second harness arrives that has different threading
  semantics (e.g. an IDE-integrated review pane) — at that
  point we want the convention already in place so the new
  harness inherits a consistent rule.
