//! The pipeline, one section per stage. Each stage consumes what the
//! previous one produced:
//!
//!   1 content — resources/content/ → a valid skill
//!   2 check   — that skill, verified in isolation from the CHECKOUT
//!   3 install — a valid skill → $HOME symlinks
//!   4 smoke   — the DEPLOYED skill, verified against everything installed
//!
//! Stages 2 and 4 are the same mechanism (see `harness`) pointed at the
//! two sides of install. Reads `shared` for the registry, agents, and
//! roots; nothing here reaches back into the CLI.

use crate::deploy::Deploy;
use crate::harness::{
    agent_available, check_prompt, control_plan, detect_leak, dump_path, invocation, judge_agent,
    judge_prompt, plan_one, plan_tests, read_verdict, score_reply, snapshot_checkout, test_name,
    Assertion, Authority, Fixture, Kind as TestKind, Outcome, Plan, Reach, Selection, Skipped,
    Stage, Stress, Verdict, KIRO_READONLY_AGENT, KIRO_READONLY_NAME,
};
use crate::shared::{checkout_skill, usage, Agent};
use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

// ─────────────────────────────────────────────────────────────────
// Stage 1 · content — resources/content/ → a valid skill.
//
// Only skills are deployed. Each resources/content/skills/<name>/
// SKILL.md needs YAML frontmatter with a non-empty name + description.
// ─────────────────────────────────────────────────────────────────

