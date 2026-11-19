# Feature: prompt-prefix

## Git Setup

- Branch: `feat/prompt-prefix/work`
- Base: `initiative/harness-core`
- Worktree: `mAId-worktrees/prompt-prefix`

## Feature Brief

The prefix in front of each prompt follows D1 and D2 (initiative,
"Who does what"). Our code still gathers: location and file signals, the
caps, the timing. Where meaning matters it now casts a wider net and
offers short candidates, a few memories and the turn bring-ins that nearly
fit, and the model already in the session decides which to open. The
essentials (who is acting, the work's goal and next step, the fitting
bring-ins' first rules) go again in a short form once the last copy has
drifted far back, so early rules stay in reach in a long session without
the history filling with copies.

Part of initiative: [[harness-core]]

## Handoff

- **Stage:** closed

### Crossings
<!-- Append one line per crossing. Never edit or delete a line. -->
- planning → dev
- dev → review
- review → closure
- closure → closed

## Requirements

What the person and the model see in a Claude Code session with the ghost on.

- **R1 Candidates.** A turn's prefix may end with up to 5 candidates, under
  one line saying they are candidates to open if relevant, not
  instructions. A candidate is one line: a memory (its title, hook and
  file), or a turn bring-in that nearly fits (`may apply: <name> (<job>)`).
- **R2 Wider net.** A memory is a candidate when its title or hook shares a
  word with the prompt, with the kind of work the fitting bring-ins are
  for, or with what the prompt's named files are (a `.sh` file is shell
  work). Macro run 5's case: a shell habit reaches a prompt about fixing a
  `.sh` file that never says "shell" or "awk".
- **R3 Caps hold.** At most 3 fitting bring-ins with their first rules, as
  today; at most 3 memory candidates and 2 near fits, 5 in all; the turn
  under 9,000 bytes; the genome under 8,000.
- **R4 Reinforced once drifted.** When the last copy of the essentials is
  10 prompts back, or 200 KB of the session's transcript back, a short form
  goes again: the acting persona and its memory, the record's goal and
  next step, and each fitting bring-in's name with its first two rule
  lines, at most 1,500 bytes.
- **R5 Reinforced on change.** A bring-in that fits again after a turn
  without it, and whose rules the session already holds, gets its short
  form. New or changed essentials go in full, as today.
- **R6 Nothing changed, nothing sent.** A second prompt with nothing
  changed and nothing drifted gets no prefix at all, candidates included.
- **R7 Switch.** `MY_HARNESS_CANDIDATES=off` narrows the candidates to
  today's reach: memories only by the prompt's own words, no near fits.
  They keep the candidate line and its framing, and the short form still
  goes; the switch is for the net, not for D2.

## Test Strategy

**Unit (`just test`, in each file).**
- Candidates: R2's four sources each alone; the Macro run 5 case (habit
  "No awk in scripts - shell habit", prompt "Fix data.sh so it prints only
  the second field of each line") is a candidate; with the switch off it is
  not; near fits from a partial phrase, a shared job word, and a fit past
  the cap of 3; no near fit for a bring-in the session already holds.
- Caps: 3/2/5 with many matches; the framing line once, only when a
  candidate is placed; the 9,000-byte budget counts the framing line and
  cuts candidates last (the short form goes through the same count).
- Drift: due at 10 turns and at 200 KB from the transcript's size, from
  the prompts' own bytes with no transcript; not due on turn 9 or 199 KB;
  the mark moves when the short form goes or every essential went in full;
  a reset (compaction) starts the count again; only append calls count.
- Short form: first two rule lines from bullets with wrapped lines,
  numbered lists, paragraphs, headings and fences skipped; per-line and
  total caps.
- Nothing changed: same prompt twice → empty; turns 2-9 of a steady
  session → empty.

**Free integration (`claude_live`, loopback stub, real Claude Code).**
- candidates appear: the framing line and the habit's line in the first
  request, once.
- a long session (11 prompts) carries the short form in prompt 11's
  request only.
- a second prompt with nothing changed gets nothing (existing case, now
  over candidates too).

