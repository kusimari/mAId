//! ghost-claude — the ghost in Claude Code, at a command line: the five
//! entry points for a hook, the plugin or a person at a shell, and the
//! ghost's own install (validate, install, uninstall, on, off, status,
//! rollback). The CLI is this shell's own surface, so the enums clap reads
//! live here and map to infusion's own; what each entry point answers is
//! `infusion`, and each install verb is its own module.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use ghost::config::Config;
use ghost::environment::{self, Host};
use ghost::Ghost;
use infusion::adaptation::ledger;
use infusion::call::{Call, Carrier, Infusion, Item, Slice};
use plain::claude_config::ConfigDir;
use serde_json::{json, Value};
use shell_claude::install::Setup;
use shell_claude::installed::Claude;
use shell_claude::transcript::Transcripts;
use shell_claude::{hook, mcp};
use std::io::Read;
use std::process::ExitCode;

/// How a carrier keeps what it is given, as the CLI names it. The help
/// each value gets is the person's, so it is written here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum CarrierArg {
    /// Kept in the session's history: send only what changed.
    Append,
    /// Sent with one call only: send everything every time.
    Transient,
    /// Asked for: return everything, and record it as sent.
    OnDemand,
}

impl From<CarrierArg> for Carrier {
    fn from(a: CarrierArg) -> Carrier {
        match a {
            CarrierArg::Append => Carrier::Append,
            CarrierArg::Transient => Carrier::Transient,
            CarrierArg::OnDemand => Carrier::OnDemand,
        }
    }
}

/// How much of the genome a session is given, as the CLI names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
enum SliceArg {
    /// All of it.
    #[default]
    Whole,
    /// A sub-agent's: who and what, without what waits for the person
    /// (the environment's detail and pending decisions).
    SubSession,
}

impl From<SliceArg> for Slice {
    fn from(a: SliceArg) -> Slice {
        match a {
            SliceArg::Whole => Slice::Whole,
            SliceArg::SubSession => Slice::SubSession,
        }
    }
}

#[derive(Parser)]
#[command(
    name = "ghost-claude",
    about = "The core of my harness: the five entry points, the ghost and the environment."
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

/// The call shape every entry point takes.
#[derive(clap::Args)]
struct CallArgs {
    /// The session (or sub-session) this call is for.
    #[arg(long)]
    session: Option<String>,
    /// How the carrier keeps what it is given.
    #[arg(long, value_enum, default_value_t = CarrierArg::OnDemand)]
    carrier: CarrierArg,
    /// Forget what this session was sent first (after a compaction, a
    /// clear or a resume).
    #[arg(long)]
    reset: bool,
    /// The carrier's own input as JSON; `-` reads it from stdin.
    #[arg(long)]
    input: Option<String>,
    /// How much of the genome to give: a sub-agent takes `sub-session`.
    #[arg(long, value_enum, default_value_t = SliceArg::Whole)]
    slice: SliceArg,
    /// Print the items as JSON instead of their text.
    #[arg(long, conflicts_with = "claude_event")]
    json: bool,
    /// Answer as a Claude Code hook for this event: its JSON output with
    /// the items as `additionalContext`, and always exit 0, since exit 2
    /// blocks the prompt and any other failure shows the person an error.
    #[arg(long, value_name = "EVENT")]
    claude_event: Option<String>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Who the session is: personas, bring-ins, environment, pending.
    Genome(CallArgs),
    /// What this turn needs: the acting persona, the work's record, the
    /// rules of the bring-ins that fit, and the memories that may bear on it.
    Expression(CallArgs),
    /// What happened in a session, packed; with none named, the sessions
    /// waiting to be reflected on.
    Experience(CallArgs),
    /// Mark a session for sleep: its end, or a close or reflect.
    Sleep(CallArgs),
    /// Turn experience into changes: the reflector's brief, then the
    /// decision batch, then what the person's decisions applied.
    Adaptation(CallArgs),
    /// What reflections saw and the person decided, counted by thing.
    Ledger {
        #[arg(long)]
        json: bool,
    },
    /// The ghost, every part fused, as JSON.
    Ghost,
    /// Each bring-in's readiness where this runs.
    Environment {
        #[arg(long)]
        json: bool,
    },
    /// Serve the on-demand carrier as an MCP server on stdio: the genome
    /// as a resource and a prompt, expression as a tool.
    Mcp {
        /// The environment variable that holds the shell's session id.
        #[arg(long, value_name = "NAME")]
        session_env: Option<String>,
    },
    /// Check every `my-harness` declaration in the checkout's resources
    /// and the ghost's plugin.
    Validate(Checkout),
    /// Install the ghost: its profile, plain's install when Claude Code
    /// does not have this checkout's, its plugin, its record. With
    /// `--prefix`: only the package, into that dir (a new or empty dir, or
    /// an earlier install of it, which is replaced whole).
    Install {
        #[command(flatten)]
        checkout: Checkout,
        #[arg(long)]
        prefix: Option<std::path::PathBuf>,
        /// With `--prefix`: the executable to put in it (default this one).
        #[arg(long, requires = "prefix")]
        exe: Option<std::path::PathBuf>,
    },
    /// Remove the ghost's plugin, profile and record; plain stays.
    Uninstall(Checkout),
    /// Switch the ghost on at a scope (user, project or local).
    On {
        #[arg(default_value = "user")]
        scope: String,
    },
    /// Switch the ghost off at a scope (user, project or local).
    Off {
        #[arg(default_value = "user")]
        scope: String,
    },
    /// The ghost's install and what it sees, from here.
    Status(Checkout),
    /// The ghost's profile one generation back; plain stays.
    Rollback(Checkout),
}

