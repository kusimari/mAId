# Backlog: kdevkit-functional-style-and-idiomatic-libs

## What

kdevkit should require Design and Dev to **frame each
function around user intent** ("what would a user logically
think needs to be done"), then **reach for the language /
framework / community-idiomatic library that already does
the job** rather than reimplementing logic by hand. The
default style is functional and fluent — chains over
mutable accumulators; library calls over hand-rolled state
machines — unless a typed pattern-match is genuinely the
honest tool for the case.

## Why

The drift this guards against, repeatedly seen in feat/
resources-and-kaimux: design declares "validate
frontmatter," dev writes a 50-line hand-rolled YAML-ish
parser. The user's mental model was *"load YAML, check
fields exist."* The honest implementation is two library
calls and a struct. The mismatch is invisible at planning
time and only surfaces at code review when a reader asks
"why is this so much code for so little work."

The same pattern hits filesystem walks (hand `read_dir`
loops vs. `walkdir` or known-glob iteration), state
machines (mutable counters vs. `Iterator::filter`/
`partition`/`count`), and error accumulation (early-return
chains vs. `Vec` of problems collected once). Each surface
gets re-derived per feature unless kdevkit names the
default.

## Open questions

- **Where in kdevkit does this land?** Probably §6 (Design
  interview) gets a line about "name the library or idiom
  before designing the function," and §7 Code Review Gate
  gets a finder angle for "is this hand-rolling something
  a library does?". Or one new section between §6 and §7.
- **How prescriptive?** Strong rule ("never hand-roll if a
  library exists") will hit cases where the dep is heavier
  than the hand-roll (xshell vs. duct earlier in this
  thread). Probably a heuristic + a "name the alternative
  you considered" Decision Log entry, not a hard ban.
- **Language scope.** The principle is language-agnostic
  but the *idioms* differ: Rust's `Iterator::partition` /
  serde-as-schema, Python's `dataclasses` / `pathlib`,
  Go's stdlib-first, etc. kdevkit currently doesn't carry
  per-language guidance — does this addition force that,
  or stay generic?
- **Interaction with V-model.** §6's V-model framing
  separates Requirements (what user observes) from Design
  (how). This rule sits in Design, but its trigger is the
  *Requirements-side mental model* of the function. Worth
  threading through both sections or keeping it in Design.

## Trigger to promote

- A code review surfaces "this could have been a library
  call" or "this hand-rolls X" three or more times across
  features. The pattern is then established enough to
  encode.
- Someone (probably the user) adds the rule manually to
  project.md's Agent Development > kdevkit block,
  effectively making the prompt a one-off — at which point
  it should graduate into kdevkit so every project gets
  it.
