# Feature: fixtures-discovery-vs-content-split

## Git Setup

- Branch: `feat/fixtures-discovery-vs-content-split`
- Base: `main` (7f49d20)

## Feature Brief

Give the skill fixture suite a **named, self-evident test taxonomy**,
and split it along the axis it was already straddling: does a skill
*say* the right thing, versus does it *fire at all*.

Today all 13 fixtures open by handing the agent the skill name and its
file path, so all 13 answer only the first question — a skill whose
`description:` is too weak to auto-trigger passes the entire suite,
because the fixture does the triggering on its behalf. And nothing in
the repo tells a future author which tests a new skill is expected to
have: the classes exist only as an unwritten pattern, so each new
skill gets whatever fixtures its author happened to think of.

This feature names four purpose classes — **activation**,
**contract**, **behavior**, **discovery** — and makes them legible in
three places: documented in `project.md`, declared per-fixture as a
`class:` field, and selectable as a `just` verb. The payoff is a
**checklist for creating a skill** ("which classes does my skill
need?") and a **diagnostic pair** at runtime: when a skill's contract
fixture passes but its discovery fixture fails, the fault is localised
to triggering rather than prose, with no bisection.

## Requirements

The audience is two-fold: the person running
`just resources::verify-skills`, whose observable surface is which
fixtures pass and what a failure tells them; and the future author (a
human or an agent) adding a skill, whose observable surface is the
documented taxonomy and the `class:` field they must fill in.

### Test-suite experience

- Running the suite exercises **both** questions per skill: that the
  skill describes its behavior correctly, and that a user who merely
  states a task gets that behavior without naming the skill.
- **Discovery** fixtures state the task only. The prompt names no
  skill and no file path, so the agent must select the skill from what
  its harness already surfaces. A prompt mentioning the skill's name,
  its marker text, or a `SKILL.md` path is not a discovery fixture.
- **Activation, contract, and behavior** fixtures keep naming the
  skill and path. When one fails, the reader can conclude the skill's
  content is at fault without wondering whether it simply failed to
  load.
- Every skill has discovery coverage — including the two whose
  behavior produces no file (`writing-style`, `browser`).
- For a skill whose behavior lands as a **filesystem artefact**, the
  artefact is the proof of discovery: a no-op agent leaves nothing
  behind and fails. No marker assertion is used.
- For a skill whose behavior is **prose in the reply**, the proof of
  discovery is the skill's own self-announce marker appearing without
  the prompt ever asking for it. An agent that answers competently
  from general ability, never having loaded the skill, fails.
- A failing discovery fixture reads as a real signal about trigger
  strength, not as a test to loosen.

### Authoring experience

- Each fixture declares which class it belongs to, readable at the top
  of the file without inferring it from which assertion keys are
  present.
- A fixture that omits its class, or names one that doesn't exist,
  fails loudly as malformed rather than running under a silent
  default.
- The runner and the `just` surface can scope a run to one class, so a
  reader can ask "show me every discovery test" and get an answer by
  running it, not by grepping.
- `project.md` names the four classes, states the question each one
  answers, and says which classes a new skill is expected to carry —
  so a future author has a checklist rather than a pattern to reverse-
  engineer.
- No skill content changes. `SKILL.md` files are untouched; this
  feature moves fixtures, the runner, the Justfile, and docs.

## The four test classes

The taxonomy is the durable artefact of this feature, so it is stated
here in full and mirrored into `project.md`.

**Class is orthogonal to verification style.** The runner already
documents *how* a fixture checks — substring, semantic/judge,
behavioral. A class says *what question the fixture answers*. Two
fixtures can share a mechanism and answer different questions (a judge
call can verify a contract recitation or a prose rewrite); one class
can be served by different mechanisms depending on whether the skill
leaves an artefact. Conflating the two axes is how the suite drifted
into 13 fixtures that all answered the same question.

| Class | Question it answers | Prompt names the skill? | Typical style |
|---|---|---|---|
| `activation` | Does the skill load and announce itself at all? | yes | substring |
| `contract` | Does the skill correctly state its own rules — guardrails, refusals, absence paths, the shape of what it *would* do? | yes | semantic (judge) |
| `behavior` | With the skill loaded, does the agent actually *do* the thing — produce the artefact or the styled output? | yes | behavioral where an artefact exists; judge for prose |
| `discovery` | Does the skill fire from a bare task, with no name or path supplied? | **no** | behavioral (artefact) or substr + judge (prose) |

- **`activation`** is the cheapest possible smoke: the skill is
  reachable, parses, and self-announces. It proves nothing about
  behavior and is not a substitute for the other classes. One per
  skill is plenty; it may be omitted where a `behavior` fixture in the
  same set already asserts the marker.
- **`contract`** is where non-artefact rules live — a refused action,
  a stop-with-error, a "what would you do with X" recitation, a safety
  posture. Describe-only by nature: the compliant agent writes
  nothing, so there is no artefact to assert on. This is the class
  `project.md`'s "reserve substring/semantic for genuinely
  non-artefact behavior" note is about.
- **`behavior`** is the load-bearing class. Prefer it behavioral
  (seeded workdir + assert) wherever the skill's correct action leaves
  an inspectable change; fall back to judge only when the output is
  irreducibly prose. One per load-bearing behavior, not one per skill.
- **`discovery`** is the end-to-end class this feature adds. It is the
  only class that tests the skill's `description:`/trigger, because
  it is the only one that doesn't do the triggering for the agent.
  A discovery fixture necessarily asserts behavior too — the isolating
  counterpart is the same skill's `contract` or `behavior` fixture,
  which is why the pair is diagnostic.

**What a new skill is expected to carry.** At minimum: one
`discovery` fixture (does it fire?) and one `behavior` fixture per
load-bearing behavior (does it work?). Add a `contract` fixture for
each guardrail or absence path that produces no artefact. Add
`activation` only if no other fixture already proves the skill loads.

## Test Strategy

Per `project.md`'s two layers:

- **`just test` (unit, load-bearing, §8 Test Gate).** The fixture
  runner is bash and has no unit-test surface, so the `class:` field's
  validation is enforced by the runner itself (a malformed fixture
  fails the run, exactly as a missing `prompt:` already does). This
  feature touches no Rust; the gate must stay green with no new unit
  test because there is no new Rust code path.
- **`just resources::verify-skills` (functional, user-driven,
  credit-costing).** Where the fixtures live. Per `project.md`
  "Functional tests are user-driven," the agent prepares fixtures and
  names the commands; running them is the user's call.

Success criteria — every fixture's post-feature class:

| Fixture | Class | Names skill? | Proof | Style |
|---|---|---|---|---|
| `writing-style` | activation | yes | marker present | substr |
| `browser-safety` | contract | yes | judge rubric | judge |
| `kdevkit-dev-loop` | contract | yes | judge rubric | judge |
| `notes-add-note` | contract | yes | judge rubric | judge |
| `notes-vault-selector` | contract | yes | judge rubric | judge |
| `writing-style-learning-loop` | contract | yes | judge rubric | judge |
| `writing-style-formatter` | behavior | yes | styled rewrite + change log | judge |
| `kdevkit-planning` | discovery | **no** | seeded workdir gains artefact | behavioral |
| `kdevkit-closure` | discovery | **no** | seeded workdir gains artefact | behavioral |
| `kdevkit-agents-md` | discovery | **no** | seeded workdir gains artefact | behavioral |
| `notes` | discovery | **no** | seeded workdir gains artefact | behavioral |
| `notes-git-commit` | discovery | **no** | seeded workdir gains artefact | behavioral |
| `notes-topic-no-stub` | discovery | **no** | seeded workdir gains artefact | behavioral |
| `writing-style-discovery` *(new)* | discovery | **no** | self-emergent marker | substr + judge |
| `browser-discovery` *(new)* | discovery | **no** | self-emergent marker | substr + judge |

Coverage after this feature, by skill: `kdevkit` gets contract +
discovery×3; `notes` contract×2 + discovery×3; `writing-style`
activation + contract + behavior + discovery; `browser` contract +
discovery. Every skill has at least one `discovery` fixture — the bar
this feature establishes.

The discriminating question for every discovery fixture: *could an
agent that never loaded the skill pass this?* For the artefact class
the answer is no — the assert greps for files and content in the
skill's specific shape. For the prose class the marker is the guard:
text the agent can only know from the skill file, which the prompt
never supplies.

### Additional runner cases

- A fixture with no `class:` fails as malformed (message names the
  four valid classes).
- A fixture with an unrecognised `class:` fails as malformed.
- `--class <name>` runs only that class; an unknown value exits
  non-zero rather than silently matching nothing.
- `--class` composes with the existing positional name selector and
  `--tools`.

### Why the two discovery classes prove discovery differently

The runner makes the choice for us. `resources/tests/run`'s fixture
loop treats the two mechanisms as **mutually exclusive**: a fixture
carrying `--- setup ---` / `--- assert ---` blocks goes to
`assert_behavioral`, and its `expect_substr:` is never read. So a
marker assertion in a behavioral fixture is inert — which is why the
artefact class relies on the assert alone, and why the prose class
(which has no artefact to assert on) is the only place a marker can do
work.

## Design

**Rationale first.** The alternative considered and rejected was a
blanket sweep: strip the name and path from all 13 fixtures. That
conflates two independent failure modes in every test — a red fixture
could mean bad content *or* failed triggering, so every failure needs
manual bisection. Worse, it weakens the content-side fixtures for no
gain: their job is to interrogate what the skill *says*, and
pre-loading the skill is the correct way to isolate that. Assigning
one question per class keeps each test single-variable and makes the
pair diagnostic.

The second rejected option was adding a marker assertion to the
behavioral fixtures to separate "never fired" from "fired but
misbehaved". The runner makes that inert (see above), and the artefact
already carries the signal: nothing written means nothing fired.

The third rejected option was inferring class from a fixture's shape
(has `assert` → behavioral → "discovery"). Rejected because shape and
purpose are orthogonal — a judge fixture can be `contract` or
`behavior` or `discovery`, so inference would be wrong roughly as
often as right, and it would leave the taxonomy implicit, which is the
problem this feature exists to fix. An explicit declared field is the
whole point.

Five mechanical moves:

1. **Declare `class:` in all 15 fixtures.** A single new line per
   `.smoke` file, parsed exactly like the existing `prompt:` /
   `tools:` / `expect_substr:` lines (`grep '^class:' | sed`), which
   keeps the fixture format's one-key-per-line convention intact. No
   default: a missing or unknown class is a malformed-fixture failure,
   because a silent default is how an unlabelled fixture would slip
   back in.

2. **Behavioral fixtures become discovery fixtures** (6 files). Drop
   the `Load the <skill> from <path>` opening; keep the operational
   scaffolding the harness genuinely needs — the "current directory is
   a vault / repo" orientation and the "actually write the files,
   don't just describe" instruction, both task framing rather than
   skill identification. Also delete the trailing
   `When done print [<skill>] applies` instruction: the runner never
   evaluates it for a behavioral fixture, so it is dead text that
   misleads a reader into thinking the marker is checked. The
   `--- setup ---` / `--- assert ---` blocks are unchanged — the
   asserts already discriminate.

3. **Two new prose discovery fixtures.** `writing-style-discovery`
   poses a bare formatting task; `browser-discovery` poses a bare
   browser task. Each asserts `expect_substr: [<skill>] applies` —
   self-emergent, since the prompt never mentions it — plus a judge
   narrative confirming the behavior. `browser-discovery` must not
   actually drive Chrome: it asks how the agent *would* proceed,
   matching `browser-safety`'s existing describe-only shape.

4. **Runner gains `--class <name>`.** Parsed alongside `--tools` in
   the existing `while`/`case` arg loop; the fixture loop skips
   fixtures whose class doesn't match, mirroring how the positional
   `SELECTOR` already filters by name. The runner's header comment
   (its `--help` output, extracted by the existing `awk`) gains the
   four classes and their purpose, so `resources/tests/run --help` is
   itself a source of truth.

5. **`just` surface and docs.** Add
   `just resources::verify-skills-class <class> [agent]`, following
   the existing `verify-skills-one` shape (`[no-cd]`, one-liner over
   the runner, agent selector). It carries the same
   `[confirm]`-and-credits caveat as `verify-skills` since a class can
   span many fixtures. Then document the taxonomy in `project.md`
   Testing (the four classes, the question each answers, what a new
   skill is expected to carry) and register the new verb in
   `project.md`'s Architecture entrypoints list, where the other
   `resources::*` verbs are enumerated.

Naming: `<skill>-discovery` for the new fixtures, matching the
existing `<skill>-<behavior>` convention. Class values are lowercase
single words to keep the `class:` line trivially greppable.

## Implementation Plan

- [ ] Add `class:` to all 13 existing fixtures per the Test Strategy
      table; runner parses it, and fails a fixture whose class is
      missing or unrecognised.
- [ ] Rewrite the 6 behavioral fixtures as `class: discovery`: drop
      the skill name / path opening and the dead
      `print [<skill>] applies` line; keep workdir orientation and the
      write-not-describe instruction; leave setup/assert untouched.
- [ ] Add `resources/tests/skills/writing-style-discovery.smoke`:
      bare formatting task, self-emergent marker substr, judge
      narrative for the styled rewrite.
- [ ] Add `resources/tests/skills/browser-discovery.smoke`: bare
      browser task, self-emergent marker substr, judge narrative for
      the snapshot→act→verify loop and allowlist posture;
      describe-only so no real Chrome is driven.
- [ ] Both new fixtures carry `tools: claude,kiro,codex`.
- [ ] Runner: `--class <name>` filter + unknown-class rejection;
      update the header comment (`--help`) with the four classes and
      the purpose of each.
- [ ] Justfile: `verify-skills-class <class> [agent]`.
- [ ] `project.md`: document the four classes and what a new skill is
      expected to carry (Testing); register the new verb
      (Architecture entrypoints).
- [ ] Quality Gate (`just fmt-check` + `just lint` + `just check`)
      and Test Gate (`just test`) stay green.
- [ ] Dry-run the runner's class filter and malformed-fixture guards
      without spending credits (a bad-class fixture must fail before
      any agent is invoked), then hand off / run the functional layer
      across all three agents; treat a discovery failure as a
      trigger-strength finding to record, not a prompt to soften.

- *Risk note:* the discovery fixtures may expose that a skill's
  `description:` does not reliably auto-trigger on a bare task across
  all three harnesses. That is the feature working as intended — the
  finding gets recorded (and fixed in the skill, or filed) rather than
  papered over by re-adding the skill name.
- *Risk note:* a bare task must still make the intended skill the
  obvious match. Prompts that could equally invoke a second skill will
  read as flaky; wording is tuned per fixture to stay unambiguous.
  Deliberately near-boundary (clash-probing) prompts are explicitly
  not part of this feature.
- *Risk note:* `browser-discovery` is the one fixture whose skill
  drives real, credential-bearing Chrome. It stays describe-only for
  exactly that reason; a slip into imperative task framing would make
  the suite act on a live browser session.
- *Risk note:* adding a required `class:` field is a breaking change
  to the fixture format — every existing fixture must be updated in
  the same commit as the runner check, or the suite fails wholesale.
  Kept as one slice for that reason.

## Decision Log

- **Name four purpose classes, orthogonal to verification style
  (2026-07-28).** The runner already documented substring / semantic /
  behavioral, but those are *mechanisms*, not *purposes* — which is
  how the suite ended up with 13 fixtures that all answered the same
  question. `activation` / `contract` / `behavior` / `discovery` name
  the question; the mechanism stays a separate, existing axis. Class
  is declared explicitly rather than inferred from fixture shape,
  because shape doesn't determine purpose (a judge fixture can be any
  of three classes).
