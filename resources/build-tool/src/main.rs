//! build-tool — mAId's tooling for installing checked-in markdown
//! resources into the AI tools that consume them. Single-file
//! Rust crate; the whole job is small enough that splitting it
//! into modules adds noise.
//!
//! Invoked via the project's Justfile:
//!   just resources::install-skills [target]    populate each destination (after content checks)
//!   just resources::uninstall-skills [target]  remove the managed destinations
//!   just resources::status-skills [target]     report each destination's current state
//! An optional `--agent <claude|kiro|codex|desktop>` scopes any of the
//! three verbs to one target; the default is all of them.
//!
//! Each registry row names the strategy that populates its destination:
//! symlink for the coding agents (live edits), copy for the desktop app
//! (which refuses a symlinked plugin dir). See the registry comment.
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
// The job: mAId keeps skills in the checkout; each target expects them
// somewhere under $HOME and discovers them there natively. The registry
// maps checkout source → target destination, one row per target, and
// each row names the `Strategy` that populates it:
//
//   Link   — the target's layout matches the checkout, so symlink the
//            destination straight at the source dir. mAId owns it.
//   FanOut — the target owns the destination dir and puts its own
//            entries there, so we can't replace it; mirror each source
//            child in as its own symlink and leave the rest alone.
//   Copy   — the destination must hold real files, not symlinks. Copy
//            the source tree in. This is the only strategy where the
//            destination can go *stale* (source edited after install),
//            which `status` reports; the symlink strategies expose edits
//            live and so cannot drift.
//
// Strategy is the third dimension, alongside destination and agent:
// a new target is a row, never a new code path. That is deliberate —
// the one place this tool previously diverged per-agent by hand (MCP
// registration) is how a target got silently omitted.
//
// Skills are all that's installed. There is no global instruction
// preamble: loading a project's AGENTS.md / project.md is kdevkit's
// work-time instruction, and AGENTS.md is a repo-root convention, not a
// global per-tool file.
// ─────────────────────────────────────────────────────────────────

type Entry = (&'static str, &'static str, Strategy, &'static str); // (home_subpath, source_subpath, strategy, agent)

/// A concrete destination to manage, resolved from an entry:
/// (destination, source, strategy). The strategy rides along because
/// every verb dispatches on it — planning, installing and removing a
/// copied tree all differ from the symlink case.
type Link = (PathBuf, PathBuf, Strategy);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Strategy {
    Link,
    FanOut,
    Copy,
}

/// The targets mAId deploys to. An install/uninstall/status can be
/// scoped to one (`--agent`) or, by default, cover them all. This list
/// is also the recognized universe used to validate `--agent`.
const AGENTS: &[&str] = &["claude", "kiro", "codex", "desktop"];

/// Where the desktop app reads org-provisioned plugins from. **Absolute,
/// not `$HOME`-relative**: the app hardcodes this system path, so a
/// per-user `~/Library/...` copy is never read. That is what makes this
/// the one target needing elevation.
///
/// Registry rows are `$HOME`-relative by default; a `/`-rooted row is
/// resolved against `Roots::system` instead, which is `/` in production and
/// a tempdir under test.
const DESKTOP_PLUGIN_PATH: &str = "/Library/Application Support/Claude/org-plugins/maid";

/// The skills that reach the desktop target: **document-shaped only**.
/// Terminal-shaped skills (built around a checkout or a shell session)
/// have nothing to act on in a document workspace, so they are excluded
/// by design rather than pending. See project.md "Desktop target".
const DESKTOP_SKILLS: &[&str] = &["notes", "writing-style"];

/// Written into every `Copy` destination so ownership is a fact rather
/// than an inference. Copy destinations can be system paths that other
/// tools legitimately populate, so "contents differ from source" must not
/// be read as "safe to delete" — only this marker licenses removal.
const OWNED_MARKER: &str = ".maid-managed";

const REGISTRY: &[Entry] = &[
    (
        ".claude/skills",
        "resources/content/skills",
        Strategy::Link,
        "claude",
    ),
    (
        ".kiro/steering/skills",
        "resources/content/skills",
        Strategy::Link,
        "kiro",
    ),
    (
        ".codex/skills",
        "resources/content/skills",
        Strategy::FanOut,
        "codex",
    ),
    // The source is the packaged plugin under build output, not the
    // content tree — the desktop app reads a plugin (manifest + skills),
    // not a bare skills dir. `package_desktop_plugin` builds it.
    (
        DESKTOP_PLUGIN_PATH,
        "target/desktop-plugin/maid",
        Strategy::Copy,
        "desktop",
    ),
];

/// Where a registry destination is rooted. Most rows are `$HOME`-relative;
/// the desktop row is absolute, because the app hardcodes a system path.
///
/// Both roots are passed explicitly rather than read from the environment:
/// the tests need to redirect the absolute root to a tempdir, and a
/// process-global (env var or static) would leak between parallel tests —
/// which showed up here as a real cross-test failure, not a hypothetical.
#[derive(Clone, Copy)]
struct Roots<'a> {
    home: &'a Path,
    /// Prefix applied to absolute destinations. `/` in production; a
    /// tempdir under test.
    system: &'a Path,
}

impl<'a> Roots<'a> {
    fn new(home: &'a Path) -> Self {
        Self {
            home,
            system: Path::new("/"),
        }
    }

    /// Resolve one registry destination. An absolute row is re-rooted at
    /// `system`, so it escapes `$HOME` by design rather than by the
    /// accident of `Path::join`'s absolute-RHS behaviour.
    fn resolve(&self, dest: &str) -> PathBuf {
        let dest = Path::new(dest);
        match dest.strip_prefix("/") {
            Ok(rel) => self.system.join(rel),
            Err(_) => self.home.join(dest),
        }
    }
}

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
    /// The destination matches the source: a symlink pointing where
    /// REGISTRY says, or (for Copy) a tree whose contents are identical.
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
    /// Copy only: the destination exists but its contents differ from
    /// source — the source was edited after the last install. A symlink
    /// can't reach this state, which is why copy needs it: the drift is
    /// otherwise silent.
    Stale,
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
fn expand(entry: Entry, roots: Roots, checkout: &Path) -> io::Result<Vec<Link>> {
    let (home_sub, source_sub, strategy, _agent) = entry;
    let home = roots.resolve(home_sub);
    let source = checkout.join(source_sub);
    match strategy {
        // Copy manages one destination tree, like Link — the difference
        // is the mechanism, not the arity.
        Strategy::Link | Strategy::Copy => Ok(vec![(home, source, strategy)]),
        Strategy::FanOut => {
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
            Ok(links.into_iter().map(|(h, s)| (h, s, strategy)).collect())
        }
    }
}

