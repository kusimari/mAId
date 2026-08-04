//! build-tool — mAId's tooling for installing checked-in markdown
//! resources into the AI tools that consume them. Single-file
//! Rust crate; the whole job is small enough that splitting it
//! into modules adds noise.
//!
//! Invoked via the project's Justfile:
//!   just resources::install-skills [agent]    create the $HOME-facing symlinks (after content checks)
//!   just resources::uninstall-skills [agent]  remove the managed symlinks
//!   just resources::status-skills [agent]     report each managed symlink's current state
//! An optional `--agent <claude|kiro|codex>` scopes any of the three
//! to one coding agent; the default is all of them.
//!
//! `just resources::verify` is a separate verb that drives
//! `claude --print` (and kiro/codex) against the installed content;
//! that lives in the Justfile and shells into `resources/tests/run`,
//! not in this binary.

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

// ─────────────────────────────────────────────────────────────────
// 1. Registry — the deployment manifest.
//
// The job: mAId keeps skills in the checkout; each coding agent expects
// them under its own home dir and discovers them there natively (claude
// ~/.claude/skills, kiro ~/.kiro/steering, codex ~/.codex/skills — all
// verified to load skills with no extra preamble). The registry maps
// checkout source → agent home, one row per target, in one of two
// shapes (`Kind`):
//
//   Link   — the agent's home layout matches the checkout, so symlink
//            the home path straight at the source dir. mAId owns it.
//   FanOut — the agent owns the home dir and puts its own entries
//            there, so we can't replace it; mirror each source child in
//            as its own symlink and leave the rest alone.
//
// Skills are all that's installed. There is no global instruction
// preamble: loading a project's AGENTS.md / project.md is kdevkit's
// work-time instruction, and AGENTS.md is a repo-root convention, not a
// global per-tool file.
// ─────────────────────────────────────────────────────────────────

type Entry = (&'static str, &'static str, Kind, &'static str); // (home_subpath, source_subpath, kind, agent)

/// A concrete symlink to manage, resolved from an entry: (home, source).
type Link = (PathBuf, PathBuf);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Link,
    FanOut,
}

/// The coding agents mAId deploys to. An install/uninstall/status can
/// be scoped to one (`--agent`) or, by default, cover them all. This
/// list is also the recognized universe used to validate `--agent`.
const AGENTS: &[&str] = &["claude", "kiro", "codex"];

const REGISTRY: &[Entry] = &[
    (
        ".claude/skills",
        "resources/content/skills",
        Kind::Link,
        "claude",
    ),
    (
        ".kiro/steering/skills",
        "resources/content/skills",
        Kind::Link,
        "kiro",
    ),
    (
        ".codex/skills",
        "resources/content/skills",
        Kind::FanOut,
        "codex",
    ),
];

/// Filter REGISTRY to the rows an `--agent` selection acts on: `None`
/// = every row (the default), `Some(a)` = just that agent's rows. An
/// unrecognized agent is rejected by the caller before this runs.
fn selected_entries(agent: Option<&str>) -> Vec<Entry> {
    REGISTRY
        .iter()
        .filter(|(.., a)| agent.is_none_or(|sel| sel == *a))
        .copied()
        .collect()
}

/// Validate an `--agent` value against the known set, returning it
/// back for chaining. An unknown agent is a hard error listing the
/// valid names, so a typo never silently installs nothing.
fn validate_agent(agent: Option<&str>) -> Result<Option<&str>> {
    match agent {
        Some(a) if !AGENTS.contains(&a) => Err(anyhow!(
            "unknown coding agent {a:?} (known: {})",
            AGENTS.join(", ")
        )),
        other => Ok(other),
    }
}

// ─────────────────────────────────────────────────────────────────
// 2. Content checks.
//
// Only skills are deployed. Each resources/content/skills/<name>/
// SKILL.md needs YAML frontmatter with a non-empty name + description.
// ─────────────────────────────────────────────────────────────────