- **Split by class rather than sweeping all fixtures (2026-07-28).**
  Considered stripping name+path from all 13. Chose the split:
  activation/contract/behavior keep the explicit load so a failure
  isolates content; discovery drops it so a failure isolates
  triggering. One variable per test, and the pair triangulates cause.
- **No marker assertion on artefact-class discovery fixtures
  (2026-07-28).** Considered asserting the self-announce marker inside
  the behavioral fixtures to separate "never fired" from "fired but
  misbehaved". Rejected as inert in the current runner: the fixture
  loop routes setup/assert fixtures to `assert_behavioral` and never
  evaluates their `expect_substr:`. The artefact is the proof instead.
  Consequence: the dead `print [<skill>] applies` line in those 6
  prompts is removed rather than made load-bearing.
- **Marker IS the proof for prose-class discovery (2026-07-28).**
  `writing-style` and `browser` produce no filesystem artefact, so a
  bare-task judge alone could be passed by a capable agent that never
  loaded the skill. The self-emergent marker is the one signal only a
  loaded skill can produce, so both new discovery fixtures assert it.
  Not a contradiction of the decision above — that one covers the case
  where an artefact already carries the proof.
- **`class:` has no default (2026-07-28).** Considered defaulting an
  absent class to `contract` (the most common) to avoid touching every
  fixture. Rejected: a silent default is exactly how an unlabelled
  fixture slips back in, defeating the self-evidence this feature is
  for. Fail loudly instead, and update all fixtures in one slice.
