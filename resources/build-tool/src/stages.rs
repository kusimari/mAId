//! The pipeline, one section per stage. Each stage consumes what the
//! previous one produced:
//!
//!   1 content — resources/content/ → a valid skill
//!   2 check   — that skill, verified in isolation from the CHECKOUT
//!   3 install — a valid skill → $HOME symlinks
//!   4 smoke   — the DEPLOYED skill, verified against everything installed
//!
//! Stages 2 and 4 are the same mechanism (see `harness`) pointed at the
//! two sides of install. Reads `shared` for the registry, agents, and
//! roots; nothing here reaches back into the CLI.

use crate::deploy::Deploy;
use crate::harness::{
    agent_available, check_prompt, control_plan, detect_leak, dump_path, invocation, judge_agent,
    judge_prompt, plan_one, plan_tests, read_verdict, score_reply, snapshot_checkout, test_name,
    Assertion, Authority, Fixture, Kind as TestKind, Outcome, Plan, Reach, Selection, Skipped,
    Stage, Stress, Verdict, KIRO_READONLY_AGENT, KIRO_READONLY_NAME,
};
use crate::harvest;
use crate::install_record::{self, Dirs, Item, Package, Repo, What};
use crate::memory;
use crate::reads;
use crate::shared::{checkout_skill, on_path, usage, Agent, BROWSER_SERVER};
use anyhow::{anyhow, Context, Result};
use ghost::contract::{Slot, Standard};
use shell_claude::config_dir::ConfigDir;
use std::fs;
use std::path::{Path, PathBuf};

// ─────────────────────────────────────────────────────────────────
// Stage 1 · content — resources/content/ → a valid skill.
//
// Only skills are deployed. Each resources/content/skills/<name>/
// SKILL.md needs YAML frontmatter with a non-empty name + description.
// ─────────────────────────────────────────────────────────────────

/// Validate every root skills are authored under (in the checkout,
/// `shared::skill_roots`; after the build, the one profile dir),
/// returning the count of validated files or the joined error list.
/// Caller decides whether to print or abort.
pub fn check_content(roots: &[PathBuf]) -> Result<usize, Vec<String>> {
    let skills: Vec<PathBuf> = roots
        .iter()
        .flat_map(|root| fs::read_dir(root.join("skills")).into_iter().flatten())
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|dir| dir.join("SKILL.md").exists())
        .collect();
    let errs: Vec<String> = twice(&skills)
        .into_iter()
        .chain(skills.iter().flat_map(|dir| {
            check_one_skill(&dir.join("SKILL.md"))
                .err()
                .into_iter()
                .chain(repo_pointers(dir))
        }))
        .chain(
            roots
                .iter()
                .flat_map(|root| manifests(root))
                .filter_map(|(standard, p)| {
                    declaration(standard, &p)
                        .err()
                        .map(|e| format!("{}: {e}", p.display()))
                }),
        )
        .collect();
    match errs.is_empty() {
        true => Ok(skills.len()),
        false => Err(errs),
    }
}

/// A name two roots both have a skill under: the build copies both to one
/// place, so one would end up inside the other.
fn twice(skills: &[PathBuf]) -> Vec<String> {
    let mut by_name: std::collections::BTreeMap<String, Vec<&PathBuf>> = Default::default();
    for dir in skills {
        let name = dir.file_name().unwrap_or_default().to_string_lossy();
        by_name.entry(name.into_owned()).or_default().push(dir);
    }
    by_name
        .into_iter()
        .filter(|(_, dirs)| dirs.len() > 1)
        .map(|(name, dirs)| {
            let at: Vec<String> = dirs.iter().map(|d| d.display().to_string()).collect();
            format!("skill {name} is in more than one root: {}", at.join(", "))
        })
        .collect()
}

/// The bring-in manifests shipped beside the skills: each MCP server's
/// `servers/<name>/server.json`, and each overlay's
/// `overlays/<name>/plugin.json` (a borrowed plugin's declaration).
fn manifests(content_dir: &Path) -> Vec<(Standard, PathBuf)> {
    [
        ("servers", "server.json", Standard::Mcp),
        ("overlays", "plugin.json", Standard::Plugin),
    ]
    .into_iter()
    .flat_map(|(dir, file, standard)| {
        let mut found: Vec<_> = fs::read_dir(content_dir.join(dir))
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|e| e.path().join(file))
            .filter(|p| p.exists())
            .collect();
        found.sort();
        found.into_iter().map(move |p| (standard, p))
    })
    .collect()
}

/// Text that sends an installed session to the mAId repo, which it never
/// has (see project.md "Skill state"). Matched lowercased.
const REPO_POINTERS: [&str; 5] = [
    "resources/content/",
    "ghost-in-shell/",
    "maid checkout",
    "just install",
    "just resources::",
];

/// Every line of every `.md` file in a skill dir, modules included, that
/// names one of `REPO_POINTERS`.
fn repo_pointers(skill_dir: &Path) -> Vec<String> {
    md_files(skill_dir)
        .into_iter()
        .flat_map(|p| match fs::read_to_string(&p) {
            Err(e) => vec![format!("{}: cannot read: {e}", p.display())],
            Ok(text) => text
                .lines()
                .enumerate()
                .filter_map(|(n, line)| {
                    let line = line.to_lowercase();
                    REPO_POINTERS
                        .iter()
                        .find(|pat| line.contains(*pat))
                        .map(|pat| {
                            format!(
                                "{}:{}: points the installed skill at the repo ({pat})",
                                p.display(),
                                n + 1
                            )
                        })
                })
                .collect(),
        })
        .collect()
}

fn md_files(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        // The entry's own type, so a linked dir is not followed into a cycle.
        .flat_map(|e| match e.file_type().is_ok_and(|t| t.is_dir()) {
            true => md_files(&e.path()),
            false => {
                let p = e.path();
                p.extension()
                    .is_some_and(|x| x == "md")
                    .then_some(p)
                    .into_iter()
                    .collect()
            }
        })
        .collect();
    found.sort();
    found
}

/// Validate one SKILL.md: readable, `---`-fenced YAML frontmatter, and
/// `name` + `description` both non-empty. Errors name the file, since a
/// caller reports them as a list across the whole tree.
///
/// Defers to gray_matter for the envelope and serde for the schema — the
/// in-function struct IS the schema.
fn check_one_skill(path: &Path) -> Result<(), String> {
    #[derive(serde::Deserialize)]
    struct Frontmatter {
        name: String,
        description: String,
    }

    let at = |e: String| format!("{}: {e}", path.display());
    let body = fs::read_to_string(path).map_err(|e| at(format!("cannot read: {e}")))?;
    let fm = gray_matter::Matter::<gray_matter::engine::YAML>::new()
        .parse::<Frontmatter>(&body)
        .map_err(|e| at(e.to_string()))?
        .data
        .ok_or_else(|| at("missing or unterminated YAML frontmatter".into()))?;
    if fm.name.trim().is_empty() || fm.description.trim().is_empty() {
        return Err(at("name and description must be non-empty".into()));
    }
    declaration(Standard::Skill, path).map_err(at)
}

/// A unit's bring-in declaration, if it has one, reads, and its rules
/// pointer names a section that is there, so a renamed heading never ships.
fn declaration(standard: Standard, path: &Path) -> Result<(), String> {
    let path = std::path::absolute(path).map_err(|e| e.to_string())?;
    match ghost::contract::read(standard, &path)? {
        Slot::Declared(d) => match &d.rules {
            Some(pointer) => ghost::contract::rules(&path, pointer).map(drop),
            None => Ok(()),
        },
        Slot::Undeclared { .. } => Ok(()),
    }
}

// ─────────────────────────────────────────────────────────────────
// Stage 3 · install — a valid skill → wherever the agents read it.
//
// These three verbs say WHAT should happen and let `deploy` decide HOW.
// They contain no knowledge of $HOME layout, symlinks, or any agent's
// directory conventions; swapping in an agent's own install command
// replaces the `Deploy` impl and leaves this section untouched.
// ─────────────────────────────────────────────────────────────────

/// Validate content, failing with every error found.
pub fn cmd_validate(roots: &[PathBuf]) -> Result<u8> {
    let count = check_content(roots)
        .map_err(|errs| anyhow!("Content validation failed:\n{}", errs.join("\n")))?;
    eprintln!("validated {count} content file(s)");
    Ok(0)
}

/// Validate the content about to be deployed, then deploy it.
pub fn cmd_install(
    target: &impl Deploy,
    content_dir: &Path,
    parts: &(impl Dirs + ?Sized),
    dry_run: bool,
    force: bool,
    agent: Option<Agent>,
) -> Result<u8> {
    if !content_dir.join("skills").is_dir() {
        return Err(anyhow!(
            "no skills at {} — build the profile first (just install)",
            content_dir.display()
        ));
    }
    cmd_validate(std::slice::from_ref(&content_dir.to_path_buf()))?;
    // Recorded even when install stops partway: the list says what is there.
    let done = target.install(agent, dry_run, force);
    let recorded = match dry_run {
        true => Ok(()),
        false => record_placed(target, parts, agent, |_| true),
    };
    let rc = outcome(
        done.inspect_err(|_| warn_unrecorded(&recorded))?,
        dry_run,
        false,
    )?;
    recorded.map(|()| rc)
}