/// Plan a single destination, dispatching on its strategy. One entry
/// point so every verb gets copy support from the same place.
fn plan_for(home: PathBuf, source: PathBuf, strategy: Strategy) -> io::Result<Plan> {
    match strategy {
        Strategy::Copy => plan_one_copy(home, source),
        Strategy::Link | Strategy::FanOut => plan_one(home, source),
    }
}

/// Compare a Copy destination against its source tree. Content-based,
/// not mtime-based: mtimes shift with checkout order and clock skew,
/// which would report phantom staleness.
fn plan_one_copy(home: PathBuf, source: PathBuf) -> io::Result<Plan> {
    // Ownership is positive, never inferred: a directory only counts as
    // ours if it carries the marker file install writes. Without that,
    // "differs from source" and "belongs to somebody else" are the same
    // observation — and this destination is a system path an MDM or
    // another tool may legitimately populate, so guessing means deleting
    // a stranger's data.
    let cmp = match fs::symlink_metadata(&home) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            if source.is_dir() {
                Comparison::Missing
            } else {
                Comparison::SourceMissing
            }
        }
        Err(e) => return Err(e),
        // A symlink where a real tree belongs was not us: the app refuses
        // symlinked plugin dirs, which is why this strategy exists.
        Ok(meta) if meta.file_type().is_symlink() => Comparison::WrongTarget(fs::read_link(&home)?),
        Ok(meta) if meta.is_dir() => {
            if !home.join(OWNED_MARKER).is_file() {
                Comparison::BlockedByRealDir
            } else if !source.is_dir() {
                // Ours, but nothing to compare against (nothing packaged
                // this run). Removable by uninstall; install skips it.
                Comparison::SourceMissing
            } else if trees_match(&source, &home)? {
                Comparison::Match
            } else {
                Comparison::Stale
            }
        }
        Ok(_) => Comparison::BlockedByRealFile,
    };
    Ok(Plan { home, source, cmp })
}

