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
use crate::memory;
use crate::part::{self, Item, What};
use crate::reads;
use crate::shared::{checkout_skill, on_path, usage, Agent, BringIn as Marketplace};
use anyhow::{anyhow, Context, Result};
use maid_core::contract::Standard;
use maid_core::parts::{BringIn, Source, SourceKind, Tenure, Unit};
use std::fs;
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
pub fn check_content(content_dir: &Path) -> Result<usize, Vec<String>> {
    let skills: Vec<PathBuf> = fs::read_dir(content_dir.join("skills"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|dir| dir.join("SKILL.md").exists())
        .collect();
    let errs: Vec<String> = skills
        .iter()
        .flat_map(|dir| {
            check_one_skill(&dir.join("SKILL.md"))
                .err()
                .into_iter()
                .chain(repo_pointers(dir))
        })
        .collect();
    match errs.is_empty() {
        true => Ok(skills.len()),
        false => Err(errs),
    }
}

/// Text that sends an installed session to the mAId repo, which it never
/// has (see project.md "Skill state"). Matched lowercased.
const REPO_POINTERS: [&str; 4] = [
    "resources/content/",
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
    (!fm.name.trim().is_empty() && !fm.description.trim().is_empty())
        .then_some(())
        .ok_or_else(|| at("name and description must be non-empty".into()))
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
pub fn cmd_validate(content_dir: &Path) -> Result<u8> {
    let count = check_content(content_dir)
        .map_err(|errs| anyhow!("Content validation failed:\n{}", errs.join("\n")))?;
    eprintln!("validated {count} content file(s)");
    Ok(0)
}

/// Validate the content about to be deployed, then deploy it.
pub fn cmd_install(
    target: &impl Deploy,
    content_dir: &Path,
    parts: &Path,
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
    cmd_validate(content_dir)?;
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
    parts: &Path,
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
        eprintln!("build-tool: part list not updated: {e:#}");
    }
}

/// What `Deploy` places; the browser MCP is recorded by its own script.
const DEPLOYED_KINDS: &[What] = &[What::Skill, What::Plugin, What::Marketplace];

/// Write what the target reports placed into the part list, for each
/// selected agent.
fn record_placed(
    target: &impl Deploy,
    parts: &Path,
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
        part::record(parts, a, DEPLOYED_KINDS, None, items)?;
    }
    Ok(())
}

/// Record one kind of entry on one agent in the part list, for a step
/// outside `Deploy` (the browser MCP). No names clears that kind.
pub fn cmd_record(
    parts: &Path,
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
        Some(name) => part::record_named(parts, agent, kind, scope, name, items)?,
        None => part::record(parts, agent, &[kind], scope, items)?,
    }
    Ok(0)
}

// ─────────────────────────────────────────────────────────────────
// Bring-ins — what mAId's part brings into the ghost, for the core.
// ─────────────────────────────────────────────────────────────────

/// Record mAId's persona and its bring-ins in its part list: each shipped
/// skill, the personal memory server when a store is set, and each vendor
/// plugin, borrowed. See project.md "My harness".
pub fn cmd_install_bring_ins(
    parts: &Path,
    profile: &Path,
    checkout: Checkout,
    store_file: &Path,
    vendors: &[Marketplace],
    dry_run: bool,
) -> Result<u8> {
    // A bring-in is recorded only while install has it placed somewhere.
    let items = bring_ins(
        profile,
        &project_source(checkout),
        memory::store(store_file)?.as_deref(),
        vendors,
        &placed_anywhere(parts)?,
        &part::bring_ins(parts)?,
        &today(),
    )?;
    println!(
        "bring-ins: {} recorded for persona {}",
        items.len(),
        part::PERSONA
    );
    if !dry_run {
        part::record_bring_ins(parts, items)?;
    }
    Ok(0)
}

