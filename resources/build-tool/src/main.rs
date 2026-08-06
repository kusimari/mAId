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

/// The flags both verification stages share. They differ only in which
/// side of install they read a skill from, so they take the same surface.
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

#[derive(Subcommand)]
enum Cmd {
    /// Verify skills BEFORE install: the kinds whose prompt carries the
    /// skill's text, so no deployment is needed.
    Check(VerifyArgs),
    /// Verify skills AFTER install, from the deployed tree: the kinds
    /// where the agent must find the skill among everything installed.
    Smoke(VerifyArgs),
    /// Both stages: check, then smoke. Runs the second even when the
    /// first reports failures, so one sweep is one report.
    Verify(VerifyArgs),
    /// Validate content and create/refresh $HOME-facing symlinks.
    Install {
        /// Plan without making changes.
        #[arg(long)]
        dry_run: bool,
        /// Replace symlinks that point elsewhere.
        #[arg(long)]
        force: bool,
        /// Scope to one coding agent (claude|kiro|codex); default all.
        #[arg(long)]
        agent: Option<String>,
    },
    /// Remove install-managed symlinks.
    Uninstall {
        /// Plan without making changes.
        #[arg(long)]
        dry_run: bool,
        /// Remove whatever is at the managed path, including foreign
        /// symlinks and non-symlinks.
        #[arg(long)]
        force: bool,
        /// Scope to one coding agent (claude|kiro|codex); default all.
        #[arg(long)]
        agent: Option<String>,
    },
    /// Report each managed symlink's state.
    Status {
        /// Scope to one coding agent (claude|kiro|codex); default all.
        #[arg(long)]
        agent: Option<String>,
    },
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

/// Resolve the roots and the `--agent` token here, so a stage receives
/// validated values and never sees a raw CLI string.
fn run(cli: Cli) -> Result<u8> {
    let root = repo_root()?;
    // How skills reach the agents. The only place this choice is made:
    // everything below declares what it wants and this decides how. When
    // an agent grows its own install command, a different `Deploy` impl
    // goes here and nothing downstream changes.
    //
    // Resolved lazily, because `check` must work with no $HOME at all —
    // it carries each skill inline, so requiring a home would reintroduce
    // the coupling that stage exists to avoid.
    let target = || -> Result<Symlinks> {
        Ok(Symlinks {
            home: home_dir()?,
            checkout: root.clone(),
        })
    };
    match cli.cmd {
        Cmd::Install {
            dry_run,
            force,
            agent,
        } => stages::cmd_install(
            &target()?,
            &root.join("resources/content"),
            dry_run,
            force,
            validate_agent(agent.as_deref())?,
        ),
        Cmd::Uninstall {
            dry_run,
            force,
            agent,
        } => stages::cmd_uninstall(
            &target()?,
            dry_run,
            force,
            validate_agent(agent.as_deref())?,
        ),
        Cmd::Status { agent } => stages::cmd_status(&target()?, validate_agent(agent.as_deref())?),
        // check needs no deployment, so it never resolves $HOME.
        Cmd::Check(args) => verify(Stage::Check, args, &NoDeploy, &root, false),
        Cmd::Smoke(args) => verify(Stage::Smoke, args, &target()?, &root, false),
        Cmd::Verify(args) => {
            // Both stages, and the second runs even when the first fails:
            // a 40-minute sweep that stops a third of the way in is a
            // different tool from one that reports everything. The
            // check-passes/smoke-fails split is the diagnostic this
            // feature is largely justified by, so both halves must run.
            let check = verify(Stage::Check, clone_args(&args), &NoDeploy, &root, true)?;
            let smoke = verify(Stage::Smoke, args, &target()?, &root, true)?;
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
