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
//! one of the two, then has `BringIns` register the vendor marketplaces
//! with the plugin agents, and nothing in `stages` knows which.

use crate::install_record::{self, Dirs, Item, What};
use crate::shared::{
    agent_marketplace, claude_env, marketplace_root, on_path, selected_entries, Agent, BringIn,
    Entry, Kind, Link, LEGACY_LINKS, MARKETPLACE, PLUGIN_AGENTS, PLUGIN_ID,
    PROFILE_MARKETPLACE_DIR,
};
use anyhow::{anyhow, Context, Result};
use shell_claude::config_dir::ConfigDir;
use std::cell::RefCell;
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
    /// A mAId link install removes: an old location, or a skill this
    /// install does not ship.
    Legacy(PathBuf),
    /// An entry mAId did not write in a dir the agent owns; kept.
    Theirs,
    /// The agent's CLI would not report its plugins.
    Unreadable(String),
    /// A CLI call to place it failed.
    Failed(String),
    /// mAId's, kept because a plugin that is not mAId's still uses it.
    InUse,
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
            State::Theirs => "not mAId's (kept)".into(),
            State::Unreadable(why) => format!("unreadable ({why}); run: just install"),
            State::Failed(why) => format!("FAILED ({why})"),
            State::InUse => "in use by a plugin not mAId's (kept)".into(),
        }
    }
}