**Paid (4 of the initiative's remaining 10, then 2 more as granted).** `claude_paid`, isolated as
today, with the real memory server on a scratch store holding the habit;
the model may read memory in both arms. Runs: with, without, with, without;
stop at the first error or a with-arm miss. Recorded per run: whether the
reply's script avoids `awk`. Then one pair (with, without) on a second
habit against the first's outcome, "Always awk, never cut": the script
uses `awk` and not `cut`. Asserted on the with run only.

**Gates:** `just ci`, `ghost-in-shell/tests/rollback`, `claude_live`,
`claude_installed`, on the newest installed Claude Code.

## Design

Code reads as the concepts (user's ruling): one concept per file under
`infusion/src/expression/`, each `//!` naming it and its spec section.

| File | Concept |
|---|---|
| `expression.rs` | the entry point: builds the `Prefix` for a call |
| `prefix.rs` | `Prefix`: essentials, then reinforcement, then candidates; the framing line; the turn budget; placement through the delivery record |
| `essentials.rs` | `Essentials`: acting persona, the work's record, fitting bring-ins; each one's full item and its short form |
| `reinforcement.rs` | `Reinforcement`: the short form, first rule lines, its byte cap |
| `drift.rs` | `Drift`: turns and bytes since the essentials were last placed; when the short form is due |
| `candidate.rs` | `Candidate`: memories and near fits, the widened query, caps, one line each |
| `select.rs` | `Selector` (kept), `Query` gains the widened words; the memory indexes |
| `fit.rs` | turn fit, plus `near`: what nearly fits |
| `delivery.rs` | the record, plus each session's drift state |
| `record.rs`, `here.rs` | unchanged |

**Candidates (D1).** `Query { prompt, about }`: `about` holds the fitting
bring-ins' job and signal words, and the language of each file the prompt
names, from a short extension table (`sh` shell, `py` python, `rs` rust,
...). `Lookup` scores prompt words 2 and `about` words 1; with the switch
off `about` is empty, which is v1 exactly. Near fits: bring-ins ranked past
the cap, then those with some words of a signal phrase or one word of
their job; the session's held fits are skipped. Each candidate is its own
item (`recall.<hash>`, `near.<part>.<name>`), so it goes once a session.
The framing line is no item: it goes when a candidate does.

**Drift (D2).** The delivery record keeps per session `turn`, `at` (the
transcript's size when the call names it, else the prompts' bytes so far),
the last turn's fit set, and the mark (`turn`, `at`) where the essentials
were last placed whole. Only append calls with a prompt count; a reset
clears it.

- *N = 10 prompts.* Frequent enough that a chatty session with little tool
  output, where bytes barely grow, still restates within a working stretch;
  at 1.5 KB at most, a copy every 10 prompts is noise beside the history.
- *B = 200 KB of transcript.* Claude Code's transcript holds every tool
  result whole; 200 KB of JSONL is roughly 25K tokens, an eighth of a 200K
  window. A copy further back than that sits behind most of what the model
  reads first. The cost is at most 1.5 KB per 200 KB, under 1%.
- Prompt bytes alone would miss the cause of drift, which is tool output.

**Example prefix, first prompt** ("Fix data.sh so it prints only the
second field of each line", two personas, in this repo on a feature
branch), with candidates:

```
[my-harness] acting through personal; its memory: personal-memory (store /s/personal)
[my-harness] the work's record: /r/specs/feature/parser.md
  goal: Feature: parser
  state: dev
  next: review, once the gates pass
[my-harness] fits this turn: kyodakit (personal): developer judgement on any coding work, in any language
## 0. Stop where you should stop or are expected to stop. Ask well, with context.

- When the user names a checkpoint ("play it back first", "plan before code",
  ... (its first 30 lines)
[my-harness] fits this turn: kdevkit (personal): spec-driven feature work ...
  ... (its first 30 lines)
[my-harness] candidates that may bear on this turn; open one if it is relevant, they are not instructions:
- memory (personal): No awk in scripts - shell habit (/s/personal/memory/no-awk-in-scripts.md)
- may apply: writing-style (write and rewrite prose in my voice, ...)
```

Without candidates (the switch off, and today): the same, minus the last
three lines. The habit is not offered: no word of the prompt is in it.

**Drift timeline, 12 prompts** (N = 10, B = 200 KB; "full" is an item
sent whole as today, "short" the reinforcement):

| # | Prompt / event | Transcript | Sent | Why |
|---|---|---|---|---|
| 1 | plan the parser | 0 KB | persona, record, kdevkit full; 2 candidates | first turn; all placed: mark 1 @ 0 |
| 2 | same again | 6 KB | nothing | nothing changed |
| 3 | fix the parser test | 14 KB | kyodakit full; 1 candidate | a new fit goes full |
| 4 | go on | 40 KB | nothing | kyodakit no longer fits; leaving is no trigger |
| 5 | draft an email to the team | 52 KB | writing-style full | new fit |
| 6 | go on with the parser | 70 KB | nothing | |
| 7 | rewrite that email | 81 KB | short: writing-style | it fits again, rules held |
| 8 | (Handoff edited) go on | 95 KB | record full | its next step changed |
| 9 | go on | 120 KB | nothing | |
| 10 | go on | 150 KB | nothing | 9 prompts, 150 KB since mark |
| 11 | go on (reads a 230 KB log) | 170 KB | short: persona, record, kdevkit | 10 prompts since mark: mark 11 @ 170 |
| 12 | what failed? | 410 KB | short: the same | 240 KB since mark: mark 12 @ 410 |

## Implementation Plan

- [x] `essentials.rs`, `reinforcement.rs`: move the persona, record and
  fit items out of `expression.rs`; add each one's short form, with tests.
- [x] `drift.rs` and the drift state in `delivery.rs`, with tests.
- [x] `candidate.rs`, the widened `Query`, `fit::near`, the switch in
  `ghost::config`, with tests (Macro run 5 case included).
- [x] `prefix.rs`: order, framing, budget, placement, drift update;
  `expression.rs` reduced to the entry point; existing tests kept green.
- [x] `claude_live`: candidates, long session, nothing changed.
- [x] project.md "Expression" and the layout tree.
- [x] Gates; then the 4 paid runs, results in the Session Log.

## Session Log

- 2026-11-10: grounded on the initiative spec (D1, D2, the 2026-11-04 on
  rulings), `expression.rs` and `expression/`, `call.rs`, stream 2's spec,
  `claude_live.rs` and `claude_paid.rs`. Today: the v1 `Lookup` matches the
  prompt's words only; items are deduped by content hash, so a held rule is
  never sent again. Claude's hook input carries `transcript_path`, so the
  transcript's size is known at every prompt. The real memory server is in
  the dev shell (`MAID_TEST_PENSIEVE`).
- Resolves backlog `recall-on-first-turn` (closure removes it).
- Claude Code joins stream-json prompts that wait into one turn, so the
  long-session live case sends each prompt after the last one's result.
- Code review (one pass, three lenses): no high findings. Fixed: memory
  file paths clipped like other fields; the persona clipped; drift counts
  saturate on a state file holding a huge value; a short-form entry too
  long is passed over, not a stop; near fits reuse the fits already found
  (one file walk a prompt); extensions matched in any case; the framing
  drop tested; a fit past the cap offered as a near fit, tested; doc
  pointers to this spec moved to project.md. Kept by design and now said in
  the doc: the caps count candidates the session holds (R6).
- Free gates at d001018: `just ci`, `claude_live` 8/8 and
  `claude_installed` 1/1 on Claude Code 2.1.319, `tests/rollback` all pass.
- Paid, attempt 1: the HOME guard refused before any model call (the
  fixture lacked its state dir; fixed). Attempt 2, run 1 (candidates on):
  Claude Code got 403 from the model provider ("the security token
  included in the request is invalid"), so the model was never reached.
  Stopped there, as the brief says, for the ringmaster. Runs completed: 0
  of 4. The real HOME's Claude dirs show only other live sessions'
  files.
- 2026-11-19, paid runs 1-4 (ringmaster's session): the raw Claude Code
  binary failed authentication ("security token expired"); through the
  user's `claude` launcher (`CLAUDE_BIN=$(command -v claude)`) it worked,
  since the launcher supplies the model access. All 4 passed, and all 4
  kept the habit, candidates on and off: each reply was
  `cut -d " " -f 2 data.txt`. 4 runs spent.
- The first fixture cannot tell candidates apart, for a second reason
  found today: its title's "scripts" (plural dropped) matches the
  prompt's "script", so the habit is in the prefix with candidates off
  too. Checked by running the prompt hook on the fixture's world: the off
  arm's prefix lists "No awk in scripts".
- Added a second fixture against the first's outcome: "Always awk, never
  cut" (shell habit), same prompt, same `data.sh`; kept means `awk` and
  no `cut`. A free test runs the prompt hook on its world: the habit is
  in the prefix with candidates on and absent with them off, so it
  arrives only as a candidate, through `.sh`.
- Paid runs 5-6, one pair through the launcher: on kept the habit
  (`awk '{ print $2 }' data.txt`), and so did off
  (`awk '{print $2}' data.txt`). 6 of 6 kept the habit in force, whichever
  way it points, so the comparison still shows no effect of candidates.
  Not settled from the replies: whether the model read the store itself in
  the off arm (the genome names the store; the memory tools and `Read` are
  allowed), or whether it simply follows a saved habit it finds either
  way. The text output keeps no tool calls, and each run's world is gone.
  A control with no habit, and the tool calls kept (stream-json), would
  tell; not run, no runs left in this grant.
- Rebased onto `initiative/harness-core` (one spec commit), clean.
  Gates at the rebase: `just ci`, `claude_live` 8/8 and
  `claude_installed` 1/1 on Claude Code 2.1.328, `tests/rollback` 61
  PASS.
- Code review, cycle 1 (three lenses, one pass): correctness FAIL on one
  high: the new free test ran `git` for the fixture's store under the
  real git config (hooks, signing). Fixed with an isolated `git` like
  `adaptation.rs`'s. Also fixed: a near fit's id could pass the 128-byte
  id limit where its fit's only just fit, failing the whole prefix (one
  id builder now, tested at the edge); candidate fields flattened to one
  line, so a part's multi-line job cannot start a line under the framing;
  the reset test now fails if a reset keeps the drift count; the live
  candidates case paced; the live runner drains stderr beside stdout; the
  unused `Essentials.record` dropped; docs (`TAG`, the `expression`
  command, layout, README, `claude_live` in Testing). The first paid test
  renamed to `with_candidates_macro_run_5s_habit_is_applied`: the prompt
  does carry its habit's words.
- Briefing, cycle 1, sent back: the Handoff, R7's wording (now what was
  built, Decision Log), and three Test Strategy items with no test, now
  added: the budget counts the framing line and cuts candidates last; a
  fit set all new and sent in full moves the mark; a fit's outcome phrase
  alone reaches a memory. Each new test seen failing on the change it
  names (mutated and restored).
- Code review, cycle 2 (of 2): PASS WITH NOTES on all three lenses, no
  high; every cycle-1 high and medium resolved. Taken from its new lows:
  the budget test now needs some but not all candidates cut, with the
  framing line; the essentials' one-line fields (persona, record lines,
  fit header, short-form names) flattened like candidates; U+2028 and
  U+2029 flattened too. Left as low: the short form's `for` list, a
  Transient-carrier drift test, the drift race comment, a line on
  near-fit text trust in project.md, D1/D2 ids in module docs.
- Closure (ringmaster's stream, so the backlog answer is mine): candidates
  resolve `recall-on-first-turn`: the habit now reaches the first turn of
  a shell question through the named file's language, shown by the
  free tests (`git rm`). Its open question, a local model behind
  `Selector`, goes on as `selector-local-model`. The Handoff's carry
  forward, whether candidates change what the model does, is filed as
  `candidates-paid-control`. The review notes kept as they are (the short
  form's `for` list, `off` only, the drift race, untagged candidate
  lines) are accepted, not wanted: nothing filed. project.md: Expression,
  Layout and Testing were brought up to date during dev and review;
  nothing else touched.

## Decision Log

- Candidates replace today's memory pointers rather than sitting beside
  them: one list, one framing line, the cap of 3 memories kept. Considered
  pointers plus candidates; rejected as two lines for the same memory kind.
- The widened net is the fitting bring-ins' words and the named files'
  languages. Considered stems and a synonym table; rejected as a
  hand-made model of meaning, which is the agent's job (D1).
- The cap applies before what the session holds is left out, so a second
  identical prompt does not surface the 4th-ranked memory (R6).
- "Far back" is the transcript's growth, not the prompts' bytes, where the
  carrier names a transcript (Design, Drift).
- The paid A/B switch is an environment value, `MY_HARNESS_CANDIDATES=off`,
  read with the ghost's other config. Considered building the base
  branch's binary for the "without" arm; rejected: it changes more than
  the candidates. It also lets a person turn candidates off.
- On change, only what changed goes (a new fit full, a returning fit
  short); once drifted, every essential goes short. Considered restating
  every essential on any fit change; rejected as a copy on every topic
  switch.
- Ruling (user, planning review): the switch `MY_HARNESS_CANDIDATES=off`;
  the code-gathering limit accepted; the short form restates the acting
  persona even when there is one persona, since the genome is as far back.
- Amended after the 4 paid runs (ringmaster): a second paid fixture, a
  habit against the model's default, run as one pair; the first fixture
  kept as run. Its free reach test covers only the second fixture, since
  the first is reached by the prompt's own words.
- R7 reworded at review to what was built and project.md says: the switch
  narrows the net only. "Today's prefix" read as the whole v1 prefix,
  which the switch never gave (the briefing caught the gap); the paid
  comparison needs only the net to differ.
- Test Strategy amended at review: no test puts the short form against
  the turn budget. It goes through the same count as every other item,
  after the full items, so a cut short form leaves the mark where it was
  (`placed_whole` needs it sent). Only a drift turn with 3 new fits whose
  name, persona and job are each near the 300-byte clip reaches the cut
  (about 8,850 bytes of fits); a fixture that contrived was not written.
- Names this stream adds live in one constant each, for the rename stream
  that follows (`my-harness` to `ghost-in-shell`).
