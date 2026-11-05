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
//!   just resources::status-whole [agent]      check every part's list against each harness
//!   just resources::smoke-skills [agent]      verify against the deployed tree
//!   just resources::verify-skills [agent]     both verification stages
//! An optional `--agent <claude|kiro|codex|agy>` scopes any of them to one
//! coding agent (the verification verbs take a comma list); the default
//! is all of them.

use anyhow::{Context, Result};
use build_tool::deploy::{BringIns, Deployment, NoDeploy, Plugins, Symlinks};
use build_tool::harness::{Selection, Stage};
use build_tool::memory;
use build_tool::part::What;
use build_tool::shared::{
    claude_config, codex_home, config_dir, home_dir, memory_store_file, parts_dir, profile_dir,
    repo_root, usage, validate_agent, validate_agents, Agent, UsageError, BRING_INS, CONTENT_DIR,
    PROFILE_CONTENT_DIR,
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
    /// Check every part list in the parts dir against each harness's own
    /// listing or config. Exits 1 when anything placed is missing or
    /// unreadable, or a list was refused.
    Whole {
        /// Scope to one coding agent (claude|kiro|codex|agy); default all.
        #[arg(long)]
        agent: Option<String>,
    },
    /// Verify skills AFTER install, from the deployed tree: the kinds
    /// where the agent must find the skill among everything installed.
    Smoke(VerifyArgs),
    /// Both verification stages. Runs the second even when the first
    /// reports failures, so one sweep is one report.
    Verify(VerifyArgs),
    /// Record, in mAId's part list, what a step outside install placed on
    /// one agent: replaces that agent's entries of this kind and scope.
    Record {
        #[arg(long)]
        agent: String,
        /// skill|mcp|agent|plugin|marketplace|setting
        #[arg(long)]
        kind: String,
        /// Where in the agent, when it has more than one place.
        #[arg(long)]
        scope: Option<String>,
        /// The step meant to place these and did not, and why.
        #[arg(long)]
        skipped: Option<String>,
        /// Replace only the entries with this name, keeping the kind's others.
        #[arg(long)]
        only: Option<String>,
        /// What was placed; none clears this kind and scope.
        names: Vec<String>,
    },
    /// The personal memory store: set it, report it, harvest into a store.
    #[command(subcommand)]
    Memory(MemoryCmd),
}

