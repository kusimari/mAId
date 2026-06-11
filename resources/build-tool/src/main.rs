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
//   - resources/content/agents.md       plain markdown preamble (no
//                                        frontmatter — cross-tool
//                                        AGENTS.md standard). Check:
//                                        present + non-empty.
//   - resources/content/skills/<name>/SKILL.md
//                                       YAML frontmatter required:
//                                        name + description, both
//                                        non-empty.
// ─────────────────────────────────────────────────────────────────

/// Validate `resources/content/`, returning the count of validated
/// files or the joined error list. Caller decides whether to print or
/// abort.
fn check_content(content_dir: &Path) -> Result<usize, Vec<String>> {
    let agents_md = content_dir.join("agents.md");
    let agents_result: Option<Result<(), String>> =
        agents_md.exists().then(|| check_agents_md(&agents_md));

    let skill_results: Vec<Result<(), String>> = fs::read_dir(content_dir.join("skills"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("SKILL.md"))
        .filter(|p| p.exists())
        .map(|p| check_one_skill(&p))
        .collect();

    let (oks, errs): (Vec<_>, Vec<_>) = agents_result
        .into_iter()
        .chain(skill_results)
        .partition(Result::is_ok);

    if errs.is_empty() {
        Ok(oks.len())
    } else {
        Err(errs.into_iter().map(Result::unwrap_err).collect())
    }
}

fn check_agents_md(path: &Path) -> Result<(), String> {
    fs::read_to_string(path)
        .map_err(|e| format!("{}: cannot read: {e}", path.display()))
        .and_then(|body| {
            (!body.trim().is_empty())
                .then_some(())
                .ok_or_else(|| format!("{}: AGENTS.md preamble is empty", path.display()))
        })
}

fn check_one_skill(path: &Path) -> Result<(), String> {
    fs::read_to_string(path)
        .map_err(|e| format!("{}: cannot read: {e}", path.display()))
        .and_then(|body| {
            check_skill_frontmatter(&body).map_err(|e| format!("{}: {e}", path.display()))
        })
}

/// Validate a SKILL.md's YAML frontmatter: `---` fence + `name` and
/// `description` fields, both non-empty. Implementation defers to
/// gray_matter (envelope) + serde (schema) — the schema is the
/// in-function struct.
fn check_skill_frontmatter(content: &str) -> Result<(), String> {
    #[derive(serde::Deserialize)]
    struct SkillFrontmatter {
        name: String,
        description: String,
    }

    gray_matter::Matter::<gray_matter::engine::YAML>::new()
        .parse::<SkillFrontmatter>(content)
        .map_err(|e| e.to_string())?
        .data
        .ok_or_else(|| "missing or unterminated YAML frontmatter".to_string())
        .and_then(|fm| {
            (!fm.name.trim().is_empty() && !fm.description.trim().is_empty())
                .then_some(())
                .ok_or_else(|| "name and description must be non-empty".into())
        })
}

// ─────────────────────────────────────────────────────────────────
// 3. Compare — the shared core for install / uninstall / status.
//
// Each verb walks REGISTRY and asks `plan_one()` what it sees at the
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
    /// Remove install-managed symlinks.
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
    let count = check_content(&checkout.join("resources").join("content"))
        .map_err(|errs| anyhow!("Content validation failed:\n{}", errs.join("\n")))?;
    eprintln!("validated {count} content file(s)");

    let failures: usize = REGISTRY
        .iter()
        .map(|e| install_one(*e, home, checkout, dry_run, force))
        .collect::<io::Result<Vec<bool>>>()?
        .into_iter()
        .filter(|&fail| fail)
        .count();
    Ok(if failures > 0 { 1 } else { 0 })
}

fn cmd_uninstall(home: &Path, checkout: &Path, dry_run: bool, force: bool) -> Result<u8> {
    let failures: usize = REGISTRY
        .iter()
        .map(|e| uninstall_one(*e, home, checkout, dry_run, force))
        .collect::<io::Result<Vec<bool>>>()?
        .into_iter()
        .filter(|&fail| fail)
        .count();
    Ok(if failures > 0 { 1 } else { 0 })
}

fn cmd_status(home: &Path, checkout: &Path) -> Result<u8> {
    REGISTRY.iter().try_for_each(|entry| -> io::Result<()> {
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
        Ok(())
    })?;
    Ok(0)
}

