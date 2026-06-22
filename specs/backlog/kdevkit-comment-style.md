# Backlog: kdevkit-comment-style

## What

A kdevkit convention for **code comments**: a comment captures *intent*
— the why that can't be read off the code — and stays terse. It does not
restate what the code already says, and does not narrate the decision
trail or the bug that led to the change. Where intent needs an external
reference (an upstream doc, a sibling module, a standard), a short
pointer is fine; an elaboration is not.

Rule of thumb:
- **Write** the reason this exists / the non-obvious constraint /
  the gotcha a future reader would trip on.
- **Don't write** a paraphrase of the line below it, the history of how
  we got here, or the alternatives we rejected — those belong in the
  commit message / PR / Decision Log.
- **External refs:** a terse pointer (`see project.md "<section>"`,
  "AL2023 minimal GnuPG lacks gpg-agent") yes; a retelling of the
  source's content, no.

## Why

Observed in `env-rebuild-separation` Stream 3. Comments accreted
decision-history and bug-narrative that belonged in the commit/PR:

- `# nix equivalent of AWS's "swap gnupg2-minimal → gnupg2-full"` —
  decision provenance, not intent.
- `# ...which breaks mise's GPG verification of runtime downloads` —
  the bug that led here, i.e. *why we changed it*, not what the line is.
- An `./exec` header that retold the retired wrapper, the GLIBC_TUNABLES
  tweak, the mAId-`just` analogy, and the why-not-cd-hook reasoning —
  several paragraphs of how-we-arrived narrative above ~10 lines of code.

The trimmed forms keep the intent and drop the story, e.g.:
- `# Full GnuPG: AL2023's minimal build lacks gpg-agent, which mise needs`
  `# to verify runtime downloads.`
- `# Global manager only; workspaces pin their own runtimes (project.md).`

This pairs with the existing Conventional Commits rule ("Body explains
*why*; the diff is authoritative for *what*") — the **commit** carries
the decision trail; the **comment** carries only the present-tense
intent. Same separation, applied to inline comments.

## Shape (for promotion to a feature)

- Add a short "Comment style" bullet to `SKILL.md` §9 cross-cutting
  rules (always-on), next to Conventional Commits — both govern where
  prose about a change lives. One or two lines: intent over restatement,
  terse, history → commit/PR, external refs as pointers not retellings.
- Optionally fold a one-line cue into the §7 dev-loop "Write for intent"
  section, which already governs how code reads — comments are part of
  that legibility.
- Keep it guidance, not a gate: the Code Review Gate can note a comment
  that narrates history, but it shouldn't hard-stop on comment phrasing.

## Open questions

- Is this its own §9 bullet, or a clause appended to the existing
  Conventional Commits / "Write for intent" rules? (Leans: a short
  standalone bullet that cross-references both.)
- Does it need a worked before/after example in `SKILL.md`, or does the
  one-line statement suffice? (Examples bloat the always-on file; lean
  toward a tight statement, with the example living here in the backlog.)
