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
use std::path::{Path, PathBuf};

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

// ─────────────────────────────────────────────────────────────────
// Prompt envelopes.
//
// An explicit prompt names the skill and the path it lives at, so the
// test isolates the skill's content from whether it would have been
// found. It deliberately does NOT quote the announce line: it tells the
// agent to follow the skill's own instruction for opening a response, so
// an agent that never read the file cannot emit the marker.
//
// An implicit prompt is the task alone — no skill name, no path, no
// marker — so the skill can only apply if the agent recognised the task
// and loaded it unprompted.
// ─────────────────────────────────────────────────────────────────

/// The self-announce line a skill declares, when it has one.
pub fn marker_for(skill: &str) -> String {
    format!("[{skill}] applies")
}

/// True when `skill` declares a self-announce line in its SKILL.md — i.e.
/// when the marker is a contract we may assert.
///
/// Read from the skill itself rather than a list here, which would drift
/// from it. The payoff is real for skills whose output a user then
/// inspects and trusts; a workflow skill like kdevkit earns it
/// differently — its evidence is the artefacts it leaves — so a skill
/// without a marker skips the two announce-only kinds and is proven to
/// have fired by its enact / integration artefacts instead.
pub fn skill_announces(source: &Path, skill: &str) -> bool {
    std::fs::read_to_string(source)
        .map(|body| body.contains(&marker_for(skill)))
        .unwrap_or(false)
}

/// Whether a kind can assert anything for this skill. An announce-only
/// kind against a marker-less skill has nothing in the reply that a
/// generally-capable model wouldn't also produce, so it is skipped with
/// a reason rather than run as a vacuous pass.
pub fn applicable(kind: Kind, source: &Path, skill: &str) -> Result<(), String> {
    if kind.announce_only() && !skill_announces(source, skill) {
        return Err(format!(
            "{skill} declares no announce contract — artefacts prove it (see enact/integration)"
        ));
    }
    Ok(())
}

/// Build the prompt for one test. `source` is the SKILL.md path an
/// explicit prompt should name; implicit prompts ignore it.
pub fn prompt(kind: Kind, skill: &str, source: &Path, task: &str) -> String {
    match kind.reach() {
        Reach::Explicit => format!(
            "Load the `{skill}` skill from {} and follow its instructions —\n\
             including its instruction for how to open a response that uses it.\n\
             \n\
             Then do this:\n\
             {task}\n",
            source.display()
        ),
        Reach::Implicit => format!("{task}\n"),
    }
}

/// The task the two generated kinds fire on. `activation` asks the skill
/// to describe itself; `discovery` needs a real task, which only an enact
/// section supplies.
pub fn generated_task(kind: Kind, fixture: &Fixture) -> Option<String> {
    match kind {
        Kind::Activation => Some("In one short sentence, say what this skill lets me do.".into()),
        Kind::Discovery => fixture.enact.as_ref().map(|s| s.task.clone()),
        _ => None,
    }
}

// ─────────────────────────────────────────────────────────────────
// Dry-run structural checks.
//
// Run instead of calling an agent under --dry-run. These are the
// assertions that were impossible when every fixture hand-wrote its own
// paths: nothing could check that the codex run actually pointed at
// codex's skills root.
// ─────────────────────────────────────────────────────────────────

/// Why a constructed prompt is structurally wrong. `Ok(())` means the
/// prompt is well-formed for its kind.
pub fn check_prompt(kind: Kind, skill: &str, source: &Path, prompt: &str) -> Result<()> {
    match kind.reach() {
        Reach::Explicit => explicit_carries_path(source, prompt),
        Reach::Implicit => implicit_leaks_nothing(skill, prompt),
    }
}

fn explicit_carries_path(source: &Path, prompt: &str) -> Result<()> {
    let want = source.display().to_string();
    if prompt.contains(&want) {
        return Ok(());
    }
    Err(anyhow!("explicit prompt missing its own skill path {want}"))
}

/// An implicit prompt must not identify the skill it should trigger —
/// otherwise the test measures instruction-following, not discovery.
///
/// The check is "does the skill's name appear at all", not a list of
/// phrasings. A denylist of specific forms looked thorough and missed the
/// obvious: `Run the kdevkit closure phase` named the skill outright and
/// still reported "names no skill".
///
/// Two carve-outs, both about words that are ordinary English as well as
/// skill names. A name that is a common noun ("notes", "browser") may
/// appear in a longer domain phrase the task legitimately needs — an
/// "Obsidian-shaped notes vault" is scene-setting, not a skill
/// reference — so a bare common-noun name is allowed, but the skill's
/// documented *invocation verb* is not: `add note in ./ for:` IS the
/// skill's API, and pasting it in makes the prompt a call rather than a
/// task. `SKILL.md` and the per-agent skills roots are always leaks.
fn implicit_leaks_nothing(skill: &str, prompt: &str) -> Result<()> {
    let lower = prompt.to_lowercase();
    let leak = if prompt.contains(&marker_for(skill)) {
        Some("the announce marker".to_string())
    } else if prompt.contains("SKILL.md") {
        Some("a SKILL.md path".to_string())
    } else if let Some(agent) = Agent::ALL
        .iter()
        .find(|a| prompt.contains(&format!("/.{}/", a.name())))
    {
        Some(format!("a {} skills path", agent.name()))
    } else if skill.contains('-') && lower.contains(skill) {
        // A hyphenated name is never ordinary English, so any mention leaks.
        Some(format!("the skill name '{skill}'"))
    } else if lower.contains(&format!("{skill} skill")) {
        Some(format!("the phrase '{skill} skill'"))
    } else {
        invocations(skill)
            .iter()
            .find(|verb| lower.contains(*verb))
            .map(|verb| format!("the skill's invocation verb '{verb}'"))
    };

    match leak {
        None => Ok(()),
        Some(what) => Err(anyhow!("implicit prompt leaks {what}")),
    }
}

/// The command syntax a skill documents as its entry point. Empty for a
/// skill invoked by intent alone rather than a fixed verb.
fn invocations(skill: &str) -> &'static [&'static str] {
    match skill {
        "notes" => &["add note", "close notes", "merge buffer"],
        _ => &[],
    }
}

// ─────────────────────────────────────────────────────────────────
// Verdicts — reading a judge's answer out of an agent's reply.
//
// Every bug in this section shipped once and was found only by spending
// API credits on a paid sweep. They are unit tests now.
// ─────────────────────────────────────────────────────────────────

/// A judged test's outcome. `Unparseable` is explicit rather than folded
/// into a failure: a verdict that could not be read means the harness is
/// broken, which is a different problem from the skill being wrong, and
/// silently treating it as either one hides it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Verdict {
    Pass(String),
    Fail(String),
    Unparseable,
}