#[derive(Subcommand)]
enum MemoryCmd {
    /// Set the personal store (an existing checkout).
    SetStore { path: std::path::PathBuf },
    /// Report the store, its server on claude, and Claude's own memory.
    Status,
    /// Copy each harness's own memory into a store dir, leaving the originals.
    Harvest {
        /// The store dir; memories land in its `memory/`.
        #[arg(long)]
        to: std::path::PathBuf,
        /// Only Claude's projects whose working directory starts with this.
        #[arg(long)]
        project: Option<String>,
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
    //
    // Only install and uninstall act on what mAId owns, so only they stop
    // on a part list that cannot be read; the rest report it.
    let deployment_for = |acting: bool| -> Result<Deployment> {
        Ok(Deployment {
            links: Symlinks {
                home: home_dir()?,
                source: profile_dir()?,
            },
            plugins: Plugins {
                home: home_dir()?,
                claude: claude_config()?,
                profile: profile_dir()?,
            },
            bring_ins: match acting {
                true => BringIns::new(home_dir()?, claude_config()?, BRING_INS, &parts_dir()?)?,
                false => {
                    BringIns::reporting(home_dir()?, claude_config()?, BRING_INS, &parts_dir()?)
                }
            },
        })
    };
    let deployment = || deployment_for(false);
    match cli.cmd {
        // No deployment target at all: the guarantee is structural.
        Cmd::Check(args) => verify(Stage::Check, args, &NoDeploy, &root, false),
        Cmd::Validate => stages::cmd_validate(&root.join(CONTENT_DIR)),
        Cmd::Install(a) => {
            let agent = validate_agent(a.agent.as_deref())?;
            let rc = stages::cmd_install(
                &deployment_for(true)?,
                &profile_dir()?.join(PROFILE_CONTENT_DIR),
                &parts_dir()?,
                a.dry_run,
                a.force,
                agent,
            )?;
            let memory = stages::cmd_install_memory(
                &claude_config()?,
                &memory_store_file()?,
                &profile_dir()?,
                &parts_dir()?,
                a.dry_run,
                agent,
            )?;
            let reads = stages::cmd_install_reads(
                &claude_config()?,
                &profile_dir()?,
                &config_dir()?,
                &parts_dir()?,
                a.dry_run,
                agent,
            )?;
            // What the ghost is made of, whichever harness was selected.
            let bring_ins = stages::cmd_install_bring_ins(
                &parts_dir()?,
                &profile_dir()?,
                &config_dir()?,
                stages::Checkout::read(&root),
                &memory_store_file()?,
                BRING_INS,
                a.dry_run,
            )?;
            Ok(rc.max(memory).max(reads).max(bring_ins))
        }
        Cmd::Uninstall(a) => {
            let agent = validate_agent(a.agent.as_deref())?;
            let rc = stages::cmd_uninstall(
                &deployment_for(true)?,
                &parts_dir()?,
                a.dry_run,
                a.force,
                agent,
            )?;
            let memory = stages::cmd_uninstall_memory(
                &claude_config()?,
                &profile_dir()?,
                &parts_dir()?,
                a.dry_run,
                agent,
            )?;
            let reads =
                stages::cmd_uninstall_reads(&claude_config()?, &parts_dir()?, a.dry_run, agent)?;
            let bring_ins = stages::cmd_uninstall_bring_ins(&parts_dir()?, BRING_INS, a.dry_run)?;
            Ok(rc.max(memory).max(reads).max(bring_ins))
        }
        Cmd::Status(a) => stages::cmd_status(&deployment()?, validate_agent(a.agent.as_deref())?),
        Cmd::Whole { agent } => build_tool::whole::cmd_whole(
            &parts_dir()?,
            &home_dir()?,
            &claude_config()?,
            validate_agent(agent.as_deref())?,
        ),
        Cmd::Smoke(args) => verify(Stage::Smoke, args, &deployment()?, &root, false),
        Cmd::Verify(args) => {
            let check = verify(Stage::Check, clone_args(&args), &NoDeploy, &root, true)?;
            let smoke = verify(Stage::Smoke, args, &deployment()?, &root, true)?;
            Ok(check.max(smoke))
        }
        Cmd::Record {
            agent,
            kind,
            scope,
            skipped,
            only,
            names,
        } => {
            let kind = What::parse(&kind).map_err(|e| usage(e.to_string()))?;
            // Resolved before anything is written, so a missing one fails
            // the call with the list untouched.
            let (parts, profile) = (parts_dir()?, profile_dir()?);
            let rc = stages::cmd_record(
                &parts,
                Agent::parse(&agent)?,
                kind,
                scope.as_deref(),
                skipped.as_deref(),
                only.as_deref(),
                &names,
            )?;
            if kind == What::Mcp {
                let checkout = stages::Checkout::read(&root);
                stages::sync_browser_bring_in(&parts, &profile, checkout)
                    .context("the entry is recorded; the browser server's bring-in is not")?;
            }
            Ok(rc)
        }
        Cmd::Memory(m) => match m {
            MemoryCmd::SetStore { path } => {
                println!("{}", memory::set_store(&memory_store_file()?, &path)?);
                Ok(0)
            }
            MemoryCmd::Status => stages::cmd_memory_status(
                &claude_config()?,
                &memory_store_file()?,
                &profile_dir()?,
                &parts_dir()?,
            ),
            MemoryCmd::Harvest { to, project } => {
                stages::cmd_harvest(&claude_config()?, &codex_home()?, &to, project.as_deref())
            }
        },
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
