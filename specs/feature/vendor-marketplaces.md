# Feature: vendor-marketplaces

## Git Setup

- Branch: `feat/vendor-marketplaces/work`
- Base: `initiative/my-harness`
- Worktree: `mAId-worktrees/vendor-marketplaces`

## Feature Brief

`just install` also brings in plugin marketplaces that a vendor publishes,
registered through each base harness's own plugin CLI, beside mAId's own
skills. The first one is the vendor's public document-skills marketplace,
so claude and codex sessions can author spreadsheets, slide decks, Word
documents and PDFs (`xlsx`, `pptx`, `docx`, `pdf`). mAId registers it and
installs the plugin; the content stays the vendor's. `just uninstall`
removes only what mAId added, and the part list says what was placed,
skipped and why.

Part of initiative: [[my-harness]]

## Handoff

- **Stage:** closed
- **Ready for:** squash-merge into `initiative/my-harness` by the
  ringmaster
- **Carry forward:** none open. Ownership, pinning, the uninstall order
  and the unreadable-list rule are in project.md Architecture
  "Bring-ins"; the runtime list is in Dependencies.
- **Deliberately left:** a CLI call that fails after placing its item
  leaves it unowned, so mAId keeps it (ruled: strays over deletions).
  `needs` is copied by hand from the vendor's skill text and can drift
  (accepted). No backlog item for either, nor for the last review's low
  notes (the refusal line also prints for kiro; no rollback check that
  install stops on an unreadable list).

### Crossings

<!-- Append one line per crossing; never edit or delete a line. -->

- planning → dev
- dev → review
- review → closure
- closure → closed

## Requirements

- After `just install`, a new claude session and a new codex session each
  have the vendor's `xlsx`, `pptx`, `docx` and `pdf` skills, and everything
  they had before.
- `just status` shows, per harness, the vendor marketplace and its plugin
  beside mAId's own.
- `just install <agent>` and `just uninstall <agent>` act only on that
  harness. A second install changes nothing.
- If the user had already registered that marketplace, or installed that
  plugin, mAId leaves it as it is, does not claim it, and does not remove
  it on uninstall.
- `just uninstall` removes the plugin and marketplace mAId added. If the
  user has installed another plugin from that marketplace, the marketplace
  stays, so their plugin keeps working.
- If the vendor's repo cannot be reached, mAId's own skills still install;
  the run says what failed and exits non-zero.
- `just status` prints one line per command the document skills run that
  is not on PATH, so a user sees why a loaded skill would fail.
