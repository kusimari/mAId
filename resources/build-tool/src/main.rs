//! build-tool — mAId's tooling for installing checked-in markdown
//! resources into the AI tools that consume them. Single-file
//! Rust crate; the whole job is small enough that splitting it
//! into modules adds noise.
//!
//! Invoked via the project's Justfile:
//!   just install     create the $HOME-facing symlinks (after content checks)
//!   just uninstall   remove the managed symlinks
//!   just status      report each managed symlink's current state
//!
//! `just verify` is a separate verb that drives `claude --print`
//! against the installed content; that lives in the Justfile and
//! shells into `resources/tests/run`, not in this binary.

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

// ─────────────────────────────────────────────────────────────────
// 1. Registry — the deployment manifest.
//
// Six entries: four for the merged AGENTS.md preamble (legacy
// CLAUDE.md / KIRO.md alongside AGENTS.md, all pointing at the
// same source), two for the skills tree. Drop the legacy filenames
// when Claude Code adds AGENTS.md as a default-read location.
// ─────────────────────────────────────────────────────────────────

type Entry = (&'static str, &'static str); // (home_subpath, source_subpath)

const REGISTRY: &[Entry] = &[
    (".claude/CLAUDE.md", "resources/content/agents.md"),
    (".claude/AGENTS.md", "resources/content/agents.md"),
    (".claude/skills", "resources/content/skills"),
    (".kiro/steering/KIRO.md", "resources/content/agents.md"),
    (".kiro/steering/AGENTS.md", "resources/content/agents.md"),
    (".kiro/steering/skills", "resources/content/skills"),
];

// ─────────────────────────────────────────────────────────────────
// 2. Content checks.
//
// Two shapes the AI tools care about:
//   - resources/content/agents.md     — plain markdown preamble
//                                        (cross-tool AGENTS.md
//                                        standard, no frontmatter).
//                                        Check: present + non-empty.
//   - resources/content/skills/<name>/SKILL.md
//                                      — frontmatter required:
//                                        name + description.
// ─────────────────────────────────────────────────────────────────

/// Walk content under `content_dir`, collecting all problems.
/// Returns the number of files validated, or the joined error
/// messages. The intentional empty case (zero content) is fine —
/// `install` simply has nothing to check.
fn check_content(content_dir: &Path) -> Result<usize, String> {
    let mut errors: Vec<String> = Vec::new();
    let mut count = 0usize;

    // AGENTS.md preamble — presence + non-empty.
    let preamble = content_dir.join("agents.md");
    if preamble.exists() {
        match fs::read_to_string(&preamble) {
            Ok(c) if c.trim().is_empty() => {
                errors.push(format!(
                    "{}: AGENTS.md preamble is empty",
                    preamble.display()
                ));
            }
            Ok(_) => count += 1,
            Err(e) => {
                errors.push(format!("{}: cannot read: {e}", preamble.display()));
            }
        }
    }

    // Skills — each subdirectory's SKILL.md must validate.
    let skills_dir = content_dir.join("skills");
    if let Ok(entries) = fs::read_dir(&skills_dir) {
        for entry in entries.flatten() {
            let Ok(ft) = entry.file_type() else { continue };
            if !ft.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let skill_md = entry.path().join("SKILL.md");
            if !skill_md.exists() {
                continue;
            }
            match fs::read_to_string(&skill_md) {
                Ok(content) => match check_skill_frontmatter(&content) {
                    Ok(()) => count += 1,
                    Err(msg) => errors.push(format!("{}: {msg}", skill_md.display())),
                },
                Err(e) => errors.push(format!("{}: cannot read: {e}", skill_md.display())),
            }
        }
    }

    if !errors.is_empty() {
        return Err(format!("Content validation failed:\n{}", errors.join("\n")));
    }
    Ok(count)
}

