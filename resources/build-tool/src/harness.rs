//! Driving a coding agent against a skill and scoring the reply. Used by
//! the check and smoke stages, owned by neither — the two are the same
//! mechanism pointed at different skill sources, so this sits beside
//! them rather than inside either.
//!
//! ── Two axes, five kinds of skill test ───────────────────────────────
//!
//! How the skill is reached:
//!   explicit — the prompt names the skill and the path it lives at.
//!              Isolates content: a failure means the skill is wrong,
//!              not that it failed to load. Needs no install, because
//!              the path can name the checkout.
//!   implicit — the prompt states only the task. The agent must find and
//!              load the right skill itself, competing against every
//!              other installed skill for a capped, shared description
//!              listing — which only exists once deployed.
//!
//! What is verified:
//!   announce — the skill's self-announce marker appears, proving the
//!              file was read rather than the answer improvised.
//!   recite   — the skill plays back the contract it was designed for
//!              (its guardrails, refusals, absence paths).
//!   perform  — the skill enacts that contract (actually does the
//!              thing), asserted against artefacts where it leaves any.
//!
//! The kinds are those axes composed, and the reach axis is also the
//! install boundary — which is why each kind names its owning stage:
//!
//!   activation   explicit + announce  pre-install   reachable and parseable?
//!   discovery    implicit + announce  post-install  does its description trigger?
//!   playback     explicit + recite    pre-install   does it state its rules right?
//!   enact        explicit + perform   pre-install   loaded, does it do it?
//!   integration  implicit + perform   post-install  does it fire AND do it?
//!
//! `playback + implicit` is deliberately empty — reciting rules is not a
//! task a user phrases implicitly, so the cell has no natural test.
//!
//! `activation` and `discovery` are GENERATED from a fixture's `skill:`
//! field, so no fixture authors them and none writes a skill path. That
//! is what keeps the per-agent paths in the registry instead of
//! hand-copied into every prompt, where they had already drifted.

use crate::shared::Agent;
use anyhow::{anyhow, Result};
use std::fmt;
use std::path::Path;

// ─────────────────────────────────────────────────────────────────
// Kinds — the two axes, and which stage owns each composition.
// ─────────────────────────────────────────────────────────────────

/// How a prompt reaches the skill. This is also the install boundary:
/// an explicit prompt carries a path, so it can point at the checkout;
/// an implicit one needs the deployed listing to compete in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reach {
    Explicit,
    Implicit,
}

/// Which pipeline stage runs a kind. Derived from `Reach`, never set
/// independently — the boundary is the reach axis, so making this a
/// separate fact would let the two disagree.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    /// Pre-install: reads the skill from the checkout, mutates nothing.
    Check,
    /// Post-install: reads the deployed tree, requires `install` to have run.
    Smoke,
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Stage::Check => "check",
            Stage::Smoke => "smoke",
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Activation,
    Discovery,
    Playback,
    Enact,
    Integration,
}

