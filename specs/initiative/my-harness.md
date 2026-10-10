# Initiative: my-harness

- Branch: `initiative/my-harness` (cut from `main` at d34934a)
- Status: running (ringmaster). This is the generic half of a two-repo
  initiative; the other half lives in a private repo that installs a work
  part beside mAId's. Nothing here depends on that repo.

## Goal

I launch plain `claude`, `codex`, `kiro-cli` or `agy` and it starts as my
agent: the vendor's base harness, with my harness and my memory loaded
through the base harness's own extension points. No wrapper. mAId delivers
one part (maid-harness); other parts may be installed beside it, and the
whole is their union. Memory each base harness keeps on the machine moves
into a store I own and that travels.

## Experience

- After `just install`, a new session of each base harness has every
  mAId skill and MCP server, plus the vendor plugin marketplaces mAId
  brings in, and keeps everything it came with or another part added.
- agy loads mAId's skills, as the other three do.
- `just status --whole` lists, per base harness, what every installed part
  says it placed and whether it is there.
- A memory written in one base harness is recalled in the others, and a
  store pulled on another machine carries it.
- `just memory-harvest` copies each base harness's own memory into a store,
  leaving the originals.
- `command -v claude` (and the others) is the vendor binary.

## Macro test

Under a scratch HOME:

1. `just install`; `just status --whole` lists every mAId capability as
   present for each base harness, read from the harness's own report.
2. With a second part's list placed beside mAId's (a fixture), `--whole`
   reports both; removing that list leaves mAId whole.
3. A Claude Code session saves a memory; it lands in the configured store;
   fresh codex, kiro and agy sessions recall it.
4. `just memory-harvest` copies seeded native memories into the store and
   leaves the originals byte-identical.

## Constraints

- Public repo: generic terms only; no names of any private part, store or
  employer.
- Embedded, not wrapped: no wrapper scripts, aliases or `PATH` shims.
- mAId depends on no other part; the part-list format is defined here.
- Memory copy-out copies; it never deletes.
- A one-value setting another part may own (Claude's
  `autoMemoryDirectory`) is set only when empty or already mAId's.
- Paid tests: up to 25 smoke-fixture runs for this half.
- Pushes, PRs and the merge to `main` are the user's.

## Streams

| # | Feature | Ships | Needs | Status |
|---|---|---|---|---|
| M1 | harness-contract | Vocabulary, additivity rules, part-list format; install writes `parts/maid-harness.json` | - | merged |
| M5 | whole-status | `just status --whole` reads every part list and checks each harness | M1 | merged |
| M3 | vendor-marketplaces | Vendor marketplaces as bring-ins (backlog `office-document-skills`) | M1 | merged |
| M2 | memory | Memory skill, the way into the store per harness, `just memory-harvest` | M1 | running |
| M6 | kiro-global-mcp | kiro gets mAId's MCP server from kiro's global MCP config, not a named sub-agent | M1 | merged |
| M4 | agy-verified | agy loads skills and the memory include | M2 | planned |

## Decision Log

- Ruling: generic half of the my-harness initiative, approved with the
  whole design · why: the user approved the full spec in the private repo
  · cost if wrong: Experience edits here
- Ruling: part lists at `~/.local/state/my-harness/parts/<part>.json` ·
  why: XDG state, owned by no single part · cost if wrong: a path rename
- Ruling: part lists also record skipped steps (`"skipped": "<why>"`) ·
  why: whole-status shows expected vs present; silence on a skipped piece
  reads as nothing expected · cost if wrong: one optional field
- Ruling: vendor document skills' runtime (python libs, office tools) is
  an environment prerequisite, listed in project.md Dependencies; mAId
  does not put it on a session's PATH · why: no base harness offers a
  clean PATH extension point, and the user's PATH is the environment's ·
  cost if wrong: the skills load but fail until the env provides them
- Ruling: the default memory store is named only by an explicit choice
  (mAId's `memory-store --default`, or another part's own config);
  Claude's `autoMemoryDirectory` follows the default store, and a part
  removes its own value when it no longer owns the default · why: keeps
  rule 3 order-independent across parts · cost if wrong: a machine with
  no default until the user picks one
- Ruling: Claude Code's `autoMemoryDirectory` is one dir for every
  project, so the store's memory is cross-project · why: verified against
  claude's settings schema; no per-project form exists · cost if wrong:
  a noisy index; keep it small
- Ruling: new stream M6, kiro MCP through kiro's global config · why: a
  server registered into a named agent is lost when another part
  re-renders that agent; the global config is read by any agent that
  includes it · cost if wrong: one more stream
- Ruling (to ratify): vendor plugin marketplaces reach claude and codex
  only; kiro and agy are recorded as skipped · why: neither has a plugin
  marketplace, and reaching them means copying proprietary vendor files
  into a skills path · cost if wrong: document skills missing in kiro
  and agy
- Ruling: a part owns Claude's `autoMemoryDirectory` only when it equals
  the value that part recorded setting; a hand-set value is left alone ·
  why: rule 2, same fail-safe direction as bring-in ownership · cost if
  wrong: a stray setting the user removes by hand