/// Validate `resources/content/`, returning the count of validated
/// files or the joined error list. Caller decides whether to print or
/// abort.
pub fn check_content(content_dir: &Path) -> Result<usize, Vec<String>> {
    // Walk every skill, partition into (oks, errs), and report the Ok
    // count or the FULL error list — matching on the partitioned tuple
    // so there's no intermediate binding but all errors are still kept.
    match fs::read_dir(content_dir.join("skills"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("SKILL.md"))
        .filter(|p| p.exists())
        .map(|p| check_one_skill(&p))
        .partition::<Vec<_>, _>(Result::is_ok)
    {
        (oks, errs) if errs.is_empty() => Ok(oks.len()),
        (_, errs) => Err(errs.into_iter().map(Result::unwrap_err).collect()),
    }
}

/// Validate one SKILL.md: readable, `---`-fenced YAML frontmatter, and
/// `name` + `description` both non-empty. Errors name the file, since a
/// caller reports them as a list across the whole tree.
///
/// Defers to gray_matter for the envelope and serde for the schema — the
/// in-function struct IS the schema.
fn check_one_skill(path: &Path) -> Result<(), String> {
    #[derive(serde::Deserialize)]
    struct Frontmatter {
        name: String,
        description: String,
    }

    let at = |e: String| format!("{}: {e}", path.display());
    let body = fs::read_to_string(path).map_err(|e| at(format!("cannot read: {e}")))?;
    let fm = gray_matter::Matter::<gray_matter::engine::YAML>::new()
        .parse::<Frontmatter>(&body)
        .map_err(|e| at(e.to_string()))?
        .data
        .ok_or_else(|| at("missing or unterminated YAML frontmatter".into()))?;
    (!fm.name.trim().is_empty() && !fm.description.trim().is_empty())
        .then_some(())
        .ok_or_else(|| at("name and description must be non-empty".into()))
}

// ─────────────────────────────────────────────────────────────────
// Stage 3 · install — a valid skill → wherever the agents read it.
//
// These three verbs say WHAT should happen and let `deploy` decide HOW.
// They contain no knowledge of $HOME layout, symlinks, or any agent's
// directory conventions; swapping in an agent's own install command
// replaces the `Deploy` impl and leaves this section untouched.
// ─────────────────────────────────────────────────────────────────

/// Validate content, failing with every error found.
pub fn cmd_validate(content_dir: &Path) -> Result<u8> {
    let count = check_content(content_dir)
        .map_err(|errs| anyhow!("Content validation failed:\n{}", errs.join("\n")))?;
    eprintln!("validated {count} content file(s)");
    Ok(0)
}

/// Validate the content about to be deployed, then deploy it.
pub fn cmd_install(
    target: &impl Deploy,
    content_dir: &Path,
    dry_run: bool,
    force: bool,
    agent: Option<Agent>,
) -> Result<u8> {
    if !content_dir.join("skills").is_dir() {
        return Err(anyhow!(
            "no skills at {} — build the profile first (just install)",
            content_dir.display()
        ));
    }
    cmd_validate(content_dir)?;
    outcome(target.install(agent, dry_run, force)?, dry_run, false)
}

pub fn cmd_uninstall(
    target: &impl Deploy,
    dry_run: bool,
    force: bool,
    agent: Option<Agent>,
) -> Result<u8> {
    outcome(target.uninstall(agent, dry_run, force)?, dry_run, true)
}

pub fn cmd_status(target: &impl Deploy, agent: Option<Agent>) -> Result<u8> {
    for report in target.status(agent)? {
        println!("{:<28} {}", report.label, report.state.describe());
    }
    Ok(0)
}

/// Print what happened at each location, and exit non-zero when any was
/// left alone because acting would have destroyed something.
fn outcome(reports: Vec<crate::deploy::Report>, dry_run: bool, removing: bool) -> Result<u8> {
    let mut skipped = 0usize;
    for report in &reports {
        let (line, skip) = outcome_line(report, dry_run, removing);
        skipped += usize::from(skip);
        println!("{line}");
    }
    Ok(if skipped > 0 { 1 } else { 0 })
}

/// One location's line, and whether it was left alone in a way that fails
/// the run.
fn outcome_line(report: &crate::deploy::Report, dry_run: bool, removing: bool) -> (String, bool) {
    use crate::deploy::State;
    let tag = if dry_run { "(dry-run) " } else { "" };
    let mut skipped = 0usize;
    let at = &report.label;
    // Whether force applied is now decided by the shim (Report::acted),
    // not re-derived here — that re-derivation is exactly how "install
    // --force over a real file" once reported a deletion that never
    // happened: create() correctly refuses Occupied even with force,
    // but a (state, force) guess here assumed force always acted.
    let line = match (&report.state, report.acted) {
        (State::Ok(_) | State::Missing, _) => format!("{tag}done          {at}"),
        (State::SourceMissing, _) => format!("{tag}skip          {at} (source missing)"),
        (State::Wrong { found, .. }, true) => {
            format!("{tag}replaced      {at} (was {})", found.display())
        }
        (State::Wrong { found, .. }, false) => {
            skipped += 1;
            format!(
                "{tag}skip          {at} (points at {}; --force to replace)",
                found.display()
            )
        }
        (State::Occupied("dir"), _) => {
            skipped += 1;
            format!("{tag}skip          {at} (real dir; not ours — refusing)")
        }
        (State::Occupied(what), true) => format!("{tag}removed       {at} (was {what})"),
        (State::Occupied(what), false) => {
            skipped += 1;
            format!("{tag}skip          {at} (existing {what}; --force to replace)")
        }
        (State::Legacy(found), _) => {
            format!("{tag}removed       {at} (old link to {})", found.display())
        }
        (State::NoCli(cli), _) => format!("{tag}skip          {at} ({cli} not on PATH)"),
        // The user's own skill in the agent's dir: expected, not a failure.
        (State::Theirs, _) => format!("{tag}skip          {at} (not mAId's; kept)"),
        (State::Unreadable(why), _) => {
            skipped += 1;
            format!("{tag}skip          {at} (unreadable: {why})")
        }
        (State::Current(_) | State::Stale { .. } | State::Disabled { .. }, true) if removing => {
            format!("{tag}removed       {at}")
        }
        (State::Stale { found, .. }, true) => format!("{tag}updated       {at} (was {found})"),
        (State::Disabled { found, .. }, true) => {
            format!("{tag}updated       {at} (was {found}; still disabled)")
        }
        (State::Disabled { found, want }, false) if found != want => {
            format!("{tag}skip          {at} (disabled, not updated; enable it, then just install)")
        }
        (State::Disabled { .. }, false) => format!("{tag}done          {at} (disabled)"),
        (State::Current(_) | State::Stale { .. }, _) => format!("{tag}done          {at}"),
    };
    (line, skipped > 0)
}

// ─────────────────────────────────────────────────────────────────
// Stage 2 · check — pre-install verification.
//
// The three explicit kinds, reading each skill from the CHECKOUT. No
// install, no $HOME: a content change is provable before it is made live
// for every session on the machine.
//
// Stage 4 · smoke — post-install verification.
//
// The two implicit kinds, where the agent must find the skill among
// everything deployed. Requires install to have run.
// ─────────────────────────────────────────────────────────────────

/// Run one verification stage. `dry_run` constructs and structurally
/// checks every prompt without calling an agent, which costs nothing.
/// How a sweep runs, beyond what it selects.
#[derive(Clone, Copy, Debug)]
pub struct RunOptions<'a> {
    pub dry_run: bool,
    pub stress: Option<Stress<'a>>,
    /// Runs per test. Above 1, each test also prints a `TALLY` line.
    pub repeat: usize,
    /// Add a control run, without the skill, beside each enact test.
    pub control: bool,
}

