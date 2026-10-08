//! Deployment — the one place that knows how an agent lays out `$HOME`.
//!
//! The pipeline above declares **what** should be deployed; this decides
//! **how**. That split is deliberate and load-bearing: knowing each
//! agent's home layout is the least durable knowledge in this repo, so it
//! is quarantined behind one trait rather than spread through the stages.
//!
//! Where an agent can install mAId itself, we hand it over: claude and
//! codex install the profile's marketplace with their own plugin CLI
//! (`Plugins`). kiro and agy have no such command, so `Symlinks` links
//! their skills dirs at the profile. `Deployment` routes each agent to
//! one of the two, and nothing in `stages` knows which.

use crate::shared::{
    marketplace_root, on_path, selected_entries, Agent, Entry, Kind, Link, MARKETPLACE,
    PLUGIN_AGENTS, PLUGIN_ID, PROFILE_MARKETPLACE_DIR,
};
use anyhow::{anyhow, Context, Result};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// What a deployment target reports about one managed location.
#[derive(Debug, PartialEq, Eq)]
pub enum State {
    /// Deployed, and pointing where it should.
    Ok(PathBuf),
    /// Not deployed.
    Missing,
    /// Deployed, but pointing somewhere else.
    Wrong { found: PathBuf, want: PathBuf },
    /// Something not ours occupies the location.
    Occupied(&'static str),
    /// The source we would deploy from doesn't exist.
    SourceMissing,
    /// A plugin, enabled, at this install's version.
    Current(String),
    /// A plugin, enabled, at another version than this install's.
    Stale { found: String, want: String },
    /// A plugin the user disabled.
    Disabled { found: String, want: String },
    /// The agent's CLI is not on PATH, so there is nothing to install into.
    NoCli(&'static str),
    /// A link an older mAId install left where an agent now gets a plugin.
    Legacy(PathBuf),
    /// The agent's CLI would not report its plugins.
    Unreadable(String),
}

impl State {
    /// The human-facing summary `status` prints.
    pub fn describe(&self) -> String {
        match self {
            State::Ok(target) => format!("ok -> {}", target.display()),
            State::Missing => "missing".into(),
            State::SourceMissing => "source missing".into(),
            State::Wrong { found, want } => {
                format!("WRONG -> {} (expected {})", found.display(), want.display())
            }
            State::Occupied(what) => format!("non-symlink ({what})"),
            State::Current(v) => format!("ok {v}"),
            State::Stale { found, want } => {
                format!("STALE {found} (this install is {want}; run: just install)")
            }
            State::Disabled { found, want } if found == want => format!("disabled {found}"),
            State::Disabled { found, want } => {
                format!("disabled {found}, STALE (this install is {want})")
            }
            State::NoCli(cli) => format!("{cli} not on PATH"),
            State::Legacy(target) => format!("old link -> {}", target.display()),
            State::Unreadable(why) => format!("unreadable ({why}); run: just install"),
        }
    }
}

/// One deployment location and what is currently there.
#[derive(Debug)]
pub struct Report {
    /// How the location is named to the user, relative to the target root.
    pub label: String,
    pub state: State,
    /// Whether this call actually changed the location. `false` for
    /// `status`, and for a location `create`/`remove` declined to touch —
    /// the caller reports and counts from this, not by re-deriving it
    /// from `(state, force)`.
    pub acted: bool,
}

/// How a set of skills reaches the agents that consume them.
///
/// Every method takes the agent selection and reports what it found or
/// did; none of them lets the caller see a path. A stage that wanted to
/// know where a skill physically lives would be reaching through this
/// boundary rather than across it.
pub trait Deploy {
    /// Deploy, and report what each location looked like beforehand.
    fn install(&self, agent: Option<Agent>, dry_run: bool, force: bool) -> Result<Vec<Report>>;

    /// Remove what we deployed, leaving anything we don't own.
    fn uninstall(&self, agent: Option<Agent>, dry_run: bool, force: bool) -> Result<Vec<Report>>;

    /// Report without changing anything.
    fn status(&self, agent: Option<Agent>) -> Result<Vec<Report>>;

    /// Whether this agent's skills are deployed — the smoke stage's
    /// precondition, asked as a question rather than a path lookup.
    fn is_deployed(&self, agent: Agent) -> bool;
}

/// Deployment by symlink into `$HOME`, per the registry.
///
/// For agents with no plugin command of their own; see the module comment.
pub struct Symlinks {
    pub home: PathBuf,
    /// What the registry's source paths are relative to: the mAId profile.
    /// Links name this path, never the store path behind it, so a new
    /// generation reaches every agent without relinking.
    pub source: PathBuf,
}

impl Deploy for Symlinks {
    fn install(&self, agent: Option<Agent>, dry_run: bool, force: bool) -> Result<Vec<Report>> {
        self.act(agent, |link| self.create(link, dry_run, force))
    }

    fn uninstall(&self, agent: Option<Agent>, dry_run: bool, force: bool) -> Result<Vec<Report>> {
        self.act(agent, |link| self.remove(link, dry_run, force))
    }

    fn status(&self, agent: Option<Agent>) -> Result<Vec<Report>> {
        self.act(agent, |link| Ok((self.inspect(link), false)))
    }

    fn is_deployed(&self, agent: Agent) -> bool {
        // A skills dir existing proves nothing about whether we deployed
        // into it (a FanOut dir is the agent's own). Ask instead whether
        // at least one of our own links resolves to something there —
        // `State::Ok` alone is not enough, since it only means the
        // symlink points where the registry says, not that the target
        // still exists (the source can be deleted out from under an
        // otherwise-correct symlink).
        selected_entries(Some(agent))
            .into_iter()
            .filter_map(|entry| self.expand(entry).ok())
            .flatten()
            .any(|(home, source)| {
                matches!(self.inspect(&(home, source.clone())), State::Ok(_)) && exists(&source)
            })
    }
}

impl Symlinks {
    /// Resolve the selection to concrete locations and apply `f` to each,
    /// in a deterministic order so two runs are diffable.
    fn act(
        &self,
        agent: Option<Agent>,
        f: impl Fn(&Link) -> io::Result<(State, bool)>,
    ) -> Result<Vec<Report>> {
        let mut out = Vec::new();
        for entry in selected_entries(agent) {
            for link in self.expand(entry)? {
                let (state, acted) = f(&link)?;
                out.push(Report {
                    label: self.label(&link.0),
                    state,
                    acted,
                });
            }
        }
        Ok(out)
    }

    /// Name a location by its path relative to `$HOME`, so a fan-out
    /// child reads as `<dir>/<name>`.
    fn label(&self, home_path: &Path) -> String {
        home_path
            .strip_prefix(&self.home)
            .unwrap_or(home_path)
            .display()
            .to_string()
    }

    /// Resolve a registry entry to the concrete symlinks it manages —
    /// `Link` yields one; `FanOut` yields one per child. FanOut unions the
    /// source's current children (what should exist) with home symlinks
    /// already pointing into this source (so a child renamed or removed in
    /// source is still reaped, not orphaned as a dangling link in a dir we
    /// don't own). Keyed by home path for dedupe + deterministic order.
    fn expand(&self, entry: Entry) -> io::Result<Vec<Link>> {
        let (home_sub, source_sub, kind, _agent) = entry;
        let home = self.home.join(home_sub);
        let source = self.source.join(source_sub);
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

    /// What is at a location now.
    fn inspect(&self, (home, source): &Link) -> State {
        // Inspect home first: a symlink already pointing at `source` is
        // ours to reap even if `source` is now gone (an orphaned fan-out
        // child). SourceMissing only when there's nothing at home to act on.
        match fs::symlink_metadata(home) {
            Err(_) if exists(source) => State::Missing,
            Err(_) => State::SourceMissing,
            Ok(meta) if meta.file_type().is_symlink() => match fs::read_link(home) {
                Ok(found) if found == *source => State::Ok(found),
                Ok(found) => State::Wrong {
                    found,
                    want: source.clone(),
                },
                Err(_) => State::Missing,
            },
            Ok(meta) if meta.is_dir() => State::Occupied("dir"),
            Ok(_) => State::Occupied("file"),
        }
    }

    fn create(&self, link: &Link, dry_run: bool, force: bool) -> io::Result<(State, bool)> {
        let (home, source) = link;
        let state = self.inspect(link);
        // Missing, or a symlink another mAId install left, is ours to set.
        // Any other symlink needs --force. Occupied is never actionable,
        // force or not: mAId does not overwrite a real file or directory.
        let act = match &state {
            State::Missing => true,
            State::Wrong { found, .. } => force || is_maids(found),
            _ => false,
        };
        if act && !dry_run {
            if let State::Wrong { .. } = &state {
                fs::remove_file(home)?;
            }
            if let Some(parent) = home.parent() {
                fs::create_dir_all(parent)?;
            }
            std::os::unix::fs::symlink(source, home)?;
        }
        Ok((state, act))
    }

    fn remove(&self, link: &Link, dry_run: bool, force: bool) -> io::Result<(State, bool)> {
        let (home, _) = link;
        let state = self.inspect(link);
        let act = match &state {
            State::Ok(_) => true,
            // A link another mAId install left is ours, as on install.
            State::Wrong { found, .. } if is_maids(found) => true,
            // --force reaps foreign symlinks and files, never a real
            // directory: mAId only ever creates symlinks, so a real dir at
            // a managed path belongs to the owning tool.
            State::Wrong { .. } | State::Occupied("file") if force => true,
            _ => false,
        };
        if act && !dry_run {
            fs::remove_file(home)?;
        }
        Ok((state, act))
    }
}

/// Every agent, each deployed the one way it supports.
pub struct Deployment {
    pub links: Symlinks,
    pub plugins: Plugins,
}

impl Deploy for Deployment {
    fn install(&self, agent: Option<Agent>, dry_run: bool, force: bool) -> Result<Vec<Report>> {
        let mut out = self.links.install(agent, dry_run, force)?;
        out.extend(self.plugins.install(agent, dry_run, force)?);
        Ok(out)
    }

    fn uninstall(&self, agent: Option<Agent>, dry_run: bool, force: bool) -> Result<Vec<Report>> {
        let mut out = self.links.uninstall(agent, dry_run, force)?;
        out.extend(self.plugins.uninstall(agent, dry_run, force)?);
        Ok(out)
    }

    fn status(&self, agent: Option<Agent>) -> Result<Vec<Report>> {
        let mut out = self.links.status(agent)?;
        out.extend(self.plugins.status(agent)?);
        Ok(out)
    }

    fn is_deployed(&self, agent: Agent) -> bool {
        match agent.is_plugin() {
            true => self.plugins.is_deployed(agent),
            false => self.links.is_deployed(agent),
        }
    }
}

/// Deployment as a plugin, through the agent's own CLI, from the
/// marketplace in the profile. The CLI owns its config and cache; mAId
/// only asks it to add, update and remove `maid@maid`.
pub struct Plugins {
    pub home: PathBuf,
    pub profile: PathBuf,
}

/// The entries of the marketplace root, each a link into the profile.
const ROOT_ENTRIES: [&str; 3] = [".claude-plugin", ".agents", "plugins"];

/// What an agent's plugin list says about mAId's plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub version: String,
    pub enabled: bool,
}

impl Deploy for Plugins {
    fn install(&self, agent: Option<Agent>, dry_run: bool, _force: bool) -> Result<Vec<Report>> {
        let agents = plugin_agents(agent);
        if agents.is_empty() {
            return Ok(vec![]);
        }
        let want = self.want()?;
        let root = self.root();
        let mut out = vec![self.ensure_root(dry_run)?];
        for (a, legacy, kind) in agents {
            // The old links stay until the plugin is in: an agent whose CLI
            // is missing, or whose install fails, keeps its skills.
            if !on_path(a.cli()) {
                out.push(self.report(a, State::NoCli(a.cli()), false));
                continue;
            }
            self.repair(a, dry_run)?;
            let state = self.state(a, Some(&want))?;
            let (steps, acted) = install_steps(a, &state, self.recorded(a)?.as_deref(), &root);
            if !dry_run {
                for step in &steps {
                    self.cli(a, step)?;
                }
                self.check_installed(a, &want, acted)?;
            }
            out.push(self.report(a, state, acted));
            out.extend(self.reap(legacy, kind, dry_run)?);
        }
        Ok(out)
    }

    fn uninstall(&self, agent: Option<Agent>, dry_run: bool, _force: bool) -> Result<Vec<Report>> {
        let mut out = Vec::new();
        for (a, legacy, kind) in plugin_agents(agent) {
            out.extend(self.reap(legacy, kind, dry_run)?);
            self.repair(a, dry_run)?;
            let state = self.state(a, None)?;
            if matches!(state, State::NoCli(_)) {
                out.push(self.report(a, state, false));
                continue;
            }
            let installed = state != State::Missing;
            let steps = uninstall_steps(a, installed, self.recorded(a)?.is_some());
            if !dry_run {
                for step in &steps {
                    self.cli(a, step)?;
                }
            }
            out.push(self.report(a, state, installed));
        }
        // The root is shared by every plugin agent, so it goes only with all.
        if agent.is_none() {
            out.extend(self.remove_root(dry_run)?);
        }
        Ok(out)
    }

    /// One agent's CLI refusing to answer is reported on its lines, so
    /// the rest of the status still prints.
    fn status(&self, agent: Option<Agent>) -> Result<Vec<Report>> {
        let want = self.want().ok();
        // One line: status prints one location per line.
        let unreadable = |e: anyhow::Error| {
            State::Unreadable(
                format!("{e:#}")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
            )
        };
        let mut out = Vec::new();
        for (a, legacy, kind) in plugin_agents(agent) {
            // Old links still there: a dry-run reap lists them.
            out.extend(self.reap(legacy, kind, true)?);
            let state = self.state(a, want.as_deref()).unwrap_or_else(unreadable);
            let has_cli = !matches!(state, State::NoCli(_));
            out.push(self.report(a, state, false));
            if has_cli {
                out.push(Report {
                    label: format!("{} marketplace {MARKETPLACE}", a.name()),
                    state: self
                        .recorded(a)
                        .map(|r| marketplace_state(r, &self.root()))
                        .unwrap_or_else(unreadable),
                    acted: false,
                });
            }
        }
        Ok(out)
    }

    fn is_deployed(&self, agent: Agent) -> bool {
        let Ok(want) = self.want() else {
            return false;
        };
        matches!(self.state(agent, Some(&want)), Ok(State::Current(_)))
            && self.recorded(agent).is_ok_and(|r| r == Some(self.root()))
    }
}

impl Plugins {
    fn root(&self) -> PathBuf {
        marketplace_root(&self.profile)
    }

    fn source(&self) -> PathBuf {
        self.profile.join(PROFILE_MARKETPLACE_DIR)
    }

    /// The plugin version this install built.
    fn want(&self) -> Result<String> {
        let manifest = self.source().join("plugins/maid/.codex-plugin/plugin.json");
        let text = fs::read_to_string(&manifest).with_context(|| {
            format!(
                "no plugin manifest at {} — build the profile first (just install)",
                manifest.display()
            )
        })?;
        plugin_version(&text).with_context(|| format!("{}", manifest.display()))
    }

    fn report(&self, agent: Agent, state: State, acted: bool) -> Report {
        Report {
            label: format!("{} plugin {PLUGIN_ID}", agent.name()),
            state,
            acted,
        }
    }

    /// Run the agent's CLI with this deployment's HOME, returning stdout.
    fn cli(&self, agent: Agent, args: &[String]) -> Result<String> {
        let out = Command::new(agent.cli())
            .args(args)
            .env("HOME", &self.home)
            .output()
            .with_context(|| format!("cannot run {}", agent.cli()))?;
        if !out.status.success() {
            return Err(anyhow!(
                "{} {} failed: {}",
                agent.cli(),
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    fn state(&self, agent: Agent, want: Option<&str>) -> Result<State> {
        if !on_path(agent.cli()) {
            return Ok(State::NoCli(agent.cli()));
        }
        let json = self.cli(agent, &strings(&["plugin", "list", "--json"]))?;
        Ok(plugin_state(parse_listed(agent, &json)?, want))
    }

    /// Where the agent's `maid` marketplace points, if it has one. codex
    /// records the resolved path, so one that resolves to the root is the
    /// root (HOME itself may sit behind a symlink).
    fn recorded(&self, agent: Agent) -> Result<Option<PathBuf>> {
        let json = self.cli(
            agent,
            &strings(&["plugin", "marketplace", "list", "--json"]),
        )?;
        let root = self.root();
        Ok(
            parse_recorded(agent, &json)?.map(|found| match same_dir(&found, &root) {
                true => root,
                false => found,
            }),
        )
    }

    /// A CLI can exit 0 without doing what it was asked, so read back
    /// both the plugin and its marketplace.
    fn check_installed(&self, agent: Agent, want: &str, acted: bool) -> Result<()> {
        let after = self.state(agent, Some(want))?;
        let installed = match &after {
            State::Current(_) => true,
            State::Disabled { found, .. } => !acted || found == want,
            _ => false,
        };
        let recorded = self.recorded(agent)?;
        if installed && recorded.as_deref() == Some(self.root().as_path()) {
            return Ok(());
        }
        Err(anyhow!(
            "{} plugin {PLUGIN_ID} is {} after install, its marketplace {}",
            agent.name(),
            after.describe(),
            marketplace_state(recorded, &self.root()).describe()
        ))
    }

    /// codex refuses every plugin command while the dir its `maid`
    /// marketplace was added from is gone (an isolated run that died
    /// before restoring). Removing that record lets install add it again.
    fn repair(&self, agent: Agent, dry_run: bool) -> Result<()> {
        if agent != Agent::Codex || !on_path(agent.cli()) {
            return Ok(());
        }
        match self.recorded(agent) {
            Ok(_) => return Ok(()),
            // Only that refusal; any other error is not ours to guess at.
            Err(e) if format!("{e:#}").contains(&format!("`{MARKETPLACE}` at ")) => {}
            Err(e) => return Err(e),
        }
        if !dry_run {
            self.cli(
                agent,
                &strings(&["plugin", "marketplace", "remove", MARKETPLACE]),
            )?;
        }
        Ok(())
    }

    /// Make the root a real dir whose entries link into the profile path.
    fn ensure_root(&self, dry_run: bool) -> Result<Report> {
        let root = self.root();
        let mut acted = false;
        for entry in ROOT_ENTRIES {
            let (at, want) = (root.join(entry), self.source().join(entry));
            match fs::symlink_metadata(&at) {
                Ok(m) if m.file_type().is_symlink() => {
                    if fs::read_link(&at)? == want {
                        continue;
                    }
                    if !dry_run {
                        fs::remove_file(&at)?;
                    }
                }
                Ok(_) => {
                    return Err(anyhow!(
                        "{} is not a link mAId made; move it and re-run",
                        at.display()
                    ))
                }
                Err(_) => {}
            }
            acted = true;
            if !dry_run {
                fs::create_dir_all(&root)?;
                std::os::unix::fs::symlink(&want, &at)?;
            }
        }
        Ok(Report {
            label: self.label(&root),
            state: match acted {
                true => State::Missing,
                false => State::Ok(self.source()),
            },
            acted,
        })
    }

    fn remove_root(&self, dry_run: bool) -> Result<Vec<Report>> {
        let root = self.root();
        if !root.is_dir() {
            return Ok(vec![]);
        }
        if !dry_run {
            for entry in ROOT_ENTRIES {
                let at = root.join(entry);
                if fs::symlink_metadata(&at).is_ok_and(|m| m.file_type().is_symlink()) {
                    fs::remove_file(at)?;
                }
            }
        }
        // Anything else in it is not ours, so the dir stays.
        let removed = dry_run || fs::remove_dir(&root).is_ok();
        Ok(vec![Report {
            label: self.label(&root),
            state: match removed {
                true => State::Ok(self.source()),
                false => State::Occupied("dir"),
            },
            acted: removed,
        }])
    }

    /// Remove the links an older mAId install left at an agent's skills
    /// path: the path itself (Link) or each entry under it (FanOut).
    /// Matched by `is_maids`, so a skill no longer shipped goes too.
    fn reap(&self, home_sub: &str, kind: Kind, dry_run: bool) -> Result<Vec<Report>> {
        let at = self.home.join(home_sub);
        let candidates = match kind {
            Kind::Link => vec![at],
            Kind::FanOut => match fs::read_dir(&at) {
                Ok(entries) => {
                    let mut v: Vec<PathBuf> =
                        entries.filter_map(|e| Some(e.ok()?.path())).collect();
                    v.sort();
                    v
                }
                Err(_) => vec![],
            },
        };
        let mut out = Vec::new();
        for link in candidates {
            let Ok(target) = fs::read_link(&link) else {
                continue;
            };
            if !is_maids(&target) {
                continue;
            }
            if !dry_run {
                fs::remove_file(&link)?;
            }
            out.push(Report {
                label: self.label(&link),
                state: State::Legacy(target),
                acted: true,
            });
        }
        Ok(out)
    }

    fn label(&self, path: &Path) -> String {
        path.strip_prefix(&self.home)
            .unwrap_or(path)
            .display()
            .to_string()
    }
}

/// The plugin agents an `--agent` selection covers.
fn plugin_agents(agent: Option<Agent>) -> Vec<(Agent, &'static str, Kind)> {
    PLUGIN_AGENTS
        .iter()
        .filter(|(a, ..)| agent.is_none_or(|sel| sel == *a))
        .copied()
        .collect()
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| s.to_string()).collect()
}

fn plugin_version(manifest: &str) -> Result<String> {
    let v: serde_json::Value = serde_json::from_str(manifest)?;
    v["version"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("plugin manifest has no version"))
}

/// mAId's plugin in an agent's `plugin list --json`. claude lists one
/// entry per scope; mAId installs at user scope.
fn parse_listed(agent: Agent, json: &str) -> Result<Option<Listed>> {
    let v: serde_json::Value = serde_json::from_str(json)
        .with_context(|| format!("{} plugin list --json: not JSON", agent.cli()))?;
    let (entries, id_key) = match agent {
        Agent::Claude => (v.as_array(), "id"),
        _ => (v["installed"].as_array(), "pluginId"),
    };
    let entries =
        entries.ok_or_else(|| anyhow!("{} plugin list --json: unexpected shape", agent.cli()))?;
    let ours = entries
        .iter()
        .find(|e| e[id_key] == PLUGIN_ID && (agent != Agent::Claude || e["scope"] == "user"));
    ours.map(|e| {
        Ok(Listed {
            version: e["version"]
                .as_str()
                .ok_or_else(|| anyhow!("{PLUGIN_ID} listed with no version"))?
                .to_string(),
            enabled: e["enabled"]
                .as_bool()
                .ok_or_else(|| anyhow!("{PLUGIN_ID} listed with no enabled flag"))?,
        })
    })
    .transpose()
}

/// The path an agent's `maid` marketplace was added from.
fn parse_recorded(agent: Agent, json: &str) -> Result<Option<PathBuf>> {
    let v: serde_json::Value = serde_json::from_str(json)
        .with_context(|| format!("{} plugin marketplace list --json: not JSON", agent.cli()))?;
    let (entries, path_key) = match agent {
        Agent::Claude => (v.as_array(), "path"),
        _ => (v["marketplaces"].as_array(), "root"),
    };
    let entries = entries.ok_or_else(|| {
        anyhow!(
            "{} plugin marketplace list --json: unexpected shape",
            agent.cli()
        )
    })?;
    Ok(entries
        .iter()
        .find(|e| e["name"] == MARKETPLACE)
        .and_then(|e| e[path_key].as_str())
        .map(PathBuf::from))
}

/// Whether two paths name the same existing directory.
fn same_dir(a: &Path, b: &Path) -> bool {
    a == b || matches!((fs::canonicalize(a), fs::canonicalize(b)), (Ok(x), Ok(y)) if x == y)
}

/// Whether an agent's `maid` marketplace is the one this install made.
fn marketplace_state(recorded: Option<PathBuf>, root: &Path) -> State {
    match recorded {
        None => State::Missing,
        Some(found) if found == root => State::Ok(found),
        Some(found) => State::Wrong {
            found,
            want: root.to_path_buf(),
        },
    }
}

/// `want` is `None` when there is no install to compare against.
fn plugin_state(listed: Option<Listed>, want: Option<&str>) -> State {
    let Some(Listed { version, enabled }) = listed else {
        return State::Missing;
    };
    let want = want.unwrap_or(&version).to_string();
    match (enabled, version == want) {
        (false, _) => State::Disabled {
            found: version,
            want,
        },
        (true, true) => State::Current(version),
        (true, false) => State::Stale {
            found: version,
            want,
        },
    }
}

/// The CLI calls that bring one agent to this install, and whether they
/// change the plugin (as opposed to only repointing its marketplace).
fn install_steps(
    agent: Agent,
    state: &State,
    recorded: Option<&Path>,
    root: &Path,
) -> (Vec<Vec<String>>, bool) {
    let root = root.display().to_string();
    let mut steps = Vec::new();
    if recorded != Some(Path::new(&root)) {
        // claude repoints a marketplace on add; codex refuses a new path.
        if agent == Agent::Codex && recorded.is_some() {
            steps.push(strings(&["plugin", "marketplace", "remove", MARKETPLACE]));
        }
        steps.push(strings(&["plugin", "marketplace", "add", &root]));
    }
    let plugin = match (agent, state) {
        (Agent::Claude, State::Missing) => {
            Some(&["plugin", "install", PLUGIN_ID, "--scope", "user"][..])
        }
        // claude keeps a disabled plugin disabled across an update.
        (Agent::Claude, State::Stale { .. }) => {
            Some(&["plugin", "update", PLUGIN_ID, "--scope", "user"][..])
        }
        (Agent::Claude, State::Disabled { found, want }) if found != want => {
            Some(&["plugin", "update", PLUGIN_ID, "--scope", "user"][..])
        }
        // `codex plugin add` installs or updates, and re-enables a disabled
        // plugin, so a disabled one is left alone.
        (Agent::Codex, State::Missing | State::Stale { .. }) => {
            Some(&["plugin", "add", PLUGIN_ID][..])
        }
        _ => None,
    };
    let acted = plugin.is_some();
    steps.extend(plugin.map(strings));
    (steps, acted)
}

/// The CLI calls that remove mAId's plugin and marketplace from an agent.
fn uninstall_steps(agent: Agent, installed: bool, recorded: bool) -> Vec<Vec<String>> {
    let remove = match agent {
        Agent::Claude => strings(&["plugin", "uninstall", PLUGIN_ID, "--scope", "user"]),
        _ => strings(&["plugin", "remove", PLUGIN_ID]),
    };
    // Plugin before marketplace: codex orphans the plugin's config entry
    // when its marketplace goes first.
    [
        installed.then_some(remove),
        recorded.then(|| strings(&["plugin", "marketplace", "remove", MARKETPLACE])),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// A target that deploys nothing, for the check stage.
///
/// Check carries each skill's text inline, so it has no deployment to
/// read or create. Handing it this instead of `Symlinks` makes that
/// structural: there is no `$HOME` to resolve and no path to
/// accidentally read, so "check needs no install" cannot regress into a
/// convention the prompt merely honours.
pub struct NoDeploy;

impl Deploy for NoDeploy {
    fn install(&self, _: Option<Agent>, _: bool, _: bool) -> Result<Vec<Report>> {
        Err(anyhow!("this stage does not deploy"))
    }

    fn uninstall(&self, _: Option<Agent>, _: bool, _: bool) -> Result<Vec<Report>> {
        Err(anyhow!("this stage does not deploy"))
    }

    fn status(&self, _: Option<Agent>) -> Result<Vec<Report>> {
        Err(anyhow!("this stage has no deployment to report"))
    }

    /// Nothing is deployed, and nothing needs to be.
    fn is_deployed(&self, _: Agent) -> bool {
        false
    }
}

fn exists(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok()
}

/// Whether a link target is a mAId skills tree, or a skill in one: a
/// profile's, or a checkout's from before the profile. Matched by path
/// shape, not provenance.
fn is_maids(target: &Path) -> bool {
    let roots = ["share/maid/skills", "resources/content/skills"];
    let tree = |p: &Path| roots.iter().any(|r| p.ends_with(r));
    tree(target) || target.parent().is_some_and(tree)
}

// ─────────────────────────────────────────────────────────────────
// Tests — the shim's own. Every case here is about $HOME layout, which
// is exactly the knowledge this module quarantines.
// ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sym(home: &Path, profile: &Path) -> Symlinks {
        Symlinks {
            home: home.to_path_buf(),
            source: profile.to_path_buf(),
        }
    }

    /// Deploy and report, the way the install verb does.
    fn install(
        home: &Path,
        profile: &Path,
        dry_run: bool,
        force: bool,
        agent: Option<Agent>,
    ) -> Vec<Report> {
        sym(home, profile).install(agent, dry_run, force).unwrap()
    }

    fn uninstall(
        home: &Path,
        profile: &Path,
        dry_run: bool,
        force: bool,
        agent: Option<Agent>,
    ) -> Vec<Report> {
        sym(home, profile).uninstall(agent, dry_run, force).unwrap()
    }

    fn status(home: &Path, profile: &Path, agent: Option<Agent>) -> Vec<Report> {
        sym(home, profile).status(agent).unwrap()
    }

    fn write(p: &Path, s: &str) {
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, s).unwrap();
    }

    /// A profile with a skills dir but no skills in it.
    fn make_profile() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("share/maid/skills")).unwrap();
        dir
    }

    /// A profile with two child skills, for the FanOut entry.
    fn make_profile_with_skills() -> TempDir {
        let dir = make_profile();
        for name in ["kdevkit", "notes"] {
            write(
                &dir.path()
                    .join("share/maid/skills")
                    .join(name)
                    .join("SKILL.md"),
                "---\nname: x\ndescription: y\n---\nbody.\n",
            );
        }
        dir
    }

    fn exists_at(p: &Path) -> bool {
        fs::symlink_metadata(p).is_ok()
    }

    #[test]
    fn install_creates_every_registry_location() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        let reports = install(home.path(), profile.path(), false, false, None);
        assert!(!reports.is_empty());
        // Every location was Missing beforehand, and exists now.
        assert!(reports.iter().all(|r| r.state == State::Missing));
        assert!(exists_at(&home.path().join(".kiro/steering/skills")));
        assert!(exists_at(&home.path().join(".gemini/config/skills")));
        // Plugin agents get no links.
        assert!(!exists_at(&home.path().join(".claude/skills")));
        assert!(!exists_at(&home.path().join(".codex/skills")));
    }

    #[test]
    fn install_is_idempotent() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        install(home.path(), profile.path(), false, false, None);
        let again = install(home.path(), profile.path(), false, false, None);
        assert!(again.iter().all(|r| matches!(r.state, State::Ok(_))));
    }

