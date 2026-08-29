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

## Research round 2 (2026-08-29) — Thread A verified, and partly wrong

Picked up in the dedicated session this item asked for. Two
investigations ran: the runtimes' actual enforcement surfaces, and
where in this repo a code-owned mechanism would live. Findings are
recorded here as facts; the D-open-1 decision they feed is still
the user's.

### The portability asymmetry was outdated

Thread A's claim that "only Claude Code's `PreToolUse` hook can
force a call outright" no longer holds:

- **Codex CLI ships a full lifecycle-hooks system**
  (`features.hooks` in `config.toml`, or `hooks.json`) whose event
  vocabulary mirrors Claude Code's — `PreToolUse`, `PostToolUse`,
  `PermissionRequest`, `PreCompact`, `PostCompact`, `SessionStart`
  (including `source: "compact"`), `SessionEnd`, `SubagentStart`,
  `SubagentStop`, `UserPromptSubmit`, `Stop`. `PreToolUse` blocks
  deterministically via exit 2 or
  `hookSpecificOutput.permissionDecision: "deny"` — the same
  contract shape as Claude Code. Hooks require trust via `/hooks`
  and are hash-bound. Codex's own docs caution they are "a useful
  guardrail, not a complete enforcement boundary" (hosted tools
  skip the local hook path).
- **Kiro blocks on `PreToolUse`, `UserPromptSubmit` and
  `PreTaskExec`** — but only its **`command`** action type is
  deterministic. Its `agent` action injects a prompt, which is
  model compliance again and therefore not enforcement.
- **Claude Code confirmed**, more precisely: `PreToolUse` blocks
  via exit 2 (unconditional; JSON cannot override) and may
  *rewrite* a call via `updatedInput`.
- **No cross-agent standard is emerging.** The MCP spec
  (2026-07-28) adds no forcing mechanism and states plainly that
  "MCP itself cannot enforce these security principles at the
  protocol level" — tool safety is host responsibility.

So the gap is now "three similar-but-not-identical schemas, each
capable of true enforcement," not "one runtime has it, two don't."

### The limitation that actually reshapes the design

**No hook on any of the three can force a call the agent never
attempted.** They gate and deny; they do not invoke. That
undercuts Thread A's own proposal: a companion CLI "the skill
instructs the agent to invoke at phase-transition points" still
depends on the model choosing to invoke it — exactly the
behaviour measured to collapse on codex under load. Adding a hook
does not fix that on its own.

The reframe: **stop trying to force the transition call; gate the
actions the agent already wants to take.** kdevkit's phase
boundaries coincide with tool calls that are structurally
necessary anyway — `git commit`, `git push`, the merge, the spec
write. `phases/dev.md:322` rewrites the Handoff *after* the Push
Gate, so a hook on that push can read the Handoff block plus git
state and refuse an inconsistent transition. Enforcement then
lands where compliance is already unavoidable, and there is
nothing for the agent to remember.

### Where a mechanism would live in this repo

- **Workspace is two crates** — `resources/build-tool` and
  `kaimux` (`Cargo.toml:2`). Neither is a natural host: build-tool
  is a deploy-time symlink installer, not a runtime companion, and
  kaimux's domain is tmux pane bookkeeping, not spec state. A
  small third member crate is the clean fit; workspace membership
  is a one-line addition.
- **Per-agent deployment shape is already data** —
  `resources/build-tool/src/shared.rs:60-95`: `Kind::{Link,
  FanOut}` × `Agent::{Claude, Kiro, Codex}`, with Claude →
  `.claude/skills`, Kiro → `.kiro/steering/skills`, Codex →
  `.codex/skills` (fan-out). This is the existing anchor point if
  hook config ever ships alongside the skill symlinks.
- **No hooks are shipped for any agent today** — no `PreToolUse`,
  `SessionStart`, `hooks.json`, or `.claude/settings` anywhere
  under `resources/`. This is new infrastructure, not an extension.
- **The Handoff block is agent-authored markdown**, schema at
  `interviews.md:86-110`, rewritten by prose instruction at four
  points (`phases/plan.md:207`, `phases/dev.md:322`,
  `phases/review.md:186`, `phases/close.md:43`), with the always-on
  invariant at `SKILL.md:381`. Nothing parses or writes it
  programmatically — this is the surface any mechanism must own.