pub fn cmd_verify(
    target: &impl Deploy,
    checkout: &Path,
    selection: &Selection,
    opts: RunOptions,
) -> Result<u8> {
    let RunOptions {
        dry_run,
        stress: stressed,
        repeat,
        control,
    } = opts;
    let repeat = repeat.max(1);
    // Under `verify`, a --kind naming only the other stage's kinds leaves
    // this stage with nothing to do. That is a scoped-away no-op, not a
    // failure — the other stage runs the tests.
    if selection.kinds.is_empty() {
        println!("no {} kinds selected", selection.stage);
        return Ok(0);
    }

    let (fixtures, broken) = load_fixtures(checkout, selection)?;
    if fixtures.is_empty() && broken.is_empty() {
        // A typo in the fixture selector is a bad invocation, exactly
        // like a bad --agent or --kind value — it must exit 2, not 1.
        return Err(usage(format!(
            "no fixtures matched{}",
            selection
                .fixture
                .as_deref()
                .map(|f| format!(" selector {f:?}"))
                .unwrap_or_default()
        )));
    }

    // Smoke reads the deployed tree, so say so plainly rather than
    // failing every test with a confusing path error.
    if selection.stage == Stage::Smoke && !dry_run {
        require_installed(target, selection)?;
    }

    let judge = (!dry_run)
        .then(|| {
            let available: Vec<Agent> = Agent::ALL
                .iter()
                .copied()
                .filter(|a| agent_available(*a))
                .collect();
            judge_agent(std::env::var("VERIFY_JUDGE").ok().as_deref(), &available)
        })
        .transpose()?;

    let before = snapshot_checkout(checkout);
    // Each unparseable fixture is one failure, reported before any test
    // runs so it is visible at the top rather than buried mid-sweep.
    let mut failures = broken.len();
    for why in &broken {
        report("fixture", &Outcome::Fail(why.clone()));
    }
    // The generated kinds are per-skill; several fixtures share a skill.
    let mut claimed = Vec::new();

    for fixture in &fixtures {
        // The skill's text, read once per fixture. An explicit prompt
        // carries it inline, so a check test installs nothing; an implicit
        // one needs it only to know whether a marker may be asserted.
        let body = match fs::read_to_string(checkout_skill(checkout, &fixture.skill)) {
            Ok(body) => body,
            Err(e) => {
                report(
                    "fixture",
                    &Outcome::Fail(format!("cannot read {}: {e}", fixture.skill)),
                );
                failures += 1;
                continue;
            }
        };

        for (kind, agent) in plan_tests(fixture, selection, &mut claimed) {
            // Decide everything first (pure), then do it (effectful).
            let plan = match plan_one(fixture, kind, agent, &body, stressed) {
                Ok(plan) => plan,
                Err(Skipped(why)) => {
                    report(&test_name_for(fixture, kind, agent), &Outcome::Skip(why));
                    continue;
                }
            };
            let plans = std::iter::once(plan.clone())
                .chain(control.then(|| control_plan(&plan, fixture)).flatten());
            for plan in plans {
                let mut passed = 0;
                for run in 1..=repeat {
                    let outcome = match dry_run {
                        true => check_plan(&plan),
                        false => execute(&plan, judge),
                    };
                    let name = match repeat {
                        1 => plan.name.clone(),
                        _ => format!("{} [run {run}/{repeat}]", plan.name),
                    };
                    report(&name, &outcome);
                    match outcome {
                        Outcome::Fail(_) => failures += 1,
                        Outcome::Pass(_) => passed += 1,
                        Outcome::Skip(_) => {}
                    }
                }
                if repeat > 1 {
                    println!("TALLY {}: {passed}/{repeat}", plan.name);
                }
            }
        }
    }

    // Behavioral tests can escape their scratch dir; catch it here rather
    // than leaving it to be found in git log later.
    if let (Some(before), Some(after)) = (before, snapshot_checkout(checkout)) {
        let leaked = detect_leak(&before, &after);
        if !leaked.is_empty() {
            eprintln!(
                "\x1b[31mLEAK\x1b[0m a test wrote into the checkout (not its scratch workdir):"
            );
            for line in &leaked {
                eprintln!("  {line}");
            }
            eprintln!("  also check git log — a leaked close-notes verb can COMMIT these.");
            failures += 1;
        }
    }

    println!();
    match failures {
        0 => {
            println!("all tests passed");
            Ok(0)
        }
        n => {
            println!("{n} test(s) failed");
            Ok(1)
        }
    }
}