/// Validate `resources/content/`, returning the count of validated
/// files or the joined error list. Caller decides whether to print or
/// abort.
fn check_content(content_dir: &Path) -> Result<usize, Vec<String>> {
    // Walk every skill, partition into (oks, errs), and report the Ok
    // count or the FULL error list — matching on the partitioned tuple
    // so there's no intermediate binding but all errors are still kept.
    match fs::read_dir(content_dir.join("skills"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("SKILL.md"))
        .filter(|p| p.exists())
        .map(|p| check_one_skill(&p))
        .partition::<Vec<_>, _>(Result::is_ok)
    {
        (oks, errs) if errs.is_empty() => Ok(oks.len()),
        (_, errs) => Err(errs.into_iter().map(Result::unwrap_err).collect()),
    }
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

/// Resolve a registry entry to the concrete symlinks it manages —
/// `Link` yields one; `FanOut` yields one per child. FanOut unions the
/// source's current children (what should exist) with home symlinks
/// already pointing into this source (so a child renamed or removed in
/// source is still reaped, not orphaned as a dangling link in a dir we
/// don't own). Keyed by home path for dedupe + deterministic order.
fn expand(entry: Entry, home_root: &Path, checkout: &Path) -> io::Result<Vec<Link>> {
    let (home_sub, source_sub, kind, _agent) = entry;
    let home = home_root.join(home_sub);
    let source = checkout.join(source_sub);
    match kind {
        Kind::Link => Ok(vec![(home, source)]),
        Kind::FanOut => {
            use std::collections::BTreeMap;
            let mut links: BTreeMap<PathBuf, PathBuf> = BTreeMap::new();
            if source.is_dir() {
                for e in fs::read_dir(&source)?.filter_map(Result::ok) {
                    links.insert(home.join(e.file_name()), e.path());
                }
            }
            if home.is_dir() {
                for e in fs::read_dir(&home)?.filter_map(Result::ok) {
                    let h = e.path();
                    if let Ok(target) = fs::read_link(&h) {
                        if target.starts_with(&source) {
                            links.entry(h).or_insert(target);
                        }
                    }
                }
            }
            Ok(links.into_iter().collect())
        }
    }
}

fn plan_one(home: PathBuf, source: PathBuf) -> io::Result<Plan> {
    // Inspect home first: a symlink already pointing at `source` is a
    // Match to reap even if `source` is now gone (an orphaned fan-out
    // child). SourceMissing only when there's nothing at home to act on.
    let cmp = match fs::symlink_metadata(&home) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            if path_exists(&source) {
                Comparison::Missing
            } else {
                Comparison::SourceMissing
            }
        }
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

fn run(cli: Cli) -> Result<u8> {
    let root = repo_root()?;
    let home = home_dir()?;
    match cli.cmd {
        Cmd::Install {
            dry_run,
            force,
            agent,
        } => cmd_install(&home, &root, dry_run, force, agent.as_deref()),
        Cmd::Uninstall {
            dry_run,
            force,
            agent,
        } => cmd_uninstall(&home, &root, dry_run, force, agent.as_deref()),
        Cmd::Status { agent } => cmd_status(&home, &root, agent.as_deref()),
    }
}

fn cmd_install(
    home: &Path,
    checkout: &Path,
    dry_run: bool,
    force: bool,
    agent: Option<&str>,
) -> Result<u8> {
    let agent = validate_agent(agent)?;
    let count = check_content(&checkout.join("resources").join("content"))
        .map_err(|errs| anyhow!("Content validation failed:\n{}", errs.join("\n")))?;
    eprintln!("validated {count} content file(s)");

    let failures: usize = selected_entries(agent)
        .iter()
        .map(|e| expand(*e, home, checkout))
        .collect::<io::Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .map(|(h, s)| install_one(h, s, dry_run, force))
        .collect::<io::Result<Vec<bool>>>()?
        .into_iter()
        .filter(|&fail| fail)
        .count();
    Ok(if failures > 0 { 1 } else { 0 })
}

fn cmd_uninstall(
    home: &Path,
    checkout: &Path,
    dry_run: bool,
    force: bool,
    agent: Option<&str>,
) -> Result<u8> {
    let agent = validate_agent(agent)?;
    let failures: usize = selected_entries(agent)
        .iter()
        .map(|e| expand(*e, home, checkout))
        .collect::<io::Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .map(|(h, s)| uninstall_one(h, s, dry_run, force))
        .collect::<io::Result<Vec<bool>>>()?
        .into_iter()
        .filter(|&fail| fail)
        .count();
    Ok(if failures > 0 { 1 } else { 0 })
}

fn cmd_status(home: &Path, checkout: &Path, agent: Option<&str>) -> Result<u8> {
    let agent = validate_agent(agent)?;
    for entry in selected_entries(agent) {
        for (h, s) in expand(entry, home, checkout)? {
            // Label by the home path relative to $HOME so fan-out
            // children read as `.codex/skills/<name>`.
            let label = h.strip_prefix(home).unwrap_or(&h).display().to_string();
            let plan = plan_one(h.clone(), s)?;
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
            println!("{label:<28} {state}");
        }
    }
    Ok(0)
}

/// Apply install logic to a single concrete link. Returns `Ok(true)` if
/// the link was a soft skip (counts as a failure for exit code), `Ok(false)`
/// otherwise. Real I/O errors propagate as `Err`.
fn install_one(home: PathBuf, source: PathBuf, dry_run: bool, force: bool) -> io::Result<bool> {
    let plan = plan_one(home, source)?;
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

fn uninstall_one(home: PathBuf, source: PathBuf, dry_run: bool, force: bool) -> io::Result<bool> {
    let plan = plan_one(home, source)?;
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
        Comparison::BlockedByRealDir => {
            // mAId only ever installs symlinks, so a real dir at a
            // managed path belongs to the owning tool — never removed,
            // even with --force (which only reaps foreign symlinks/files).
            eprintln!(
                "{tag}skip          {target} (real dir; not mAId-managed — refusing to remove)"
            );
            Ok(true)
        }
        Comparison::BlockedByRealFile => {
            eprintln!(
                "{tag}skip          {target} (existing file; not managed; use --force to remove)"
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

    /// The shipped content must pass its own validator.
    ///
    /// Every other test here builds a synthetic tree in a `TempDir`, so
    /// none of them look at what actually deploys — which is how two
    /// skills reached `main` with a `description:` that YAML read as a
    /// nested mapping (an unquoted `": "` inside the value). The suite
    /// was green; `install-skills` refused to run. This closes that gap:
    /// if content in the repo can't be installed, `just test` says so.
    #[test]
    fn shipped_content_validates() {
        let content = repo_root()
            .expect("repo root resolves under cargo test")
            .join("resources/content");
        match check_content(&content) {
            Ok(n) => assert!(
                n > 0,
                "no skills found under {} — the walk is broken",
                content.display()
            ),
            Err(errs) => panic!(
                "shipped content is not installable:\n  {}",
                errs.join("\n  ")
            ),
        }
    }

    #[test]
    fn check_content_collects_multiple_errors() {
        // Two bad skills — caller sees ALL problems, not just the first.
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("skills/foo/SKILL.md"),
            "---\nname: foo\n---\n",
        );
        write(
            &dir.path().join("skills/bar/SKILL.md"),
            "---\nname: bar\n---\n",
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
        dir
    }

    /// Resolve `REGISTRY[0]` (`.claude/skills`, a `Kind::Link` entry) to
    /// its single (home, source) pair for the per-link plan_one tests.
    fn link0(home: &Path, checkout: &Path) -> (PathBuf, PathBuf) {
        expand(REGISTRY[0], home, checkout)
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
    }

    #[test]
    fn plan_one_missing_when_no_target() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        let (h, s) = link0(home.path(), checkout.path());
        let p = plan_one(h, s).unwrap();
        assert_eq!(p.cmp, Comparison::Missing);
    }

    #[test]
    fn plan_one_match_after_install() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        cmd_install(home.path(), checkout.path(), false, false, None).unwrap();
        let (h, s) = link0(home.path(), checkout.path());
        let p = plan_one(h, s).unwrap();
        assert_eq!(p.cmp, Comparison::Match);
    }

    #[test]
    fn plan_one_wrong_target_when_foreign_symlink() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        let (h, s) = link0(home.path(), checkout.path());
        ensure_parent(&h).unwrap();
        std::os::unix::fs::symlink("/nowhere", &h).unwrap();
        let p = plan_one(h, s).unwrap();
        assert!(matches!(p.cmp, Comparison::WrongTarget(_)));
    }

    #[test]
    fn plan_one_blocked_by_real_file() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        let (h, s) = link0(home.path(), checkout.path());
        ensure_parent(&h).unwrap();
        write(&h, "user content");
        let p = plan_one(h, s).unwrap();
        assert_eq!(p.cmp, Comparison::BlockedByRealFile);
    }

    // ── install ─────────────────────────────────────────────────

    #[test]
    fn install_fresh_home_creates_every_entry() {
        let checkout = make_checkout_with_skills();
        let home = TempDir::new().unwrap();
        let rc = cmd_install(home.path(), checkout.path(), false, false, None).unwrap();
        assert_eq!(rc, 0);
        // Every managed symlink is the set of expanded links across all
        // entries (fan-out entries contribute one link per source child).
        for entry in REGISTRY {
            for (h, s) in expand(*entry, home.path(), checkout.path()).unwrap() {
                assert_eq!(fs::read_link(&h).unwrap(), s);
            }
        }
    }

    #[test]
    fn install_second_run_is_idempotent() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        cmd_install(home.path(), checkout.path(), false, false, None).unwrap();
        let rc = cmd_install(home.path(), checkout.path(), false, false, None).unwrap();
        assert_eq!(rc, 0);
    }

    #[test]
    fn install_dry_run_makes_no_changes() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        cmd_install(home.path(), checkout.path(), true, false, None).unwrap();
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

        let rc = cmd_install(home.path(), checkout.path(), false, false, None).unwrap();
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

        let rc = cmd_install(home.path(), checkout.path(), false, true, None).unwrap();
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

        let rc = cmd_install(home.path(), checkout.path(), false, false, None).unwrap();
        assert_eq!(rc, 1);
        assert_eq!(fs::read_to_string(&target).unwrap(), "user content");
    }

    // ── uninstall ───────────────────────────────────────────────

    #[test]
    fn uninstall_clean_home_is_noop() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        assert_eq!(
            cmd_uninstall(home.path(), checkout.path(), false, false, None).unwrap(),
            0
        );
    }

    #[test]
    fn uninstall_removes_managed_symlinks() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        cmd_install(home.path(), checkout.path(), false, false, None).unwrap();

        let rc = cmd_uninstall(home.path(), checkout.path(), false, false, None).unwrap();
        assert_eq!(rc, 0);
        for entry in REGISTRY {
            assert!(!path_exists(&home.path().join(entry.0)));
        }
    }

    #[test]
    fn uninstall_idempotent() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        cmd_install(home.path(), checkout.path(), false, false, None).unwrap();
        cmd_uninstall(home.path(), checkout.path(), false, false, None).unwrap();
        assert_eq!(
            cmd_uninstall(home.path(), checkout.path(), false, false, None).unwrap(),
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

        let rc = cmd_uninstall(home.path(), checkout.path(), false, false, None).unwrap();
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

        let rc = cmd_uninstall(home.path(), checkout.path(), false, true, None).unwrap();
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

        let rc = cmd_uninstall(home.path(), checkout.path(), false, false, None).unwrap();
        assert_eq!(rc, 1);
        assert_eq!(fs::read_to_string(&target).unwrap(), "user content");
    }

    #[test]
    fn uninstall_dry_run_makes_no_changes() {
        let checkout = make_checkout_with_skills();
        let home = TempDir::new().unwrap();
        cmd_install(home.path(), checkout.path(), false, false, None).unwrap();

        cmd_uninstall(home.path(), checkout.path(), true, false, None).unwrap();
        for entry in REGISTRY {
            for (h, _) in expand(*entry, home.path(), checkout.path()).unwrap() {
                assert!(path_exists(&h));
            }
        }
    }

    // ── status ──────────────────────────────────────────────────

    #[test]
    fn status_runs_clean_on_fresh_install() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        cmd_install(home.path(), checkout.path(), false, false, None).unwrap();
        assert_eq!(cmd_status(home.path(), checkout.path(), None).unwrap(), 0);
    }

    // ── fan-out kind (codex-owned skills dir) ───────────────────

    /// A checkout whose skills source holds two child skills, for
    /// exercising the `Kind::FanOut` entry (`.codex/skills`).
    fn make_checkout_with_skills() -> TempDir {
        let dir = make_checkout();
        for name in ["kdevkit", "notes"] {
            write(
                &dir.path()
                    .join("resources/content/skills")
                    .join(name)
                    .join("SKILL.md"),
                "---\nname: x\ndescription: y\n---\nbody.\n",
            );
        }
        dir
    }

    #[test]
    fn fanout_expands_to_one_link_per_source_child() {
        let checkout = make_checkout_with_skills();
        let home = TempDir::new().unwrap();
        let codex_skills = REGISTRY
            .iter()
            .find(|(h, _, k, _)| *k == Kind::FanOut && h.contains("codex"))
            .copied()
            .expect("a codex fan-out entry");
        let links = expand(codex_skills, home.path(), checkout.path()).unwrap();
        let names: Vec<_> = links
            .iter()
            .map(|(h, _)| h.file_name().unwrap().to_str().unwrap().to_string())
            .collect();
        assert_eq!(names, vec!["kdevkit", "notes"]); // sorted, one per child
    }

    #[test]
    fn fanout_installs_children_and_preserves_tool_owned_siblings() {
        let checkout = make_checkout_with_skills();
        let home = TempDir::new().unwrap();
        // Codex owns ~/.codex/skills and ships its own .system/ inside.
        let system_marker = home.path().join(".codex/skills/.system/.marker");
        write(&system_marker, "codex-owned");

        let rc = cmd_install(home.path(), checkout.path(), false, false, None).unwrap();
        assert_eq!(rc, 0);

        // Each source skill is now a symlink child of ~/.codex/skills.
        for name in ["kdevkit", "notes"] {
            let child = home.path().join(".codex/skills").join(name);
            assert!(child.is_symlink(), "{name} not linked into codex skills");
            assert_eq!(
                fs::read_link(&child).unwrap(),
                checkout.path().join("resources/content/skills").join(name)
            );
        }
        // Codex's own .system/ is untouched.
        assert_eq!(fs::read_to_string(&system_marker).unwrap(), "codex-owned");
    }

    #[test]
    fn fanout_uninstall_removes_children_leaves_tool_owned() {
        let checkout = make_checkout_with_skills();
        let home = TempDir::new().unwrap();
        let system_marker = home.path().join(".codex/skills/.system/.marker");
        write(&system_marker, "codex-owned");

        cmd_install(home.path(), checkout.path(), false, false, None).unwrap();
        let rc = cmd_uninstall(home.path(), checkout.path(), false, false, None).unwrap();
        assert_eq!(rc, 0);

        for name in ["kdevkit", "notes"] {
            assert!(!path_exists(&home.path().join(".codex/skills").join(name)));
        }
        // The tool-owned sibling survives the round-trip.
        assert_eq!(fs::read_to_string(&system_marker).unwrap(), "codex-owned");
    }

    #[test]
    fn fanout_reaps_orphaned_child_after_source_removal() {
        // Install two skills, then remove one from source. expand() must
        // still surface the orphaned home symlink so uninstall reaps it
        // rather than leaving a dangling link in codex's dir.
        let checkout = make_checkout_with_skills();
        let home = TempDir::new().unwrap();
        cmd_install(home.path(), checkout.path(), false, false, None).unwrap();
        // Drop "notes" from the source checkout.
        fs::remove_dir_all(checkout.path().join("resources/content/skills/notes")).unwrap();

        let codex = REGISTRY
            .iter()
            .find(|(h, _, k, _)| *k == Kind::FanOut && h.contains("codex"))
            .copied()
            .unwrap();
        let homes: Vec<_> = expand(codex, home.path(), checkout.path())
            .unwrap()
            .into_iter()
            .map(|(h, _)| h)
            .collect();
        // The orphaned link is still in the managed set.
        assert!(homes.iter().any(|h| h.ends_with("notes")));

        // Uninstall reaps it — no dangling symlink left behind.
        cmd_uninstall(home.path(), checkout.path(), false, false, None).unwrap();
        assert!(!path_exists(&home.path().join(".codex/skills/notes")));
        assert!(!path_exists(&home.path().join(".codex/skills/kdevkit")));
    }

    #[test]
    fn fanout_uninstall_force_refuses_to_delete_tool_owned_real_dir() {
        // A real dir at a fan-out child path (a codex-owned skill sharing
        // a name with one of ours) is never mAId's to remove — even with
        // --force, which only reaps foreign symlinks and files.
        let checkout = make_checkout_with_skills();
        let home = TempDir::new().unwrap();
        let collision = home.path().join(".codex/skills/kdevkit/OWNED.md");
        write(&collision, "codex-owned skill");

        let rc = cmd_uninstall(home.path(), checkout.path(), false, true, None).unwrap();
        assert_eq!(rc, 1); // soft-skip counts as a failure exit
        assert_eq!(fs::read_to_string(&collision).unwrap(), "codex-owned skill");
    }

    #[test]
    fn fanout_source_missing_yields_no_links() {
        // A bare checkout: skills dir exists but is empty → no children.
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        let codex_skills = REGISTRY
            .iter()
            .find(|(h, _, k, _)| *k == Kind::FanOut && h.contains("codex"))
            .copied()
            .unwrap();
        assert!(expand(codex_skills, home.path(), checkout.path())
            .unwrap()
            .is_empty());
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
        // A deferred module in a subdirectory: not validated, not registered,
        // reachable only because the registry links the skill *directory*.
        write(
            &checkout
                .path()
                .join("resources/content/skills/example/phases/dev.md"),
            "# deferred module\n",
        );
        let home = TempDir::new().unwrap();

        assert_eq!(
            cmd_install(home.path(), checkout.path(), false, false, None).unwrap(),
            0
        );
        for entry in REGISTRY {
            for (h, _) in expand(*entry, home.path(), checkout.path()).unwrap() {
                assert!(h.is_symlink() || h.exists(), "missing {}", h.display());
            }
        }
        // Skill is reachable through every tool's deployed skills path —
        // claude and kiro symlink the whole dir; codex fans out per-child
        // (example is linked into codex's own skills dir as a sibling).
        for tool_skills in [
            ".claude/skills/example/SKILL.md",
            ".kiro/steering/skills/example/SKILL.md",
            ".codex/skills/example/SKILL.md",
        ] {
            assert!(
                home.path().join(tool_skills).exists(),
                "skill not visible via {tool_skills}"
            );
        }
        // Deferred modules must resolve through every tool's path too — a
        // skill that loads them on demand is broken if only SKILL.md arrives.
        for tool_module in [
            ".claude/skills/example/phases/dev.md",
            ".kiro/steering/skills/example/phases/dev.md",
            ".codex/skills/example/phases/dev.md",
        ] {
            assert!(
                home.path().join(tool_module).exists(),
                "deferred module not visible via {tool_module}"
            );
        }

        assert_eq!(cmd_status(home.path(), checkout.path(), None).unwrap(), 0);

        assert_eq!(
            cmd_uninstall(home.path(), checkout.path(), false, false, None).unwrap(),
            0
        );
        for entry in REGISTRY {
            for (h, _) in expand(*entry, home.path(), checkout.path()).unwrap() {
                assert!(!path_exists(&h), "still present: {}", h.display());
            }
        }
    }

    // ── agent selector (--agent) ─────────────────────────────────

    #[test]
    fn selected_entries_default_is_all() {
        assert_eq!(selected_entries(None).len(), REGISTRY.len());
    }

    #[test]
    fn selected_entries_scopes_to_one_agent() {
        let codex = selected_entries(Some("codex"));
        assert_eq!(codex.len(), 1);
        assert_eq!(codex[0].3, "codex");
    }

    #[test]
    fn validate_agent_rejects_unknown() {
        assert!(validate_agent(Some("bogus")).is_err());
        assert!(validate_agent(Some("claude")).is_ok());
        assert!(validate_agent(None).is_ok());
    }

    #[test]
    fn install_scoped_to_one_agent_touches_only_that_agent() {
        // Install only codex; codex's skills dir is populated, the
        // other agents' home paths stay absent.
        let checkout = make_checkout_with_skills();
        let home = TempDir::new().unwrap();
        let rc = cmd_install(home.path(), checkout.path(), false, false, Some("codex")).unwrap();
        assert_eq!(rc, 0);

        // codex fan-out children exist…
        assert!(path_exists(&home.path().join(".codex/skills/kdevkit")));
        // …while claude and kiro whole-dir links were never made.
        assert!(!path_exists(&home.path().join(".claude/skills")));
        assert!(!path_exists(&home.path().join(".kiro/steering/skills")));
    }

    #[test]
    fn uninstall_scoped_leaves_other_agents_installed() {
        // Install all three, then uninstall only claude: claude's link
        // is gone, kiro's and codex's survive.
        let checkout = make_checkout_with_skills();
        let home = TempDir::new().unwrap();
        cmd_install(home.path(), checkout.path(), false, false, None).unwrap();

        let rc = cmd_uninstall(home.path(), checkout.path(), false, false, Some("claude")).unwrap();
        assert_eq!(rc, 0);
        assert!(!path_exists(&home.path().join(".claude/skills")));
        assert!(path_exists(&home.path().join(".kiro/steering/skills")));
        assert!(path_exists(&home.path().join(".codex/skills/kdevkit")));
    }

    #[test]
    fn cmd_install_unknown_agent_errors() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        assert!(cmd_install(home.path(), checkout.path(), false, false, Some("bogus")).is_err());
    }
}