/// Uninstall keeps only the bring-ins some harness still has placed, so a
/// whole uninstall clears them with the persona.
pub fn cmd_uninstall_bring_ins(parts: &Path, vendors: &[Marketplace], dry_run: bool) -> Result<u8> {
    let placed = placed_anywhere(parts)?;
    let mut kept = part::bring_ins(parts)?;
    kept.retain(|b| still_placed(b, &placed, vendors));
    if !dry_run {
        part::record_bring_ins(parts, kept)?;
    }
    Ok(0)
}

fn bring_ins(
    profile: &Path,
    project: &Source,
    store: Option<&Path>,
    vendors: &[Marketplace],
    placed: &[Item],
    recorded: &[BringIn],
    today: &str,
) -> Result<Vec<BringIn>> {
    let own = |name: &str| BringIn {
        name: name.into(),
        unit: None,
        source: Some(project.clone()),
        tenure: Tenure::Own,
        route: None,
        values: Default::default(),
    };
    let skills = profile
        .join(crate::shared::PROFILE_CONTENT_DIR)
        .join("skills");
    let mut names: Vec<String> = fs::read_dir(&skills)
        .with_context(|| format!("cannot list {}", skills.display()))?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| skills.join(n).join("SKILL.md").is_file())
        .collect();
    names.sort();
    let mut items: Vec<BringIn> = names
        .iter()
        .map(|n| BringIn {
            unit: Some(Unit {
                standard: Standard::Skill,
                path: skills.join(n).join("SKILL.md"),
            }),
            ..own(n)
        })
        .collect();
    if let Some(store) = store {
        items.push(BringIn {
            values: [("store".to_string(), store.display().to_string())].into(),
            ..own(memory::SERVER)
        });
    }
    for v in vendors {
        for plugin in v.plugins {
            // When it was first brought in, which a rerun keeps.
            let since = recorded
                .iter()
                .find(|b| b.name == *plugin)
                .and_then(|b| match &b.tenure {
                    Tenure::Borrowed { since, .. } => since.clone(),
                    Tenure::Own => None,
                })
                .unwrap_or_else(|| today.into());
            items.push(BringIn {
                name: plugin.to_string(),
                unit: None,
                source: Some(Source {
                    kind: SourceKind::Vendor,
                    at: Some(v.source.into()),
                    rev: Some(v.rev.into()),
                }),
                tenure: Tenure::Borrowed {
                    from: v.marketplace.into(),
                    since: Some(since),
                },
                route: None,
                values: Default::default(),
            });
        }
    }
    items.retain(|b| still_placed(b, placed, vendors));
    Ok(items)
}

/// Whether some harness still has what this bring-in needs placed: the
/// memory server, the vendor's plugin, or mAId's skills (through the plugin
/// or as links).
fn still_placed(b: &BringIn, placed: &[Item], vendors: &[Marketplace]) -> bool {
    let has = |kind: What, name: &str| placed.iter().any(|i| i.kind == kind && i.name == name);
    if b.name == memory::SERVER {
        return has(What::Mcp, memory::SERVER);
    }
    if let Tenure::Borrowed { from, .. } = &b.tenure {
        return vendors
            .iter()
            .filter(|v| v.marketplace == from)
            .any(|v| has(What::Plugin, &v.plugin_id(&b.name)));
    }
    has(What::Plugin, crate::shared::PLUGIN_ID) || placed.iter().any(|i| i.kind == What::Skill)
}

/// Everything mAId's list records placed, on every harness.
fn placed_anywhere(parts: &Path) -> Result<Vec<Item>> {
    let mut placed = vec![];
    for agent in Agent::ALL {
        placed.extend(part::placed(parts, *agent)?);
    }
    Ok(placed)
}

/// What install reads from the checkout it runs from: its `origin` URL,
/// and its revision with `-dirty` when the tree has changes. Read there,
/// not from the profile, so the profile stays addressed by its content.
#[derive(Debug, Default)]
pub struct Checkout {
    pub origin: Option<String>,
    pub head: Option<String>,
    pub dirty: bool,
}

impl Checkout {
    /// Read through the `git` that PATH finds by rule 8. With no git, or
    /// outside a repo, every field is empty.
    pub fn read(dir: &Path) -> Checkout {
        let Some(git) = crate::shared::find_on(std::env::var_os("PATH").as_deref(), "git") else {
            return Checkout::default();
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
        }
    }
}

