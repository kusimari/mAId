//! Deployment — the one place that knows how an agent lays out `$HOME`.
//!
//! The pipeline above declares **what** should be deployed; this decides
//! **how**. That split is deliberate and load-bearing: knowing each
//! agent's home layout is the least durable knowledge in this repo, so it
//! is quarantined behind one trait rather than spread through the stages.
//!
//! Ideally an agent would install its own skills and we would just hand
//! one over. None can, today — verified against all three CLIs: `claude`
//! and `kiro-cli` expose no skill verb at all, and `codex plugin add`
//! takes `PLUGIN@MARKETPLACE`, so it cannot install from a local
//! checkout. Until one of them grows that command, `Symlinks` below is
//! the whole implementation.
//!
//! When one does, it becomes a second `Deploy` impl — an
//! `AgentManaged { … }` that shells out to the agent's own verb — and
//! nothing in `stages` changes. That is the point of the boundary.

use crate::shared::{selected_entries, Agent, Entry, Kind, Link};
use anyhow::{anyhow, Result};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

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
/// The shim. It exists because no agent can install a skill for us yet;
/// see the module comment.
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
        // The skills ROOT existing is the wrong question for a FanOut
        // agent: codex owns ~/.codex/skills and may have it
        // present-but-empty (or non-empty for reasons unrelated to us),
        // so the directory existing proves nothing about whether we
        // deployed into it. Ask instead whether at least one of our own
        // links resolves to something that is actually there —
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
    /// child reads as `.codex/skills/<name>`.
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
    use crate::shared::REGISTRY;
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
        assert!(exists_at(&home.path().join(".claude/skills")));
        assert!(exists_at(&home.path().join(".codex/skills/kdevkit")));
        assert!(exists_at(&home.path().join(".gemini/config/skills")));
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
        assert!(!exists_at(&home.path().join(".claude/skills")));
    }

    /// A real file at a managed path is never clobbered without --force.
    #[test]
    fn a_users_own_file_survives_install() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        let claimed = home.path().join(".claude/skills");
        write(&claimed, "mine");
        install(home.path(), profile.path(), false, false, None);
        assert_eq!(fs::read_to_string(&claimed).unwrap(), "mine");
    }

    #[test]
    fn a_foreign_symlink_is_replaced_only_with_force() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        let managed = home.path().join(".claude/skills");
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
        let claude = home.path().join(".claude/skills");
        let codex = home.path().join(".codex/skills/notes");
        for link in [&claude, &codex] {
            fs::create_dir_all(link.parent().unwrap()).unwrap();
        }
        std::os::unix::fs::symlink(old.path().join("resources/content/skills"), &claude).unwrap();
        std::os::unix::fs::symlink(old.path().join("share/maid/skills/notes"), &codex).unwrap();

        install(home.path(), profile.path(), false, false, None);
        assert_eq!(
            fs::read_link(&claude).unwrap(),
            profile.path().join("share/maid/skills")
        );
        assert_eq!(
            fs::read_link(&codex).unwrap(),
            profile.path().join("share/maid/skills/notes")
        );
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
        // Something the agent owns, beside our fan-out children.
        let theirs = home.path().join(".codex/skills/their-own");
        write(&theirs, "theirs");

        uninstall(home.path(), profile.path(), false, false, None);
        assert!(!exists_at(&home.path().join(".claude/skills")));
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
        let theirs = home.path().join(".codex/skills");
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
        assert!(!exists_at(&home.path().join(".claude/skills")));

        install(home.path(), profile.path(), false, false, None);
        let after = status(home.path(), profile.path(), None);
        assert!(after.iter().all(|r| matches!(r.state, State::Ok(_))));
    }

    #[test]
    fn a_scoped_install_touches_only_that_agent() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        install(
            home.path(),
            profile.path(),
            false,
            false,
            Some(Agent::Codex),
        );
        assert!(exists_at(&home.path().join(".codex/skills/kdevkit")));
        assert!(!exists_at(&home.path().join(".claude/skills")));
        assert!(!exists_at(&home.path().join(".kiro/steering/skills")));
        assert!(!exists_at(&home.path().join(".gemini/config/skills")));
    }

    #[test]
    fn a_scoped_uninstall_leaves_other_agents_deployed() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        install(home.path(), profile.path(), false, false, None);
        uninstall(
            home.path(),
            profile.path(),
            false,
            false,
            Some(Agent::Claude),
        );
        assert!(!exists_at(&home.path().join(".claude/skills")));
        assert!(exists_at(&home.path().join(".kiro/steering/skills")));
        assert!(exists_at(&home.path().join(".gemini/config/skills")));
    }

    /// FanOut mirrors each source child rather than replacing the dir the
    /// agent owns.
    #[test]
    fn fanout_yields_one_link_per_source_child() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        let fanout = *REGISTRY
            .iter()
            .find(|(.., k, _)| *k == Kind::FanOut)
            .unwrap();
        let links = sym(home.path(), profile.path()).expand(fanout).unwrap();
        assert_eq!(links.len(), 2, "one per child skill");
    }

    /// A child removed from source is still reaped rather than left
    /// dangling in a directory we don't own.
    #[test]
    fn fanout_reaps_an_orphaned_child() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        install(
            home.path(),
            profile.path(),
            false,
            false,
            Some(Agent::Codex),
        );
        fs::remove_dir_all(profile.path().join("share/maid/skills/notes")).unwrap();

        let reports = uninstall(
            home.path(),
            profile.path(),
            false,
            false,
            Some(Agent::Codex),
        );
        assert!(reports.len() >= 2, "the orphan is still planned for");
        assert!(!exists_at(&home.path().join(".codex/skills/notes")));
    }

    #[test]
    fn is_deployed_answers_the_smoke_precondition() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        let target = sym(home.path(), profile.path());
        assert!(!target.is_deployed(Agent::Claude));
        assert!(!target.is_deployed(Agent::Agy));
        install(home.path(), profile.path(), false, false, None);
        assert!(target.is_deployed(Agent::Claude));
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
        assert!(!sym(home.path(), profile.path()).is_deployed(Agent::Claude));
    }

    /// The bug an audit caught: for a FanOut agent, an existing-but-empty
    /// (or unrelated-content) skills root must NOT read as deployed. Only
    /// codex is FanOut, and its root existing proves nothing about
    /// whether WE put anything there.
    #[test]
    fn an_empty_fanout_root_does_not_count_as_deployed() {
        let profile = make_profile();
        let home = TempDir::new().unwrap();
        // codex's root exists, pre-created, but nothing of ours is in it.
        fs::create_dir_all(home.path().join(".codex/skills")).unwrap();
        assert!(!sym(home.path(), profile.path()).is_deployed(Agent::Codex));
    }

    #[test]
    fn a_fanout_deployment_is_detected_once_a_child_resolves() {
        let profile = make_profile_with_skills();
        let home = TempDir::new().unwrap();
        assert!(!sym(home.path(), profile.path()).is_deployed(Agent::Codex));
        install(
            home.path(),
            profile.path(),
            false,
            false,
            Some(Agent::Codex),
        );
        assert!(sym(home.path(), profile.path()).is_deployed(Agent::Codex));
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
        let claimed = home.path().join(".claude/skills");
        write(&claimed, "mine");

        let reports = install(home.path(), profile.path(), false, true, None);
        let this = reports
            .iter()
            .find(|r| r.label == ".claude/skills")
            .unwrap();
        assert!(
            !this.acted,
            "create() must not act on a real file, force or not"
        );
        assert_eq!(fs::read_to_string(&claimed).unwrap(), "mine");
    }
}
