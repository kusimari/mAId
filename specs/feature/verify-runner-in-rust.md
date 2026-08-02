---
name: verify-runner-in-rust
description: Fold the skill-verification runner into build-tool as its fourth verb alongside install/uninstall/status, splitting the 1202-line main.rs into modules with tests beside the code they test, so the fixture format, five test kinds, prompt construction and verdict extraction become unit-testable instead of only provable by spending API credits.
metadata:
  type: feature
---

# Feature: verify-runner-in-rust

## Git Setup

- Branch: `refactor/verify-runner-in-rust`
- Base: `main` @ `333686a`

## Feature Brief

Skill verification becomes `build-tool`'s fourth verb — peer to
install, uninstall, and status — instead of an 890-line bash script
referenced from the Justfile, so the logic it has accumulated (a
fixture format, five test kinds, per-agent prompt construction, and
per-agent verdict extraction) is covered by `just test` in
milliseconds rather than only by paid tri-tool sweeps. The binary is
reorganised into modules to receive it, with each module's tests beside
the code they test. The verb surface
(`just resources::verify-skills` and its three siblings) keeps its
names, flags, and output shape; only what sits behind them changes.

## What

`resources/tests/run` is 890 lines of bash. It now owns the whole
skill-test taxonomy: per-agent skill paths, explicit/implicit prompt
construction, the five test kinds, judge invocation and verdict
extraction, behavioral setup/assert execution, and the `--kind` /
`--tools` / `--dry-run` surface.

Move it into `resources/build-tool` as a fourth subcommand, reusing
the deps already present (`anyhow`, `clap`) and the `REGISTRY` its
`skill_path` currently hand-copies. Split the binary into modules on
the way in, since a 1202-line `main.rs` cannot absorb it honestly. The
Justfile verbs (`verify-skills`, `…-one`, `…-kind`, `…-dry`) keep their
names and behavior; only what sits behind them changes.

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

### Verb surface — unchanged

- `just resources::verify-skills [agent]` — full sweep, still
  `[confirm]`-gated.
- `just resources::verify-skills-one <name> [agent]` — single fixture.
- `just resources::verify-skills-kind <kind> [agent]` — one kind across
  every skill, still `[confirm]`-gated.
- `just resources::verify-skills-dry [name]` — free structural check.

Every capability the runner accepts today survives: a positional
fixture selector, agent scoping, kind scoping, `--dry-run`,
`--stressed`, and `--help`.