pub fn cmd_uninstall(
    target: &impl Deploy,
    parts: &(impl Dirs + ?Sized),
    dry_run: bool,
    force: bool,
    agent: Option<Agent>,
) -> Result<u8> {
    let done = target.uninstall(agent, dry_run, force);
    // What uninstall did not reach is still there; a skip is not.
    let recorded = match dry_run {
        true => Ok(()),
        false => record_placed(target, parts, agent, |i| i.skipped.is_none()),
    };
    let rc = outcome(
        done.inspect_err(|_| warn_unrecorded(&recorded))?,
        dry_run,
        true,
    )?;
    recorded.map(|()| rc)
}

/// The step's own error is returned; a part-list failure beside it is
/// still said.
fn warn_unrecorded(recorded: &Result<()>) {
    if let Err(e) = recorded {
        eprintln!("build-tool: install record not updated: {e:#}");
    }
}

/// What `Deploy` places; the browser MCP is recorded by its own script.
const DEPLOYED_KINDS: &[What] = &[What::Skill, What::Plugin, What::Marketplace];

/// Write what the target reports placed into the install record, for each
/// selected agent.
fn record_placed(
    target: &impl Deploy,
    parts: &(impl Dirs + ?Sized),
    agent: Option<Agent>,
    keep: impl Fn(&Item) -> bool,
) -> Result<()> {
    let placed = target.placed(agent)?;
    for &a in Agent::ALL
        .iter()
        .filter(|a| agent.is_none_or(|sel| sel == **a))
    {
        let items = placed
            .iter()
            .filter(|(g, i)| *g == a && keep(i))
            .map(|(_, i)| i.clone())
            .collect();
        install_record::record(parts, a, DEPLOYED_KINDS, None, items)?;
    }
    Ok(())
}

/// Record one kind of entry on one agent in the install record, for a step
/// outside `Deploy` (the browser MCP). No names clears that kind.
pub fn cmd_record(
    parts: &(impl Dirs + ?Sized),
    agent: Agent,
    kind: What,
    scope: Option<&str>,
    skipped: Option<&str>,
    only: Option<&str>,
    names: &[String],
) -> Result<u8> {
    // Each setting is owned by the step that writes it, with its value.
    if kind == What::Setting {
        return Err(usage("a setting is recorded by the step that sets it"));
    }
    let items = names
        .iter()
        .map(|n| Item {
            scope: scope.map(str::to_string),
            skipped: skipped.map(str::to_string),
            ..Item::placed(kind, n)
        })
        .collect();
    match only {
        Some(name) if names.iter().any(|n| n != name) => {
            return Err(usage(format!("--only {name} records that name only")))
        }
        Some(name) => install_record::record_named(parts, agent, kind, scope, name, items)?,
        None => install_record::record(parts, agent, &[kind], scope, items)?,
    }
    Ok(0)
}

// ─────────────────────────────────────────────────────────────────
// The package — what install records of mAId as a whole, and where each
// placed thing's files are and where it came from.
// ─────────────────────────────────────────────────────────────────

/// Record mAId as a package in its install record: this repo at its
/// revision, where its files and config are, and each entry's files and
/// source. See project.md "Plain install".
pub fn cmd_install_package(
    parts: &(impl Dirs + ?Sized),
    profile: &Path,
    config: &Path,
    checkout: Checkout,
    dry_run: bool,
) -> Result<u8> {
    if dry_run {
        return Ok(0);
    }
    let root = profile.join(crate::shared::PROFILE_CONTENT_DIR);
    let pensieve = checkout.pensieve.clone();
    install_record::record_package(
        parts,
        Package {
            source: Some(project_source(checkout)),
            root: Some(root.clone()),
            config: Some(config.to_path_buf()),
        },
    )?;
    place(parts, &root, pensieve.as_ref())?;
    Ok(0)
}

/// Name each placed entry's files and, when not this repo, its source:
/// mAId's plugin and skills under the profile, its servers' dirs, the
/// vendor's plugins at their pinned revision, the memory server's package
/// at the flake's pin. Rerun after the browser step records its server.
pub fn place(parts: &(impl Dirs + ?Sized), root: &Path, pensieve: Option<&Repo>) -> Result<()> {
    install_record::amend(parts, |i| {
        if i.skipped.is_none() {
            facts(i, root, pensieve);
        }
    })
}

fn facts(i: &mut Item, root: &Path, pensieve: Option<&Repo>) {
    let dir = |rel: &str| Some(root.join(rel)).filter(|d| d.is_dir());
    let vendor = crate::shared::BRING_INS.iter().find(|v| match i.kind {
        What::Marketplace => i.name == v.marketplace,
        What::Plugin => v.plugins.iter().any(|p| i.name == v.plugin_id(p)),
        _ => false,
    });
    if let Some(v) = vendor {
        i.source = Some(Repo {
            repo: Some(v.source.into()),
            rev: Some(v.rev.into()),
        });
        return;
    }
    match (i.kind, i.name.as_str()) {
        (What::Plugin, crate::shared::PLUGIN_ID) => i.at = dir("marketplace/plugins/maid"),
        (What::Skill, n) => i.at = dir(&format!("skills/{n}")),
        (What::Mcp, BROWSER_SERVER) => i.at = dir(&format!("servers/{BROWSER_SERVER}")),
        (What::Mcp, memory::SERVER) => {
            i.at = dir(&format!("servers/{}", memory::SERVER));
            i.source = pensieve.cloned();
        }
        _ => {}
    }
}

/// What install reads from the checkout it runs from: its `origin` URL, its
/// revision with `-dirty` when the tree has changes, and the memory
/// package's pin in `flake.lock`. Read there, not from the profile, so the
/// profile stays addressed by its content.
#[derive(Debug, Default)]
pub struct Checkout {
    pub origin: Option<String>,
    pub head: Option<String>,
    pub dirty: bool,
    /// The memory package as the checkout's `flake.lock` pins it.
    pub pensieve: Option<Repo>,
}

impl Checkout {
    /// Read through the `git` that PATH finds by rule 8. With no git, or
    /// outside a repo, the git fields are empty; the lock needs no git.
    pub fn read(dir: &Path) -> Checkout {
        let pensieve = locked_pensieve(dir);
        let Some(git) = crate::shared::find_on(std::env::var_os("PATH").as_deref(), "git") else {
            return Checkout {
                pensieve,
                ..Checkout::default()
            };
        };
        let run = |args: &[&str]| {
            std::process::Command::new(&git)
                .arg("-C")
                .arg(dir)
                .args(args)
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
        };
        Checkout {
            origin: run(&["remote", "get-url", "origin"]).filter(|s| !s.is_empty()),
            head: run(&["rev-parse", "HEAD"]).filter(|s| !s.is_empty()),
            // Untracked files are not built, so they do not make it dirty;
            // and status must not take the index lock, even on a dry run.
            dirty: run(&[
                "--no-optional-locks",
                "status",
                "--porcelain",
                "--untracked-files=no",
            ])
            .is_some_and(|s| !s.is_empty()),
            pensieve,
        }
    }
}

/// The memory package as the `flake.lock` in `dir` pins it.
fn locked_pensieve(dir: &Path) -> Option<Repo> {
    pinned(
        &fs::read_to_string(dir.join("flake.lock")).ok()?,
        "pensieve",
    )
}

/// An input `flake.lock` pins on GitHub, as a project source. Any other pin
/// (a path on this machine, say) gives none, so no local path is recorded.
fn pinned(lock: &str, input: &str) -> Option<Repo> {
    let lock: serde_json::Value = serde_json::from_str(lock).ok()?;
    // The root's inputs name each input's node, which may differ from it.
    let root = lock["root"].as_str().unwrap_or("root");
    let node = lock["nodes"][root]["inputs"][input]
        .as_str()
        .unwrap_or(input);
    let locked = &lock["nodes"][node]["locked"];
    let field = |k: &str| locked[k].as_str().filter(|v| !v.is_empty());
    if field("type")? != "github" {
        return None;
    }
    Some(Repo {
        repo: Some(format!(
            "https://github.com/{}/{}",
            field("owner")?,
            field("repo")?
        )),
        rev: Some(field("rev")?.to_string()),
    })
}

/// This repo as a source.
fn project_source(c: Checkout) -> Repo {
    Repo {
        repo: c.origin.and_then(|u| remote(&u)),
        rev: c.head.map(|h| match c.dirty {
            true => format!("{h}-dirty"),
            false => h,
        }),
    }
}

/// A remote as the install record may hold it: a URL or `host:path`, with any
/// user or token taken out. A path on this machine is left out, since
/// nothing installed names a checkout.
fn remote(url: &str) -> Option<String> {
    if let Some((scheme, rest)) = url.split_once("://") {
        if scheme == "file" {
            return None;
        }
        // A host holds no `@`, so everything up to the last one is user
        // or token, even one with a `/` in it.
        let rest = rest.rsplit_once('@').map_or(rest, |(_, r)| r);
        return Some(format!("{scheme}://{rest}"));
    }
    // scp-like `[user@]host:path`: a host before the first ':' with no '/'.
    let (host, path) = url.split_once(':')?;
    let host = host.rsplit_once('@').map_or(host, |(_, h)| h);
    (!host.is_empty() && !host.contains('/') && !host.starts_with('~') && !host.starts_with('.'))
        .then(|| format!("{host}:{path}"))
}

// ─────────────────────────────────────────────────────────────────
// Memory — the personal store's server, Claude's own memory, harvest.
// ─────────────────────────────────────────────────────────────────

/// The command that serves the store over MCP stdio, as the memory's
/// `serve` value: the same command install registers on claude.
fn serve(profile: &Path, store: &Path) -> String {
    serde_json::json!([
        launcher(profile).display().to_string(),
        "--store",
        store.display().to_string()
    ])
    .to_string()
}

