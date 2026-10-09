//! build-tool — the binary shim: parse the CLI, resolve the roots and
//! the `--agent` token, then dispatch into a stage. All the work lives
//! in the pipeline; see `lib.rs` for the crate's shape.
//!
//! Invoked via the project's Justfile, in pipeline order:
//!   just resources::check-skills [agent]      verify each skill from the checkout (no install needed)
//!   just resources::install-profile           validate the checkout, build it into the profile
//!   just resources::install-skills [agent]    install the claude/codex plugin, link the rest at the profile
//!   just resources::uninstall-skills [agent]  remove the plugin and the managed symlinks
//!   just resources::status-skills [agent]     report each plugin's version and each managed symlink
//!   just resources::smoke-skills [agent]      verify against the deployed tree
//!   just resources::verify-skills [agent]     both verification stages
//! An optional `--agent <claude|kiro|codex|agy>` scopes any of them to one
//! coding agent (the verification verbs take a comma list); the default
//! is all of them.

use anyhow::Result;
use build_tool::deploy::{Deployment, NoDeploy, Plugins, Symlinks};
use build_tool::harness::{Selection, Stage};
use build_tool::shared::{
    home_dir, profile_dir, repo_root, usage, validate_agent, validate_agents, UsageError,
    CONTENT_DIR, PROFILE_CONTENT_DIR,
};
use build_tool::stages;
use clap::{Parser, Subcommand};
use std::path::Path;
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "build-tool",
    about = "mAId build-tool — check / install / uninstall / status / smoke."
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

/// The flags the two verification stages share — they differ only in
/// which side of install they read a skill from.
#[derive(clap::Args)]
struct VerifyArgs {
    /// Only this fixture (its basename without `.smoke`).
    fixture: Option<String>,
    /// Comma-separated kinds; default every kind this stage owns.
    #[arg(long)]
    kind: Option<String>,
    /// Scope to one or more coding agents (claude|kiro|codex|agy, comma
    /// separated); default all.
    #[arg(long)]
    agent: Option<String>,
    /// Construct and structurally check every prompt without calling an
    /// agent. Costs nothing.
    #[arg(long)]
    dry_run: bool,
    /// Prepend a long conversational prefix to stress retention.
    #[arg(long, conflicts_with = "drift")]
    stressed: bool,
    /// Put a long unrelated conversation between the skill and the task,
    /// to test rules read early and acted on later.
    #[arg(long)]
    drift: bool,
    /// Run each test this many times and print a pass tally per test.
    #[arg(long, default_value_t = 1)]
    repeat: usize,
    /// Add a control run, without the skill, beside each enact test.
    #[arg(long)]
    control: bool,
}

/// The flags the three deployment verbs share.
#[derive(clap::Args)]
struct DeployArgs {
    /// Plan without making changes.
    #[arg(long)]
    dry_run: bool,
    /// Act even where something not ours is in the way.
    #[arg(long)]
    force: bool,
    /// Scope to one coding agent (claude|kiro|codex|agy); default all.
    #[arg(long)]
    agent: Option<String>,
}

/// The verbs, in pipeline order. Each arm's doc comment is its `--help`
/// text and `run()` below dispatches in the same order, so a reader sees
/// the whole surface and what each verb does in two adjacent places.
#[derive(Subcommand)]
enum Cmd {
    /// Verify skills BEFORE install: the kinds whose prompt carries the
    /// skill's text, so no deployment is needed.
    Check(VerifyArgs),
    /// Validate the checkout's content, before it is built into the profile.
    Validate,
    /// Validate the profile's content, install the claude/codex plugin,
    /// and link the other agents at it.
    Install(DeployArgs),
    /// Remove what install deployed, leaving anything not ours.
    Uninstall(DeployArgs),
    /// Report what is deployed and whether it points where it should.
    Status(DeployArgs),
    /// Verify skills AFTER install, from the deployed tree: the kinds
    /// where the agent must find the skill among everything installed.
    Smoke(VerifyArgs),
    /// Both verification stages. Runs the second even when the first
    /// reports failures, so one sweep is one report.
    Verify(VerifyArgs),
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(rc) => ExitCode::from(rc),
        Err(e) => {
            eprintln!("build-tool: {e:#}");
            // 2 for a bad invocation, matching clap's own usage errors;
            // 1 is reserved for "the run happened and something failed".
            ExitCode::from(match e.downcast_ref::<UsageError>() {
                Some(_) => 2,
                None => 1,
            })
        }
    }
}

