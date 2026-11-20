# Feature: reader-rules

## Git Setup

- Branch: `feat/reader-rules/work`
- Base: `initiative/my-harness`

## Feature Brief

Someone writing a whole-status reader in any language, in a repo that never
reads mAId's code, can build it from the contract text alone and check it
against a shared set of example cases: same state per entry, same exit code.

Part of initiative: [[my-harness]]

## Handoff

- **Stage:** closed
- **Ready for:** nothing; squash-merge into `initiative/my-harness` is the
  ringmaster's
- **Carry forward:** none open. The `find_on` change for install and the
  root rule for permission cases are in project.md's rules and format.
- **Deliberately left:** none wanted. PATH as one stub dir per case was
  ruled acceptable; a case for a CLI that fails to start and exit 2 are
  not filed (exit 2 is usage, not a reader rule; the first is covered by
  rule 8's text and a reader's own tests).

### Crossings

- planning → dev
- dev → review
- review → closure
- closure → closed

## Requirements

- `specs/project.md`, `### My harness`, carries a numbered list of reader
  rules. Each rule changes a state or the exit code; nothing else is a rule.
  It covers: which files in the parts dir are read; when a list is refused,
  and that a refusal fails the run; that only failing to list the parts dir
  aborts; the six states and which fail; CLI handling; file and dir
  handling; text decoding; scope; the exit rule.
- A reader written from that text alone gets the expected result on every
  case in the conformance set.
- The conformance set is plain data: per case, a parts dir, a HOME, what
  each CLI answers, any file modes to set, and the expected state per entry,
  refused files and exit code. Its format is described in the contract, so
  another repo's tests can copy the directory and run it.
- mAId's own `just test` runs mAId's reader against every case.
- Where mAId's reader broke a written rule, the code is fixed, not the rule.
- Generic terms only; the other reader is "a reader in another repo".

## Test Strategy

- **Conformance test** (new `tests/conformance.rs`, in `just test`): for
  each case dir, copy `parts/` and `home/` into a temp dir, make stub CLIs
  from the case's answers on a PATH holding only them, set modes, then:
  - call `whole::check` and compare (harness, part, kind, name, scope,
    state) and the refused file names with `expect`, as sorted lists;
  - run the `build-tool whole` binary with that HOME, parts dir and PATH,
    and compare its exit code with `expect.exit`.
  All failing cases are reported together. Nothing outside the temp dir is
  read or written, and no real CLI can be found.
- Red first: the cases for D1, D6, D7, D8 and D11 below fail against today's
  `whole.rs`; each code change turns its case green.
- Existing `whole.rs` unit tests stay and still pass.
- No paid tests. `just ci` passes.

## Design

The rules are written from `whole.rs` as it is. Where it had no deliberate
rule, the least one is chosen, and it is one a reader in another language
would write without reading Rust: a JSON number, not a Rust integer; any
string is present, not "truthy"; an order-free result, not whatever order
the filesystem lists.

**Rules taken from the code as it is** (no change):

- Parts dir: every entry named `<stem>.json`, stem not empty and not
  starting with `.`. Absent dir: no lists. A dir that exists and cannot be
  listed (permissions, not a dir) aborts: no report, exit 1. An entry that
  cannot be read (permissions, a directory) is refused.
- Refused: not UTF-8, not JSON (a byte-order mark is not JSON), not an
  object, no `format`, `format` not 1, `part` not the stem (exact),
  `harnesses` not an object of arrays, an entry not an object, `kind` or
  `name` not a string, `scope` or `skipped` neither a string nor null.
  Unknown keys are ignored. A repeated key: the last counts.
- D2: null is absent; any string is present, empty included. So
  `"skipped": ""` is skipped, and `"scope": ""` is a scope.
- D3: a reader does not read `value`; any type passes.
- Entry order of checks: `skipped` first (whatever the harness or kind),
  then unknown harness or kind: not checkable, then the table.
- D5: names compare exactly with what the listing or dir holds, never
  joined into a path: skill `a/b` or `.` is missing.
- D9: a skill is present when `<skills>/<name>/SKILL.md` is a regular file
  after links. If that cannot be confirmed (absent, a dir, its own dir
  unreadable), missing. Only a skills dir that cannot be listed is
  unreadable.
- D10: agent files are every regular file (after links) named `<stem>.md`
  (claude) or `<stem>.json` (kiro), dotfiles included, extension exact.
- D12: UTF-8, strict, no BOM stripped, for every file read and every
  stdout. stderr is decoded lossily and only feeds a reason.
