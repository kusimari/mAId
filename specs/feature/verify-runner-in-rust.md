---
name: verify-runner-in-rust
description: Restructure build-tool as the pipeline it is — author, check, install, smoke — folding skill verification in as first-class verbs and splitting it at the install boundary, so the three kinds needing no deployment run pre-install and unit tests replace paid sweeps as the gate.
metadata:
  type: feature
---

# Feature: verify-runner-in-rust

## Git Setup

- Branch: `refactor/verify-runner-in-rust`
- Base: `main` @ `333686a`

## Feature Brief

Skill verification moves into `build-tool` as first-class verbs, so the
logic it has accumulated (a fixture format, five test kinds, per-agent
prompt construction, per-agent verdict extraction) is covered by
`just test` in milliseconds rather than only by paid tri-tool sweeps.

Taking the tool as a whole rather than porting a script, the binary is
reorganised as the pipeline it actually is — author a skill, check it
in isolation, install it, smoke-test it deployed — which splits
verification into the **two stages it was always conflating**: the
three kinds that need no install (so a content change is provable
before it goes live) and the two that can only be tested once deployed,
competing for a shared description budget against every installed
skill. Four files carry it — shim, shared vocabulary, agent-driving
harness, stages — each with its own tests. The verb surface
(`just resources::verify-skills` and its three siblings) keeps its
names, flags, and output shape; only what sits behind them changes.

## What

`resources/tests/run` is 890 lines of bash. It now owns the whole
skill-test taxonomy: per-agent skill paths, explicit/implicit prompt
construction, the five test kinds, judge invocation and verdict
extraction, behavioral setup/assert execution, and the `--kind` /
`--tools` / `--dry-run` surface.

Move it into `resources/build-tool` as first-class verbs, reusing the
deps already present (`anyhow`, `clap`) and the `REGISTRY` its
`skill_path` currently hand-copies. Restructure the binary on the way
in — a 1202-line `main.rs` cannot absorb it honestly, and the runner's
own stages don't all belong on the same side of install. The Justfile
verbs (`verify-skills`, `…-one`, `…-kind`, `…-dry`) keep their names
and behavior; two new verbs expose the boundary.

Keep in bash only what is genuinely shell: the fixtures'
`--- setup ---` / `--- assert ---` blocks are shell by design and
should stay shell, executed by the Rust runner.

## Why

**The project already draws this line, and the runner is on the wrong
side of it.** `project.md` Architecture says: Rust where types help,
bash where shelling out to other tools is the job. That was true when
the script only shelled out to `claude` / `kiro-cli` / `codex`. It is
no longer — the shelling out is now a small part of a program that
parses a fixture format, branches across five kinds, and interprets
model output.