/// Apply install logic to a single registry entry. Returns `Ok(true)` if
/// the entry was a soft skip (counts as a failure for exit code), `Ok(false)`
/// otherwise. Real I/O errors propagate as `Err`.
fn install_one(
    entry: Entry,
    home: &Path,
    checkout: &Path,
    dry_run: bool,
    force: bool,
) -> io::Result<bool> {
    let plan = plan_one(entry, home, checkout)?;
    let tag = if dry_run { "(dry-run) " } else { "" };
    let target = plan.home.display();
    match plan.cmp {
        Comparison::Match => {
            println!("{tag}ok        {target}");
            Ok(false)
        }
        Comparison::Missing => {
            if !dry_run {
                ensure_parent(&plan.home)?;
                std::os::unix::fs::symlink(&plan.source, &plan.home)?;
            }
            println!("{tag}created   {target}");
            Ok(false)
        }
        Comparison::WrongTarget(current) if force => {
            if !dry_run {
                fs::remove_file(&plan.home)?;
                std::os::unix::fs::symlink(&plan.source, &plan.home)?;
            }
            println!("{tag}replaced  {target} (was {})", current.display());
            Ok(false)
        }
        Comparison::WrongTarget(current) => {
            eprintln!(
                "{tag}skip      {target} (points elsewhere: {}; use --force to replace)",
                current.display()
            );
            Ok(true)
        }
        Comparison::BlockedByRealFile => {
            eprintln!("{tag}skip      {target} (existing file; not overwriting)");
            Ok(true)
        }
        Comparison::BlockedByRealDir => {
            eprintln!("{tag}skip      {target} (existing dir; not overwriting)");
            Ok(true)
        }
        Comparison::SourceMissing => {
            println!("{tag}skip      {target} (source missing)");
            Ok(false)
        }
    }
}

fn uninstall_one(
    entry: Entry,
    home: &Path,
    checkout: &Path,
    dry_run: bool,
    force: bool,
) -> io::Result<bool> {
    let plan = plan_one(entry, home, checkout)?;
    let tag = if dry_run { "(dry-run) " } else { "" };
    let target = plan.home.display();
    match plan.cmp {
        Comparison::Match => {
            if !dry_run {
                fs::remove_file(&plan.home)?;
            }
            println!("{tag}removed       {target}");
            Ok(false)
        }
        Comparison::Missing | Comparison::SourceMissing => {
            println!("{tag}not-installed {target}");
            Ok(false)
        }
        Comparison::WrongTarget(current) if force => {
            if !dry_run {
                fs::remove_file(&plan.home)?;
            }
            println!(
                "{tag}force-removed {target} (was symlink -> {})",
                current.display()
            );
            Ok(false)
        }
        Comparison::WrongTarget(current) => {
            eprintln!(
                "{tag}skip          {target} (foreign symlink -> {}; use --force to remove)",
                current.display()
            );
            Ok(true)
        }
        Comparison::BlockedByRealFile if force => {
            if !dry_run {
                fs::remove_file(&plan.home)?;
            }
            println!("{tag}force-removed {target} (was file)");
            Ok(false)
        }
        Comparison::BlockedByRealDir if force => {
            if !dry_run {
                fs::remove_dir_all(&plan.home)?;
            }
            println!("{tag}force-removed {target} (was dir)");
            Ok(false)
        }
        Comparison::BlockedByRealFile => {
            eprintln!(
                "{tag}skip          {target} (existing file; not managed; use --force to remove)"
            );
            Ok(true)
        }
        Comparison::BlockedByRealDir => {
            eprintln!(
                "{tag}skip          {target} (existing dir; not managed; use --force to remove)"
            );
            Ok(true)
        }
    }
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
    let home = PathBuf::from(&raw);
    (!raw.is_empty() && home.is_absolute())
        .then_some(home)
        .ok_or_else(|| anyhow!("HOME must be a non-empty absolute path (got {raw:?})"))
}

