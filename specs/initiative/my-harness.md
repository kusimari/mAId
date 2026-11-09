# Initiative: my-harness

- Branch: `initiative/my-harness` (cut from `main` at d34934a)
- Status: closed for Claude Code (2026-10-12); every stream merged except
  M4 (agy, parked for the Next round). This is the generic half of a two-repo
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
- A one-value setting another part may own (today Claude's
  `env.CLAUDE_CODE_DISABLE_AUTO_MEMORY`) is set only when empty, already
  mAId's, or identical to what mAId would write and owned by no other part
  (adopted).
- Paid tests: up to 25 smoke-fixture runs for this half.
- Pushes, PRs and the merge to `main` are the user's.

## Streams

| # | Feature | Ships | Needs | Status |
|---|---|---|---|---|
| M1 | harness-contract | Vocabulary, additivity rules, part-list format; install writes `parts/maid-harness.json` | - | merged |
| M5 | whole-status | `just status --whole` reads every part list and checks each harness | M1 | merged |
| M3 | vendor-marketplaces | Vendor marketplaces as bring-ins (backlog `office-document-skills`) | M1 | merged |
| M2 | memory | Memory skill, the way into the store per harness, `just memory-harvest` | M1 | merged |
| M6 | kiro-global-mcp | kiro gets mAId's MCP server from kiro's global MCP config, not a named sub-agent | M1 | merged |
| M9 | reader-rules-values | Rule 14 says how a setting's value compares (string-only) and that a dotted name is tried first as a top-level key; cases added | M7, M8 | merged |
| M10 | memory-mcp | Register pensieve as personal-memory; Claude's own memory off; old memory skill and config removed; harvest by project prefix | M9 | merged |
| M11 | conformance-generic | Shared conformance data names no part: list-setting's plugin paths become generic | M8 | merged |
| M12 | initiative-fixes | Defects from the initiative briefing: bring-in ownership kept when a CLI is missing, contract and README sentences made true, dead fixture code removed, missing tests | M10 | merged |
| M13 | installed-skills-never-need-repo | No skill sends the runtime to the repo; skills' user-owned files readable and writable without a prompt (backlog of the same name) | M7 | merged |
| M4 | agy-verified | agy loads skills and the memory include | M2 | parked: claude first |
| M7 | claude-plugin-reads | A session reads every file of mAId's skills (kdevkit's modules) with no prompt, by a setting mAId installs and owns | M2 | merged |
| M8 | reader-rules | The whole-status reader's error rules written into the contract, so any reader implements them from text | M5 | merged |

## Next round

After the user has tested the setup on Claude Code: streams that make the
same resources and memory work in codex, kiro-cli and agy (G3 kiro-union
is built and parked; M4 agy; codex end to end). Not started on its own.

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
- Ruling (user, 2026-10-11): Claude Code first; kiro, codex and agy follow
  later. Parked: G3 kiro-union (built, branch kept), M4 agy. Macro test
  proves claude only · why: the user's need is one set of resources and
  one memory across agents, proven on one agent first · cost if wrong:
  kiro/codex gaps found later
- Ruling (user): each repo's whole-status is a semantic mirror; the
  reader error rules become contract text (M8); neither repo names the
  other. Skills reach Claude only for now; other agents in Next round.
- Ruling: M7 fixes the prompt on reading mAId's skill files in Claude,
  in install, as a setting mAId owns · why: a by-hand settings edit is
  not a fix · cost if wrong: prompts on skill-module reads
- Ruling (user, 2026-10-11): the hand-added plugin-read rule in the user's
  Claude settings stays (other sessions rely on it). An element identical
  to what mAId would write is adopted: recorded as mAId's, reported as
  already present, removed on uninstall like one mAId wrote · why: the
  user's call · cost if wrong: uninstall removes a rule another session
  added
- Ruling: M9 clarifies rule 14 in the shared text (setting values compare
  only as strings; a dotted name is first a top-level key) · why: G4's
  review found two readings readers could differ on · cost if wrong: a
  non-string setting value is reported not checkable
- Ruling: adopting an identical value applies to every setting a part
  owns, autoMemoryDirectory included (M9 brings mAId and the shared text
  in line; G5 builds to it) · why: the user's adopt ruling, one rule for
  all settings · cost if wrong: uninstall removes a value set by hand