**One deliberate change at the flag layer.** The runner's `--tools
<list>` becomes the `--agent` selector the other three verbs already
use. It is the last place in the repo where selecting a coding agent
has its own vocabulary, and the Just verbs pass their selector
positionally, so the four `verify-skills*` verbs are invoked exactly as
before — the rename is invisible to anyone using Just, and visible only
when calling the binary directly. `--kinds` is dropped as an alias for
`--kind`; nothing in the repo uses it.

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
- **The tool now reads as one surface.** `build-tool --help` lists
  four peer actions — install, uninstall, status, verify — instead of
  three plus a bash script referenced from the Justfile.
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

### The refactor's own evidence (slices 1–2)

The module split and test redistribution change no behavior, so their
success criterion is that **nothing moves except location**: `just ci`
green, and the test count still exactly 45. A count that drifts means
a case was dropped, merged, or invented during the move. The two
cross-module tests keep their names in `tests/integration.rs` so the
relocation is greppable.

Note when checking that count: splitting into modules plus a `tests/`
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
- **Kind and tool selection.** `--kind` and `--tools` reject unknown
  values; the requested set is also the required set, so a requested
  tool absent from PATH is a failure and an unrequested one is a skip.

### Free structural A/B (the port's parity evidence)

`resources/tests/run --dry-run` on the current suite produces 104
lines and exits 0; that output is captured before the port. The Rust
runner's `--dry-run` must reproduce it for the same fixture set. This
is the strongest evidence available without spending credits, because
the dry-run path exercises fixture parsing, prompt construction, and
both structural checks across every fixture, kind, and agent.

### User-driven paid sweep (handed off, not run)

A full `just resources::verify-skills` is the only proof that agent
invocation, judging, and the behavioral path still work end to end.
Per `project.md`, an agentic session stops at `just test` and names
the command. This feature hands off:

```
just resources::verify-skills-dry                  # free; must match the captured baseline
just resources::verify-skills-one notes-git-commit  # one behavioral fixture, tri-tool
just resources::verify-skills                      # full sweep
```

## Design

### Verify is the missing fourth action, not a second tool

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

So: **a fourth subcommand on the existing binary**, peer to its three
siblings in the same `Cmd` enum, sharing one `--agent` selector
convention.

```
build-tool install   [--agent A] [--dry-run] [--force]
build-tool uninstall [--agent A] [--dry-run] [--force]
build-tool status    [--agent A]
build-tool verify    [--agent A] [<fixture>] [--kind K] [--dry-run] [--stressed]
```

`--dry-run` already means "plan without acting" on install and
uninstall; on verify it means "construct prompts without calling an
agent." Same word, same promise, so the runner's existing
`--dry-run` needs no rename. Today's `--tools <list>` collapses into
the established `--agent` selector rather than introducing a second
name for the same concept — the last remaining place in the repo where
agent selection has its own vocabulary.

One binary means one `cargo run -p build-tool` with no ambiguity, so
the `default-run` workaround the two-bin draft required disappears.

### Revisiting build-tool: a pipeline, not a pile of modules

`main.rs` is 1202 lines — 566 of logic, 636 in a single `mod tests` at
the bottom. Its own doc comment justifies that: *"the whole job is
small enough that splitting it into modules adds noise."* Adding the
runner **falsifies that premise.** Patching the runner into a file
that would then approach 2500 lines is the half job to avoid.

But "split it into modules" is not yet a design. The organising
question is what the tool *does*, and read that way it is a **pipeline
over one resource**, with a shared vocabulary underneath:

```
resources/content/  ──authored──▶  a valid skill  ──installed──▶  $HOME symlinks  ──verified──▶  agent behavior
                     (content)                      (install)                       (verify)
```

Three stages, each consuming the previous stage's output, plus the
vocabulary all three speak (which agents exist, where each expects
skills, where the roots are). That gives one axis for the top level —
*stage* — instead of the two my earlier draft mixed (data shapes flat
beside a stage directory, which is why it read wrong).

```
resources/build-tool/src/
├── main.rs              clap Cli/Cmd; dispatch into the stages. Thin.
├── lib.rs               module wiring + the crate's doc comment
│
│   ── shared vocabulary: what all three stages speak ──
├── agent.rs             Agent (claude|kiro|codex): parse/validate the
│                        --agent selector; skills_root; skill_path(skill)
├── registry.rs          REGISTRY + Kind — the deployment manifest, pure data
├── paths.rs             repo_root, home_dir
│
│   ── stage 1 · resources → a valid skill ──
├── content.rs           parse + validate SKILL.md into a typed Skill
│                        (name, description, announce contract)
│
│   ── stage 2 · skill → $HOME ──
├── install/
│   ├── mod.rs           the install / uninstall / status verbs
│   ├── plan.rs          Comparison + Plan + expand: what is at the home
│   │                    path, and what links an entry resolves to. Pure.
│   └── apply.rs         the mutations: symlink, remove, force semantics
│
│   ── stage 3 · installed skill → agent behavior ──
└── verify/
    ├── mod.rs           orchestration: fixtures × kinds × agents;
    │                    reporting, exit code, leak tripwire
    ├── fixture.rs       .smoke parsing → sections, fields, and which
    │                    kinds the sections imply
    ├── kind.rs          the five kinds as the two composed axes
    ├── prompt.rs        explicit/implicit envelopes + dry-run leak checks
    ├── invoke.rs        per-agent driving + reply normalisation, one impl each
    ├── judge.rs         grader selection, judge prompt, Verdict + extraction
    └── assertion.rs     the two assertion paths: judged reply, behavioral shell
