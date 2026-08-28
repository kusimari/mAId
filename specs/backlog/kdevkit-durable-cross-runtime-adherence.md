---
name: kdevkit-durable-cross-runtime-adherence
description: A skill's prose-only instructions do not reliably survive a long/loaded session on every target coding-agent runtime — confirmed empirically on codex (GPT-5-family), which reverted to its pre-fix failure rate under conversational load even after a prose fix that worked in a fresh single-shot read. Two independent research threads (a structural/code-owned-state investigation, and a published skill-authoring guide) both point toward moving durable state and enforcement out of prose and into mechanism, but neither has been synthesized into a decision or a design yet. Captures the problem and both threads for whoever picks this up next.
metadata:
  type: backlog
---

# kdevkit — make skill adherence durable across a long/loaded session, on every target runtime

## The problem

kdevkit (and the review-briefing skill it dispatches, `kreviewkit`)
are markdown "skill" instruction files an AI coding agent reads and
is expected to remember and self-apply — phase transitions, gates,
Handoff-block rewrites, all of it prose the agent must recall
correctly, with nothing in code enforcing it. This works well on
Claude Code. It does not reliably hold on OpenAI's Codex CLI
(GPT-5-family), and the gap gets *worse*, not better, exactly when
kdevkit's own design most needs it to hold: across a long or resumed
session.

**What was actually measured**, not theorized:

- `kreviewkit`'s ~300-line contract, read fresh and asked to recite
  itself, was followed correctly by codex only ~50% of the time
  across 5–6 samples (claude and kiro: ~100%). A single sample had
  originally looked clean — the multi-sample check is what surfaced
  this, per the project's own "one sample proves nothing" testing
  rule.
- A prose fix — a non-negotiables checklist near the top, named tags
  around the three specific points codex kept dropping, and a
  terminal self-check — raised codex's *unstressed*, fresh-context
  rate to 80%, with no regression on claude/kiro. This fix is shipped
  (see `resources/content/skills/kreviewkit/SKILL.md`) and is a real,
  worthwhile improvement on its own.
- The same fixed file, tested with the build-tool's existing
  `--stressed` flag (prepends ~4.6KB of unrelated prior conversation,
  simulating a loaded session — a capability that existed in the
  harness but had never actually been exercised by any fixture until
  this investigation), **collapsed back to ~33% on codex** — reverting
  to the exact same failure mode as before the fix (describing the
  contract as caller-defined rather than the skill's own binding
  requirement). Claude held ~100% stressed. Kiro showed a minor,
  inconclusive dip.
- A one-sample spot-check of the three kdevkit A/B fixtures
  (consolidation-skip, handoff-resume) under `--stressed` was mostly
  clean (3/4), but the one kiro failure showed a *different* shape —
  not a dropped rule, but over-caution (stopping to ask two
  clarifying questions it correctly didn't ask unstressed). One
  sample is not a rate; this is flagged as undone work, not a
  finding.

**The conclusion the data supports:** no amount of prose scaffolding,
authored carefully or not, reliably survives conversational load on
codex. This isn't a codex bug or a loading defect — codex loads a
directly-invoked skill's full body, untruncated. It's a genuine
model-family instruction-decay characteristic, independently
documented in OpenAI's own GPT-5 prompting guide (which recommends
*re-asserting* instructions every few turns for exactly this reason)
and consistent with the general "lost in the middle" long-context
literature. Anthropic's own Agent Skills documentation says the same
thing about Claude, in fact: skill content does not automatically
survive context compaction and must be deliberately re-invoked or
enforced outside the model's memory — Claude just happens to be far
more resistant to *ordinary* conversational load before compaction
forces the issue.

## Why this matters specifically for kdevkit

kdevkit's entire premise is a workflow that spans sessions — that is
what the checked-in `## Handoff` block exists for. Unlike
`kreviewkit`, which mandates a fresh context on every invocation by
its own contract and is therefore *structurally* insulated from this
problem, kdevkit is the skill most exposed to exactly the failure
mode this investigation found. The three kdevkit A/B fixes shipped
in stream 6 were verified fresh-context only; whether they hold
across a genuinely long or resumed session — the normal case for
kdevkit, not the edge case — is unverified, not just under-sampled.

## Two independent investigation threads, not yet synthesized

Two separate lines of research were run in response to this finding.
Neither has been turned into a decision or a design — that is
deliberately left for whoever picks this up, in a separate session,
with room to evaluate both (and anything else that turns up) on their
merits rather than inheriting a conclusion reached under time
pressure.

### Thread A — move durable state and enforcement into code