/// Strip terminal control sequences from an agent's reply.
///
/// Hand-rolled deliberately: a parser crate cannot be added to this
/// flake's offline cargo closure. The `sed` expression this replaces used
/// the character class `[a-zA-Z]` as its terminator, which does not match
/// the `?25l` cursor-hide sequence kiro emits — so kiro's verdict became
/// unreadable and every judged kiro test reported no verdict. Matching
/// the grammar (CSI, then parameter and intermediate bytes, then one
/// final byte in 0x40..=0x7E) is what fixes that class of miss rather
/// than the one instance.
pub fn strip_ansi(text: &str) -> String {
    let bytes: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != '\x1b' {
            if bytes[i] != '\r' {
                out.push(bytes[i]);
            }
            i += 1;
            continue;
        }
        // ESC. A CSI sequence is ESC '[' params intermediates final;
        // anything else two-byte we drop as ESC + one byte.
        i += 1;
        if i < bytes.len() && bytes[i] == '[' {
            i += 1;
            while i < bytes.len() && !matches!(bytes[i], '\x40'..='\x7e') {
                i += 1;
            }
            i += 1; // the final byte
        } else {
            i += 1;
        }
    }
    out
}

/// The literal placeholder the judge prompt asks the model to replace.
/// Any line still carrying it is the template echoed back, not an answer.
const VERDICT_PLACEHOLDER: &str = "<one short sentence";

/// Read a verdict out of a judge's raw reply.
///
/// Three guards, each for a bug that shipped:
///
/// - Lines carrying the instruction template's placeholder are dropped.
///   The judge prompt necessarily contains the literal `PASS — <one short
///   sentence …>`, and codex's `exec` echoes the prompt back, so a
///   first-match scan read the template instead of the model's answer and
///   every judged codex test passed unconditionally.
/// - The token must start its line, so a mid-sentence "…would FAIL" in
///   the judge's prose is not mistaken for the verdict.
/// - Escapes and a leading `> ` are stripped first: kiro prefixes its
///   reply with a cursor-hide sequence and a quote marker, which
///   line-anchoring alone then failed to see past.
pub fn read_verdict(raw: &str) -> Verdict {
    strip_ansi(raw)
        .lines()
        .map(|l| l.trim_start().trim_start_matches("> ").trim_start())
        .filter(|l| !l.contains(VERDICT_PLACEHOLDER))
        .find_map(|l| {
            l.strip_prefix("PASS")
                .map(|rest| Verdict::Pass(clean_reason(rest)))
                .or_else(|| {
                    l.strip_prefix("FAIL")
                        .map(|rest| Verdict::Fail(clean_reason(rest)))
                })
        })
        .unwrap_or(Verdict::Unparseable)
}

/// The judge's one-line reason, with the separator it was asked to use.
fn clean_reason(rest: &str) -> String {
    rest.trim_start()
        .trim_start_matches(['—', '-', ':'])
        .trim()
        .to_string()
}

/// The prompt that asks one agent to score another's answer.
pub fn judge_prompt(question: &str, answer: &str, expected: &str) -> String {
    format!(
        "You are evaluating whether an AI agent's answer is consistent with a skill's intended narrative.\n\
         \n\
         Question:\n{question}\n\
         \n\
         Answer:\n{answer}\n\
         \n\
         Expected narrative:\n{expected}\n\
         \n\
         Reply with exactly one line in this format:\n\
         PASS — {VERDICT_PLACEHOLDER} describing what's right>\n\
         or\n\
         FAIL — {VERDICT_PLACEHOLDER} describing what's missing or wrong>\n"
    )
}

/// Which agent grades judged tests, whichever produced the answer.
///
/// One fixed grader, not the answering agent. Self-grading let each agent
/// mark its own homework, and the grader rather than the skill decided the
/// result: codex failed its own notes answer for omitting something the
/// answer stated, and its own writing-style rewrite as "not first-person"
/// when it opened "I want to…". Claude passed both verbatim. A per-agent
/// grader also makes results incomparable across agents, which defeats
/// running tri-tool at all.
///
/// Falls back to the first available agent so a host without the
/// preferred grader still runs.
pub fn judge_agent(preferred: Option<&str>, available: &[Agent]) -> Result<Agent> {
    if let Some(token) = preferred {
        let want = Agent::parse(token)?;
        if available.contains(&want) {
            return Ok(want);
        }
    }
    if preferred.is_none() && available.contains(&Agent::Claude) {
        return Ok(Agent::Claude);
    }
    available
        .first()
        .copied()
        .ok_or_else(|| anyhow!("no coding agent available to judge with"))
}

// ─────────────────────────────────────────────────────────────────
// Invocation — driving a coding agent non-interactively.
//
// The single indirection point. Per-tool CLI flags change across
// versions; when they do, this is the only place to touch.
//
// What an invocation returns is load-bearing: the *reply*, not the
// session transcript. Codex `exec` prints a transcript — the prompt
// echoed back, then tool output, then the reply — so capturing it whole
// silently breaks every reply-level assertion (see Verdict, and the
// marker, which appears in tool output whenever the agent cats a
// SKILL.md). Codex's --output-last-message writes just the final
// message; claude --print and kiro's non-interactive chat already emit
// the reply alone.
// ─────────────────────────────────────────────────────────────────

/// How much filesystem authority an invocation gets. A reply-only test
/// needs none; a behavioral test needs to write inside its scratch dir.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Authority {
    ReadOnly,
    Workdir,
}

/// The argv for driving one agent, plus whether its reply lands on stdout
/// or in a file. Pure — building it takes no process, which is what makes
/// the flag surface testable.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Invocation {
    pub program: String,
    pub args: Vec<String>,
    /// Set when the agent writes its final message to a file rather than
    /// stdout, so the caller reads that instead of the transcript.
    pub reply_file: Option<PathBuf>,
    pub cwd: Option<PathBuf>,
}

/// Build the invocation for an agent. `reply_to` is where an agent that
/// cannot emit a bare reply on stdout should write it.
pub fn invocation(
    agent: Agent,
    prompt: &str,
    authority: Authority,
    workdir: Option<&Path>,
    reply_to: &Path,
) -> Invocation {
    let owned = |s: &str| s.to_string();
    match agent {
        Agent::Claude => Invocation {
            program: owned("claude"),
            args: vec![
                owned("--print"),
                owned("--dangerously-skip-permissions"),
                prompt.to_string(),
            ],
            reply_file: None,
            cwd: workdir.map(Path::to_path_buf),
        },
        Agent::Kiro => Invocation {
            program: owned("kiro-cli"),
            args: vec![
                owned("chat"),
                owned("--no-interactive"),
                // Trust everything only where the test seeded a workdir to
                // act in; a reply-only test gets an empty trust list.
                match authority {
                    Authority::Workdir => owned("--trust-all-tools"),
                    Authority::ReadOnly => owned("--trust-tools="),
                },
                prompt.to_string(),
            ],
            reply_file: None,
            cwd: workdir.map(Path::to_path_buf),
        },
        Agent::Codex => {
            let mut args = vec![owned("exec")];
            if let Some(dir) = workdir {
                args.push(owned("--cd"));
                args.push(dir.display().to_string());
            }
            args.push(owned("--sandbox"));
            args.push(owned(match authority {
                Authority::Workdir => "workspace-write",
                Authority::ReadOnly => "read-only",
            }));
            // A seeded scratch dir may not be a git tree.
            args.push(owned("--skip-git-repo-check"));
            args.push(owned("-o"));
            args.push(reply_to.display().to_string());
            args.push(prompt.to_string());
            Invocation {
                program: owned("codex"),
                args,
                reply_file: Some(reply_to.to_path_buf()),
                cwd: None,
            }
        }
    }
}