/// This repo as a source.
fn project_source(c: Checkout) -> Source {
    Source {
        kind: SourceKind::Project,
        at: c.origin.and_then(|u| remote(&u)),
        rev: c.head.map(|h| match c.dirty {
            true => format!("{h}-dirty"),
            false => h,
        }),
    }
}

/// A remote as the part list may hold it: a URL or `host:path`, with any
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

/// Today's UTC date as `YYYY-MM-DD` (days to civil date, after Howard
/// Hinnant's algorithm), so no date crate is needed for one stamp.
fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    civil(secs / 86_400)
}

fn civil(days: u64) -> String {
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

// ─────────────────────────────────────────────────────────────────
// Memory — the personal store's server, Claude's own memory, harvest.
// ─────────────────────────────────────────────────────────────────

/// The profile's memory server.
fn launcher(profile: &Path) -> PathBuf {
    profile.join("bin/pensieve")
}

/// Install's claude memory step: register `personal-memory` on the
/// personal store, turn Claude's own automatic memory off, and take away
/// the memory dir an older mAId set. Each is recorded in the part list.
pub fn cmd_install_memory(
    home: &Path,
    store_file: &Path,
    profile: &Path,
    parts: &Path,
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
        claimed: part::placed_by_others(parts, Agent::Claude, What::Mcp, memory::SERVER),
        cli_present,
    };
    let memory::ServerOutcome {
        out: server,
        failed,
    } = memory::install_server(home, &plan, dry_run)?;
    println!("{}", server.line);
    let registered = server.item.as_ref().is_some_and(|i| i.skipped.is_none());
    let off = memory::install_off(home, &ownership(parts)?, registered, cli_present, dry_run)?;
    println!("{}", off.line);
    let legacy = legacy_setting(home, parts, dry_run)?;
    if !dry_run {
        // mAId owns only what it recorded, so an unrecorded change is the user's to undo.
        let unrecorded =
            |what: &str| format!("claude {what} may be set but is not recorded; check it by hand");
        part::record_named(
            parts,
            Agent::Claude,
            What::Mcp,
            None,
            memory::SERVER,
            server.item.into_iter().collect(),
        )
        .with_context(|| unrecorded(memory::SERVER))?;
        part::record_setting(parts, Agent::Claude, memory::SETTING, off.item)
            .with_context(|| unrecorded(memory::SETTING))?;
        if legacy {
            part::record_setting(parts, Agent::Claude, memory::LEGACY_SETTING, None)?;
        }
    }
    Ok(u8::from(failed))
}

pub fn cmd_uninstall_memory(
    home: &Path,
    profile: &Path,
    parts: &Path,
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
        home,
        &launcher(profile),
        part::placed_by_others(parts, Agent::Claude, What::Mcp, memory::SERVER),
        on_path(Agent::Claude.cli()),
        dry_run,
    )?;
    println!("{}", server.line);
    let off = memory::uninstall_off(home, &ownership(parts)?, dry_run)?;
    println!("{}", off.line);
    let legacy = legacy_setting(home, parts, dry_run)?;
    if !dry_run {
        part::record_named(
            parts,
            Agent::Claude,
            What::Mcp,
            None,
            memory::SERVER,
            server.item.into_iter().collect(),
        )?;
        part::record_setting(parts, Agent::Claude, memory::SETTING, None)?;
        if legacy {
            part::record_setting(parts, Agent::Claude, memory::LEGACY_SETTING, None)?;
        }
    }
    Ok(u8::from(failed))
}

/// Remove the memory dir an older mAId set, while it is the value mAId
/// recorded. Whether the part list holds an entry for it to clear: one
/// recorded as skipped has no value and nothing to remove, but still goes.
fn legacy_setting(home: &Path, parts: &Path, dry_run: bool) -> Result<bool> {
    let Some(item) = part::recorded(parts, Agent::Claude, What::Setting, memory::LEGACY_SETTING)?
    else {
        return Ok(false);
    };
    if let Some(line) = memory::remove_legacy(home, item.value.as_deref(), dry_run)? {
        println!("{line}");
    }
    Ok(true)
}

