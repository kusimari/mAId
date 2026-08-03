//! build-tool — the binary shim: parse the CLI, resolve the roots and
//! the `--agent` token, then dispatch into a stage. All the work lives
//! in the pipeline; see `lib.rs` for the crate's shape.
//!
//! Invoked via the project's Justfile:
//!   just resources::install-skills [agent]    create the $HOME-facing symlinks (after content checks)
//!   just resources::uninstall-skills [agent]  remove the managed symlinks
//!   just resources::status-skills [agent]     report each managed symlink's current state
//! An optional `--agent <claude|kiro|codex>` scopes any of the three
//! to one coding agent; the default is all of them.

use anyhow::{Context, Result};
use build_tool::harness::{Selection, Stage};
use build_tool::shared::{home_dir, repo_root, validate_agent, validate_agents, UsageError};
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
    /// Verify skills BEFORE install, from the checkout: the kinds whose
    /// prompt names the skill's path, so no deployment is needed.
    Check(VerifyArgs),
    /// Verify skills AFTER install, from the deployed tree: the kinds
    /// where the agent must find the skill among everything installed.
    Smoke(VerifyArgs),
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
    let home = home_dir()?;
    match cli.cmd {
        Cmd::Install {
            dry_run,
            force,
            agent,
        } => stages::cmd_install(
            &home,
            &root,
            dry_run,
            force,
            validate_agent(agent.as_deref())?,
        ),
        Cmd::Uninstall {
            dry_run,
            force,
            agent,
        } => stages::cmd_uninstall(
            &home,
            &root,
            dry_run,
            force,
            validate_agent(agent.as_deref())?,
        ),
        Cmd::Status { agent } => {
            stages::cmd_status(&home, &root, validate_agent(agent.as_deref())?)
        }
        Cmd::Check(args) => verify(Stage::Check, args, &home, &root),
        Cmd::Smoke(args) => verify(Stage::Smoke, args, &home, &root),
    }
}

/// Both verification stages, which differ only in their `Stage`.
fn verify(stage: Stage, args: VerifyArgs, home: &Path, root: &Path) -> Result<u8> {
    let selection = Selection::resolve(
        stage,
        args.fixture.as_deref(),
        args.kind.as_deref(),
        validate_agents(args.agent.as_deref())?,
    )?;
    let stress = args
        .stressed
        .then(|| std::fs::read_to_string(root.join("resources/tests/conversational-stream.txt")))
        .transpose()
        .context("--stressed needs resources/tests/conversational-stream.txt")?;
    stages::cmd_verify(home, root, &selection, args.dry_run, stress.as_deref())
}
