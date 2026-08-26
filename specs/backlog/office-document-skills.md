# Backlog: office-document-skills

## What

Make spreadsheet / presentation / document authoring available in every
coding agent mAId deploys to, by registering the vendor's published
document skills rather than vendoring them.

The vendor publishes `xlsx`, `pptx`, `docx` and `pdf` skills in a public
repo with a plugin marketplace at its root. Registering that marketplace
is one verb; the skills then load in claude, and the same content is
reachable by the other harnesses through their own skill paths.

Shape follows the browser MCP, which `project.md` already calls out as
"the one resource that isn't a skill": mAId owns the **registration
verb** and supplies the **runtime** from its own flake, while the vendor
owns the content. Concretely:

- `resources::install-document-skills [agent]` — register the vendor
  marketplace with each harness, following the existing
  `<action>-<resource-kind>` + target-selector pattern.
- Add the Python libraries the skills expect to the repo flake, the way
  `nodejs_22` is already there for the browser MCP.

## Why

A GUI agent runtime was evaluated as a second surface for this repo's
skills (see the abandoned `install-strategies-and-claude-desktop-target`
work, tag `abandoned/claude-desktop-skill-wiring`). The conclusion was
that it lost to the terminal for every skill this repo owns — same
models, fewer permissions, copied content instead of live edits.

Its **one** genuine advantage was document artefacts: real `.xlsx` with
working formulas, `.pptx`, formatted `.docx`. Investigation showed that
advantage is not a property of that runtime at all — it is the same
published skills plus a sandbox that happens to have their dependencies
preinstalled. So the capability is portable, and porting it removes the
last reason to keep a second runtime in the loop.

The reviewing UX (opening the result in Excel or Keynote) is already
acceptable, so an in-app preview pane is not a reason to prefer the GUI.

## Open questions

- **Licensing decides whether registration is the only option.** The
  four document skills are **source-available, not open source** — the
  `xlsx` skill's frontmatter reads
  `license: Proprietary. LICENSE.txt has complete terms`, while the rest
  of that repo is Apache 2.0. This repo is public, so vendoring the
  content is not a step to take casually. Registering a marketplace
  sidesteps it entirely; confirm before considering any copy-based
  approach.
- **LibreOffice is the heavyweight dependency.** `xlsx` shells out to
  `soffice` for formula recalculation. Not installed here, and nixpkgs
  LibreOffice on `aarch64-darwin` has historically been painful to
  build — so this may belong in the env layer as a cask rather than in
  mAId's flake. Worth checking before committing either way.
  Mitigation: recalculation is the *only* thing it is needed for, so a
  first increment can skip it and see whether that path is ever hit.
- **Python dependencies.** The skills expect `openpyxl`, `pandas` and
  `markitdown` preinstalled and say explicitly not to `pip install`
  first — true inside a vendor sandbox, false on a developer machine.
  None are present here. Adding them to the flake keeps the capability
  self-contained (the browser MCP precedent), but they are heavier than
  `nodejs_22`.
- **Which harnesses actually honour a marketplace.** Verified for
  claude. kiro and codex discover skills at their own paths, so whether
  a marketplace registration reaches them, or whether they need the
  skills present as files, is unverified — and it decides whether one
  verb covers all three or the targets diverge.
- **Verification shape.** These skills produce binary artefacts, so the
  existing text-fixture harness (`resources/tests/run`) cannot assert on
  output the way it does for markdown skills. Probably a smoke check
  that a generated file opens and contains an expected cell/slide,
  rather than a full fixture.

## Notes

Investigated 2026-08-26. The vendor's own GUI runtime bundles these same
four skills internally, which is what confirmed they are ordinary skills
rather than a runtime feature. Several internal capability packages
re-publish them near-verbatim, so a corporate install path also exists if
the public marketplace turns out to be awkward — but the public one is
correct for this repo, which is machine- and site-agnostic.
