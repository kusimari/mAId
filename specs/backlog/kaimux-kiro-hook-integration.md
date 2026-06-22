---
name: kaimux-kiro-hook-integration
description: Real Kiro lifecycle hook integration — close the v1 deferred slice so wrapped Kiro panes drive the dashboard's state machine the same way Claude does.
metadata:
  type: backlog
---

# Kaimux: real Kiro hook integration

## What

Wire Kiro's lifecycle events into the kaimux state machine so
wrapped `kiro-cli` panes go through Working / Waiting / Done
the same way Claude does. Today (v1) Kiro is observation-only:
the pane registers, but no events fire because the Kiro hook
shape isn't wired.

## Why deferred from v1

The agent-orch design that became kaimux v1 wrote a
Claude-shape JSON to `<cwd>/.kiro/agents/kaimux.json`, which
Kiro logged as `invalid agent config` on every prompt — so
the v1 narrative tightened to "Kiro is observation-only" and
PR #26 dropped the file write entirely. That fixes the noise
but leaves the broader integration unfinished.

## What needs to happen

Three open questions:

1. **Injection point.** Kiro hooks live inside agent persona
   JSONs. Two paths under consideration:
   - Merge the kaimux hooks into the user's chosen default
     agent persona (invasive — modifies user content).
   - Ship a project-scoped stub persona under
     `<cwd>/.kiro/agents/kaimux.json` and use it.
   - Or a third we haven't surfaced.
2. **Event shape.** Kiro events are camelCase and the
   container is inline rather than the matcher-array shape
   Claude uses. Need to confirm the exact schema against
   current Kiro docs / source.
3. **Functional coverage.** The F1–F8 fixture already wraps
   a real `kiro-cli` and observes that the row registers + the
   kind column is correct. With hooks wired, F4.3-style
   assertions on Kiro's lifecycle should pass without the
   "Kiro hooks are out of scope in v1 — that's expected"
   carve-out.

## Why this is the right next slice after v1 lands

The whole kaimux design pivots on the lifecycle state machine
driving the picker's triage sort (waiting > done > idle >
working). With Kiro observation-only, mixed-fleet users see
Kiro panes stuck in Idle even when they're actively working —
so the dashboard's value prop ("which agent needs attention")
underdelivers for them. This slice closes that gap.

## Out of scope

Whatever the deeper Kiro hook story is (custom event types,
multi-stage tool runs, cancellation semantics) — start with
the four-event minimum that maps to the existing State enum.

## Provenance

Tracked through PR #24 review as the deferred Kiro drift
item; explicitly named in `specs/feature/kaimux.md` →
"Open issues deferred to post-PR-#24 slices" and the
follow-up section "Kiro state tracking". PR #26 closed the
spec ↔ code drift (drop the bogus write); this item closes
the actual functional gap.
