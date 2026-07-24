# Feature: notes-and-behavioral-verification

## Git Setup

- Branch: `feat/notes-behavioral-verification`
- Base: `main` (3e94fb0)

## Feature Brief

Two capabilities land together. First, the `notes` skill stops
auto-minting empty `topics/<topic>.md` stubs: a `[[topic]]`
reference to a nonexistent topic page now leaves the wikilink
dangling (Obsidian renders unresolved links fine) instead of
writing a substance-less `kind: topic` + `# <topic>` file. A
topic page is created only when it accrues real substance.

Second, the `notes` skill's verification moves from *recitation*
(the agent *says* the right thing) to *behavioral* (the agent
*writes* the right files into a seeded vault), across all three
coding agents. The stub-fix is the first behavioral fixture —
its correct behavior is an **absent file**, impossible to fake
with a recitation probe — and becomes the template the remaining
notes smokes are reshaped toward.

Out of scope, deferred back to backlog: behavioral verification
for `writing-style` (its output is prose, not an artefact — a
different design problem).

## Requirements

The experience layer here has two audiences: the human running
`add note …` (whose observable surface is the files that land in
the vault), and the person running `just resources::verify-skills`
(whose observable surface is which fixtures pass).

### Notes behavior (what lands in the vault)

- Writing a note that references a **novel** `[[topic]]` (one with
  no existing `topics/<topic>.md`) creates the note itself with
  correct frontmatter and body, and does **not** create
  `topics/<topic>.md`. The wikilink is left dangling.
- An **existing** `topics/<topic>.md` is never overwritten or
  touched by a mere reference to it.
- A topic page is created only when the user has real substance
  to put in it — a definition, links, or accumulated notes, i.e.
  more than frontmatter + a bare heading. (Prose guidance, not a
  mechanical byte count.)
- Every other notes behavior (classification, vault selection,
  reminder shape, git flow) is unchanged.

### Verification (what a verify run observes)

- Running the notes fixtures against `claude`, `kiro`, and
  `codex` exercises the skill's *behavior on a seeded vault*, not
  just its recitation, wherever the behavior produces an
  inspectable artefact.
- The stub-fix fixture passes only when both hold: the note
  landed correctly **and** the topic stub is absent — a
  do-nothing agent fails it.
- Fixtures that assert on a genuinely non-artefact behavior (e.g.
  the vault-resolution *error* message when a named vault is
  unset) may stay judge/substring; the reshape is
  behavioral-where-an-artefact-exists, not behavioral-everywhere.

## Test Strategy

Per `project.md`'s two layers:

- **`just test` (unit, load-bearing, §8 Test Gate).** The skill
  change is pure Markdown content; the content validator still
  passes (frontmatter intact). No new Rust unit test — there's no
  new code path in `build-tool`. This gate must stay green.
- **`just resources::verify-skills` (functional, user-driven,
  credit-costing).** This is where the behavioral fixtures live.
  Per `project.md` "Functional tests are user-driven," the agent
  **prepares** the fixtures and names the exact command; the user
  runs it. Do not run it in this session.

Success criteria, mapped to the functional layer:

1. **Stub-fix fixture (new, behavioral, tri-tool).** Seed a
   minimal Obsidian-shaped vault; drive the agent to add a note
   referencing a novel `[[topic]]`; assert the note file exists
   with correct frontmatter **and** `topics/<topic>.md` is
   absent. `tools: claude,kiro,codex`.
2. **Reshaped notes smokes.** The existing recitation smokes are
   brought to behavioral where an artefact exists, and — at
   minimum — corrected where they assert the *old* stub behavior
   (`notes-add-note` currently calls the auto-stub the correct
   outcome; that narrative is now wrong and must flip).

The stub-fix fixture is worth building (confirmed): we are
inverting a skill rule, an existing fixture currently encodes the
old rule, and the behavior is cleanly artefact-observable with a
presence+absence pair that a no-op can't satisfy.

## Design

**Rationale.** The behavioral fixture shape already exists — the
kdevkit smokes (`kdevkit-planning`, `kdevkit-closure`, …) use the
harness's `--- setup --- / --- assert ---` blocks: `resources/tests/run`
seeds a fresh `mktemp -d`, runs the agent there with write access,
then runs the assert shell against the resulting tree (a non-zero
exit fails). No harness change is needed — this feature *uses* the
existing behavioral machinery, it doesn't extend it. The design
work is (a) the one-line skill-rule inversion and (b) the seed +
assert shell for each fixture.