    #[test]
    fn dry_run_changes_nothing() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        install(home.path(), profile.path(), true, false, None);
        assert!(!exists_at(&home.path().join(".kiro/steering/skills")));
    }

    /// A real file at a managed path is never clobbered without --force.
    #[test]
    fn a_users_own_file_survives_install() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        let claimed = home.path().join(".kiro/steering/skills");
        write(&claimed, "mine");
        install(home.path(), profile.path(), false, false, None);
        assert_eq!(fs::read_to_string(&claimed).unwrap(), "mine");
    }

    #[test]
    fn a_foreign_symlink_is_replaced_only_with_force() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        let managed = home.path().join(".kiro/steering/skills");
        fs::create_dir_all(managed.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(profile.path(), &managed).unwrap();

        install(home.path(), profile.path(), false, false, None);
        assert_eq!(fs::read_link(&managed).unwrap(), profile.path());

        install(home.path(), profile.path(), false, true, None);
        assert_ne!(fs::read_link(&managed).unwrap(), profile.path());
    }

    /// The latest install takes over what an earlier one left, from a
    /// checkout or another profile, without --force.
    #[test]
    fn a_link_another_maid_install_left_is_replaced_without_force() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        let old = TempDir::new().unwrap();
        let kiro = home.path().join(".kiro/steering/skills");
        let agy = home.path().join(".gemini/config/skills");
        for link in [&kiro, &agy] {
            fs::create_dir_all(link.parent().unwrap()).unwrap();
        }
        std::os::unix::fs::symlink(old.path().join("resources/content/skills"), &kiro).unwrap();
        std::os::unix::fs::symlink(old.path().join("share/maid/skills"), &agy).unwrap();

        install(home.path(), profile.path(), false, false, None);
        for link in [&kiro, &agy] {
            assert_eq!(
                fs::read_link(link).unwrap(),
                profile.path().join("share/maid/skills")
            );
        }
    }

    #[test]
    fn only_a_maid_skills_tree_or_one_skill_in_it_is_maids() {
        for yes in [
            "../p/share/maid/skills",
            "/p/share/maid/skills/",
            "/c/resources/content/skills/notes",
        ] {
            assert!(is_maids(Path::new(yes)), "{yes}");
        }
        for no in [
            "/p/share/maid/skills/notes/sub",
            "/p/share/maid/skills-old",
            "/p/my-share/maid/skills",
            "/p",
        ] {
            assert!(!is_maids(Path::new(no)), "{no}");
        }
    }

    #[test]
    fn uninstall_removes_only_what_we_deployed() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        install(home.path(), profile.path(), false, false, None);
        // Something the agent owns, beside our link.
        let theirs = home.path().join(".kiro/steering/their-own");
        write(&theirs, "theirs");

        uninstall(home.path(), profile.path(), false, false, None);
        assert!(!exists_at(&home.path().join(".kiro/steering/skills")));
        assert!(exists_at(&theirs), "the agent's own entry must survive");
    }

    #[test]
    fn uninstall_is_idempotent_on_a_clean_home() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        let reports = uninstall(home.path(), profile.path(), false, false, None);
        assert!(reports
            .iter()
            .all(|r| matches!(r.state, State::Missing | State::SourceMissing)));
    }

    /// --force reaps foreign symlinks and files, never a real directory:
    /// mAId only ever creates symlinks, so a real dir belongs to the tool.
    #[test]
    fn force_uninstall_refuses_to_delete_a_real_directory() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        let theirs = home.path().join(".kiro/steering/skills");
        fs::create_dir_all(theirs.join("their-skill")).unwrap();

        uninstall(home.path(), profile.path(), false, true, None);
        assert!(theirs.join("their-skill").exists());
    }

    #[test]
    fn status_reports_every_location_without_changing_it() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        let before = status(home.path(), profile.path(), None);
        assert!(before.iter().all(|r| r.state == State::Missing));
        assert!(!exists_at(&home.path().join(".kiro/steering/skills")));

        install(home.path(), profile.path(), false, false, None);
        let after = status(home.path(), profile.path(), None);
        assert!(after.iter().all(|r| matches!(r.state, State::Ok(_))));
    }

    #[test]
    fn a_scoped_install_touches_only_that_agent() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        install(home.path(), profile.path(), false, false, Some(Agent::Kiro));
        assert!(exists_at(&home.path().join(".kiro/steering/skills")));
        assert!(!exists_at(&home.path().join(".gemini/config/skills")));
    }

    #[test]
    fn a_scoped_uninstall_leaves_other_agents_deployed() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        install(home.path(), profile.path(), false, false, None);
        uninstall(home.path(), profile.path(), false, false, Some(Agent::Kiro));
        assert!(!exists_at(&home.path().join(".kiro/steering/skills")));
        assert!(exists_at(&home.path().join(".gemini/config/skills")));
    }

    /// FanOut mirrors each source child rather than replacing the dir the
    /// agent owns.
    #[test]
    fn fanout_yields_one_link_per_source_child() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        let fanout = (".x/skills", "share/maid/skills", Kind::FanOut, Agent::Kiro);
        let links = sym(home.path(), profile.path()).expand(fanout).unwrap();
        assert_eq!(links.len(), 2, "one per child skill");
    }

    #[test]
    fn is_deployed_answers_the_smoke_precondition() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        let target = sym(home.path(), profile.path());
        assert!(!target.is_deployed(Agent::Kiro));
        assert!(!target.is_deployed(Agent::Agy));
        install(home.path(), profile.path(), false, false, None);
        assert!(target.is_deployed(Agent::Kiro));
        assert!(target.is_deployed(Agent::Agy));
    }

    /// A dangling symlink is not deployed: an agent reading through it
    /// finds no skill.
    #[test]
    fn a_dangling_link_does_not_count_as_deployed() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        install(home.path(), profile.path(), false, false, None);
        fs::remove_dir_all(profile.path().join("share/maid/skills")).unwrap();
        assert!(!sym(home.path(), profile.path()).is_deployed(Agent::Kiro));
    }

    #[test]
    fn state_descriptions_name_what_is_wrong() {
        assert!(State::Missing.describe().contains("missing"));
        assert!(State::Occupied("dir").describe().contains("dir"));
        assert!(State::Wrong {
            found: PathBuf::from("/a"),
            want: PathBuf::from("/b"),
        }
        .describe()
        .contains("WRONG"));
    }
    /// The bug an audit caught: --force over a real FILE must never be
    /// reported as acted-on. `create()` already refused to touch it;
    /// `outcome()` was re-deriving "did this act" from (state, force) and
    /// assumed force always means yes, so install printed "removed" for a
    /// file it never touched and exited 0 for a blocked install.
    #[test]
    fn force_install_never_reports_acting_on_a_real_file() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        let claimed = home.path().join(".kiro/steering/skills");
        write(&claimed, "mine");

        let reports = install(home.path(), profile.path(), false, true, None);
        let this = reports
            .iter()
            .find(|r| r.label == ".kiro/steering/skills")
            .unwrap();
        assert!(
            !this.acted,
            "create() must not act on a real file, force or not"
        );
        assert_eq!(fs::read_to_string(&claimed).unwrap(), "mine");
    }

    // ── Plugins ──────────────────────────────────────────────────

    fn plugins(home: &Path, profile: &Path) -> Plugins {
        Plugins {
            home: home.to_path_buf(),
            profile: profile.to_path_buf(),
        }
    }

    fn steps(agent: Agent, state: State, recorded: Option<&str>) -> (Vec<String>, bool) {
        let (steps, acted) = install_steps(
            agent,
            &state,
            recorded.map(Path::new),
            Path::new("/s/marketplace"),
        );
        (steps.into_iter().map(|s| s.join(" ")).collect(), acted)
    }

    fn stale() -> State {
        State::Stale {
            found: "1.0.0-a".into(),
            want: "1.0.0-b".into(),
        }
    }

    fn disabled(found: &str) -> State {
        State::Disabled {
            found: found.into(),
            want: "1.0.0-b".into(),
        }
    }

    #[test]
    fn claude_listing_finds_the_user_scope_entry() {
        let json = r#"[
          {"id":"maid@maid","version":"1.0.0-p","scope":"project","enabled":true},
          {"id":"maid@maid","version":"1.0.0-u","scope":"user","enabled":false},
          {"id":"other@x","version":"9","scope":"user","enabled":true}]"#;
        assert_eq!(
            parse_listed(Agent::Claude, json).unwrap(),
            Some(Listed {
                version: "1.0.0-u".into(),
                enabled: false
            })
        );
        assert_eq!(parse_listed(Agent::Claude, "[]").unwrap(), None);
    }

    #[test]
    fn codex_listing_finds_the_installed_entry() {
        let json = r#"{"installed":[{"pluginId":"maid@maid","version":"1.0.0-c","enabled":true}],
                       "available":[]}"#;
        assert_eq!(
            parse_listed(Agent::Codex, json).unwrap(),
            Some(Listed {
                version: "1.0.0-c".into(),
                enabled: true
            })
        );
        assert_eq!(
            parse_listed(Agent::Codex, r#"{"installed":[]}"#).unwrap(),
            None
        );
    }

    /// Output we cannot read must not pass as "not installed", or install
    /// would re-add over whatever is there.
    #[test]
    fn an_unreadable_listing_is_an_error() {
        assert!(parse_listed(Agent::Claude, "not json").is_err());
        assert!(parse_listed(Agent::Claude, "{}").is_err());
        assert!(parse_listed(Agent::Codex, "[]").is_err());
        assert!(parse_listed(Agent::Codex, r#"{"installed":[{"pluginId":"maid@maid"}]}"#).is_err());
        let no_flag = r#"{"installed":[{"pluginId":"maid@maid","version":"1"}]}"#;
        assert!(parse_listed(Agent::Codex, no_flag).is_err());
    }

    #[test]
    fn the_recorded_marketplace_path_is_read_per_cli() {
        let claude = r#"[{"name":"other","path":"/o"},{"name":"maid","path":"/s/marketplace"}]"#;
        let codex = r#"{"marketplaces":[{"name":"maid","root":"/s/marketplace"}]}"#;
        assert_eq!(
            parse_recorded(Agent::Claude, claude).unwrap(),
            Some(PathBuf::from("/s/marketplace"))
        );
        assert_eq!(
            parse_recorded(Agent::Codex, codex).unwrap(),
            Some(PathBuf::from("/s/marketplace"))
        );
        assert_eq!(parse_recorded(Agent::Claude, "[]").unwrap(), None);
        assert!(parse_recorded(Agent::Codex, "[]").is_err());
    }

    #[test]
    fn plugin_state_compares_against_this_install() {
        let listed = |v: &str, enabled| {
            Some(Listed {
                version: v.into(),
                enabled,
            })
        };
        assert_eq!(plugin_state(None, Some("b")), State::Missing);
        assert_eq!(
            plugin_state(listed("b", true), Some("b")),
            State::Current("b".into())
        );
        assert_eq!(
            plugin_state(listed("a", true), Some("b")),
            State::Stale {
                found: "a".into(),
                want: "b".into()
            }
        );
        assert_eq!(
            plugin_state(listed("a", false), Some("b")),
            State::Disabled {
                found: "a".into(),
                want: "b".into()
            }
        );
        // No install to compare with: whatever is there counts as current.
        assert_eq!(
            plugin_state(listed("a", true), None),
            State::Current("a".into())
        );
    }

    #[test]
    fn a_missing_plugin_is_installed_after_its_marketplace_is_added() {
        assert_eq!(
            steps(Agent::Claude, State::Missing, None),
            (
                vec![
                    "plugin marketplace add /s/marketplace".to_string(),
                    "plugin install maid@maid --scope user".to_string()
                ],
                true
            )
        );
        assert_eq!(
            steps(Agent::Codex, State::Missing, None),
            (
                vec![
                    "plugin marketplace add /s/marketplace".to_string(),
                    "plugin add maid@maid".to_string()
                ],
                true
            )
        );
    }

    #[test]
    fn a_current_plugin_on_the_right_marketplace_needs_nothing() {
        for agent in [Agent::Claude, Agent::Codex] {
            assert_eq!(
                steps(
                    agent,
                    State::Current("1.0.0-b".into()),
                    Some("/s/marketplace")
                ),
                (vec![], false)
            );
        }
    }

    #[test]
    fn a_stale_plugin_is_updated() {
        assert_eq!(
            steps(Agent::Claude, stale(), Some("/s/marketplace")).0,
            ["plugin update maid@maid --scope user"]
        );
        assert_eq!(
            steps(Agent::Codex, stale(), Some("/s/marketplace")).0,
            ["plugin add maid@maid"]
        );
    }

    /// claude's update keeps a plugin disabled; codex's add re-enables it,
    /// so a disabled codex plugin is not touched.
    #[test]
    fn a_disabled_plugin_stays_disabled() {
        assert_eq!(
            steps(Agent::Claude, disabled("1.0.0-a"), Some("/s/marketplace")),
            (
                vec!["plugin update maid@maid --scope user".to_string()],
                true
            )
        );
        assert_eq!(
            steps(Agent::Claude, disabled("1.0.0-b"), Some("/s/marketplace")),
            (vec![], false)
        );
        assert_eq!(
            steps(Agent::Codex, disabled("1.0.0-a"), Some("/s/marketplace")),
            (vec![], false)
        );
    }

    /// claude repoints a marketplace on add; codex refuses a changed path,
    /// so its old one is removed first.
    #[test]
    fn a_marketplace_at_another_path_is_repointed() {
        let current = || State::Current("1.0.0-b".into());
        assert_eq!(
            steps(Agent::Claude, current(), Some("/old")).0,
            ["plugin marketplace add /s/marketplace"]
        );
        assert_eq!(
            steps(Agent::Codex, current(), Some("/old")).0,
            [
                "plugin marketplace remove maid",
                "plugin marketplace add /s/marketplace"
            ]
        );
    }

    #[test]
    fn uninstall_removes_the_plugin_before_its_marketplace() {
        let joined = |a, i, r| {
            uninstall_steps(a, i, r)
                .into_iter()
                .map(|s| s.join(" "))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            joined(Agent::Codex, true, true),
            ["plugin remove maid@maid", "plugin marketplace remove maid"]
        );
        assert_eq!(
            joined(Agent::Claude, true, true),
            [
                "plugin uninstall maid@maid --scope user",
                "plugin marketplace remove maid"
            ]
        );
        assert!(joined(Agent::Claude, false, false).is_empty());
    }

    #[test]
    fn the_wanted_version_is_read_from_the_profile() {
        let profile = make_profile();
        let home = TempDir::new().unwrap();
        let err = plugins(home.path(), profile.path()).want().unwrap_err();
        assert!(format!("{err:#}").contains("just install"), "{err:#}");
        write(
            &profile
                .path()
                .join("share/maid/marketplace/plugins/maid/.codex-plugin/plugin.json"),
            r#"{"name":"maid","version":"1.0.0-abc"}"#,
        );
        assert_eq!(
            plugins(home.path(), profile.path()).want().unwrap(),
            "1.0.0-abc"
        );
    }

    #[test]
    fn the_marketplace_root_links_into_the_profile_path() {
        let state = TempDir::new().unwrap();
        let profile = state.path().join("profile");
        let home = TempDir::new().unwrap();
        let p = plugins(home.path(), &profile);
        let root = state.path().join("marketplace");

        assert!(p.ensure_root(false).unwrap().acted);
        for entry in ROOT_ENTRIES {
            assert_eq!(
                fs::read_link(root.join(entry)).unwrap(),
                profile.join("share/maid/marketplace").join(entry)
            );
        }
        assert!(!p.ensure_root(false).unwrap().acted, "idempotent");

        // A root left pointing at another profile is repointed.
        let other = plugins(home.path(), &state.path().join("other/profile"));
        fs::create_dir_all(state.path().join("other")).unwrap();
        fs::rename(&root, state.path().join("other/marketplace")).unwrap();
        assert!(other.ensure_root(false).unwrap().acted);
        fs::rename(state.path().join("other/marketplace"), &root).unwrap();
        assert!(p.ensure_root(false).unwrap().acted);
        assert_eq!(
            fs::read_link(root.join("plugins")).unwrap(),
            profile.join("share/maid/marketplace/plugins")
        );

        p.remove_root(false).unwrap();
        assert!(!exists_at(&root));
    }

    #[test]
    fn a_real_dir_in_the_marketplace_root_is_refused_and_kept() {
        let state = TempDir::new().unwrap();
        let home = TempDir::new().unwrap();
        let p = plugins(home.path(), &state.path().join("profile"));
        let theirs = state.path().join("marketplace/plugins");
        fs::create_dir_all(&theirs).unwrap();
        assert!(p.ensure_root(false).is_err());
        p.remove_root(false).unwrap();
        assert!(theirs.is_dir());
    }

    /// Old links into any mAId skills tree go, including a codex link to a
    /// skill no longer shipped; anything else at those paths stays.
    #[test]
    fn old_maid_links_are_reaped_and_nothing_else() {
        let home = TempDir::new().unwrap();
        let h = home.path();
        let p = plugins(h, Path::new("/s/maid/profile"));
        let link = |at: &str, to: &str| {
            let at = h.join(at);
            fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::os::unix::fs::symlink(to, at).unwrap();
        };
        link(".claude/skills", "/s/maid/profile/share/maid/skills");
        link(
            ".codex/skills/kdevkit",
            "/s/maid/profile/share/maid/skills/kdevkit",
        );
        link(
            ".codex/skills/gone",
            "/old/checkout/resources/content/skills/gone",
        );
        link(".codex/skills/foreign", "/elsewhere/skills/foreign");
        write(&h.join(".codex/skills/own/SKILL.md"), "mine");

        let mut reaped = p.reap(".claude/skills", Kind::Link, false).unwrap();
        reaped.extend(p.reap(".codex/skills", Kind::FanOut, false).unwrap());
        let labels: Vec<&str> = reaped.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                ".claude/skills",
                ".codex/skills/gone",
                ".codex/skills/kdevkit"
            ]
        );
        assert!(!exists_at(&h.join(".claude/skills")));
        assert!(!exists_at(&h.join(".codex/skills/kdevkit")));
        assert!(!exists_at(&h.join(".codex/skills/gone")));
        assert!(exists_at(&h.join(".codex/skills/foreign")));
        assert!(exists_at(&h.join(".codex/skills/own/SKILL.md")));
    }

    #[test]
    fn a_foreign_claude_skills_link_is_not_reaped() {
        let home = TempDir::new().unwrap();
        let at = home.path().join(".claude/skills");
        fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink("/elsewhere/skills", &at).unwrap();
        let p = plugins(home.path(), Path::new("/s/maid/profile"));
        assert!(p
            .reap(".claude/skills", Kind::Link, false)
            .unwrap()
            .is_empty());
        assert!(exists_at(&at));
    }

    #[test]
    fn dry_run_reaps_nothing() {
        let home = TempDir::new().unwrap();
        let at = home.path().join(".claude/skills");
        fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink("/p/share/maid/skills", &at).unwrap();
        let p = plugins(home.path(), Path::new("/s/maid/profile"));
        assert_eq!(p.reap(".claude/skills", Kind::Link, true).unwrap().len(), 1);
        assert!(exists_at(&at));
    }

    /// The latest install's uninstall removes a link an older mAId install
    /// left, without --force, the same rule install uses.
    #[test]
    fn uninstall_removes_a_link_another_maid_install_left() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        let kiro = home.path().join(".kiro/steering/skills");
        fs::create_dir_all(kiro.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink("/old/checkout/resources/content/skills", &kiro).unwrap();
        uninstall(home.path(), profile.path(), false, false, Some(Agent::Kiro));
        assert!(!exists_at(&kiro));
    }

    #[test]
    fn a_marketplace_elsewhere_reads_as_wrong() {
        let root = Path::new("/s/marketplace");
        assert_eq!(marketplace_state(None, root), State::Missing);
        assert_eq!(
            marketplace_state(Some(root.into()), root),
            State::Ok(root.into())
        );
        assert!(matches!(
            marketplace_state(Some("/tmp/x/marketplace".into()), root),
            State::Wrong { .. }
        ));
    }

    #[test]
    fn a_path_through_a_symlinked_home_is_the_same_dir() {
        let real = TempDir::new().unwrap();
        let alias = TempDir::new().unwrap();
        let via = alias.path().join("home");
        std::os::unix::fs::symlink(real.path(), &via).unwrap();
        fs::create_dir_all(real.path().join("marketplace")).unwrap();
        assert!(same_dir(
            &real.path().join("marketplace"),
            &via.join("marketplace")
        ));
        assert!(!same_dir(&real.path().join("marketplace"), alias.path()));
        assert!(!same_dir(Path::new("/gone/a"), Path::new("/gone/b")));
    }
}
