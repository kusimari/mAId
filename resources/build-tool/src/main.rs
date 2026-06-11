//! build-tool — mAId's build crate. Validates source markdown and
//! manages the $HOME-facing symlinks that each AI tool reads from.
//!
//! Invoked via the project's Justfile: `just deploy`, `just status`,
//! `just undeploy`, `just validate`. Each recipe expands to
//! `cargo run -p build-tool --release --quiet -- <verb>`.

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

mod deploy;
mod registry;
mod schema;
mod sources;

use deploy::{
    deploy as do_deploy, undeploy as do_undeploy, DeployOptions, DeployResult, DeployStatus,
    ExistingKind, UndeployResult, UndeployStatus,
};
use registry::REGISTRY;

#[derive(Parser)]
#[command(
    name = "build-tool",
    about = "mAId build crate — validate, deploy, undeploy, status."
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Walk resources/content/ and validate frontmatter.
    Validate,
    /// Validate, then create/refresh $HOME-facing symlinks.
    Deploy {
        /// Plan without making changes.
        #[arg(long)]
        dry_run: bool,
        /// Replace symlinks that point elsewhere.
        #[arg(long)]
        force: bool,
    },
    /// Remove deploy-managed symlinks.
    Undeploy {
        /// Plan without making changes.
        #[arg(long)]
        dry_run: bool,
        /// Remove whatever is at the managed path, including foreign
        /// symlinks and non-symlinks.
        #[arg(long)]
        force: bool,
    },
    /// Report each managed symlink's state.
    Status,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let rc = match run(cli) {
        Ok(rc) => rc,
        Err(e) => {
            eprintln!("build-tool: {e:#}");
            1
        }
    };
    ExitCode::from(rc)
}

fn run(cli: Cli) -> Result<u8> {
    match cli.cmd {
        Cmd::Validate => cmd_validate(),
        Cmd::Deploy { dry_run, force } => cmd_deploy(dry_run, force),
        Cmd::Undeploy { dry_run, force } => cmd_undeploy(dry_run, force),
        Cmd::Status => cmd_status(),
    }
}

fn repo_root() -> Result<PathBuf> {
    // CARGO_MANIFEST_DIR points at <checkout>/resources/build-tool/.
    // Walk up two levels to reach the workspace root.
    let manifest = std::env::var("CARGO_MANIFEST_DIR")
        .context("CARGO_MANIFEST_DIR not set — invoke via `cargo run -p build-tool ...`")?;
    let manifest_path = PathBuf::from(manifest);
    let resources_dir = manifest_path
        .parent()
        .context("expected resources/build-tool/ to have a parent (resources/)")?;
    let root = resources_dir
        .parent()
        .context("expected resources/ to have a parent (workspace root)")?;
    Ok(root.to_path_buf())
}

fn home_dir() -> Result<PathBuf> {
    let raw = std::env::var("HOME").context("HOME is not set")?;
    if raw.is_empty() {
        return Err(anyhow!("HOME is empty"));
    }
    let home = PathBuf::from(raw);
    if !home.is_absolute() {
        return Err(anyhow!(
            "HOME must be an absolute path (got {})",
            home.display()
        ));
    }
    Ok(home)
}

fn cmd_validate() -> Result<u8> {
    let root = repo_root()?;
    let content_dir = root.join("resources").join("content");
    match sources::walk(&content_dir) {
        Ok(records) => {
            println!("validated {} source file(s)", records.len());
            Ok(0)
        }
        Err(msg) => {
            eprintln!("{msg}");
            Ok(1)
        }
    }
}

fn cmd_deploy(dry_run: bool, force: bool) -> Result<u8> {
    // Validate first so we don't deploy a broken tree.
    let vrc = cmd_validate()?;
    if vrc != 0 {
        return Ok(vrc);
    }

    let home = home_dir()?;
    let root = repo_root()?;
    let opts = DeployOptions {
        home: &home,
        checkout: &root,
        dry_run,
        force,
    };
    let results = do_deploy(&opts)?;
    let mut failures = 0u8;
    for r in &results {
        print_deploy(r, dry_run, &mut failures);
    }
    Ok(failures.min(1))
}