- **kaimux's registry is still flat**, confirmed:
  `kaimux/src/main.rs:112-113` keys `Session` on `pane_id` with no
  parent/lineage field. The initiative spec's claim stands.

### A precondition this surfaced

**The harness cannot yet measure what this item demands.**
`--stressed` is a bare boolean (`resources/build-tool/src/main.rs:53-55`)
prepending a 29-line `resources/tests/conversational-stream.txt`;
there is **no N-sample flag**. Multi-sample runs are hand-driven
re-invocation. Since this item's own rule is that whatever gets
built here must be verified the way the problem was discovered —
multiple samples, under load — repeated-sampling support in the
harness looks like a precondition to measuring any mechanism,
not a nicety.

## Research round 3 (2026-08-29) — the option space, surveyed

Five parallel investigations, at the user's direction to establish
what options exist *before* committing to a path: LLM-in-state-machine
prior art; constrained-output techniques; enforcement outside the agent
runtime; published skill-authoring guidance re-read for mechanisms;
and hook-based enforcement precedents. Findings below; no path chosen.

### The reframe: "force" was the wrong goal

Not one system surveyed forces a transition. gastown — the most
state-machine-shaped prior art — does not force `gt done`; an agent
still decides to call it. spec-kit gates nothing at all: phases are
prompt files, ordering is advisory, the convergence loop is
model-judged. What the good implementations do is **refuse to trust a
transition that observable state contradicts.**

So three goals must be separated, because only two are reachable:

1. **The right transition always happens** — unavailable in any
   mechanism found, on any runtime. Abandon it.
2. **No illegal transition is ever accepted** — reachable, and this is
   what every credible precedent actually implements.
3. **A missing transition is observable** — reachable and cheap; the
   already-noted "an unwritten handoff is an assertable signal."

### Where determinism actually comes from

Of the hardening techniques available, only two produce determinism:
a **deterministic precondition re-check** against observable state, and
a **fail-closed default**. Closed enums, abstention members, and
N-sample agreement merely make a non-deterministic input less noisy.

This yields the governing design rule: **shrink the agent's job to the
smallest residual judgement that git and the spec cannot settle.** Most
of a kdevkit transition is computable without a model — whether a
`feat|fix|refactor`-type commit exists, whether `- [ ]` boxes remain,
whether a remote branch exists, whether exactly one `## Handoff` block
is present. Every point moved out of the agent's judgement is a point
that stops depending on a model at all. A switch on an agent verdict is
defensible only when the agent picks among options code already
validated.