/// Validate SKILL.md-style frontmatter. Four checks:
///   1. File begins with `---\n` (or `---\r\n`).
///   2. A closing `---` line exists.
///   3. `name:` is present and (after balanced unquoting) non-empty.
///   4. `description:` is present and (after balanced unquoting) non-empty.
fn check_skill_frontmatter(content: &str) -> Result<(), String> {
    if !content.starts_with("---\n") && !content.starts_with("---\r\n") {
        return Err("missing YAML frontmatter (file must start with '---')".into());
    }

    let lines: Vec<&str> = content.split('\n').collect();
    let Some(end) = lines
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, l)| **l == "---" || **l == "---\r")
        .map(|(i, _)| i)
    else {
        return Err("unterminated YAML frontmatter (no closing '---')".into());
    };

    let mut have_name = false;
    let mut have_desc = false;
    for line in &lines[1..end] {
        if let Some(rest) = line.strip_prefix("name:") {
            if !unquote(rest.trim()).is_empty() {
                have_name = true;
            }
        } else if let Some(rest) = line.strip_prefix("description:") {
            if !unquote(rest.trim()).is_empty() {
                have_desc = true;
            }
        }
    }
    if !have_name {
        return Err("missing required field: name".into());
    }
    if !have_desc {
        return Err("missing required field: description".into());
    }
    Ok(())
}

/// Strip a single layer of balanced `"..."` or `'...'` from `s`.
fn unquote(s: &str) -> &str {
    if s.len() >= 2 {
        let b = s.as_bytes();
        if (b[0] == b'"' && b[s.len() - 1] == b'"') || (b[0] == b'\'' && b[s.len() - 1] == b'\'') {
            return &s[1..s.len() - 1];
        }
    }
    s
}

// ─────────────────────────────────────────────────────────────────
// 3. Compare — the shared core for install / uninstall / status.
//
// Each verb walks REGISTRY and asks `compare()` what it sees at the
// home path. The verb decides the action.
// ─────────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Eq)]
enum Comparison {
    /// The symlink exists and points where REGISTRY says it should.
    Match,
    /// A symlink exists but points somewhere else.
    WrongTarget(PathBuf),
    /// Nothing at the home path.
    Missing,
    /// A real (non-symlink) file is at the home path.
    BlockedByRealFile,
    /// A real (non-symlink) directory is at the home path.
    BlockedByRealDir,
    /// REGISTRY's source path doesn't exist in the checkout.
    SourceMissing,
}

struct Plan {
    home: PathBuf,
    source: PathBuf,
    cmp: Comparison,
}

fn plan_one(entry: Entry, home_root: &Path, checkout: &Path) -> io::Result<Plan> {
    let home = home_root.join(entry.0);
    let source = checkout.join(entry.1);

    if !path_exists(&source) {
        return Ok(Plan {
            home,
            source,
            cmp: Comparison::SourceMissing,
        });
    }

    let cmp = match fs::symlink_metadata(&home) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Comparison::Missing,
        Err(e) => return Err(e),
        Ok(meta) if meta.file_type().is_symlink() => {
            let current = fs::read_link(&home)?;
            if current == source {
                Comparison::Match
            } else {
                Comparison::WrongTarget(current)
            }
        }
        Ok(meta) if meta.is_dir() => Comparison::BlockedByRealDir,
        Ok(_) => Comparison::BlockedByRealFile,
    };
    Ok(Plan { home, source, cmp })
}

fn path_exists(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok()
}

fn ensure_parent(p: &Path) -> io::Result<()> {
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────
// 4. CLI dispatch.
// ─────────────────────────────────────────────────────────────────

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
    },
    /// Remove deploy-managed symlinks.
    Uninstall {
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
    match run(cli) {
        Ok(rc) => ExitCode::from(rc),
        Err(e) => {
            eprintln!("build-tool: {e:#}");
            ExitCode::from(1)
        }
    }
}

fn run(cli: Cli) -> Result<u8> {
    let root = repo_root()?;
    let home = home_dir()?;
    match cli.cmd {
        Cmd::Install { dry_run, force } => cmd_install(&home, &root, dry_run, force),
        Cmd::Uninstall { dry_run, force } => cmd_uninstall(&home, &root, dry_run, force),
        Cmd::Status => cmd_status(&home, &root),
    }
}