/// Install's claude step for reading mAId's skill files and the user's
/// skill state: grant mAId's plugin dir and config dir, and record each
/// element.
pub fn cmd_install_reads(
    home: &Path,
    profile: &Path,
    config: &Path,
    parts: &Path,
    dry_run: bool,
    agent: Option<Agent>,
) -> Result<u8> {
    if agent.is_some_and(|a| a != Agent::Claude) {
        return Ok(0);
    }
    for grant in [reads::plugin_grant(profile), reads::config_grant(config)] {
        let setting = grant.list.setting();
        let out = reads::install_claude(
            home,
            &grant,
            &recorded_settings(parts, setting)?,
            &part::claimed_by_others(parts, Agent::Claude, setting),
            on_path(Agent::Claude.cli()),
            dry_run,
        )?;
        out.lines.iter().for_each(|l| println!("{l}"));
        if !dry_run {
            part::record_named(parts, Agent::Claude, What::Setting, None, setting, out.items)
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
    home: &Path,
    parts: &Path,
    dry_run: bool,
    agent: Option<Agent>,
) -> Result<u8> {
    if agent.is_some_and(|a| a != Agent::Claude) {
        return Ok(0);
    }
    for list in reads::List::ALL {
        let setting = list.setting();
        let own = recorded_settings(parts, setting)?;
        reads::uninstall_claude(home, list, &own, dry_run)?
            .iter()
            .for_each(|l| println!("{l}"));
        if !dry_run {
            part::record_setting(parts, Agent::Claude, setting, None)?;
        }
    }
    Ok(0)
}

/// Every element of the list setting `name` mAId's part list records
/// placed on claude.
fn recorded_settings(parts: &Path, name: &str) -> Result<Vec<Item>> {
    Ok(part::placed(parts, Agent::Claude)?
        .into_iter()
        .filter(|i| i.kind == What::Setting && i.name == name)
        .collect())
}

/// The setting `name` as mAId's part list records it placed on claude.
fn recorded_setting(parts: &Path, name: &str) -> Result<Option<Item>> {
    Ok(part::placed(parts, Agent::Claude)?
        .into_iter()
        .find(|i| i.kind == What::Setting && i.name == name))
}

/// What mAId owns of Claude's automatic-memory setting: what its part list
/// says it set or adopted.
fn ownership(parts: &Path) -> Result<memory::Ownership> {
    let item = recorded_setting(parts, memory::SETTING)?;
    Ok(memory::Ownership {
        adopted: item.as_ref().is_some_and(|i| i.adopted),
        recorded: item.and_then(|i| i.value),
        claimed: part::claimed_by_others(parts, Agent::Claude, memory::SETTING),
    })
}

pub fn cmd_memory_status(
    home: &Path,
    store_file: &Path,
    profile: &Path,
    parts: &Path,
) -> Result<u8> {
    let store = memory::store(store_file)?;
    memory::status(
        home,
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
pub fn cmd_harvest(home: &Path, codex_home: &Path, to: &Path, project: Option<&str>) -> Result<u8> {
    if !to.is_dir() {
        return Err(usage(format!(
            "{} is not a directory; clone the store first",
            to.display()
        )));
    }
    println!("harvest  into {}", to.join("memory").display());
    harvest::harvest(home, codex_home, to, project)?
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
        let text = fs::read_to_string(part::path(&parts)).unwrap();
        for harness in ["kiro-cli", "agy"] {
            assert!(text.contains(&format!("\"{harness}\"")), "{text}");
        }
        assert!(text.contains(r#""name": "notes""#), "{text}");
        cmd_uninstall(&target, &parts, false, false, None).unwrap();
        assert!(!part::path(&parts).exists());
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
        let text = fs::read_to_string(part::path(&parts)).unwrap();
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
        let names: Vec<_> = part::placed(&parts, Agent::Claude)
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
        assert!(fs::read_to_string(part::path(&parts))
            .unwrap()
            .contains("skipped"));
        cmd_uninstall(&target, &parts, false, false, None).unwrap();
        let text = fs::read_to_string(part::path(&parts)).unwrap();
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
        let errs = check_content(dir.path()).unwrap_err();
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
        assert_eq!(check_content(dir.path()).unwrap(), 1);
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

    // ── bring-ins ────────────────────────────────────────────────

    const VENDOR: Marketplace = Marketplace {
        marketplace: "a-vendor",
        source: "https://example.org/skills.git",
        rev: "abc",
        plugins: &["docs"],
        needs: &[],
    };

    fn profile_with(skills: &[&str]) -> TempDir {
        let p = TempDir::new().unwrap();
        for s in skills {
            let d = p.path().join("share/maid/skills").join(s);
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join("SKILL.md"), "---\nname: x\ndescription: y\n---\n").unwrap();
        }
        fs::create_dir_all(p.path().join("share/maid/skills/not-a-skill")).unwrap();
        p
    }

    /// mAId's plugin, the vendor's and the memory server, placed somewhere.
    fn all_placed() -> Vec<Item> {
        vec![
            Item::placed(What::Plugin, crate::shared::PLUGIN_ID),
            Item::placed(What::Plugin, VENDOR.plugin_id("docs")),
            Item::placed(What::Mcp, memory::SERVER),
        ]
    }

    #[test]
    fn uninstall_drops_a_borrowed_plugin_no_harness_still_has() {
        let p = profile_with(&["notes"]);
        let parts = TempDir::new().unwrap();
        let plugins = vec![
            Item::placed(What::Plugin, crate::shared::PLUGIN_ID),
            Item::placed(What::Plugin, VENDOR.plugin_id("docs")),
        ];
        part::record(parts.path(), Agent::Codex, &[What::Plugin], None, plugins).unwrap();
        let none = Path::new("/nowhere/memory-store");
        cmd_install_bring_ins(
            parts.path(),
            p.path(),
            Checkout::default(),
            none,
            &[VENDOR],
            false,
        )
        .unwrap();
        let names = || -> Vec<String> {
            part::bring_ins(parts.path())
                .unwrap()
                .into_iter()
                .map(|b| b.name)
                .collect()
        };
        assert_eq!(names(), ["docs", "notes"]);
        let mine = vec![Item::placed(What::Plugin, crate::shared::PLUGIN_ID)];
        part::record(parts.path(), Agent::Codex, &[What::Plugin], None, mine).unwrap();
        cmd_uninstall_bring_ins(parts.path(), &[VENDOR], false).unwrap();
        assert_eq!(names(), ["notes"]);
    }

    #[test]
    fn what_install_did_not_place_is_not_a_bring_in() {
        let p = profile_with(&["notes"]);
        let none = bring_ins(
            p.path(),
            &project(),
            None,
            &[VENDOR],
            &[],
            &[],
            "2026-10-31",
        )
        .unwrap();
        assert!(none.is_empty(), "{none:?}");
        let links = [Item::placed(What::Skill, "notes")];
        let skills = bring_ins(
            p.path(),
            &project(),
            None,
            &[VENDOR],
            &links,
            &[],
            "2026-10-31",
        )
        .unwrap();
        let names: Vec<_> = skills.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(names, ["notes"]);
    }

    fn project() -> Source {
        Source {
            kind: SourceKind::Project,
            at: None,
            rev: Some("r1".into()),
        }
    }

    #[test]
    fn install_records_each_skill_the_memory_server_and_the_vendor_borrowed() {
        let p = profile_with(&["notes", "kdevkit"]);
        let items = bring_ins(
            p.path(),
            &project(),
            Some(Path::new("/s")),
            &[VENDOR],
            &all_placed(),
            &[],
            "2026-10-31",
        )
        .unwrap();
        let names: Vec<_> = items.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(names, ["kdevkit", "notes", "personal-memory", "docs"]);
        let notes = &items[1];
        assert_eq!(
            notes.unit.as_ref().unwrap().path,
            p.path().join("share/maid/skills/notes/SKILL.md")
        );
        assert_eq!(
            (&notes.tenure, notes.source.as_ref()),
            (&Tenure::Own, Some(&project()))
        );
        assert_eq!(items[2].values["store"], "/s");
        assert_eq!(
            items[3].tenure,
            Tenure::Borrowed {
                from: "a-vendor".into(),
                since: Some("2026-10-31".into())
            }
        );
        assert_eq!(items[3].route(), maid_core::parts::Route::Report);
    }

    #[test]
    fn no_store_records_no_memory_server_and_a_rerun_keeps_since() {
        let p = profile_with(&["notes"]);
        let first = bring_ins(
            p.path(),
            &project(),
            None,
            &[VENDOR],
            &all_placed(),
            &[],
            "2026-01-01",
        )
        .unwrap();
        assert!(first.iter().all(|b| b.name != "personal-memory"));
        let again = bring_ins(
            p.path(),
            &project(),
            None,
            &[VENDOR],
            &all_placed(),
            &first,
            "2026-10-31",
        )
        .unwrap();
        assert_eq!(again, first);
    }

    #[test]
    fn the_memory_server_is_a_bring_in_only_while_install_has_it_placed() {
        let p = profile_with(&["notes"]);
        let (parts, conf) = (TempDir::new().unwrap(), TempDir::new().unwrap());
        let store_file = conf.path().join("memory-store");
        fs::write(&store_file, "/s/personal\n").unwrap();
        let names = || -> Vec<String> {
            part::bring_ins(parts.path())
                .unwrap()
                .into_iter()
                .map(|b| b.name)
                .collect()
        };
        let plugin = vec![Item::placed(What::Plugin, crate::shared::PLUGIN_ID)];
        part::record(parts.path(), Agent::Claude, &[What::Plugin], None, plugin).unwrap();
        let skipped = Item::skipped(What::Mcp, memory::SERVER, memory::STORE_MISSING);
        part::record_named(
            parts.path(),
            Agent::Claude,
            What::Mcp,
            None,
            memory::SERVER,
            vec![skipped],
        )
        .unwrap();
        cmd_install_bring_ins(
            parts.path(),
            p.path(),
            Checkout::default(),
            &store_file,
            &[],
            false,
        )
        .unwrap();
        assert_eq!(names(), ["notes"]);
        let placed = Item::placed(What::Mcp, memory::SERVER);
        part::record_named(
            parts.path(),
            Agent::Claude,
            What::Mcp,
            None,
            memory::SERVER,
            vec![placed],
        )
        .unwrap();
        cmd_install_bring_ins(
            parts.path(),
            p.path(),
            Checkout::default(),
            &store_file,
            &[],
            false,
        )
        .unwrap();
        assert_eq!(names(), ["notes", "personal-memory"]);
        part::record_named(
            parts.path(),
            Agent::Claude,
            What::Mcp,
            None,
            memory::SERVER,
            vec![],
        )
        .unwrap();
        cmd_uninstall_bring_ins(parts.path(), &[], false).unwrap();
        assert_eq!(
            names(),
            ["notes"],
            "removing the memory server keeps the skills the plugin still places"
        );
        part::record(parts.path(), Agent::Claude, &[What::Plugin], None, vec![]).unwrap();
        cmd_uninstall_bring_ins(parts.path(), &[], false).unwrap();
        assert!(names().is_empty());
        assert!(
            !part::path(parts.path()).exists(),
            "the list goes with its last bring-in"
        );
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
            })
            .rev
        };
        assert_eq!(at(Some("abc"), false).as_deref(), Some("abc"));
        assert_eq!(at(Some("abc"), true).as_deref(), Some("abc-dirty"));
        assert_eq!(at(None, true), None);
    }

    #[test]
    fn civil_dates_are_right_across_leap_days() {
        assert_eq!(civil(0), "1970-01-01");
        assert_eq!(civil(11_016), "2000-02-29");
        assert_eq!(civil(20_757), "2026-10-31");
    }
}