Externally: confirmed prior art exists for exactly this shape of
problem. Claude Code, Codex CLI, and Kiro each have a hooks
mechanism (`PreToolUse`, `SessionStart`/compact-matched re-injection,
etc.) that can block an action or re-inject context deterministically
— outside the model's memory entirely — but the three runtimes'
hook schemas are not identical and hooks are not confirmed portable
as-is. The one mechanism confirmed to work identically across all
three today, with zero per-runtime integration, is plain shell/exec:
a small companion CLI the skill instructs the agent to invoke at
phase-transition points, which would own the state machine (read the
Handoff block + git state, compute the phase, refuse illegal
transitions, rewrite the Handoff itself) rather than asking the agent
to recall and self-apply it. Anthropic's own engineering writeup
(`anthropic.com/engineering/building-effective-agents`) independently
endorses this class of fix — they cite forcing absolute-filepath tool
calls over path-memory as the resolution to an analogous "the model's
memory of state is unreliable" problem. MCP was also investigated as
a transport for the same idea (useful for stream 5's
cross-agent/session-orchestration case specifically) but does not, on
its own, force invocation any better than prose does on Codex/Kiro —
only Claude Code's `PreToolUse` hook can force a call outright.

Internally: the repo's registry/deploy architecture (`build-tool`)
already models per-agent deployment shape (`Kind::Link` for
claude/kiro's whole-directory symlink vs. `Kind::FanOut` for codex's
per-skill symlinks), so a codex-specific override, if ever needed,
is a small, contained change for codex specifically — converting
claude or kiro to the same per-skill shape would be a much bigger
change against this repo's stated "one canonical source" principle.

### Thread B — published skill-authoring guidance (external)

`stephanmiller.com/the-agent-skills-guide-i-wish-id-had` (fetched and
summarized 2026-08-28) independently arrives at overlapping and
additional patterns, worth weighing against Thread A rather than
assuming either is complete on its own:

- **Keep skill bodies thin; push bulk content to on-demand
  references/scripts.** A loaded skill body "sits in context until
  the session ends or hits a compaction boundary" — bulk content
  competes for the same decaying attention this investigation
  measured. This is a different lever than the checklist/self-check
  fix already shipped: shrink the always-loaded surface rather than
  reinforce it.
- **Write instructions as judgment/intent, not rigid numbered steps**,
  so a model that loses track of exact sequence can still recover by
  applying the underlying goal — a hedge against step-drop rather
  than a fix for it.
- **A living "gotchas" section**, appending one-line corrective rules
  the moment a real failure is observed, rather than trying to
  anticipate everything up front.
- **A strict "one hop" rule for references** — if `SKILL.md` points
  to a file that points to another file, "the agent will half-read
  the chain, lose the thread, and miss things." (This matches
  Anthropic's own published Skills best-practices, found in the
  earlier research thread too — corroboration, not a new claim.)
- **Scripts for anything that must be deterministic**, since "only
  its output enters the context window, not its source" — the
  model can't misremember logic it never has to hold in its head at
  all. This is the same conclusion as Thread A's CLI recommendation,
  reached independently from a different angle (skill-authoring
  practice, not agent-reliability research).
- **Skill-scoped hooks**, explicitly named as the mechanism that lets
  "the rule live and die with the skill instead of polluting every
  session" — Claude-Code-specific, with the same portability caveat
  Thread A found (the article itself notes other agents "must fall
  back to bundled scripts and always-on instructions instead").
- **Treat a skill's reliability as decaying over time, not fixed
  after one eval pass** — "A skill that passed its evals on Tuesday
  is not a skill that's still good in a month" — and mine real
  session transcripts for recurring failures rather than relying on
  a one-time test. Explicitly called an interim measure by its own
  author, not a finished answer.

## What's genuinely open

- **Thread A and B converge on "move deterministic logic into
  scripts/CLI" but diverge on how much prose restructuring (thin
  bodies, judgment-over-steps, gotchas) can independently help before
  reaching for mechanism.** Untested against this project's own
  stress harness — worth measuring rather than assuming either
  article/research thread's framing transfers directly.
- **No synthesis or design decision has been made.** This is
  deliberately a backlog item, not a design doc — the next session
  should treat both threads (and anything else it finds) as inputs to
  weigh, not a conclusion to implement.
- **The N-sample + `--stressed` regime itself is unfinished
  process, not just a one-off finding.** Whatever gets built here
  should be tested the same way it was discovered — multiple samples,
  under simulated load — not trusted on a fresh single-shot pass the
  way the first "fix" was, before the user's own insistence on
  sampling and stress-testing caught that it wasn't actually fixed.
- **The three kdevkit A/B fixes' stress-tolerance is spot-checked,
  not verified** (one sample each, mixed/inconclusive result) — real
  work, not yet done, whichever direction this backlog item resolves.
- **Initiative context**: this directly feeds `kdevkit-decompose-and-
  harden`'s D-open-1 decision (the code-vs-prose boundary blocking
  streams 4–5, "deterministic-phasing" and "session-orchestration").
  The empirical finding here is evidence for resolving that boundary
  toward code, but the *design* of what code, and how much, is
  exactly what this backlog item leaves open.

## Trigger to promote

A dedicated session — the user has explicitly deferred this to
"a separate session to solve for the problem," not a continuation of
the current one.