/// The profile's memory server.
fn launcher(profile: &Path) -> PathBuf {
    profile.join("bin/pensieve")
}

/// Install's claude memory step: register `personal-memory` on the
/// personal store, turn Claude's own automatic memory off, and take away
/// the memory dir an older mAId set. Each is recorded in the install record.
pub fn cmd_install_memory(
    claude: &ConfigDir,
    store_file: &Path,
    profile: &Path,
    parts: &(impl Dirs + ?Sized),
    dry_run: bool,
    agent: Option<Agent>,
) -> Result<u8> {
    if agent.is_some_and(|a| a != Agent::Claude) {
        return Ok(0);
    }
    let cli_present = on_path(Agent::Claude.cli());
    let store = memory::store(store_file)?;
    let launcher = launcher(profile);
    let plan = memory::ServerPlan {
        launcher: &launcher,
        store: store.as_deref(),
        claimed: install_record::placed_by_others(parts, Agent::Claude, What::Mcp, memory::SERVER),
        cli_present,
    };
    let memory::ServerOutcome {
        out: server,
        failed,
    } = memory::install_server(claude, &plan, dry_run)?;
    println!("{}", server.line);
    let registered = server.item.as_ref().is_some_and(|i| i.skipped.is_none());
    let off = memory::install_off(claude, &ownership(parts)?, registered, cli_present, dry_run)?;
    println!("{}", off.line);
    let legacy = legacy_setting(claude, parts, dry_run)?;
    if !dry_run {
        // mAId owns only what it recorded, so an unrecorded change is the user's to undo.
        let unrecorded =
            |what: &str| format!("claude {what} may be set but is not recorded; check it by hand");
        // The settings the server was bound with.
        let item = server.item.map(|mut i| {
            if let (None, Some(store)) = (&i.skipped, &store) {
                i.values = [
                    ("store".to_string(), store.display().to_string()),
                    ("index".to_string(), memory::INDEX.to_string()),
                    ("serve".to_string(), serve(profile, store)),
                ]
                .into();
            }
            i
        });
        install_record::record_named(
            parts,
            Agent::Claude,
            What::Mcp,
            None,
            memory::SERVER,
            item.into_iter().collect(),
        )
        .with_context(|| unrecorded(memory::SERVER))?;
        install_record::record_setting(parts, Agent::Claude, memory::SETTING, off.item)
            .with_context(|| unrecorded(memory::SETTING))?;
        if legacy {
            install_record::record_setting(parts, Agent::Claude, memory::LEGACY_SETTING, None)?;
        }
    }
    Ok(u8::from(failed))
}

pub fn cmd_uninstall_memory(
    claude: &ConfigDir,
    profile: &Path,
    parts: &(impl Dirs + ?Sized),
    dry_run: bool,
    agent: Option<Agent>,
) -> Result<u8> {
    if agent.is_some_and(|a| a != Agent::Claude) {
        return Ok(0);
    }
    let memory::ServerOutcome {
        out: server,
        failed,
    } = memory::uninstall_server(
        claude,
        &launcher(profile),
        install_record::placed_by_others(parts, Agent::Claude, What::Mcp, memory::SERVER),
        on_path(Agent::Claude.cli()),
        dry_run,
    )?;
    println!("{}", server.line);
    let off = memory::uninstall_off(claude, &ownership(parts)?, dry_run)?;
    println!("{}", off.line);
    let legacy = legacy_setting(claude, parts, dry_run)?;
    if !dry_run {
        install_record::record_named(
            parts,
            Agent::Claude,
            What::Mcp,
            None,
            memory::SERVER,
            server.item.into_iter().collect(),
        )?;
        install_record::record_setting(parts, Agent::Claude, memory::SETTING, None)?;
        if legacy {
            install_record::record_setting(parts, Agent::Claude, memory::LEGACY_SETTING, None)?;
        }
    }
    Ok(u8::from(failed))
}

/// Remove the memory dir an older mAId set, while it is the value mAId
/// recorded. Whether the install record holds an entry for it to clear: one
/// recorded as skipped has no value and nothing to remove, but still goes.
fn legacy_setting(claude: &ConfigDir, parts: &(impl Dirs + ?Sized), dry_run: bool) -> Result<bool> {
    let Some(item) =
        install_record::recorded(parts, Agent::Claude, What::Setting, memory::LEGACY_SETTING)?
    else {
        return Ok(false);
    };
    if let Some(line) = memory::remove_legacy(claude, item.value.as_deref(), dry_run)? {
        println!("{line}");
    }
    Ok(true)
}

/// Install's claude step for reading mAId's skill files and the user's
/// skill state: grant mAId's plugin dir and config dir, and record each
/// element.
pub fn cmd_install_reads(
    claude: &ConfigDir,
    profile: &Path,
    config: &Path,
    parts: &(impl Dirs + ?Sized),
    dry_run: bool,
    agent: Option<Agent>,
) -> Result<u8> {
    if agent.is_some_and(|a| a != Agent::Claude) {
        return Ok(0);
    }
    for grant in [reads::plugin_grant(profile), reads::config_grant(config)] {
        let setting = grant.list.setting();
        let out = reads::install_claude(
            claude,
            &grant,
            &recorded_settings(parts, setting)?,
            &install_record::claimed_by_others(parts, Agent::Claude, setting),
            on_path(Agent::Claude.cli()),
            dry_run,
        )?;
        out.lines.iter().for_each(|l| println!("{l}"));
        if !dry_run {
            install_record::record_named(parts, Agent::Claude, What::Setting, None, setting, out.items)
                .with_context(|| {
                    format!(
                        "claude {setting} may hold mAId's elements but they are not recorded; check it by hand"
                    )
                })?;
        }
    }
    Ok(0)
}

/// Uninstall's claude step for skill reads and skill state: remove mAId's
/// recorded elements only.
pub fn cmd_uninstall_reads(
    claude: &ConfigDir,
    parts: &(impl Dirs + ?Sized),
    dry_run: bool,
    agent: Option<Agent>,
) -> Result<u8> {
    if agent.is_some_and(|a| a != Agent::Claude) {
        return Ok(0);
    }
    for list in reads::List::ALL {
        let setting = list.setting();
        let own = recorded_settings(parts, setting)?;
        reads::uninstall_claude(claude, list, &own, dry_run)?
            .iter()
            .for_each(|l| println!("{l}"));
        if !dry_run {
            install_record::record_setting(parts, Agent::Claude, setting, None)?;
        }
    }
    Ok(0)
}

/// Every element of the list setting `name` mAId's install record records
/// placed on claude.
fn recorded_settings(parts: &(impl Dirs + ?Sized), name: &str) -> Result<Vec<Item>> {
    Ok(install_record::placed(parts, Agent::Claude)?
        .into_iter()
        .filter(|i| i.kind == What::Setting && i.name == name)
        .collect())
}

/// The setting `name` as mAId's install record records it placed on claude.
fn recorded_setting(parts: &(impl Dirs + ?Sized), name: &str) -> Result<Option<Item>> {
    Ok(install_record::placed(parts, Agent::Claude)?
        .into_iter()
        .find(|i| i.kind == What::Setting && i.name == name))
}

/// What mAId owns of Claude's automatic-memory setting: what its install record
/// says it set or adopted.
fn ownership(parts: &(impl Dirs + ?Sized)) -> Result<memory::Ownership> {
    let item = recorded_setting(parts, memory::SETTING)?;
    Ok(memory::Ownership {
        adopted: item.as_ref().is_some_and(|i| i.adopted),
        recorded: item.and_then(|i| i.value),
        claimed: install_record::claimed_by_others(parts, Agent::Claude, memory::SETTING),
    })
}

pub fn cmd_memory_status(
    claude: &ConfigDir,
    store_file: &Path,
    profile: &Path,
    parts: &(impl Dirs + ?Sized),
) -> Result<u8> {
    let store = memory::store(store_file)?;
    memory::status(
        claude,
        store.as_deref(),
        &launcher(profile),
        &ownership(parts)?,
    )?
    .iter()
    .for_each(|l| println!("{l}"));
    Ok(0)
}

/// Copy each harness's native memory into `<to>/memory/`. Refuses, before
/// writing anything, a `to` that is not a directory.
pub fn cmd_harvest(
    claude: &ConfigDir,
    codex_home: &Path,
    to: &Path,
    project: Option<&str>,
) -> Result<u8> {
    if !to.is_dir() {
        return Err(usage(format!(
            "{} is not a directory; clone the store first",
            to.display()
        )));
    }
    println!("harvest  into {}", to.join("memory").display());
    harvest::harvest(claude, codex_home, to, project)?
        .lines()
        .iter()
        .for_each(|l| println!("{l}"));
    Ok(0)
}

pub fn cmd_status(target: &impl Deploy, agent: Option<Agent>) -> Result<u8> {
    for report in target.status(agent)? {
        println!("{:<28} {}", report.label, report.state.describe());
    }
    Ok(0)
}

/// Print what happened at each location, and exit non-zero when any was
/// left alone because acting would have destroyed something.
fn outcome(reports: Vec<crate::deploy::Report>, dry_run: bool, removing: bool) -> Result<u8> {
    let mut skipped = 0usize;
    for report in &reports {
        let (line, skip) = outcome_line(report, dry_run, removing);
        skipped += usize::from(skip);
        println!("{line}");
    }
    Ok(if skipped > 0 { 1 } else { 0 })
}