A trap specific to constrained decoding, from OpenAI's own docs: "the
model will always try to adhere to the provided schema," so a closed
enum **forces a pick even when no value is right**, converting "I don't
know" into a confident wrong answer. Schema conformance is additionally
void on a safety refusal (which "does not necessarily follow the
schema"), on truncation, and on unsupported schema keywords. An
explicit abstention member is therefore not optional.

### The four gateable surfaces

A phase transition can be gated in four distinct places, and no single
mechanism covers more than one:

| Surface | Mechanism class | Blind to |
|---|---|---|
| The **file write** (Handoff block rewritten wrongly, or not at all) | agent-runtime hooks (`PreToolUse`) | anything done via shell instead of the write tool |
| The **git artifact** (a commit whose state contradicts the block) | git hooks | any transition not accompanied by a commit |
| The **sequencing between phases** | wrapper CLI owning the session lifecycle | anything *inside* a long phase |
| **Publication** | server-side hooks, branch protection, CI | everything up to the push, which already happened |

The most important consequence: **a phase transition is fundamentally a
file write, not a git operation.** Git hooks — the most portable option
— gate the artifact of a transition, never the reasoning, and are blind
until the agent chooses to commit.

### Mechanism catalogue

**Agent-runtime hooks.** All three runtimes expose a
`PreToolUse`-equivalent that blocks deterministically (see round 2).
Newly found: `hooks:` can be declared **inside `SKILL.md`
frontmatter**, registering on invoke and firing for the rest of the
session — so mAId's existing markdown-symlink deploy could carry
enforcement with no new install machinery. Claude-only. Anthropic's
skills documentation endorses this for precisely our symptom: if a
skill "seems to stop influencing behavior after the first response …
use hooks to enforce behavior deterministically." Also newly found:
exit 2 on `Stop`/`SubagentStop` prevents the agent ending its turn (a
refuse-to-finish lever, distinct from blocking a call); `context: fork`
gives per-phase context isolation as a frontmatter field; Kiro's
`PreTaskExec` is blockable and fires before a spec task starts.

**Git hooks.** Runtime-agnostic, fire on the agent's own git calls.
Tested this session: a hook reading the `## Handoff` block and refusing
a commit works; hooks propagate into worktrees automatically. Ordinary
`pre-commit`/`pre-push` are skipped by `--no-verify`, but
`prepare-commit-msg` ("not suppressed by the `--no-verify` option") and
`reference-transaction` (fires on *any* ref update; tested yielding
`fatal: ref updates aborted by hook`) are not. Defeated by a
`core.hooksPath` override, and `core.hooksPath` is itself per-clone and
untracked — so a fresh clone enforces nothing.

**Wrapper CLI owning the session lifecycle.** The only mechanism that
makes phase *sequencing* structural rather than remembered, and the
natural home for "ask an agent which phase, switch on the answer."
Cheaper than it looks: `resources/build-tool/src/harness.rs:735`
already builds headless invocations for all three agents with
`Authority::{Workdir,ReadOnly}` mapped to real sandbox flags. Caveat —
its claude arm passes `--dangerously-skip-permissions`, the broad
posture the initiative spec explicitly warns against inheriting.
Schema-constrained verdicts are available on two of three:
`claude --print --json-schema`, `codex exec --output-schema`; Kiro has
event JSONL only, no schema flag — a real asymmetry against the
tool-agnostic invariant. Hermetic flags (`--bare`,
`--ephemeral --ignore-user-config`) would enforce the maker-knowledge
firewall in code rather than prose.

**Server-side / CI.** The only genuinely unbypassable option — a bare-repo
`pre-receive` was tested rejecting `git push --no-verify`. But it gates
publication only, and the repo has no `.github/workflows` at all.

**`just` gating and sentinel state.** Portable, near-free, already
precedented by the `[confirm(...)]` guards at `resources/Justfile:42`
and `kaimux/Justfile:42`. Advisory — nothing compels the agent to use
`just` over `cargo`/`git` directly.

**Sandbox / authority narrowing.** Deterministic capability denial
outside the model, but coarse — it cannot express conditional phase
logic. Useful for read-only reviewer gates, not transitions.

**Forced tool invocation (`tool_choice: required`).** The one mechanism
that truly forces a call — but **raw-API only.** Not controllable
inside Claude Code, Codex or Kiro, so it does not rescue "force the
agent to invoke the phase CLI." It only applies where our own code is
the API caller, i.e. to the advisor call.

### Prior art that has already built this

- **`superpowers-flow-enforcer`** — the closest isomorph, gating
  `init → brainstorming → planning → tdd → review → finishing` with
  state in a JSON file. Its central rule is the one this research
  converges on independently: **a phase advances only when a hook
  writes the flag, never by self-report.** Deliberate skips require a
  stated reason and are recorded in state.
- **Probity** (`nizos/probity`) — TDD Guard's multi-runtime successor,
  covering Claude Code, Codex and Copilot CLI from one binary behind an
  `--agent` flag, reading each agent's session transcript directly.
  Rules may be deterministic *or* AI-validated (`ctx.agent?.reason`).
  **Fails closed when no config is found.** Proof that a single
  implementation can serve multiple vendors — the adapters are payload
  normalisers, not separate implementations.
- **`devkit`** — inverts control: an MCP server owns sequencing
  (`devkit_start/advance/status`), on the stated principle that "the
  engine holds state — the agent doesn't self-report workflow
  position." Step types include engine-run Command (zero tokens),
  loop-with-gate, and branch.
- **TDD Guard** — Claude-only and superseded, but two transferable
  choices: state files the agent is **denied read access to** (so it
  cannot tamper with its own guard state), and LLM judgment reusing the
  host's own auth so there is no separate key or billing.
- **BMAD v6 `sprint_status.py`** — closed enum vocabulary rejected with
  non-zero exit, atomic temp-file write via `os.replace`, and
  post-write re-validation. Its own code comment states the switch
  risk exactly: orchestrators "branch on the echo, so a free-spelled
  value … would silently fall through every branch they write."
- **XState guards** — fail-closed by construction: with no fallback, a
  failing guard "produces no transition." Guards must be pure, so an
  agent verdict enters as *context a guard reads*, never as the guard.
- **Temporal** — the sharpest statement of the boundary: workflow code
  is deterministic and replayed, while "non-deterministic operations
  like API calls, LLM/AI invocations" are quarantined in Activities.

### The decisive risk against hooks: silent fail-open

Hook enforcement degrades **invisibly**, and the failure is
indistinguishable from success. The reporter's formulation is the whole
problem: *"a hook that does not fire looks exactly like a hook with
nothing to object to."* Documented, all currently open against
`anthropics/claude-code`:

- **#76897 — hooks stop firing after switching into a linked
  worktree**, not restored by `/hooks` reload, byte-identical settings,
  silent. **Substantially walked back in round 4 — see below; do not
  cite this as established.**
- **#88738, #76322** — `PreToolUse` silently stops firing mid-session
  and never recovers.
- **#79480** — project `.claude/settings.json` hooks silently not
  registered; **#76413, #85904** — listed by `/hooks` but never
  executed.
- **#89251** — under bypass-permissions mode Claude Code's *own* system
  prompt instructs the agent to "make file changes with sed, heredocs,
  or short scripts, rather than using the dedicated Read, Edit, or
  Write tools," so a `Write|Edit` matcher never fires. Default product
  behaviour, not adversarial evasion. TDD Guard's need to deny
  `Bash(echo|printf|sed|awk|perl)` independently confirms a write-tool
  matcher does not hold.
- Codex's own docs call hooks "a useful guardrail, not a complete
  enforcement boundary"; hosted tools skip the local hook path.

This **inverts the comparison with prose**: prose drift is at least
observable in the transcript, whereas a dead hook yields a
clean-looking session with no guardrail at all. Any hook-based design
therefore requires a **positive liveness assertion** — the hook writing
proof-of-execution that something later checks — rather than inferring
safety from the absence of a complaint.

### The install asymmetry

| Runtime | Can a repo ship enforcement by default? |
|---|---|
| Claude Code | **Yes** — committed `.claude/settings.json`, "recommended for team consistency" |
| Copilot CLI | **Yes** — `.github/hooks/*.json` from the default branch |
| Codex | **No** — both the feature flag and `hooks.json` live in `~/.codex/`, user scope, trust/hash-bound via `/hooks` |
| Kiro | Undocumented in the only multi-runtime precedent (Probity omits Kiro entirely) |

The runtime that cannot be enforced by default is Codex — precisely the
one measured at ~33% prose adherence. This is the central obstacle to
any hook-first answer, and it is not fixable by better authoring.

### Corrections to earlier rounds

- **The OpenAI citation was overstated.** Round 1 cited OpenAI's GPT-5
  guide as recommending instruction re-assertion "every few turns for
  exactly this reason." That advice is scoped to **Markdown formatting**
  adherence — "appending a Markdown instruction every 3-5 user
  messages" — not instruction-following generally. The measured codex
  collapse stands on its own evidence; this citation should be narrowed
  rather than relied on.
- **`.codex/skills` is correct, not a defect.** A report claimed Codex
  loads skills from `.agents/skills`, which would make
  `shared.rs:82` wrong. Verified on this machine: `~/.codex/skills`
  exists, `~/.agents` does not, on codex `0.150.1.392`. The registry is
  right for the installed version; treat the neutral-path claim as
  version-dependent.
- **`tool_choice`/forced invocation does not apply** inside a
  coding-agent CLI (round 2 implied it might).

### What is still genuinely open

- **Which surface to gate**, given no mechanism covers more than one and
  the file-write surface is both the truest and the least reliably
  observable.
- **Whether a hook-first design is defensible at all** given silent
  fail-open plus the worktree bug, without first building the liveness
  assertion that would detect it.
- **What Kiro can actually do** — absent from Probity, no schema flag,
  blocking mechanism unspecified in a single source. The tool-agnostic
  invariant cannot be assessed until this is settled, and kiro is not
  installed on this machine.
- **Whether the advisor call earns its place at all**, if the
  precondition re-check is doing the real work and the residual
  judgement shrinks to near-nothing.

## Research round 4 (2026-08-29) — measurements, and four corrections

Round 3's conclusions were tested rather than trusted. Four claims in
this file and in the initiative spec did not survive.

### Corrections

- **#76897 (hooks die in a worktree) does not reproduce, and was
  mis-weighted.** The issue is `platform:windows`, stale, zero comments,
  no maintainer response, and its reporter disclaims causation: "I did
  not have a chance to confirm hooks were firing correctly in this same
  session before `EnterWorktree` ran … so I can't rule out an unrelated
  pre-existing cause." The mechanism-identified version is **#90104**:
  hook execution runs "under a separate sandbox scope … **fixed at
  session start**," which `EnterWorktree` fails to update. That predicts
  a session *started* in a worktree is fine — which is what kdevkit
  does, and what measurement confirms.
- **The gastown anti-coordinator citation appears to be a
  mis-citation.** Three fetches found none of the quotes this repo
  relies on ("No coordinator — patrol steps + Dogs"; "the beads ARE the
  state"; "zero hysteresis by construction"). Contrary evidence: gastown
  **ships** a persistent coordinator — "The **Mayor** acts as the global
  coordinator", "Always start with the Mayor", started by `gt up`.
  Uncorroborated rather than disproven (one page unreached), but
  `specs/initiative/kdevkit-decompose-and-harden.md` D-open-2 and R1's
  "deliberately not adopted" paragraph both rest on it and must be
  re-grounded before either is cited again.
- **kaimux's "no extra parent process" is not a position on
  coordinators.** In context (`specs/feature/kaimux.md:234`) it
  describes `execvp` semantics for one wrapped pane: "The wrapper's pid
  becomes the agent's pid."
- **Round 2's "no hooks are shipped for any agent today" was wrong.**
  `kaimux` already installs a hook-driven state machine into
  `~/.claude/settings.json`; `apply_event` (`kaimux/src/main.rs:127-142`)
  maps `UserPromptSubmit|PreToolUse|PostToolUse → Working`,
  `Notification → Waiting`, `Stop → Done`. Hooks exist in this repo,
  just not under `resources/`.

### Measured: hooks in a linked worktree (claude 2.1.251.736, Linux)

Throwaway repo plus linked worktree, project-local committed
`.claude/settings.json`, 5 `claude -p` invocations (~$0.90).

| Scenario | Fired | Block effective |
|---|---|---|
| Normal repo, `Bash` matcher | yes | n/a |
| Linked worktree, session **started there** | yes | n/a |
| Linked worktree, `exit 2` | yes | **yes** — command refused |
| Linked worktree, `Write\|Edit` matcher | yes | n/a |
| Linked worktree, **`--bare`** | **no** | silently unhooked, exit 0, no warning |

**`--bare` is a confirmed silent-disable vector**, and it collides with
round 3's own suggestion to use `--bare` for a hermetic advisor call:
the hermetic invocation is precisely the one with zero enforcement.
#89251 did not reproduce in one sample (the agent chose `Write`, not a
heredoc) — but it carries `has repro`, and one sample is not a rate.
Not tested: mid-session `EnterWorktree`, which cannot be driven from
`-p`. **No liveness/heartbeat prior art exists** — both searches empty.

**Liveness assertion, concretely.** The `PreToolUse` hook appends
`{session_id, timestamp, tool}` to a state file; the phase gate refuses
any transition without fresh proof matching the current session id,
failing closed on absent or stale proof. This converts "nothing
objected" — indistinguishable from a dead hook — into "the hook
provably ran this session."

### Measured: kiro-cli 2.20.1

Present as `kiro-cli` (earlier "not installed" was a wrong probe).
Headless `--no-interactive`; resume via `-r`/`--resume-id`; authority
via `--trust-all-tools`/`--trust-tools=`; `agent
create|edit|validate|set-default`. **No schema-constrained output** —
`-f/--format json` is documented "for list commands (used with
`--list-models` and `--list-sessions`)". **No hooks in the CLI at all**,
consistent with kaimux's own finding that Kiro is observation-only
(`kaimux/src/main.rs:370-374`, and
`specs/backlog/kaimux-kiro-hook-integration.md`). So the hook path
covers two of three runtimes, not three.

