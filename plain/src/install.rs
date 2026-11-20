//! Install: place the validated resources on each agent, then record
//! what was placed and where it came from (the package, its entries'
//! files and sources). See project.md "My harness" (the install record).

use crate::memory;
use crate::place::{outcome, Deploy};
use crate::profile::Profile;
use crate::record::{self, Dirs, Item, Package, Repo, What};
use crate::shared::{find_on, usage, Agent, BROWSER_SERVER};
use crate::stage::{self, Runtime};
use crate::validate::cmd_validate;
use anyhow::{anyhow, Result};
use std::fs;
use std::path::{Path, PathBuf};

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

/// The step's own error is returned; a record failure beside it is
/// still said.
pub(crate) fn warn_unrecorded(recorded: &Result<()>) {
    if let Err(e) = recorded {
        eprintln!("plain: install record not updated: {e:#}");
    }
}

/// What `Deploy` places; the browser MCP is recorded by its own script.
pub(crate) const DEPLOYED_KINDS: &[What] = &[What::Skill, What::Plugin, What::Marketplace];

/// Write what the target reports placed into the install record, for each
/// selected agent.
pub(crate) fn record_placed(
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
        record::record(parts, a, DEPLOYED_KINDS, None, items)?;
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
        Some(name) => record::record_named(parts, agent, kind, scope, name, items)?,
        None => record::record(parts, agent, &[kind], scope, items)?,
    }
    Ok(0)
}

// ─────────────────────────────────────────────────────────────────
// The package — what install records of mAId as a whole, and where each
// placed thing's files are and where it came from.
// ─────────────────────────────────────────────────────────────────

/// Record mAId as a package in its install record: this repo at its
/// revision, where its files and config are, and each entry's files and
/// source. See project.md "My harness" (the install record).
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
    let pensieve = crate::external::pensieve_source(profile, checkout.pensieve.clone());
    record::record_package(
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
    record::amend(parts, |i| {
        if i.skipped.is_none() {
            facts(i, root, pensieve);
        }
    })
}