/// Dispatch. The only decisions here are how skills reach the agents and
/// how a `--agent` token resolves; everything else belongs to a stage.
fn run(cli: Cli) -> Result<u8> {
    let root = repo_root()?;
    // The one place the deployment mechanisms are chosen: plugins where
    // the agent has a plugin CLI, links elsewhere. No stage knows which.
    //
    // Lazy, because `check` must work with no $HOME at all — it carries
    // each skill inline, so resolving a home would reintroduce the
    // coupling that stage exists to avoid.
    let deployment = || -> Result<Deployment> {
        Ok(Deployment {
            links: Symlinks {
                home: home_dir()?,
                source: profile_dir()?,
            },
            plugins: Plugins {
                home: home_dir()?,
                profile: profile_dir()?,
            },
        })
    };
    match cli.cmd {
        // No deployment target at all: the guarantee is structural.
        Cmd::Check(args) => verify(Stage::Check, args, &NoDeploy, &root, false),
        Cmd::Validate => stages::cmd_validate(&root.join(CONTENT_DIR)),
        Cmd::Install(a) => stages::cmd_install(
            &deployment()?,
            &profile_dir()?.join(PROFILE_CONTENT_DIR),
            a.dry_run,
            a.force,
            validate_agent(a.agent.as_deref())?,
        ),
        Cmd::Uninstall(a) => stages::cmd_uninstall(
            &deployment()?,
            a.dry_run,
            a.force,
            validate_agent(a.agent.as_deref())?,
        ),
        Cmd::Status(a) => stages::cmd_status(&deployment()?, validate_agent(a.agent.as_deref())?),
        Cmd::Smoke(args) => verify(Stage::Smoke, args, &deployment()?, &root, false),
        Cmd::Verify(args) => {
            let check = verify(Stage::Check, clone_args(&args), &NoDeploy, &root, true)?;
            let smoke = verify(Stage::Smoke, args, &deployment()?, &root, true)?;
            Ok(check.max(smoke))
        }
    }
}

/// How many copies of the stress stream --drift puts after the skill:
/// about 4,000 words.
const DRIFT_REPEATS: usize = 5;

/// `Verify` runs both stages, so its arguments are consumed twice.
fn clone_args(a: &VerifyArgs) -> VerifyArgs {
    VerifyArgs {
        fixture: a.fixture.clone(),
        kind: a.kind.clone(),
        agent: a.agent.clone(),
        dry_run: a.dry_run,
        stressed: a.stressed,
        drift: a.drift,
        repeat: a.repeat,
        control: a.control,
    }
}

/// Both verification stages, which differ only in their `Stage`.
/// `both_stages_run` is set by `verify`, which tolerates a `--kind` this
/// stage does not own because the other stage will run it.
fn verify(
    stage: Stage,
    args: VerifyArgs,
    target: &impl build_tool::deploy::Deploy,
    root: &Path,
    both_stages_run: bool,
) -> Result<u8> {
    let resolve = match both_stages_run {
        true => Selection::resolve_for_both,
        false => Selection::resolve,
    };
    let selection = resolve(
        stage,
        args.fixture.as_deref(),
        args.kind.as_deref(),
        validate_agents(args.agent.as_deref())?,
    )?;
    let stream = (args.stressed || args.drift)
        .then(|| std::fs::read_to_string(root.join("resources/tests/conversational-stream.txt")))
        .transpose()
        .map_err(|e| {
            usage(format!(
                "--stressed and --drift need resources/tests/conversational-stream.txt: {e}"
            ))
        })?
        // Drift repeats the stream so the rules sit thousands of words back.
        .map(|s| match args.drift {
            true => s.repeat(DRIFT_REPEATS),
            false => s,
        });
    let stress = stream.as_deref().map(|s| match args.drift {
        true => build_tool::harness::Stress::AfterSkill(s),
        false => build_tool::harness::Stress::Before(s),
    });
    stages::cmd_verify(
        target,
        root,
        &selection,
        stages::RunOptions {
            dry_run: args.dry_run,
            stress,
            repeat: args.repeat,
            control: args.control,
        },
    )
}
