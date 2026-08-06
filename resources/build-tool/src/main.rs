//! build-tool — the binary shim: parse the CLI, resolve the roots and
//! the `--agent` token, then dispatch into a stage. All the work lives
//! in the pipeline; see `lib.rs` for the crate's shape.
//!
//! Invoked via the project's Justfile, in pipeline order:
//!   just resources::check-skills [agent]      verify each skill from the checkout (no install needed)
//!   just resources::install-skills [agent]    create the $HOME-facing symlinks (after content checks)
//!   just resources::uninstall-skills [agent]  remove the managed symlinks
//!   just resources::status-skills [agent]     report each managed symlink's current state
//!   just resources::smoke-skills [agent]      verify against the deployed tree
//!   just resources::verify-skills [agent]     both verification stages
//! An optional `--agent <claude|kiro|codex>` scopes any of them to one
//! coding agent (the verification verbs take a comma list); the default
//! is all of them.

use anyhow::Result;
use build_tool::deploy::{NoDeploy, Symlinks};
use build_tool::harness::{Selection, Stage};
use build_tool::shared::{home_dir, repo_root, usage, validate_agent, validate_agents, UsageError};
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
    /// Scope to one or more coding agents (claude|kiro|codex, comma
    /// separated); default all.
    #[arg(long)]
    agent: Option<String>,
    /// Construct and structurally check every prompt without calling an
    /// agent. Costs nothing.
    #[arg(long)]
    dry_run: bool,
    /// Prepend a long conversational prefix to stress retention.
    #[arg(long)]
    stressed: bool,
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
    /// Scope to one coding agent (claude|kiro|codex); default all.
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
    /// Validate content and deploy it so the agents can find it.
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
    // The one place the deployment mechanism is chosen. When an agent
    // grows its own install command, a different `Deploy` impl goes here
    // and no stage changes.
    //
    // Lazy, because `check` must work with no $HOME at all — it carries
    // each skill inline, so resolving a home would reintroduce the
    // coupling that stage exists to avoid.
    let deployment = || -> Result<Symlinks> {
        Ok(Symlinks {
            home: home_dir()?,
            checkout: root.clone(),
        })
    };
    match cli.cmd {
        // No deployment target at all: the guarantee is structural.
        Cmd::Check(args) => verify(Stage::Check, args, &NoDeploy, &root, false),
        Cmd::Install(a) => stages::cmd_install(
            &deployment()?,
            &root.join("resources/content"),
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

/// `Verify` runs both stages, so its arguments are consumed twice.
fn clone_args(a: &VerifyArgs) -> VerifyArgs {
    VerifyArgs {
        fixture: a.fixture.clone(),
        kind: a.kind.clone(),
        agent: a.agent.clone(),
        dry_run: a.dry_run,
        stressed: a.stressed,
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
    let stress = args
        .stressed
        .then(|| std::fs::read_to_string(root.join("resources/tests/conversational-stream.txt")))
        .transpose()
        .map_err(|e| {
            usage(format!(
                "--stressed needs resources/tests/conversational-stream.txt: {e}"
            ))
        })?;
    stages::cmd_verify(target, root, &selection, args.dry_run, stress.as_deref())
}
