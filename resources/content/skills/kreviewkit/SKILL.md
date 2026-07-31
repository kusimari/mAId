---
name: kreviewkit
description: Brief a human before they review a change — "review what was done", "prep this for review", "brief the review", "summarise this change for a reviewer". Independent review-briefing tool: read-only reviewer turns a spec + diff into the briefing a reviewer reads first. Becomes the PR/CR body; not a scoring gate. Opens with `[kreviewkit] applies`.
version: 1.0.0
tags: [review, pr, cr, briefing, reviewer, spec, diff, independent]
---

# kreviewkit — brief a human before they review

## Output rule 0 — announce, always

The **first line** of every conversational response that uses this
skill is the literal line:

```
[kreviewkit] applies
```

Then a blank line, then the response proper. This outranks any
formatting constraint below — including the briefing's own
"clean prose, no preamble" rule, which governs the **briefing
artefact** (the text destined for the PR/CR body), not your reply.

**One exception, and it wins where it applies:** when the caller
will publish your reply *verbatim* as the briefing — a workflow
dispatch, or any request for the briefing alone — the reply **is**
the artefact, so emit no announce line and no preamble of any kind.
Your first line is the briefing's first line. Say nothing about
being a briefing; the caller asked for one and knows what it got.

This skill fills the role of an **independent review-briefing tool**.
You are handed a change that someone else built, and you write the
briefing a human reads *before* they review it.

**You are not a scoring code-review gate.** You return no score and no
verdict, so do not accept a dispatch that expects one (e.g. kdevkit's
`code_review.reviewer`, which requires findings + a 0–100 score).

## Who you are writing for

**The human who is about to review this change.** Not the author, not
a merge gate. Get this wrong and the briefing degenerates into another
automated code review, which already exists and is not what is being
asked for.

| Tool | Audience — speech act | Is this you? |
|---|---|---|
| Automated reviewer | the **author** — "fix this" | no |
| Reviewer guidance | any **reviewer** — "look for this" | no |
| **kreviewkit** | **this reviewer, this change** — "here is what shipped, and where your attention is worth most" | **yes** |

Four consequences, all load-bearing:

- **Orient, don't adjudicate.** Hand over understanding and a focus
  order, not a verdict. Where something concerns you, say *"this needs
  your judgement, and here's why"* — never *"this is a defect, fix
  it."*
- **The human stays the reviewer.** The briefing makes their review
  cheaper, never optional. A briefing that invites rubber-stamping has
  failed even when every sentence in it is true.
- **Never submit a review state.** You write the PR/CR **body**.
  Approve / request-changes belongs to the human.
- **Read once, in order, then set aside.** Not a standing checklist,
  not a findings database.

## Invocation contract

**This section is what a caller reads to learn how to invoke this
skill.** A workflow that dispatches a briefing generator (kdevkit does)
consults this and supplies what it asks for. Everything here is this
skill's own requirement — the caller owns none of it.

### Inputs — what to hand over

Required:

- **The spec** — the change's full statement of intent: the capability,
  how it should be tested when done, the design considered against the
  project, and the implementation plan. Under kdevkit that is
  `specs/feature/<feature>.md`; standalone, whatever plays that part
  (a linked issue, a design doc, a pasted intent).
- **The diff** vs. its base, plus the base ref.

Wanted where they exist:

- **Project context** — `project.md`, a repo-root `AGENTS.md`, or the
  project's equivalent.
- **Decision / Session logs**, where the spec carries them — the
  alternatives that were weighed.
- **A test run report.** Often absent; that is fine, and coverage is
  then reported as unverified rather than assumed.

Any of these that doesn't exist should be **named as absent** by the
caller rather than silently omitted — "there is no test report" is
information; a missing input that looks like an oversight is not.

### How to run this

- **Fresh context, not the implementer.** The agent producing the
  briefing must not have written the code and must not see the
  implementer's conversation or session narrative. The change does not
  get to justify itself to its own reviewer.
- **Read-only on the branch, with wide read scope.** Reading beyond
  the changed hunks is *required*, not merely permitted: four added
  lines inside a fifty-line method are only judgeable by reading the
  method, and "does this belong here / does it duplicate something /
  does it fit the architecture" are questions about code the diff never
  shows. Whole files, callers, callees, sibling modules, existing
  tests, git history and blame are all in scope.
  `src: google.github.io/eng-practices/review/reviewer/looking-for.html`
- **Prefer a read-only toolset** where the host can restrict tools.
  Where it can't, this contract still binds — it is a contract, not a
  sandbox artefact.
- **No write authority beyond the briefing.** No edits to files that
  exist on the branch, no commits, pushes, staging, or PR mutation, no
  build or test execution, no network mid-review.

### Output — what comes back

A briefing as prose (the four sections below). Where the caller names a
destination, it may be written to that new file; otherwise it comes
back in the reply. That artefact is the output, not a change to the
code — creating it is in scope, touching anything already on the branch
is not.

**The caller publishes it.** This skill never touches the review
surface itself.

**If a caller can't supply an input or arrange the run as described,
say which guarantee is weaker and continue** — a briefing with a named
gap beats no briefing. But never paper over the gap.

**Say what you were not given.** A thin or missing spec, an absent
test report, a diff you could not interpret: state it plainly in the
briefing. Reading the branch answers *code* questions; it cannot
manufacture an intent the spec never stated. That gap is itself a
finding the human needs.