/// The checkout an install verb works from.
#[derive(clap::Args)]
struct Checkout {
    /// The mAId checkout (default: the working dir).
    #[arg(long, default_value = ".")]
    root: std::path::PathBuf,
}

impl Checkout {
    fn setup(&self) -> Result<Setup> {
        Setup::from_env(&std::path::absolute(&self.root)?)
    }
}

fn main() -> ExitCode {
    let hook = std::env::args().any(|a| a == "--claude-event" || a.starts_with("--claude-event="));
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) if hook => {
            eprintln!("ghost-claude: {e}");
            return ExitCode::SUCCESS;
        }
        Err(e) => e.exit(),
    };
    match std::panic::catch_unwind(|| run(cli)) {
        Ok(Ok(())) => ExitCode::SUCCESS,
        Ok(Err(e)) => {
            eprintln!("ghost-claude: {e:#}");
            fail(hook)
        }
        // The panic message is already on stderr.
        Err(_) => fail(hook),
    }
}

fn fail(hook: bool) -> ExitCode {
    match hook {
        true => ExitCode::SUCCESS,
        false => ExitCode::FAILURE,
    }
}

fn run(cli: Cli) -> Result<()> {
    if let Some(rc) = install_verb(&cli.cmd)? {
        return match rc {
            0 => Ok(()),
            n => Err(anyhow::anyhow!(
                "finished, with {n} step(s) left alone; see above"
            )),
        };
    }
    let config = Config::from_env()?;
    // Where this shell keeps its own things, read the same way for every
    // command, so a config dir it cannot take fails them all alike.
    let dir = ConfigDir::from_env()?;
    let installed = Claude {
        config: dir.clone(),
        cache: Some(config.shells().join("claude")),
        vars: None,
    };
    let ctx = Infusion {
        config,
        host: Host::from_env(),
        sources: Box::new(Transcripts {
            projects: dir.map(|d| d.projects()),
        }),
        shell: Box::new(installed),
    };
    type Entry = fn(&Infusion, &Call) -> Result<Vec<Item>>;
    let (entry, a): (Entry, &CallArgs) = match &cli.cmd {
        Cmd::Genome(a) => (infusion::genome, a),
        Cmd::Expression(a) => (infusion::expression, a),
        Cmd::Experience(a) => (infusion::experience, a),
        Cmd::Sleep(a) => (infusion::sleep, a),
        Cmd::Adaptation(a) => (infusion::adaptation, a),
        Cmd::Mcp { session_env } => {
            let session = mcp::Session {
                id: session_env
                    .as_deref()
                    .and_then(|k| std::env::var(k).ok())
                    .filter(|v| !v.is_empty()),
                cwd: std::env::current_dir().ok(),
            };
            let stdin = std::io::stdin().lock();
            return Ok(mcp::serve(&ctx, &session, stdin, std::io::stdout().lock())?);
        }
        Cmd::Ledger { json } => {
            let records = ledger::read(&ctx.config)?;
            let rows = ledger::summary(&records);
            let dismissed = ledger::dismissed_count(&records);
            match json {
                true => println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &serde_json::json!({"things": rows, "dismissed": dismissed})
                    )?
                ),
                false => print!("{}", ledger::render(&rows, dismissed)),
            }
            return Ok(());
        }
        Cmd::Ghost => {
            let g = fuse(&ctx)?;
            println!("{}", serde_json::to_string_pretty(&g)?);
            return Ok(());
        }
        Cmd::Validate(_)
        | Cmd::Install { .. }
        | Cmd::Uninstall(_)
        | Cmd::On { .. }
        | Cmd::Off { .. }
        | Cmd::Status(_)
        | Cmd::Rollback(_) => unreachable!("an install verb is run before the entry points"),
        Cmd::Environment { json } => {
            let ghost = fuse(&ctx)?;
            let rows = environment::derive(&ghost, &ctx.host);
            match json {
                true => println!("{}", serde_json::to_string_pretty(&rows)?),
                false => {
                    for r in rows {
                        let why = r
                            .readiness
                            .why()
                            .map(|w| format!(" ({w})"))
                            .unwrap_or_default();
                        println!("{:<11} {} ({}){why}", r.readiness.label(), r.name, r.part);
                    }
                    for n in &ghost.not_understood {
                        println!("not understood {n}");
                    }
                }
            }
            return Ok(());
        }
    };
    let items = entry(&ctx, &call(a)?)?;
    match (&a.claude_event, a.json) {
        (Some(_), _) if items.is_empty() => {}
        (Some(event), _) => {
            let context: String = items.iter().map(|i| i.text.as_str()).collect();
            let out = json!({"hookSpecificOutput": {
                "hookEventName": event, "additionalContext": context}});
            println!("{out}");
        }
        (None, true) => println!("{}", json!({ "items": items })),
        (None, false) => items.iter().for_each(|i| print!("{}", i.text)),
    }
    Ok(())
}