fn cmd_undeploy(dry_run: bool, force: bool) -> Result<u8> {
    let home = home_dir()?;
    let root = repo_root()?;
    let opts = DeployOptions {
        home: &home,
        checkout: &root,
        dry_run,
        force,
    };
    let results = do_undeploy(&opts)?;
    let mut failures = 0u8;
    for r in &results {
        print_undeploy(r, dry_run, &mut failures);
    }
    Ok(failures.min(1))
}

fn cmd_status() -> Result<u8> {
    let home = home_dir()?;
    let root = repo_root()?;
    for entry in REGISTRY {
        let target = home.join(entry.home_subpath);
        let expected = root.join(entry.source_subpath);
        let state = match std::fs::symlink_metadata(&target) {
            Ok(meta) if meta.file_type().is_symlink() => match std::fs::read_link(&target) {
                Ok(cur) if cur == expected => format!("ok -> {}", cur.display()),
                Ok(cur) => format!(
                    "WRONG -> {} (expected {})",
                    cur.display(),
                    expected.display()
                ),
                Err(e) => format!("symlink read error: {e}"),
            },
            Ok(meta) => {
                let kind = if meta.is_dir() { "dir" } else { "file" };
                format!("non-symlink ({kind})")
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => "missing".into(),
            Err(e) => format!("lstat error: {e}"),
        };
        println!("{:<28} {}", entry.home_subpath, state);
    }
    Ok(0)
}

fn print_deploy(r: &DeployResult, dry_run: bool, failures: &mut u8) {
    let tag = if dry_run { "(dry-run) " } else { "" };
    let target = r.target.display();
    match &r.status {
        DeployStatus::Created => println!("{tag}created   {target}"),
        DeployStatus::AlreadyOk => println!("{tag}ok        {target}"),
        DeployStatus::Replaced => println!("{tag}replaced  {target}"),
        DeployStatus::SkippedMissingSource => {
            println!("{tag}skip      {target} (source missing)")
        }
        DeployStatus::SkippedNonSymlink { existing } => {
            eprintln!(
                "{tag}skip      {target} (existing {}; not overwriting)",
                kind_str(*existing)
            );
            *failures = failures.saturating_add(1);
        }
        DeployStatus::SkippedWrongSymlink { current_target } => {
            eprintln!(
                "{tag}skip      {target} (points elsewhere: {}; use --force to replace)",
                current_target.display()
            );
            *failures = failures.saturating_add(1);
        }
    }
}

fn print_undeploy(r: &UndeployResult, dry_run: bool, failures: &mut u8) {
    let tag = if dry_run { "(dry-run) " } else { "" };
    let target = r.target.display();
    match &r.status {
        UndeployStatus::NotDeployed => println!("{tag}not-deployed  {target}"),
        UndeployStatus::Removed { .. } => println!("{tag}removed       {target}"),
        UndeployStatus::ForceRemoved { existing } => {
            println!("{tag}force-removed {target} (was {})", kind_str(*existing))
        }
        UndeployStatus::SkippedForeignSymlink { current_target } => {
            eprintln!(
                "{tag}skip          {target} (foreign symlink -> {}; use --force to remove)",
                current_target.display()
            );
            *failures = failures.saturating_add(1);
        }
        UndeployStatus::SkippedNonSymlink { existing } => {
            eprintln!(
                "{tag}skip          {target} (existing {}; not managed; use --force to remove)",
                kind_str(*existing)
            );
            *failures = failures.saturating_add(1);
        }
    }
}

fn kind_str(k: ExistingKind) -> &'static str {
    match k {
        ExistingKind::File => "file",
        ExistingKind::Dir => "dir",
        ExistingKind::Symlink => "symlink",
    }
}