/// Do two directory trees hold the same relative paths with the same
/// file bytes? Compares the union of both sides so an extra file in the
/// destination counts as a difference, not just a missing one.
fn trees_match(a: &Path, b: &Path) -> io::Result<bool> {
    let (mut ra, mut rb) = (Vec::new(), Vec::new());
    collect_tree(a, a, &mut ra)?;
    collect_tree(b, b, &mut rb)?;
    // The ownership marker is written by install, not present in source,
    // so it must not read as drift.
    rb.retain(|p| p != Path::new(OWNED_MARKER));
    ra.sort();
    rb.sort();
    if ra != rb {
        return Ok(false);
    }
    for rel in ra {
        let (pa, pb) = (a.join(&rel), b.join(&rel));
        // A symlink is drift by definition here: Copy exists because the
        // consuming app refuses symlinked entries, and `fs::read` would
        // follow one and report a false match. Compare link-ness first.
        let (ma, mb) = (fs::symlink_metadata(&pa)?, fs::symlink_metadata(&pb)?);
        if ma.file_type().is_symlink() || mb.file_type().is_symlink() {
            return Ok(false);
        }
        if fs::read(&pa)? != fs::read(&pb)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Collect every file path under `dir`, relative to `root`. Directories
/// are walked, not recorded — an empty dir carries no content, and
/// recording it would make a stray empty dir read as drift.
fn collect_tree(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for e in fs::read_dir(dir)?.filter_map(Result::ok) {
        let p = e.path();
        // symlink_metadata, not metadata: a symlink inside the tree is
        // content in its own right and must not be followed.
        let meta = fs::symlink_metadata(&p)?;
        if meta.is_dir() {
            collect_tree(root, &p, out)?;
        } else if let Ok(rel) = p.strip_prefix(root) {
            out.push(rel.to_path_buf());
        }
    }
    Ok(())
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
// 3b. Desktop plugin packaging.
//
// The desktop app reads a *plugin* — a directory with a manifest that
// bundles skills — not a bare skills dir. So the Copy row's source is
// assembled under build output rather than pointing at the content tree:
//
//   target/desktop-plugin/maid/
//   ├── .claude-plugin/plugin.json
//   └── skills/<name>/…            (one per DESKTOP_SKILLS entry)
//
// Assembling under `target/` keeps the content tree unmutated and gives
// install/status/uninstall one source of truth to compare against.
// ─────────────────────────────────────────────────────────────────

/// Assemble the desktop plugin from the declared skill subset. Returns
/// the skills packaged and those skipped (present in content, not
/// document-shaped), so the caller can report the omission as intent.
fn package_desktop_plugin(checkout: &Path) -> Result<(Vec<String>, Vec<String>)> {
    let content = checkout.join("resources/content/skills");
    let out = checkout.join("target/desktop-plugin/maid");

    // Rebuild from scratch: a skill dropped from DESKTOP_SKILLS must not
    // survive in the packaged output and get copied onward.
    //
    // A root-owned `out` is a reachable state, not a hypothetical: the
    // documented `sudo install-skills desktop` leaves build output owned by
    // root, and every later unprivileged run would fail here. Say what to
    // do rather than surfacing a bare permission error on a path the user
    // never chose.
    if out.exists() {
        fs::remove_dir_all(&out).map_err(|e| {
            if e.kind() == io::ErrorKind::PermissionDenied {
                anyhow!(
                    "cannot rebuild {} ({e}).\n\
                     A previous `sudo` run left build output root-owned. Reclaim it:\n\
                     \x20 sudo rm -rf {}",
                    out.display(),
                    out.display()
                )
            } else {
                anyhow!("clearing {}: {e}", out.display())
            }
        })?;
    }
    fs::create_dir_all(out.join(".claude-plugin"))
        .with_context(|| format!("creating {}", out.display()))?;

    // `name` namespaces the skills in the app (maid:notes), `description`
    // shows in its plugin manager. Kept minimal on purpose — `version` is
    // optional and would need bumping to signal updates, which the
    // content-comparison install already handles. Serialized rather than
    // hand-written: the app silently ignores a manifest it cannot parse.
    let manifest = serde_json::json!({
        "name": "maid",
        "description": "Document-shaped skills from the mAId content tree.",
    });
    fs::write(
        out.join(".claude-plugin/plugin.json"),
        format!("{}\n", serde_json::to_string_pretty(&manifest)?),
    )
    .context("writing plugin manifest")?;

    // Package the declared skills that exist. A declared skill absent
    // from content is reported, not fatal: the content tree is the source
    // of truth for what exists, and a partial content tree is a normal
    // state (a fresh checkout, or a test fixture with a subset).
    let mut packaged = Vec::new();
    let mut absent = Vec::new();
    for skill in DESKTOP_SKILLS {
        let src = content.join(skill);
        if src.join("SKILL.md").is_file() {
            copy_tree(&src, &out.join("skills").join(skill))
                .with_context(|| format!("packaging skill {skill:?}"))?;
            packaged.push((*skill).to_string());
        } else {
            absent.push((*skill).to_string());
        }
    }
    if !absent.is_empty() {
        eprintln!(
            "desktop plugin: declared but not in content: {} (nothing to package)",
            absent.join(", ")
        );
    }

    // Everything in content that is deliberately not document-shaped.
    let mut skipped: Vec<String> = fs::read_dir(&content)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|e| e.path().join("SKILL.md").is_file())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| !DESKTOP_SKILLS.contains(&n.as_str()))
        .collect();
    skipped.sort();
    Ok((packaged, skipped))
}

/// Is the desktop destination writable without elevation? The app's
/// plugin dir is system-wide, so check before doing any work — a
/// half-installed plugin is worse than a clear refusal.
///
/// Probes the nearest existing ancestor *directory*: the leaf usually
/// doesn't exist on a first install, and whatever sits at the leaf may not
/// be a directory at all (a hand-placed file is a case install handles on
/// its own, so it must not be misreported here as a permission problem).
fn desktop_writable(roots: Roots) -> bool {
    let dest = roots.resolve(DESKTOP_PLUGIN_PATH);
    let mut probe = dest.as_path();
    loop {
        if probe.is_dir() {
            // Best available portable check: create and remove a temp
            // entry. `metadata().permissions()` can't see ACLs.
            let canary = probe.join(".maid-write-probe");
            let ok = fs::create_dir(&canary).is_ok();
            if ok {
                let _ = fs::remove_dir(&canary);
            }
            return ok;
        }
        match probe.parent() {
            Some(p) => probe = p,
            None => return false,
        }
    }
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
    /// Validate content and populate each target's destination.
    Install {
        /// Plan without making changes.
        #[arg(long)]
        dry_run: bool,
        /// Replace symlinks that point elsewhere.
        #[arg(long)]
        force: bool,
        /// Scope to one target (claude|kiro|codex|desktop); default all.
        #[arg(long)]
        agent: Option<String>,
    },
    /// Remove install-managed destinations.
    Uninstall {
        /// Plan without making changes.
        #[arg(long)]
        dry_run: bool,
        /// Remove whatever is at the managed path, including foreign
        /// symlinks and non-symlinks. Never removes an unmarked
        /// directory at a Copy destination — that is shared ground.
        #[arg(long)]
        force: bool,
        /// Scope to one target (claude|kiro|codex|desktop); default all.
        #[arg(long)]
        agent: Option<String>,
    },
    /// Report each managed destination's state.
    Status {
        /// Scope to one target (claude|kiro|codex|desktop); default all.
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
        } => cmd_install(Roots::new(&home), &root, dry_run, force, agent.as_deref()),
        Cmd::Uninstall {
            dry_run,
            force,
            agent,
        } => cmd_uninstall(Roots::new(&home), &root, dry_run, force, agent.as_deref()),
        Cmd::Status { agent } => cmd_status(Roots::new(&home), &root, agent.as_deref()),
    }
}

fn cmd_install(
    roots: Roots,
    checkout: &Path,
    dry_run: bool,
    force: bool,
    agent: Option<&str>,
) -> Result<u8> {
    let agent = validate_agent(agent)?;
    let count = check_content(&checkout.join("resources").join("content"))
        .map_err(|errs| anyhow!("Content validation failed:\n{}", errs.join("\n")))?;
    eprintln!("validated {count} content file(s)");

    let mut entries = selected_entries(agent);
    let touches_desktop = entries.iter().any(|(.., a)| *a == "desktop");
    let mut desktop_skipped = false;

    if touches_desktop {
        // Package before planning: the Copy row's source is build output,
        // so it must exist before the destination is compared against it.
        let (packaged, skipped) = package_desktop_plugin(checkout)?;
        eprintln!("desktop plugin: packaged {}", packaged.join(", "));
        if !skipped.is_empty() {
            // Named, not silent — the subset is a declared scoping rule
            // (document-shaped only), so the omission should read as
            // intent rather than as a missing skill.
            eprintln!(
                "desktop plugin: skipped {} (not document-shaped)",
                skipped.join(", ")
            );
        }
        // Soft skip, not an error: the default verb (no selector) includes
        // this row, and its destination is a root-owned system path on a
        // stock machine. Aborting would mean an unprivileged
        // `install-skills` installed nothing at all — including the three
        // targets that need no elevation. Every other refusal in this file
        // is per-row; this matches.
        if !dry_run && !desktop_writable(roots) {
            eprintln!(
                "skip      {} (not writable; needs elevation)\n\
                 \x20         run: sudo just resources::install-skills desktop",
                roots.resolve(DESKTOP_PLUGIN_PATH).display()
            );
            entries.retain(|(.., a)| *a != "desktop");
            desktop_skipped = true;
        }
    }

    let failures: usize = entries
        .iter()
        .map(|e| expand(*e, roots, checkout))
        .collect::<io::Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .map(|(h, s, strat)| match strat {
            Strategy::Copy => install_one_copy(h, s, dry_run, force),
            Strategy::Link | Strategy::FanOut => install_one(h, s, dry_run, force),
        })
        .collect::<io::Result<Vec<bool>>>()?
        .into_iter()
        .filter(|&fail| fail)
        .count();
    Ok(if failures > 0 || desktop_skipped {
        1
    } else {
        0
    })
}