**The logic is untestable where it is.** `just test` covers the
content validator and symlink state machine with 45 unit tests (plus
kaimux's 53); the runner has none, because bash offers no unit
surface. Everything in it has been verified by running the real
agents, which costs credits and minutes. Bugs that unit tests would
have caught in milliseconds instead surfaced during paid sweeps:

- Verdict extraction read the judge's own instruction template out of
  codex's echoed transcript, so every judged test on codex passed
  unconditionally.
- Anchoring the verdict on `^(PASS|FAIL)` then silently broke kiro,
  whose reply is prefixed with a cursor-hide escape and `> `.
- Two `local` statements referenced variables assigned in the same
  statement (shellcheck SC2318), malforming generated test names.
- Behavioral failures reported `assert failed —` with nothing after
  it, because the asserts are silent `test`/`grep -q` under `set -e`.

Each is the kind of thing a table-driven test over sample agent output
would have pinned immediately. In Rust they become fixtures in a test
module; in bash they are only found by spending API credits.

**Types would carry real weight here.** A fixture is a parsed
document, a test kind is a closed enum, a verdict is
`Pass | Fail(reason) | Unparseable`, and prompt construction is a pure
function of (tool, skill, task) — all currently expressed as strings
flowing through `grep`/`sed`/`awk`. The dry-run checks in particular
(explicit prompt must contain that agent's path; implicit prompt must
leak no skill name, path or marker) are assertions about a data
structure, hand-rolled today as substring matching.

## Requirements

The user-facing surface is the four Just verbs and the fixture files.
Both are contracts this feature preserves.

### Verb surface — every existing verb keeps working, two are added

The four verbs keep their names, gating, and behavior:

- `just resources::verify-skills [agent]` — full sweep (check then
  smoke), still `[confirm]`-gated.
- `just resources::verify-skills-one <name> [agent]` — single fixture.
- `just resources::verify-skills-kind <kind> [agent]` — one kind across
  every skill, still `[confirm]`-gated.
- `just resources::verify-skills-dry [name]` — free structural check.

Every capability the runner accepts today survives: a positional
fixture selector, agent scoping, kind scoping, `--dry-run`,
`--stressed`, and `--help`.

**Two verbs are added, exposing the install boundary the runner already
straddles:**

- `just resources::check-skills [agent]` — the three kinds that need no
  install (`activation`, `playback`, `enact`), reading each skill from
  the checkout. No deployed state required, nothing in `$HOME` touched
  or read. Still costs API credits, so still `[confirm]`-gated.
- `just resources::smoke-skills [agent]` — the two kinds that can only
  be tested deployed (`discovery`, `integration`). Requires
  `install-skills` to have run; says so and stops if the symlinks are
  absent, rather than failing every test obscurely.

Both follow the established `<action>-<resource-kind>` naming.
`verify-skills` remains the both-stages sweep, so no existing habit or
documentation reference breaks.

**One deliberate change at the flag layer.** The runner's `--tools
<list>` becomes the `--agent` selector the other verbs already use. The
Just verbs pass their selector positionally, so every `verify-skills*`
verb is invoked exactly as before — the rename is invisible through
Just and visible only when calling the binary directly. `--kinds` is
dropped as an alias for `--kind`; nothing in the repo uses it.

**A kind selector that contradicts the stage is an error, not a silent
no-op.** `check --kind discovery` fails with a message naming why
(discovery needs the deployed listing to compete in), instead of
running zero tests and reporting success. `verify-skills-kind <kind>`
routes to whichever stage owns that kind, so the existing verb needs no
knowledge of the split.

### Fixture files — unchanged

No `.smoke` fixture is edited by this feature. The format keeps every
field and section it has today: `skill:`, `tools:`, `--- playback ---`,
`--- enact ---`, `--- setup ---`, `--- assert ---`, and the
`task:` / `expect:` / `expect_substr:` keys within a section. The
`--- setup ---` and `--- assert ---` blocks stay shell, executed by
the runner — they are shell by design.

### Output the user reads

- Per-test result lines keeping today's tokens and colour:
  `PASS`, `FAIL`, `SKIP`, plus indented detail lines.
- Test names keep today's shape, so a run is diffable against a prior
  run: `<fixture> <kind> via <tool>`, and `skill:<skill> <kind> via
  <tool>` for the two generated kinds.
- A trailing `all tests passed` or `<n> test(s) failed`, with exit code
  0 or 1 respectively; a usage error still exits 2.
- The leak warning still fires when a test writes into the checkout,
  naming the offending paths and counting as a failure.
- On a judged test with no parseable verdict, and on a failed
  behavioral assert, the full output is still written to a temp file
  whose path is printed.

### What a contributor observes

- `just test` now covers the runner: fixture parsing, prompt
  construction, the dry-run structural checks, and verdict extraction
  all have unit tests, so a regression in them fails in under a second
  with no API credits spent.
- `just resources::verify-skills-dry` remains free and, for an
  unchanged fixture set, reports the same results as before this
  change.
- `--help` now prints a generated usage summary rather than the
  script's header comment. The information stays, the rendering
  changes.
- **The tool now reads as one surface.** `build-tool --help` lists the
  pipeline — check, install, uninstall, status, smoke — instead of
  three verbs plus a bash script referenced from the Justfile.
- **A content change can be checked before it goes live.**
  `check-skills` runs against the working tree, so a `SKILL.md` edit is
  verifiable without first installing it — which today means making it
  live for every other session on the machine.
- **A failure says which side of install it is on.** Pre-install
  failure = the content is wrong. Post-install failure with check
  passing = deployment or description-budget competition. Today both
  read as "verify failed."
- **Tests sit beside the code they test.** Opening any module shows
  its logic and its cases together, rather than one 636-line test
  module at the foot of a 1200-line file. Cross-module tests (real
  shipped content, the install round-trip) live in
  `tests/integration.rs`.

### Out of scope

- **Behavioral-test containment and the sandbox asymmetry.** The
  existing leak tripwire is ported as-is; the real confinement fix
  stays in `specs/backlog/test-runner-workdir-containment.md` and
  `specs/backlog/test-runner-sandbox-asymmetry.md`.
- **Parallelism.** The sweep stays sequential.
- **Fixture-format changes.** A redesign would make the port
  unreviewable against a suite whose current results are trusted.
- **`resources/tests/browser-functional`.** A separate attended test
  that drives real Chrome; it stays bash.

## Test Strategy

`project.md` names `just test` as the load-bearing layer and
`just resources::verify-skills` as user-driven. That split decides this
feature's evidence: unit tests are the gate, and the paid sweep is
handed off.

### The refactor's own evidence (slices 1–3)

The module split and test redistribution change no behavior, so their
success criterion is that **nothing moves except location**: `just ci`
green, and the test count still exactly 45. A count that drifts means
a case was dropped, merged, or invented during the move. The two
cross-module tests keep their names in `tests/integration.rs` so the
relocation is greppable.

Note when checking that count: adding a `lib.rs` and a `tests/`
directory makes cargo report **several test binaries** rather than one
`test result:` line, so the tripwire is the sum across targets, not a
single number. Verified against the repo toolchain while planning.

### Unit tests (the gate — `just test`)

These are the point of the feature. Each bullet is a case the bash
version could not express.

- **Skill-path resolution.** Each agent resolves to its own skills
  root, derived from `REGISTRY` rather than restated; an unknown agent
  is an error. This is the case that would have caught the drift the
  backlog flagged.
- **Fixture parsing.** A section-delimited fixture parses into its
  sections and fields. Malformed fixtures are rejected with today's
  messages: missing `skill:`, neither section present, a section
  missing `task:`, a playback section with no `expect:` or
  `expect_substr:`, an enact section with no assert block and no
  expectation, and an unknown tool in `tools:`. A missing `tools:`
  defaults to claude.
- **Prompt construction.** An explicit prompt carries the skill name
  and that agent's own path and does not quote the announce marker; an
  implicit prompt carries the task alone.
- **Dry-run structural checks.** The explicit check fails when the
  prompt lacks the agent's own path. The implicit check catches every
  leak class the bash version learned about: the announce marker, a
  `SKILL.md` mention, each of the three agent roots, a hyphenated
  skill name anywhere, a common-noun skill name only in the
  `<name> skill` phrasing, and a documented invocation verb. Its
  carve-out also holds — a bare common-noun name in a domain phrase is
  not a leak.
- **Verdict extraction.** Table-driven over recorded agent output, one
  case per bug the bash version shipped:
  - a codex-style echoed transcript containing the judge's own
    instruction template must not read as `PASS`;
  - a kiro-style reply prefixed with a cursor-hide escape and `> `
    must still yield its verdict;
  - a verdict token mid-sentence in the judge's prose must not be read
    as the verdict;
  - output with no token at all yields the unparseable case, not a
    silent pass.
- **Announce-contract detection.** A skill declaring its announce line
  is detected from the deployed content; one that does not is reported
  as skippable for the two announce-only kinds.
- **Kind and agent selection.** `--kind` and `--agent` reject unknown
  values; the requested set is also the required set, so a requested
  agent absent from PATH is a failure and an unrequested one is a skip.
- **Stage ownership of kinds.** Each kind maps to exactly one stage,
  and the mapping is total — every kind is owned, none twice. Asking
  `check` for a smoke-only kind is an error naming why, not an empty
  successful run; `verify-skills-kind <kind>` routes to the owning
  stage.
- **Skill source per stage.** `check` resolves a skill to its checkout
  path and `smoke` to the agent's installed root, both from the same
  `Agent` type — the case that pins the two stages as one mechanism
  over two sources, and that would have caught today's inconsistency
  (announce read from the checkout, explicit prompts pointed at
  `$HOME`).

### Free structural A/B (the port's parity evidence)

`resources/tests/run --dry-run` on the current suite produces 104
lines and exits 0; that output is captured before the port. The Rust
runner's `--dry-run` must reproduce it for the same fixture set. This
is the strongest evidence available without spending credits, because
the dry-run path exercises fixture parsing, prompt construction, and
both structural checks across every fixture, kind, and agent.

### User-driven paid sweep (handed off, not run)

Only a real run proves agent invocation, judging, and the behavioral
path still work end to end. Per `project.md`, an agentic session stops
at `just test` and names the command. This feature hands off, in the
order the new split makes useful:

```
just resources::verify-skills-dry                   # free; must match the captured baseline
just resources::check-skills                        # pre-install: content correct, no $HOME touched
just resources::install-skills                      # then deploy
just resources::smoke-skills                        # post-install: discovery competes for real
just resources::verify-skills-one notes-git-commit   # one behavioral fixture, tri-tool
just resources::verify-skills                       # both stages, full sweep
```

The first three lines are the new capability: content can be proven
correct *before* it is made live for every session on the machine, and
if `check` passes while `smoke` fails, the fault is deployment or
description-budget competition rather than the skill's content.

## Design

### Verification is a missing action, not a second tool

Start from the verb surface `project.md` already declares — every verb
reads `<action>-<resource-kind>`:

| action | skills | browser-mcp |
|---|---|---|
| install | `build-tool install` | `browser/manage install` |
| uninstall | `build-tool uninstall` | `browser/manage uninstall` |
| status | `build-tool status` | `browser/manage status` |
| **verify** | **`resources/tests/run`** ← the hole | `tests/browser-functional` |

Read that way, verify is not a new tool looking for a home. It is the
**one missing action in a four-action surface whose other three the
binary already implements** — and the only one that fell out of the
binary, into bash, purely because it was written before the taxonomy
existed. `build-tool` is the right name and the right owner: "build" in
its ordinary sense already spans compiling, testing, and packaging, so
a build tool that validates content and installs it but cannot verify
it is the anomaly.

So: **subcommands on the existing binary**, peers in the same `Cmd`
enum, sharing one `--agent` selector convention. But once verification
splits across the install boundary (next section), "verify" is not one
verb — it is two, and naming them separately is what makes the boundary
visible at the surface rather than buried in a flag:

```
build-tool check     [--agent A] [<fixture>] [--kind K] [--dry-run] [--stressed]
build-tool install   [--agent A] [--dry-run] [--force]
build-tool uninstall [--agent A] [--dry-run] [--force]
build-tool status    [--agent A]
build-tool smoke     [--agent A] [<fixture>] [--kind K] [--dry-run] [--stressed]
```

Listed in pipeline order, which is also the order a contributor runs
them: `check` needs no install and mutates nothing; `smoke` requires
the deployment to exist. A bare `verify` remains available as a
convenience meaning "check then smoke", so the existing habit still
works and the full sweep is still one command.

`--dry-run` keeps its meaning across all five — "plan without acting"
for install/uninstall, "construct prompts without calling an agent" for
check/smoke. Today's `--tools <list>` collapses into the established
`--agent` selector, the last place in the repo where agent selection
has its own vocabulary.

One binary means one `cargo run -p build-tool` with no ambiguity, so
the `default-run` workaround the two-bin draft required disappears.

### Verification is two stages, on opposite sides of install

The five kinds compose two axes — explicit/implicit reach × announce /
recite / perform. Reading them against the install boundary shows the
reach axis *is* that boundary:

| kind | reach | needs install? | why |
|---|---|---|---|
| `activation` | explicit | **no** | the prompt names the path — it can name the checkout |
| `playback` | explicit | **no** | same |
| `enact` | explicit | **no** | same |
| `discovery` | implicit | **yes** | the agent must find it among *every installed skill* |
| `integration` | implicit | **yes** | same |

An explicit prompt says "load the skill at `<path>` and follow it," so
the path can point straight into `resources/content/`. Nothing needs
deploying: the skill is tested **in isolation**, and a failure means
the content is wrong. That is a unit/integration test in the ordinary
sense — run in the project's own context, before anything is deployed.

An implicit prompt states only the task, so the skill has to win
discovery unaided — against every other installed skill, out of a
description listing `project.md` records as *capped and shared*. That
can only be exercised where the deployment actually is. It is a smoke
test in the ordinary sense: run against the real environment, after
the project's effects are pushed there.

**Today's runner already straddles this boundary, inconsistently and by
accident.** `skill_announces()` reads the announce contract from the
checkout (`../content/skills/<name>/SKILL.md`), while `skill_path()`
points every explicit prompt at `$HOME`. Same script, both sides, no
model of the distinction — which is also why `project.md` states
"requires the managed symlinks already deployed" as a precondition for
*all* of verify, over-constraining the three kinds that need no install
at all.

Modelling it properly buys three things beyond tidiness:

- **A fast, free-standing feedback loop.** The explicit kinds become
  runnable against the working tree with no install and no `$HOME`
  mutation — so a content change can be checked *before* it is made
  live for every other session on the machine.
- **An honest failure signal.** A pre-install failure is a content
  defect. A post-install failure, given pre-install passed, is a
  *deployment or competition* defect — a description that lost the
  budget, a broken symlink, a collision with another skill. Today both
  land as "verify failed."
- **CI-eligibility.** The pre-install stage mutates nothing and needs
  no deployed state, so it can run anywhere. The post-install stage
  inherently cannot.

### Structure: four files, one per category

The top level should make the shim / shared / harness / stages
distinction evident — and **one file per category does that**, not one
directory per category. Four categories, four files:

```
resources/content/ ─▶ a valid skill ─▶ checked in isolation ─▶ $HOME ─▶ smoke-tested deployed
                      (1 content)      (2 check · pre-install)  (3 install)  (4 smoke · post-install)
```

```
resources/build-tool/src/
├── main.rs      the shim: clap Cli/Cmd + dispatch. Nothing else.
├── lib.rs       module wiring + the pipeline doc comment
├── shared.rs    vocabulary every stage speaks:
│                  Agent (selector parsing, skills_root, skill_path)
│                  REGISTRY + Kind — the manifest, pure data
│                  repo_root, home_dir
├── harness.rs   driving a coding agent + scoring the reply. Used by
│                stages 2 and 4, owned by neither:
│                  .smoke parsing → sections, fields, implied kinds
│                  the five kinds: reach, verification, owning stage
│                  explicit/implicit envelopes + leak checks
│                  per-agent invocation + reply normalisation
│                  judge selection, judge prompt, Verdict
│                  the two assertion paths: judged reply, behavioral shell
└── stages.rs    the pipeline, in order, one § per stage:
                   1 content — resources → a valid, typed Skill
                   2 check   — pre-install, explicit reach, CHECKOUT source
                   3 install — plan (pure) then apply (effectful)
                   4 smoke   — post-install, implicit reach, installed source
```

**Why files and not directories.** The repo's own precedent is large
cohesive single files — `kaimux/src/main.rs` is 2404 lines, today's
`build-tool/src/main.rs` is 1202 — and this tool is *typed bash*: a
program whose job is to shell out in a well-typed way. Estimating from
the current sources (build-tool carries ~407 lines of logic across its
five sections; the bash runner ~489 of code), one file per category
lands at roughly:

| file | code | + comments | + tests | total |
|---|---|---|---|---|
| `main.rs` | ~60 | ~90 | — | **~90** |
| `shared.rs` | ~85 | ~130 | ~100 | **~230** |
| `harness.rs` | ~400 | ~650 | ~350 | **~1000** |
| `stages.rs` | ~350 | ~500 | ~450 | **~950** |

None exceeds what `kaimux` already carries in one file. Split further —
the 15-file draft this replaces — and each file averages ~150 lines,
which is where `mod` declarations, `use` lines, and cross-file
navigation cost more than the boundary buys. **Section comments carry
the internal structure**, exactly as both current sources already do
(`// ── skill locations ──`, `// 3. Compare ──`); that is the idiom this
codebase reads in, and it survived 1202 and 2404 lines respectively.

The two seams the earlier draft made into directories become sections
instead, and lose nothing that matters:

- **install's pure/effectful seam** — `plan` (what is at the home path)
  versus `apply` (the mutations) stay separate *functions* with
  separate tests, which is what makes force/dry-run correctness
  testable. The seam is in the types and the test cases; a file
  boundary added nothing to it.
- **harness's internal pipeline** — fixture → prompt → invoke → judge →
  assert is a linear flow read top-to-bottom in one file, which is
  arguably clearer than six files whose order you have to reconstruct.

**Why `harness.rs` is a peer of `stages.rs`, not part of it.** It is
the thing that makes stages 2 and 4 *the same mechanism pointed at two
different skill sources*. Folding it into either stage would make one
the owner and the other a client, implying a hierarchy that isn't
there. And not inside `shared.rs`: vocabulary and machinery are
different kinds of thing, which is the distinction the four-way split
exists to state.

**The revisit trigger, stated rather than pre-empted.** If `harness.rs`
runs past ~1200 lines in practice, the one seam worth extracting is the
`.smoke` parser — a self-contained grammar with no dependency on the
rest of the harness. Do that when the size is real, not in advance;
pre-splitting on a projection is how the 15-file draft happened.

**What the two verify stages actually differ by** — and it is only
this, which is why they share the harness:

| | stage 2 · check | stage 4 · smoke |
|---|---|---|
| skill source | `resources/content/` | agent's installed skills root |
| reach | explicit (path named) | implicit (task only) |
| kinds | activation, playback, enact | discovery, integration |
| precondition | none | `install` has run |
| mutates `$HOME` | no | no (but reads it) |

`shared/agent.rs` is what makes that a parameter rather than a fork:
`Agent::skill_path(skill)` for the deployed path, and the checkout path
for pre-install — one type answering both, from one manifest.

### Why this shape, and what it rules out

`main.rs` is 1202 lines — 566 of logic, 636 in a single `mod tests` at
the bottom. Its own doc comment justifies that: *"the whole job is
small enough that splitting it into modules adds noise."* Adding the
runner **falsifies that premise**; patching it into a file that would
then approach 2500 lines is the half job to avoid.

But "split it into modules" is not a design. Three properties make the
shape above the right one:

**Dependencies run strictly one direction.** `shared` depends on
nothing. `harness` depends on `shared`. Each stage depends on `shared`,
on `harness` if it drives an agent, and on the *output type* of the
stage before it — never forward. Four files make this easy to verify by
reading the `use` lines at the top of each: `stages.rs` imports from
`harness` and `shared`, `harness.rs` imports from `shared`, `shared.rs`
imports from neither. Building the stages in that order (see the plan)
is how the claim gets checked rather than asserted.

**The four files answer four different questions.** "How is it invoked"
→ `main.rs`. "What do all stages speak" → `shared.rs`. "How do we drive
an agent" → `harness.rs`. "What are the steps" → `stages.rs`. One file
each, so finding the right one is a single decision rather than a walk
through nested directories.

**The duplication the backlog flagged has one home.** `shared.rs` owns
`Agent::skill_path(skill)` derived from `REGISTRY`. Install asks "where
is agent X's skills tree"; smoke asks "where does agent X read skill Y
from"; check needs neither but wants the same `Agent` type for
`--agent`. One definition, three consumers — that hand-copy drifting
from the registry is exactly the bug the backlog names.

Two specific placements this settles:

- **Stage 1 returns a typed `Skill`, not a bool.** Content validation
  answers yes/no today, while the runner separately greps the same
  `SKILL.md` for the announce marker to decide whether the two
  announce-only kinds can assert anything. Two readers of the same
  bytes for the same reason. `Skill` carries the announce contract, and
  both verify stages consume it — stage 1's output feeding downstream
  rather than being re-derived.
- **Kinds are types; kind *derivation* belongs to the fixture.** A
  `playback` section implies playback, an `enact` section implies enact
  *and* integration, and the `skill:` field alone implies the two
  generated kinds. That mapping is fixture knowledge, so it is a method
  on the parsed fixture rather than a rule spread through a run loop as
  bash had it. The kind type carries its reach, its verification, and
  **which stage owns it** — the new fact this design adds.

What this rules out, explicitly: a single `verify` module (it would
straddle install, the conflation this whole section exists to undo);
`harness` inside `stages` (it belongs to two stages, neither of which
owns it); and a directory-per-category layout (the repo's own 1202- and
2404-line single files are the precedent, and section comments already
carry internal structure at that scale).

**Tests follow the same split.** The single 636-line `mod tests` becomes
one `#[cfg(test)] mod tests` per file — idiomatic Rust placement, and
the answer to "incorporate tests in the right way": each file's cases
sit with the code they cover rather than all four concerns' cases piling
up at the foot of one file. The existing 45 bodies move **verbatim** —
no rewording, no consolidation — so the redistribution is reviewable as
a pure relocation with the count as proof.

Where each group lands, which is also a check on the boundaries (a test
that won't sit cleanly in one file means the seam is wrong):

| today's group | lands in |
|---|---|
| frontmatter + `check_content` cases | `stages.rs` (content §) |
| `plan_one`, `expand`, fan-out expansion | `stages.rs` (install § — plan) |
| install / uninstall / force / dry-run | `stages.rs` (install § — apply) |
| `selected_entries`, `validate_agent` | `shared.rs` |
| `cmd_*` dispatch-level cases | `stages.rs` (install §) |

Two tests resist co-location, because they are genuinely cross-stage
rather than unit:

- `shipped_content_validates` — points the validator at the *real*
  `resources/content/`, not a synthetic tree.
- `structural_install_to_real_directory_layout` — a full
  install → status → uninstall round-trip.

These become `tests/integration.rs`, cargo's convention for exercising
the crate from outside. That needs the package to expose `lib.rs`
beside the binary — which the pipeline's modules want regardless, and
which is what makes unit-testing verify's internals possible at all.

Fixtures stay at `resources/tests/skills/`, and
`resources/tests/browser-functional` stays put — an attended test that
drives real Chrome, out of scope here. `resources/tests/run` is
deleted.

### What each library earns its place for

- **`clap`** (already vendored) — the flag surface, and the `--help`
  output that replaces the header-comment `awk` trick.
- **`anyhow`** (already vendored) — error plumbing, as in `build-tool`.
- **`tempfile`** (vendored today as a dev-dependency) — scratch
  workdirs for behavioral tests and the temp dumps on failure,
  replacing `mktemp` plus a `trap`-based cleanup with a guard that
  cleans up on drop.
- **`strip-ansi-escapes`** — the one candidate new dependency. Verdict
  extraction currently strips escapes with a hand-written `sed`
  expression whose character class missed the cursor-hide sequence and
  silently broke kiro. A parser that implements the escape grammar is
  the right tool; the hand-rolled regex is precisely the code that
  proved it could not be got right by hand. **Subject to the
  vendoring check in the Session Log** — if the flake's offline cargo
  cache can't take a new crate, an escape-stripping helper is written
  by hand *with the unit tests this feature exists to add*, which is
  still strictly better than today's untested `sed`.
- **No YAML parser.** `gray_matter` is right for `SKILL.md`
  frontmatter and wrong here — the fixture format is a line-oriented
  `--- section ---` grammar, not YAML. A small hand-written scanner is
  the honest tool, and it is the thing unit tests will pin.
- **No parallelism crate.** Out of scope, so nothing to add.

### Shape

Types where the bash version had strings flowing through
`grep`/`sed`/`awk`:

- `Agent` and `Kind` as closed enums, so the `--agent` / `--kind`
  validation, the "requested set is the required set" rule, and each
  kind's owning stage are exhaustive matches rather than substring
  tests against a space-padded string.
- `Fixture` as a parsed document — the `skill`, the tool list, and
  optional playback / enact / setup / assert sections — with the
  malformed-fixture guards as parse errors carrying today's messages.
- `Verdict` as `Pass(String) | Fail(String) | Unparseable`, which is
  what makes the four shipped bugs table-driven test cases.
- Prompt construction as a pure function of (tool, skill, task), which
  is what lets the dry-run checks assert against a value instead of
  re-deriving the prompt.

Per-agent quirks go behind one trait with an implementation per agent —
how to invoke it read-only, how to invoke it with a scratch workdir,
and how to normalise its reply before a verdict is read off it. Today
those quirks are spread across a `case` statement, a `sed` pipeline,
and three comments; a new agent should be one implementation.

The five kinds keep their current composition — two axes, explicit or
implicit reach crossed with announce / recite / perform — and the
generated `activation` and `discovery` kinds keep deriving from a
skill's name plus one real task, so no fixture gains a skill path.

The design comments that explain *why* the runner is shaped this way —
why codex needs `--output-last-message`, why the judge is one fixed
agent rather than each agent grading itself, why an implicit prompt
must not name the skill — are load-bearing and move with the code.
They are the record of failures already paid for.

## Implementation Plan

The pipeline is built stage by stage in dependency order — shared
vocabulary, then stage 1, then stage 2, then the tests redistributed —
so the runner arrives into a package already shaped to receive it.
Slices 1–3 are a pure refactor with **no behavior change and no
test-body edits**; the 45 tests passing unchanged is the proof.
Building the stages in order also means each slice compiles against
only what precedes it, which is the check that the one-directional
dependency claim in Design actually holds.

- [x] **Extract `shared.rs` + `lib.rs`.** The `Agent` type (selector
      parsing, `skills_root`, `skill_path` from `REGISTRY`), `REGISTRY`
      + `Kind` as pure data, and the roots. Existing agent-selection
      tests move here verbatim; `main.rs` keeps compiling against it.
- [x] **Extract `stages.rs` — content and install.** Content § gains
      the typed `Skill` carrying the announce contract (so both verify
      stages consume it rather than re-reading the file); install §
      keeps plan (pure) and apply (effectful) as separate functions.
      `main.rs` drops to clap + dispatch. Logic verbatim.
- [x] **Redistribute the tests** per the table above, bodies verbatim,
      with the two cross-stage tests moving to `tests/integration.rs`.
      Count still 45 summed across targets — the number is the
      relocation's proof. Slices 1–3 land green with no behavior change
      and no harness code yet.
- [x] **Build `harness.rs` — fixtures and kinds.** Parse `.smoke`
      sections and fields; derive which kinds each section implies;
      model each kind's reach, verification, and **owning stage**;
      reject every malformed shape with today's messages (`tools:`
      defaults to claude). Unit-tested.
- [x] **Build `harness.rs` — prompts.** Explicit/implicit envelopes
      parameterised by skill source, every implicit-leak class, the
      common-noun carve-out. Unit-tested, including that `check`
      resolves to the checkout and `smoke` to the installed root.
- [x] **Build `harness.rs` — judging and invocation.** Grader
      selection, judge prompt, `Verdict` extraction (table-driven over
      recorded output for the four shipped bugs), then per-agent
      driving, availability checks, reply normalisation, and the
      read-only versus workdir shapes.
- [x] **Build `harness.rs` — assertions.** Both paths (judged reply,
      behavioral shell) plus the leak tripwire.
- [x] **Add stage 2 (`check`) and stage 4 (`smoke`)** to `stages.rs`,
      with their two subcommands: check runs the three no-install kinds
      against checkout-sourced skills, mutating nothing and erroring on
      a smoke-only `--kind`; smoke runs the two implicit kinds against
      installed skills with a clear stop when the symlinks are absent.
      Wire `verify` as the check-then-smoke convenience.
- [x] **Rewire and document.** Point the four existing Just verbs at
      the new binary, add `check-skills` / `smoke-skills`, delete
      `resources/tests/run`, and update `README.md` + `project.md` —
      including correcting the "requires the managed symlinks already
      deployed" line, which is true only of the smoke stage.

- *Risk note:* the paid sweep is the only end-to-end proof and it is
  user-driven, so the free `--dry-run` parity check against the
  captured 104-line baseline carries most of the review's confidence.
  Any dry-run divergence is a defect, not a rendering difference.
- *Risk note:* the behavioral path executes fixture shell and seeds a
  scratch workdir. It is also the path with a known containment leak
  that this feature deliberately does not fix. Porting the tripwire
  faithfully matters more than tidying it.
- *Risk note:* the module split touches every line of the one binary
  that mutates `$HOME`, so it is the riskiest slice by diff size while
  being the least risky by behavior — provided it stays a pure
  relocation. Logic and test bodies move verbatim; anything that wants
  rewording during the move is a separate commit, not smuggled into
  the refactor. A 45-test count that changes is the tripwire.
- *Risk note:* a large mechanical diff is where a real change hides
  best. The Code Review Gate sees the diff without the spec, so the
  commit messages for slices 1–3 must say "relocation only" for the
  reviewer to check against.
- *Risk note:* `--help` output changes shape. It is the one
  user-facing regression in the port and is called out rather than
  hidden.

## Session Log

- **2026-08-03** · Build complete; all nine slices shipped. Relocation
  slices landed with the 45-test count intact, then the harness and the
  two verify stages. Final gate: `just ci` green, 149 build-tool tests
  (147 lib + 2 integration) + 53 kaimux, up from 45.
  **Dry-run parity is exact:** `check --dry-run` + `smoke --dry-run`
  reproduce the pre-port bash listing at 102 lines identical, PASS/FAIL/
  SKIP tokens included, exit 0 — verified independently by the Code
  Review Gate against `git show main:resources/tests/run`.
  Two open questions answered: `strip-ansi-escapes` **cannot** be
  vendored (offline cargo closure, not in the registry index), so escape
  stripping is hand-rolled against the grammar with the tests this
  feature exists to add; and the containment fix stays in its backlog
  item, with the port carrying the tripwire and *widening* it (see
  below).
  Three regressions found by the review gate and fixed: a non-zero agent
  exit scored as a reply, `detect_leak` blind to removed status lines,
  and `--agent` having lost the comma-list form `--tools` had. Parity
  re-verified after each.
  Four defects found by my own parity diff rather than by review: the
  generated kinds ran once per fixture instead of once per skill, the
  announce contract was never consulted, and two of my four bug-tests
  did not fail when their bug was reintroduced (a `?25l` terminator that
  is alphabetic, and a decoy carrying the wrong token).

- **2026-08-02** · Promoted from backlog; grounded in
  `resources/tests/run` (890 lines), `build-tool` (1202 lines, 45
  tests), `kaimux` (53 tests), the 12 fixtures, and the vendored
  crate set. Baseline captured: `just ci` green, `--dry-run` 104
  PASS lines exit 0, `shellcheck -S warning` clean. Branch
  `refactor/verify-runner-in-rust` cut from `main` @ `333686a`.
- **2026-08-02** · Open question carried into dev: whether a new
  crate (`strip-ansi-escapes`) can be added to the flake's cargo
  closure, or whether the port must hand-roll escape stripping. The
  Design records both paths; the answer lands in the Decision Log at
  the slice that needs it.
- **2026-08-02** · Plan amended before any dev work, on review
  feedback: the runner moves into the existing `build-tool` package
  rather than a separate `resources/verify-tool/` crate. Reviewer's
  point — model it as another Rust source in build-tool and share
  what's needed. The separate-crate rationale in the first draft did
  not survive scrutiny; see the Decision Log. Proved the packaging
  out against the repo toolchain first rather than assuming cargo's
  behavior.
- **2026-08-02** · Amended again, same review round, on two further
  points. (1) `build-tool` is the right name — "build" already
  connotes compiling, testing, and packaging, so verification is a
  job a build tool is expected to have, not a foreign tenant. (2) The
  two-bin shape was an unjustified asymmetry: retrospecting the verb
  surface against `project.md`'s `<action>-<resource-kind>` rule
  shows install / uninstall / status / **verify** as four peer
  actions, three already in the binary — verify is the one that fell
  out into bash before the taxonomy existed. It becomes a fourth
  subcommand, and `--tools` collapses into the established `--agent`
  selector. (3) Patching the runner into a 1202-line `main.rs` would
  be the half job the reviewer named: the file's own doc comment
  justifies being single-file on the grounds that the job is small,
  which adding the runner falsifies. So the feature now includes the
  module split and moves the 636-line test module next to the code it
  tests, sequenced *first* so the runner lands into a package already
  shaped for it.
- **2026-08-02** · Fifth amendment: file granularity. Reviewer asked
  what value the `shared/` `harness/` `stages/` directories add over
  one file each, noting "we are still using rust as a better typed
  bash." Measured instead of guessing — the repo's own precedent is
  2404- and 1202-line single files with section comments, and
  projecting current sizes puts the largest new file near ~1000
  lines. Collapsed 15 files to four. The two seams the directories
  existed for survive as functions with their own tests (install's
  pure/effectful split) and as a linear top-to-bottom flow
  (harness's fixture → prompt → invoke → judge → assert), neither of
  which needed a file boundary. Slices dropped 11 → 9.
- **2026-08-02** · Fourth amendment, same review round, and the one
  that changed the feature rather than the layout. Reviewer asked for a
  step back — a fresh look at the design, not bash-to-Rust — and made
  three observations: some kinds need no install (call the agent,
  point it at the skill, check it functions); the install-dependent
  kinds are "at the mercy of all other skills"; and projects normally
  run unit/integration before deploy and smoke after. That maps the
  reach axis onto the install boundary: explicit → pre-install,
  implicit → post-install. Verification is therefore **two stages, not
  one**, and today's bash already straddles the boundary by accident
  (`skill_announces` reads the checkout while `skill_path` points at
  `$HOME`) — which is also why `project.md`'s "requires the managed
  symlinks already deployed" over-constrains three of five kinds.
  Restructured to shim / shared / harness / stages so the boundary is
  evident, split `verify` into `check` and `smoke` verbs, and added
  stage-ownership of kinds as a modelled fact. New capability that
  falls out: content is provable before it is made live for every
  session on the machine, and a check-passes/smoke-fails result now
  localises the fault to deployment or description-budget competition.
- **2026-08-02** · Amended a third time, same review round: the layout
  itself was questioned. Reviewer's framing — common things at the
  root, then authoring skills from resources, then installing them,
  then verifying them (with kinds belonging to each smoke) — is a
  **pipeline**, and my flat split wasn't one: it mixed data-shape
  modules with a stage directory. Reorganised the top level onto that
  single axis: shared vocabulary (`agent` / `registry` / `paths`) +
  one module per stage (`content`, `install/`, `verify/`), dependencies
  one-directional. Two consequences fell out that the earlier layout
  had hidden — stage 1 should return a typed `Skill` carrying the
  announce contract instead of a bool (verify currently re-greps the
  same file for it), and kind derivation belongs on the parsed fixture
  rather than in the run loop. Slices resequenced to build the stages
  in dependency order, which is also how the one-direction claim gets
  checked: each slice compiles against only what precedes it.

## Decision Log

- **`strip-ansi-escapes` cannot be vendored; hand-rolled instead.** The
  flake's cargo closure is offline and the crate is absent from the
  registry index, so the Design's stated fallback applies. The
  hand-rolled version matches the escape grammar (CSI, parameters, final
  byte in `0x40..=0x7E`) rather than the `sed` class `[a-zA-Z]` it
  replaces — which is the actual defect, since `?25l` happens to end in
  an alphabetic byte and `\x1b[3~` does not.
- **The leak tripwire widened rather than being ported verbatim.** The
  plan said carry it as-is, and I did initially — but the review gate
  showed the bash version only compared for *added* status lines, so the
  very incident it documents (`close notes` running `git commit`, which
  removes an untracked line) could pass silently. Reporting both
  directions is a one-line change inside the scope the plan already
  claimed, and leaving a knowingly-blind tripwire in place would have
  been worse than the small scope growth. The real containment fix still
  belongs to its backlog item.
- **`--agent` takes a list; the install verbs do not.** `--tools`
  accepted `claude,kiro`, and collapsing it to a single agent silently
  turned a two-agent run into a three-agent one. Verification verbs use
  `validate_agents`; install/uninstall/status keep the single-agent
  `validate_agent`, since scoping an install to two agents was never a
  thing the surface offered.
- **A non-zero agent exit is an invocation failure, not a reply.** The
  bash version distinguished these and the first port did not, so an
  expired token read as "response missing <marker>" — pointing the reader
  at content that was fine. Scoring only reached on a clean exit now.

- **Verification belongs in `build-tool`, as verbs on one binary.**
  Derived from `project.md`'s `<action>-<resource-kind>` verb rule
  rather than from where the code happens to sit today: the actions on
  a skill are peers, and the binary already implements three of them.
  Verification is in bash only because it predates the taxonomy. Three
  rejected alternatives, all mine from earlier drafts of this spec:
  - *A separate `resources/verify-tool/` crate* — argued on the
    grounds that it would keep verify's agent-driving surface away
    from the one binary that mutates `$HOME`. Wrong: code paths don't
    leak into each other and clap dispatch is exhaustive, so a
    package boundary protects nothing. It would have cost a third
    manifest, re-declared deps, a cross-crate API, and generalising
    `repo_root()` for a second caller.
  - *A second bin in the same package* — no better. Two bins for peer
    actions on one resource kind is an asymmetry with nothing behind
    it, and it needed a `default-run` workaround to keep
    `cargo run -p build-tool` unambiguous.
  - *One `verify` verb* — the shape three drafts assumed. It hides the
    install boundary the runner already straddles; see the split
    decision below.
  Also rejected duplicating the skills roots in the runner: that
  drift is exactly what the backlog item flags.
- **`build-tool` keeps its name.** "Build" in ordinary use already
  spans compiling, testing, and packaging, so a build tool that
  validates and installs content but cannot check or smoke-test it is
  the anomaly — the added verbs make the name more accurate, not less.
  Considered renaming to something resource-neutral; rejected as churn
  across the Justfile, README, `project.md`, and `repo_root()`'s
  sentinel check for no gain.
- **The split is in scope, and lands first.** `main.rs` is 1202 lines
  and its doc comment justifies being single-file because "the whole
  job is small enough that splitting it into modules adds noise" — a
  premise adding the runner falsifies. Patching the runner into it
  would leave a ~2500-line file and the stated rationale false.
  Sequenced before the runner so it is reviewable as a pure relocation
  with the 45-test count as its proof, rather than tangled with new
  logic.
- **Verification splits at the install boundary.** The reach axis
  (explicit/implicit) *is* the install boundary, so the five kinds
  divide into three that need no deployment and two that require it.
  Modelled as two stages with two verbs (`check`, `smoke`) rather than
  one `verify` with a flag, because a flag would leave the boundary
  invisible at the surface — and the boundary is the point. Rejected
  keeping one `verify` stage: it forces the three isolation kinds to
  wait on an install that makes the content live for every session on
  the machine, and it flattens two different failure meanings into one
  message. `verify` survives as a check-then-smoke convenience so no
  existing habit breaks. A kind selector that contradicts its stage is
  an error, not a silent empty pass.
- **One file per category, not one directory.** Four files —
  `main.rs` (shim), `shared.rs`, `harness.rs`, `stages.rs` — with
  section comments carrying internal structure. Grounded in two
  measurements rather than taste: the repo's own precedent is
  `kaimux/src/main.rs` at 2404 lines and `build-tool/src/main.rs` at
  1202, both single files with `// ── section ──` headers; and
  projecting from current sizes (build-tool ~407 code lines,
  the bash runner ~489) puts the largest new file near ~1000 including
  comments and tests — under what kaimux already carries. **Superseded
  my own 15-file draft**, which averaged ~150 lines per file: at that
  size `mod` declarations, `use` lines, and cross-file navigation cost
  more than the boundaries buy. The tool is *typed bash* — a program
  whose job is to shell out in a well-typed way — so the types and the
  test cases carry the design, not the directory tree. Revisit trigger,
  stated rather than pre-empted: if `harness.rs` passes ~1200 lines,
  extract the `.smoke` parser (a self-contained grammar with no
  dependency on the rest of the harness) — when the size is real, not
  on a projection.
- **`harness` is a peer of `stages`, not part of it or of
  `shared`.** It is the same mechanism — fixtures, prompts,
  invocation, judging — pointed at two different skill sources, so
  putting it inside either verify stage would make one the owner and
  the other a client, implying a hierarchy that isn't there. Rejected
  `shared/harness/`: vocabulary and machinery are different kinds of
  thing, and flattening them is what made an earlier draft read wrong.
- **The layout axis is the pipeline, not the file's existing
  sections.** `resources → skill → $HOME → verified behavior` is three
  stages over one resource, plus the vocabulary all three speak. So the
  top level is: three small shared modules (`agent`, `registry`,
  `paths`) + one module per stage (`content`, `install/`, `verify/`),
  with dependencies running strictly one direction. **Superseded my own
  earlier draft** that split flatly along the file's numbered sections
  — that mixed two axes, putting data shapes (`registry`, `content`,
  `roots`) flat beside a stage directory (`verify/`), which is why it
  read as subdividing a file rather than stating an architecture.
  Directories only where there is a real internal seam (`install/`
  splits pure planning from effectful mutation; `verify/` is the
  largest surface); a directory per small module would be the ceremony
  the original doc comment warned about.
- **`Agent` is a shared type, not a per-stage string.** Install asks
  "where is agent X's skills tree", verify asks "where does agent X
  read skill Y from" — the same knowledge, which is precisely why the
  runner's hand-copied `skill_path` drifted from `REGISTRY`. One type
  in `shared/agent.rs` derived from the manifest, so `--agent` parses
  identically across every verb, and the two verify stages differ only
  in which path they ask it for.
- **Stage 1 returning a typed `Skill` was planned and NOT built.**
  The intent was to fold the announce contract into content validation's
  output, so the two verify stages consume it rather than re-reading the
  file. As shipped, `check_content` still answers with a count and
  `skill_announces()` re-reads `SKILL.md` per test. The duplication it
  would have removed is real but cheap (a small file read, no
  correctness consequence), and the change touches stage 1's signature
  and every caller — so it is recorded here as deliberately deferred
  rather than quietly dropped. Filed as a backlog item at closure.
- **Kind derivation belongs to the fixture.** The five kinds are types
  (`harness/kind.rs`), but *which* kinds a fixture yields is fixture
  knowledge — a playback section implies playback, an enact section
  implies enact and integration, the `skill:` field alone implies the
  two generated kinds. So it is a method on the parsed fixture rather
  than a rule spread through the run loop, which is where the bash
  version kept it.
- **Cross-module tests go to `tests/integration.rs`.**
  `shipped_content_validates` (validates the real shipped content) and
  `structural_install_to_real_directory_layout` (full install round
  trip) don't belong to any single module. Cargo's `tests/` directory
  is the idiomatic home, and exercising the crate from outside
  requires the `lib.rs` target the runner's modules want anyway.
- **`--tools` becomes `--agent`.** The runner is the last place in the
  repo where selecting a coding agent has its own vocabulary. The Just
  verbs pass their selector positionally, so all four `verify-skills*`
  verbs are invoked exactly as before; the rename is visible only when
  calling the binary directly. `--kinds` is dropped as an alias for
  `--kind` — nothing uses it.
- **Straight port, fixture format frozen.** No `.smoke` file is
  edited. Considered redesigning the format at the same time;
  rejected because the current suite's results are the only
  trustworthy reference for reviewing the port.
- **Sequential, not parallel.** The backlog raised concurrency; a
  ~40-minute sweep is a real cost, but parallelism changes result
  ordering and interacts with agent rate limits and per-test scratch
  dirs. Deferred to keep the port's diff reviewable; worth a backlog
  item at closure.
- **Containment stays open.** The two containment backlog items are
  not closed by this feature. The port carries the existing tripwire
  as-is and makes the real fix cheaper to land afterwards in one
  place instead of two.