fn cmd_install(home: &Path, checkout: &Path, dry_run: bool, force: bool) -> Result<u8> {
    // Validate content first; refuse to install a broken tree.
    let count =
        check_content(&checkout.join("resources").join("content")).map_err(|msg| anyhow!(msg))?;
    eprintln!("validated {count} content file(s)");

    let mut failures = 0u8;
    for entry in REGISTRY {
        let plan = plan_one(*entry, home, checkout)?;
        let tag = if dry_run { "(dry-run) " } else { "" };
        let target = plan.home.display();
        match plan.cmp {
            Comparison::Match => println!("{tag}ok        {target}"),
            Comparison::Missing => {
                if !dry_run {
                    ensure_parent(&plan.home)?;
                    std::os::unix::fs::symlink(&plan.source, &plan.home)?;
                }
                println!("{tag}created   {target}");
            }
            Comparison::WrongTarget(current) if force => {
                if !dry_run {
                    fs::remove_file(&plan.home)?;
                    std::os::unix::fs::symlink(&plan.source, &plan.home)?;
                }
                println!("{tag}replaced  {target} (was {})", current.display());
            }
            Comparison::WrongTarget(current) => {
                eprintln!(
                    "{tag}skip      {target} (points elsewhere: {}; use --force to replace)",
                    current.display()
                );
                failures = failures.saturating_add(1);
            }
            Comparison::BlockedByRealFile => {
                eprintln!("{tag}skip      {target} (existing file; not overwriting)");
                failures = failures.saturating_add(1);
            }
            Comparison::BlockedByRealDir => {
                eprintln!("{tag}skip      {target} (existing dir; not overwriting)");
                failures = failures.saturating_add(1);
            }
            Comparison::SourceMissing => {
                println!("{tag}skip      {target} (source missing)");
            }
        }
    }
    Ok(if failures > 0 { 1 } else { 0 })
}

fn cmd_uninstall(home: &Path, checkout: &Path, dry_run: bool, force: bool) -> Result<u8> {
    let mut failures = 0u8;
    for entry in REGISTRY {
        let plan = plan_one(*entry, home, checkout)?;
        let tag = if dry_run { "(dry-run) " } else { "" };
        let target = plan.home.display();
        match plan.cmp {
            Comparison::Match => {
                if !dry_run {
                    fs::remove_file(&plan.home)?;
                }
                println!("{tag}removed       {target}");
            }
            Comparison::Missing | Comparison::SourceMissing => {
                println!("{tag}not-installed {target}");
            }
            Comparison::WrongTarget(current) if force => {
                if !dry_run {
                    fs::remove_file(&plan.home)?;
                }
                println!(
                    "{tag}force-removed {target} (was symlink -> {})",
                    current.display()
                );
            }
            Comparison::WrongTarget(current) => {
                eprintln!(
                    "{tag}skip          {target} (foreign symlink -> {}; use --force to remove)",
                    current.display()
                );
                failures = failures.saturating_add(1);
            }
            Comparison::BlockedByRealFile if force => {
                if !dry_run {
                    fs::remove_file(&plan.home)?;
                }
                println!("{tag}force-removed {target} (was file)");
            }
            Comparison::BlockedByRealDir if force => {
                if !dry_run {
                    fs::remove_dir_all(&plan.home)?;
                }
                println!("{tag}force-removed {target} (was dir)");
            }
            Comparison::BlockedByRealFile => {
                eprintln!(
                    "{tag}skip          {target} (existing file; not managed; use --force to remove)"
                );
                failures = failures.saturating_add(1);
            }
            Comparison::BlockedByRealDir => {
                eprintln!(
                    "{tag}skip          {target} (existing dir; not managed; use --force to remove)"
                );
                failures = failures.saturating_add(1);
            }
        }
    }
    Ok(if failures > 0 { 1 } else { 0 })
}

fn cmd_status(home: &Path, checkout: &Path) -> Result<u8> {
    for entry in REGISTRY {
        let plan = plan_one(*entry, home, checkout)?;
        let state = match plan.cmp {
            Comparison::Match => format!("ok -> {}", plan.source.display()),
            Comparison::Missing => "missing".into(),
            Comparison::SourceMissing => "source missing".into(),
            Comparison::WrongTarget(cur) => format!(
                "WRONG -> {} (expected {})",
                cur.display(),
                plan.source.display()
            ),
            Comparison::BlockedByRealFile => "non-symlink (file)".into(),
            Comparison::BlockedByRealDir => "non-symlink (dir)".into(),
        };
        println!("{:<28} {}", entry.0, state);
    }
    Ok(0)
}