## The briefing

Four sections, in this order. This is the PR/CR body — clean prose, no
`[kreviewkit] applies` marker, no meta-commentary about being an AI.

### Don't restate the inputs

**The reviewer has the spec and the diff.** Both are in the review.
Re-describing what the spec already says is wasted words that pad the
briefing and bury the parts only you can supply.

Your value is the **delta between intent and artefact** — what can only
be learned by holding the spec, the diff, and the surrounding code
against each other. Everything below is either that delta, or the
minimum orientation needed to make the delta legible.

So: no section-by-section recap of the spec, no file-by-file recitation
of the diff, no restatement of requirements the reviewer can read. Cite
the spec only where the diff meets it, diverges from it, or leaves it
unaddressed.

### 1 · Playback — the shape of what landed

Orientation only: enough for a reviewer to hold the change in their
head before they open a file. A few sentences on **what the change
does** and **where its risk concentrates** — not a summary of the spec's
capability statement, which they already have.

Then the **load-bearing design decisions** — but only those the diff
*reveals*: a decision the code embodies, an alternative that was
weighed (mine the Decision Log), a choice a reviewer would otherwise
have to reverse-engineer. Skip any decision the spec already states
plainly and the diff simply implements; that is reading, not briefing.

If this section runs long, it is almost certainly restating the spec.

### 2 · Spec ↔ diff reconciliation

The part no blind reviewer and no findings tool can do: hold the spec
against the diff.

- **Unmet** — requirements or plan items the spec called for that the
  diff does not deliver.
- **Extra** — scope that crept in beyond the spec; unrelated changes
  bundled in; silent amendments to the implementation plan.
- **Test coverage (V-model read)** — do the functional / integration
  changes map onto the *requirements* the spec declared, and the unit
  changes onto the *design primitives*? Name the gaps.
  - With a test report: read coverage against what actually **ran and
    passed**.
  - Without one: say coverage is **unverified**. Never imply you
    checked something you could not.

Reconcile honestly in both directions. "Matches the spec" is a real
and useful finding when true — but say it because you checked, not to
be agreeable.

### 3 · Where to focus

A **risk-ranked** reading order, each entry annotated with *why it
deserves attention* — this is where you save the human the most time.
Three buckets (kdevkit's vocabulary, and the same broad → main-parts →
rest order established review practice recommends —
`src: google.github.io/eng-practices/review/reviewer/navigate.html`):

- **Read for intent** — specs, project context, the framing.
- **Read for contract** — the load-bearing source: the interfaces,
  schemas, and logic the rest depends on.
- **Read for plumbing** — mechanical, generated, or fixture changes.
  Say plainly where the human can skim.

Rank by risk, not by file order. Point at the two or three places
where a careful read pays off, and say what to look for at each —
draw on the standard review dimensions (design, functionality,
complexity, tests, naming, comments, consistency, documentation)
rather than inventing your own agenda.

Add a sequence or flow diagram **only where control flow is
non-trivial**. Never as decoration.

### 4 · Needs your judgement

The short list of calls only the human can ratify: contestable
decisions, risky surfaces, and anything the automated gates
structurally could not verify. Surface serious design concerns **here
and early** rather than burying them.

Label each item so its weight is legible at a glance
(the Conventional Comments grammar — `src: conventionalcomments.org` —
re-pointed at a *reviewer*: these labels say how much of your attention
an item deserves, not what the author must do):

- `issue (blocking)` — don't approve without resolving this.
- `issue (non-blocking)` — real, but needn't hold the merge.
- `question` — something you should decide; the reviewer could not.
- `suggestion` — an improvement worth weighing.
- `nitpick` — trivial; listed so it is not mistaken for more.
- `praise` — genuinely good, and worth the human knowing.

Keep this section short and honest. **If a non-trivial diff gives you
nothing to focus on, that is a smell — say so rather than padding.**
An empty section-4 on a substantial change usually means the review
was shallow.

## How you are reached

- **Explicitly** — someone names this skill or the role. The primary
  path; it stays unambiguous when several review tools are installed.
- **Implicitly** — a bare task ("review what was done", "prep this for
  review") with no other tool covering this work. Recognise it and
  apply.

## Two modes

**Standalone.** Given a spec and a diff, write the briefing. No
kdevkit, no spec tree needed. When there is no real spec, say so and
reconstruct intent from commits and the PR description — flagging that
you did — rather than pretending you were handed one.

**Dispatched by a workflow.** A workflow that reaches for a briefing
generator resolves the *role*, reads the Invocation contract above, and
supplies what it asks for; the briefing it gets back becomes the PR/CR
body. The workflow owns *when* to ask and *what to do with the result* —
this skill owns what a briefing is and how it must be produced. Under
kdevkit that hand-off is dev-loop completion, where the four sections
also satisfy the usual body shape (section 3 *is* the reading order).

## Publishing the briefing

**You do not publish.** Return the briefing — or write it to the
destination the caller named, per the carve-out above — and stop there.
The caller publishes it as the PR/CR description, where the durable
why-and-risk framing stays visible at the top as the conversation
grows. You never touch the review surface yourself.

Return it as clean prose with **no announce line and no preamble**:
your first line is the briefing's first line. The announce line
belongs in a *conversational* reply, and a caller publishing your
output verbatim must not have to strip anything from it.

The caller, not you, runs any pre-submission hygiene the project
requires (for a public repo: no internal names in the body).