/// One location's line, and whether it was left alone in a way that fails
/// the run.
fn outcome_line(report: &crate::deploy::Report, dry_run: bool, removing: bool) -> (String, bool) {
    use crate::deploy::State;
    let tag = if dry_run { "(dry-run) " } else { "" };
    let mut skipped = 0usize;
    let at = &report.label;
    // Whether force applied is now decided by the shim (Report::acted),
    // not re-derived here — that re-derivation is exactly how "install
    // --force over a real file" once reported a deletion that never
    // happened: create() correctly refuses Occupied even with force,
    // but a (state, force) guess here assumed force always acted.
    let line = match (&report.state, report.acted) {
        (State::Ok(_) | State::Missing, _) => format!("{tag}done          {at}"),
        (State::SourceMissing, _) => format!("{tag}skip          {at} (source missing)"),
        (State::Wrong { found, .. }, true) => {
            format!("{tag}replaced      {at} (was {})", found.display())
        }
        (State::Wrong { found, .. }, false) => {
            skipped += 1;
            format!(
                "{tag}skip          {at} (points at {}; --force to replace)",
                found.display()
            )
        }
        (State::Occupied("dir"), _) => {
            skipped += 1;
            format!("{tag}skip          {at} (real dir; not ours — refusing)")
        }
        (State::Occupied(what), true) => format!("{tag}removed       {at} (was {what})"),
        (State::Occupied(what), false) => {
            skipped += 1;
            format!("{tag}skip          {at} (existing {what}; --force to replace)")
        }
        (State::Legacy(found), _) => {
            format!("{tag}removed       {at} (old link to {})", found.display())
        }
        (State::NoCli(cli), _) => format!("{tag}skip          {at} ({cli} not on PATH)"),
        // The user's own skill in the agent's dir: expected, not a failure.
        (State::Theirs, _) => format!("{tag}skip          {at} (not mAId's; kept)"),
        (State::Unreadable(why), _) => {
            skipped += 1;
            format!("{tag}skip          {at} (unreadable: {why})")
        }
        (State::Failed(why), _) => {
            skipped += 1;
            format!("{tag}FAILED        {at} ({why})")
        }
        (State::InUse, _) => {
            format!("{tag}skip          {at} (in use by a plugin not mAId's; kept)")
        }
        (State::Current(_) | State::Stale { .. } | State::Disabled { .. }, true) if removing => {
            format!("{tag}removed       {at}")
        }
        (State::Stale { found, .. }, true) => format!("{tag}updated       {at} (was {found})"),
        (State::Disabled { found, .. }, true) => {
            format!("{tag}updated       {at} (was {found}; still disabled)")
        }
        (State::Disabled { found, want }, false) if found != want => {
            format!("{tag}skip          {at} (disabled, not updated; enable it, then just install)")
        }
        (State::Disabled { .. }, false) => format!("{tag}done          {at} (disabled)"),
        (State::Current(_) | State::Stale { .. }, _) => format!("{tag}done          {at}"),
    };
    (line, skipped > 0)
}

// ─────────────────────────────────────────────────────────────────
// Stage 2 · check — pre-install verification.
//
// The three explicit kinds, reading each skill from the CHECKOUT. No
// install, no $HOME: a content change is provable before it is made live
// for every session on the machine.
//
// Stage 4 · smoke — post-install verification.
//
// The two implicit kinds, where the agent must find the skill among
// everything deployed. Requires install to have run.
// ─────────────────────────────────────────────────────────────────

/// Run one verification stage. `dry_run` constructs and structurally
/// checks every prompt without calling an agent, which costs nothing.
/// How a sweep runs, beyond what it selects.
#[derive(Clone, Copy, Debug)]
pub struct RunOptions<'a> {
    pub dry_run: bool,
    pub stress: Option<Stress<'a>>,
    /// Runs per test. Above 1, each test also prints a `TALLY` line.
    pub repeat: usize,
    /// Add a control run, without the skill, beside each enact test.
    pub control: bool,
}

pub fn cmd_verify(
    target: &impl Deploy,
    checkout: &Path,
    selection: &Selection,
    opts: RunOptions,
) -> Result<u8> {
    let RunOptions {
        dry_run,
        stress: stressed,
        repeat,
        control,
    } = opts;
    let repeat = repeat.max(1);
    // Under `verify`, a --kind naming only the other stage's kinds leaves
    // this stage with nothing to do. That is a scoped-away no-op, not a
    // failure — the other stage runs the tests.
    if selection.kinds.is_empty() {
        println!("no {} kinds selected", selection.stage);
        return Ok(0);
    }

    let (fixtures, broken) = load_fixtures(checkout, selection)?;
    if fixtures.is_empty() && broken.is_empty() {
        // A typo in the fixture selector is a bad invocation, exactly
        // like a bad --agent or --kind value — it must exit 2, not 1.
        return Err(usage(format!(
            "no fixtures matched{}",
            selection
                .fixture
                .as_deref()
                .map(|f| format!(" selector {f:?}"))
                .unwrap_or_default()
        )));
    }

    // Smoke reads the deployed tree, so say so plainly rather than
    // failing every test with a confusing path error.
    if selection.stage == Stage::Smoke && !dry_run {
        require_installed(target, selection)?;
    }

    let judge = (!dry_run)
        .then(|| {
            let available: Vec<Agent> = Agent::ALL
                .iter()
                .copied()
                .filter(|a| agent_available(*a))
                .collect();
            judge_agent(std::env::var("VERIFY_JUDGE").ok().as_deref(), &available)
        })
        .transpose()?;

    let before = snapshot_checkout(checkout);
    // Each unparseable fixture is one failure, reported before any test
    // runs so it is visible at the top rather than buried mid-sweep.
    let mut failures = broken.len();
    for why in &broken {
        report("fixture", &Outcome::Fail(why.clone()));
    }
    // The generated kinds are per-skill; several fixtures share a skill.
    let mut claimed = Vec::new();

    for fixture in &fixtures {
        // The skill's text, read once per fixture. An explicit prompt
        // carries it inline, so a check test installs nothing; an implicit
        // one needs it only to know whether a marker may be asserted.
        let body = match fs::read_to_string(checkout_skill(checkout, &fixture.skill)) {
            Ok(body) => body,
            Err(e) => {
                report(
                    "fixture",
                    &Outcome::Fail(format!("cannot read {}: {e}", fixture.skill)),
                );
                failures += 1;
                continue;
            }
        };

        for (kind, agent) in plan_tests(fixture, selection, &mut claimed) {
            // Decide everything first (pure), then do it (effectful).
            let plan = match plan_one(fixture, kind, agent, &body, stressed) {
                Ok(plan) => plan,
                Err(Skipped(why)) => {
                    report(&test_name_for(fixture, kind, agent), &Outcome::Skip(why));
                    continue;
                }
            };
            let plans = std::iter::once(plan.clone())
                .chain(control.then(|| control_plan(&plan, fixture)).flatten());
            for plan in plans {
                let mut passed = 0;
                for run in 1..=repeat {
                    let outcome = match dry_run {
                        true => check_plan(&plan),
                        false => execute(&plan, judge),
                    };
                    let name = match repeat {
                        1 => plan.name.clone(),
                        _ => format!("{} [run {run}/{repeat}]", plan.name),
                    };
                    report(&name, &outcome);
                    match outcome {
                        Outcome::Fail(_) => failures += 1,
                        Outcome::Pass(_) => passed += 1,
                        Outcome::Skip(_) => {}
                    }
                }
                if repeat > 1 {
                    println!("TALLY {}: {passed}/{repeat}", plan.name);
                }
            }
        }
    }

    // Behavioral tests can escape their scratch dir; catch it here rather
    // than leaving it to be found in git log later.
    if let (Some(before), Some(after)) = (before, snapshot_checkout(checkout)) {
        let leaked = detect_leak(&before, &after);
        if !leaked.is_empty() {
            eprintln!(
                "\x1b[31mLEAK\x1b[0m a test wrote into the checkout (not its scratch workdir):"
            );
            for line in &leaked {
                eprintln!("  {line}");
            }
            eprintln!("  also check git log — a leaked close-notes verb can COMMIT these.");
            failures += 1;
        }
    }

    println!();
    match failures {
        0 => {
            println!("all tests passed");
            Ok(0)
        }
        n => {
            println!("{n} test(s) failed");
            Ok(1)
        }
    }
}

fn report(name: &str, outcome: &Outcome) {
    let colour = match outcome {
        Outcome::Pass(_) => "32",
        Outcome::Fail(_) => "31",
        Outcome::Skip(_) => "33",
    };
    let line = format!("\x1b[{colour}m{}\x1b[0m {name}", outcome.token());
    match outcome {
        Outcome::Fail(_) => eprintln!("{line}"),
        _ => println!("{line}"),
    }
    if !outcome.detail().is_empty() {
        println!("  {}", outcome.detail());
    }
}

/// Read every `.smoke` under `resources/tests/skills/`, in name order so
/// two runs are diffable.
fn load_fixtures(checkout: &Path, selection: &Selection) -> Result<(Vec<Fixture>, Vec<String>)> {
    let dir = checkout.join("resources/tests/skills");
    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
        .with_context(|| format!("reading fixtures from {}", dir.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "smoke"))
        .collect();
    paths.sort();

    // A malformed fixture fails itself and the run continues, as bash did:
    // aborting the sweep would let one typo cost forty minutes of paid
    // agent time and return no results at all.
    let (parsed, broken): (Vec<_>, Vec<_>) = paths
        .iter()
        .filter_map(|p| {
            let name = p.file_stem()?.to_string_lossy().to_string();
            selection.covers(&name).then_some((name, p))
        })
        .map(|(name, path)| {
            fs::read_to_string(path)
                .with_context(|| format!("reading {}", path.display()))
                .and_then(|body| Fixture::parse(&name, &body))
        })
        .partition(Result::is_ok);

    Ok((
        parsed.into_iter().map(Result::unwrap).collect(),
        broken
            .into_iter()
            .map(|e| format!("{:#}", e.unwrap_err()))
            .collect(),
    ))
}

