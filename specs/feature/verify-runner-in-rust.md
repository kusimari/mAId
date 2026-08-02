---
name: verify-runner-in-rust
description: Move the skill-verification runner from bash to Rust as a workspace member beside build-tool, so the fixture format, five test kinds, prompt construction and verdict extraction become unit-testable instead of only provable by spending API credits.
metadata:
  type: feature
---

# Feature: verify-runner-in-rust

## Git Setup

- Branch: `refactor/verify-runner-in-rust`
- Base: `main` @ `333686a`

## Feature Brief

The skill-verification runner becomes a Rust workspace member instead
of an 890-line bash script, so the logic it has accumulated — a fixture
format, five test kinds, per-agent prompt construction, and per-agent
verdict extraction — is covered by `just test` in milliseconds rather
than only by paid tri-tool sweeps. The verb surface
(`just resources::verify-skills` and its three siblings) keeps its
names, flags, and output shape; only what sits behind them changes.

## What

`resources/tests/run` is 890 lines of bash. It now owns the whole
skill-test taxonomy: per-agent skill paths, explicit/implicit prompt
construction, the five test kinds, judge invocation and verdict
extraction, behavioral setup/assert execution, and the `--kind` /
`--tools` / `--dry-run` surface.

Move it to a Rust crate — a workspace member beside
`resources/build-tool`, reusing the deps already present (`anyhow`,
`clap`). The Justfile verbs (`verify-skills`, `…-one`, `…-kind`,
`…-dry`) keep their names and behavior; only what sits behind them
changes.

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

Every flag the runner accepts today still works and means the same
thing: a positional fixture selector, `--tools <list>`, `--kind
<list>` (with `--kinds` as an accepted alias), `--dry-run`,
`--stressed`, and `--help`.

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

### Unit tests (the gate — `just test`)

These are the point of the feature. Each bullet is a case the bash
version could not express.

- **Skill-path resolution.** Each agent resolves to its own skills
  root; an unknown agent is an error. One case asserting the three
  paths match the deployment manifest.
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

### Why a second crate rather than a build-tool subcommand

The backlog left this open. Two things decide it.

Against folding it into `build-tool`: that crate is a
`$HOME`-mutating symlink state machine, and
`specs/backlog/build-tool-language-rust-vs-python.md` rests its
keep-it-in-Rust verdict on exactly that narrowness. Giving the
installer a surface that drives coding agents, seeds scratch
directories, and executes fixture shell widens the blast radius of the
one tool in the repo that can replace files in `$HOME`.

For sharing something: the runner's per-agent skill paths are a
restatement of the deployment manifest, and the backlog names that
duplication directly. Two copies of "where does kiro read skills from"
is how they drifted before.

So: **a new binary crate `resources/verify-tool/`, and the skills-root
knowledge moves into a library target inside the existing `build-tool`
package** (`src/lib.rs` alongside `src/main.rs`), which the new crate
depends on. That keeps the manifest single-source, keeps the installer
binary pure-symlink, and adds no third package. `verify-tool` mirrors
`build-tool` in name and shape: one job, one crate, invoked through
Just.

Fixtures stay at `resources/tests/skills/`, and
`resources/tests/browser-functional` stays put. `resources/tests/run`
is deleted.

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

- [ ] Scaffold `resources/verify-tool/` as a workspace member; extract
      the skills-root manifest into a `build-tool` library target and
      resolve skill paths through it, with unit tests asserting the
      three agent roots.
- [ ] Port the fixture format: parse sections and fields, reject every
      malformed shape with today's messages, default `tools:` to
      claude. Unit-tested.
- [ ] Port prompt construction and the two dry-run structural checks,
      including every implicit-leak class and the common-noun
      carve-out. Unit-tested.
- [ ] Port verdict extraction behind a per-agent normalisation trait,
      table-driven over recorded output for the four shipped bugs.
- [ ] Port agent invocation, availability checks, and judge
      resolution, preserving the read-only and workdir invocation
      shapes each agent needs.
- [ ] Port the five kinds, the response and behavioral assertion
      paths, the leak tripwire, and the reporting and exit-code
      contract.
- [ ] Rewire the four Just verbs to the new binary, delete
      `resources/tests/run`, and update `README.md` and `project.md`
      for the new layout and the `just test` coverage.

- *Risk note:* the paid sweep is the only end-to-end proof and it is
  user-driven, so the free `--dry-run` parity check against the
  captured 104-line baseline carries most of the review's confidence.
  Any dry-run divergence is a defect, not a rendering difference.
- *Risk note:* the behavioral path executes fixture shell and seeds a
  scratch workdir. It is also the path with a known containment leak
  that this feature deliberately does not fix. Porting the tripwire
  faithfully matters more than tidying it.
- *Risk note:* moving the registry into a library target touches
  `build-tool`, the one crate that mutates `$HOME`. The move must be a
  pure relocation with its existing tests unchanged and still passing.
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

## Decision Log

- **New crate, shared manifest.** `resources/verify-tool/` as a
  second binary crate rather than a `build-tool` subcommand — keeps
  the `$HOME`-mutating installer narrow, which is the premise the
  Rust-vs-Python decision record rests on. Considered a single crate
  (rejected: widens the installer's blast radius) and full
  duplication of the skills roots (rejected: that drift is what the
  backlog item flags). The manifest moves to a library target in the
  `build-tool` package so there is one source of truth and no third
  package.
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