**Possible deploy-target defect:** `~/.kiro/` contains both a native
`skills/` directory (populated by other tooling) and
`steering/skills → mAId`. `build-tool` deploys to
`.kiro/steering/skills` (`shared.rs:~70`), so kdevkit may be loading as
always-on steering context rather than as a progressively-disclosed
skill. Directly relevant to the context-bloat half of the initiative.
Inferred from directory layout; needs verification.

**Verified not a defect:** `.codex/skills` is correct for codex
`0.150.1.392` — `~/.codex/skills` exists, `~/.agents` does not.

### The tmux master-orchestrator sketch, assessed

`awslabs/cli-agent-orchestrator` (1.1k★) validates the shape: a
supervisor that *is* a provider CLI session, workers as full CLI
processes with native auth, covering all three targets. It specifies
neither the message channel nor completion detection.

`yohey-w/multi-agent-shogun` (1.4k★) supplies the scar tissue, and two
findings decide the design:

- **"Message content is never sent through tmux — only a short 'you
  have mail' nudge."** Files + `flock` are the bus, `inotifywait` the
  watcher; `send-keys` is a doorbell. Payloads through tmux cause
  "character corruption and transmission hangs"; Enter must be sent
  separately for Codex; nudges interrupt active turns and need
  idle-flag suppression; "Claude Code's `Stop` hook only fires at turn
  end. An idle agent … has no turn ending," so a file watcher, not a
  hook, must be the wake path. Completion is a **report file**.