**Reach for what exists.** The seed is plain `mkdir`/`cat`
heredocs (as every existing behavioral smoke does); the assert is
POSIX `test`/`grep` (matching `kdevkit-closure`'s style). No new
dependency, no new fixture format.

### Skill rule change

`resources/content/skills/notes/SKILL.md`, `## Linking`
(~line 194). Replace the "create a one-line stub" rule and its
fenced stub template with the no-auto-stub rule: leave the link
dangling; create `topics/<topic>.md` only when it accrues real
substance (definition, links, accumulated notes — more than
frontmatter + heading); never overwrite an existing topic page.
The `notes-add-note` judge narrative (a sibling fixture)
references the old behavior and flips in lockstep.

### Fixtures

- **New `notes-topic-no-stub.smoke`** (behavioral, tri-tool). Seed
  a vault with `NOTES_VAULT` pointed at the scratch dir and at
  least one pre-existing topic page (to also assert "existing
  topic untouched"). Drive `add note for: insight: <body>
  referencing [[<novel-topic>]]`. Assert: the insight file exists
  under `insights/` with `kind: insight`; `topics/<novel-topic>.md`
  does **not** exist; the pre-existing topic page is byte-identical.
- **`notes-add-note.smoke`** — flip the judge narrative's
  topic-stub clause from "creates a one-line stub" to "leaves the
  `[[taxes]]` link dangling; does NOT create `topics/taxes.md`."
  Everything else (classification, filename, frontmatter) stands.
- **`notes.smoke` / `notes-vault-selector.smoke` /
  `notes-git-commit.smoke`** — assessed for behavioral conversion.
  `notes.smoke` (reminder → which dir) has a clean artefact and
  converts. `notes-vault-selector` and `notes-git-commit` carry
  error-path and git-flow assertions that are partly non-artefact;
  convert the artefact-observable parts, keep judge/substring for
  the rest. Decided per-fixture during the slice, recorded in the
  Decision Log.

### Open questions (from the backlog, resolved at design time)

- **Minimal seedable vault** → `mkdir -p insights topics` + a
  `NOTES_VAULT` export in the prompt/setup + one seed topic page.
  No bare-repo remote needed for the stub fixture (git flow is a
  separate fixture's concern).
- **Definition of "substance"** → prose guidance: more than
  frontmatter + heading (a definition, links, or accumulated
  notes). Not a byte count.
- **Migration of ~180 existing stubs** → out of scope. That's a
  bulk mutation of the user's *personal vault* (not repo content);
  this feature changes go-forward behavior only. Left as a
  backlog note.
- **Retrieval impact** → the Dataview/ripgrep "notes by topic"
  queries key on `[[X]]` outlinks + frontmatter `topics:`, not
  file existence, so they work with the topic file absent. No
  change needed.

## Implementation Plan

- [ ] Slice 1 — invert the `## Linking` rule in `notes/SKILL.md`
      (no auto-stub; substance-gated creation; never overwrite),
      and write `notes-topic-no-stub.smoke` (behavioral, tri-tool:
      note lands + stub absent + existing topic untouched). These
      land in one commit — the behavior change and its test
      together.
- [ ] Slice 2 — flip the `notes-add-note.smoke` judge narrative's
      stub clause to match the new rule (dangling link, no stub).
- [ ] Slice 3 — reshape the remaining notes smokes toward
      behavioral where an artefact exists (`notes.smoke` converts;
      `notes-vault-selector` / `notes-git-commit` convert the
      artefact-observable parts, keep judge/substring for the
      error/flow assertions). Decide per fixture; log the calls.

- *Risk note:* the functional layer is user-driven and
  credit-costing — this session cannot green the behavioral
  fixtures itself. The unit gate (`just test`) plus a careful read
  of each fixture against the harness is the in-session evidence;
  the actual tri-tool run is handed to the user.
- *Risk note:* `codex exec` closes stdin and `--skip-git-repo-check`;
  `kiro-cli` and `claude` differ in flags. The seed must not assume
  a git tree (the stub fixture doesn't need one) and must export
  `NOTES_VAULT` in a way all three inherit — verify against the
  three `tool_invoke` branches in `resources/tests/run`.

## Session Log

<!-- append: date · what was done · decisions made -->

- 2026-07-24 · Promoted backlog item → feature; branch
  `feat/notes-behavioral-verification`. Scope set with user: Slice 1
  (rule change + behavioral fixture) **plus** reshaping the existing
  notes recitation smokes; `writing-style` verification deferred
  back to backlog. Substance line = prose guidance. Stub fixture
  confirmed worth building → tri-tool.

## Decision Log

<!-- append: decision · rationale · alternatives rejected -->

- **Use the existing behavioral harness, add no new fixture
  format.** `resources/tests/run` already seeds a scratch dir and
  runs setup/assert blocks (the kdevkit smokes). Considered
  extending the harness for vault-specific seeding; rejected — the
  `NOTES_VAULT` env export + `mkdir` seed fits the current shape.
- **Substance = prose guidance, not a byte count.** A mechanical
  "frontmatter + heading + ≥1 sentence/link" bright line was
  considered; rejected as over-specification that the agent would
  litigate on every write. The skill states the *intent* (a page
  exists because someone had something to say).
- **`writing-style` deferred.** Its output is *how prose reads*,
  which resists a shell assert; it's a distinct design question
  (observable proxy vs. staying judge-mode) and doesn't belong in
  the notes slice. Returns to backlog at closure.
