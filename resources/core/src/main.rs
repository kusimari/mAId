//! maid-core — the CLI over the core's entry points, for a hook, an
//! adapter or a person at a shell. Every call is answered the same way the
//! library answers it; see `lib.rs`.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use maid_core::config::Config;
use maid_core::entry::{self, Call, Carrier, Ctx, Item};
use maid_core::environment::{self, Host};
use maid_core::map;
use serde_json::{json, Value};
use std::io::Read;
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "maid-core",
    about = "The core of my harness: the five entry points, the map and the environment."
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
    #[arg(long, value_enum, default_value_t = Carrier::OnDemand)]
    carrier: Carrier,
    /// Forget what this session was sent first (after a compaction, a
    /// clear or a resume).
    #[arg(long)]
    reset: bool,
    /// The carrier's own input as JSON; `-` reads it from stdin.
    #[arg(long)]
    input: Option<String>,
    /// Print the items as JSON instead of their text.
    #[arg(long)]
    json: bool,
}

#[derive(Subcommand)]
enum Cmd {
    /// Who the session is: personas, bring-ins, environment, pending.
    Genome(CallArgs),
    /// What this turn needs (not filled yet: answers nothing).
    Expression(CallArgs),
    /// What happened in a session (not filled yet: answers nothing).
    Experience(CallArgs),
    /// Mark an outcome done (not filled yet: answers nothing).
    Sleep(CallArgs),
    /// Turn experience into changes (not filled yet: answers nothing).
    Adaptation(CallArgs),
    /// The harness map, every part fused, as JSON.
    Map,
    /// Each bring-in's readiness where this runs.
    Environment {
        #[arg(long)]
        json: bool,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("maid-core: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    let ctx = Ctx {
        config: Config::from_env()?,
        host: Host::from_env(),
    };
    type Entry = fn(&Ctx, &Call) -> Result<Vec<Item>>;
    let (entry, a): (Entry, &CallArgs) = match &cli.cmd {
        Cmd::Genome(a) => (entry::genome, a),
        Cmd::Expression(a) => (entry::expression, a),
        Cmd::Experience(a) => (entry::experience, a),
        Cmd::Sleep(a) => (entry::sleep, a),
        Cmd::Adaptation(a) => (entry::adaptation, a),
        Cmd::Map => {
            let m = fuse(&ctx)?;
            println!("{}", serde_json::to_string_pretty(&m)?);
            return Ok(());
        }
        Cmd::Environment { json } => {
            let rows = environment::derive(&fuse(&ctx)?, &ctx.host);
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
                }
            }
            return Ok(());
        }
    };
    let items = entry(&ctx, &call(a)?)?;
    match a.json {
        true => println!("{}", json!({ "items": items })),
        false => items.iter().for_each(|i| print!("{}", i.text)),
    }
    Ok(())
}

fn fuse(ctx: &Ctx) -> Result<map::Map> {
    map::fuse(&ctx.config.parts)
        .with_context(|| format!("cannot read {}", ctx.config.parts.display()))
}

fn call(a: &CallArgs) -> Result<Call> {
    let input = match a.input.as_deref() {
        None => Value::Null,
        Some("-") => {
            let mut text = String::new();
            std::io::stdin().read_to_string(&mut text)?;
            serde_json::from_str(&text).context("--input - is not JSON")?
        }
        Some(text) => serde_json::from_str(text).context("--input is not JSON")?,
    };
    Ok(Call {
        session: a.session.clone(),
        carrier: a.carrier,
        reset: a.reset,
        input,
    })
}