fn report(name: &str, outcome: &Outcome) {
    let colour = match outcome {
        Outcome::Pass(_) => "32",
        Outcome::Fail(_) => "31",
        Outcome::Skip(_) => "33",
    };
    let line = format!("\x1b[{colour}m{}\x1b[0m {name}", outcome.token());
    match outcome {
        Outcome::Fail(_) => eprintln!("{line}"),
        _ => println!("{line}"),
    }
    if !outcome.detail().is_empty() {
        println!("  {}", outcome.detail());
    }
}

/// Read every `.smoke` under `resources/tests/skills/`, in name order so
/// two runs are diffable.
fn load_fixtures(checkout: &Path, selection: &Selection) -> Result<(Vec<Fixture>, Vec<String>)> {
    let dir = checkout.join("resources/tests/skills");
    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
        .with_context(|| format!("reading fixtures from {}", dir.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "smoke"))
        .collect();
    paths.sort();

    // A malformed fixture fails itself and the run continues, as bash did:
    // aborting the sweep would let one typo cost forty minutes of paid
    // agent time and return no results at all.
    let (parsed, broken): (Vec<_>, Vec<_>) = paths
        .iter()
        .filter_map(|p| {
            let name = p.file_stem()?.to_string_lossy().to_string();
            selection.covers(&name).then_some((name, p))
        })
        .map(|(name, path)| {
            fs::read_to_string(path)
                .with_context(|| format!("reading {}", path.display()))
                .and_then(|body| Fixture::parse(&name, &body))
        })
        .partition(Result::is_ok);

    Ok((
        parsed.into_iter().map(Result::unwrap).collect(),
        broken
            .into_iter()
            .map(|e| format!("{:#}", e.unwrap_err()))
            .collect(),
    ))
}

/// Stop early when the smoke stage's precondition is unmet, naming the
/// verb that fixes it.
fn require_installed(target: &impl Deploy, selection: &Selection) -> Result<()> {
    let missing: Vec<&str> = selection
        .agents
        .iter()
        .filter(|a| !target.is_deployed(**a))
        .map(|a| a.name())
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    Err(anyhow!(
        "smoke needs the skills deployed, but {} has no skills tree.\n\
         Run `just install` first, or use `check` for the \
         pre-install kinds (activation, playback, enact) which need no install.",
        missing.join(", ")
    ))
}

/// Name a test that was skipped before its plan existed.
fn test_name_for(fixture: &Fixture, kind: TestKind, agent: Agent) -> String {
    test_name(
        match kind.announce_only() {
            true => &fixture.skill,
            false => &fixture.name,
        },
        kind,
        agent,
    )
}