- **The orchestrator is not the state holder.** Shogun delegates
  instantly; its context is wiped by `/clear` and state lives in files.

So the master must be **stateless-by-construction**, re-deriving phase
from spec + git each cycle — after which a tiny context buys cost and
latency, not correctness. What the architecture buys is **context
isolation and crash recovery, not determinism**: an LLM master choosing
transitions is the same soft classifier one pane over.

kaimux gaps for this shape: no `send-keys`, `split-window` or
`kill-pane`; `Session` has no parent field (`main.rs:111-124`); `Wrap`
is `execvp`-based (`:516-521`) so the wrapper *becomes* the agent and
cannot supervise it. Kiro fires no lifecycle events and Codex is not a
wrapper kind, so completion detection degrades to `pane-exited` plus
report-file polling on the two runtimes needing most help.

### ACP as a control plane

JSON-RPC 2.0 over stdio, protocol version `1`, client↔agent (MCP is
agent↔tools; they compose — Kiro's `session/new` takes `mcpServers`).
Genuine wins over `send-keys`: turn completion is `session/prompt`
returning a `stopReason`; typed `tool_call`/`tool_call_update` events
with `status` and `locations`; `session/load` resume; clean
`session/cancel`. Prior art exists for exactly this use —
`recailai/jockey` "coordinates Claude Code, Gemini CLI, and Codex CLI
via ACP", CompozyOS uses the approval channel as a workflow gate.

**But it is not an enforcement point.** `session/request_permission` is
a real client-answered round-trip, and the spec sanctions automatic
gating ("clients MAY automatically allow or reject"), yet permission is
`MAY` not `MUST`, the filesystem page notes nothing "forbids direct
access," and the terminal page "doesn't impose any obligation on Agents
to route command execution through the Client." So ACP relocates the
gate into our code without making it unconditional.

**Two disqualifiers for mAId specifically:** Claude Code is **not** an
ACP agent — only the Claude Agent SDK, via a Zed-built adapter, so
driving it may not run what this repo deploys; and Codex support is
also adapter-only. ACP does **not** close Kiro's missing-schema gap.

### What the four rounds add up to

No mechanism provides unconditional interception. Every candidate is
best-effort at the point of enforcement, and the three layers are
separable:

1. **Determinism lives in a precondition re-check plus a fail-closed
   default.** Pure computation over git and the spec, no runtime
   dependency, works on the bare path. This is the only layer that is
   portable *and* deterministic, and the only one that cannot silently
   fail open.
2. **Enforcement is per-runtime and best-effort** — hooks on claude and
   codex (with a mandatory liveness assertion, since silence is
   indistinguishable from a dead hook), nothing on kiro CLI, plus
   optional git hooks for the commit surface and CI for publication.
3. **Orchestration is optional and buys isolation, not determinism** —
   tmux or ACP, stateless master, files as the bus.

The layering matters because it inverts the original instinct: the
cheapest layer (1) delivers the actual guarantee, while the expensive
layers (2, 3) deliver observability and isolation.

## Research round 5 (2026-08-29) — the field, and what it breaks

Three comparisons against shipped agentic-coding systems, plus an
adversarial stress-test of the draft design. The draft does not survive
intact.

### Correction: the gastown citation was right; round 4's retraction was wrong

All four disputed quotes exist verbatim in
`docs/design/convoy/mountain-eater.md` (commit `649b832`), including
the rejected-vs-shipped table at `:481`. **Round 4's "mis-citation"
finding is withdrawn.** The reconciliation:

> "The reason single-coordinator approaches fail is **hysteresis**. Any
> agent maintaining an 'I'm driving this epic' loop will lose that
> thread at compaction."

gastown **ships** the Mayor as router/dispatcher ("Always start with the
Mayor") *and* **rejects** a coordinator that holds the progress thread.
Routing by a persistent agent is fine; remembering where we are is the
named failure mode. Their rule: "**Discover, Don't Track**." Kilo Code
independently deprecated its Orchestrator mode — "there's no need for a
dedicated orchestrator" — so two systems vote for the router living in
the always-on skill, not a separate agent.

### The missing mechanism: restrict capability, don't gate sequence

Every mode-based tool enforces **what a phase can do**, and none
enforces transition order:

| Tool | Restriction |
|---|---|
| Cline Plan | "cannot modify any files or execute commands… This constraint is intentional" |
| Roo Architect | `read`, `mcp`, restricted `edit` (markdown only) |
| Roo Ask | `read`, `mcp` only |
| Kilo `plan` | read-only plus writes confined to `.kilo/plans/` |
| Kilo `ask` | read-only plus a safe bash allowlist (`cat`, `grep`, `git log`, `jq`) |

Roo states it plainly: "No mandatory transition sequence is described —
nothing forces Architect → Code."

This is **strictly stronger than a transition gate** — it removes the
capability rather than detecting misuse — and it is portable to all
three runtimes today: `allowed-tools`/`disallowedTools` (claude),
`sandbox_mode` plus filesystem deny globs (codex), `--trust-tools=`
(kiro). **It is the only enforcement mechanism found that works on
kiro.** A plan phase that physically cannot write outside `specs/`
requires no rule recall at all.

### The invocation-compliance failure, measured

`spec-workflow-mcp` #199 is decisive against "one command is easier to
remember than five rules": an MCP *tool* — more salient than a shell
command — was skipped at effectively 100%. "Across dozens of specs,
**zero** implementation logs are being created." Root cause was
**position, not salience**: "AI agents follow numbered steps
sequentially. They execute step 6 (mark complete), consider the task
done, and never reach step 7."

A `kdevkit-state advance` placed after commit-and-push sits in exactly
that slot. **Treat the one-command claim as refuted.**

The answer, already shipped by `superpowers-flow-enforcer`: hooks
**write** state rather than only denying actions. Its `PostToolUse`
handler syncs workflow state from observed events, so nothing depends on
the agent choosing to call anything. Complementary trick: **gate the
push rather than following it**, so the observable event the agent wants
cannot happen unless state advanced — desire as the forcing function
instead of memory.

### Markdown-block-as-state: the fragile end of a real spectrum

Not categorically an anti-pattern — `Backlog.md` (6.5k★) stores task
state as markdown in git deliberately. But kdevkit's variant is the
worst shape surveyed: **one** machine field (`Phase:`) among four
free-prose fields, inside a document humans are *expected* to edit,
**replaced wholesale** (maximal git-merge conflict surface), with a
template placeholder line that enumerates every legal value — which
`kdevkit-phase-boundary.smoke` already needs a defensive assert against.

Documented harm elsewhere:

- **`Backlog.md` #860** — unlocked read-modify-write: "**7 of 8
  concurrent writes lost** pre-fix," silently, both writers reporting
  success. Fixed with a **per-checkout** filesystem lock, in-lock
  re-read, fail-fast, non-zero exit — "locks are per-checkout, so
  sibling worktrees never falsely contend." kdevkit runs features in
  sibling worktrees.
- **`taskmaster-ai`** (28k★) — races (#1567, "later writes would
  overwrite earlier ones"), repeated self-corruption (#931, #854,
  #1004), stale state after a legitimate hand-edit (#348). Now moving
  to SQLite + JSONL (#1619) citing "transactions for atomicity, WAL
  mode for concurrent access" and git-friendly line-oriented merges.
- **BMAD #2767** — `compile-epic-context` "silently drops
  reference-only requirements, so design contracts never reach the
  implementing agent." Silent loss in the very mechanism meant to
  prevent it.

Hand-editing is a legitimate need, not deviance (#348's reporter wanted
a model to review their task list), so "the CLI is the only writer" is
both unenforceable and undesirable. Drift is detectable via a recorded
hash of the block, but not attributable.

### Other adopted learnings

- **Close the transition set.** oh-my-claudecode permits exactly four
  stage orderings and "deliberately omits… arbitrary stages." A closed
  table is testable and fail-closed by construction; "any phase may
  return to any earlier phase" is a free graph.
- **Forward advance should require a human ack.** Cursor ("Click to
  build the plan when ready"), Cline and Roo all gate on a user action.
  Auto-advancing on passing facts is shipped by nobody.
- **Backward should revert, not relabel.** Cursor treats the plan as a
  re-runnable checkpoint — revert, refine, re-run, "often faster than
  fixing an in-progress agent." An audit line leaves bad work in place.
- **Make the return undischargeable by forgetting.** gastown's gate
  files one fix bead per finding with mandated sections (`## Context /
  ## Issue / ## Location / ## Expected Fix / ## Acceptance Criteria`),
  adds them as blocking dependencies — "the gate bead becomes blocked
  again until all fixes are closed" — exits without waiting, and a
  **fresh worker re-runs every review step from the top**.
- **Count returns.** Phase-thrash has zero documented hits, so it is
  untested rather than known-bad; a counter makes bouncing visible
  without capping it. Nearest cap precedent is BMAD v4's 3-failure halt.
- **Emit facts, not just a verdict.** gastown's ZFC principle: "Go
  provides transport. Agents provide cognition… **No hardcoded
  thresholds.** Expose the age as data and let the agent decide." Our
  predicates are crisp and threshold-free, which keeps them transport —
  but the JSON should carry the facts alongside any verdict, and no
  tunable threshold should ever be added.
- **Liveness has scarred gastown already.** Three heartbeat stores with
  differing thresholds caused a false stuck-agent escalation (incident
  hq-qxl9), and a healthy agent on "one very long patrol turn" reads as
  stale for hours. One store, self-reported over inferred, plus an
  explicit answer for the long-turn case.

### Research before plan: recordable, never gated

Evidence is split on phase status but unanimous on the gate. Only
oh-my-claudecode makes research a phase (`autoresearch`), bounded by a
**wall-clock ceiling** rather than a predicate. Cursor, Cline and Kilo
fold it into planning. Roo/Kilo `ask` is read-only chat — no artifact,
no exit criterion.

gastown does have a first-class pre-plan pipeline
(`mol-idea-to-plan`: intake → prd-review → human-clarify →
generate-plan → prd-align → plan-review → create-beads), bounded by a
named artifact, fixed round counts, and — the transferable part —
**exactly one human gate placed at peak ambiguity**: "There is one
human gate… After PRD review: You answer clarifying questions before
plan generation. This is the only step requiring your input."

**There is no observable predicate for "research is done."** So
research may be *recorded* (`Phase: research`, so a dying session
resumes) but must never be *gated* by code. This session is the
supporting case: five parallel investigations produced five corrections
to checked-in specs and one measured experiment, and deliberately ended
without a plan.

### Where the design now stands

Ranked changes, from the stress-test:

1. **Hooks write state, not merely deny it** — `PostToolUse` observes
   the commit/push and advances the block in code, removing invocation
   compliance from the critical path on claude and codex.
2. **Move machine state to a line-oriented, per-checkout-locked
   sidecar; keep the block for judgment.** `Carry forward` and
   `Deliberately left` stay human prose; `Phase` moves out. Atomic
   write, in-lock re-read, fail-fast on contention, and cross-check
   sidecar against block with fail-closed on disagreement — which also
   detects hand-edits.
3. **Reposition the call before the terminal-feeling act** — gate the
   push rather than following it — **and count returns.**

Plus the capability-restriction layer, which is new and is the only
enforcement that reaches kiro.

## Trigger to promote

A dedicated session — the user has explicitly deferred this to
"a separate session to solve for the problem," not a continuation of
the current one. **Rounds 2–4 above were that session's research; the
option space is surveyed, measured, and four earlier claims are
corrected. No path is chosen.** The next step is D-open-1: which
surface to gate, and whether layer 1 ships on its own first.