/// Whether an agent's CLI is on PATH.
pub fn agent_available(agent: Agent) -> bool {
    let program = match agent {
        Agent::Claude => "claude",
        Agent::Kiro => "kiro-cli",
        Agent::Codex => "codex",
    };
    std::process::Command::new("command")
        .args(["-v", program])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
        || std::env::var_os("PATH").is_some_and(|paths| {
            std::env::split_paths(&paths).any(|dir| dir.join(program).is_file())
        })
}

// ─────────────────────────────────────────────────────────────────
// Assertions — how a reply or a workdir is scored.
// ─────────────────────────────────────────────────────────────────

/// What a test asserts against. A behavioral test inspects the workdir it
/// seeded; a reply test scores the text. Which one applies is a property
/// of the fixture, not of the kind.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Assertion {
    /// Run the fixture's assert shell in the seeded workdir.
    Behavioral { setup: String, assert: String },
    /// Score the reply: a literal substring, a judged narrative, or both.
    Reply {
        substr: Option<String>,
        narrative: Option<String>,
    },
}

/// Decide what a given kind of a given fixture asserts.
///
/// An assert block makes the enact/integration kinds artefact-based; the
/// announce-only kinds always score the reply, since the marker IS the
/// assertion. For a marker-less skill an implicit reply test would prove
/// nothing, so the marker is only added where the skill promises one.
pub fn assertion_for(fixture: &Fixture, kind: Kind, announces: bool) -> Assertion {
    let artefact_kind = matches!(kind, Kind::Enact | Kind::Integration);
    if let (true, Some(assert)) = (artefact_kind, fixture.assert.as_ref()) {
        return Assertion::Behavioral {
            setup: fixture.setup.clone().unwrap_or_default(),
            assert: assert.clone(),
        };
    }
    if kind.announce_only() {
        return Assertion::Reply {
            substr: Some(marker_for(&fixture.skill)),
            narrative: None,
        };
    }
    let section = fixture.section_for(kind);
    let mut substr = section.and_then(|s| s.substr.clone());
    // An implicit reply test needs SOME proof the skill fired, and the
    // marker is the only reply-level evidence available — but only for a
    // skill that promises one.
    if substr.is_none() && kind.reach() == Reach::Implicit && announces {
        substr = Some(marker_for(&fixture.skill));
    }
    Assertion::Reply {
        substr,
        narrative: section.and_then(|s| s.narrative.clone()),
    }
}

/// Score a reply against a literal substring, case-insensitively as the
/// bash `grep -qiF` did.
pub fn reply_contains(reply: &str, want: &str) -> bool {
    reply.to_lowercase().contains(&want.to_lowercase())
}

// ─────────────────────────────────────────────────────────────────
// Running a test — the reply and behavioral paths.
// ─────────────────────────────────────────────────────────────────

/// What one test run produced.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Outcome {
    Pass(String),
    Fail(String),
    /// Not run, with the reason — a marker-less skill's announce kinds,
    /// or an agent absent from PATH when it wasn't required.
    Skip(String),
}

impl Outcome {
    pub fn token(&self) -> &'static str {
        match self {
            Outcome::Pass(_) => "PASS",
            Outcome::Fail(_) => "FAIL",
            Outcome::Skip(_) => "SKIP",
        }
    }

    pub fn detail(&self) -> &str {
        match self {
            Outcome::Pass(d) | Outcome::Fail(d) | Outcome::Skip(d) => d,
        }
    }
}

/// Score a reply against its assertion. Pure — the process work happens
/// in the caller, so the scoring rules are testable without an agent.
///
/// A substring and a narrative both apply when both are given: the
/// literal is the cheap structural check, the judge the semantic one.
pub fn score_reply(
    reply: &str,
    substr: Option<&str>,
    narrative_verdict: Option<&Verdict>,
) -> Outcome {
    if let Some(want) = substr {
        if !reply_contains(reply, want) {
            return Outcome::Fail(format!("response missing {want:?}"));
        }
    }
    match narrative_verdict {
        None => Outcome::Pass(match substr {
            Some(_) => "substr".into(),
            None => "no assertion".into(),
        }),
        Some(Verdict::Pass(why)) => Outcome::Pass(why.clone()),
        Some(Verdict::Fail(why)) => Outcome::Fail(why.clone()),
        Some(Verdict::Unparseable) => Outcome::Fail("judge returned no PASS/FAIL token".into()),
    }
}

/// The name a result line carries. Kept in the shape the bash runner
/// used so a run stays diffable against a prior run.
pub fn test_name(fixture: &str, kind: Kind, agent: Agent) -> String {
    match kind.announce_only() {
        true => format!("skill:{fixture} {} via {}", kind.name(), agent.name()),
        false => format!("{fixture} {} via {}", kind.name(), agent.name()),
    }
}

// ─────────────────────────────────────────────────────────────────
// Leak tripwire.
//
// Behavioral tests seed a scratch workdir, but containment is by
// convention: the agent keeps ambient filesystem authority, so a
// differently-resolved relative path lands in the checkout instead. It
// has happened — notes-git-commit runs `close notes`, which runs
// `git commit`, and twice committed insight files into the repo while
// reporting PASS. Snapshot before, diff after, so a leak is reported by
// the suite rather than found later in git log.
//
// See specs/backlog/test-runner-workdir-containment.md for the real fix;
// this only detects.
// ─────────────────────────────────────────────────────────────────

/// A `git status --porcelain` snapshot of the checkout, or `None` when it
/// isn't a git tree (nothing to compare).
pub fn snapshot_checkout(checkout: &Path) -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["-C"])
        .arg(checkout)
        .args(["status", "--porcelain"])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Paths that appeared in the checkout since the snapshot. Empty means no
/// leak; any entry is a test that wrote outside its scratch dir.
pub fn detect_leak(before: &str, after: &str) -> Vec<String> {
    let prior: Vec<&str> = before.lines().collect();
    after
        .lines()
        .filter(|line| !prior.contains(line))
        .map(str::to_string)
        .collect()
}

// ─────────────────────────────────────────────────────────────────
// Selection — which fixtures, kinds, and agents a run covers.
// ─────────────────────────────────────────────────────────────────

/// What a run was asked to cover. The requested set is also the required
/// set: an agent named here but missing from PATH is a failure, not a
/// skip, because "verify across the agents we asked for" is the point.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Selection {
    pub stage: Stage,
    pub fixture: Option<String>,
    pub kinds: Vec<Kind>,
    pub agents: Vec<Agent>,
}

