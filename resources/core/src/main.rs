//! maid-core — the CLI over the core's entry points, for a hook, an
//! adapter or a person at a shell. Every call is answered the same way the
//! library answers it; see `lib.rs`.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use maid_core::config::Config;
use maid_core::entry::{self, Call, Carrier, Ctx, Item, Slice};
use maid_core::environment::{self, Host};
use maid_core::{map, mcp};
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
    /// How much of the genome to give: a sub-agent takes `sub-session`.
    #[arg(long, value_enum, default_value_t = Slice::Whole)]
    slice: Slice,
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
    /// What this turn needs (not filled yet: answers nothing).
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
    /// The harness map, every part fused, as JSON.
    Map,
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
}

fn main() -> ExitCode {
    let hook = std::env::args().any(|a| a == "--claude-event" || a.starts_with("--claude-event="));
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) if hook => {
            eprintln!("maid-core: {e}");
            return ExitCode::SUCCESS;
        }
        Err(e) => e.exit(),
    };
    match std::panic::catch_unwind(|| run(cli)) {
        Ok(Ok(())) => ExitCode::SUCCESS,
        Ok(Err(e)) => {
            eprintln!("maid-core: {e:#}");
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
            let records = maid_core::ledger::read(&ctx.config)?;
            let rows = maid_core::ledger::summary(&records);
            let dismissed = maid_core::ledger::dismissed_count(&records);
            match json {
                true => println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &serde_json::json!({"things": rows, "dismissed": dismissed})
                    )?
                ),
                false => print!("{}", maid_core::ledger::render(&rows, dismissed)),
            }
            return Ok(());
        }
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
        session: a
            .session
            .clone()
            .or_else(|| Call::session_from_input(&input)),
        carrier: a.carrier,
        reset: a.reset,
        input,
        slice: a.slice,
    })
}