impl State {
    /// Why a location was skipped, for the part list: short, and naming
    /// no path, since the list outlives any checkout.
    pub fn reason(&self) -> String {
        match self {
            State::Wrong { .. } => "points elsewhere".into(),
            State::Occupied(what) => format!("a {what} is there"),
            State::Theirs => "not mAId's".into(),
            State::NoCli(cli) => format!("{cli} not on PATH"),
            State::SourceMissing => "source missing".into(),
            State::Missing => "missing".into(),
            State::Unreadable(_) => "unreadable".into(),
            State::Ok(_) | State::Current(_) | State::Legacy(_) => "in place".into(),
            State::Stale { .. } => "stale".into(),
            State::Disabled { .. } => "disabled".into(),
            State::Failed(_) => "failed".into(),
            State::InUse => "in use".into(),
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

    /// What of mAId's is on each selected agent now, and what install
    /// would place but could not, with why: the part list's entries.
    fn placed(&self, agent: Option<Agent>) -> Result<Vec<(Agent, Item)>>;
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
        // New links first: an install that fails keeps the old ones.
        let mut out = self.act(agent, |link, kind| self.create(link, kind, dry_run, force))?;
        out.extend(self.reap_unshipped(agent, dry_run)?);
        out.extend(self.reap_legacy(agent, dry_run)?);
        Ok(out)
    }

    fn uninstall(&self, agent: Option<Agent>, dry_run: bool, force: bool) -> Result<Vec<Report>> {
        let mut out = self.act(agent, |link, kind| self.remove(link, kind, dry_run, force))?;
        out.extend(self.reap_unshipped(agent, dry_run)?);
        out.extend(self.reap_legacy(agent, dry_run)?);
        Ok(out)
    }

    fn status(&self, agent: Option<Agent>) -> Result<Vec<Report>> {
        let mut out = self.act(agent, |link, _| Ok((self.inspect(link), false)))?;
        out.extend(self.reap_unshipped(agent, true)?);
        out.extend(self.reap_legacy(agent, true)?);
        Ok(out)
    }

    fn placed(&self, agent: Option<Agent>) -> Result<Vec<(Agent, Item)>> {
        let mut out = Vec::new();
        for entry in selected_entries(agent) {
            let (.., kind, a) = entry;
            for link in self.expand(entry)? {
                // A Link row is one dir link carrying every skill.
                let skills = match kind {
                    Kind::Link => skill_names(&link.1)?,
                    Kind::FanOut => link
                        .1
                        .file_name()
                        .map(|n| n.to_string_lossy().into())
                        .into_iter()
                        .collect(),
                };
                let skipped = match classify(kind, self.inspect(&link)) {
                    State::Ok(_) if exists(&link.1) => None,
                    State::Ok(_) | State::Missing | State::SourceMissing => continue,
                    other => Some(other.reason()),
                };
                out.extend(skills.into_iter().map(|s| {
                    let item = Item::placed(What::Skill, s);
                    (
                        a,
                        Item {
                            skipped: skipped.clone(),
                            ..item
                        },
                    )
                }));
            }
        }
        Ok(out)
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
    /// Remove mAId links in a FanOut dir to skills this profile does not
    /// ship (dropped, or only in another profile); a dry run lists them.
    fn reap_unshipped(&self, agent: Option<Agent>, dry_run: bool) -> Result<Vec<Report>> {
        let mut out = Vec::new();
        for (home_sub, source_sub, kind, _) in selected_entries(agent) {
            if kind != Kind::FanOut {
                continue;
            }
            let source = self.source.join(source_sub);
            for found in reap(&self.home, home_sub, kind, true)? {
                let at = self.home.join(&found.label);
                if !matches!(&found.state, State::Legacy(t) if ours(kind, t)) {
                    continue;
                }
                if at.file_name().is_some_and(|n| exists(&source.join(n))) {
                    continue;
                }
                if !dry_run {
                    fs::remove_file(&at)?;
                }
                out.push(found);
            }
        }
        Ok(out)
    }

    /// Remove the links an older mAId install left where the selected
    /// agents no longer read skills; a dry run lists them.
    fn reap_legacy(&self, agent: Option<Agent>, dry_run: bool) -> Result<Vec<Report>> {
        let mut out = Vec::new();
        for (a, home_sub, kind) in LEGACY_LINKS {
            if agent.is_none_or(|sel| sel == *a) {
                out.extend(reap(&self.home, home_sub, *kind, dry_run)?);
            }
        }
        Ok(out)
    }

    /// Resolve the selection to concrete locations and apply `f` to each,
    /// in a deterministic order so two runs are diffable.
    fn act(
        &self,
        agent: Option<Agent>,
        f: impl Fn(&Link, Kind) -> io::Result<(State, bool)>,
    ) -> Result<Vec<Report>> {
        let mut out = Vec::new();
        for entry in selected_entries(agent) {
            for link in self.expand(entry)? {
                let (state, acted) = f(&link, entry.2)?;
                let state = classify(entry.2, state);
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
    /// `Link` yields one; `FanOut` yields one per source child, sorted so
    /// two runs are diffable. Links to children no longer shipped are
    /// `reap_unshipped`'s.
    fn expand(&self, entry: Entry) -> io::Result<Vec<Link>> {
        let (home_sub, source_sub, kind, _agent) = entry;
        let home = self.home.join(home_sub);
        let source = self.source.join(source_sub);
        match kind {
            Kind::Link => Ok(vec![(home, source)]),
            Kind::FanOut if source.is_dir() => {
                let mut links: Vec<Link> = fs::read_dir(&source)?
                    .filter_map(Result::ok)
                    .map(|e| (home.join(e.file_name()), e.path()))
                    .collect();
                links.sort();
                Ok(links)
            }
            Kind::FanOut => Ok(vec![]),
        }
    }

    /// What is at a location now.
    fn inspect(&self, (home, source): &Link) -> State {
        // Inspect home first: a symlink already pointing at `source` is
        // ours to reap even if `source` is now gone. SourceMissing only when
        // there's nothing at home to act on.
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

    fn create(
        &self,
        link: &Link,
        kind: Kind,
        dry_run: bool,
        force: bool,
    ) -> io::Result<(State, bool)> {
        let (home, source) = link;
        let state = self.inspect(link);
        // Missing, or a symlink another mAId install left, is ours to set.
        // Any other symlink needs --force. Occupied is never actionable,
        // force or not: mAId does not overwrite a real file or directory.
        let act = match &state {
            State::Missing => true,
            State::Wrong { found, .. } => force_reaches(kind, force) || ours(kind, found),
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

    fn remove(
        &self,
        link: &Link,
        kind: Kind,
        dry_run: bool,
        force: bool,
    ) -> io::Result<(State, bool)> {
        let (home, _) = link;
        let state = self.inspect(link);
        let act = match &state {
            State::Ok(_) => true,
            // A link another mAId install left is ours, as on install.
            State::Wrong { found, .. } if ours(kind, found) => true,
            // --force reaps foreign symlinks and files, never a real
            // directory: mAId only ever creates symlinks, so a real dir at
            // a managed path belongs to the owning tool.
            State::Wrong { .. } | State::Occupied("file") if force_reaches(kind, force) => true,
            _ => false,
        };
        if act && !dry_run {
            fs::remove_file(home)?;
        }
        Ok((state, act))
    }
}

/// Every agent, each deployed the one way it supports, then the vendor
/// marketplaces mAId brings in, last so that their failing never stops
/// mAId's own skills.
pub struct Deployment {
    pub links: Symlinks,
    pub plugins: Plugins,
    pub bring_ins: BringIns,
}

impl Deploy for Deployment {
    fn install(&self, agent: Option<Agent>, dry_run: bool, force: bool) -> Result<Vec<Report>> {
        let mut out = self.links.install(agent, dry_run, force)?;
        out.extend(self.plugins.install(agent, dry_run, force)?);
        out.extend(self.bring_ins.install(agent, dry_run, force)?);
        Ok(out)
    }

    fn uninstall(&self, agent: Option<Agent>, dry_run: bool, force: bool) -> Result<Vec<Report>> {
        let mut out = self.links.uninstall(agent, dry_run, force)?;
        out.extend(self.plugins.uninstall(agent, dry_run, force)?);
        out.extend(self.bring_ins.uninstall(agent, dry_run, force)?);
        Ok(out)
    }

    fn status(&self, agent: Option<Agent>) -> Result<Vec<Report>> {
        let mut out = self.links.status(agent)?;
        out.extend(self.plugins.status(agent)?);
        out.extend(self.bring_ins.status(agent)?);
        Ok(out)
    }

    /// One call for every kind `stages` records, since it replaces them
    /// all per harness.
    fn placed(&self, agent: Option<Agent>) -> Result<Vec<(Agent, Item)>> {
        let mut out = self.links.placed(agent)?;
        out.extend(self.plugins.placed(agent)?);
        out.extend(self.bring_ins.placed(agent)?);
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
    /// Claude's config, which every `claude` started here is given.
    pub claude: ConfigDir,
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
        let mut out = Vec::new();
        if agents.iter().any(|(a, ..)| self.links_for(*a)) {
            out.push(self.ensure_root(dry_run)?);
        }
        for (a, legacy, kind) in agents {
            // The old links stay until the plugin is in: an agent whose CLI
            // is missing, or whose install fails, keeps its skills.
            if !on_path(a.cli()) {
                out.push(self.report(a, State::NoCli(a.cli()), false));
                continue;
            }
            self.repair(a, dry_run)?;
            let state = self.state(a, Some(&want))?;
            let (steps, acted) =
                install_steps(a, &state, self.recorded(a)?.as_deref(), &self.root(a));
            if !dry_run {
                for step in &steps {
                    self.cli(a, step)?;
                }
                self.check_installed(a, &want, acted)?;
            }
            out.push(self.report(a, state, acted));
            out.extend(reap(&self.home, legacy, kind, dry_run)?);
        }
        Ok(out)
    }

    fn uninstall(&self, agent: Option<Agent>, dry_run: bool, _force: bool) -> Result<Vec<Report>> {
        let mut out = Vec::new();
        for (a, legacy, kind) in plugin_agents(agent) {
            out.extend(reap(&self.home, legacy, kind, dry_run)?);
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
        if plugin_agents(agent)
            .iter()
            .any(|(a, ..)| self.links_for(*a))
        {
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
            out.extend(reap(&self.home, legacy, kind, true)?);
            let state = self.state(a, want.as_deref()).unwrap_or_else(unreadable);
            let has_cli = !matches!(state, State::NoCli(_));
            out.push(self.report(a, state, false));
            if has_cli {
                out.push(Report {
                    label: format!("{} marketplace {MARKETPLACE}", a.name()),
                    state: self
                        .recorded(a)
                        .map(|r| marketplace_state(r, &self.root(a)))
                        .unwrap_or_else(unreadable),
                    acted: false,
                });
            }
        }
        Ok(out)
    }

    /// Read back from each CLI's own listings.
    fn placed(&self, agent: Option<Agent>) -> Result<Vec<(Agent, Item)>> {
        let mut out = Vec::new();
        for (a, ..) in plugin_agents(agent) {
            if !on_path(a.cli()) {
                let why = State::NoCli(a.cli()).reason();
                out.push((
                    a,
                    Item::skipped(What::Marketplace, MARKETPLACE, why.clone()),
                ));
                out.push((a, Item::skipped(What::Plugin, PLUGIN_ID, why)));
                continue;
            }
            match marketplace_state(self.recorded(a)?, &self.root(a)) {
                State::Ok(_) => out.push((a, Item::placed(What::Marketplace, MARKETPLACE))),
                State::Missing => {}
                other => out.push((
                    a,
                    Item::skipped(What::Marketplace, MARKETPLACE, other.reason()),
                )),
            }
            if self.state(a, None)? != State::Missing {
                out.push((a, Item::placed(What::Plugin, PLUGIN_ID)));
            }
        }
        Ok(out)
    }

    fn is_deployed(&self, agent: Agent) -> bool {
        let Ok(want) = self.want() else {
            return false;
        };
        matches!(self.state(agent, Some(&want)), Ok(State::Current(_)))
            && self
                .recorded(agent)
                .is_ok_and(|r| r == Some(self.root(agent)))
    }
}

impl Plugins {
    /// Where `agent`'s `maid` marketplace points.
    fn root(&self, agent: Agent) -> PathBuf {
        agent_marketplace(agent, &self.profile)
    }

    /// The real dir of links into the profile, for the agents that need one.
    fn links(&self) -> PathBuf {
        marketplace_root(&self.profile)
    }

    /// Whether `agent`'s marketplace is the links dir.
    fn links_for(&self, agent: Agent) -> bool {
        self.root(agent) == self.links()
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

    fn cli(&self, agent: Agent, args: &[String]) -> Result<String> {
        run_cli(&self.home, &self.claude, agent, args)
    }

    fn state(&self, agent: Agent, want: Option<&str>) -> Result<State> {
        if !on_path(agent.cli()) {
            return Ok(State::NoCli(agent.cli()));
        }
        let json = self.cli(agent, &strings(&["plugin", "list", "--json"]))?;
        Ok(plugin_state(parse_listed(agent, &json, PLUGIN_ID)?, want))
    }

    /// Where the agent's `maid` marketplace points, if it has one. codex
    /// records the resolved path, so one that resolves to the root is the
    /// root (HOME itself may sit behind a symlink).
    fn recorded(&self, agent: Agent) -> Result<Option<PathBuf>> {
        let json = self.cli(
            agent,
            &strings(&["plugin", "marketplace", "list", "--json"]),
        )?;
        let root = self.root(agent);
        Ok(
            parse_recorded(agent, &json, MARKETPLACE)?.map(|found| match same_dir(&found, &root) {
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
        if installed && recorded.as_deref() == Some(self.root(agent).as_path()) {
            return Ok(());
        }
        Err(anyhow!(
            "{} plugin {PLUGIN_ID} is {} after install, its marketplace {}",
            agent.name(),
            after.describe(),
            marketplace_state(recorded, &self.root(agent)).describe()
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
        let root = self.links();
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
        let root = self.links();
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

    fn label(&self, path: &Path) -> String {
        path.strip_prefix(&self.home)
            .unwrap_or(path)
            .display()
            .to_string()
    }
}

/// The vendor marketplaces in shared.rs `BRING_INS` and their plugins,
/// placed through each plugin agent's own CLI. Neither CLI records who
/// added a marketplace, so an item is mAId's when mAId's part list names
/// it as placed, or this run added it. Anything else is the user's and is
/// left as it is.
pub struct BringIns {
    pub home: PathBuf,
    /// Claude's config, which every `claude` started here is given.
    pub claude: ConfigDir,
    pub list: &'static [BringIn],
    owned: RefCell<Vec<(Agent, What, String)>>,
    failed: RefCell<Vec<(Agent, What, String)>>,
    /// Why the part list could not be read, when ownership is unknown.
    refused: Option<String>,
}

/// Whose an item on an agent is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Held {
    Absent,
    Ours,
    Theirs,
}

/// One agent's view of one bring-in, read from its CLI.
struct Seen {
    marketplace: Held,
    path: Option<PathBuf>,
    plugins: Vec<(String, Held, Option<Listed>)>,
    /// A plugin from the marketplace that is not mAId's is installed.
    others: bool,
}

/// One CLI call and the item it places or removes.
type Step = (What, String, Vec<String>);

impl Deploy for BringIns {
    fn install(&self, agent: Option<Agent>, dry_run: bool, _force: bool) -> Result<Vec<Report>> {
        let mut out = Vec::new();
        for (a, ..) in plugin_agents(agent) {
            if !on_path(a.cli()) {
                continue;
            }
            for b in self.list {
                let seen = match self.seen(a, b) {
                    Ok(seen) => seen,
                    Err(e) => {
                        out.push(self.fail(a, What::Marketplace, b.marketplace, &e));
                        continue;
                    }
                };
                let held: Vec<_> = seen
                    .plugins
                    .iter()
                    .map(|(id, h, _)| (id.clone(), *h))
                    .collect();
                let steps = install_steps_for(a, b, seen.marketplace, &held);
                let mut stopped = false;
                for (kind, name, args) in &steps {
                    if stopped {
                        out.push(self.fail(
                            a,
                            *kind,
                            name,
                            &anyhow!("its marketplace was not added"),
                        ));
                        continue;
                    }
                    let done = match dry_run {
                        true => Ok(String::new()),
                        false => run_cli(&self.home, &self.claude, a, args),
                    };
                    match done {
                        Ok(_) => {
                            if !dry_run {
                                self.own(a, *kind, name);
                            }
                            out.push(self.report(a, *kind, name, State::Missing, true));
                        }
                        Err(e) => {
                            out.push(self.fail(a, *kind, name, &e));
                            // No plugin installs from a marketplace that is not there.
                            stopped = *kind == What::Marketplace;
                        }
                    }
                }
                let acted =
                    |kind: What, name: &str| steps.iter().any(|(k, n, _)| *k == kind && n == name);
                if !acted(What::Marketplace, b.marketplace) {
                    out.push(self.report(
                        a,
                        What::Marketplace,
                        b.marketplace,
                        seen.mkt_state(),
                        false,
                    ));
                }
                for (id, held, listed) in seen.plugins {
                    if !acted(What::Plugin, &id) {
                        out.push(self.report(
                            a,
                            What::Plugin,
                            &id,
                            plugin_held_state(held, listed),
                            false,
                        ));
                    }
                }
            }
        }
        Ok(out)
    }

    fn uninstall(&self, agent: Option<Agent>, dry_run: bool, _force: bool) -> Result<Vec<Report>> {
        let mut out = Vec::new();
        for (a, ..) in plugin_agents(agent) {
            if !on_path(a.cli()) {
                continue;
            }
            for b in self.list {
                let seen = match self.seen(a, b) {
                    Ok(seen) => seen,
                    Err(e) => {
                        out.push(self.fail(a, What::Marketplace, b.marketplace, &e));
                        continue;
                    }
                };
                let held: Vec<_> = seen
                    .plugins
                    .iter()
                    .map(|(id, h, _)| (id.clone(), *h))
                    .collect();
                for (kind, name, args) in
                    uninstall_steps_for(a, b, seen.marketplace, &held, seen.others)
                {
                    let done = match dry_run {
                        true => Ok(String::new()),
                        false => run_cli(&self.home, &self.claude, a, &args),
                    };
                    out.push(match done {
                        Ok(_) => {
                            if !dry_run {
                                self.disown(a, kind, &name);
                            }
                            // Any listed state reads as "removed" once acted on.
                            let was = seen
                                .plugins
                                .iter()
                                .find(|(id, ..)| *id == name)
                                .map(|(_, _, l)| plugin_state(l.clone(), None))
                                .unwrap_or_else(|| State::Current(String::new()));
                            self.report(a, kind, &name, was, true)
                        }
                        Err(e) => self.fail(a, kind, &name, &e),
                    });
                }
                match seen.marketplace {
                    Held::Ours if seen.others => out.push(self.report(
                        a,
                        What::Marketplace,
                        b.marketplace,
                        State::InUse,
                        false,
                    )),
                    Held::Theirs => out.push(self.report(
                        a,
                        What::Marketplace,
                        b.marketplace,
                        State::Theirs,
                        false,
                    )),
                    _ => {}
                }
                for (id, held, _) in &seen.plugins {
                    if *held == Held::Theirs {
                        out.push(self.report(a, What::Plugin, id, State::Theirs, false));
                    }
                }
            }
        }
        Ok(out)
    }

    fn status(&self, agent: Option<Agent>) -> Result<Vec<Report>> {
        let mut out: Vec<Report> = self
            .refused
            .iter()
            .map(|why| Report {
                label: "part list".into(),
                state: State::Failed(format!("{why}; mAId's bring-ins read as not mAId's")),
                acted: false,
            })
            .collect();
        let agents = plugin_agents(agent);
        for (a, ..) in &agents {
            if !on_path(a.cli()) {
                continue;
            }
            for b in self.list {
                match self.seen(*a, b) {
                    Ok(seen) => {
                        out.push(self.report(
                            *a,
                            What::Marketplace,
                            b.marketplace,
                            seen.mkt_state(),
                            false,
                        ));
                        for (id, held, listed) in seen.plugins {
                            out.push(self.report(
                                *a,
                                What::Plugin,
                                &id,
                                plugin_held_state(held, listed),
                                false,
                            ));
                        }
                    }
                    Err(e) => out.push(self.report(
                        *a,
                        What::Marketplace,
                        b.marketplace,
                        State::Unreadable(one_line(&e)),
                        false,
                    )),
                }
            }
        }
        if !agents.is_empty() {
            out.extend(missing_needs(self.list, on_path));
        }
        Ok(out)
    }

    fn placed(&self, agent: Option<Agent>) -> Result<Vec<(Agent, Item)>> {
        let mut out = Vec::new();
        for &a in Agent::ALL
            .iter()
            .filter(|a| agent.is_none_or(|sel| sel == **a))
        {
            for b in self.list {
                let items = || {
                    std::iter::once((What::Marketplace, b.marketplace.to_string()))
                        .chain(b.plugins.iter().map(|p| (What::Plugin, b.plugin_id(p))))
                };
                let unlisted = |why: String| {
                    items()
                        .map(|(kind, name)| {
                            let ours = self.ours(a, kind, &name);
                            (a, unlisted_item(kind, &name, ours, why.clone()))
                        })
                        .collect::<Vec<_>>()
                };
                if !a.is_plugin() {
                    out.extend(unlisted(NO_MARKETPLACE.into()));
                    continue;
                }
                if !on_path(a.cli()) {
                    out.extend(unlisted(State::NoCli(a.cli()).reason()));
                    continue;
                }
                // One vendor's listing must not stop the whole part list.
                let seen = match self.seen(a, b) {
                    Ok(seen) => seen,
                    Err(_) => {
                        out.extend(unlisted(State::Unreadable(String::new()).reason()));
                        continue;
                    }
                };
                let held = std::iter::once(seen.marketplace)
                    .chain(seen.plugins.iter().map(|(_, h, _)| *h));
                for ((kind, name), held) in items().zip(held) {
                    let failed = self.failed.borrow().contains(&(a, kind, name.clone()));
                    if let Some(item) = placed_item(kind, &name, held, failed) {
                        out.push((a, item));
                    }
                }
            }
        }
        Ok(out)
    }

    /// Smoke checks mAId's own skills; a vendor's are not under test.
    fn is_deployed(&self, _: Agent) -> bool {
        false
    }
}

/// Why an agent with no plugin CLI gets no bring-in.
const NO_MARKETPLACE: &str = "no plugin marketplace";

impl BringIns {
    /// Ownership is read once from mAId's part list in `parts`.
    pub fn new(
        home: PathBuf,
        claude: ConfigDir,
        list: &'static [BringIn],
        parts: &(impl Dirs + ?Sized),
    ) -> Result<BringIns> {
        let mut owned = Vec::new();
        for (a, ..) in PLUGIN_AGENTS {
            owned.extend(
                install_record::placed(parts, *a)?
                    .into_iter()
                    .map(|i| (*a, i.kind, i.name)),
            );
        }
        Ok(BringIns {
            home,
            claude,
            list,
            owned: RefCell::new(owned),
            failed: RefCell::default(),
            refused: None,
        })
    }

    /// For reporting only: a part list that cannot be read leaves nothing
    /// owned, and `status` says so instead of stopping.
    pub fn reporting(
        home: PathBuf,
        claude: ConfigDir,
        list: &'static [BringIn],
        parts: &(impl Dirs + ?Sized),
    ) -> BringIns {
        BringIns::new(home.clone(), claude.clone(), list, parts).unwrap_or_else(|e| BringIns {
            home,
            claude,
            list,
            owned: RefCell::default(),
            failed: RefCell::default(),
            refused: Some(one_line(&e)),
        })
    }

    fn ours(&self, agent: Agent, kind: What, name: &str) -> bool {
        self.owned
            .borrow()
            .iter()
            .any(|(a, k, n)| (*a, *k, n.as_str()) == (agent, kind, name))
    }

    /// What uninstall removed is no longer mAId's, even if the listing
    /// after it cannot be read.
    fn disown(&self, agent: Agent, kind: What, name: &str) {
        self.owned
            .borrow_mut()
            .retain(|(a, k, n)| (*a, *k, n.as_str()) != (agent, kind, name));
    }

    fn own(&self, agent: Agent, kind: What, name: &str) {
        self.owned
            .borrow_mut()
            .push((agent, kind, name.to_string()));
    }

    fn seen(&self, agent: Agent, b: &BringIn) -> Result<Seen> {
        let plugins = run_cli(
            &self.home,
            &self.claude,
            agent,
            &strings(&["plugin", "list", "--json"]),
        )?;
        let markets = run_cli(
            &self.home,
            &self.claude,
            agent,
            &strings(&["plugin", "marketplace", "list", "--json"]),
        )?;
        let held =
            |present: bool, kind: What, name: &str| match (present, self.ours(agent, kind, name)) {
                (false, _) => Held::Absent,
                (true, true) => Held::Ours,
                (true, false) => Held::Theirs,
            };
        let path = parse_cloned(agent, &markets, b.marketplace)?;
        let ids: Vec<String> = b.plugins.iter().map(|p| b.plugin_id(p)).collect();
        let listed = ids
            .iter()
            .map(|id| {
                let l = parse_listed(agent, &plugins, id)?;
                Ok((id.clone(), held(l.is_some(), What::Plugin, id), l))
            })
            .collect::<Result<Vec<_>>>()?;
        let from_it = format!("@{}", b.marketplace);
        let others = listed_ids(agent, &plugins)?.iter().any(|(id, user)| {
            id.ends_with(&from_it) && !(*user && self.ours(agent, What::Plugin, id))
        });
        Ok(Seen {
            marketplace: held(path.is_some(), What::Marketplace, b.marketplace),
            path,
            plugins: listed,
            others,
        })
    }

    fn report(&self, agent: Agent, kind: What, name: &str, state: State, acted: bool) -> Report {
        let what = match kind {
            What::Marketplace => "marketplace",
            _ => "plugin",
        };
        Report {
            label: format!("{} {what} {name}", agent.name()),
            state,
            acted,
        }
    }

    fn fail(&self, agent: Agent, kind: What, name: &str, e: &anyhow::Error) -> Report {
        self.failed
            .borrow_mut()
            .push((agent, kind, name.to_string()));
        self.report(agent, kind, name, State::Failed(one_line(e)), false)
    }
}

impl Seen {
    fn mkt_state(&self) -> State {
        match (self.marketplace, &self.path) {
            (Held::Ours, Some(p)) => State::Ok(p.clone()),
            (Held::Theirs, _) => State::Theirs,
            _ => State::Missing,
        }
    }
}

/// An error on one line, as status prints one location per line.
fn one_line(e: &anyhow::Error) -> String {
    format!("{e:#}")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn plugin_held_state(held: Held, listed: Option<Listed>) -> State {
    match held {
        Held::Theirs => State::Theirs,
        _ => plugin_state(listed, None),
    }
}

/// The part-list entry for one bring-in item, if it has one.
fn placed_item(kind: What, name: &str, held: Held, failed: bool) -> Option<Item> {
    match held {
        Held::Ours => Some(Item::placed(kind, name)),
        Held::Theirs => Some(Item::skipped(kind, name, State::Theirs.reason())),
        Held::Absent if failed => Some(Item::skipped(kind, name, "failed")),
        Held::Absent => None,
    }
}

/// The part-list entry for an item whose listing was not seen (no CLI, or
/// a listing that cannot be read): what mAId placed stays its own, so a
/// later run can still remove it.
fn unlisted_item(kind: What, name: &str, ours: bool, why: String) -> Item {
    match ours {
        true => Item::placed(kind, name),
        false => Item::skipped(kind, name, why),
    }
}

/// The CLI calls that place one bring-in on an agent: the marketplace if
/// nothing by its name is there (codex pinned to the commit), then each
/// plugin not there.
fn install_steps_for(
    agent: Agent,
    b: &BringIn,
    marketplace: Held,
    plugins: &[(String, Held)],
) -> Vec<Step> {
    let add = match agent {
        Agent::Codex => strings(&["plugin", "marketplace", "add", b.source, "--ref", b.rev]),
        _ => strings(&["plugin", "marketplace", "add", b.source]),
    };
    let install = |id: &str| match agent {
        Agent::Claude => strings(&["plugin", "install", id, "--scope", "user"]),
        _ => strings(&["plugin", "add", id]),
    };
    (marketplace == Held::Absent)
        .then(|| (What::Marketplace, b.marketplace.to_string(), add))
        .into_iter()
        .chain(
            plugins
                .iter()
                .filter(|(_, h)| *h == Held::Absent)
                .map(|(id, _)| (What::Plugin, id.clone(), install(id))),
        )
        .collect()
}

/// The CLI calls that remove what mAId placed of one bring-in: its
/// plugins, then its marketplace unless a plugin not mAId's uses it
/// (removing a marketplace also uninstalls every plugin from it).
fn uninstall_steps_for(
    agent: Agent,
    b: &BringIn,
    marketplace: Held,
    plugins: &[(String, Held)],
    others: bool,
) -> Vec<Step> {
    let remove = |id: &str| match agent {
        Agent::Claude => strings(&["plugin", "uninstall", id, "--scope", "user"]),
        _ => strings(&["plugin", "remove", id]),
    };
    plugins
        .iter()
        .filter(|(_, h)| *h == Held::Ours)
        .map(|(id, _)| (What::Plugin, id.clone(), remove(id)))
        .chain((marketplace == Held::Ours && !others).then(|| {
            (
                What::Marketplace,
                b.marketplace.to_string(),
                strings(&["plugin", "marketplace", "remove", b.marketplace]),
            )
        }))
        .collect()
}

/// One status line per command a bring-in's skills run that `found` does
/// not find.
fn missing_needs(list: &[BringIn], found: impl Fn(&str) -> bool) -> Vec<Report> {
    list.iter()
        .flat_map(|b| {
            let label = format!("{} needs", b.plugins.join(", "));
            b.needs.iter().filter(|c| !found(c)).map(move |c| Report {
                label: label.clone(),
                state: State::NoCli(c),
                acted: false,
            })
        })
        .collect()
}

/// Remove the mAId-shaped links at a path (Link) or under it (FanOut),
/// matched by `is_maids`; a dry run lists them.
fn reap(home: &Path, home_sub: &str, kind: Kind, dry_run: bool) -> Result<Vec<Report>> {
    let at = home.join(home_sub);
    let candidates = match kind {
        Kind::Link => vec![at],
        Kind::FanOut => match fs::read_dir(&at) {
            Ok(entries) => {
                let mut v: Vec<PathBuf> = entries.filter_map(|e| Some(e.ok()?.path())).collect();
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
            label: link
                .strip_prefix(home)
                .unwrap_or(&link)
                .display()
                .to_string(),
            state: State::Legacy(target),
            acted: true,
        });
    }
    Ok(out)
}

/// What a managed location's state means for its row: in a FanOut dir,
/// anything not mAId's is the user's.
fn classify(kind: Kind, state: State) -> State {
    match (kind, state) {
        (Kind::FanOut, State::Wrong { found, .. }) if !ours(Kind::FanOut, &found) => State::Theirs,
        (Kind::FanOut, State::Occupied(_)) => State::Theirs,
        (_, state) => state,
    }
}

/// The skills a profile skills dir ships, sorted.
fn skill_names(dir: &Path) -> io::Result<Vec<String>> {
    let mut names: Vec<String> = match fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|e| Some(e.ok()?.file_name().to_string_lossy().into_owned()))
            .collect(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => vec![],
        Err(e) => return Err(e),
    };
    names.sort();
    Ok(names)
}

/// `--force` reaches a Link row's one path, which is mAId's. A FanOut dir
/// is the agent's, so a foreign entry there is kept even with `--force`.
fn force_reaches(kind: Kind, force: bool) -> bool {
    force && kind == Kind::Link
}

/// Whether a link at a managed location is mAId's to replace or remove.
/// A FanOut dir is shared with the user, and mAId only ever linked
/// profile skills there, so a checkout-shaped link in it is the user's.
fn ours(kind: Kind, target: &Path) -> bool {
    match kind {
        Kind::Link => is_maids(target),
        Kind::FanOut => target.parent().is_some_and(|p| p.ends_with(PROFILE_SKILLS)),
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

/// The entries of a plugin agent's `plugin list --json`, and the key
/// that names each plugin. `None` for a shape this does not know.
pub fn listed_plugins(
    agent: Agent,
    v: &serde_json::Value,
) -> Option<(&Vec<serde_json::Value>, &'static str)> {
    match agent {
        Agent::Claude => v.as_array().map(|e| (e, "id")),
        _ => v["installed"].as_array().map(|e| (e, "pluginId")),
    }
}

/// The entries of a plugin agent's `plugin marketplace list --json`, each
/// with a `name`, and the key that holds where it was added from.
pub fn listed_marketplaces(
    agent: Agent,
    v: &serde_json::Value,
) -> Option<(&Vec<serde_json::Value>, &'static str)> {
    match agent {
        Agent::Claude => v.as_array().map(|e| (e, "path")),
        _ => v["marketplaces"].as_array().map(|e| (e, "root")),
    }
}

/// An agent's CLI with `home` as HOME; claude also gets `claude`'s config dir.
fn cli_command(home: &Path, claude: &ConfigDir, agent: Agent) -> Command {
    let mut cmd = Command::new(agent.cli());
    cmd.env("HOME", home);
    if agent == Agent::Claude {
        claude_env(&mut cmd, claude);
    }
    cmd
}

/// Run an agent's CLI (see `cli_command`), returning stdout.
fn run_cli(home: &Path, claude: &ConfigDir, agent: Agent, args: &[String]) -> Result<String> {
    let out = cli_command(home, claude, agent)
        .args(args)
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

/// The entries of an agent's `plugin list --json`, and the key each
/// names its plugin by.
fn plugin_entries(agent: Agent, json: &str) -> Result<(Vec<serde_json::Value>, &'static str)> {
    let v: serde_json::Value = serde_json::from_str(json)
        .with_context(|| format!("{} plugin list --json: not JSON", agent.cli()))?;
    let (entries, id_key) = listed_plugins(agent, &v)
        .ok_or_else(|| anyhow!("{} plugin list --json: unexpected shape", agent.cli()))?;
    Ok((entries.clone(), id_key))
}

/// Plugin `id` in an agent's `plugin list --json`. claude lists one entry
/// per scope; mAId installs at user scope.
fn parse_listed(agent: Agent, json: &str, id: &str) -> Result<Option<Listed>> {
    let (entries, id_key) = plugin_entries(agent, json)?;
    let ours = entries
        .iter()
        .find(|e| e[id_key] == id && (agent != Agent::Claude || e["scope"] == "user"));
    ours.map(|e| {
        Ok(Listed {
            version: e["version"]
                .as_str()
                .ok_or_else(|| anyhow!("{id} listed with no version"))?
                .to_string(),
            enabled: e["enabled"]
                .as_bool()
                .ok_or_else(|| anyhow!("{id} listed with no enabled flag"))?,
        })
    })
    .transpose()
}

/// Every plugin id an agent lists, in any scope, and whether it is at the
/// scope mAId installs at (claude's user scope; codex has one).
fn listed_ids(agent: Agent, json: &str) -> Result<Vec<(String, bool)>> {
    let (entries, id_key) = plugin_entries(agent, json)?;
    Ok(entries
        .iter()
        .filter_map(|e| {
            let user = agent != Agent::Claude || e["scope"] == "user";
            e[id_key].as_str().map(|id| (id.to_string(), user))
        })
        .collect())
}

/// The path an agent's local marketplace `name` was added from.
fn parse_recorded(agent: Agent, json: &str, name: &str) -> Result<Option<PathBuf>> {
    marketplace_path(agent, json, name, &["path"])
}

/// Where an agent keeps marketplace `name`: its clone of a git one, or
/// the dir a local one (the user's, under the same name) was added from.
fn parse_cloned(agent: Agent, json: &str, name: &str) -> Result<Option<PathBuf>> {
    marketplace_path(agent, json, name, &["installLocation", "path"])
}

/// The path an agent lists for marketplace `name`; codex lists one, claude
/// one per kind of source.
fn marketplace_path(
    agent: Agent,
    json: &str,
    name: &str,
    claude_keys: &[&str],
) -> Result<Option<PathBuf>> {
    let v: serde_json::Value = serde_json::from_str(json)
        .with_context(|| format!("{} plugin marketplace list --json: not JSON", agent.cli()))?;
    let (entries, key) = listed_marketplaces(agent, &v).ok_or_else(|| {
        anyhow!(
            "{} plugin marketplace list --json: unexpected shape",
            agent.cli()
        )
    })?;
    let codex_key = [key];
    let path_keys = match agent {
        Agent::Claude => claude_keys,
        _ => &codex_key[..],
    };
    Ok(entries
        .iter()
        .find(|e| e["name"] == name)
        .and_then(|e| path_keys.iter().find_map(|k| e[*k].as_str()))
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

    fn placed(&self, _: Option<Agent>) -> Result<Vec<(Agent, Item)>> {
        Ok(vec![])
    }
}

fn exists(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok()
}

const PROFILE_SKILLS: &str = "share/maid/skills";

/// Whether a link target is a mAId skills tree, or a skill in one: a
/// profile's, or a checkout's from before the profile. Matched by path
/// shape, not provenance.
fn is_maids(target: &Path) -> bool {
    let roots = [PROFILE_SKILLS, "resources/content/skills"];
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
        for skill in ["kdevkit", "notes"] {
            assert_eq!(
                fs::read_link(home.path().join(".kiro/skills").join(skill)).unwrap(),
                profile.path().join("share/maid/skills").join(skill)
            );
        }
        assert!(exists_at(&home.path().join(".gemini/config/skills")));
        // Plugin agents get no links.
        assert!(!exists_at(&home.path().join(".claude/skills")));
        assert!(!exists_at(&home.path().join(".codex/skills")));
    }

    fn placed(home: &Path, profile: &Path) -> Vec<(Agent, Item)> {
        sym(home, profile).placed(None).unwrap()
    }

    fn skill(name: &str) -> Item {
        Item::placed(What::Skill, name)
    }

    #[test]
    fn placed_lists_every_shipped_skill_for_each_link_agent() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        // The user's own skill, at a name mAId does not ship.
        write(&home.path().join(".kiro/skills/mine/SKILL.md"), "mine");
        install(home.path(), profile.path(), false, false, None);
        let by = |(a, i): &(Agent, Item)| (a.cli(), i.clone());
        let mut got = placed(home.path(), profile.path());
        got.sort_by_key(by);
        let want = [Agent::Kiro, Agent::Agy]
            .into_iter()
            .flat_map(|a| [(a, skill("kdevkit")), (a, skill("notes"))]);
        let mut want: Vec<_> = want.collect();
        want.sort_by_key(by);
        assert_eq!(got, want);
    }

    #[test]
    fn a_location_install_left_alone_is_placed_as_skipped() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        write(&home.path().join(".kiro/skills/notes/SKILL.md"), "mine");
        write(&home.path().join(".gemini/config/skills/x"), "mine");
        install(home.path(), profile.path(), false, false, None);
        let got = placed(home.path(), profile.path());
        let skipped = |a, name: &str| {
            got.iter()
                .find(|(g, i)| *g == a && i.name == name)
                .and_then(|(_, i)| i.skipped.clone())
        };
        assert_eq!(skipped(Agent::Kiro, "notes").as_deref(), Some("not mAId's"));
        assert_eq!(skipped(Agent::Kiro, "kdevkit"), None);
        for name in ["kdevkit", "notes"] {
            assert_eq!(skipped(Agent::Agy, name).as_deref(), Some("a dir is there"));
        }
    }

    #[test]
    fn a_skip_past_a_foreign_link_names_no_path() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        let elsewhere = home.path().join("elsewhere");
        fs::create_dir_all(&elsewhere).unwrap();
        fs::create_dir_all(home.path().join(".gemini/config")).unwrap();
        std::os::unix::fs::symlink(&elsewhere, home.path().join(".gemini/config/skills")).unwrap();
        install(home.path(), profile.path(), false, false, None);
        let reasons: Vec<_> = placed(home.path(), profile.path())
            .into_iter()
            .filter(|(a, _)| *a == Agent::Agy)
            .map(|(_, i)| i.skipped)
            .collect();
        assert_eq!(reasons, vec![Some("points elsewhere".to_string()); 2]);
    }

    #[test]
    fn nothing_is_placed_after_uninstall() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        install(home.path(), profile.path(), false, false, None);
        uninstall(home.path(), profile.path(), false, false, None);
        assert_eq!(placed(home.path(), profile.path()), vec![]);
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
        assert!(!exists_at(&home.path().join(".kiro/skills")));
    }

    /// A real file at a managed path is never clobbered without --force.
    #[test]
    fn a_users_own_file_survives_install() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        let claimed = home.path().join(".gemini/config/skills");
        write(&claimed, "mine");
        install(home.path(), profile.path(), false, false, None);
        assert_eq!(fs::read_to_string(&claimed).unwrap(), "mine");
    }

    #[test]
    fn a_foreign_symlink_is_replaced_only_with_force() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        let managed = home.path().join(".gemini/config/skills");
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
        let kiro = home.path().join(".kiro/skills/kdevkit");
        let agy = home.path().join(".gemini/config/skills");
        for link in [&kiro, &agy] {
            fs::create_dir_all(link.parent().unwrap()).unwrap();
        }
        std::os::unix::fs::symlink(old.path().join("share/maid/skills/kdevkit"), &kiro).unwrap();
        std::os::unix::fs::symlink(old.path().join("share/maid/skills"), &agy).unwrap();

        install(home.path(), profile.path(), false, false, None);
        let skills = profile.path().join("share/maid/skills");
        assert_eq!(fs::read_link(&kiro).unwrap(), skills.join("kdevkit"));
        assert_eq!(fs::read_link(&agy).unwrap(), skills);
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
        let theirs = home.path().join(".kiro/skills/their-own/SKILL.md");
        write(&theirs, "theirs");

        uninstall(home.path(), profile.path(), false, false, None);
        assert!(!exists_at(&home.path().join(".kiro/skills/kdevkit")));
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
        let theirs = home.path().join(".gemini/config/skills");
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
        assert!(!exists_at(&home.path().join(".kiro/skills")));

        install(home.path(), profile.path(), false, false, None);
        let after = status(home.path(), profile.path(), None);
        assert!(after.iter().all(|r| matches!(r.state, State::Ok(_))));
    }

    #[test]
    fn a_scoped_install_touches_only_that_agent() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        install(home.path(), profile.path(), false, false, Some(Agent::Kiro));
        assert!(exists_at(&home.path().join(".kiro/skills/kdevkit")));
        assert!(!exists_at(&home.path().join(".gemini/config/skills")));
    }

    #[test]
    fn a_scoped_uninstall_leaves_other_agents_deployed() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        install(home.path(), profile.path(), false, false, None);
        uninstall(home.path(), profile.path(), false, false, Some(Agent::Kiro));
        assert!(!exists_at(&home.path().join(".kiro/skills/kdevkit")));
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
        let claimed = home.path().join(".gemini/config/skills");
        write(&claimed, "mine");

        let reports = install(home.path(), profile.path(), false, true, None);
        let this = reports
            .iter()
            .find(|r| r.label == ".gemini/config/skills")
            .unwrap();
        assert!(
            !this.acted,
            "create() must not act on a real file, force or not"
        );
        assert_eq!(fs::read_to_string(&claimed).unwrap(), "mine");
    }

    /// kiro's skills dir is the user's too: their skills stay, and the
    /// steering link an older install left goes.
    #[test]
    fn kiro_install_reaps_the_old_steering_link_and_keeps_the_users_skills() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        let h = home.path();
        let own = h.join(".kiro/skills/my-own/SKILL.md");
        let steering_doc = h.join(".kiro/steering/notes.md");
        write(&own, "mine");
        write(&steering_doc, "mine");
        let old = h.join(".kiro/steering/skills");
        std::os::unix::fs::symlink(profile.path().join("share/maid/skills"), &old).unwrap();

        let reports = install(h, profile.path(), false, false, Some(Agent::Kiro));
        let reaped = reports
            .iter()
            .find(|r| r.label == ".kiro/steering/skills")
            .unwrap();
        assert!(matches!(reaped.state, State::Legacy(_)) && reaped.acted);
        assert!(!exists_at(&old));
        assert!(exists_at(&h.join(".kiro/skills/kdevkit")));
        assert!(exists_at(&own) && exists_at(&steering_doc));

        std::os::unix::fs::symlink(profile.path().join("share/maid/skills"), &old).unwrap();
        uninstall(h, profile.path(), false, false, Some(Agent::Kiro));
        assert!(!exists_at(&h.join(".kiro/skills/kdevkit")));
        assert!(!exists_at(&old));
        assert_eq!(fs::read_to_string(&own).unwrap(), "mine");
    }

    #[test]
    fn a_foreign_steering_skills_link_is_kept() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        let at = home.path().join(".kiro/steering/skills");
        fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink("/elsewhere/skills", &at).unwrap();
        install(home.path(), profile.path(), false, false, Some(Agent::Kiro));
        assert_eq!(fs::read_link(&at).unwrap(), Path::new("/elsewhere/skills"));
    }

    /// Status and a dry run list the old link and leave it.
    #[test]
    fn status_lists_the_old_steering_link_without_removing_it() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        let old = home.path().join(".kiro/steering/skills");
        fs::create_dir_all(old.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink("/old/checkout/resources/content/skills", &old).unwrap();
        for reports in [
            status(home.path(), profile.path(), Some(Agent::Kiro)),
            install(home.path(), profile.path(), true, false, Some(Agent::Kiro)),
        ] {
            assert!(
                reports
                    .iter()
                    .any(|r| r.label == ".kiro/steering/skills"
                        && matches!(r.state, State::Legacy(_)))
            );
        }
        assert!(exists_at(&old));
    }

    /// The user's own skill with a mAId skill's name is reported, not replaced.
    #[test]
    fn a_users_kiro_skill_named_like_a_maid_skill_is_kept() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        let theirs = home.path().join(".kiro/skills/notes/SKILL.md");
        write(&theirs, "mine");
        for force in [false, true] {
            let reports = install(home.path(), profile.path(), false, force, Some(Agent::Kiro));
            let this = reports
                .iter()
                .find(|r| r.label == ".kiro/skills/notes")
                .unwrap();
            assert_eq!(this.state, State::Theirs);
            assert!(!this.acted);
        }
        uninstall(home.path(), profile.path(), false, true, Some(Agent::Kiro));
        assert_eq!(fs::read_to_string(&theirs).unwrap(), "mine");
    }

    /// A rollback or a later install can drop a skill; its link goes.
    #[test]
    fn a_skill_dropped_from_the_profile_goes_on_the_next_install() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        let notes = home.path().join(".kiro/skills/notes");
        install(home.path(), profile.path(), false, false, Some(Agent::Kiro));
        fs::remove_dir_all(profile.path().join("share/maid/skills/notes")).unwrap();

        let listed = status(home.path(), profile.path(), Some(Agent::Kiro));
        let this = listed
            .iter()
            .find(|r| r.label == ".kiro/skills/notes")
            .unwrap();
        assert!(matches!(this.state, State::Legacy(_)));
        install(home.path(), profile.path(), true, false, Some(Agent::Kiro));
        assert!(exists_at(&notes));

        install(home.path(), profile.path(), false, false, Some(Agent::Kiro));
        assert!(!exists_at(&notes));
        assert!(exists_at(&home.path().join(".kiro/skills/kdevkit")));
    }

    #[test]
    fn uninstall_removes_a_kiro_link_to_a_skill_no_longer_shipped() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        let notes = home.path().join(".kiro/skills/notes");
        let other = home.path().join(".kiro/skills/gone");
        install(home.path(), profile.path(), false, false, Some(Agent::Kiro));
        fs::remove_dir_all(profile.path().join("share/maid/skills/notes")).unwrap();
        std::os::unix::fs::symlink("/other/profile/share/maid/skills/gone", &other).unwrap();
        uninstall(home.path(), profile.path(), false, false, Some(Agent::Kiro));
        assert!(!exists_at(&notes) && !exists_at(&other));
    }

    /// isolated-verify installs a throwaway profile, then the owner's
    /// again: the throwaway's links must not outlive it.
    #[test]
    fn an_install_from_another_profile_takes_over_every_kiro_link() {
        let first = make_profile_with_skills();
        let second = make_profile();
        write(
            &second.path().join("share/maid/skills/kdevkit/SKILL.md"),
            "---\nname: x\ndescription: y\n---\nbody.\n",
        );
        let home = TempDir::new().unwrap();
        install(home.path(), first.path(), false, false, Some(Agent::Kiro));
        install(home.path(), second.path(), false, false, Some(Agent::Kiro));
        assert!(!exists_at(&home.path().join(".kiro/skills/notes")));
        assert_eq!(
            fs::read_link(home.path().join(".kiro/skills/kdevkit")).unwrap(),
            second.path().join("share/maid/skills/kdevkit")
        );
    }

    #[test]
    fn force_keeps_a_foreign_link_named_like_a_maid_skill_in_kiros_dir() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        let theirs = home.path().join(".kiro/skills/notes");
        fs::create_dir_all(theirs.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink("/their/notes", &theirs).unwrap();
        let reports = install(home.path(), profile.path(), false, true, Some(Agent::Kiro));
        let this = reports
            .iter()
            .find(|r| r.label == ".kiro/skills/notes")
            .unwrap();
        assert_eq!((&this.state, this.acted), (&State::Theirs, false));
        assert_eq!(fs::read_link(&theirs).unwrap(), Path::new("/their/notes"));
        uninstall(home.path(), profile.path(), false, true, Some(Agent::Kiro));
        assert_eq!(fs::read_link(&theirs).unwrap(), Path::new("/their/notes"));
    }

    /// A user's own link to a skill in a checkout looks like mAId's, but
    /// mAId never made one in kiro's dir: kept, shipped name or not.
    #[test]
    fn a_users_link_to_a_checkout_skill_in_kiros_dir_is_kept() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        let mut links = vec![];
        for name in ["wip", "kdevkit"] {
            let at = home.path().join(".kiro/skills").join(name);
            let to = PathBuf::from("/work/mAId/resources/content/skills").join(name);
            fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::os::unix::fs::symlink(&to, &at).unwrap();
            links.push((at, to));
        }
        let reports = install(home.path(), profile.path(), false, false, Some(Agent::Kiro));
        let this = reports
            .iter()
            .find(|r| r.label == ".kiro/skills/kdevkit")
            .unwrap();
        assert_eq!((&this.state, this.acted), (&State::Theirs, false));
        uninstall(home.path(), profile.path(), false, false, Some(Agent::Kiro));
        for (at, to) in links {
            assert_eq!(fs::read_link(at).unwrap(), to);
        }
    }

    // ── Plugins ──────────────────────────────────────────────────

    // ── bring-ins ────────────────────────────────────────────────

    const VENDOR: BringIn = BringIn {
        marketplace: "vend",
        source: "https://example.org/v.git",
        rev: "0123456789012345678901234567890123456789",
        plugins: &["docs"],
        needs: &["tool-a", "tool-b"],
    };

    fn bring_steps(agent: Agent, market: Held, plugin: Held) -> Vec<String> {
        install_steps_for(agent, &VENDOR, market, &[("docs@vend".into(), plugin)])
            .into_iter()
            .map(|(.., args)| args.join(" "))
            .collect()
    }

    fn unbring_steps(agent: Agent, market: Held, plugin: Held, others: bool) -> Vec<String> {
        uninstall_steps_for(
            agent,
            &VENDOR,
            market,
            &[("docs@vend".into(), plugin)],
            others,
        )
        .into_iter()
        .map(|(.., args)| args.join(" "))
        .collect()
    }

    #[test]
    fn a_bring_in_with_nothing_there_adds_the_marketplace_then_the_plugin() {
        assert_eq!(
            bring_steps(Agent::Codex, Held::Absent, Held::Absent),
            [
                "plugin marketplace add https://example.org/v.git --ref 0123456789012345678901234567890123456789",
                "plugin add docs@vend",
            ]
        );
        assert_eq!(
            bring_steps(Agent::Claude, Held::Absent, Held::Absent),
            [
                "plugin marketplace add https://example.org/v.git",
                "plugin install docs@vend --scope user",
            ]
        );
    }

    #[test]
    fn the_users_marketplace_gets_only_the_plugin_and_their_plugin_nothing() {
        assert_eq!(
            bring_steps(Agent::Codex, Held::Theirs, Held::Absent),
            ["plugin add docs@vend"]
        );
        assert!(bring_steps(Agent::Codex, Held::Theirs, Held::Theirs).is_empty());
        assert!(bring_steps(Agent::Claude, Held::Ours, Held::Theirs).is_empty());
    }

    #[test]
    fn a_bring_in_already_placed_needs_nothing() {
        assert!(bring_steps(Agent::Claude, Held::Ours, Held::Ours).is_empty());
        assert!(bring_steps(Agent::Codex, Held::Ours, Held::Ours).is_empty());
    }

    #[test]
    fn uninstall_removes_maids_plugin_before_maids_marketplace() {
        assert_eq!(
            unbring_steps(Agent::Claude, Held::Ours, Held::Ours, false),
            [
                "plugin uninstall docs@vend --scope user",
                "plugin marketplace remove vend",
            ]
        );
        assert_eq!(
            unbring_steps(Agent::Codex, Held::Ours, Held::Ours, false),
            ["plugin remove docs@vend", "plugin marketplace remove vend"]
        );
    }

    /// Removing a marketplace also uninstalls every plugin from it.
    #[test]
    fn a_marketplace_another_plugin_uses_is_kept() {
        assert_eq!(
            unbring_steps(Agent::Codex, Held::Ours, Held::Ours, true),
            ["plugin remove docs@vend"]
        );
    }

    #[test]
    fn uninstall_leaves_the_users_marketplace_and_plugin() {
        assert!(unbring_steps(Agent::Claude, Held::Theirs, Held::Theirs, false).is_empty());
        assert_eq!(
            unbring_steps(Agent::Claude, Held::Theirs, Held::Ours, false),
            ["plugin uninstall docs@vend --scope user"]
        );
        assert!(unbring_steps(Agent::Codex, Held::Absent, Held::Absent, false).is_empty());
    }

    #[test]
    fn a_bring_in_item_is_placed_only_when_maids() {
        let m = What::Marketplace;
        assert_eq!(
            placed_item(m, "vend", Held::Ours, false),
            Some(Item::placed(m, "vend"))
        );
        assert_eq!(
            placed_item(m, "vend", Held::Theirs, false),
            Some(Item::skipped(m, "vend", "not mAId's"))
        );
        assert_eq!(placed_item(m, "vend", Held::Absent, false), None);
        assert_eq!(
            placed_item(m, "vend", Held::Absent, true),
            Some(Item::skipped(m, "vend", "failed"))
        );
    }

    /// A listing not seen once must not turn what mAId placed into
    /// something it no longer owns.
    #[test]
    fn an_unlisted_item_keeps_what_maid_placed_as_its_own() {
        let m = What::Marketplace;
        let why = || "unreadable".to_string();
        assert_eq!(
            unlisted_item(m, "vend", true, why()),
            Item::placed(m, "vend")
        );
        assert_eq!(
            unlisted_item(m, "vend", false, why()),
            Item::skipped(m, "vend", "unreadable")
        );
    }

    #[test]
    fn the_users_local_marketplace_under_the_vendors_name_is_found() {
        let claude = r#"[{"name":"vend","source":"directory","path":"/u/vend"}]"#;
        assert_eq!(
            parse_cloned(Agent::Claude, claude, "vend").unwrap(),
            Some(PathBuf::from("/u/vend"))
        );
    }

    #[test]
    fn status_reports_a_part_list_it_cannot_read_and_carries_on() {
        let parts = TempDir::new().unwrap();
        let text = r#"{"format":2,"package":"maid","agents":{}}"#;
        fs::write(install_record::path(parts.path()), text).unwrap();
        let home = PathBuf::from("/nowhere");
        assert!(BringIns::new(home.clone(), cc(&home), &[VENDOR], parts.path()).is_err());
        let b = BringIns::reporting(home.clone(), cc(&home), &[VENDOR], parts.path());
        // kiro has no plugin CLI, so no CLI runs here.
        let lines = b.status(Some(Agent::Kiro)).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].label, "part list");
        assert!(
            matches!(&lines[0].state, State::Failed(why) if why.contains("format 2")),
            "{:?}",
            lines[0].state
        );
    }

    #[test]
    fn agents_without_a_plugin_cli_record_the_bring_in_skipped() {
        let parts = TempDir::new().unwrap();
        let b = BringIns::new(
            PathBuf::from("/nowhere"),
            cc(Path::new("/nowhere")),
            &[VENDOR],
            parts.path(),
        )
        .unwrap();
        for agent in [Agent::Kiro, Agent::Agy] {
            assert_eq!(
                b.placed(Some(agent)).unwrap(),
                [
                    (
                        agent,
                        Item::skipped(What::Marketplace, "vend", "no plugin marketplace")
                    ),
                    (
                        agent,
                        Item::skipped(What::Plugin, "docs@vend", "no plugin marketplace")
                    ),
                ]
            );
        }
    }

    #[test]
    fn ownership_is_what_the_part_list_names_placed_per_harness() {
        let parts = TempDir::new().unwrap();
        let kinds = &[What::Plugin, What::Marketplace];
        let mine = vec![
            Item::placed(What::Marketplace, "vend"),
            Item::skipped(What::Plugin, "docs@vend", "not mAId's"),
        ];
        install_record::record(parts.path(), Agent::Claude, kinds, None, mine).unwrap();
        let b = BringIns::new(
            PathBuf::from("/nowhere"),
            cc(Path::new("/nowhere")),
            &[VENDOR],
            parts.path(),
        )
        .unwrap();
        assert!(b.ours(Agent::Claude, What::Marketplace, "vend"));
        assert!(!b.ours(Agent::Claude, What::Plugin, "docs@vend"));
        assert!(!b.ours(Agent::Codex, What::Marketplace, "vend"));
        b.own(Agent::Codex, What::Marketplace, "vend");
        assert!(b.ours(Agent::Codex, What::Marketplace, "vend"));
        b.disown(Agent::Codex, What::Marketplace, "vend");
        assert!(!b.ours(Agent::Codex, What::Marketplace, "vend"));
        assert!(b.ours(Agent::Claude, What::Marketplace, "vend"));
    }

    #[test]
    fn status_names_each_command_the_skills_run_that_is_missing() {
        let lines: Vec<_> = missing_needs(&[VENDOR], |c| c == "tool-a")
            .into_iter()
            .map(|r| format!("{} {}", r.label, r.state.describe()))
            .collect();
        assert_eq!(lines, ["docs needs tool-b not on PATH"]);
        assert!(missing_needs(&[VENDOR], |_| true).is_empty());
    }

    #[test]
    fn a_git_marketplace_is_found_by_its_clone_and_every_plugin_id_is_listed() {
        let claude = r#"[{"name":"vend","source":"git","installLocation":"/h/.claude/plugins/marketplaces/vend"}]"#;
        assert_eq!(
            parse_cloned(Agent::Claude, claude, "vend").unwrap(),
            Some(PathBuf::from("/h/.claude/plugins/marketplaces/vend"))
        );
        let listed = r#"[{"id":"docs@vend","scope":"user"},{"id":"other@vend","scope":"project"}]"#;
        assert_eq!(
            listed_ids(Agent::Claude, listed).unwrap(),
            [("docs@vend".into(), true), ("other@vend".into(), false)]
        );
        // mAId's own marketplace is still found only by the path it was added from.
        assert_eq!(parse_recorded(Agent::Claude, claude, "vend").unwrap(), None);
    }

    /// Claude's config under `home`, the variable unset.
    fn cc(home: &Path) -> ConfigDir {
        ConfigDir::new(home, None)
    }

    #[test]
    fn a_plugin_call_gives_claude_only_the_config_dir_its_value_names() {
        let var = |cmd: &Command| {
            cmd.get_envs()
                .find(|(k, _)| *k == "CLAUDE_CONFIG_DIR")
                .map(|(_, v)| v.map(PathBuf::from))
        };
        let home = Path::new("/h");
        let unset = cli_command(home, &cc(home), Agent::Claude);
        assert_eq!(var(&unset), Some(None), "removed, not inherited");
        let set = ConfigDir::new(home, Some(Path::new("/c")));
        let cmd = cli_command(home, &set, Agent::Claude);
        assert_eq!(var(&cmd), Some(Some(PathBuf::from("/c"))));
        assert_eq!(var(&cli_command(home, &set, Agent::Codex)), None);
    }

    fn plugins(home: &Path, profile: &Path) -> Plugins {
        Plugins {
            home: home.to_path_buf(),
            claude: ConfigDir::new(home, None),
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
            parse_listed(Agent::Claude, json, PLUGIN_ID).unwrap(),
            Some(Listed {
                version: "1.0.0-u".into(),
                enabled: false
            })
        );
        assert_eq!(parse_listed(Agent::Claude, "[]", PLUGIN_ID).unwrap(), None);
    }

    #[test]
    fn codex_listing_finds_the_installed_entry() {
        let json = r#"{"installed":[{"pluginId":"maid@maid","version":"1.0.0-c","enabled":true}],
                       "available":[]}"#;
        assert_eq!(
            parse_listed(Agent::Codex, json, PLUGIN_ID).unwrap(),
            Some(Listed {
                version: "1.0.0-c".into(),
                enabled: true
            })
        );
        assert_eq!(
            parse_listed(Agent::Codex, r#"{"installed":[]}"#, PLUGIN_ID).unwrap(),
            None
        );
    }

    /// Output we cannot read must not pass as "not installed", or install
    /// would re-add over whatever is there.
    #[test]
    fn an_unreadable_listing_is_an_error() {
        assert!(parse_listed(Agent::Claude, "not json", PLUGIN_ID).is_err());
        assert!(parse_listed(Agent::Claude, "{}", PLUGIN_ID).is_err());
        assert!(parse_listed(Agent::Codex, "[]", PLUGIN_ID).is_err());
        assert!(parse_listed(
            Agent::Codex,
            r#"{"installed":[{"pluginId":"maid@maid"}]}"#,
            PLUGIN_ID
        )
        .is_err());
        let no_flag = r#"{"installed":[{"pluginId":"maid@maid","version":"1"}]}"#;
        assert!(parse_listed(Agent::Codex, no_flag, PLUGIN_ID).is_err());
    }

    #[test]
    fn the_recorded_marketplace_path_is_read_per_cli() {
        let claude = r#"[{"name":"other","path":"/o"},{"name":"maid","path":"/s/marketplace"}]"#;
        let codex = r#"{"marketplaces":[{"name":"maid","root":"/s/marketplace"}]}"#;
        assert_eq!(
            parse_recorded(Agent::Claude, claude, MARKETPLACE).unwrap(),
            Some(PathBuf::from("/s/marketplace"))
        );
        assert_eq!(
            parse_recorded(Agent::Codex, codex, MARKETPLACE).unwrap(),
            Some(PathBuf::from("/s/marketplace"))
        );
        assert_eq!(
            parse_recorded(Agent::Claude, "[]", MARKETPLACE).unwrap(),
            None
        );
        assert!(parse_recorded(Agent::Codex, "[]", MARKETPLACE).is_err());
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

        let mut reaped = reap(h, ".claude/skills", Kind::Link, false).unwrap();
        reaped.extend(reap(h, ".codex/skills", Kind::FanOut, false).unwrap());
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
        assert!(reap(home.path(), ".claude/skills", Kind::Link, false)
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
        assert_eq!(
            reap(home.path(), ".claude/skills", Kind::Link, true)
                .unwrap()
                .len(),
            1
        );
        assert!(exists_at(&at));
    }

    /// The latest install's uninstall removes a link an older mAId install
    /// left, without --force, as install replaces it.
    #[test]
    fn uninstall_removes_a_link_another_maid_install_left() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        let kiro = home.path().join(".kiro/skills/kdevkit");
        fs::create_dir_all(kiro.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink("/old/profile/share/maid/skills/kdevkit", &kiro).unwrap();
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