impl Kind {
    pub const ALL: &'static [Kind] = &[
        Kind::Activation,
        Kind::Discovery,
        Kind::Playback,
        Kind::Enact,
        Kind::Integration,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Activation => "activation",
            Kind::Discovery => "discovery",
            Kind::Playback => "playback",
            Kind::Enact => "enact",
            Kind::Integration => "integration",
        }
    }

    pub fn parse(token: &str) -> Result<Kind> {
        Kind::ALL
            .iter()
            .copied()
            .find(|k| k.name() == token)
            .ok_or_else(|| {
                anyhow!(
                    "unknown kind {token:?} (known: {})",
                    Kind::ALL
                        .iter()
                        .map(|k| k.name())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
    }

    pub fn reach(self) -> Reach {
        match self {
            Kind::Activation | Kind::Playback | Kind::Enact => Reach::Explicit,
            Kind::Discovery | Kind::Integration => Reach::Implicit,
        }
    }

    /// The owning stage, read off `reach` so the two cannot disagree.
    pub fn stage(self) -> Stage {
        match self.reach() {
            Reach::Explicit => Stage::Check,
            Reach::Implicit => Stage::Smoke,
        }
    }

    /// True when the kind asserts only the announce marker. Such a kind
    /// is meaningless for a skill that declares no marker: nothing in
    /// the reply would distinguish "the skill fired" from "the model is
    /// capable", so those skills prove themselves by artefacts instead.
    pub fn announce_only(self) -> bool {
        matches!(self, Kind::Activation | Kind::Discovery)
    }
}

// ─────────────────────────────────────────────────────────────────
// Fixtures — the .smoke format.
//
// A fixture carries only what is specific to it: which skill, which
// agents, and one section per thing being verified. Never a path, never
// a "load the skill" preamble — those come from the registry and the
// prompt envelopes, so they can't drift per fixture.
//
//   skill: <skill-name>          required
//   tools: claude,kiro,codex     agents to run (default claude)
//   --- playback ---             optional; explicit recitation test
//   task: <the question>
//   expect: <narrative the judge scores against>
//   --- enact ---                optional; drives enact + integration
//   task: <the imperative task, phrased as a user would>
//   expect: <narrative>          for prose skills with no artefact
//   expect_substr: <string>      optional extra literal check
//   --- setup ---                optional; seeds a scratch workdir
//   <shell>
//   --- assert ---               optional; inspects the workdir after
//   <shell>
// ─────────────────────────────────────────────────────────────────

/// One `task:`/`expect:` section of a fixture.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Section {
    pub task: String,
    pub narrative: Option<String>,
    pub substr: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Fixture {
    /// Fixture basename without `.smoke` — the selector a user passes.
    pub name: String,
    pub skill: String,
    pub agents: Vec<Agent>,
    pub playback: Option<Section>,
    pub enact: Option<Section>,
    pub setup: Option<String>,
    pub assert: Option<String>,
}

impl Fixture {
    /// Parse a `.smoke` file. `name` is the basename without extension;
    /// every malformed shape is an error naming the fixture, since a
    /// silently-skipped fixture reads as a pass.
    pub fn parse(name: &str, body: &str) -> Result<Fixture> {
        let header = header(body);
        let skill = field(&header, "skill")
            .ok_or_else(|| anyhow!("fixture {name} malformed (missing skill:)"))?;

        // Default claude rather than every agent: a fixture that says
        // nothing about tools has only been thought about for one.
        let agents = match field(&header, "tools") {
            None => vec![Agent::Claude],
            Some(list) => list
                .split(',')
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(|t| {
                    Agent::parse(t)
                        .map_err(|e| anyhow!("fixture {name}: unknown tool in tools: field — {e}"))
                })
                .collect::<Result<Vec<_>>>()?,
        };
        if agents.is_empty() {
            return Err(anyhow!("fixture {name} malformed (tools: lists no agents)"));
        }

        let setup = block(body, "setup").filter(|s| !s.trim().is_empty());
        let assert = block(body, "assert").filter(|s| !s.trim().is_empty());

        let playback = section(name, body, "playback", false)?;
        let enact = section(name, body, "enact", assert.is_some())?;
        if playback.is_none() && enact.is_none() {
            return Err(anyhow!(
                "fixture {name} malformed (needs a --- playback --- or --- enact --- section)"
            ));
        }

        Ok(Fixture {
            name: name.to_string(),
            skill,
            agents,
            playback,
            enact,
            setup,
            assert,
        })
    }

    /// Which kinds this fixture yields — the mapping the run loop used to
    /// carry. A `playback` section implies playback; an `enact` section
    /// implies enact *and* integration (same task, explicit then
    /// implicit); the `skill:` field alone implies the two generated
    /// kinds, which is why they need no section.
    pub fn kinds(&self) -> Vec<Kind> {
        let mut kinds = vec![Kind::Activation, Kind::Discovery];
        if self.playback.is_some() {
            kinds.push(Kind::Playback);
        }
        if self.enact.is_some() {
            kinds.extend([Kind::Enact, Kind::Integration]);
        }
        kinds.retain(|k| *k != Kind::Discovery || self.enact.is_some());
        kinds
    }

    /// The section a kind draws its task from. Both generated kinds
    /// synthesise their own, so they have none.
    pub fn section_for(&self, kind: Kind) -> Option<&Section> {
        match kind {
            Kind::Playback => self.playback.as_ref(),
            Kind::Enact | Kind::Integration => self.enact.as_ref(),
            Kind::Activation | Kind::Discovery => None,
        }
    }
}

/// Parse one section, enforcing the shape its kind requires.
fn section(fixture: &str, body: &str, name: &str, has_assert: bool) -> Result<Option<Section>> {
    let Some(text) = block(body, name).filter(|t| !t.trim().is_empty()) else {
        return Ok(None);
    };
    let task = field(&text, "task")
        .ok_or_else(|| anyhow!("fixture {fixture} malformed ({name} section needs a task:)"))?;
    let narrative = field(&text, "expect");
    let substr = field(&text, "expect_substr");

    // An enact section proves itself by artefacts (an assert block) or by
    // the reply (a narrative / literal). With neither it asserts nothing.
    if narrative.is_none() && substr.is_none() && !has_assert {
        return Err(anyhow!(
            "fixture {fixture} malformed ({name} section needs {}expect:, or expect_substr:)",
            if name == "enact" {
                "an --- assert --- block, "
            } else {
                ""
            }
        ));
    }
    Ok(Some(Section {
        task,
        narrative,
        substr,
    }))
}

/// True for a `--- <marker> ---` fence line.
fn is_fence(line: &str) -> bool {
    let t = line.trim();
    t.starts_with("--- ") && t.ends_with(" ---")
}

/// The lines before the first fence, carrying `skill:` and `tools:`.
/// Always present, even when empty — a fixture with no header simply
/// fails the `skill:` check with a message naming the fixture.
fn header(body: &str) -> String {
    body.lines()
        .take_while(|l| !is_fence(l))
        .fold(String::new(), |mut acc, l| {
            acc.push_str(l);
            acc.push('\n');
            acc
        })
}

/// The lines of a `--- <marker> ---` block, which runs to the next fence
/// or EOF. `None` when the fixture has no such block.
fn block(body: &str, marker: &str) -> Option<String> {
    let fence = format!("--- {marker} ---");
    let mut out = String::new();
    let mut grabbing = false;
    let mut found = false;
    for line in body.lines() {
        if is_fence(line) {
            grabbing = line.trim() == fence;
            found |= grabbing;
            continue;
        }
        if grabbing {
            out.push_str(line);
            out.push('\n');
        }
    }
    found.then_some(out)
}

/// The first `<field>:` value in a block. One key per line, matching the
/// fixture header's shape.
fn field(text: &str, name: &str) -> Option<String> {
    text.lines()
        .find_map(|l| l.strip_prefix(&format!("{name}:")))
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

// ─────────────────────────────────────────────────────────────────
// Skill sources — where a stage reads the skill under test from.
// ─────────────────────────────────────────────────────────────────

/// Resolve the SKILL.md a stage should point an explicit prompt at.
/// `Check` reads the checkout (no install needed); `Smoke` reads what the
/// agent has deployed. One function so the two can't be conflated —
/// the bash runner read the announce contract from one and built prompts
/// against the other.
pub fn skill_source(
    stage: Stage,
    agent: Agent,
    home: &Path,
    checkout: &Path,
    skill: &str,
) -> Result<std::path::PathBuf> {
    match stage {
        Stage::Check => Ok(crate::shared::checkout_skill(checkout, skill)),
        Stage::Smoke => agent.installed_skill(home, skill).ok_or_else(|| {
            anyhow!(
                "{} has no deployed skills path in the registry",
                agent.name()
            )
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── kinds ────────────────────────────────────────────────────

    #[test]
    fn kind_parse_round_trips_every_name() {
        for kind in Kind::ALL {
            assert_eq!(Kind::parse(kind.name()).unwrap(), *kind);
        }
        assert!(Kind::parse("bogus").is_err());
    }

    /// The five kind names as documented, spelled out — a name-derived
    /// test would round-trip a typo in the token the Justfile passes.
    #[test]
    fn kind_names_are_the_documented_tokens() {
        let names: Vec<&str> = Kind::ALL.iter().map(|k| k.name()).collect();
        assert_eq!(
            names,
            [
                "activation",
                "discovery",
                "playback",
                "enact",
                "integration"
            ]
        );
    }

    /// Stage ownership is total and matches the reach axis: every kind is
    /// owned by exactly one stage, and no explicit kind requires a
    /// deployment. This is the fact the whole two-stage split rests on.
    #[test]
    fn every_kind_maps_to_the_stage_its_reach_implies() {
        for kind in Kind::ALL {
            let want = match kind.reach() {
                Reach::Explicit => Stage::Check,
                Reach::Implicit => Stage::Smoke,
            };
            assert_eq!(kind.stage(), want, "{}", kind.name());
        }
        // Both stages own at least one kind, or a stage would be dead.
        assert!(Kind::ALL.iter().any(|k| k.stage() == Stage::Check));
        assert!(Kind::ALL.iter().any(|k| k.stage() == Stage::Smoke));
    }

    #[test]
    fn pre_install_kinds_are_exactly_the_explicit_three() {
        let check: Vec<&str> = Kind::ALL
            .iter()
            .filter(|k| k.stage() == Stage::Check)
            .map(|k| k.name())
            .collect();
        assert_eq!(check, ["activation", "playback", "enact"]);
    }

    #[test]
    fn post_install_kinds_are_exactly_the_implicit_two() {
        let smoke: Vec<&str> = Kind::ALL
            .iter()
            .filter(|k| k.stage() == Stage::Smoke)
            .map(|k| k.name())
            .collect();
        assert_eq!(smoke, ["discovery", "integration"]);
    }

    // ── fixture parsing ──────────────────────────────────────────

    fn fx(body: &str) -> Result<Fixture> {
        Fixture::parse("t", body)
    }

    #[test]
    fn parses_skill_and_defaults_tools_to_claude() {
        let f = fx("skill: notes\n--- playback ---\ntask: q\nexpect: n\n").unwrap();
        assert_eq!(f.skill, "notes");
        assert_eq!(f.agents, vec![Agent::Claude]);
    }

    #[test]
    fn parses_explicit_tool_list() {
        let f = fx("skill: notes\ntools: claude,kiro,codex\n--- enact ---\ntask: t\nexpect: n\n")
            .unwrap();
        assert_eq!(f.agents, vec![Agent::Claude, Agent::Kiro, Agent::Codex]);
    }

    #[test]
    fn parses_both_sections_with_setup_and_assert() {
        let f = fx("skill: notes\n\
                    --- playback ---\ntask: q\nexpect: narr\n\
                    --- enact ---\ntask: t\nexpect_substr: lit\n\
                    --- setup ---\ngit init -q\n\
                    --- assert ---\ntest -f x\n")
        .unwrap();
        assert_eq!(f.playback.as_ref().unwrap().task, "q");
        assert_eq!(
            f.playback.as_ref().unwrap().narrative.as_deref(),
            Some("narr")
        );
        assert_eq!(f.enact.as_ref().unwrap().substr.as_deref(), Some("lit"));
        assert_eq!(f.setup.as_deref(), Some("git init -q\n"));
        assert_eq!(f.assert.as_deref(), Some("test -f x\n"));
    }

    #[test]
    fn enact_with_assert_block_needs_no_expectation() {
        let f = fx("skill: notes\n--- enact ---\ntask: t\n--- assert ---\ntest -f x\n").unwrap();
        assert!(f.enact.as_ref().unwrap().narrative.is_none());
        assert!(f.assert.is_some());
    }

    // ── malformed shapes ─────────────────────────────────────────

    #[test]
    fn rejects_missing_skill() {
        let e = fx("--- enact ---\ntask: t\nexpect: n\n")
            .unwrap_err()
            .to_string();
        assert!(e.contains("missing skill:"), "{e}");
    }

    #[test]
    fn rejects_no_sections() {
        let e = fx("skill: notes\n").unwrap_err().to_string();
        assert!(e.contains("playback"), "{e}");
    }

    #[test]
    fn rejects_section_without_task() {
        let e = fx("skill: notes\n--- playback ---\nexpect: n\n")
            .unwrap_err()
            .to_string();
        assert!(e.contains("needs a task:"), "{e}");
    }

    #[test]
    fn rejects_playback_without_any_expectation() {
        let e = fx("skill: notes\n--- playback ---\ntask: q\n")
            .unwrap_err()
            .to_string();
        assert!(e.contains("expect:"), "{e}");
    }

    /// An enact section with no assert block and no expectation asserts
    /// nothing at all, which would report a pass for an idle agent.
    #[test]
    fn rejects_enact_with_neither_assert_nor_expectation() {
        let e = fx("skill: notes\n--- enact ---\ntask: t\n")
            .unwrap_err()
            .to_string();
        assert!(e.contains("assert"), "{e}");
    }

    #[test]
    fn rejects_unknown_tool() {
        let e = fx("skill: notes\ntools: claude,bogus\n--- enact ---\ntask: t\nexpect: n\n")
            .unwrap_err()
            .to_string();
        assert!(e.contains("bogus"), "{e}");
    }

    // ── kind derivation ──────────────────────────────────────────

    #[test]
    fn playback_only_fixture_yields_no_enact_or_integration() {
        let f = fx("skill: notes\n--- playback ---\ntask: q\nexpect: n\n").unwrap();
        let kinds = f.kinds();
        assert!(kinds.contains(&Kind::Playback));
        assert!(kinds.contains(&Kind::Activation));
        assert!(!kinds.contains(&Kind::Enact));
        assert!(!kinds.contains(&Kind::Integration));
    }

    /// One enact section drives two kinds — the same task reached
    /// explicitly, then implicitly.
    #[test]
    fn enact_section_yields_both_enact_and_integration() {
        let f = fx("skill: notes\n--- enact ---\ntask: t\nexpect: n\n").unwrap();
        let kinds = f.kinds();
        assert!(kinds.contains(&Kind::Enact));
        assert!(kinds.contains(&Kind::Integration));
        assert_eq!(
            f.section_for(Kind::Enact),
            f.section_for(Kind::Integration),
            "both kinds must draw the same task"
        );
    }

    /// Discovery needs a real task to fire on, and only an enact section
    /// supplies one — a playback question is not something a user phrases
    /// implicitly.
    #[test]
    fn discovery_requires_an_enact_task() {
        let playback_only = fx("skill: notes\n--- playback ---\ntask: q\nexpect: n\n").unwrap();
        assert!(!playback_only.kinds().contains(&Kind::Discovery));

        let with_enact = fx("skill: notes\n--- enact ---\ntask: t\nexpect: n\n").unwrap();
        assert!(with_enact.kinds().contains(&Kind::Discovery));
    }

    #[test]
    fn generated_kinds_have_no_authored_section() {
        let f = fx("skill: notes\n--- enact ---\ntask: t\nexpect: n\n").unwrap();
        assert!(f.section_for(Kind::Activation).is_none());
        assert!(f.section_for(Kind::Discovery).is_none());
    }

    // ── skill sources ────────────────────────────────────────────

    /// The stage decides which side of install a prompt reads from. Both
    /// resolved against the same root, so a resolver ignoring its stage
    /// would collide here.
    #[test]
    fn check_reads_the_checkout_and_smoke_reads_the_deployment() {
        let root = Path::new("/same");
        let checked = skill_source(Stage::Check, Agent::Claude, root, root, "notes").unwrap();
        let smoked = skill_source(Stage::Smoke, Agent::Claude, root, root, "notes").unwrap();
        assert!(
            checked.starts_with("/same/resources/content/skills"),
            "{checked:?}"
        );
        assert!(smoked.starts_with("/same/.claude/skills"), "{smoked:?}");
        assert_ne!(checked, smoked);
    }

    /// Pre-install there is one copy of a skill, so the agent is
    /// irrelevant — which is exactly why the explicit kinds need no
    /// deployment.
    #[test]
    fn check_source_is_the_same_for_every_agent() {
        let root = Path::new("/r");
        let first = skill_source(Stage::Check, Agent::Claude, root, root, "notes").unwrap();
        for agent in Agent::ALL {
            assert_eq!(
                skill_source(Stage::Check, *agent, root, root, "notes").unwrap(),
                first
            );
        }
    }

    /// Post-install each agent reads its own tree, so all three differ.
    #[test]
    fn smoke_source_differs_per_agent() {
        let root = Path::new("/r");
        let mut seen: Vec<std::path::PathBuf> = Vec::new();
        for agent in Agent::ALL {
            let p = skill_source(Stage::Smoke, *agent, root, root, "notes").unwrap();
            assert!(
                !seen.contains(&p),
                "{} duplicates another agent",
                agent.name()
            );
            seen.push(p);
        }
    }
}
