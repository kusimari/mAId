## Feature planning

Trigger: a populated spec lacks the user's review (§3
spec-already-drafted rule), or `<feature>` is being started
fresh.

### Four short interviews

**Ground first.** Before opening interview 1, read
`project.md`, scan related feature specs in
`$SPEC_ROOT/feature/`, survey the corners of the
codebase the feature touches, and survey what the language /
ecosystem already offers for the problem (see "Reach for what
exists" below). The interview is calibrated
to what's there now, not the user's recollection. Findings
worth keeping land in the Session Log as work progresses;
the grounding step does not introduce a new artefact (no
`research.md`).

When entering a feature with no spec on disk (start mode), run
four short interviews in fixed order — Requirements → Test
Strategy → Design → Implementation Plan. Tests sit immediately
after requirements so success criteria are declared before the
design converges; the dev loop (§7) then has a verifiable
target, not a sketch to validate after the fact. Skip topics
existing project context already answers.

**Inline-Read `interviews.md`** for the interview-by-interview
prompt shape and the feature file template body. After the
four interviews, write the feature spec, then return here for
the Plan-commit rule.

**Answer the interviews yourself from the grounding, and write
the file.** The interviews are *your* checklist for what the spec
must cover, not a questionnaire to hand the user. Draft each
answer from `project.md`, the backlog item, and the code you just
read, then **write `$SPEC_ROOT/feature/<feature>.md` before you
ask the user anything.** The spec on disk is what the user reacts
to — a list of questions is not a reviewable artefact, and neither
is a set of interview answers in chat.

Ask only what you genuinely cannot infer, and ask it *in the
spec*: record the open question in the Session Log or inline, and
carry on. A single blocking question is warranted only when
proceeding either way would waste the work (§5's ambiguity rule);
"what should the flag be called" is not that. If the user's
request already says to do the file work, treat any urge to
open with clarifying questions as the ordering mistake the
Plan-commit rule warns about.

### Requirements smell test (always-on)

The spec's three top sections pair with the project's test
layers in V-model fashion:

- **Feature Brief** = the *capability* (what the user can
  now do).
- **Requirements** = the *experience* (what the user
  touches and observes) — verified by **functional /
  integration tests**.
- **Design** = *how it's built* (schemas, plumbing,
  libraries, project conventions) — verified by **unit
  tests**.

Functional/integration tests are pinned to Requirements so
they assert in user-observable terms; unit tests are pinned
to Design so they assert design primitives. That pairing is
the *why* behind the smell test below — a Requirements
bullet that names internals can't be verified by a test
phrased in user-observable terms, so it's in the wrong
section.

Before writing each Requirements bullet, check it against
the smell test; move violators to Design.

A Requirements bullet belongs in Design if it names any of:

- A library / framework name, or any third-party tool the
  user doesn't invoke directly.
- A file path / config key / data shape the user doesn't
  see in the surface they interact with.
- A function / class / trait / type / schema name from the
  implementation.
- An internal subcommand, hook event name, or protocol verb
  that's not part of the user-facing surface.

The discipline generalises across feature types — a CLI
feature (experience = flags and output), an app feature
(experience = screens and visible state), a skill change
(experience = the cues the agent recognises and the
artefacts it produces), a service endpoint (experience =
request shape and response).

The discipline is guidance, not rigid form. `interviews.md`'s
template and prompts are best-practice scaffolding; the
spec's exact section layout adapts to the feature. The
strictness lives in the **gates** — Planning Review (§6),
Agent-dev Review (§7, loops freely with Code Review), and
Closure Review (§8) — not in the heading shape.

### Reach for what exists (design-time, always-on)

A design move, not a coding-style preference, and the mirror
of the smell test above: the smell test keeps library names
*out* of Requirements; this puts the *right* library *into*
Design. Before deciding *how* a non-trivial piece of work is
built, **survey what the language / ecosystem already offers
and name the well-known library or idiom that already does
the job** — "load YAML with the established parser into a
typed struct," not a hand-rolled frontmatter parser; a known
filesystem-walk crate, not a hand `read_dir` recursion.

The justification is **inherited expertise** — a battle-tested
dependency encodes vetted edge cases and community practice
the agent would otherwise re-derive badly — **not** DRY.
"Shorter code" is the wrong reason; "someone already solved
this correctly" is the right one.

Guard: **well-known *and* earns its weight.** Don't pull a
new or heavy dependency for a trivial job a few honest lines
or an already-present import handle; weigh the dependency
against the hand-roll and say so when the hand-roll wins (a
lightweight direct call can beat a heavyweight dep). The rule
is language-agnostic — "the idiom *this* language / codebase
speaks," never a fixed per-language library list.

For load-bearing design choices, record the alternative
weighed in the Decision Log ("considered X; chose Y
because …"). Recommended, not mandatory for every helper.

### Initiative-stream auto-link

When the feature being started is a stream of an active
initiative (the initiative's Streams list names this feature's
branch or feature-spec basename — see §10), §6 Planning
auto-populates the `Part of initiative: [[<name>]]` line in
the feature spec, immediately after `## Feature Brief`. No
prompt; the link populates silently when the match is
unambiguous. If two or more active initiatives reference the
same name, ask one line to disambiguate.

### Plan-commit rule

The populated spec must reach the user as a reviewable artefact
before any code work begins. Order matters:

1. Finish the four interviews and write
   `$SPEC_ROOT/feature/<feature>.md`.
2. Confirm readiness with the user; iterate on the spec if
   needed.
3. **Commit** the spec as `plan(<feature>): initial spec`.
4. **Push** the feature branch.
5. **Open the Planning Review Gate** (PR/CR with the
   phase-specific body shape — see below).
6. **Then** wait for the planning → dev cue (§5).

The cue gates the *move* to dev — not the planning commit. The
commit + push + review must happen first so the user has
something concrete to react to. Reversing this order (waiting
for the cue before committing) is the most common ordering
mistake — a planning agent can read "confirm readiness" as the
exit-from-planning cue and stop there. It isn't. Steps 3–5 are
the artefact; step 6 is the gate after the artefact exists.

This rule is the single source of truth for both planning entry
paths — fresh-from-interviews and spec-on-disk (§3); §3 cites it
rather than duplicating.

Skip steps 3–6 if `planning_phase: false` (§2) — spec edits ride
with the first dev commit.

### Planning Review Gate

Fires after the `plan(<feature>):` push. Apply §9 Review
Gates. Phase-specific body content: **Spec summary**
(R / T / D / I one-liners) + **Open questions**.

