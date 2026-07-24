---
name: notes-and-behavioral-verification
description: Bring every skill up to the behavioral (setup/assert, tri-tool) verification bar that kdevkit already meets — with the notes "stop auto-creating empty topic stubs" behavior change as the concrete first slice, since its assertion ("a [[topic]] reference does NOT create a stub file") is the natural first behavioral fixture. Merged from two backlog items: the notes behavior fix and the broader all-skills verification push.
metadata:
  type: backlog
---

# notes stub fix + behavioral verification for all skills

## Why these are one thread

`behavioral-verification-for-all-skills` needs a first concrete
behavioral fixture to build, and its own open question is "what's
the minimal seedable vault for notes?" The `notes` stub-fix
supplies exactly that: its behavior change ("a `[[topic]]`
reference does **not** create a `topics/<topic>.md` stub") is a
clean, artefact-observable assertion — seed a vault, drive the
agent to write a note with a novel `[[topic]]`, assert the stub
file is *absent*. Fix the behavior and write its behavioral test
in the same session; the notes fixture becomes the template for
the rest of the all-skills push.

Structure this as: **Slice 1 (notes) lands the behavior change +
its behavioral fixture together.** Slices 2+ (writing-style, and
re-shaping the remaining notes smokes) follow as separate slices —
or promote the whole thing to an initiative if the matrix cost
warrants sequencing across branches.

---

## Slice 1 · notes — stop auto-creating empty topic stubs (+ behavioral fixture)

### What

Change the `notes` skill's **Linking** rule so a `[[topic]]`
reference to a nonexistent `topics/<topic>.md` does **not**
auto-create a one-line stub. Leave the wikilink dangling; create
`topics/<topic>.md` only when there is real substance to put in
it.

Current rule (`resources/content/skills/notes/SKILL.md`, under
`## Linking`, ~line 194):

> When a `[[topic]]` reference is written and
> `topics/<topic>.md` doesn't exist, create a one-line stub
> (`kind: topic` + `# <topic>`).

Proposed rule:

> When a `[[topic]]` reference is written and
> `topics/<topic>.md` doesn't exist, **do not create a stub.**
> Leave the link dangling — Obsidian renders unresolved links
> fine — and write `topics/<topic>.md` only when it accrues real
> substance (a definition, links, accumulated notes). Never
> overwrite an existing topic page.

Then write the **behavioral fixture** (`--- setup --- /
--- assert ---`, `tools: claude,kiro,codex`): seed a minimal
Obsidian-shaped vault, drive the agent to add a note referencing a
novel `[[topic]]`, and assert `topics/<that-topic>.md` was **not**
created while the note itself landed with the right frontmatter.

### Why (behavior)

- **Empty stubs are noise.** Every `[[topic]]` mint produces a
  substance-less `kind: topic` + `# <topic>` file. The `topics/`
  dir already holds ~180 such stubs, most 26–50 bytes. They
  inflate the vault, clutter search, and pad the Obsidian graph
  with information-free nodes.
- **Obsidian handles dangling links natively.** Unresolved
  `[[wikilinks]]` render as clickable placeholders and still show
  in the graph as unresolved nodes. The stub buys nothing.
- **Substance-gated creation matches intent.** A topic page should
  exist because someone had something to say — not as a side
  effect of referencing it once.
- Observed live: an `[[opinion-intelligence]]` reference in
  `conversations/comms-strategy-jam.md` auto-minted an empty stub
  that had to be manually removed.

### Why (fixture, per the verification push)

The current `notes` smokes (`notes.smoke`, `notes-add-note`,
`notes-git-commit`, `notes-vault-selector`) mostly check the agent
*says* the right thing, not that it *writes* the right files into
a vault. The stub-fix is the ideal first behavioral fixture
because its correct behavior is an *absence of a file* — trivially
assertable, and impossible to fake with a recitation probe.

### Open questions

- **Minimal seedable vault.** What's the smallest Obsidian-shaped
  seed the fixture needs? Does the `notes-git-commit` flow need a
  local bare repo standing in as the "remote"?
- **Migration of existing stubs.** ~180 empty stubs already exist
  in the personal vault. One-shot cleanup (delete files that
  are frontmatter + `# <title>` only), or leave them and only
  change go-forward behavior? Deleting turns their inbound links
  dangling — the desired end state, but a bulk vault mutation
  worth doing deliberately, and outside this repo (it's user
  vault data, not repo content).
- **Definition of "substance."** Frontmatter + heading + one
  sentence? A link? Needs a bright line so the skill doesn't
  re-litigate on every write.
- **Retrieval impact.** The Dataview/ripgrep "list notes by topic
  X" queries key on `[[X]]` outlinks + frontmatter `topics:`, not
  file existence — confirm they still work with the topic file
  absent. (They should.)

---

## Slices 2+ · behavioral, tri-tool verification for the remaining skills

### What

Apply the pattern `agents-md-ecosystem-alignment` established for
kdevkit — recitation probes replaced with **behavioral** fixtures
that seed a scratch project, drive the agent, and assert on the
artefacts it produced, run across all three coding agents
(claude/kiro/codex) via the harness's three styles (substring /
semantic-judge / behavioral) and the `--tools` selector — to every
skill, not just kdevkit.