- mAId's part list names, for claude and codex, the marketplace and plugin
  as placed, or skipped with why (the user's, the CLI missing, a failure);
  for kiro and agy, both skipped with "no plugin marketplace".

## Test Strategy

### Unit (`just test`, load-bearing)

- The CLI calls install makes for each case: nothing there (add the
  marketplace, codex pinned to the commit, then the plugin); the user's
  marketplace there (plugin only); the user's plugin there (nothing);
  mAId's own from a previous install (nothing).
- The calls uninstall makes: mAId's plugin, then mAId's marketplace; the
  marketplace kept while another plugin from it is installed; nothing
  for the user's.
- What is placed: present and mAId's is placed; present and not mAId's
  is skipped "not mAId's"; absent is not listed; no CLI is skipped with
  the CLI's reason; kiro and agy are skipped "no plugin marketplace".
- Ownership is read from the previous part list: placed entries only,
  per harness; a missing list owns nothing.
- The registry row: an https git source, a full 40-hex commit, named
  plugins, spelled out literally.

### Integration (free, temp HOME, network)

- `cargo test -p build-tool --test plugins -- --ignored` gains a bring-in
  test against the real CLIs: install lists the marketplace and plugin in
  both and records them placed; a second install is a no-op; uninstall
  removes both; a marketplace the user added first is kept and recorded
  as not mAId's; a plugin the user installed from mAId's marketplace keeps
  the marketplace on uninstall.
- `nix develop -c resources/tests/rollback` gains checks that the part
  list names the bring-in placed for claude and codex and skipped for
  kiro, and that uninstall removes it.

### Paid

- None planned. codex's skill listing is read free through its own
  app-server (`skills/list`), which was checked by hand during planning
  (below); claude's own plugin listing is the evidence there.

## Design

**Registration, not vendoring.** The four skills are source-available,
not open source (`license: Proprietary` in each frontmatter), and this
repo is public, so mAId copies nothing. It asks each harness's plugin CLI
to add the vendor's git marketplace and install one plugin from it,
which is the same extension point mAId's own plugin uses.

**Declared in the registry.** `shared.rs` gains `BRING_INS`, one row per
vendor marketplace: the name both CLIs list it under
(`anthropic-agent-skills`), the git source
(`https://github.com/anthropics/skills.git`), the commit codex is pinned
to, and the plugins installed from it (`document-skills`). The plugin
agents (`PLUGIN_AGENTS`) are the ones that get it.

**A third deployer.** `deploy.rs` gains `BringIns`, a `Deploy` beside
`Symlinks` and `Plugins`, wired into `Deployment` last, so a failure in
it never stops mAId's own plugin. It must be inside `Deployment`:
`record_placed` replaces every plugin and marketplace entry of a harness
with what `Deployment::placed` returns, so a separate verb's entries would
be wiped by the next `install-skills`. The CLI helpers it shares with
`Plugins` (running a CLI with HOME, reading `plugin list` and
`marketplace list`) take the id or name as an argument instead of the
`maid` constants.

**Ownership comes from the part list.** Neither CLI records who added a
marketplace, and the user may add the same URL. So an item is mAId's when
mAId's previous part list names it as placed for that harness, or this
run added it. `part.rs` gains a reader for that; `BringIns` loads it once
and adds to it as it acts. Losing the list fails safe: mAId then treats
its own registration as the user's and leaves it. Each item is judged on
its own: with the user's marketplace there, mAId still installs the plugin
from it (and owns only the plugin).

**Uninstall order and the shared marketplace.** Plugin before
marketplace, as for `maid` (codex orphans the plugin's config entry
otherwise). `claude plugin marketplace remove` also uninstalls every
plugin from it (checked: the user's `example-skills` went with it), so
mAId keeps the marketplace, still recorded as placed, while any plugin
from it that is not mAId's is installed.

**Failures.** A CLI call that fails (no network, the vendor moved the
repo) becomes a new `State::Failed`: the line says so, the run exits
non-zero, the part list records it skipped "failed". mAId's own skills
are already in by then.

**Pinning.** Registering at install is not a runtime fetch: nothing
fetches at session start, and nothing installed names a checkout.
- codex: `marketplace add <source> --ref <commit>` pins it exactly;
  config.toml records the ref.
- claude: its CLI takes only a branch or tag after `#` (a commit fails:
  "Remote branch … not found"), and the vendor publishes no tags. So
  claude clones the default branch at registration and caches the plugin
  at that commit (its listed version is the short commit). Third-party
  marketplaces do not auto-update by default (vendor docs), so it stays
  there until the user updates it. Not pinned to the registry's commit.
  Writing claude's settings to pin it (`extraKnownMarketplaces` with a
  `sha`) would bypass its CLI; rejected.

**kiro and agy: skipped.** `kiro-cli` has no plugin or marketplace
command (`--help-all`), and agy is not installed here to check. Reaching
the skills there would mean copying the vendor's files to a skills path,
which is the vendoring this design avoids. Recorded skipped, reason "no
plugin marketplace".

**Runtime: a prerequisite of the environment.** The skills shell out to
`python` with openpyxl, pandas, pypdf, pdfplumber, reportlab, pypdfium2,
lxml, defusedxml, python-pptx, markitdown and pdf2image; to `soffice`,
`pandoc`, `qpdf`, `pdftotext`/`pdftoppm`; and to node with the `docx` and
`pptxgenjs` packages. All but the two node packages are in nixpkgs. The
browser MCP precedent works because its registration names an absolute
profile path. A skill runs bare command names from the session's PATH,
and no harness lets a part put a runtime there cleanly:
- claude puts an enabled plugin's `bin/` on the Bash PATH, but after the
  user's own entries (vendor docs), so a user's own `python3` without the
  libraries wins. This machine has one.
- codex plugins have no `bin/` (its app-server plugin schema has none);
  its `shell_environment_policy` would replace PATH, a one-value setting.
- Changing the user's PATH is their environment, not mAId's (rule 1,
  Hard constraints).

So the runtime is the environment's, like git or Chrome. project.md's
Dependencies table lists exactly what the skills expect. The registry row
names the commands they run (`needs`), and `status` prints one line per
command missing from PATH; Python libraries are listed, not probed.

## Implementation Plan

- [x] `shared.rs`: `BringIn` and `BRING_INS`, with the literal-row test.
- [x] `part.rs`: read the placed entries per harness from mAId's list,
  with tests.
- [x] `deploy.rs`: generalise the CLI listing helpers by id/name; add
  `BringIns` (install, uninstall, status, placed, `is_deployed` false)
  with pure step functions and their unit tests; `State::Failed`; wire it
  into `Deployment` and `main.rs`.
- [x] `status`: one line per missing command a bring-in `needs`.
- [x] `tests/plugins.rs`: the ignored real-CLI bring-in test.
- [x] `resources/tests/rollback`: part-list checks for the bring-in.
- [x] project.md: Architecture (bring-ins), Hard constraints (the
  registrations mAId owns), Dependencies (what the document skills
  expect), Testing (the plugins test needs network).

- *Risk note:* the rollback test now clones the vendor repo once per run
  (network, a few seconds).
- *Risk note:* claude's plugin version is the vendor's commit at first
  install, so two machines can differ; status prints the listed version.

## Session Log

- 2026-10-10 · Closure. Last code review (the rebase commit and the
  status change, correctness, fail_on medium): PASS WITH NOTES, lows
  only, left. Ignored plugins test 3/3 and rollback 19/19 at the head.
  project.md sections touched, each checked: Architecture (Bring-ins,
  Deploy list), Tech Stack (Dependencies row), Layout (deploy.rs,
  plugins.rs), Testing (network, new checks), Deployment (install now
  registers bring-ins, added at close), Hard constraints (registrations
  mAId owns). Backlog answer: `office-document-skills` is closed by this
  feature (promoted into this spec in the first planning commit); no
  other item is closed, and none is filed.
- 2026-10-10 · Review rulings: claude and codex only; strays over
  deletions; fresh installs only for the pinned commit; `needs`
  staleness accepted. Change: plain `status` keeps working on a part list
  it cannot read (reports it, ownership unknown); install and uninstall
  still stop; `status --whole` already reports a refused list and goes
  on. One unit test.
- 2026-10-10 · Rebased onto `initiative/my-harness` after whole-status
  (M5) merged. Conflicts in `deploy.rs` (M5 extracted `listed_plugins` /
  `listed_marketplaces`; the bring-in helpers now use them) and the
  project.md Testing paragraph. The rollback test now also checks that
  `just status --whole` shows the vendor marketplace and plugin present
  on claude and codex. `just ci` (260 unit tests), ignored plugins test
  3/3, rollback 19/19, all green after the rebase.
- 2026-10-10 · Review briefing returned four defects, fixed: a no-CLI
  bring-in test (`tests/no_cli.rs`), the `deploy.rs` header names
  `BringIns`, uninstall prints "removed" for a bring-in item, and the
  rollback test re-run at the head (all 15 pass). Ignored plugins test
  3/3 and `just ci` green.
- 2026-10-10 · Code Review Gate, cycle 3 (override, fail_on medium):
  PASS WITH NOTES. Applied its low note: uninstall drops what it removed
  from the in-run ownership, so a listing that fails after it cannot
  record a removed item as placed. `just ci` and the ignored plugins test
  green again.
- 2026-10-10 · Code Review Gate, cycle 2 of 2: FAIL (high). The
  unreadable-listing fix recorded mAId's own placed items as skipped, so
  ownership was lost for good. Fixed: an unreadable listing keeps what
  mAId owns as placed; a user's local marketplace under the vendor's name
  is found by its path. `just ci`, the ignored plugins test (3/3) and the
  rollback test (all pass) re-run green. The retry budget is spent, so
  the fix has had no third review (hard-stop: needs the user's override).
- 2026-10-10 · Code Review Gate (correctness lens; no named-risk path):
  PASS WITH NOTES, no high. Applied: plugins after a failed marketplace
  add are recorded failed; an unreadable vendor listing is recorded
  skipped rather than stopping the whole part list; the part-list reader
  refuses an unknown format before ownership is read; a claude plugin of
  the same id at another scope keeps the marketplace; mAId's own
  marketplace lookup keeps its `path` key; the rollback check rejects a
  listing that is not JSON. Left (low): a CLI call that fails after
  placing its item leaves it unowned, so mAId keeps it rather than
  removing it, the safe direction.
- 2026-10-10 · Dev. `just ci` green (230 unit tests); `cargo test -p
  build-tool --test plugins -- --ignored` 3/3 (the bring-in test, an
  unreachable source, and the existing plugin test); `nix develop -c
  resources/tests/rollback` all 15 checks pass, two of them new. The
  skills call bare `python` (31 times) rather than `python3`, so `needs`
  names `python`; ImageMagick (`magick`) and `pdfimages` added from the
  PDF skill's own text.
- 2026-10-10 · Planning grounding, all in a temp HOME. The vendor
  marketplace is at the root of `https://github.com/anthropics/skills`
  (main at `dbd4588f9e10`, no tags), named `anthropic-agent-skills`; its
  `document-skills` plugin lists `xlsx`, `docx`, `pptx`, `pdf`. claude
  2.1: `marketplace add <https url>` and `plugin install
  document-skills@anthropic-agent-skills --scope user` work; `#<commit>`
  fails; `owner/repo` tries SSH and failed here. codex 0.162: `marketplace
  add anthropics/skills --ref <commit>` reads the claude-format manifest;
  `plugin add` installs it; its app-server `skills/list` shows exactly
  `document-skills:{docx,pdf,pptx,xlsx}`. Removing the marketplace in
  claude also removed a second plugin from it. kiro-cli 2.29 has no
  plugin command; agy is not installed.

## Decision Log

- Promoted from backlog `office-document-skills`; the backlog's
  `install-document-skills` verb is replaced by a registry row that
  `just install` acts on · the part list is rewritten per harness from
  one `placed` call, so a separate verb would lose its entries.
- Ruling (ringmaster): runtime is an environment prerequisite, listed in
  project.md; status names missing commands · runtime not supplied from the flake · no harness extension point puts
  it on the skills' PATH ahead of the user's own tools; considered
  claude plugin `bin/` (loses to the user's PATH), codex env policy
  (replaces PATH), a PATH change (the user's environment).
- Considered pinning both harnesses by fetching the vendor repo into the
  profile as a nix input and registering it by path · exact pins and
  rollback, but it puts proprietary content in mAId's build output and
  makes every vendor update a mAId change; the brief settles on
  registration from git.