- Ruling: a part names the default memory store only when no other part's
  file names one (rule 3); otherwise it reports and skips the claim · why:
  install order must not decide · cost if wrong: work memory not routed
  until the user clears the other default
- Ruling: a part never adopts a setting value another part's list records
  as placed · why: two lists must never both claim one value, or one
  part's uninstall removes another's setting · cost if wrong: none known
- Note: supersedes "a hand-set value is left alone" above for an exact
  match; any other value is still left alone.
- Ruling (user, 2026-10-12): memory is served by MCP servers, one per
  persona (corporate bound to the corporate store, personal to the
  personal store); each server states what belongs in it and owns its
  store and format; coding agents only register servers. Research first
  (M10): an existing server to depend on, and how to bring memory into
  the agent loop on demand or as a standing instruction. G5 and M2's
  setting-based memory are redesigned after it · why: least coupling to
  any coding agent's internals; on-demand recall scales · cost if wrong:
  a server process per persona
- Ruling (user, 2026-10-12): memory server is one standalone public
  package both parts depend on (like the browser MCP), not one per repo.
  Python, standard library only, single file: no build, no binary per
  platform, runs on the python3 both parts already have. A last search for
  an existing simple server comes first; building our own is accepted.
  Each store has an about.md (what belongs here); Claude's own automatic
  memory is turned off; memory-harvest picks projects by path prefix ·
  why: one small implementation to maintain · cost if wrong: a third repo
  to publish and pin
- Ruling (user, 2026-10-12): the memory server is `pensieve`, its own
  public repo (to be github.com/kusimari/pensieve, published by the
  user), built locally first; as simple as possible, Python libraries
  allowed where they make it simpler. Tools follow Anthropic's memory
  tool command set plus search; files follow Claude Code's memory format;
  per-store standing text from the store's about.md · why: no existing
  server sends per-store instructions · cost if wrong: one small repo
- Ruling: pensieve built (local repo, branch feat/pensieve at 146f867,
  unpublished; merge to its main and publishing are the user's). M10 and
  G5 pin that commit from the local repo for testing; both re-pin to the
  published GitHub commit before any CR/PR · why: a local path must not
  reach a published lock file · cost if wrong: a re-pin commit per repo
- Ruling: Claude's own automatic memory is turned off by the setting
  `env.CLAUDE_CODE_DISABLE_AUTO_MEMORY` = "1" (a string, so the readers
  need no rule change), verified before building · why: least change to
  two readers · cost if wrong: fall back to a boolean rule
- Ruling: Macro test step for memory is now: in a scratch HOME with both
  parts installed, a Claude session has personal-memory and work-memory;
  a work fact lands in the work store and a personal fact in the personal
  store; a fresh session recalls each; Claude's own memory dir stays empty
- Ruling: no default personal store in code; the user sets it once with
  `just memory-store <path>`; Claude's own memory goes off only while a
  part's own memory server is registered · why: no personal names in a
  public repo; never leave a machine with no memory · cost if wrong: one
  setup command
- Macro test run 1 (Claude only, scratch HOME): this part's install,
  personal-memory, memory-off setting, routing, recall and uninstall PASS.
  The other part's registration failed and is being fixed there.
- Macro test (Claude only, scratch HOME, two runs): union, status, this
  part's server and setting, routing, recall and uninstall PASS. The
  remaining checks need the user's real install.
- Ruling: the initiative briefing's defects go back as fix stream M12;
  defect 2 (the shared setting after one part uninstalls) is fixed by
  making the contract sentence true, not by a hand-over mechanism, as in
  the other part · why: least code; reinstall is the repair · cost if
  wrong: Claude's own memory on until the other part reinstalls
- Close: Macro test passed for Claude in a scratch HOME (two runs); the
  initiative review briefing came back clean after M12. Open for the user:
  publish pensieve and re-pin the flake from the local path (backlog
  repin-pensieve-published) before any PR; the real-install checks.
- Ruling (user, 2026-10-13): backlog installed-skills-never-need-repo
  is taken into this initiative as M13 · why: same family as the
  plugin-read fix; it bites every writing-style session · cost if wrong:
  one more stream