/// Stop early when the smoke stage's precondition is unmet, naming the
/// verb that fixes it.
fn require_installed(target: &impl Deploy, selection: &Selection) -> Result<()> {
    let missing: Vec<&str> = selection
        .agents
        .iter()
        .filter(|a| !target.is_deployed(**a))
        .map(|a| a.name())
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    Err(anyhow!(
        "smoke needs the skills deployed, but {} has no skills tree.\n\
         Run `just install` first, or use `check` for the \
         pre-install kinds (activation, playback, enact) which need no install.",
        missing.join(", ")
    ))
}

/// Name a test that was skipped before its plan existed.
fn test_name_for(fixture: &Fixture, kind: TestKind, agent: Agent) -> String {
    test_name(
        match kind.announce_only() {
            true => &fixture.skill,
            false => &fixture.name,
        },
        kind,
        agent,
    )
}

/// Check a plan structurally, without calling an agent. Free.
fn check_plan(plan: &Plan) -> Outcome {
    // A control run carries no skill, so it is checked as an implicit
    // prompt is: it must not leak the skill's name, path or marker.
    let kind = match plan.control {
        true => TestKind::Integration,
        false => plan.kind,
    };
    if plan.control {
        return match check_prompt(kind, &plan.skill, &plan.prompt) {
            Ok(()) => Outcome::Pass("dry: control carries no skill".into()),
            Err(e) => Outcome::Fail(format!("dry: {e}")),
        };
    }
    match check_prompt(kind, &plan.skill, &plan.prompt) {
        Ok(()) => Outcome::Pass(match plan.kind.reach() {
            Reach::Explicit => "dry: carries the skill".into(),
            Reach::Implicit => "dry: names no skill".into(),
        }),
        Err(e) => Outcome::Fail(format!("dry: {e}")),
    }
}

/// Run a plan against its agent and score the result.
fn execute(plan: &Plan, judge: Option<Agent>) -> Outcome {
    if !agent_available(plan.agent) {
        return Outcome::Fail(format!("{} required but not on PATH", plan.agent.name()));
    }
    match &plan.assertion {
        Assertion::Behavioral { setup, assert } => run_behavioral(plan, setup, assert),
        Assertion::Reply {
            substr,
            narrative,
            seed,
        } => run_reply(
            plan,
            substr.as_deref(),
            narrative.as_deref(),
            seed.as_deref(),
            judge,
        ),
    }
}

/// Drive an agent read-only and score its reply.
fn run_reply(
    plan: &Plan,
    substr: Option<&str>,
    narrative: Option<&str>,
    seed: Option<&str>,
    judge: Option<Agent>,
) -> Outcome {
    let (agent, prompt, label) = (plan.agent, plan.prompt.as_str(), plan.name.as_str());
    // A scratch cwd, so a relative path in the prompt cannot reach the
    // checkout the runner was started from.
    let Ok(scratch) = tempfile::TempDir::new() else {
        return Outcome::Fail("could not create scratch dir".into());
    };
    if let Some(seed) = seed {
        if let Err(e) = shell(seed, scratch.path()) {
            return Outcome::Fail(format!("setup failed: {e}"));
        }
    }
    let reply = match drive(agent, prompt, Authority::ReadOnly, Some(scratch.path())) {
        Ok(r) => r,
        Err(e) => return Outcome::Fail(format!("{} invocation failed: {e}", agent.name())),
    };

    // Bash printed a 200-char head on both common reply failures; without
    // it the two most frequent outcomes give the reader nothing to look
    // at, while the rarer unparseable case gets a whole file.
    let head: String = reply.chars().take(200).collect();
    let mut dump = None;
    let verdict = match (narrative, judge) {
        (Some(want), Some(judge)) => {
            let jp = judge_prompt(prompt, &reply, want);
            match drive(judge, &jp, Authority::ReadOnly, Some(scratch.path())) {
                Ok(raw) => {
                    let verdict = read_verdict(&raw);
                    if verdict == Verdict::Unparseable {
                        let path = dump_path("verify-judge", label);
                        let _ = fs::write(&path, &raw);
                        dump = Some(path);
                    }
                    Some(verdict)
                }
                Err(e) => return Outcome::Fail(format!("judge invocation failed: {e}")),
            }
        }
        (Some(_), None) => return Outcome::Fail("no judge available".into()),
        (None, _) => None,
    };
    match score_reply(&reply, substr, verdict.as_ref(), dump.as_deref()) {
        Outcome::Fail(why) => Outcome::Fail(format!("{why}\n  reply head: {head}...")),
        other => other,
    }
}

/// Seed a scratch workdir, run the agent inside it, then run the fixture's
/// assert shell against the result. Tests execution, not recall.
///
/// The assert runs under `set -x` so a failure names the exact check that
/// broke: asserts are silent `test`/`grep -q` under `set -e`, so without
/// the trace every behavioral failure read "assert failed" with nothing
/// after it — undiagnosable without hand-reproducing the fixture.
fn run_behavioral(plan: &Plan, setup: &str, assert: &str) -> Outcome {
    let (agent, prompt, label) = (plan.agent, plan.prompt.as_str(), plan.name.as_str());
    let Ok(work) = tempfile::TempDir::new() else {
        return Outcome::Fail("could not create scratch workdir".into());
    };
    let dir = work.path();
    if let Err(e) = shell(setup, dir) {
        return Outcome::Fail(format!("setup failed: {e}"));
    }

    // The agent's own failure is not fatal: the assert decides, since a
    // non-zero exit with correct artefacts is still a pass.
    let reply = drive(agent, prompt, Authority::Workdir, Some(dir)).unwrap_or_default();

    match shell(&format!("set -x\n{assert}"), dir) {
        Ok(_) => Outcome::Pass("behavioral".into()),
        Err(trace) => {
            let culprit = trace
                .lines()
                .rfind(|l| l.starts_with('+'))
                .map(|l| l.trim_start_matches('+').trim())
                .unwrap_or("<no trace>")
                .to_string();
            // The scratch dir is removed on drop, so the tree is the only
            // record of what the agent actually created — often the
            // fastest way to see a file landed one directory up.
            let tree = shell("find . -not -path '*/.git/*' | head -60", dir).unwrap_or_default();
            let dump = dump_path("verify-behavioral", label);
            let _ = fs::write(
                &dump,
                format!(
                    "=== assert trace ===\n{trace}\n\
                     === agent reply ===\n{reply}\n\
                     === workdir tree ===\n{tree}\n"
                ),
            );
            Outcome::Fail(format!(
                "failed check: {culprit} (trace + reply → {})",
                dump.display()
            ))
        }
    }
}

/// Run a shell fragment in `dir`, returning its combined output on
/// failure. `set -e` so a fixture's first broken command stops it.
fn shell(script: &str, dir: &Path) -> std::result::Result<String, String> {
    let out = std::process::Command::new("bash")
        .arg("-c")
        .arg(format!("set -e\n{script}"))
        .current_dir(dir)
        .output()
        .map_err(|e| e.to_string())?;
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    match out.status.success() {
        true => Ok(combined),
        false => Err(combined),
    }
}

/// Invoke a coding agent and return its REPLY — not the transcript.
fn drive(
    agent: Agent,
    prompt: &str,
    authority: Authority,
    workdir: Option<&Path>,
) -> Result<String> {
    let last = tempfile::NamedTempFile::new().context("creating reply file")?;
    let inv = invocation(agent, prompt, authority, workdir, last.path());
    if let (Agent::Kiro, Authority::ReadOnly, Some(dir)) = (agent, authority, workdir) {
        let agents = dir.join(".kiro/agents");
        fs::create_dir_all(&agents).context("creating kiro agent dir")?;
        fs::write(
            agents.join(format!("{KIRO_READONLY_NAME}.json")),
            KIRO_READONLY_AGENT,
        )
        .context("writing kiro read-only profile")?;
    }

    let mut cmd = std::process::Command::new(&inv.program);
    cmd.args(&inv.args).stdin(std::process::Stdio::null());
    if let Some(dir) = &inv.cwd {
        cmd.current_dir(dir);
    }
    let out = cmd
        .output()
        .with_context(|| format!("running {}", inv.program))?;

    // An agent that writes its final message to a file: read that, and
    // never the transcript on stdout.
    let reply = match &inv.reply_file {
        Some(path) => fs::read_to_string(path).context("reading agent reply file")?,
        None => format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    };

    // A non-zero exit is the agent failing, not the skill: an expired
    // token or a rate limit would otherwise be scored as a reply and
    // reported as "response missing <marker>", sending the reader to
    // debug content that is fine.
    if !out.status.success() {
        return Err(anyhow!(
            "exited {} — {}",
            out.status
                .code()
                .map_or("by signal".into(), |c| c.to_string()),
            reply.trim().lines().next_back().unwrap_or("no output")
        ));
    }
    Ok(reply)
}

