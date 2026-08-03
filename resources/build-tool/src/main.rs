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

use anyhow::Result;
use build_tool::shared::{home_dir, repo_root, validate_agent};
use build_tool::stages;
use clap::{Parser, Subcommand};
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "build-tool",
    about = "mAId build-tool — install / uninstall / status."
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
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
            ExitCode::from(1)
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
    }
}
