//! transform — mAId's build crate. Validates source markdown and
//! manages the $HOME-facing symlinks that each AI tool reads from.
//!
//! Invoked via `cargo xtask <verb>` (alias in `.cargo/config.toml`).

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod deploy;
mod registry;
mod schema;
mod sh;
mod sources;

use deploy::{
    deploy as do_deploy, undeploy as do_undeploy, DeployOptions, DeployResult, DeployStatus,
    ExistingKind, UndeployResult, UndeployStatus,
};
use registry::REGISTRY;

#[derive(Parser)]
#[command(
    name = "transform",
    about = "mAId build crate — validate, deploy, install."
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Walk sources/ and validate frontmatter.
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
    /// Build any sources/<name>/ Rust crates into dist/, then deploy.
    Install,
    /// Inverse of install — undeploy.
    Uninstall,
    /// Run tests/functional/run --no-tools (structural smoke).
    TestSmoke,
    /// Run tests/functional/run (tool-driven functional smoke).
    TestFunctional,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let rc = match run(cli) {
        Ok(rc) => rc,
        Err(e) => {
            eprintln!("transform: {e:#}");
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
        Cmd::Install => cmd_install(),
        Cmd::Uninstall => cmd_undeploy(false, false),
        Cmd::TestSmoke => cmd_test(true),
        Cmd::TestFunctional => cmd_test(false),
    }
}

fn repo_root() -> Result<PathBuf> {
    // CARGO_MANIFEST_DIR points at <checkout>/transform/. Walk up one.
    let manifest = std::env::var("CARGO_MANIFEST_DIR")
        .context("CARGO_MANIFEST_DIR not set — invoke via `cargo xtask <verb>`")?;
    Ok(PathBuf::from(manifest)
        .parent()
        .context("expected transform/ to have a parent")?
        .to_path_buf())
}

fn home_dir() -> Result<PathBuf> {
    Ok(PathBuf::from(
        std::env::var("HOME").context("HOME is not set")?,
    ))
}

fn cmd_validate() -> Result<u8> {
    let root = repo_root()?;
    let sources_dir = root.join("sources");
    match sources::walk(&sources_dir) {
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

/// Discover Rust workspace members under `sources/<name>/Cargo.toml`,
/// build each in release, copy `target/release/<name>` to `dist/<name>`.
fn cmd_install() -> Result<u8> {
    // Validate + deploy first; if either fails, surface and stop.
    let drc = cmd_deploy(false, false)?;
    if drc != 0 {
        return Ok(drc);
    }

    let root = repo_root()?;
    let members = discover_members(&root.join("sources"))?;
    if members.is_empty() {
        return Ok(0);
    }

    std::fs::create_dir_all(root.join("dist"))?;
    for name in &members {
        eprintln!("building {name}...");
        crate::sh!(&format!("cargo build -p {name} --release"))?
            .dir(&root)
            .run()?;
        let from = root.join("target/release").join(name);
        let to = root.join("dist").join(name);
        if from.exists() {
            std::fs::copy(&from, &to)
                .with_context(|| format!("copy {} → {}", from.display(), to.display()))?;
            println!("installed {}", to.display());
        }
    }
    Ok(0)
}

fn cmd_test(no_tools: bool) -> Result<u8> {
    let root = repo_root()?;
    let runner = root.join("tests/functional/run");
    let cmd = if no_tools {
        format!("{} --no-tools", runner.display())
    } else {
        runner.display().to_string()
    };
    let status = crate::sh!(&cmd)?.dir(&root).unchecked().run()?;
    Ok(if status.status.success() { 0 } else { 1 })
}

fn discover_members(sources_dir: &Path) -> Result<Vec<String>> {
    let mut out = Vec::new();
    if !sources_dir.exists() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(sources_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        if entry.path().join("Cargo.toml").exists() {
            out.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    out.sort();
    Ok(out)
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
                "{tag}skip          {target} (existing {}; not managed by maid; use --force to remove)",
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
