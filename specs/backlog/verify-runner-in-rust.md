---
name: verify-runner-in-rust
description: Move the skill-verification runner from bash to Rust, as a workspace member alongside build-tool. The bash script has grown to 860 lines carrying prompt construction, per-agent invocation, verdict parsing and five test kinds — logic the project already builds in Rust everywhere else, and logic that is currently untestable.
metadata:
  type: backlog
---

# Move the verify runner to Rust

## What

`resources/tests/run` is 860 lines of bash. It now owns the whole
skill-test taxonomy: per-agent skill paths, explicit/implicit prompt
construction, the five test kinds, judge invocation and verdict
extraction, behavioral setup/assert execution, and the `--kind` /
`--tools` / `--dry-run` surface.

Move it to a Rust crate — a workspace member beside
`resources/build-tool`, reusing the deps already present (`anyhow`,
`clap`, `serde`, `gray_matter`). The Justfile verbs
(`verify-skills`, `…-one`, `…-kind`, `…-dry`) keep their names and
behavior; only what sits behind them changes.

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
content validator and symlink state machine with 53 unit tests; the
runner has none, because bash offers no unit surface. Everything in it
has been verified by running the real agents, which costs credits and
minutes. Bugs that unit tests would have caught in milliseconds
instead surfaced during paid sweeps:

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

## Open questions

- **Crate boundary.** New `resources/tests/` crate, or a subcommand of
  `build-tool` (which already knows the registry and therefore the
  per-agent skill paths — `skill_path` in the runner duplicates
  `REGISTRY`)? Sharing that source of truth argues for one crate; the
  install path staying pure-symlink argues for two.
- **Concurrency.** A sequential sweep is ~40 minutes and each test is
  an independent subprocess. Rust makes parallelism easy, but agents
  may rate-limit and behavioral tests each seed a workdir — needs a
  cap, and results must stay deterministic in ordering.
- **Where the containment fix lands.** See
  [[test-runner-workdir-containment]] and
  [[test-runner-sandbox-asymmetry]] — behavioral tests can escape their
  scratch dir and commit into the checkout. If that is fixed in Rust
  rather than twice, this migration should carry it.
- **Judge tooling.** Verdict parsing is per-agent-quirk today (codex
  transcripts, kiro's `> ` prefix and `▸ Credits:` footer, claude's
  stdin warning). Worth normalising behind one trait with a per-agent
  impl, so a new agent is one impl rather than edits scattered through
  a case statement.
- **Migration shape.** Straight port first and refactor after, or
  redesign the fixture format at the same time? A straight port keeps
  the diff reviewable against a suite whose current results are
  trusted.