Currently only **kdevkit** uses behavioral fixtures
(`kdevkit-planning`, `kdevkit-agents-md`, `kdevkit-closure`). The
rest still use older shapes:

- `notes` — the remaining substring/judge smokes beyond Slice 1.
- `writing-style` — a substring smoke checking the skill announces
  itself, not that prose comes out in the intended style.

For each skill, ask "what artefact or observable change proves the
agent carried this out?" and write a setup/assert fixture, running
`tools: claude,kiro,codex`. Keep judge/substring only where a
skill genuinely has no artefact to inspect (as the dev-loop
gate-ordering reasoning does).

### Why

1. **Behavioral tests catch what recitation can't.** The
   `kdevkit-closure` fixture initially failed on claude/codex
   because they *correctly refused* an ambiguous seed — a
   recitation probe would have sailed through. Testing the
   artefact tests the purpose.
2. **Three agents ⇒ robust.** A skill that drives all three to the
   same artefacts doesn't lean on one tool's prompt-following
   quirks. The harness already supports this; only kdevkit uses it.

As skills are the sole deployed artefact (post skills-only
deploy), their verification is the whole safety net. Leaving
`notes` and `writing-style` on recitation smokes means we don't
know they *work* across the three agents — only that they load.

### Open questions

- **writing-style is prose, not an artefact.** Its output is *how*
  text reads, which resists a shell `assert`. Is this the one
  skill that legitimately stays judge-mode (semantic), or is there
  an observable proxy (e.g. "rewrite this sentence" → assert the
  em-dash/spaced-hyphen convention appears)?
- **Cost/cadence.** Every skill × 3 agents × behavioral multiplies
  the (user-driven, credit-costing) verify surface. Scope which
  skills are load-bearing enough to warrant the full matrix vs. a
  single-agent behavioral check. This is the main argument for
  promoting to an initiative.
- **Per-skill fixture count.** kdevkit collapsed 17 → 4 phase-keyed
  fixtures. notes/writing-style are smaller; likely 1–2 behavioral
  fixtures each. Confirm the phase/behavior mapping per skill at
  planning time.

---

## Trigger to promote

- The empty-stub clutter in `topics/` becomes a real drag on
  search/graph, or the user re-flags deleting an auto-minted stub
  (Slice 1's behavior trigger — already happened once).
- A vault cleanup pass is scheduled anyway (bundle the rule change
  + one-shot stub deletion).
- A verify pass on notes or writing-style surfaces a gap a
  recitation smoke missed (Slices 2+ trigger).

## Note on editing the skill

`resources/content/skills/notes/SKILL.md` (and
`writing-style/SKILL.md`) are the sources behind the managed
skills symlinks — edit them here in the repo, not under
`~/.claude/skills/`. Changes land in the next session. Fixtures
live under `resources/tests/skills/<name>.smoke`.