// ─────────────────────────────────────────────────────────────────
// Tests.
// ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Validate frontmatter through the real file path, as production does.
    fn frontmatter_of(body: &str) -> Result<(), String> {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("SKILL.md");
        fs::write(&p, body).unwrap();
        check_one_skill(&p)
    }

    fn write(p: &Path, s: &str) {
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, s).unwrap();
    }

    // ── install / uninstall lines ───────────────────────────────

    /// The plugin lines a user reads, and which ones fail the run.
    #[test]
    fn plugin_outcome_lines_say_what_happened() {
        use crate::deploy::{Report, State};
        let line = |state: State, acted: bool, removing: bool| {
            let r = Report {
                label: "codex plugin maid@maid".into(),
                state,
                acted,
            };
            outcome_line(&r, false, removing)
        };
        let dis = |found: &str| State::Disabled {
            found: found.into(),
            want: "b".into(),
        };
        for (got, want, fails) in [
            (line(dis("a"), true, false), "updated       codex plugin maid@maid (was a; still disabled)", false),
            (line(dis("a"), false, false), "skip          codex plugin maid@maid (disabled, not updated; enable it, then just install)", false),
            (line(dis("b"), false, false), "done          codex plugin maid@maid (disabled)", false),
            (line(dis("a"), true, true), "removed       codex plugin maid@maid", false),
            (line(State::Stale { found: "a".into(), want: "b".into() }, true, false), "updated       codex plugin maid@maid (was a)", false),
            (line(State::Stale { found: "a".into(), want: "b".into() }, true, true), "removed       codex plugin maid@maid", false),
            (line(State::NoCli("codex"), false, false), "skip          codex plugin maid@maid (codex not on PATH)", false),
            (line(State::Unreadable("refused".into()), false, false), "skip          codex plugin maid@maid (unreadable: refused)", true),
            (line(State::Legacy("/p/share/maid/skills".into()), true, false), "removed       codex plugin maid@maid (old link to /p/share/maid/skills)", false),
            (line(State::Theirs, false, true), "skip          codex plugin maid@maid (not mAId's; kept)", false),
        ] {
            assert_eq!(got, (want.to_string(), fails));
        }
    }

    // ── content checks ──────────────────────────────────────────
    #[test]
    fn check_content_empty_root_ok() {
        let dir = TempDir::new().unwrap();
        assert_eq!(check_content(&[dir.path().to_path_buf()]).unwrap(), 0);
    }

    #[test]
    fn check_content_skill_with_frontmatter_ok() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("skills/foo/SKILL.md"),
            "---\nname: foo\ndescription: bar\n---\nbody.\n",
        );
        assert_eq!(check_content(&[dir.path().to_path_buf()]).unwrap(), 1);
    }

    #[test]
    fn one_skill_name_under_two_roots_is_refused() {
        let (a, b) = (TempDir::new().unwrap(), TempDir::new().unwrap());
        for dir in [&a, &b] {
            write(
                &dir.path().join("skills/notes/SKILL.md"),
                "---\nname: notes\ndescription: capture notes\n---\nbody.\n",
            );
        }
        let roots = [a.path().to_path_buf(), b.path().to_path_buf()];
        let errs = check_content(&roots).unwrap_err();
        assert_eq!(errs.len(), 1, "{errs:?}");
        assert!(
            errs[0].starts_with("skill notes is in more than one root: "),
            "{}",
            errs[0]
        );
        assert_eq!(check_content(&roots[..1]).unwrap(), 1);
    }

    /// A profile with one valid skill, its home, and a parts dir.
    fn installable() -> (TempDir, crate::deploy::Symlinks, PathBuf) {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("profile/share/maid/skills/notes/SKILL.md"),
            "---\nname: notes\ndescription: d\n---\nbody.\n",
        );
        let target = crate::deploy::Symlinks {
            home: dir.path().join("home"),
            source: dir.path().join("profile"),
        };
        let parts = dir.path().join("parts");
        (dir, target, parts)
    }

    fn content(dir: &TempDir) -> PathBuf {
        dir.path().join("profile/share/maid")
    }

    #[test]
    fn install_writes_the_part_list_and_uninstall_removes_it() {
        let (dir, target, parts) = installable();
        cmd_install(&target, &content(&dir), &parts, false, false, None).unwrap();
        let text = fs::read_to_string(install_record::path(&parts)).unwrap();
        for harness in ["kiro-cli", "agy"] {
            assert!(text.contains(&format!("\"{harness}\"")), "{text}");
        }
        assert!(text.contains(r#""name": "notes""#), "{text}");
        cmd_uninstall(&target, &parts, false, false, None).unwrap();
        assert!(!install_record::path(&parts).exists());
    }

    /// Installs into kiro, then fails before the rest.
    struct Partway;

    impl Deploy for Partway {
        fn install(
            &self,
            _: Option<Agent>,
            _: bool,
            _: bool,
        ) -> Result<Vec<crate::deploy::Report>> {
            Err(anyhow!("codex plugin install failed"))
        }
        fn uninstall(
            &self,
            _: Option<Agent>,
            _: bool,
            _: bool,
        ) -> Result<Vec<crate::deploy::Report>> {
            unreachable!()
        }
        fn status(&self, _: Option<Agent>) -> Result<Vec<crate::deploy::Report>> {
            unreachable!()
        }
        fn is_deployed(&self, _: Agent) -> bool {
            unreachable!()
        }
        fn placed(&self, _: Option<Agent>) -> Result<Vec<(Agent, Item)>> {
            Ok(vec![(Agent::Kiro, Item::placed(What::Skill, "notes"))])
        }
    }

    #[test]
    fn an_install_that_stops_partway_records_what_it_placed() {
        let (dir, _, parts) = installable();
        let err = cmd_install(&Partway, &content(&dir), &parts, false, false, None).unwrap_err();
        assert!(err.to_string().contains("codex"), "{err}");
        let text = fs::read_to_string(install_record::path(&parts)).unwrap();
        assert!(text.contains(r#""name": "notes""#), "{text}");
    }

    #[test]
    fn a_dry_run_writes_no_part_list() {
        let (dir, target, parts) = installable();
        cmd_install(&target, &content(&dir), &parts, true, false, None).unwrap();
        assert!(!parts.exists());
    }

    #[test]
    fn record_refuses_a_setting_and_writes_nothing() {
        let dir = TempDir::new().unwrap();
        let parts = dir.path().join("parts");
        let err = cmd_record(
            &parts,
            Agent::Claude,
            What::Setting,
            None,
            None,
            None,
            &["k".into()],
        )
        .unwrap_err();
        assert!(
            err.downcast_ref::<crate::shared::UsageError>().is_some(),
            "{err}"
        );
        assert!(!parts.exists());
    }

    #[test]
    fn record_only_replaces_that_name_and_keeps_the_kinds_others() {
        let dir = TempDir::new().unwrap();
        let parts = dir.path().join("parts");
        let rec = |only, names: &[&str]| {
            let names: Vec<String> = names.iter().map(|n| n.to_string()).collect();
            cmd_record(&parts, Agent::Claude, What::Mcp, None, None, only, &names)
        };
        rec(Some("personal-memory"), &["personal-memory"]).unwrap();
        rec(Some("chrome-devtools"), &["chrome-devtools"]).unwrap();
        rec(Some("chrome-devtools"), &[]).unwrap();
        let names: Vec<_> = install_record::placed(&parts, Agent::Claude)
            .unwrap()
            .into_iter()
            .map(|i| i.name)
            .collect();
        assert_eq!(names, ["personal-memory"]);
        assert!(rec(Some("a"), &["b"]).is_err());
    }

    #[test]
    fn uninstall_drops_a_skip_but_keeps_what_is_still_there() {
        let (dir, target, parts) = installable();
        cmd_record(
            &parts,
            Agent::Kiro,
            What::Mcp,
            Some("s"),
            None,
            None,
            &["m".into()],
        )
        .unwrap();
        // agy's skills dir is the user's, so install skips it.
        write(&dir.path().join("home/.gemini/config/skills/x"), "mine");
        cmd_install(&target, &content(&dir), &parts, false, false, None).unwrap();
        assert!(fs::read_to_string(install_record::path(&parts))
            .unwrap()
            .contains("skipped"));
        cmd_uninstall(&target, &parts, false, false, None).unwrap();
        let text = fs::read_to_string(install_record::path(&parts)).unwrap();
        assert!(
            !text.contains("skipped") && !text.contains("notes"),
            "{text}"
        );
        assert!(text.contains(r#""name": "m""#), "{text}");
    }

    /// A missing tree validates as zero files, so without the guard an
    /// install with no profile would link every agent at nothing.
    #[test]
    fn install_without_a_profile_names_the_fix() {
        let dir = TempDir::new().unwrap();
        let err = cmd_install(
            &crate::deploy::NoDeploy,
            dir.path(),
            dir.path(),
            false,
            false,
            None,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("just install"), "{err}");
    }

    #[test]
    fn check_content_skill_missing_description_rejected() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("skills/foo/SKILL.md"),
            "---\nname: foo\n---\n",
        );
        let errs = check_content(&[dir.path().to_path_buf()]).unwrap_err();
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
        let errs = check_content(&[dir.path().to_path_buf()]).unwrap_err();
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
        assert_eq!(check_content(&[dir.path().to_path_buf()]).unwrap(), 1);
    }

    #[test]
    fn check_content_flags_repo_pointers_in_skill_md_and_modules() {
        let dir = TempDir::new().unwrap();
        let skill = dir.path().join("skills/foo");
        write(
            &skill.join("SKILL.md"),
            "---\nname: foo\ndescription: bar\n---\nok\nIn a mAId Checkout, edit it.\n",
        );
        write(
            &skill.join("phases/a.md"),
            "x\nsee resources/content/skills/foo\n",
        );
        write(&skill.join("b.md"), "then run `just install`\n");
        write(
            &skill.join("c.md"),
            "or `just resources::browser-mcp-allow`\n",
        );
        write(&skill.join("notes.txt"), "just install\n");
        let errs = check_content(&[dir.path().to_path_buf()]).unwrap_err();
        let at = |f: &str, n: usize| format!("{}:{n}:", skill.join(f).display());
        assert_eq!(errs.len(), 4, "{errs:?}");
        for (e, (f, n, pat)) in errs.iter().zip([
            ("SKILL.md", 6, "maid checkout"),
            ("b.md", 1, "just install"),
            ("c.md", 1, "just resources::"),
            ("phases/a.md", 2, "resources/content/"),
        ]) {
            assert!(e.starts_with(&at(f, n)) && e.contains(pat), "{e}");
        }
    }

    #[test]
    fn check_content_passes_a_skill_naming_no_repo_path() {
        let dir = TempDir::new().unwrap();
        let skill = dir.path().join("skills/foo");
        write(
            &skill.join("SKILL.md"),
            "---\nname: foo\ndescription: bar\n---\nInstall the content, then check out a branch.\n",
        );
        write(&skill.join("phases/a.md"), "the user's own config\n");
        // A linked dir is not walked, so a cycle ends.
        std::os::unix::fs::symlink(&skill, skill.join("phases/loop")).unwrap();
        assert_eq!(check_content(&[dir.path().to_path_buf()]).unwrap(), 1);
    }

    // ── frontmatter (gray_matter + serde) ───────────────────────

    #[test]
    fn frontmatter_minimal_ok() {
        assert!(frontmatter_of("---\nname: foo\ndescription: bar\n---\nbody.\n").is_ok());
    }

    #[test]
    fn frontmatter_with_extra_fields_ok() {
        // Real YAML: extra fields are ignored by serde when not in the struct.
        assert!(frontmatter_of(
            "---\nname: foo\ndescription: bar\nversion: 1.0.0\ntags: [a, b]\n---\nbody.\n"
        )
        .is_ok());
    }

    #[test]
    fn frontmatter_quoted_values_ok() {
        // Real YAML handles quoting natively — no hand-rolled unquote.
        assert!(frontmatter_of("---\nname: \"foo\"\ndescription: 'bar baz'\n---\n").is_ok());
    }

    #[test]
    fn frontmatter_missing_fence_rejected() {
        let e = frontmatter_of("name: foo\ndescription: bar\n").unwrap_err();
        assert!(e.contains("frontmatter"));
    }

    #[test]
    fn frontmatter_unterminated_rejected() {
        let e = frontmatter_of("---\nname: foo\ndescription: bar\n").unwrap_err();
        assert!(e.contains("frontmatter"));
    }

    #[test]
    fn frontmatter_missing_name_rejected() {
        let e = frontmatter_of("---\ndescription: bar\n---\n").unwrap_err();
        assert!(e.contains("name"));
    }

    #[test]
    fn frontmatter_missing_description_rejected() {
        let e = frontmatter_of("---\nname: foo\n---\n").unwrap_err();
        assert!(e.contains("description"));
    }

    #[test]
    fn a_shipped_server_or_overlay_must_declare_readably() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("servers/s/server.json"),
            r#"{"name":"s","_meta":{"my-harness":{"contract":1,"scope":"turn","job":"j"}}}"#,
        );
        write(
            &dir.path().join("overlays/o/plugin.json"),
            r#"{"name":"o"}"#,
        );
        assert_eq!(check_content(&[dir.path().to_path_buf()]).unwrap(), 0);
        write(
            &dir.path().join("servers/s/server.json"),
            r#"{"name":"s","_meta":{"my-harness":{"contract":1,"scope":"turn","job":"j","rules":"README.md#x"}}}"#,
        );
        let errs = check_content(&[dir.path().to_path_buf()]).unwrap_err();
        assert!(errs[0].contains("cannot read"), "{errs:?}");
        write(&dir.path().join("servers/s/README.md"), "## X\n\nfirst\n");
        assert_eq!(check_content(&[dir.path().to_path_buf()]).unwrap(), 0);
        write(&dir.path().join("overlays/o/plugin.json"), "not json");
        let errs = check_content(&[dir.path().to_path_buf()]).unwrap_err();
        assert!(
            errs[0].contains("overlays/o/plugin.json") && errs[0].contains("not JSON"),
            "{errs:?}"
        );
    }

    #[test]
    fn a_declaration_must_read_and_its_rules_resolve() {
        let decl = |rules: &str| {
            format!("---\nname: foo\ndescription: bar\nmetadata:\n  my-harness.contract: \"1\"\n  my-harness.scope: turn\n  my-harness.job: j\n  my-harness.rules: {rules}\n---\n## Rules\n\nfirst\n")
        };
        assert!(frontmatter_of(&decl("SKILL.md#rules")).is_ok());
        let e = frontmatter_of(&decl("SKILL.md#renamed")).unwrap_err();
        assert!(e.contains("no heading"), "{e}");
        let e = frontmatter_of("---\nname: foo\ndescription: bar\nmetadata:\n  my-harness.contract: \"1\"\n  my-harness.scope: always\n  my-harness.job: j\n---\n").unwrap_err();
        assert!(e.contains("declaration"), "{e}");
    }

    /// A malformed fixture fails itself; the sweep continues. Bash did
    /// this, and aborting would let one typo cost a whole paid run.
    #[test]
    fn a_malformed_fixture_does_not_abort_the_others() {
        let dir = TempDir::new().unwrap();
        let skills = dir.path().join("resources/tests/skills");
        fs::create_dir_all(&skills).unwrap();
        fs::write(
            skills.join("good.smoke"),
            "skill: notes\n--- enact ---\ntask: t\nexpect: n\n",
        )
        .unwrap();
        // No skill: field — unparseable.
        fs::write(
            skills.join("bad.smoke"),
            "--- enact ---\ntask: t\nexpect: n\n",
        )
        .unwrap();

        let selection = Selection::resolve(Stage::Check, None, None, None).unwrap();
        let (parsed, broken) = load_fixtures(dir.path(), &selection).unwrap();

        assert_eq!(parsed.len(), 1, "the good fixture must still load");
        assert_eq!(parsed[0].name, "good");
        assert_eq!(broken.len(), 1, "the bad one is reported, not fatal");
        assert!(broken[0].contains("bad"), "{}", broken[0]);
    }

    /// The selector still scopes which fixtures are read at all.
    #[test]
    fn load_fixtures_honours_the_fixture_selector() {
        let dir = TempDir::new().unwrap();
        let skills = dir.path().join("resources/tests/skills");
        fs::create_dir_all(&skills).unwrap();
        for name in ["a", "b"] {
            fs::write(
                skills.join(format!("{name}.smoke")),
                "skill: notes\n--- enact ---\ntask: t\nexpect: n\n",
            )
            .unwrap();
        }
        let selection = Selection::resolve(Stage::Check, Some("a"), None, None).unwrap();
        let (parsed, broken) = load_fixtures(dir.path(), &selection).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "a");
        assert!(broken.is_empty());
    }

    /// A fixture-name typo is a bad invocation exactly like a bad --agent
    /// or --kind, and must exit 2 — not 1, which is reserved for "the run
    /// happened and a test failed".
    #[test]
    fn an_unmatched_fixture_selector_is_a_usage_error() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("resources/tests/skills")).unwrap();
        write(
            &dir.path().join("resources/tests/skills/real.smoke"),
            "skill: notes\n--- enact ---\ntask: t\nexpect: n\n",
        );
        let selection = Selection::resolve(Stage::Check, Some("typo-of-real"), None, None).unwrap();
        let err = cmd_verify(
            &crate::deploy::NoDeploy,
            dir.path(),
            &selection,
            RunOptions {
                dry_run: true,
                stress: None,
                repeat: 1,
                control: false,
            },
        )
        .unwrap_err();
        assert!(
            err.downcast_ref::<crate::shared::UsageError>().is_some(),
            "{err}"
        );
        assert!(err.to_string().contains("typo-of-real"));
    }

    // ── the package ──────────────────────────────────────────────

    fn pensieve() -> Repo {
        Repo {
            repo: Some("https://github.com/me/pensieve".into()),
            rev: Some("p1".into()),
        }
    }

    /// A profile laid out as the flake lays it out.
    fn profile() -> TempDir {
        let p = TempDir::new().unwrap();
        let root = p.path().join("share/maid");
        for d in [
            "skills/notes",
            "marketplace/plugins/maid",
            "servers/chrome-devtools",
            "servers/personal-memory",
        ] {
            fs::create_dir_all(root.join(d)).unwrap();
        }
        p
    }

    fn json(parts: &Path) -> serde_json::Value {
        serde_json::from_str(&fs::read_to_string(install_record::path(parts)).unwrap()).unwrap()
    }

    #[test]
    fn install_records_the_package_and_where_each_thing_came_from() {
        let (p, parts, config) = (profile(), TempDir::new().unwrap(), Path::new("/c/maid"));
        let on = |agent, items: Vec<Item>| {
            for i in items {
                install_record::record_named(
                    parts.path(),
                    agent,
                    i.kind,
                    None,
                    &i.name.clone(),
                    vec![i],
                )
                .unwrap();
            }
        };
        on(
            Agent::Claude,
            vec![
                Item::placed(What::Plugin, "maid@maid"),
                Item::placed(What::Plugin, "document-skills@anthropic-agent-skills"),
                Item::placed(What::Marketplace, "anthropic-agent-skills"),
                Item::placed(What::Mcp, memory::SERVER),
                Item::placed(What::Mcp, BROWSER_SERVER),
            ],
        );
        on(
            Agent::Kiro,
            vec![
                Item::placed(What::Skill, "notes"),
                Item::skipped(What::Skill, "gone", "why"),
            ],
        );
        let checkout = Checkout {
            origin: Some("git@github.com:me/mAId.git".into()),
            head: Some("abc".into()),
            dirty: false,
            pensieve: Some(pensieve()),
        };
        cmd_install_package(parts.path(), p.path(), config, checkout, false).unwrap();
        let m = json(parts.path());
        let root = p.path().join("share/maid");
        assert_eq!(
            m["source"],
            serde_json::json!({"repo": "github.com:me/mAId.git", "rev": "abc"})
        );
        assert_eq!(m["root"], root.display().to_string());
        assert_eq!(m["config"], "/c/maid");
        assert_eq!(m["installed"], ghost::time::today());
        let entry = |agent: &str, name: &str| {
            m["agents"][agent]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"] == name)
                .unwrap()
                .clone()
        };
        assert_eq!(
            entry("claude", "maid@maid")["at"],
            root.join("marketplace/plugins/maid").display().to_string()
        );
        assert_eq!(
            entry("claude", "maid@maid")["source"],
            serde_json::Value::Null,
            "this repo's own"
        );
        let vendor = &crate::shared::BRING_INS[0];
        for name in [
            "document-skills@anthropic-agent-skills",
            "anthropic-agent-skills",
        ] {
            assert_eq!(
                entry("claude", name)["source"],
                serde_json::json!({"repo": vendor.source, "rev": vendor.rev})
            );
        }
        let mem = entry("claude", memory::SERVER);
        assert_eq!(
            mem["at"],
            root.join("servers/personal-memory").display().to_string()
        );
        assert_eq!(mem["source"]["rev"], "p1");
        assert_eq!(
            entry("claude", BROWSER_SERVER)["at"],
            root.join("servers/chrome-devtools").display().to_string()
        );
        assert_eq!(
            entry("kiro-cli", "notes")["at"],
            root.join("skills/notes").display().to_string()
        );
        assert_eq!(entry("kiro-cli", "gone")["at"], serde_json::Value::Null);
        let text = m.to_string();
        for ghostly in [
            "\"persona\"",
            "\"bring_ins\"",
            "\"tenure\"",
            "\"route\"",
            "\"unit\"",
        ] {
            assert!(!text.contains(ghostly), "{ghostly} in the record: {text}");
        }
    }

    #[test]
    fn a_dry_run_or_nothing_placed_writes_no_package() {
        let (p, parts) = (profile(), TempDir::new().unwrap());
        cmd_install_package(
            parts.path(),
            p.path(),
            Path::new("/c"),
            Checkout::default(),
            false,
        )
        .unwrap();
        assert!(!install_record::path(parts.path()).exists());
        install_record::record(
            parts.path(),
            Agent::Agy,
            &[What::Skill],
            None,
            vec![Item::placed(What::Skill, "notes")],
        )
        .unwrap();
        let before = fs::read(install_record::path(parts.path())).unwrap();
        cmd_install_package(
            parts.path(),
            p.path(),
            Path::new("/c"),
            Checkout::default(),
            true,
        )
        .unwrap();
        assert_eq!(
            fs::read(install_record::path(parts.path())).unwrap(),
            before
        );
    }

    #[test]
    fn the_lock_gives_a_github_pin_as_its_source_and_nothing_else() {
        let lock = |locked: serde_json::Value| {
            serde_json::json!({"nodes": {"pensieve": {"locked": locked}}}).to_string()
        };
        let github = lock(serde_json::json!(
            {"type": "github", "owner": "me", "repo": "pensieve", "rev": "p1"}
        ));
        assert_eq!(pinned(&github, "pensieve"), Some(pensieve()));
        let path = lock(serde_json::json!({"type": "path", "path": "/home/me/pensieve"}));
        assert_eq!(
            pinned(&path, "pensieve"),
            None,
            "a local path is never recorded"
        );
        assert_eq!(pinned(&github, "other"), None);
        assert_eq!(pinned("not json", "pensieve"), None);
        let renamed = serde_json::json!({"root": "root", "nodes": {
            "root": {"inputs": {"pensieve": "pensieve_2"}},
            "pensieve": {"locked": {"type": "path", "path": "/elsewhere"}},
            "pensieve_2": {"locked":
                {"type": "github", "owner": "me", "repo": "pensieve", "rev": "p1"}}}});
        assert_eq!(
            pinned(&renamed.to_string(), "pensieve"),
            Some(pensieve()),
            "the root's inputs name the node"
        );
    }

    #[test]
    fn the_checkouts_lock_is_read_from_its_dir() {
        let dir = TempDir::new().unwrap();
        assert_eq!(locked_pensieve(dir.path()), None);
        let lock = serde_json::json!({"nodes": {"pensieve": {"locked":
            {"type": "github", "owner": "me", "repo": "pensieve", "rev": "p1"}}}});
        fs::write(dir.path().join("flake.lock"), lock.to_string()).unwrap();
        assert_eq!(locked_pensieve(dir.path()), Some(pensieve()));
    }

    #[test]
    fn this_repo_pins_the_memory_package_at_its_published_commit() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let lock = fs::read_to_string(root.join("flake.lock")).unwrap();
        let got = pinned(&lock, "pensieve").expect("pensieve pinned on GitHub");
        assert_eq!(
            got.repo.as_deref(),
            Some("https://github.com/kusimari/pensieve")
        );
        let rev = got.rev.unwrap();
        assert!(
            rev.len() == 40 && rev.chars().all(|c| c.is_ascii_hexdigit()),
            "{rev}"
        );
    }

    #[test]
    fn the_shipped_memory_declaration_names_its_package_store_and_index() {
        use ghost::contract::{read, Scope, Slot};
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../content");
        let path = root
            .join(memory::MANIFEST.trim_start_matches("share/maid/"))
            .canonicalize()
            .unwrap();
        let Ok(Slot::Declared(d)) = read(Standard::Mcp, &path) else {
            panic!("{path:?} holds no declaration");
        };
        assert_eq!(d.scope, Scope::Session);
        assert_eq!(d.package.as_deref(), Some("pensieve"));
        let keys: Vec<_> = d.config.keys().map(String::as_str).collect();
        assert_eq!(keys, ["index", "serve", "store"]);
        assert!(d.config.values().all(|k| !k.optional && !k.secret));
        let state = d.state.unwrap();
        assert_eq!(
            (state.local, state.permanent.as_deref()),
            (None, Some("store"))
        );
    }

    #[test]
    fn the_shipped_adapter_declaration_reads_from_a_plugin_manifest() {
        use ghost::contract::{read, Scope, Slot};
        let shipped = crate::shared::repo_root()
            .unwrap()
            .join(crate::shared::GHOST_PLUGIN_DIR)
            .join("my-harness.json");
        let decl: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(shipped).unwrap()).unwrap();
        // As the flake writes it into the Claude plugin manifest.
        let dir = TempDir::new().unwrap();
        let manifest = dir.path().join("plugin.json");
        let plugin = serde_json::json!({"name": "maid", "extensions": {"my-harness": decl}});
        fs::write(&manifest, plugin.to_string()).unwrap();
        let Ok(Slot::Declared(d)) = read(Standard::Plugin, &manifest) else {
            panic!("no declaration");
        };
        assert_eq!(d.scope, Scope::Session);
        assert!(d.job.contains("Claude Code"), "{}", d.job);
    }

    #[test]
    fn a_remote_on_this_machine_is_not_recorded() {
        for local in [
            "/home/me/mAId",
            "./x",
            "../x",
            "~/repo",
            "repo",
            "file:///x",
        ] {
            assert_eq!(remote(local), None, "{local}");
        }
        assert_eq!(
            remote("https://github.com/someone/mAId.git").as_deref(),
            Some("https://github.com/someone/mAId.git")
        );
        assert_eq!(
            remote("git@github.com:someone/mAId.git").as_deref(),
            Some("github.com:someone/mAId.git")
        );
    }

    #[test]
    fn a_user_or_token_in_a_remote_is_never_recorded() {
        let got = remote("https://me:tok3n@example.org/me/mAId.git").unwrap();
        assert_eq!(got, "https://example.org/me/mAId.git");
        let slashed = remote("https://me:ab/cd@example.org/x").unwrap();
        assert_eq!(slashed, "https://example.org/x");
    }

    #[test]
    fn the_revision_says_when_the_tree_has_changes() {
        let at = |head: Option<&str>, dirty| {
            project_source(Checkout {
                origin: None,
                head: head.map(str::to_string),
                dirty,
                pensieve: None,
            })
            .rev
        };
        assert_eq!(at(Some("abc"), false).as_deref(), Some("abc"));
        assert_eq!(at(Some("abc"), true).as_deref(), Some("abc-dirty"));
        assert_eq!(at(None, true), None);
    }

    #[test]
    fn civil_dates_are_right_across_leap_days() {
        use ghost::time::civil;
        assert_eq!(civil(0), "1970-01-01");
        assert_eq!(civil(11_016), "2000-02-29");
        assert_eq!(civil(20_757), "2026-10-31");
    }
}