/// The ghost's own install verbs, which need no infusion.
fn install_verb(cmd: &Cmd) -> Result<Option<u8>> {
    use shell_claude::{install, rollback, status, switch, uninstall};
    // Only the verbs that read a project ask where they run: a hook must
    // still answer when its working dir is gone.
    let here = || std::env::current_dir().context("cannot read the working dir");
    Ok(Some(match cmd {
        Cmd::Validate(c) => install::validate(&c.setup()?).map(|()| 0)?,
        Cmd::Install {
            checkout,
            prefix: Some(prefix),
            exe,
        } => {
            let s = checkout.setup()?;
            // Its own plugin only: nix runs this on the plugin alone.
            install::validate_plugin(&s)?;
            let exe = exe.clone().unwrap_or(s.exe.clone());
            let v = shell_claude::package::install(&s.plugin(), &exe, prefix)?;
            println!("installed ghost {v} into {}", prefix.display());
            0
        }
        Cmd::Install { checkout, .. } => install::install(&checkout.setup()?)?,
        Cmd::Uninstall(c) => uninstall::uninstall(&c.setup()?)?,
        Cmd::On { scope } | Cmd::Off { scope } => {
            let config = ConfigDir::from_env()?.context("HOME is not set")?;
            let cli = shell_claude::plugins::Cli::from_env(config);
            switch::switch(&cli, matches!(cmd, Cmd::On { .. }), scope, &here()?)?;
            0
        }
        Cmd::Status(c) => status::status(&c.setup()?, &here()?)?,
        Cmd::Rollback(c) => rollback::rollback(&c.setup()?)?,
        _ => return Ok(None),
    }))
}

fn fuse(ctx: &Infusion) -> Result<Ghost> {
    ctx.shell.ghost(std::env::current_dir().ok().as_deref())
}

fn call(a: &CallArgs) -> Result<Call> {
    let json = match a.input.as_deref() {
        None => Value::Null,
        Some("-") => {
            let mut text = String::new();
            std::io::stdin().read_to_string(&mut text)?;
            serde_json::from_str(&text).context("--input - is not JSON")?
        }
        Some(text) => serde_json::from_str(text).context("--input is not JSON")?,
    };
    let read = hook::read(&json)?;
    Ok(Call {
        session: a.session.clone().or(read.session),
        carrier: a.carrier.into(),
        reset: a.reset,
        slice: a.slice.into(),
        at: read.at,
        prompt: read.prompt,
        close: read.close,
        input: read.own,
    })
}

/// The CLI's enums mirror infusion's, so a value infusion gains and this
/// shell does not would be unreachable from a command line. These matches
/// are over infusion's own, so that does not compile.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_carrier_and_slice_infusion_has_can_be_asked_for() {
        let carrier = |c: Carrier| match c {
            Carrier::Append => CarrierArg::Append,
            Carrier::Transient => CarrierArg::Transient,
            Carrier::OnDemand => CarrierArg::OnDemand,
        };
        for c in [Carrier::Append, Carrier::Transient, Carrier::OnDemand] {
            assert_eq!(Carrier::from(carrier(c)), c);
        }
        let slice = |s: Slice| match s {
            Slice::Whole => SliceArg::Whole,
            Slice::SubSession => SliceArg::SubSession,
        };
        for s in [Slice::Whole, Slice::SubSession] {
            assert_eq!(Slice::from(slice(s)), s);
        }
    }
}
