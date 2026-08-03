//! The pipeline, one section per stage. Each stage consumes what the
//! previous one produced:
//!
//!   1 content — resources/content/ → a valid skill
//!   2 install — a valid skill → $HOME symlinks
//!
//! Reads `shared` for the registry, agents, and roots; nothing here
//! reaches back into the CLI.

use anyhow::{anyhow, Result};
use build_tool::shared::{selected_entries, Agent, Entry, Kind, Link};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

// ─────────────────────────────────────────────────────────────────
// Stage 1 · content — resources/content/ → a valid skill.
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
// Stage 2 · plan — what is at the home path, and what should be.
//
// Pure: every verb below asks `plan_one()` what it sees and decides the
// action. Nothing here mutates the filesystem, which is what makes the
// force/dry-run semantics testable apart from their effects.
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
// Stage 2 · apply — the mutations.
//
// The three verbs, each walking the selected registry rows, expanding
// them to concrete links, and acting on plan_one's verdict. `agent` is
// already an `Agent` here: the CLI owns token parsing, so a stage never
// sees an unvalidated string.
// ─────────────────────────────────────────────────────────────────

pub fn cmd_install(
    home: &Path,
    checkout: &Path,
    dry_run: bool,
    force: bool,
    agent: Option<Agent>,
) -> Result<u8> {
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

pub fn cmd_uninstall(
    home: &Path,
    checkout: &Path,
    dry_run: bool,
    force: bool,
    agent: Option<Agent>,
) -> Result<u8> {
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

pub fn cmd_status(home: &Path, checkout: &Path, agent: Option<Agent>) -> Result<u8> {
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