/// Check a plan structurally, without calling an agent. Free.
fn check_plan(plan: &Plan) -> Outcome {
    // A control run carries no skill, so it is checked as an implicit
    // prompt is: it must not leak the skill's name, path or marker.
    let kind = match plan.control {
        true => TestKind::Integration,
        false => plan.kind,
    };
    if plan.control {
        return match check_prompt(kind, &plan.skill, &plan.prompt) {
            Ok(()) => Outcome::Pass("dry: control carries no skill".into()),
            Err(e) => Outcome::Fail(format!("dry: {e}")),
        };
    }
    match check_prompt(kind, &plan.skill, &plan.prompt) {
        Ok(()) => Outcome::Pass(match plan.kind.reach() {
            Reach::Explicit => "dry: carries the skill".into(),
            Reach::Implicit => "dry: names no skill".into(),
        }),
        Err(e) => Outcome::Fail(format!("dry: {e}")),
    }
}

/// Run a plan against its agent and score the result.
fn execute(plan: &Plan, judge: Option<Agent>) -> Outcome {
    if !agent_available(plan.agent) {
        return Outcome::Fail(format!("{} required but not on PATH", plan.agent.name()));
    }
    match &plan.assertion {
        Assertion::Behavioral { setup, assert } => {
            run_behavioral(plan.agent, &plan.prompt, setup, assert, &plan.name)
        }
        Assertion::Reply {
            substr,
            narrative,
            seed,
        } => run_reply(
            plan.agent,
            &plan.prompt,
            substr.as_deref(),
            narrative.as_deref(),
            seed.as_deref(),
            judge,
            &plan.name,
        ),
    }
}

/// Drive an agent read-only and score its reply.
fn run_reply(
    agent: Agent,
    prompt: &str,
    substr: Option<&str>,
    narrative: Option<&str>,
    seed: Option<&str>,
    judge: Option<Agent>,
    label: &str,
) -> Outcome {
    // A scratch cwd, so a relative path in the prompt cannot reach the
    // checkout the runner was started from.
    let Ok(scratch) = tempfile::TempDir::new() else {
        return Outcome::Fail("could not create scratch dir".into());
    };
    if let Some(seed) = seed {
        if let Err(e) = shell(seed, scratch.path()) {
            return Outcome::Fail(format!("setup failed: {e}"));
        }
    }
    let reply = match drive(agent, prompt, Authority::ReadOnly, Some(scratch.path())) {
        Ok(r) => r,
        Err(e) => return Outcome::Fail(format!("{} invocation failed: {e}", agent.name())),
    };

    // Bash printed a 200-char head on both common reply failures; without
    // it the two most frequent outcomes give the reader nothing to look
    // at, while the rarer unparseable case gets a whole file.
    let head: String = reply.chars().take(200).collect();
    let mut dump = None;
    let verdict = match (narrative, judge) {
        (Some(want), Some(judge)) => {
            let jp = judge_prompt(prompt, &reply, want);
            match drive(judge, &jp, Authority::ReadOnly, Some(scratch.path())) {
                Ok(raw) => {
                    let verdict = read_verdict(&raw);
                    if verdict == Verdict::Unparseable {
                        let path = dump_path("verify-judge", label);
                        let _ = fs::write(&path, &raw);
                        dump = Some(path);
                    }
                    Some(verdict)
                }
                Err(e) => return Outcome::Fail(format!("judge invocation failed: {e}")),
            }
        }
        (Some(_), None) => return Outcome::Fail("no judge available".into()),
        (None, _) => None,
    };
    match score_reply(&reply, substr, verdict.as_ref(), dump.as_deref()) {
        Outcome::Fail(why) => Outcome::Fail(format!("{why}\n  reply head: {head}...")),
        other => other,
    }
}