- CLI: not on PATH: not checkable. Fails to start, exits non-zero, or
  stdout is not one JSON value (empty included) or not the listing's shape:
  unreadable. stderr never decides a state.
- D13: a listing item is turned off only by `"enabled": false` (the JSON
  value; `0` and `"false"` do not). Items that are not objects or have no
  string name are ignored.
- D14: claude's plugin listing counts only items whose `scope` is exactly
  `"user"`.
- Config files: absent: none listed. Not UTF-8 or not JSON: unreadable. A
  key counts whatever its value, null included; a listed member
  (`mcpServers`) absent or null: none listed.
- Scope: absent or null is the harness's one place (claude: user; kiro MCP:
  global config). `"user"` is the same for a claude plugin or MCP. A kiro
  MCP scope names a kiro agent, `""` included; an agent of that name that
  is not there: missing. Any other scope: not checkable.
- D15: exit 0 unless an entry is missing or unreadable, or a list was
  refused: then 1. An abort exits 1. With one harness selected, only its
  entries are reported, and refused lists still count.

**Rules the code breaks today, and the least change:**

- D1: `format` is a JSON number equal to 1, so `1.0` and `1e0` pass; `true`
  and `"1"` are refused. Today `1.0` is refused, which a reader in a
  language with one number type cannot copy. Change: compare as `f64`.
