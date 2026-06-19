# Backlog: kdevkit-feedback-repin-loop

## What

A kdevkit rule that re-applies the **stepping-back rituals** (pin to
`project.md`, survey what already exists, reach for the right owner /
idiom, decide the experience before the implementation) when a change is
triggered by **PR/CR feedback or a verify finding** — not only at the
planning phase.

Today kdevkit's altitude checks are gated on *phase*: the four interviews
and the "Reach for what exists" / requirements-smell-test discipline fire
during §6 Planning. Once the branch is in the dev loop and a change is
prompted by review feedback or a `/verify` finding, the agent drops into
*fix mode* — smallest diff that turns the red thing green — and the
stepping-back never re-fires. The gate is keyed to **when** (planning)
instead of **what** (a design decision is being made).

The fix: a feedback-triggered change that **introduces, moves, renames,
or re-scopes a component** is a design decision and must re-run a short
re-pin check before the fix is implemented, regardless of phase.

## Why

Observed directly in the `agents-mcp-readiness` feature (initiative
`env-rebuild-separation`, Stream 3). A slack-mcp regression was traced to
`./exec` having dropped a `~/.aim/mcp-servers` PATH entry. The agent
restored it **in `./exec`** (the point of failure), verified slack worked,
and stopped. Only on the user asking "shouldn't that PATH change happen
in L4, since L4 installs aim?" did the correct owner surface — and the
agent **already had every fact needed** in context (it had read
`layer-4-kelasa.sh` several times; knew L4 installs `aim` and owns the
`~/.toolbox/bin` PATH wiring). The test passing **ended the
investigation** — a false-PASS-of-design: functionally correct,
architecturally misplaced.

Across that session, nearly every altitude correction (flake→mise, the
manager/runtime boundary, node-not-global, the `./exec` entry-point
shape, aim-PATH→L4, uninstall symmetry) came from user feedback, not from
the agent stepping back on its own. A feedback-triggered re-pin would
have caught the **project-internal** ones (owner placement, boundaries,
symmetry) without any external nudging.

**Honest scope limit (why this isn't a silver bullet):** some corrections
needed *ecosystem* knowledge a re-pin can't surface — "AIM teams use
mise" and the AWS gnupg guidance live in org wikis, not in `project.md`
or the codebase. A re-pin check validates against the *project's own*
design; it would not have produced those. But it would have collapsed an
estimated large fraction of the back-and-forth, and the slack/L4
round-trip specifically would not have needed the user. The rule should
claim the project-internal win and not over-promise the ecosystem part.

## Shape (for promotion to a feature)

- **Trigger.** A dev-loop change whose origin is CR/PR feedback or a
  verify finding, AND that does more than a local edit — specifically:
  introduces a new file/component, **moves or renames** one, changes
  **where** something is installed/owned, or alters a public contract.
  Pure local bugfixes (off-by-one, a wrong string, a missing guard) do
  **not** trip it — the goal is to catch *displaced design*, not to
  ritualize every fix.
- **The re-pin check (short, inline — not the full four interviews):**
  1. **Owner.** Does `project.md` already name a layer/module/repo whose
     responsibility this change falls under? (The L4 case: "L4 installs
     aim and owns the toolbox PATH wiring" was right there.) Put it
     there, not at the point of failure.
  2. **Altitude.** Is this fix at the right tier — or am I patching a
     symptom one level below where the cause lives?
  3. **Reuse / idiom.** Does an existing mechanism already do this (a
     PATH block, an install verb, a manager) that the fix should extend
     rather than duplicate?
  4. **Symmetry.** If the change adds an install/create/enable, is the
     inverse (uninstall/delete/disable) covered? (The uninstall gap was a
     verify finding, not a design step.)
- **Cost guard.** The check is a few lines of reasoning surfaced in the
  Session/Decision Log, not a phase gate — it must not turn every review
  comment into a re-planning ceremony. If the four questions are all
  trivially "yes, this is the right spot," it leaves one log line and
  proceeds.
- **Where it lives in SKILL.md.** §7 dev loop (the Code Review Gate loop
  back) and §8 closure feedback both currently send the agent straight
  to "implement the fix." Add the re-pin check as the first step of
  *acting on* a finding, before the fix is written. Cross-reference the
  existing "Reach for what exists" (design-time) and the requirements
  smell test — this is those same disciplines, re-fired on the
  feedback path.

## Open questions

- How to classify "design decision vs local fix" cheaply enough that the
  agent reliably self-triggers? A short heuristic (introduces/moves/
  renames/re-scopes/changes-a-contract) may be enough; needs wording that
  doesn't snag trivial fixes.
- Should this also fire for the **author's own** mid-dev changes (not
  just external feedback)? The slack/L4 fix was self-initiated from a
  verify finding — so probably yes: the trigger is "a change is being
  made reactively," not "feedback arrived from outside."
- Does the re-pin belong in `SKILL.md` (always-on) or as a deferred
  checklist loaded only when the trigger fires? Leans always-on but
  terse, per the skill-file placement rule.