```

**Why this beats a flat split.** Dependencies now run strictly one
direction — `verify` reads `content`'s `Skill` and the shared
vocabulary; `install` reads `content` and `registry`; `content` depends
on nothing but `paths`. No stage reaches sideways into another's
internals, so the layout states the architecture rather than just
subdividing a file. Anyone asking "where does verification live" reads
one directory, and anyone asking "what do these three have in common"
reads three files at the root.

**The vocabulary is where the flagged duplication dies.** `agent.rs`
owns `Agent::skill_path(skill)`, derived from `REGISTRY`. Install needs
"where does agent X's skills tree live"; verify needs "where does agent
X read skill Y from" — the same knowledge, which is why the runner's
hand-copied `skill_path` drifted from the registry before. One
definition, one place, and the shared `Agent` type also means
`--agent` parses identically for all four verbs instead of once per
verb.

**Stage 1 grows a return value.** Today content validation answers
only yes/no. Verify separately greps each `SKILL.md` for the announce
marker to decide whether the two announce-only kinds can assert
anything. Both are reading the same file for the same reason, so
`content.rs` returns a typed `Skill` carrying its announce contract,
and verify consumes it — the pipeline's output feeding the next stage
instead of a second reader of the same bytes.

**`install/` and `verify/` earn directories; the rest don't.** Install
splits along a real seam — pure comparison and planning versus the
effectful mutations, which is where the force/dry-run correctness lives
and where the bulk of the existing tests point. Verify is the largest
surface and has the most distinct concerns. `content`, `agent`,
`registry`, and `paths` are each one small cohesive thing, so a
directory apiece would be the ceremony the original doc comment warned
about — the guard against over-splitting, not just under-splitting.

**Kinds live with the fixture that implies them.** Your point that the
kinds are part of each smoke is the reason `kind.rs` holds the five
kinds as *types* while `fixture.rs` owns the derivation: a `playback`
section implies the playback kind, an `enact` section implies enact
*and* integration, and the `skill:` field alone implies the two
generated kinds. That mapping is fixture knowledge, so it lives in
`fixture.rs` as a method on the parsed fixture rather than as a
free-floating rule in the run loop — which is where the bash version
kept it, spread across the loop body.

**Tests follow the same axis.** The 636-line `mod tests` splits into a
`#[cfg(test)] mod tests` per module — idiomatic Rust placement, and the
answer to "incorporate tests in the right way": a reader opening
`install/plan.rs` sees the comparison logic and its cases together
instead of scrolling past three unrelated concerns. The existing 45
bodies move **verbatim** — no rewording, no consolidation — so the
redistribution is reviewable as a pure relocation with the count as
proof.