impl Selection {
    /// Resolve CLI arguments into a selection, defaulting to every kind
    /// this stage owns and every agent.
    ///
    /// A kind belonging to the other stage is an error rather than a
    /// silent no-op: `check --kind discovery` running zero tests and
    /// reporting success would read as "discovery passes".
    pub fn resolve(
        stage: Stage,
        fixture: Option<&str>,
        kinds: Option<&str>,
        agent: Option<Agent>,
    ) -> Result<Selection> {
        let owned: Vec<Kind> = Kind::ALL
            .iter()
            .copied()
            .filter(|k| k.stage() == stage)
            .collect();

        let kinds = match kinds {
            None => owned,
            Some(list) => {
                let asked: Vec<Kind> = list
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(Kind::parse)
                    .collect::<Result<_>>()?;
                if asked.is_empty() {
                    return Err(anyhow!("--kind listed no kinds"));
                }
                if let Some(wrong) = asked.iter().find(|k| k.stage() != stage) {
                    return Err(anyhow!(
                        "kind '{}' belongs to `{}`, not `{stage}` — it is {} reach, so it needs {}",
                        wrong.name(),
                        wrong.stage(),
                        match wrong.reach() {
                            Reach::Implicit => "implicit",
                            Reach::Explicit => "explicit",
                        },
                        match wrong.stage() {
                            Stage::Smoke => "the deployed skills listing to compete in",
                            Stage::Check => "only the checkout",
                        }
                    ));
                }
                asked
            }
        };

        Ok(Selection {
            stage,
            fixture: fixture.map(str::to_string),
            kinds,
            agents: match agent {
                Some(a) => vec![a],
                None => Agent::ALL.to_vec(),
            },
        })
    }

    /// True when this fixture is in scope for the run.
    pub fn covers(&self, fixture: &str) -> bool {
        self.fixture.as_deref().is_none_or(|want| want == fixture)
    }
}