- D6: when more than one file or listing item could be the entry, the best
  state wins: present, then present (disabled), then unreadable, then
  missing. Today the last item wins in a listing, and for agent files
  whichever the filesystem lists last (for a kiro agent's MCP, first), so
  the state depends on directory order. Change: merge a listing keeping
  the better state; a kiro agent's MCP takes the union of every agent file
  of that name, with an unparsed one making an unlisted server unreadable.
- D7: a config or agent file that is JSON but not an object is unreadable,
  and so is a listed member of the wrong type, in a kiro agent too. Today
  `~/.claude.json` as `[]` is missing, `settings.json` as `[]` unreadable,
  a kiro agent `[]` present by its stem, and a kiro agent's `mcpServers` as
  an array missing. Change: check for an object in both places.
- D8: on PATH means the first absolute PATH dir holding a regular file of
  that name with any execute bit; empty and relative entries are skipped;
  no PATH: not on PATH. Today any regular file counts, so a stray
  non-executable file makes the entry unreadable while the shell would run
  the real one. Change: check the execute bits in `shared::find_on`, which
  `on_path` (install) also uses and documents as `command -v`.

- D11: a claude agent's name, by text alone (no YAML library). Lines
  split on LF, a trailing CR dropped. Frontmatter is the lines between a
  first line exactly `---` and the next line exactly `---`. `name` is the
  first line in it that starts `name:` at column 0; its value is the rest
  of that line with everything from the first ` #` removed, then trimmed,
  then one pair of matching surrounding quotes (`"` or `'`) removed. No
  frontmatter, or no `name:` line: the file names nothing (an entry by its
  stem is missing). A BOM, or a file that cannot be read or is not UTF-8:
  unreadable, by its stem. Today `gray_matter` parses YAML 1.2, which a
  stdlib-only reader cannot copy, and no frontmatter is unreadable.
  Change: a small line reader in `whole.rs`.

**Conformance set**, `resources/build-tool/tests/conformance/<case>/`:

- `case.json`: `about` (one line), optional `harness` (the selected one),
  `clis` (per CLI, per exact argument string: `stdout` or `stdout_file`,
  `stderr`, `exit`; a CLI not named is not on PATH; arguments not listed
  exit 3), optional `modes` (path under the case → octal mode, set before
  the run), and `expect`: `exit`, `aborted` (optional), `entries`
  (`harness`, `part`, `kind`, `name`, `scope` when the list has one,
  `state`), `refused` (file names).
- `parts/` is the parts dir (absent: no parts dir; a file: not a dir).
- `home/` is HOME. Placeholder files keep dirs git would drop.
- A `.gitattributes` with `* -text` keeps bytes exact (CRLF and non-UTF-8
  fixtures).
- Order of entries and reasons are not compared.

Cases, about 20, one rule group each: files read; refused lists; accepted
edge lists (D1, D2, D3, repeated key); no parts dir; parts path a file;
parts dir unreadable; a list unreadable; the passing states; the failing
states; skipped first; unknown harness and kind; the not-checkable cells;
scope; CLI not on PATH and not executable; CLI failures and stderr; listing
items (D13, D14, D6); config files (two cases); skills; claude agents
(D11: `name: x # comment`, a nested `  name:`, quotes, no `name:`, BOM);
kiro agents; one harness selected.

## Implementation Plan

- [x] Conformance harness and cases, red where D1, D6, D7, D8, D11 bite.
- [x] D1, D6, D7, D8, D11 in `whole.rs` and `shared.rs`; all cases green.
- [x] The reader rules and the conformance format in `project.md`,
  replacing today's prose paragraph; Layout and Testing lines for the new
  test.

- *Risk note:* the D8 change also reaches install's `on_path`; its tests
  stub CLIs with mode 755, so none should move. Run the whole suite.
- *Risk note:* permission cases are skipped under root; the test prints
  which.

## Session Log

- 2026-10-11 · grounded on `whole.rs`, `part.rs`, `shared::find_on`,
  `deploy::listed_*`, `main.rs` exit mapping; probed `gray_matter` and
  `serde_json` edges (frontmatter forms, `1.0`, BOM, repeated keys) with a
  throwaway test. Wrote the spec.
- 2026-10-11 · planning cue with one change: D11 becomes a text rule a
  stdlib-only reader can implement (fifth code fix). D8 in
  `shared::find_on`, D9 and the permission-case approach accepted.
- 2026-10-11 · 30 cases; against the old reader exactly the D1, D6, D7,
  D8 and D11 cases failed, every other case passed. The five fixes turned
  them green. Slices 1 and 2 land in one commit so every commit passes
  `just ci`. `resources/tests/browser-mcp` passes (it reads whole-status);
  `resources/tests/rollback` needs network and was not run (run at
  closure, below).
- 2026-10-11 · Code Review Gate: correctness, security and comment-hygiene
  all PASS WITH NOTES, none high. Took the notes: rows compared as equal
  lists (a duplicate row no longer passes), an abort must print nothing,
  permissions restored on panic, stub arguments and case links checked;
  rule 11 trims only spaces and tabs, rule 8 says "any execute bit" and an
  empty PATH entry now really is the current dir, rules 3, 6, 9 worded
  tighter; cases for a non-string `part`, a string `harnesses`, listing
  shapes, a stdout BOM, a scoped codex skill, a linked agent file, tabs
  and a no-break space. 31 cases. Not taken: a case for a CLI that fails
  to start and for exit 2 (usage is not a reader rule).
- 2026-10-11 · Review briefing returned two defects, fixed: a skipped
  case now fails the test unless `CONFORMANCE_ALLOW_SKIPS=1` (a passing
  test's output is hidden, so a root run showed nothing); `find_on` got
  unit tests for PATH order and the empty entry (watched the order test
  fail without the execute-bit check).
- 2026-10-11 · ringmaster ruling on review: D8 skips empty and relative
  PATH entries. `find_on` filters to absolute dirs (install's `on_path`
  gets it too); unit test with a relative spelling of a real dir; case
  `cli-relative-path` (PATH `""` and `.`, stub in the working dir) failed
  against the old `find_on` and passes now. Case format gained `path`;
  the run's working dir is the stub dir. 32 cases. PATH as one stub dir
  per case otherwise stays as left.
- 2026-10-11 · closure. `resources/tests/rollback` on 46664e6: all 19
  checks passed (real claude and codex CLIs, temp HOME, network). Backlog
  answer: this feature closes no backlog item (none is about whole-status
  or part-list readers) and files none. project.md: Architecture
  (`### My harness` reader rules and conformance set), Layout and Testing
  were updated during dev; Tech Stack unchanged (`gray_matter` still used
  by `memory.rs` and `stages.rs`).

## Decision Log

- Conformance cases live beside the tests that run them, not under
  `specs/` · the spec tree holds specs; the contract names the path.
- CLI answers are data, not shell scripts · any reader's test makes its
  own stubs from them; a script would tie the set to one shell.
- States come from the library, exit code from the binary · the library
  gives states without parsing free-form report text; the binary proves
  the exit code a user sees.
- D11 is a line rule, not YAML · a reader in another repo uses only its
  standard library; full YAML 1.2 cannot be implemented from text.
  Rejected: YAML 1.2 via a library (today's `gray_matter`).
- Empty and relative PATH entries are skipped, not the current dir ·
  status runs what it finds, so a `claude` planted in a checked-out repo
  would run; matching `command -v` is not worth that. Rejected: POSIX's
  empty entry as the current dir.
