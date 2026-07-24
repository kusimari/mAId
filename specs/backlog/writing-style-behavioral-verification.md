# Backlog: writing-style-behavioral-verification

## What

Bring the `writing-style` skill's verification up to the behavioral
(setup/assert, tri-tool) bar the notes and kdevkit skills now meet.
Today `writing-style.smoke` is a substring probe that only checks the
skill *announces itself*, not that prose actually comes out in the
intended style. The open design question is whether writing-style is
the one skill that legitimately stays semantic/judge-mode, or whether
there's an observable proxy worth asserting.

## Why

Skills are mAId's sole deployed artefact, so their verification is the
whole safety net. Every other load-bearing skill was moved to
behavioral fixtures (notes in this feature; kdevkit before it);
`writing-style` is the remaining recitation smoke. Leaving it there
means we know the skill *loads*, not that it *works* across the three
agents.

The hard part — and why it was split out of the notes feature rather
than bundled — is that writing-style's output is *how prose reads*,
which resists a shell `assert`. A judge (`expected_narrative:`) may be
the honest fit here, per project.md Testing's "reserve
substring/semantic for genuinely non-artefact behavior." But before
defaulting to judge-mode, look for an observable proxy: e.g. "rewrite
this sentence" → assert the em-dash / spaced-hyphen convention (or
whatever the skill's concrete typographic rules are) appears in the
output. A proxy that a no-op agent fails is worth more than a judge
call.

## Open questions

- Is there a concrete, greppable convention in `writing-style` (a
  punctuation or formatting rule) that a "rewrite this" prompt could
  make observable, or is the skill entirely about qualitative feel?
- If judge-mode is the honest answer, sharpen the
  `expected_narrative:` so it tests the *output prose*, not just that
  the skill announced itself.
- Cost/cadence: is writing-style load-bearing enough to warrant the
  full three-agent matrix, or a single-agent check?