/// Seed a scratch workdir, run the agent inside it, then run the fixture's
/// assert shell against the result. Tests execution, not recall.
///
/// The assert runs under `set -x` so a failure names the exact check that
/// broke: asserts are silent `test`/`grep -q` under `set -e`, so without
/// the trace every behavioral failure read "assert failed" with nothing
/// after it — undiagnosable without hand-reproducing the fixture.
fn run_behavioral(agent: Agent, prompt: &str, setup: &str, assert: &str, label: &str) -> Outcome {
    let Ok(work) = tempfile::TempDir::new() else {
        return Outcome::Fail("could not create scratch workdir".into());
    };
    let dir = work.path();

    if let Err(e) = shell(setup, dir) {
        return Outcome::Fail(format!("setup failed: {e}"));
    }

    // The agent's own failure is not fatal: the assert decides, since a
    // non-zero exit with correct artefacts is still a pass.
    let reply = drive(agent, prompt, Authority::Workdir, Some(dir)).unwrap_or_default();

    match shell(&format!("set -x\n{assert}"), dir) {
        Ok(_) => Outcome::Pass("behavioral".into()),
        Err(trace) => {
            let culprit = trace
                .lines()
                .rfind(|l| l.starts_with('+'))
                .map(|l| l.trim_start_matches('+').trim())
                .unwrap_or("<no trace>")
                .to_string();
            // The scratch dir is removed on drop, so the tree is the only
            // record of what the agent actually created — often the
            // fastest way to see a file landed one directory up.
            let tree = shell("find . -not -path '*/.git/*' | head -60", dir).unwrap_or_default();
            let dump = dump_path("verify-behavioral", label);
            let _ = fs::write(
                &dump,
                format!(
                    "=== assert trace ===\n{trace}\n\
                     === agent reply ===\n{reply}\n\
                     === workdir tree ===\n{tree}\n"
                ),
            );
            Outcome::Fail(format!(
                "failed check: {culprit} (trace + reply → {})",
                dump.display()
            ))
        }
    }
}

/// Run a shell fragment in `dir`, returning its combined output on
/// failure. `set -e` so a fixture's first broken command stops it.
fn shell(script: &str, dir: &Path) -> std::result::Result<String, String> {
    let out = std::process::Command::new("bash")
        .arg("-c")
        .arg(format!("set -e\n{script}"))
        .current_dir(dir)
        .output()
        .map_err(|e| e.to_string())?;
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    match out.status.success() {
        true => Ok(combined),
        false => Err(combined),
    }
}

/// Invoke a coding agent and return its REPLY — not the transcript.
fn drive(
    agent: Agent,
    prompt: &str,
    authority: Authority,
    workdir: Option<&Path>,
) -> Result<String> {
    let last = tempfile::NamedTempFile::new().context("creating reply file")?;
    let inv = invocation(agent, prompt, authority, workdir, last.path());
    if let (Agent::Kiro, Authority::ReadOnly, Some(dir)) = (agent, authority, workdir) {
        let agents = dir.join(".kiro/agents");
        fs::create_dir_all(&agents).context("creating kiro agent dir")?;
        fs::write(
            agents.join(format!("{KIRO_READONLY_NAME}.json")),
            KIRO_READONLY_AGENT,
        )
        .context("writing kiro read-only profile")?;
    }

    let mut cmd = std::process::Command::new(&inv.program);
    cmd.args(&inv.args).stdin(std::process::Stdio::null());
    if let Some(dir) = &inv.cwd {
        cmd.current_dir(dir);
    }
    let out = cmd
        .output()
        .with_context(|| format!("running {}", inv.program))?;

    // An agent that writes its final message to a file: read that, and
    // never the transcript on stdout.
    let reply = match &inv.reply_file {
        Some(path) => fs::read_to_string(path).context("reading agent reply file")?,
        None => format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    };

    // A non-zero exit is the agent failing, not the skill: an expired
    // token or a rate limit would otherwise be scored as a reply and
    // reported as "response missing <marker>", sending the reader to
    // debug content that is fine.
    if !out.status.success() {
        return Err(anyhow!(
            "exited {} — {}",
            out.status
                .code()
                .map_or("by signal".into(), |c| c.to_string()),
            reply.trim().lines().next_back().unwrap_or("no output")
        ));
    }
    Ok(reply)
}