// ─────────────────────────────────────────────────────────────────
// 6. Tests.
// ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(p: &Path, s: &str) {
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, s).unwrap();
    }

    // ── content checks ──────────────────────────────────────────

    #[test]
    fn check_content_empty_root_ok() {
        let dir = TempDir::new().unwrap();
        assert_eq!(check_content(dir.path()).unwrap(), 0);
    }

    #[test]
    fn check_content_agents_md_present_ok() {
        let dir = TempDir::new().unwrap();
        write(&dir.path().join("agents.md"), "# preamble\n\nbody.\n");
        assert_eq!(check_content(dir.path()).unwrap(), 1);
    }

    #[test]
    fn check_content_agents_md_empty_rejected() {
        let dir = TempDir::new().unwrap();
        write(&dir.path().join("agents.md"), "  \n\n  \n");
        let errs = check_content(dir.path()).unwrap_err();
        assert!(errs
            .iter()
            .any(|e| e.contains("AGENTS.md preamble is empty")));
    }

    #[test]
    fn check_content_skill_with_frontmatter_ok() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("skills/foo/SKILL.md"),
            "---\nname: foo\ndescription: bar\n---\nbody.\n",
        );
        assert_eq!(check_content(dir.path()).unwrap(), 1);
    }

    #[test]
    fn check_content_skill_missing_description_rejected() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("skills/foo/SKILL.md"),
            "---\nname: foo\n---\n",
        );
        let errs = check_content(dir.path()).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("description")));
    }

    #[test]
    fn check_content_collects_multiple_errors() {
        // Both agents.md (empty) AND a SKILL.md (missing description)
        // — caller sees ALL problems, not just the first.
        let dir = TempDir::new().unwrap();
        write(&dir.path().join("agents.md"), "");
        write(
            &dir.path().join("skills/foo/SKILL.md"),
            "---\nname: foo\n---\n",
        );
        let errs = check_content(dir.path()).unwrap_err();
        assert_eq!(errs.len(), 2);
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
        assert_eq!(check_content(dir.path()).unwrap(), 1);
    }

    // ── frontmatter (gray_matter + serde) ───────────────────────

    #[test]
    fn frontmatter_minimal_ok() {
        assert!(check_skill_frontmatter("---\nname: foo\ndescription: bar\n---\nbody.\n").is_ok());
    }

    #[test]
    fn frontmatter_with_extra_fields_ok() {
        // Real YAML: extra fields are ignored by serde when not in the struct.
        assert!(check_skill_frontmatter(
            "---\nname: foo\ndescription: bar\nversion: 1.0.0\ntags: [a, b]\n---\nbody.\n"
        )
        .is_ok());
    }

    #[test]
    fn frontmatter_quoted_values_ok() {
        // Real YAML handles quoting natively — no hand-rolled unquote.
        assert!(
            check_skill_frontmatter("---\nname: \"foo\"\ndescription: 'bar baz'\n---\n").is_ok()
        );
    }

    #[test]
    fn frontmatter_missing_fence_rejected() {
        let e = check_skill_frontmatter("name: foo\ndescription: bar\n").unwrap_err();
        assert!(e.contains("frontmatter"));
    }

    #[test]
    fn frontmatter_unterminated_rejected() {
        let e = check_skill_frontmatter("---\nname: foo\ndescription: bar\n").unwrap_err();
        assert!(e.contains("frontmatter"));
    }

    #[test]
    fn frontmatter_missing_name_rejected() {
        let e = check_skill_frontmatter("---\ndescription: bar\n---\n").unwrap_err();
        assert!(e.contains("name"));
    }

    #[test]
    fn frontmatter_missing_description_rejected() {
        let e = check_skill_frontmatter("---\nname: foo\n---\n").unwrap_err();
        assert!(e.contains("description"));
    }

    #[test]
    fn frontmatter_empty_value_rejected() {
        let e = check_skill_frontmatter("---\nname: \"\"\ndescription: bar\n---\n").unwrap_err();
        assert!(e.contains("non-empty"));
    }

    // ── plan_one (compare core) ─────────────────────────────────

    fn make_checkout() -> TempDir {
        let dir = TempDir::new().unwrap();
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
            let cur = fs::read_link(home.path().join(entry.0)).unwrap();
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
        cmd_install(home.path(), checkout.path(), true, false).unwrap();
        assert!(!path_exists(&home.path().join(REGISTRY[0].0)));
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
        assert_eq!(fs::read_to_string(&target).unwrap(), "user content");
    }

    // ── uninstall ───────────────────────────────────────────────

    #[test]
    fn uninstall_clean_home_is_noop() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        assert_eq!(
            cmd_uninstall(home.path(), checkout.path(), false, false).unwrap(),
            0
        );
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
        assert_eq!(
            cmd_uninstall(home.path(), checkout.path(), false, false).unwrap(),
            0
        );
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
        assert_eq!(fs::read_to_string(&target).unwrap(), "user content");
    }

    #[test]
    fn uninstall_dry_run_makes_no_changes() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        cmd_install(home.path(), checkout.path(), false, false).unwrap();

        cmd_uninstall(home.path(), checkout.path(), true, false).unwrap();
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
        assert_eq!(cmd_status(home.path(), checkout.path()).unwrap(), 0);
    }

    // ── structural integration test ─────────────────────────────

    #[test]
    fn structural_install_to_real_directory_layout() {
        // Full install→status→uninstall round-trip against a realistic
        // on-disk layout with a real skill exposed through the symlink.
        let checkout = make_checkout();
        write(
            &checkout
                .path()
                .join("resources/content/skills/example/SKILL.md"),
            "---\nname: example\ndescription: a sample skill\n---\nbody.\n",
        );
        let home = TempDir::new().unwrap();

        assert_eq!(
            cmd_install(home.path(), checkout.path(), false, false).unwrap(),
            0
        );
        for entry in REGISTRY {
            let target = home.path().join(entry.0);
            assert!(
                target.is_symlink() || target.exists(),
                "missing {}",
                entry.0
            );
        }
        assert!(
            home.path().join(".claude/skills/example/SKILL.md").exists(),
            "skill not visible via deployed symlink"
        );

        assert_eq!(cmd_status(home.path(), checkout.path()).unwrap(), 0);

        assert_eq!(
            cmd_uninstall(home.path(), checkout.path(), false, false).unwrap(),
            0
        );
        for entry in REGISTRY {
            assert!(!path_exists(&home.path().join(entry.0)));
        }
    }
}