- **Clash detection out of scope (2026-07-28).** Probing skill
  collisions needs deliberately near-boundary prompts — a distinct
  design with its own flakiness profile. Per the user, not something
  to take on now, and not filed as backlog.

## Session Log

- 2026-07-28 · Promoted from backlog `fixtures-drop-load-from-path-
  narrative` and re-scoped. Grounding survey: all 13 fixtures open
  with the `Load <skill> from <path>` narrative; classified the suite
  as 7 response (substr/judge) + 6 behavioral. Found `writing-style`
  and `browser` have no behavioral layer, so a behavioral-only split
  would leave them without discovery coverage — resolved by adding two
  prose discovery fixtures. Read `resources/tests/run` and found
  behavioral and response verification are mutually exclusive, making
  the `print [<skill>] applies` line in all 6 behavioral prompts dead
  text. Spec renamed from the backlog's title to match the split.
- 2026-07-28 · Extended scope on user direction: the *kinds* of skill
  tests must be self-evident, not tacit. Added the four-class taxonomy
  (`activation` / `contract` / `behavior` / `discovery`) with the
  question each answers and a "what a new skill needs" checklist,
  plus the three mechanisms that make it legible — `project.md` docs,
  a per-fixture `class:` field, and a class-scoped `just` verb.
  Grounding: confirmed the runner's existing header documents
  *mechanisms* (substr/semantic/behavioral) and that no purpose
  taxonomy existed anywhere, so class had to be a new orthogonal axis
  rather than a rename of the styles.