// ─────────────────────────────────────────────────────────────────
// 5. Roots.
// ─────────────────────────────────────────────────────────────────

fn repo_root() -> Result<PathBuf> {
    // CARGO_MANIFEST_DIR points at <checkout>/resources/build-tool/.
    // Walk up two levels to the workspace root, then sentinel-check
    // that we landed at a recognizable workspace.
    let manifest = std::env::var("CARGO_MANIFEST_DIR")
        .context("CARGO_MANIFEST_DIR not set — invoke via `cargo run -p build-tool ...`")?;
    let root = PathBuf::from(manifest)
        .parent()
        .context("expected resources/build-tool/ to have a parent (resources/)")?
        .parent()
        .context("expected resources/ to have a parent (workspace root)")?
        .to_path_buf();
    if !root.join("Cargo.toml").is_file() || !root.join("resources").is_dir() {
        return Err(anyhow!(
            "expected workspace root at {} (Cargo.toml + resources/ both required)",
            root.display()
        ));
    }
    Ok(root)
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

// ─────────────────────────────────────────────────────────────────
// 6. Tests.
// ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ── content checks ──────────────────────────────────────────

    fn write(p: &Path, s: &str) {
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, s).unwrap();
    }

    #[test]
    fn check_content_empty_root_ok() {
        let dir = TempDir::new().unwrap();
        let n = check_content(dir.path()).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn check_content_agents_md_present_ok() {
        let dir = TempDir::new().unwrap();
        write(&dir.path().join("agents.md"), "# preamble\n\nbody.\n");
        let n = check_content(dir.path()).unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn check_content_agents_md_empty_rejected() {
        let dir = TempDir::new().unwrap();
        write(&dir.path().join("agents.md"), "");
        let e = check_content(dir.path()).unwrap_err();
        assert!(e.contains("AGENTS.md preamble is empty"));
    }

    #[test]
    fn check_content_agents_md_whitespace_only_rejected() {
        let dir = TempDir::new().unwrap();
        write(&dir.path().join("agents.md"), "  \n\n  \n");
        let e = check_content(dir.path()).unwrap_err();
        assert!(e.contains("AGENTS.md preamble is empty"));
    }

    #[test]
    fn check_content_skill_with_frontmatter_ok() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("skills/foo/SKILL.md"),
            "---\nname: foo\ndescription: bar\n---\nbody.\n",
        );
        let n = check_content(dir.path()).unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn check_content_skill_missing_description_rejected() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("skills/foo/SKILL.md"),
            "---\nname: foo\n---\n",
        );
        let e = check_content(dir.path()).unwrap_err();
        assert!(e.contains("description"));
    }

    #[test]
    fn check_content_skill_quoted_empty_rejected() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("skills/foo/SKILL.md"),
            "---\nname: \"\"\ndescription: bar\n---\n",
        );
        let e = check_content(dir.path()).unwrap_err();
        assert!(e.contains("name"));
    }

    #[test]
    fn check_content_skills_only_skill_md_parsed() {
        // Sibling files in a skill dir must NOT be parsed; only SKILL.md.
        let dir = TempDir::new().unwrap();
        let skill_dir = dir.path().join("skills/multi");
        write(
            &skill_dir.join("SKILL.md"),
            "---\nname: multi\ndescription: a multi-file skill\n---\nbody.\n",
        );
        write(&skill_dir.join("setup.md"), "# Plain markdown.\n");
        let n = check_content(dir.path()).unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn check_content_dotfile_dirs_skipped() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("skills/.hidden/SKILL.md"),
            "---\nname: hidden\ndescription: x\n---\n",
        );
        let n = check_content(dir.path()).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn check_skill_frontmatter_crlf_ok() {
        assert!(check_skill_frontmatter(
            "---\r\nname: foo\r\ndescription: bar\r\n---\r\nbody.\r\n"
        )
        .is_ok());
    }

    // ── plan_one (compare core) ─────────────────────────────────

    fn make_checkout() -> TempDir {
        let dir = TempDir::new().unwrap();
        // Mirror the real layout — resources/content/{agents.md, skills/}.
        fs::create_dir_all(dir.path().join("resources/content/skills")).unwrap();
        write(
            &dir.path().join("resources/content/agents.md"),
            "# Agents preamble\n\nbody.\n",
        );
        dir
    }

    #[test]
    fn plan_one_missing_when_no_target() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        let p = plan_one(REGISTRY[0], home.path(), checkout.path()).unwrap();
        assert_eq!(p.cmp, Comparison::Missing);
    }

    #[test]
    fn plan_one_match_after_install() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        cmd_install(home.path(), checkout.path(), false, false).unwrap();
        let p = plan_one(REGISTRY[0], home.path(), checkout.path()).unwrap();
        assert_eq!(p.cmp, Comparison::Match);
    }

    #[test]
    fn plan_one_wrong_target_when_foreign_symlink() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        let entry = REGISTRY[0];
        let target = home.path().join(entry.0);
        ensure_parent(&target).unwrap();
        std::os::unix::fs::symlink("/nowhere", &target).unwrap();
        let p = plan_one(entry, home.path(), checkout.path()).unwrap();
        assert!(matches!(p.cmp, Comparison::WrongTarget(_)));
    }

    #[test]
    fn plan_one_blocked_by_real_file() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        let entry = REGISTRY[0];
        let target = home.path().join(entry.0);
        ensure_parent(&target).unwrap();
        write(&target, "user content");
        let p = plan_one(entry, home.path(), checkout.path()).unwrap();
        assert_eq!(p.cmp, Comparison::BlockedByRealFile);
    }

    // ── install ─────────────────────────────────────────────────

    #[test]
    fn install_fresh_home_creates_every_entry() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        let rc = cmd_install(home.path(), checkout.path(), false, false).unwrap();
        assert_eq!(rc, 0);
        for entry in REGISTRY {
            let target = home.path().join(entry.0);
            let cur = fs::read_link(&target).unwrap();
            assert_eq!(cur, checkout.path().join(entry.1));
        }
    }

    #[test]
    fn install_second_run_is_idempotent() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        cmd_install(home.path(), checkout.path(), false, false).unwrap();
        let rc = cmd_install(home.path(), checkout.path(), false, false).unwrap();
        assert_eq!(rc, 0);
    }

    #[test]
    fn install_dry_run_makes_no_changes() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        let rc = cmd_install(home.path(), checkout.path(), true, false).unwrap();
        assert_eq!(rc, 0);
        let target = home.path().join(REGISTRY[0].0);
        assert!(!path_exists(&target));
    }

    #[test]
    fn install_wrong_symlink_skipped_without_force() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        let entry = REGISTRY[0];
        let target = home.path().join(entry.0);
        ensure_parent(&target).unwrap();
        std::os::unix::fs::symlink("/nowhere", &target).unwrap();

        let rc = cmd_install(home.path(), checkout.path(), false, false).unwrap();
        assert_eq!(rc, 1);
        // Foreign symlink preserved.
        let cur = fs::read_link(&target).unwrap();
        assert_eq!(cur, PathBuf::from("/nowhere"));
    }

    #[test]
    fn install_wrong_symlink_replaced_with_force() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        let entry = REGISTRY[0];
        let target = home.path().join(entry.0);
        ensure_parent(&target).unwrap();
        std::os::unix::fs::symlink("/nowhere", &target).unwrap();

        let rc = cmd_install(home.path(), checkout.path(), false, true).unwrap();
        assert_eq!(rc, 0);
        let cur = fs::read_link(&target).unwrap();
        assert_eq!(cur, checkout.path().join(entry.1));
    }

    #[test]
    fn install_real_file_preserved() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        let entry = REGISTRY[0];
        let target = home.path().join(entry.0);
        ensure_parent(&target).unwrap();
        write(&target, "user content");

        let rc = cmd_install(home.path(), checkout.path(), false, false).unwrap();
        assert_eq!(rc, 1);
        let still = fs::read_to_string(&target).unwrap();
        assert_eq!(still, "user content");
    }

    // ── uninstall ───────────────────────────────────────────────

    #[test]
    fn uninstall_clean_home_is_noop() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        let rc = cmd_uninstall(home.path(), checkout.path(), false, false).unwrap();
        assert_eq!(rc, 0);
    }

    #[test]
    fn uninstall_removes_managed_symlinks() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        cmd_install(home.path(), checkout.path(), false, false).unwrap();

        let rc = cmd_uninstall(home.path(), checkout.path(), false, false).unwrap();
        assert_eq!(rc, 0);
        for entry in REGISTRY {
            assert!(!path_exists(&home.path().join(entry.0)));
        }
    }

    #[test]
    fn uninstall_idempotent() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        cmd_install(home.path(), checkout.path(), false, false).unwrap();
        cmd_uninstall(home.path(), checkout.path(), false, false).unwrap();
        let rc = cmd_uninstall(home.path(), checkout.path(), false, false).unwrap();
        assert_eq!(rc, 0);
    }

    #[test]
    fn uninstall_foreign_symlink_skipped_without_force() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        let entry = REGISTRY[0];
        let target = home.path().join(entry.0);
        ensure_parent(&target).unwrap();
        std::os::unix::fs::symlink("/nowhere", &target).unwrap();

        let rc = cmd_uninstall(home.path(), checkout.path(), false, false).unwrap();
        assert_eq!(rc, 1);
        // Foreign symlink preserved.
        assert!(path_exists(&target));
    }

    #[test]
    fn uninstall_foreign_symlink_force_removed() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        let entry = REGISTRY[0];
        let target = home.path().join(entry.0);
        ensure_parent(&target).unwrap();
        std::os::unix::fs::symlink("/nowhere", &target).unwrap();

        let rc = cmd_uninstall(home.path(), checkout.path(), false, true).unwrap();
        assert_eq!(rc, 0);
        assert!(!path_exists(&target));
    }

    #[test]
    fn uninstall_user_file_preserved_without_force() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        let entry = REGISTRY[0];
        let target = home.path().join(entry.0);
        ensure_parent(&target).unwrap();
        write(&target, "user content");

        let rc = cmd_uninstall(home.path(), checkout.path(), false, false).unwrap();
        assert_eq!(rc, 1);
        let still = fs::read_to_string(&target).unwrap();
        assert_eq!(still, "user content");
    }

    #[test]
    fn uninstall_dry_run_makes_no_changes() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        cmd_install(home.path(), checkout.path(), false, false).unwrap();

        let rc = cmd_uninstall(home.path(), checkout.path(), true, false).unwrap();
        assert_eq!(rc, 0);
        // All symlinks still in place.
        for entry in REGISTRY {
            assert!(path_exists(&home.path().join(entry.0)));
        }
    }

    // ── status ──────────────────────────────────────────────────

    #[test]
    fn status_runs_clean_on_fresh_install() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        cmd_install(home.path(), checkout.path(), false, false).unwrap();
        let rc = cmd_status(home.path(), checkout.path()).unwrap();
        assert_eq!(rc, 0);
    }

    // ── structural integration test (replaces tests/run --no-tools) ──

    #[test]
    fn structural_install_to_real_directory_layout() {
        // A full install→status→uninstall round-trip against a
        // realistic on-disk layout. Replaces the old
        // tests/functional/run --no-tools bash assertions.
        let checkout = make_checkout();
        // Add a real skill so the skills/ symlink has something to expose.
        write(
            &checkout
                .path()
                .join("resources/content/skills/example/SKILL.md"),
            "---\nname: example\ndescription: a sample skill\n---\nbody.\n",
        );
        let home = TempDir::new().unwrap();

        // install: every REGISTRY entry → Created.
        let rc = cmd_install(home.path(), checkout.path(), false, false).unwrap();
        assert_eq!(rc, 0);
        for entry in REGISTRY {
            let target = home.path().join(entry.0);
            assert!(
                target.is_symlink() || target.exists(),
                "missing {}",
                entry.0
            );
        }

        // The example skill is reachable through the deployed symlink.
        let exposed = home.path().join(".claude/skills/example/SKILL.md");
        assert!(exposed.exists(), "skill not visible via deployed symlink");

        // status returns 0 on a clean install.
        let rc = cmd_status(home.path(), checkout.path()).unwrap();
        assert_eq!(rc, 0);

        // uninstall: every entry → Removed.
        let rc = cmd_uninstall(home.path(), checkout.path(), false, false).unwrap();
        assert_eq!(rc, 0);
        for entry in REGISTRY {
            assert!(!path_exists(&home.path().join(entry.0)));
        }
    }
}