fn cmd_uninstall(
    roots: Roots,
    checkout: &Path,
    dry_run: bool,
    force: bool,
    agent: Option<&str>,
) -> Result<u8> {
    let agent = validate_agent(agent)?;
    let failures: usize = selected_entries(agent)
        .iter()
        .map(|e| expand(*e, roots, checkout))
        .collect::<io::Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .map(|(h, s, strat)| match strat {
            Strategy::Copy => uninstall_one_copy(h, s, dry_run, force),
            Strategy::Link | Strategy::FanOut => uninstall_one(h, s, dry_run, force),
        })
        .collect::<io::Result<Vec<bool>>>()?
        .into_iter()
        .filter(|&fail| fail)
        .count();
    Ok(if failures > 0 { 1 } else { 0 })
}

fn cmd_status(roots: Roots, checkout: &Path, agent: Option<&str>) -> Result<u8> {
    let agent = validate_agent(agent)?;
    let entries = selected_entries(agent);

    // Status is read-only: it never packages. Repackaging here would make
    // a report verb mutate the repo, and — worse — after the documented
    // `sudo install-skills desktop`, build output is root-owned, so an
    // unprivileged status would fail on a write it had no reason to
    // attempt. A desktop row with nothing packaged reports "source
    // missing", which is the honest state.

    for entry in entries {
        for (h, s, strat) in expand(entry, roots, checkout)? {
            // Label by the home path relative to $HOME so fan-out
            // children read as `.codex/skills/<name>`.
            let label = h
                .strip_prefix(roots.home)
                .unwrap_or(&h)
                .display()
                .to_string();
            let plan = plan_for(h.clone(), s, strat)?;
            let state = match plan.cmp {
                Comparison::Match => match strat {
                    Strategy::Copy => format!("ok (copied from {})", plan.source.display()),
                    _ => format!("ok -> {}", plan.source.display()),
                },
                Comparison::Missing => "missing".into(),
                Comparison::SourceMissing => "source missing".into(),
                // The actionable one: install is what refreshes it.
                Comparison::Stale => "STALE (source changed since install; re-run install)".into(),
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

/// Install a copied tree: replace the destination wholesale so a file
/// removed from source doesn't linger. Returns `Ok(true)` on soft skip.
fn install_one_copy(
    home: PathBuf,
    source: PathBuf,
    dry_run: bool,
    force: bool,
) -> io::Result<bool> {
    let plan = plan_one_copy(home, source)?;
    let tag = if dry_run { "(dry-run) " } else { "" };
    let target = plan.home.display();
    match plan.cmp {
        Comparison::Match => {
            println!("{tag}ok        {target}");
            Ok(false)
        }
        Comparison::Missing => {
            if !dry_run {
                copy_tree_owned(&plan.source, &plan.home)?;
            }
            println!("{tag}copied    {target}");
            Ok(false)
        }
        Comparison::Stale => {
            if !dry_run {
                // Remove then copy: an in-place overwrite would leave
                // files deleted from source behind in the destination.
                // Safe because Stale requires our ownership marker.
                fs::remove_dir_all(&plan.home)?;
                copy_tree_owned(&plan.source, &plan.home)?;
            }
            println!("{tag}refreshed {target} (was stale)");
            Ok(false)
        }
        Comparison::WrongTarget(current) if force => {
            if !dry_run {
                fs::remove_file(&plan.home)?;
                copy_tree_owned(&plan.source, &plan.home)?;
            }
            println!(
                "{tag}replaced  {target} (was symlink -> {})",
                current.display()
            );
            Ok(false)
        }
        Comparison::WrongTarget(current) => {
            eprintln!(
                "{tag}skip      {target} (foreign symlink -> {}; use --force to replace)",
                current.display()
            );
            Ok(true)
        }
        Comparison::BlockedByRealFile => {
            eprintln!("{tag}skip      {target} (existing file; not overwriting)");
            Ok(true)
        }
        Comparison::BlockedByRealDir => {
            // A real dir without our ownership marker: another tool's
            // plugin, or a hand-made one. Never overwritten, and not even
            // with --force — this is a shared system directory.
            eprintln!(
                "{tag}skip      {target} (directory is not mAId-managed;                  remove it by hand to let this tool own the path)"
            );
            Ok(true)
        }
        Comparison::SourceMissing => {
            println!("{tag}skip      {target} (source missing — nothing packaged)");
            Ok(false)
        }
    }
}

/// Recursively copy `src` to `dst`, creating parents. Plain files and
/// dirs only — the content tree is markdown, and a symlink inside a
/// copied plugin would defeat the point of copying.
fn copy_tree_owned(src: &Path, dst: &Path) -> io::Result<()> {
    copy_tree(src, dst)?;
    // Stamp ownership last, so a partially-copied tree is never claimed.
    fs::write(
        dst.join(OWNED_MARKER),
        "Written by mAId's build-tool. Removing this makes the directory\n\
         unmanaged: install and uninstall will refuse to touch it.\n",
    )
}

fn copy_tree(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for e in fs::read_dir(src)?.filter_map(Result::ok) {
        let from = e.path();
        let to = dst.join(e.file_name());
        if fs::symlink_metadata(&from)?.is_dir() {
            copy_tree(&from, &to)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
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
        // Symlinks expose edits live, so they never go stale.
        Comparison::Stale => unreachable!("Stale is a Copy-only state"),
    }
}

/// Remove a copied tree. Only removes a directory whose contents we
/// recognise as ours (Match or Stale) — a real dir we didn't put there
/// is left alone, matching the symlink path's refusal.
fn uninstall_one_copy(
    home: PathBuf,
    source: PathBuf,
    dry_run: bool,
    force: bool,
) -> io::Result<bool> {
    let plan = plan_one_copy(home, source)?;
    let tag = if dry_run { "(dry-run) " } else { "" };
    let target = plan.home.display();
    match plan.cmp {
        Comparison::Match | Comparison::Stale => {
            if !dry_run {
                fs::remove_dir_all(&plan.home)?;
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
        Comparison::BlockedByRealFile | Comparison::BlockedByRealDir => {
            eprintln!("{tag}skip          {target} (not mAId-managed — refusing to remove)");
            Ok(true)
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
        Comparison::Stale => unreachable!("Stale is a Copy-only state"),
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
    use std::os::unix::fs::PermissionsExt;
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

    /// Resolve `REGISTRY[0]` (`.claude/skills`, a `Strategy::Link` entry)
    /// to its single (home, source) pair for the per-link plan_one tests.
    fn link0(home: &Path, checkout: &Path) -> (PathBuf, PathBuf) {
        let (h, s, _) = expand(REGISTRY[0], test_roots(home), checkout)
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        (h, s)
    }

    /// Roots for tests: both the home and the absolute-destination root
    /// live inside the same tempdir, so an absolute registry row (the
    /// desktop plugin path) can never touch the real filesystem.
    fn test_roots(home: &Path) -> Roots<'_> {
        Roots { home, system: home }
    }

    /// The desktop (Copy) row's single (destination, source) pair.
    fn copy_row(home: &Path, checkout: &Path) -> (PathBuf, PathBuf) {
        let entry = *REGISTRY
            .iter()
            .find(|(.., a)| *a == "desktop")
            .expect("a desktop row");
        let (h, s, _) = expand(entry, test_roots(home), checkout)
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        (h, s)
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
        cmd_install(test_roots(home.path()), checkout.path(), false, false, None).unwrap();
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
        let rc = cmd_install(test_roots(home.path()), checkout.path(), false, false, None).unwrap();
        assert_eq!(rc, 0);
        // Every managed symlink is the set of expanded links across all
        // entries (fan-out entries contribute one link per source child).
        // Copy rows hold real trees, not links — asserted separately.
        for entry in REGISTRY {
            for (h, s, strat) in expand(*entry, test_roots(home.path()), checkout.path()).unwrap() {
                match strat {
                    Strategy::Copy => assert!(h.is_dir() && trees_match(&s, &h).unwrap()),
                    _ => assert_eq!(fs::read_link(&h).unwrap(), s),
                }
            }
        }
    }

    #[test]
    fn install_second_run_is_idempotent() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        cmd_install(test_roots(home.path()), checkout.path(), false, false, None).unwrap();
        let rc = cmd_install(test_roots(home.path()), checkout.path(), false, false, None).unwrap();
        assert_eq!(rc, 0);
    }

    #[test]
    fn install_dry_run_makes_no_changes() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        cmd_install(test_roots(home.path()), checkout.path(), true, false, None).unwrap();
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

        let rc = cmd_install(test_roots(home.path()), checkout.path(), false, false, None).unwrap();
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

        let rc = cmd_install(test_roots(home.path()), checkout.path(), false, true, None).unwrap();
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

        let rc = cmd_install(test_roots(home.path()), checkout.path(), false, false, None).unwrap();
        assert_eq!(rc, 1);
        assert_eq!(fs::read_to_string(&target).unwrap(), "user content");
    }

    // ── uninstall ───────────────────────────────────────────────

    #[test]
    fn uninstall_clean_home_is_noop() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        assert_eq!(
            cmd_uninstall(test_roots(home.path()), checkout.path(), false, false, None).unwrap(),
            0
        );
    }

    #[test]
    fn uninstall_removes_managed_symlinks() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        cmd_install(test_roots(home.path()), checkout.path(), false, false, None).unwrap();

        let rc =
            cmd_uninstall(test_roots(home.path()), checkout.path(), false, false, None).unwrap();
        assert_eq!(rc, 0);
        for entry in REGISTRY {
            assert!(!path_exists(&home.path().join(entry.0)));
        }
    }

    #[test]
    fn uninstall_idempotent() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        cmd_install(test_roots(home.path()), checkout.path(), false, false, None).unwrap();
        cmd_uninstall(test_roots(home.path()), checkout.path(), false, false, None).unwrap();
        assert_eq!(
            cmd_uninstall(test_roots(home.path()), checkout.path(), false, false, None).unwrap(),
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

        let rc =
            cmd_uninstall(test_roots(home.path()), checkout.path(), false, false, None).unwrap();
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

        let rc =
            cmd_uninstall(test_roots(home.path()), checkout.path(), false, true, None).unwrap();
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

        let rc =
            cmd_uninstall(test_roots(home.path()), checkout.path(), false, false, None).unwrap();
        assert_eq!(rc, 1);
        assert_eq!(fs::read_to_string(&target).unwrap(), "user content");
    }

    #[test]
    fn uninstall_dry_run_makes_no_changes() {
        let checkout = make_checkout_with_skills();
        let home = TempDir::new().unwrap();
        cmd_install(test_roots(home.path()), checkout.path(), false, false, None).unwrap();

        cmd_uninstall(test_roots(home.path()), checkout.path(), true, false, None).unwrap();
        for entry in REGISTRY {
            for (h, _, _) in expand(*entry, test_roots(home.path()), checkout.path()).unwrap() {
                assert!(path_exists(&h));
            }
        }
    }

    // ── status ──────────────────────────────────────────────────

    #[test]
    fn status_runs_clean_on_fresh_install() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        cmd_install(test_roots(home.path()), checkout.path(), false, false, None).unwrap();
        assert_eq!(
            cmd_status(test_roots(home.path()), checkout.path(), None).unwrap(),
            0
        );
    }

    // ── fan-out kind (codex-owned skills dir) ───────────────────

    /// A checkout whose skills source holds two child skills, for
    /// exercising the `Strategy::FanOut` entry (`.codex/skills`).
    fn make_checkout_with_skills() -> TempDir {
        let dir = make_checkout();
        // `kdevkit` is terminal-shaped (excluded from the desktop
        // target); `notes` and `writing-style` are the document-shaped
        // ones DESKTOP_SKILLS declares, so this covers both sides of the
        // packaging split.
        for name in ["kdevkit", "notes", "writing-style"] {
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
            .find(|(h, _, k, _)| *k == Strategy::FanOut && h.contains("codex"))
            .copied()
            .expect("a codex fan-out entry");
        let links = expand(codex_skills, test_roots(home.path()), checkout.path()).unwrap();
        let names: Vec<_> = links
            .iter()
            .map(|(h, ..)| h.file_name().unwrap().to_str().unwrap().to_string())
            .collect();
        // sorted, one per source child
        assert_eq!(names, vec!["kdevkit", "notes", "writing-style"]);
    }

    #[test]
    fn fanout_installs_children_and_preserves_tool_owned_siblings() {
        let checkout = make_checkout_with_skills();
        let home = TempDir::new().unwrap();
        // Codex owns ~/.codex/skills and ships its own .system/ inside.
        let system_marker = home.path().join(".codex/skills/.system/.marker");
        write(&system_marker, "codex-owned");

        let rc = cmd_install(test_roots(home.path()), checkout.path(), false, false, None).unwrap();
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

        cmd_install(test_roots(home.path()), checkout.path(), false, false, None).unwrap();
        let rc =
            cmd_uninstall(test_roots(home.path()), checkout.path(), false, false, None).unwrap();
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
        cmd_install(test_roots(home.path()), checkout.path(), false, false, None).unwrap();
        // Drop "notes" from the source checkout.
        fs::remove_dir_all(checkout.path().join("resources/content/skills/notes")).unwrap();

        let codex = REGISTRY
            .iter()
            .find(|(h, _, k, _)| *k == Strategy::FanOut && h.contains("codex"))
            .copied()
            .unwrap();
        let homes: Vec<_> = expand(codex, test_roots(home.path()), checkout.path())
            .unwrap()
            .into_iter()
            .map(|(h, ..)| h)
            .collect();
        // The orphaned link is still in the managed set.
        assert!(homes.iter().any(|h| h.ends_with("notes")));

        // Uninstall reaps it — no dangling symlink left behind.
        cmd_uninstall(test_roots(home.path()), checkout.path(), false, false, None).unwrap();
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

        let rc =
            cmd_uninstall(test_roots(home.path()), checkout.path(), false, true, None).unwrap();
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
            .find(|(h, _, k, _)| *k == Strategy::FanOut && h.contains("codex"))
            .copied()
            .unwrap();
        assert!(
            expand(codex_skills, test_roots(home.path()), checkout.path())
                .unwrap()
                .is_empty()
        );
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
            cmd_install(test_roots(home.path()), checkout.path(), false, false, None).unwrap(),
            0
        );
        for entry in REGISTRY {
            for (h, _, _) in expand(*entry, test_roots(home.path()), checkout.path()).unwrap() {
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

        assert_eq!(
            cmd_status(test_roots(home.path()), checkout.path(), None).unwrap(),
            0
        );

        assert_eq!(
            cmd_uninstall(test_roots(home.path()), checkout.path(), false, false, None).unwrap(),
            0
        );
        for entry in REGISTRY {
            for (h, _, _) in expand(*entry, test_roots(home.path()), checkout.path()).unwrap() {
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
        let rc = cmd_install(
            test_roots(home.path()),
            checkout.path(),
            false,
            false,
            Some("codex"),
        )
        .unwrap();
        assert_eq!(rc, 0);

        // codex fan-out children exist…
        assert!(path_exists(&home.path().join(".codex/skills/kdevkit")));
        // …while claude and kiro whole-dir links were never made.
        assert!(!path_exists(&home.path().join(".claude/skills")));
        assert!(!path_exists(&home.path().join(".kiro/steering/skills")));
        // …and the desktop copy row was not touched either.
        assert!(!path_exists(
            &test_roots(home.path()).resolve(DESKTOP_PLUGIN_PATH)
        ));
    }

    // ── copy strategy (the desktop target) ──────────────────────

    #[test]
    fn desktop_packages_only_document_shaped_skills() {
        let checkout = make_checkout_with_skills();
        let (packaged, skipped) = package_desktop_plugin(checkout.path()).unwrap();
        assert_eq!(packaged, vec!["notes", "writing-style"]);
        // kdevkit is terminal-shaped: present in content, deliberately out.
        assert_eq!(skipped, vec!["kdevkit"]);

        let plugin = checkout.path().join("target/desktop-plugin/maid");
        assert!(plugin.join(".claude-plugin/plugin.json").is_file());
        assert!(plugin.join("skills/notes/SKILL.md").is_file());
        assert!(plugin.join("skills/writing-style/SKILL.md").is_file());
        assert!(!plugin.join("skills/kdevkit").exists());
    }

    #[test]
    fn desktop_manifest_is_valid_json_naming_the_plugin() {
        let checkout = make_checkout_with_skills();
        package_desktop_plugin(checkout.path()).unwrap();
        let raw = fs::read_to_string(
            checkout
                .path()
                .join("target/desktop-plugin/maid/.claude-plugin/plugin.json"),
        )
        .unwrap();
        // Parsed, not string-matched: an invalid manifest is silently
        // ignored by the app, so the test has to prove it's real JSON.
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["name"], "maid");
        assert!(v["description"].as_str().is_some_and(|d| !d.is_empty()));
    }

    #[test]
    fn desktop_repackaging_drops_a_skill_removed_from_the_subset() {
        // Packaging rebuilds from scratch, so stale output can't survive
        // into the copy and reach the app.
        let checkout = make_checkout_with_skills();
        package_desktop_plugin(checkout.path()).unwrap();
        let stray = checkout
            .path()
            .join("target/desktop-plugin/maid/skills/gone/SKILL.md");
        write(&stray, "---\nname: gone\ndescription: x\n---\n");
        package_desktop_plugin(checkout.path()).unwrap();
        assert!(!stray.exists());
    }

    #[test]
    fn desktop_install_copies_real_files_not_symlinks() {
        // The whole reason Copy exists: the app refuses a symlinked
        // plugin dir, so every installed entry must be a real file.
        let checkout = make_checkout_with_skills();
        let home = TempDir::new().unwrap();
        assert_eq!(
            cmd_install(
                test_roots(home.path()),
                checkout.path(),
                false,
                false,
                Some("desktop")
            )
            .unwrap(),
            0
        );
        let dest = test_roots(home.path()).resolve(DESKTOP_PLUGIN_PATH);
        assert!(!dest.is_symlink());
        let skill = dest.join("skills/notes/SKILL.md");
        assert!(skill.is_file() && !skill.is_symlink());
    }

    #[test]
    fn desktop_install_is_idempotent() {
        let checkout = make_checkout_with_skills();
        let home = TempDir::new().unwrap();
        cmd_install(
            test_roots(home.path()),
            checkout.path(),
            false,
            false,
            Some("desktop"),
        )
        .unwrap();
        let (dest, src) = copy_row(home.path(), checkout.path());
        assert_eq!(
            plan_one_copy(dest.clone(), src.clone()).unwrap().cmp,
            Comparison::Match
        );
        assert_eq!(
            cmd_install(
                test_roots(home.path()),
                checkout.path(),
                false,
                false,
                Some("desktop")
            )
            .unwrap(),
            0
        );
        assert_eq!(plan_one_copy(dest, src).unwrap().cmp, Comparison::Match);
    }

    #[test]
    fn desktop_edited_source_reports_stale_then_refreshes() {
        let checkout = make_checkout_with_skills();
        let home = TempDir::new().unwrap();
        cmd_install(
            test_roots(home.path()),
            checkout.path(),
            false,
            false,
            Some("desktop"),
        )
        .unwrap();

        // Edit the content tree the way an author would.
        write(
            &checkout
                .path()
                .join("resources/content/skills/notes/SKILL.md"),
            "---\nname: notes\ndescription: edited\n---\nnew body.\n",
        );
        package_desktop_plugin(checkout.path()).unwrap();
        let (dest, src) = copy_row(home.path(), checkout.path());
        assert_eq!(
            plan_one_copy(dest.clone(), src.clone()).unwrap().cmp,
            Comparison::Stale,
            "an edited source must read as stale, not ok"
        );

        // Re-install refreshes it, and the new bytes actually land.
        cmd_install(
            test_roots(home.path()),
            checkout.path(),
            false,
            false,
            Some("desktop"),
        )
        .unwrap();
        assert_eq!(
            plan_one_copy(dest.clone(), src).unwrap().cmp,
            Comparison::Match
        );
        assert!(fs::read_to_string(dest.join("skills/notes/SKILL.md"))
            .unwrap()
            .contains("new body."));
    }

    #[test]
    fn desktop_refresh_drops_files_deleted_from_source() {
        // Remove-then-copy, not overwrite-in-place: a file deleted from
        // source must not linger in the destination.
        let checkout = make_checkout_with_skills();
        let home = TempDir::new().unwrap();
        cmd_install(
            test_roots(home.path()),
            checkout.path(),
            false,
            false,
            Some("desktop"),
        )
        .unwrap();
        let dest = test_roots(home.path()).resolve(DESKTOP_PLUGIN_PATH);
        assert!(dest.join("skills/writing-style/SKILL.md").is_file());

        fs::remove_dir_all(
            checkout
                .path()
                .join("resources/content/skills/writing-style"),
        )
        .unwrap();
        cmd_install(
            test_roots(home.path()),
            checkout.path(),
            false,
            false,
            Some("desktop"),
        )
        .unwrap();
        assert!(!dest.join("skills/writing-style").exists());
        assert!(dest.join("skills/notes/SKILL.md").is_file());
    }

    #[test]
    fn desktop_dry_run_makes_no_changes() {
        let checkout = make_checkout_with_skills();
        let home = TempDir::new().unwrap();
        cmd_install(
            test_roots(home.path()),
            checkout.path(),
            true,
            false,
            Some("desktop"),
        )
        .unwrap();
        assert!(!path_exists(
            &test_roots(home.path()).resolve(DESKTOP_PLUGIN_PATH)
        ));
    }

    #[test]
    fn desktop_uninstall_removes_the_tree_and_is_idempotent() {
        let checkout = make_checkout_with_skills();
        let home = TempDir::new().unwrap();
        cmd_install(
            test_roots(home.path()),
            checkout.path(),
            false,
            false,
            Some("desktop"),
        )
        .unwrap();
        let dest = test_roots(home.path()).resolve(DESKTOP_PLUGIN_PATH);
        assert!(dest.is_dir());

        assert_eq!(
            cmd_uninstall(
                test_roots(home.path()),
                checkout.path(),
                false,
                false,
                Some("desktop")
            )
            .unwrap(),
            0
        );
        assert!(!path_exists(&dest));
        // Second run is a clean no-op, not an error.
        assert_eq!(
            cmd_uninstall(
                test_roots(home.path()),
                checkout.path(),
                false,
                false,
                Some("desktop")
            )
            .unwrap(),
            0
        );
    }

    #[test]
    fn desktop_uninstall_preserves_sibling_plugins() {
        // Our destination is one plugin inside a dir the app also lets
        // others populate; uninstall must not reach outside it.
        let checkout = make_checkout_with_skills();
        let home = TempDir::new().unwrap();
        cmd_install(
            test_roots(home.path()),
            checkout.path(),
            false,
            false,
            Some("desktop"),
        )
        .unwrap();
        let sibling = home
            .path()
            .join("Library/Application Support/Claude/org-plugins/someone-else/x.md");
        write(&sibling, "not ours\n");

        cmd_uninstall(
            test_roots(home.path()),
            checkout.path(),
            false,
            false,
            Some("desktop"),
        )
        .unwrap();
        assert!(sibling.is_file(), "a sibling plugin must survive uninstall");
    }

    #[test]
    fn desktop_foreign_symlink_at_destination_is_skipped_without_force() {
        let checkout = make_checkout_with_skills();
        let home = TempDir::new().unwrap();
        let dest = test_roots(home.path()).resolve(DESKTOP_PLUGIN_PATH);
        ensure_parent(&dest).unwrap();
        let elsewhere = home.path().join("elsewhere");
        fs::create_dir_all(&elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, &dest).unwrap();

        // Soft skip (rc 1), and the foreign link is left intact.
        assert_eq!(
            cmd_install(
                test_roots(home.path()),
                checkout.path(),
                false,
                false,
                Some("desktop")
            )
            .unwrap(),
            1
        );
        assert_eq!(fs::read_link(&dest).unwrap(), elsewhere);
    }

    #[test]
    fn desktop_real_file_at_destination_is_preserved() {
        let checkout = make_checkout_with_skills();
        let home = TempDir::new().unwrap();
        let dest = test_roots(home.path()).resolve(DESKTOP_PLUGIN_PATH);
        write(&dest, "hand-written\n");
        assert_eq!(
            cmd_install(
                test_roots(home.path()),
                checkout.path(),
                false,
                false,
                Some("desktop")
            )
            .unwrap(),
            1
        );
        assert_eq!(fs::read_to_string(&dest).unwrap(), "hand-written\n");
    }

    // Regressions for the review findings. Each of these passed with the
    // pre-fix code, which is the point.

    #[test]
    fn desktop_refuses_a_foreign_directory_at_the_destination() {
        // The destination is a shared system dir another tool may populate.
        // Without an ownership marker, "differs from source" is
        // indistinguishable from "someone else's data" — and the pre-fix
        // code deleted it via the Stale path, with no --force.
        let checkout = make_checkout_with_skills();
        let home = TempDir::new().unwrap();
        let dest = test_roots(home.path()).resolve(DESKTOP_PLUGIN_PATH);
        let theirs = dest.join("THEIRS.md");
        write(&theirs, "another tool's plugin\n");

        // Soft skip, data intact — and not even --force may remove it.
        for force in [false, true] {
            assert_eq!(
                cmd_install(
                    test_roots(home.path()),
                    checkout.path(),
                    false,
                    force,
                    Some("desktop")
                )
                .unwrap(),
                1
            );
            assert!(theirs.is_file(), "foreign data deleted (force={force})");
            assert_eq!(
                cmd_uninstall(
                    test_roots(home.path()),
                    checkout.path(),
                    false,
                    force,
                    Some("desktop")
                )
                .unwrap(),
                1
            );
            assert!(theirs.is_file(), "foreign data deleted (force={force})");
        }
    }

    #[test]
    fn unwritable_desktop_dest_still_installs_the_other_targets() {
        // The default verb includes desktop, whose real destination is
        // root-owned. Aborting would install nothing at all — including the
        // three targets needing no elevation.
        let checkout = make_checkout_with_skills();
        let home = TempDir::new().unwrap();
        let sys = TempDir::new().unwrap();
        let roots = Roots {
            home: home.path(),
            system: sys.path(),
        };
        // Make the system root unwritable so the precheck fails.
        let mut perms = fs::metadata(sys.path()).unwrap().permissions();
        perms.set_mode(0o555);
        fs::set_permissions(sys.path(), perms).unwrap();

        let rc = cmd_install(roots, checkout.path(), false, false, None).unwrap();

        // Restore before assertions so TempDir can clean up.
        let mut perms = fs::metadata(sys.path()).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(sys.path(), perms).unwrap();

        assert_eq!(rc, 1, "the skip should be reported in the exit code");
        assert!(
            path_exists(&home.path().join(".claude/skills")),
            "claude must still be installed when only desktop is unwritable"
        );
        assert!(path_exists(&home.path().join(".codex/skills/notes")));
    }

    #[test]
    fn status_does_not_package_or_write() {
        // status is a report verb; it must not mutate the checkout. The
        // pre-fix version repackaged, which also broke unprivileged status
        // once a sudo install had left build output root-owned.
        let checkout = make_checkout_with_skills();
        let home = TempDir::new().unwrap();
        let out = checkout.path().join("target/desktop-plugin");
        assert!(!out.exists());
        cmd_status(test_roots(home.path()), checkout.path(), None).unwrap();
        assert!(!out.exists(), "status packaged build output");
    }

    #[test]
    fn desktop_symlink_inside_the_installed_tree_reads_as_drift() {
        // Copy exists because the app refuses symlinks. A symlinked file
        // inside the destination must not read as ok — fs::read would
        // follow it and report a false match.
        let checkout = make_checkout_with_skills();
        let home = TempDir::new().unwrap();
        cmd_install(
            test_roots(home.path()),
            checkout.path(),
            false,
            false,
            Some("desktop"),
        )
        .unwrap();
        let (dest, src) = copy_row(home.path(), checkout.path());
        let installed = dest.join("skills/notes/SKILL.md");
        let original = checkout
            .path()
            .join("resources/content/skills/notes/SKILL.md");
        fs::remove_file(&installed).unwrap();
        std::os::unix::fs::symlink(&original, &installed).unwrap();

        assert_eq!(
            plan_one_copy(dest, src).unwrap().cmp,
            Comparison::Stale,
            "a symlinked entry must be drift, not a match"
        );
    }

    #[test]
    fn trees_match_detects_extra_and_changed_files() {
        let dir = TempDir::new().unwrap();
        let (a, b) = (dir.path().join("a"), dir.path().join("b"));
        write(&a.join("x/y.md"), "same\n");
        write(&b.join("x/y.md"), "same\n");
        assert!(trees_match(&a, &b).unwrap());

        // A changed byte counts.
        write(&b.join("x/y.md"), "different\n");
        assert!(!trees_match(&a, &b).unwrap());

        // So does an extra file on the destination side only.
        write(&b.join("x/y.md"), "same\n");
        write(&b.join("x/extra.md"), "extra\n");
        assert!(!trees_match(&a, &b).unwrap());
    }

    #[test]
    fn uninstall_scoped_leaves_other_agents_installed() {
        // Install all three, then uninstall only claude: claude's link
        // is gone, kiro's and codex's survive.
        let checkout = make_checkout_with_skills();
        let home = TempDir::new().unwrap();
        cmd_install(test_roots(home.path()), checkout.path(), false, false, None).unwrap();

        let rc = cmd_uninstall(
            test_roots(home.path()),
            checkout.path(),
            false,
            false,
            Some("claude"),
        )
        .unwrap();
        assert_eq!(rc, 0);
        assert!(!path_exists(&home.path().join(".claude/skills")));
        assert!(path_exists(&home.path().join(".kiro/steering/skills")));
        assert!(path_exists(&home.path().join(".codex/skills/kdevkit")));
    }

    #[test]
    fn cmd_install_unknown_agent_errors() {
        let checkout = make_checkout();
        let home = TempDir::new().unwrap();
        assert!(cmd_install(
            test_roots(home.path()),
            checkout.path(),
            false,
            false,
            Some("bogus")
        )
        .is_err());
    }
}