Where each group lands, which is also a check on the boundaries (a test
that won't sit cleanly in one module means the seam is wrong):

| today's group | lands in |
|---|---|
| frontmatter + `check_content` cases | `content.rs` |
| `plan_one`, `expand`, fan-out expansion | `install/plan.rs` |
| install / uninstall / force / dry-run | `install/apply.rs` |
| `selected_entries`, `validate_agent` | `agent.rs` |
| `cmd_*` dispatch-level cases | `install/mod.rs` |

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

- `Tool` and `Kind` as closed enums, so the `--tools` / `--kind`
  validation and the "requested set is the required set" rule are
  exhaustive matches rather than substring tests against a
  space-padded string.
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
Slices 1–4 are a pure refactor with **no behavior change and no
test-body edits**; the 45 tests passing unchanged is the proof.
Building the stages in order also means each slice compiles against
only what precedes it, which is the check that the one-directional
dependency claim in Design actually holds.

- [ ] **Extract the shared vocabulary.** `agent.rs` (the `Agent` type,
      selector parsing, `skills_root`, `skill_path`), `registry.rs`
      (`REGISTRY` + `Kind` as pure data), `paths.rs` (roots); add
      `lib.rs`. Existing agent-selection tests move here verbatim.
- [ ] **Extract stage 1 — `content.rs`.** Frontmatter validation, plus
      returning a typed `Skill` carrying the announce contract so stage
      3 consumes it rather than re-reading the file. Its cases move
      verbatim; the announce-contract accessor is the one new test.
- [ ] **Extract stage 2 — `install/`.** `plan.rs` (pure: `Comparison`,
      `Plan`, `expand`) and `apply.rs` (the mutations and force /
      dry-run semantics); `mod.rs` keeps the three verbs. `main.rs`
      drops to clap + dispatch. Logic verbatim.
- [ ] **Redistribute the remaining tests** per the table above, bodies
      verbatim, and move the two cross-stage tests to
      `tests/integration.rs`. Count still 45 summed across targets —
      the number is the relocation's proof. Slices 1–4 land green with
      no behavior change before any runner code exists.
- [ ] **Add the `verify` subcommand** as a fourth peer in `Cmd` sharing
      the `--agent` selector, with `verify/fixture.rs` and
      `verify/kind.rs`: parse `.smoke` sections and fields, derive
      which kinds each section implies, and reject every malformed
      shape with today's messages (`tools:` defaults to claude).
      Unit-tested.
- [ ] **Port prompt construction and the dry-run checks**
      (`verify/prompt.rs`): explicit/implicit envelopes, every
      implicit-leak class, the common-noun carve-out. Unit-tested.
- [ ] **Port judging** (`verify/judge.rs`): grader selection, the judge
      prompt, and `Verdict` extraction — table-driven over recorded
      output for the four shipped bugs.
- [ ] **Port invocation** (`verify/invoke.rs`): per-agent driving,
      availability checks, reply normalisation, and the read-only
      versus workdir shapes each agent needs, one impl per agent.
- [ ] **Port orchestration** (`verify/mod.rs` + `assertion.rs`): the
      fixtures × kinds × agents loop, both assertion paths, the leak
      tripwire, reporting and the exit-code contract.
- [ ] **Rewire and document.** Point the four Just verbs at
      `build-tool verify`, delete `resources/tests/run`, and update
      `README.md` + `project.md` for the pipeline layout, the fourth
      verb, and the widened `just test` coverage.

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
  commit messages for slices 1–2 must say "relocation only" for the
  reviewer to check against.
- *Risk note:* `--help` output changes shape. It is the one
  user-facing regression in the port and is called out rather than
  hidden.

## Session Log

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

- **Verify is a fourth subcommand on `build-tool`.** Derived from
  `project.md`'s `<action>-<resource-kind>` verb rule rather than from
  where the code happens to sit today: install / uninstall / status /
  verify are four peer actions on the same resource kind, and the
  binary already implements three. Verify is in bash only because it
  predates the taxonomy. Two rejected alternatives, both mine from
  earlier drafts of this spec:
  - *A separate `resources/verify-tool/` crate* — argued on the
    grounds that it would keep verify's agent-driving surface away
    from the one binary that mutates `$HOME`. Wrong: code paths don't
    leak into each other and clap dispatch is exhaustive, so a
    package boundary protects nothing. It would have cost a third
    manifest, re-declared deps, a cross-crate API, and generalising
    `repo_root()` for a second caller.
  - *A second bin in the same package* — no better. Two bins for four
    peer actions on one resource kind is an asymmetry with nothing
    behind it, and it needed a `default-run` workaround to keep
    `cargo run -p build-tool` unambiguous. One binary, four
    subcommands, no workaround.
  Also rejected duplicating the skills roots in the runner: that
  drift is exactly what the backlog item flags.
- **`build-tool` keeps its name.** "Build" in ordinary use already
  spans compiling, testing, and packaging, so a build tool that
  validates and installs content but cannot verify it is the anomaly —
  the fourth verb makes the name more accurate, not less. Considered
  renaming to something resource-neutral; rejected as churn across the
  Justfile, README, `project.md`, and `repo_root()`'s sentinel check
  for no gain.
- **The split is in scope, and lands first.** `main.rs` is 1202 lines
  and its doc comment justifies being single-file because "the whole
  job is small enough that splitting it into modules adds noise" — a
  premise adding the runner falsifies. Patching the runner into it
  would leave a ~2500-line file and the stated rationale false.
  Sequenced before the runner so it is reviewable as a pure relocation
  with the 45-test count as its proof, rather than tangled with new
  logic.
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
  in `agent.rs` derived from the manifest, so `--agent` also parses
  identically across all four verbs.
- **Stage 1 returns a typed `Skill`, not a bool.** Content validation
  answers yes/no today, and verify separately greps the same
  `SKILL.md` for the announce marker to decide whether the two
  announce-only kinds can assert anything. Two readers of the same
  bytes for the same reason; folding the announce contract into stage
  1's output makes it the pipeline's data flow instead.
- **Kind derivation belongs to the fixture.** The five kinds are types
  (`verify/kind.rs`), but *which* kinds a fixture yields is fixture
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