/// The tests a fixture contributes to a run, in a deterministic order so
/// two runs are diffable.
///
/// `claimed` tracks the (skill, kind, agent) triples the generated kinds
/// have already produced. Those two are per-SKILL, not per-fixture — one
/// skill commonly has several fixtures (notes has five), and asking each
/// of them whether the skill announces itself would run the same test
/// five times and report it five times.
pub fn plan_tests(
    fixture: &Fixture,
    selection: &Selection,
    claimed: &mut Vec<(String, Kind, Agent)>,
) -> Vec<(Kind, Agent)> {
    let mut out = Vec::new();
    for kind in fixture.kinds() {
        if !selection.kinds.contains(&kind) {
            continue;
        }
        for agent in &fixture.agents {
            if !selection.agents.contains(agent) {
                continue;
            }
            if kind.announce_only() {
                let claim = (fixture.skill.clone(), kind, *agent);
                if claimed.contains(&claim) {
                    continue;
                }
                claimed.push(claim);
            }
            out.push((kind, *agent));
        }
    }
    out
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

    // ── prompt envelopes ─────────────────────────────────────────

    #[test]
    fn explicit_prompt_names_the_skill_and_its_path() {
        let p = prompt(
            Kind::Enact,
            "notes",
            Path::new("/x/notes/SKILL.md"),
            "do it",
        );
        assert!(p.contains("`notes`"));
        assert!(p.contains("/x/notes/SKILL.md"));
        assert!(p.contains("do it"));
    }

    /// Quoting the marker would let an agent that never opened the file
    /// emit it, so the envelope points at the skill's own instruction
    /// instead of restating it.
    #[test]
    fn explicit_prompt_never_quotes_the_announce_marker() {
        let p = prompt(Kind::Enact, "notes", Path::new("/x/SKILL.md"), "do it");
        assert!(!p.contains(&marker_for("notes")));
    }

    #[test]
    fn implicit_prompt_is_the_task_alone() {
        let p = prompt(
            Kind::Integration,
            "notes",
            Path::new("/x/SKILL.md"),
            "do it",
        );
        assert_eq!(p.trim(), "do it");
    }

    /// Every explicit kind carries a path and every implicit one does
    /// not — the property the leak checks depend on.
    #[test]
    fn reach_decides_whether_a_path_appears() {
        let src = Path::new("/x/notes/SKILL.md");
        for kind in Kind::ALL {
            let p = prompt(*kind, "notes", src, "do it");
            let carries = p.contains("/x/notes/SKILL.md");
            assert_eq!(carries, kind.reach() == Reach::Explicit, "{}", kind.name());
        }
    }

    #[test]
    fn activation_synthesises_a_task_and_discovery_borrows_the_enact_one() {
        let f = fx("skill: notes\n--- enact ---\ntask: real task\nexpect: n\n").unwrap();
        assert!(generated_task(Kind::Activation, &f).is_some());
        assert_eq!(
            generated_task(Kind::Discovery, &f).as_deref(),
            Some("real task")
        );
        assert!(generated_task(Kind::Enact, &f).is_none());
    }

    // ── dry-run checks ───────────────────────────────────────────

    #[test]
    fn explicit_check_requires_that_agents_own_path() {
        let src = Path::new("/home/u/.codex/skills/notes/SKILL.md");
        let good = prompt(Kind::Enact, "notes", src, "t");
        assert!(check_prompt(Kind::Enact, "notes", src, &good).is_ok());

        let wrong = prompt(
            Kind::Enact,
            "notes",
            Path::new("/home/u/.claude/skills/notes/SKILL.md"),
            "t",
        );
        assert!(check_prompt(Kind::Enact, "notes", src, &wrong).is_err());
    }

    #[test]
    fn implicit_check_catches_the_announce_marker() {
        let leak = format!("do it and say {}", marker_for("notes"));
        assert!(check_prompt(Kind::Integration, "notes", Path::new("/x"), &leak).is_err());
    }

    #[test]
    fn implicit_check_catches_a_skill_md_mention() {
        assert!(check_prompt(
            Kind::Integration,
            "notes",
            Path::new("/x"),
            "read the SKILL.md first"
        )
        .is_err());
    }

    #[test]
    fn implicit_check_catches_every_agents_skills_root() {
        for agent in Agent::ALL {
            let leak = format!("look in ~/.{}/skills for it", agent.name());
            assert!(
                check_prompt(Kind::Integration, "notes", Path::new("/x"), &leak).is_err(),
                "{} root not caught",
                agent.name()
            );
        }
    }

    /// A hyphenated name is never ordinary English, so any mention leaks —
    /// including mid-sentence, which a phrase denylist missed.
    #[test]
    fn implicit_check_catches_a_hyphenated_name_anywhere() {
        for text in [
            "use writing-style on this",
            "Run the writing-style formatter",
            "WRITING-STYLE please",
        ] {
            assert!(
                check_prompt(Kind::Integration, "writing-style", Path::new("/x"), text).is_err(),
                "{text:?}"
            );
        }
    }

    /// The carve-out: a common-noun name in a domain phrase is
    /// scene-setting, not a skill reference.
    #[test]
    fn implicit_check_allows_a_common_noun_name_in_a_domain_phrase() {
        assert!(check_prompt(
            Kind::Integration,
            "notes",
            Path::new("/x"),
            "The current directory is an Obsidian-shaped notes vault."
        )
        .is_ok());
    }

    /// …but the phrase "<name> skill" is a reference, not scene-setting.
    #[test]
    fn implicit_check_catches_the_name_skill_phrasing() {
        assert!(check_prompt(
            Kind::Integration,
            "notes",
            Path::new("/x"),
            "use the notes skill"
        )
        .is_err());
    }

    /// Pasting a skill's own command syntax is the same leak as naming
    /// it: the prompt becomes a call rather than a task.
    #[test]
    fn implicit_check_catches_a_documented_invocation_verb() {
        for text in ["add note in ./ for: x", "close notes in ./", "merge buffer"] {
            assert!(
                check_prompt(Kind::Integration, "notes", Path::new("/x"), text).is_err(),
                "{text:?}"
            );
        }
    }

    #[test]
    fn implicit_check_passes_a_clean_task() {
        assert!(check_prompt(
            Kind::Integration,
            "kdevkit",
            Path::new("/x"),
            "Plan a feature for adding a login page."
        )
        .is_ok());
    }

    // ── announce contract ────────────────────────────────────────

    /// Read from the skill file, not a list here — a list would drift
    /// from the content it describes.
    #[test]
    fn skill_announces_reads_the_marker_from_the_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let with = dir.path().join("with.md");
        let without = dir.path().join("without.md");
        std::fs::write(&with, "body\nYou begin with [notes] applies\n").unwrap();
        std::fs::write(&without, "body with no marker\n").unwrap();
        assert!(skill_announces(&with, "notes"));
        assert!(!skill_announces(&without, "notes"));
        // A marker for a different skill is not this skill's contract.
        assert!(!skill_announces(&with, "kdevkit"));
    }

    #[test]
    fn missing_skill_file_declares_no_contract() {
        assert!(!skill_announces(
            Path::new("/nonexistent/SKILL.md"),
            "notes"
        ));
    }

    /// The two announce-only kinds are skipped for a marker-less skill:
    /// nothing in the reply would distinguish "the skill fired" from "the
    /// model is capable". The other three still run.
    #[test]
    fn announce_only_kinds_skip_a_marker_less_skill() {
        let dir = tempfile::TempDir::new().unwrap();
        let src = dir.path().join("s.md");
        std::fs::write(&src, "a workflow skill, no marker\n").unwrap();

        for kind in Kind::ALL {
            let verdict = applicable(*kind, &src, "kdevkit");
            assert_eq!(
                verdict.is_err(),
                kind.announce_only(),
                "{} applicability wrong",
                kind.name()
            );
        }
        assert!(applicable(Kind::Activation, &src, "kdevkit")
            .unwrap_err()
            .contains("artefacts prove it"));
    }

    #[test]
    fn every_kind_applies_to_a_skill_that_announces() {
        let dir = tempfile::TempDir::new().unwrap();
        let src = dir.path().join("s.md");
        std::fs::write(&src, "opens with [notes] applies\n").unwrap();
        for kind in Kind::ALL {
            assert!(applicable(*kind, &src, "notes").is_ok(), "{}", kind.name());
        }
    }

    // ── verdicts · the four bugs that shipped ────────────────────

    /// BUG 1. codex `exec` echoes the prompt back, so the judge's own
    /// instruction template appeared in captured output. A first-match
    /// scan read the template's `PASS — <one short sentence…>` as the
    /// answer, and every judged test on codex passed unconditionally.
    #[test]
    fn template_echoed_in_a_transcript_is_not_a_verdict() {
        let transcript = "\
You are evaluating whether an AI agent's answer is consistent with a skill's intended narrative.
Reply with exactly one line in this format:
PASS — <one short sentence describing what's right>
or
FAIL — <one short sentence describing what's missing or wrong>
[tool] read SKILL.md
FAIL — the answer never mentions the vault selector
";
        assert_eq!(
            read_verdict(transcript),
            Verdict::Fail("the answer never mentions the vault selector".into()),
            "the echoed template must not be read as the verdict"
        );
    }

    /// A transcript carrying ONLY the template has no answer in it, so it
    /// is unparseable — not a pass.
    #[test]
    fn a_transcript_with_only_the_template_is_unparseable() {
        let only_template = judge_prompt("q", "a", "n");
        assert_eq!(read_verdict(&only_template), Verdict::Unparseable);
    }

    /// BUG 2. kiro prefixes its reply with a cursor-hide escape and `> `.
    /// The `sed` class `[a-zA-Z]` does not match `?25l`'s final byte, so
    /// the escape survived, the line no longer started with the token,
    /// and every judged kiro test reported no verdict.
    #[test]
    fn kiro_cursor_hide_and_quote_prefix_still_yields_a_verdict() {
        let reply =
            "\x1b[?25l> PASS — the reply states the rule correctly\n\x1b[?25h▸ Credits: 0.01";
        assert_eq!(
            read_verdict(reply),
            Verdict::Pass("the reply states the rule correctly".into())
        );
    }

    /// BUG 3. Anchoring on the token without line-anchoring let the
    /// judge's own prose decide the result.
    ///
    /// The decoy carries the SAME token as the real verdict and the
    /// opposite outcome, so an unanchored scan reads it and inverts the
    /// result. A decoy with the other token would never be consulted —
    /// the first `PASS` lookup would already have matched the real line.
    #[test]
    fn a_token_mid_sentence_is_not_the_verdict() {
        let reply = "\
This would PASS a weaker rubric, but it misses the guardrail.
FAIL — omits the guardrail entirely";
        assert_eq!(
            read_verdict(reply),
            Verdict::Fail("omits the guardrail entirely".into()),
            "an unanchored scan reads the prose and inverts the verdict"
        );
    }

    /// BUG 4. No token at all must be explicit, not silently either
    /// outcome — it means the harness could not read the judge.
    #[test]
    fn no_token_is_unparseable_not_a_pass() {
        for reply in [
            "I think the answer is broadly consistent.",
            "",
            "passed, probably",
        ] {
            assert_eq!(read_verdict(reply), Verdict::Unparseable, "{reply:?}");
        }
    }

    /// The full matrix, so a regression in one transport cannot hide
    /// behind another passing.
    #[test]
    fn every_agents_reply_shape_parses() {
        let cases: &[(&str, &str, Verdict)] = &[
            (
                "claude",
                "PASS — states the rule",
                Verdict::Pass("states the rule".into()),
            ),
            (
                "kiro",
                "\x1b[?25l> FAIL — omits the guardrail\n▸ Credits: 0.02 • Time: 3s",
                Verdict::Fail("omits the guardrail".into()),
            ),
            (
                "codex",
                "[2026-08-03] tokens used: 812\nPASS — does the thing",
                Verdict::Pass("does the thing".into()),
            ),
        ];
        for (agent, raw, want) in cases {
            assert_eq!(&read_verdict(raw), want, "{agent} reply shape");
        }
    }

    #[test]
    fn verdict_reason_survives_either_separator() {
        assert_eq!(read_verdict("PASS — why"), Verdict::Pass("why".into()));
        assert_eq!(read_verdict("PASS - why"), Verdict::Pass("why".into()));
        assert_eq!(read_verdict("PASS: why"), Verdict::Pass("why".into()));
    }

    // ── escape stripping ─────────────────────────────────────────

    /// The specific sequence that broke kiro, plus the classes around it.
    /// A terminator class of `[a-zA-Z]` misses `?25l` — the grammar's
    /// final byte is anything in 0x40..=0x7E.
    #[test]
    fn strip_ansi_handles_every_sequence_shape() {
        let cases: &[(&str, &str)] = &[
            ("\x1b[?25lhidden", "hidden"), // cursor hide — the sequence that broke kiro
            ("\x1b[?25hshown", "shown"),   // cursor show
            ("\x1b[31mred\x1b[0m", "red"), // colour
            ("\x1b[1;32mbold green\x1b[0m", "bold green"),
            ("\x1b[2Jcleared", "cleared"),
            ("plain", "plain"),
            ("carriage\r\nreturn", "carriage\nreturn"),
        ];
        for (raw, want) in cases {
            assert_eq!(&strip_ansi(raw), want, "{raw:?}");
        }
    }

    /// A final byte outside `[a-zA-Z]` is what the old `sed` class missed.
    /// `?25l` ends in `l`, which IS alphabetic, so it alone cannot tell
    /// the two implementations apart — these can. The grammar's final byte
    /// is anything in 0x40..=0x7E, which includes `@`, `~`, `\`, and `^`.
    #[test]
    fn strip_ansi_handles_non_alphabetic_final_bytes() {
        let cases: &[(&str, &str)] = &[
            ("\x1b[1@inserted", "inserted"),
            ("\x1b[3~deleted", "deleted"),
            ("\x1b[0^private", "private"),
            ("\x1b[2\\ending", "ending"),
        ];
        for (raw, want) in cases {
            assert_eq!(
                &strip_ansi(raw),
                want,
                "{raw:?} — an [a-zA-Z] terminator class misses this"
            );
        }
    }

    /// The same miss reaching `read_verdict`, which is how it actually
    /// broke: the escape survived, so the line no longer began with the
    /// token and the verdict read as unparseable.
    #[test]
    fn a_non_alphabetic_escape_before_the_token_still_yields_a_verdict() {
        let reply = "\x1b[3~PASS — the reply states the rule";
        assert_eq!(
            read_verdict(reply),
            Verdict::Pass("the reply states the rule".into())
        );
    }

    #[test]
    fn strip_ansi_leaves_ordinary_brackets_alone() {
        assert_eq!(strip_ansi("[notes] applies"), "[notes] applies");
    }

    // ── judge selection ──────────────────────────────────────────

    /// Claude by default, so results stay comparable across agents.
    #[test]
    fn judge_defaults_to_claude_when_available() {
        assert_eq!(
            judge_agent(None, &[Agent::Claude, Agent::Codex]).unwrap(),
            Agent::Claude
        );
    }

    /// Falls back rather than refusing, so a host without claude runs.
    #[test]
    fn judge_falls_back_to_the_first_available() {
        assert_eq!(
            judge_agent(None, &[Agent::Codex, Agent::Kiro]).unwrap(),
            Agent::Codex
        );
        assert_eq!(
            judge_agent(Some("claude"), &[Agent::Kiro]).unwrap(),
            Agent::Kiro
        );
    }

    #[test]
    fn judge_honours_an_explicit_override() {
        assert_eq!(
            judge_agent(Some("codex"), &[Agent::Claude, Agent::Codex]).unwrap(),
            Agent::Codex
        );
        assert!(judge_agent(Some("bogus"), &[Agent::Claude]).is_err());
    }

    #[test]
    fn judge_with_no_agents_is_an_error() {
        assert!(judge_agent(None, &[]).is_err());
    }

    /// The prompt must carry the placeholder verbatim — `read_verdict`
    /// filters on it, so the two have to agree or the filter silently
    /// stops working.
    #[test]
    fn judge_prompt_carries_the_placeholder_the_filter_looks_for() {
        let p = judge_prompt("q", "a", "n");
        assert!(p.contains(VERDICT_PLACEHOLDER));
        assert!(p.contains("q") && p.contains("a") && p.contains("n"));
    }

    // ── invocation ───────────────────────────────────────────────

    /// Read-only means read-only for EVERY agent. The bash runner gave
    /// codex `--sandbox read-only` and kiro an empty trust list but ran
    /// claude with --dangerously-skip-permissions, so a fixture meant to
    /// be read-only could still let claude edit the installed, symlinked
    /// SKILL.md. Pinning the asymmetry here rather than leaving it to a
    /// reading of three call sites.
    ///
    /// This test documents CURRENT behavior including that gap, so the
    /// fix (specs/backlog/test-runner-sandbox-asymmetry.md) has something
    /// to flip deliberately rather than drifting into place unnoticed.
    #[test]
    fn read_only_authority_is_asymmetric_across_agents_today() {
        let reply = Path::new("/tmp/r");
        let flags = |a: Agent| {
            invocation(a, "p", Authority::ReadOnly, None, reply)
                .args
                .join(" ")
        };
        assert!(flags(Agent::Codex).contains("--sandbox read-only"));
        assert!(flags(Agent::Kiro).contains("--trust-tools="));
        // The outlier — see the backlog item.
        assert!(flags(Agent::Claude).contains("--dangerously-skip-permissions"));
    }

    #[test]
    fn workdir_authority_widens_only_where_the_agent_needs_it() {
        let reply = Path::new("/tmp/r");
        let dir = Path::new("/tmp/work");
        let codex = invocation(Agent::Codex, "p", Authority::Workdir, Some(dir), reply);
        assert!(codex.args.join(" ").contains("--sandbox workspace-write"));
        assert!(codex.args.join(" ").contains("--cd /tmp/work"));

        let kiro = invocation(Agent::Kiro, "p", Authority::Workdir, Some(dir), reply);
        assert!(kiro.args.contains(&"--trust-all-tools".to_string()));
        assert_eq!(kiro.cwd.as_deref(), Some(dir));
    }

    /// Codex writes its final message to a file; the transcript on stdout
    /// must not be mistaken for the reply. The other two emit the reply
    /// alone, so they have no reply file.
    #[test]
    fn only_codex_redirects_its_reply_to_a_file() {
        let reply = Path::new("/tmp/last");
        for agent in Agent::ALL {
            let inv = invocation(*agent, "p", Authority::ReadOnly, None, reply);
            assert_eq!(
                inv.reply_file.is_some(),
                *agent == Agent::Codex,
                "{}",
                agent.name()
            );
        }
        let codex = invocation(Agent::Codex, "p", Authority::ReadOnly, None, reply);
        assert!(codex.args.join(" ").contains("-o /tmp/last"));
    }

    #[test]
    fn every_invocation_carries_the_prompt_and_the_right_program() {
        let reply = Path::new("/tmp/r");
        for (agent, program) in [
            (Agent::Claude, "claude"),
            (Agent::Kiro, "kiro-cli"),
            (Agent::Codex, "codex"),
        ] {
            let inv = invocation(agent, "the-prompt", Authority::ReadOnly, None, reply);
            assert_eq!(inv.program, program);
            assert!(
                inv.args.iter().any(|a| a == "the-prompt"),
                "{} drops the prompt",
                agent.name()
            );
        }
    }

    /// A seeded scratch dir may not be a git tree, so codex must skip the
    /// check or every behavioral test fails for the wrong reason.
    #[test]
    fn codex_skips_the_git_repo_check() {
        let inv = invocation(
            Agent::Codex,
            "p",
            Authority::Workdir,
            Some(Path::new("/tmp/w")),
            Path::new("/tmp/r"),
        );
        assert!(inv.args.contains(&"--skip-git-repo-check".to_string()));
    }

    // ── assertion selection ──────────────────────────────────────

    #[test]
    fn an_assert_block_makes_enact_behavioral() {
        let f = fx("skill: notes\n--- enact ---\ntask: t\n--- setup ---\ngit init\n--- assert ---\ntest -f x\n")
            .unwrap();
        match assertion_for(&f, Kind::Enact, true) {
            Assertion::Behavioral { setup, assert } => {
                assert!(setup.contains("git init"));
                assert!(assert.contains("test -f x"));
            }
            other => panic!("expected behavioral, got {other:?}"),
        }
    }

    /// Same fixture, same assert block: integration is behavioral too —
    /// it is the implicit half of the same proof.
    #[test]
    fn integration_shares_the_behavioral_assertion() {
        let f = fx("skill: notes\n--- enact ---\ntask: t\n--- assert ---\ntest -f x\n").unwrap();
        assert_eq!(
            assertion_for(&f, Kind::Enact, true),
            assertion_for(&f, Kind::Integration, true)
        );
    }

    #[test]
    fn without_an_assert_block_enact_scores_the_reply() {
        let f = fx("skill: notes\n--- enact ---\ntask: t\nexpect: narrative\n").unwrap();
        match assertion_for(&f, Kind::Enact, true) {
            Assertion::Reply { narrative, .. } => {
                assert_eq!(narrative.as_deref(), Some("narrative"))
            }
            other => panic!("expected reply, got {other:?}"),
        }
    }

    /// The announce-only kinds assert the marker and nothing else — it IS
    /// the assertion, so a narrative would confuse what failed.
    #[test]
    fn announce_only_kinds_assert_the_marker() {
        let f = fx("skill: notes\n--- enact ---\ntask: t\nexpect: n\n").unwrap();
        for kind in [Kind::Activation, Kind::Discovery] {
            match assertion_for(&f, kind, true) {
                Assertion::Reply { substr, narrative } => {
                    assert_eq!(substr.as_deref(), Some("[notes] applies"));
                    assert!(narrative.is_none(), "{}", kind.name());
                }
                other => panic!("expected reply, got {other:?}"),
            }
        }
    }

    /// An implicit reply test needs the marker as its "did it fire" half —
    /// but only where the skill promises one. For a marker-less skill,
    /// asserting it would fail a correct answer.
    #[test]
    fn implicit_reply_test_adds_the_marker_only_when_promised() {
        let f = fx("skill: kdevkit\n--- enact ---\ntask: t\nexpect: n\n").unwrap();
        let with = assertion_for(&f, Kind::Integration, true);
        let without = assertion_for(&f, Kind::Integration, false);
        match (with, without) {
            (Assertion::Reply { substr: a, .. }, Assertion::Reply { substr: b, .. }) => {
                assert_eq!(a.as_deref(), Some("[kdevkit] applies"));
                assert!(b.is_none(), "a marker-less skill must not be held to one");
            }
            other => panic!("{other:?}"),
        }
    }

    /// An explicit reply test never borrows the marker: its proof is the
    /// narrative, and the prompt already named the path.
    #[test]
    fn explicit_reply_test_does_not_borrow_the_marker() {
        let f = fx("skill: notes\n--- playback ---\ntask: q\nexpect: n\n").unwrap();
        match assertion_for(&f, Kind::Playback, true) {
            Assertion::Reply { substr, .. } => assert!(substr.is_none()),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn reply_matching_is_case_insensitive() {
        assert!(reply_contains(
            "The [Notes] Applies here",
            "[notes] applies"
        ));
        assert!(!reply_contains("nothing relevant", "[notes] applies"));
    }

    // ── scoring ──────────────────────────────────────────────────

    #[test]
    fn a_missing_substring_fails_before_the_judge_is_consulted() {
        let out = score_reply("nothing here", Some("[notes] applies"), None);
        assert!(matches!(out, Outcome::Fail(_)));
        assert!(out.detail().contains("[notes] applies"));
    }

    #[test]
    fn a_judge_verdict_decides_when_the_substring_passes() {
        let reply = "[notes] applies\nthe note is filed";
        assert!(matches!(
            score_reply(
                reply,
                Some("[notes] applies"),
                Some(&Verdict::Pass("good".into()))
            ),
            Outcome::Pass(_)
        ));
        assert!(matches!(
            score_reply(
                reply,
                Some("[notes] applies"),
                Some(&Verdict::Fail("bad".into()))
            ),
            Outcome::Fail(_)
        ));
    }

    /// An unreadable verdict must fail, not pass. Treating it as a pass is
    /// how every judged codex test once reported green.
    #[test]
    fn an_unparseable_verdict_fails() {
        let out = score_reply("anything", None, Some(&Verdict::Unparseable));
        assert!(matches!(out, Outcome::Fail(_)));
        assert!(out.detail().contains("no PASS/FAIL token"));
    }

    /// A test with no assertion at all would pass for an idle agent. The
    /// fixture parser rejects that shape, so this only documents that
    /// scoring itself never invents a pass reason.
    #[test]
    fn no_assertion_reports_that_it_asserted_nothing() {
        assert_eq!(
            score_reply("anything", None, None),
            Outcome::Pass("no assertion".into())
        );
    }

    // ── test naming ──────────────────────────────────────────────

    /// Names keep the bash shapes so a run stays diffable against a prior
    /// run: generated kinds are per-skill, authored kinds per-fixture.
    #[test]
    fn generated_kinds_are_named_per_skill() {
        assert_eq!(
            test_name("notes", Kind::Activation, Agent::Claude),
            "skill:notes activation via claude"
        );
        assert_eq!(
            test_name("notes-git-commit", Kind::Enact, Agent::Codex),
            "notes-git-commit enact via codex"
        );
    }

    // ── leak tripwire ────────────────────────────────────────────

    #[test]
    fn an_unchanged_checkout_reports_no_leak() {
        let before = " M resources/content/skills/notes/SKILL.md\n";
        assert!(detect_leak(before, before).is_empty());
    }

    /// The real incident: a behavioral test wrote insight files into the
    /// checkout while its assert ran in the scratch dir, so the suite
    /// reported PASS and the damage was found later in git log.
    #[test]
    fn a_new_path_in_the_checkout_is_a_leak() {
        let before = "";
        let after = "?? insights/wadler-builds-an-immutable-document-tree.md\n";
        let leaked = detect_leak(before, after);
        assert_eq!(leaked.len(), 1);
        assert!(leaked[0].contains("insights/"));
    }

    #[test]
    fn a_leak_is_detected_alongside_pre_existing_changes() {
        let before = " M Cargo.lock\n";
        let after = " M Cargo.lock\n?? stray.md\n";
        assert_eq!(detect_leak(before, after), vec!["?? stray.md".to_string()]);
    }

    // ── selection ────────────────────────────────────────────────

    #[test]
    fn check_defaults_to_the_three_pre_install_kinds() {
        let s = Selection::resolve(Stage::Check, None, None, None).unwrap();
        let names: Vec<&str> = s.kinds.iter().map(|k| k.name()).collect();
        assert_eq!(names, ["activation", "playback", "enact"]);
        assert_eq!(s.agents, Agent::ALL.to_vec());
    }

    #[test]
    fn smoke_defaults_to_the_two_post_install_kinds() {
        let s = Selection::resolve(Stage::Smoke, None, None, None).unwrap();
        let names: Vec<&str> = s.kinds.iter().map(|k| k.name()).collect();
        assert_eq!(names, ["discovery", "integration"]);
    }

    /// A kind belonging to the other stage is an error naming why. Running
    /// zero tests and reporting success would read as "discovery passes".
    #[test]
    fn asking_check_for_a_smoke_kind_is_an_error_naming_why() {
        let e = Selection::resolve(Stage::Check, None, Some("discovery"), None)
            .unwrap_err()
            .to_string();
        assert!(e.contains("discovery"), "{e}");
        assert!(e.contains("smoke"), "{e}");
        assert!(e.contains("deployed"), "{e}");
    }

    #[test]
    fn asking_smoke_for_a_check_kind_is_an_error_naming_why() {
        let e = Selection::resolve(Stage::Smoke, None, Some("playback"), None)
            .unwrap_err()
            .to_string();
        assert!(e.contains("playback") && e.contains("check"), "{e}");
    }

    #[test]
    fn an_unknown_kind_or_empty_list_is_rejected() {
        assert!(Selection::resolve(Stage::Check, None, Some("bogus"), None).is_err());
        assert!(Selection::resolve(Stage::Check, None, Some(""), None).is_err());
    }

    #[test]
    fn a_kind_this_stage_owns_is_accepted() {
        let s = Selection::resolve(Stage::Check, None, Some("playback,enact"), None).unwrap();
        assert_eq!(s.kinds, vec![Kind::Playback, Kind::Enact]);
    }

    #[test]
    fn an_agent_selector_scopes_the_run() {
        let s = Selection::resolve(Stage::Check, None, None, Some(Agent::Codex)).unwrap();
        assert_eq!(s.agents, vec![Agent::Codex]);
    }

    #[test]
    fn a_fixture_selector_scopes_to_one_fixture() {
        let s = Selection::resolve(Stage::Check, Some("notes"), None, None).unwrap();
        assert!(s.covers("notes"));
        assert!(!s.covers("writing-style"));
        let all = Selection::resolve(Stage::Check, None, None, None).unwrap();
        assert!(all.covers("anything"));
    }

    // ── planning ─────────────────────────────────────────────────

    /// A fixture contributes only the intersection of its own kinds and
    /// agents with what the run asked for.
    #[test]
    fn planning_intersects_fixture_and_selection() {
        let f =
            fx("skill: notes\ntools: claude,kiro\n--- enact ---\ntask: t\nexpect: n\n").unwrap();
        let s = Selection::resolve(Stage::Check, None, None, Some(Agent::Claude)).unwrap();
        let planned = plan_tests(&f, &s, &mut Vec::new());
        assert!(planned.iter().all(|(_, a)| *a == Agent::Claude));
        // Enact and activation are check-stage; integration/discovery are not.
        assert!(planned.iter().any(|(k, _)| *k == Kind::Enact));
        assert!(planned.iter().all(|(k, _)| k.stage() == Stage::Check));
    }

    /// An agent the fixture never declared is not run for it, even when
    /// the selection asks for every agent.
    #[test]
    fn planning_never_runs_an_agent_a_fixture_excludes() {
        let f = fx("skill: notes\ntools: claude\n--- enact ---\ntask: t\nexpect: n\n").unwrap();
        let s = Selection::resolve(Stage::Check, None, None, None).unwrap();
        assert!(plan_tests(&f, &s, &mut Vec::new())
            .iter()
            .all(|(_, a)| *a == Agent::Claude));
    }

    #[test]
    fn planning_is_deterministic() {
        let f = fx("skill: notes\ntools: claude,kiro,codex\n--- playback ---\ntask: q\nexpect: n\n--- enact ---\ntask: t\nexpect: n\n").unwrap();
        let s = Selection::resolve(Stage::Check, None, None, None).unwrap();
        assert_eq!(
            plan_tests(&f, &s, &mut Vec::new()),
            plan_tests(&f, &s, &mut Vec::new())
        );
    }

    /// The generated kinds are per-SKILL, not per-fixture. `notes` has
    /// five fixtures; without a claim list, activation would run and be
    /// reported five times for one skill.
    #[test]
    fn generated_kinds_run_once_per_skill_across_fixtures() {
        let a = fx("skill: notes\n--- enact ---\ntask: t\nexpect: n\n").unwrap();
        let b = fx("skill: notes\n--- enact ---\ntask: other\nexpect: n\n").unwrap();
        let s = Selection::resolve(Stage::Check, None, None, Some(Agent::Claude)).unwrap();
        let mut claimed = Vec::new();

        let first = plan_tests(&a, &s, &mut claimed);
        let second = plan_tests(&b, &s, &mut claimed);

        assert!(first.iter().any(|(k, _)| *k == Kind::Activation));
        assert!(
            !second.iter().any(|(k, _)| *k == Kind::Activation),
            "activation ran twice for one skill"
        );
        // The authored kinds still run for both fixtures.
        assert!(second.iter().any(|(k, _)| *k == Kind::Enact));
    }

    /// Two different skills each get their own generated kinds.
    #[test]
    fn generated_kinds_are_not_deduped_across_different_skills() {
        let a = fx("skill: notes\n--- enact ---\ntask: t\nexpect: n\n").unwrap();
        let b = fx("skill: browser\n--- enact ---\ntask: t\nexpect: n\n").unwrap();
        let s = Selection::resolve(Stage::Check, None, None, Some(Agent::Claude)).unwrap();
        let mut claimed = Vec::new();
        plan_tests(&a, &s, &mut claimed);
        let second = plan_tests(&b, &s, &mut claimed);
        assert!(second.iter().any(|(k, _)| *k == Kind::Activation));
    }

    /// Dedup is per agent too: the same skill's activation still runs
    /// once for each agent asked for.
    #[test]
    fn generated_kinds_dedupe_per_agent_not_globally() {
        let f =
            fx("skill: notes\ntools: claude,kiro\n--- enact ---\ntask: t\nexpect: n\n").unwrap();
        let s = Selection::resolve(Stage::Check, None, None, None).unwrap();
        let planned = plan_tests(&f, &s, &mut Vec::new());
        let agents: Vec<Agent> = planned
            .iter()
            .filter(|(k, _)| *k == Kind::Activation)
            .map(|(_, a)| *a)
            .collect();
        assert_eq!(agents, vec![Agent::Claude, Agent::Kiro]);
    }
}