fn facts(i: &mut Item, root: &Path, pensieve: Option<&Repo>) {
    let dir = |rel: &str| Some(root.join(rel)).filter(|d| d.is_dir());
    let vendor = crate::external::VENDORS.iter().find(|v| match i.kind {
        What::Marketplace => i.name == v.marketplace,
        What::Plugin => v.plugins.iter().any(|p| i.name == v.plugin_id(p)),
        _ => false,
    });
    if let Some(v) = vendor {
        i.source = Some(Repo {
            repo: Some(v.source.into()),
            rev: Some(v.rev.into()),
            path: None,
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
        let pensieve = crate::external::pensieve(dir);
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

/// This repo as a source.
pub fn project_source(c: Checkout) -> Repo {
    Repo {
        repo: c.origin.and_then(|u| remote(&u)),
        rev: c.head.map(|h| match c.dirty {
            true => format!("{h}-dirty"),
            false => h,
        }),
        path: None,
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
// The package into a prefix, and the profile.
// ─────────────────────────────────────────────────────────────────

/// What a generation of plain's profile must hold for the agents to use it.
pub const NEEDS: &[&str] = &[
    "share/maid/skills",
    "share/maid/marketplace/plugins/maid/.codex-plugin/plugin.json",
    "bin/maid-browser-mcp",
];

/// The runtimes plain links in when it stages without nix: whatever PATH
/// has of them.
const RUNTIMES: &[&str] = &["pensieve", "chrome-devtools-mcp"];

/// Validate `resources` and install plain's package into `prefix`.
pub fn cmd_install_prefix(resources: &Path, prefix: &Path, runtimes: &[Runtime]) -> Result<u8> {
    cmd_validate(&[resources.to_path_buf()])?;
    let staged = prefix.with_file_name(format!(
        ".{}.staged",
        prefix.file_name().unwrap_or_default().to_string_lossy()
    ));
    if staged.exists() {
        fs::remove_dir_all(&staged)?;
    }
    let version = stage::plain(resources, &staged, runtimes)?;
    stage::install(&staged, prefix, stage::MARKETPLACE)?;
    fs::remove_dir_all(&staged)?;
    println!("installed maid {version} into {}", prefix.display());
    Ok(0)
}

/// Plain's profile, built from the checkout at `root`.
pub fn profile(root: &Path, at: PathBuf) -> Profile {
    Profile {
        at,
        flake: root.to_path_buf(),
        attr: "default".into(),
        needs: NEEDS.to_vec(),
        path: std::env::var_os("PATH"),
    }
}

/// Validate the checkout's resources and build them into the profile: a
/// nix generation, or without nix the package staged into the profile dir,
/// with the runtimes PATH has.
pub fn cmd_install_profile(p: &Profile, resources: &Path) -> Result<u8> {
    cmd_validate(std::slice::from_ref(&resources.to_path_buf()))?;
    let path = p.path.clone();
    p.install(|out| {
        let runtimes: Vec<Runtime> = RUNTIMES
            .iter()
            .filter_map(|n| {
                Some(Runtime {
                    name: n.to_string(),
                    path: find_on(path.as_deref(), n)?,
                })
            })
            .collect();
        stage::plain(resources, out, &runtimes).map(drop)
    })?;
    println!("{}", p.status());
    Ok(0)
}

/// Place the profile's package on each agent (or `agent`), then claude's
/// memory and read steps, then record mAId as a package: `just install`
/// after the profile is built.
pub fn cmd_place_all(root: &Path, dry_run: bool, force: bool, agent: Option<Agent>) -> Result<u8> {
    use crate::shared::{claude_config, config_dir, manifest_dirs, memory_store_file, profile_dir};
    let (profile, records, claude) = (profile_dir()?, manifest_dirs()?, claude_config()?);
    let rc = cmd_install(
        &crate::place::Deployment::from_env(true)?,
        &profile.join(crate::shared::PROFILE_CONTENT_DIR),
        &records,
        dry_run,
        force,
        agent,
    )?;
    let memory = memory::cmd_install_memory(
        &claude,
        &memory_store_file()?,
        &profile,
        &records,
        dry_run,
        agent,
    )?;
    let reads = crate::reads::cmd_install_reads(
        &claude,
        &profile,
        &config_dir()?,
        &records,
        dry_run,
        agent,
    )?;
    // mAId as a package, whichever agent was selected.
    let package = cmd_install_package(
        &records,
        &profile,
        &config_dir()?,
        Checkout::read(root),
        dry_run,
    )?;
    Ok(rc.max(memory).max(reads).max(package))
}

/// The whole plain install from the checkout at `root`, for every agent or
/// one: `just install` (`plain setup`), and the ghost's install when it
/// installs plain as a library. The profile first: if it cannot be built,
/// nothing else runs. Then every agent placed and recorded, then the
/// browser server registered by its own step; a step that fails, such as
/// one leaving a location alone, is said, the next still runs, and the
/// run ends non-zero.
pub fn all(root: &Path, agent: Option<Agent>) -> Result<u8> {
    let resources = root.join(crate::shared::CONTENT_DIR);
    let p = profile(root, crate::shared::profile_dir()?);
    cmd_install_profile(&p, &resources)?;
    let placed = cmd_place_all(root, false, false, agent).unwrap_or_else(|e| {
        eprintln!("plain: {e:#}");
        1
    });
    let launcher = p.at.join("bin/maid-browser-mcp");
    let browser = std::process::Command::new(resources.join("browser/manage"))
        .arg("install")
        .arg(&launcher)
        .arg(agent.map_or("", |a| a.name()))
        .current_dir(root)
        .status()
        .map_err(|e| anyhow!("cannot run the browser server's install: {e}"))?;
    Ok(placed.max(u8::from(!browser.success())))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::uninstall::cmd_uninstall;
    use std::path::PathBuf;
    use tempfile::TempDir;

    pub(crate) fn write(p: &Path, s: &str) {
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, s).unwrap();
    }

    /// A profile with one valid skill, its home, and a parts dir.
    pub(crate) fn installable() -> (TempDir, crate::place::Symlinks, PathBuf) {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("profile/share/maid/skills/notes/SKILL.md"),
            "---\nname: notes\ndescription: d\n---\nbody.\n",
        );
        let target = crate::place::Symlinks {
            home: dir.path().join("home"),
            source: dir.path().join("profile"),
        };
        let parts = dir.path().join("parts");
        (dir, target, parts)
    }

    pub(crate) fn content(dir: &TempDir) -> PathBuf {
        dir.path().join("profile/share/maid")
    }

    #[test]
    fn install_writes_the_part_list_and_uninstall_removes_it() {
        let (dir, target, parts) = installable();
        cmd_install(&target, &content(&dir), &parts, false, false, None).unwrap();
        let text = fs::read_to_string(record::path(&parts)).unwrap();
        for harness in ["kiro-cli", "agy"] {
            assert!(text.contains(&format!("\"{harness}\"")), "{text}");
        }
        assert!(text.contains(r#""name": "notes""#), "{text}");
        cmd_uninstall(&target, &parts, false, false, None).unwrap();
        assert!(!record::path(&parts).exists());
    }

    /// Installs into kiro, then fails before the rest.
    struct Partway;

    impl Deploy for Partway {
        fn install(&self, _: Option<Agent>, _: bool, _: bool) -> Result<Vec<crate::place::Report>> {
            Err(anyhow!("codex plugin install failed"))
        }
        fn uninstall(
            &self,
            _: Option<Agent>,
            _: bool,
            _: bool,
        ) -> Result<Vec<crate::place::Report>> {
            unreachable!()
        }
        fn status(&self, _: Option<Agent>) -> Result<Vec<crate::place::Report>> {
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
        let text = fs::read_to_string(record::path(&parts)).unwrap();
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
        let names: Vec<_> = record::placed(&parts, Agent::Claude)
            .unwrap()
            .into_iter()
            .map(|i| i.name)
            .collect();
        assert_eq!(names, ["personal-memory"]);
        assert!(rec(Some("a"), &["b"]).is_err());
    }

    /// A missing tree validates as zero files, so without the guard an
    /// install with no profile would link every agent at nothing.
    #[test]
    fn install_without_a_profile_names_the_fix() {
        let dir = TempDir::new().unwrap();
        let err = cmd_install(
            &crate::place::NoDeploy,
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

    fn pensieve() -> Repo {
        Repo {
            repo: Some("https://github.com/me/pensieve".into()),
            rev: Some("p1".into()),
            path: None,
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
        // The memory server as nix puts it in a profile, from the store.
        if let Some(store) = std::env::var_os("MAID_TEST_PENSIEVE") {
            fs::create_dir_all(p.path().join("bin")).unwrap();
            std::os::unix::fs::symlink(store, p.path().join("bin/pensieve")).unwrap();
        }
        p
    }

    fn json(parts: &Path) -> serde_json::Value {
        serde_json::from_str(&fs::read_to_string(record::path(parts)).unwrap()).unwrap()
    }

    #[test]
    fn install_records_the_package_and_where_each_thing_came_from() {
        let (p, parts, config) = (profile(), TempDir::new().unwrap(), Path::new("/c/maid"));
        let on = |agent, items: Vec<Item>| {
            for i in items {
                record::record_named(parts.path(), agent, i.kind, None, &i.name.clone(), vec![i])
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
        assert_eq!(m["installed"], crate::time::today());
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
        let vendor = &crate::external::VENDORS[0];
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
        if std::env::var_os("MAID_TEST_PENSIEVE").is_some() {
            assert_eq!(
                mem["source"]["rev"], "p1",
                "nix's pensieve: the flake's pin"
            );
        }
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
        assert!(!record::path(parts.path()).exists());
        record::record(
            parts.path(),
            Agent::Agy,
            &[What::Skill],
            None,
            vec![Item::placed(What::Skill, "notes")],
        )
        .unwrap();
        let before = fs::read(record::path(parts.path())).unwrap();
        cmd_install_package(
            parts.path(),
            p.path(),
            Path::new("/c"),
            Checkout::default(),
            true,
        )
        .unwrap();
        assert_eq!(fs::read(record::path(parts.path())).unwrap(), before);
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
}