// ─────────────────────────────────────────────────────────────────
// Tests.
// ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Validate frontmatter through the real file path, as production does.
    fn frontmatter_of(body: &str) -> Result<(), String> {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("SKILL.md");
        fs::write(&p, body).unwrap();
        check_one_skill(&p)
    }

    fn write(p: &Path, s: &str) {
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, s).unwrap();
    }

    // ── install / uninstall lines ───────────────────────────────

    /// The plugin lines a user reads, and which ones fail the run.
    #[test]
    fn plugin_outcome_lines_say_what_happened() {
        use crate::deploy::{Report, State};
        let line = |state: State, acted: bool, removing: bool| {
            let r = Report {
                label: "codex plugin maid@maid".into(),
                state,
                acted,
            };
            outcome_line(&r, false, removing)
        };
        let dis = |found: &str| State::Disabled {
            found: found.into(),
            want: "b".into(),
        };
        for (got, want, fails) in [
            (line(dis("a"), true, false), "updated       codex plugin maid@maid (was a; still disabled)", false),
            (line(dis("a"), false, false), "skip          codex plugin maid@maid (disabled, not updated; enable it, then just install)", false),
            (line(dis("b"), false, false), "done          codex plugin maid@maid (disabled)", false),
            (line(dis("a"), true, true), "removed       codex plugin maid@maid", false),
            (line(State::Stale { found: "a".into(), want: "b".into() }, true, false), "updated       codex plugin maid@maid (was a)", false),
            (line(State::Stale { found: "a".into(), want: "b".into() }, true, true), "removed       codex plugin maid@maid", false),
            (line(State::NoCli("codex"), false, false), "skip          codex plugin maid@maid (codex not on PATH)", false),
            (line(State::Unreadable("refused".into()), false, false), "skip          codex plugin maid@maid (unreadable: refused)", true),
            (line(State::Legacy("/p/share/maid/skills".into()), true, false), "removed       codex plugin maid@maid (old link to /p/share/maid/skills)", false),
            (line(State::Theirs, false, true), "skip          codex plugin maid@maid (not mAId's; kept)", false),
        ] {
            assert_eq!(got, (want.to_string(), fails));
        }
    }

    // ── content checks ──────────────────────────────────────────
    #[test]
    fn check_content_empty_root_ok() {
        let dir = TempDir::new().unwrap();
        assert_eq!(check_content(dir.path()).unwrap(), 0);
    }

    #[test]
    fn check_content_skill_with_frontmatter_ok() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("skills/foo/SKILL.md"),
            "---\nname: foo\ndescription: bar\n---\nbody.\n",
        );
        assert_eq!(check_content(dir.path()).unwrap(), 1);
    }

    /// A missing tree validates as zero files, so without the guard an
    /// install with no profile would link every agent at nothing.
    #[test]
    fn install_without_a_profile_names_the_fix() {
        let dir = TempDir::new().unwrap();
        let err = cmd_install(&crate::deploy::NoDeploy, dir.path(), false, false, None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("just install"), "{err}");
    }

    #[test]
    fn check_content_skill_missing_description_rejected() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("skills/foo/SKILL.md"),
            "---\nname: foo\n---\n",
        );
        let errs = check_content(dir.path()).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("description")));
    }

    #[test]
    fn check_content_collects_multiple_errors() {
        // Two bad skills — caller sees ALL problems, not just the first.
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("skills/foo/SKILL.md"),
            "---\nname: foo\n---\n",
        );
        write(
            &dir.path().join("skills/bar/SKILL.md"),
            "---\nname: bar\n---\n",
        );
        let errs = check_content(dir.path()).unwrap_err();
        assert_eq!(errs.len(), 2);
    }

    #[test]
    fn check_content_skills_only_skill_md_parsed() {
        // Sibling files in a skill dir must NOT be parsed; only SKILL.md.
        let dir = TempDir::new().unwrap();
        let skill_dir = dir.path().join("skills/multi");
        write(
            &skill_dir.join("SKILL.md"),
            "---\nname: multi\ndescription: a multi-file skill\n---\nbody.\n",
        );
        write(&skill_dir.join("setup.md"), "# Plain markdown.\n");
        assert_eq!(check_content(dir.path()).unwrap(), 1);
    }

    // ── frontmatter (gray_matter + serde) ───────────────────────

    #[test]
    fn frontmatter_minimal_ok() {
        assert!(frontmatter_of("---\nname: foo\ndescription: bar\n---\nbody.\n").is_ok());
    }

    #[test]
    fn frontmatter_with_extra_fields_ok() {
        // Real YAML: extra fields are ignored by serde when not in the struct.
        assert!(frontmatter_of(
            "---\nname: foo\ndescription: bar\nversion: 1.0.0\ntags: [a, b]\n---\nbody.\n"
        )
        .is_ok());
    }

    #[test]
    fn frontmatter_quoted_values_ok() {
        // Real YAML handles quoting natively — no hand-rolled unquote.
        assert!(frontmatter_of("---\nname: \"foo\"\ndescription: 'bar baz'\n---\n").is_ok());
    }

    #[test]
    fn frontmatter_missing_fence_rejected() {
        let e = frontmatter_of("name: foo\ndescription: bar\n").unwrap_err();
        assert!(e.contains("frontmatter"));
    }

    #[test]
    fn frontmatter_unterminated_rejected() {
        let e = frontmatter_of("---\nname: foo\ndescription: bar\n").unwrap_err();
        assert!(e.contains("frontmatter"));
    }

    #[test]
    fn frontmatter_missing_name_rejected() {
        let e = frontmatter_of("---\ndescription: bar\n---\n").unwrap_err();
        assert!(e.contains("name"));
    }

    #[test]
    fn frontmatter_missing_description_rejected() {
        let e = frontmatter_of("---\nname: foo\n---\n").unwrap_err();
        assert!(e.contains("description"));
    }

    /// A malformed fixture fails itself; the sweep continues. Bash did
    /// this, and aborting would let one typo cost a whole paid run.
    #[test]
    fn a_malformed_fixture_does_not_abort_the_others() {
        let dir = TempDir::new().unwrap();
        let skills = dir.path().join("resources/tests/skills");
        fs::create_dir_all(&skills).unwrap();
        fs::write(
            skills.join("good.smoke"),
            "skill: notes\n--- enact ---\ntask: t\nexpect: n\n",
        )
        .unwrap();
        // No skill: field — unparseable.
        fs::write(
            skills.join("bad.smoke"),
            "--- enact ---\ntask: t\nexpect: n\n",
        )
        .unwrap();

        let selection = Selection::resolve(Stage::Check, None, None, None).unwrap();
        let (parsed, broken) = load_fixtures(dir.path(), &selection).unwrap();

        assert_eq!(parsed.len(), 1, "the good fixture must still load");
        assert_eq!(parsed[0].name, "good");
        assert_eq!(broken.len(), 1, "the bad one is reported, not fatal");
        assert!(broken[0].contains("bad"), "{}", broken[0]);
    }

    /// The selector still scopes which fixtures are read at all.
    #[test]
    fn load_fixtures_honours_the_fixture_selector() {
        let dir = TempDir::new().unwrap();
        let skills = dir.path().join("resources/tests/skills");
        fs::create_dir_all(&skills).unwrap();
        for name in ["a", "b"] {
            fs::write(
                skills.join(format!("{name}.smoke")),
                "skill: notes\n--- enact ---\ntask: t\nexpect: n\n",
            )
            .unwrap();
        }
        let selection = Selection::resolve(Stage::Check, Some("a"), None, None).unwrap();
        let (parsed, broken) = load_fixtures(dir.path(), &selection).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "a");
        assert!(broken.is_empty());
    }

    /// A fixture-name typo is a bad invocation exactly like a bad --agent
    /// or --kind, and must exit 2 — not 1, which is reserved for "the run
    /// happened and a test failed".
    #[test]
    fn an_unmatched_fixture_selector_is_a_usage_error() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("resources/tests/skills")).unwrap();
        write(
            &dir.path().join("resources/tests/skills/real.smoke"),
            "skill: notes\n--- enact ---\ntask: t\nexpect: n\n",
        );
        let selection = Selection::resolve(Stage::Check, Some("typo-of-real"), None, None).unwrap();
        let err = cmd_verify(
            &crate::deploy::NoDeploy,
            dir.path(),
            &selection,
            RunOptions {
                dry_run: true,
                stress: None,
                repeat: 1,
                control: false,
            },
        )
        .unwrap_err();
        assert!(
            err.downcast_ref::<crate::shared::UsageError>().is_some(),
            "{err}"
        );
        assert!(err.to_string().contains("typo-of-real"));
    }
}
