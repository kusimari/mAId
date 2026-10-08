//! Vocabulary every stage of the pipeline speaks: which coding agents
//! exist, where each expects its skills, and where the roots are.
//!
//! Depends on nothing else in the crate — the bottom of the dependency
//! order (`stages` → `harness` → `shared`).

use anyhow::{anyhow, Context, Result};
use std::fmt;
use std::path::{Path, PathBuf};

/// A bad invocation — an unknown agent or kind, a kind belonging to the
/// other stage. Distinct from a test failure so a wrapper can tell "your
/// command was wrong" (exit 2) from "the tests failed" (exit 1); clap's
/// own errors already exit 2, so collapsing these to 1 made the surface
/// disagree with itself.
#[derive(Debug)]
pub struct UsageError(pub String);

impl fmt::Display for UsageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for UsageError {}

/// Build a usage error, for the `?` paths that would otherwise produce a
/// plain `anyhow!`.
pub fn usage(msg: impl Into<String>) -> anyhow::Error {
    anyhow::Error::new(UsageError(msg.into()))
}

// ─────────────────────────────────────────────────────────────────
// Registry — the deployment manifest.
//
// The job: `just install` builds the skills into the mAId profile (see
// `profile_dir`). claude and codex install them as a plugin with their
// own CLI (`PLUGIN_AGENTS`; see deploy.rs `Plugins`). The rest have no
// such command and discover skills under their own home dir (kiro
// ~/.kiro/steering, agy ~/.gemini/config), so the registry maps profile
// source → agent home, one row per target, in one of two shapes (`Kind`):
//
//   Link   — the agent's home layout matches the profile, so symlink
//            the home path straight at the source dir. mAId owns it.
//   FanOut — the agent owns the home dir and puts its own entries
//            there, so we can't replace it; mirror each source child in
//            as its own symlink and leave the rest alone. No row uses it
//            now; it is the shape codex's pre-plugin links had, which
//            `PLUGIN_AGENTS` names so install can reap them.
//
// Skills are all that's installed. There is no global instruction
// preamble: loading a project's AGENTS.md / project.md is kdevkit's
// work-time instruction, and AGENTS.md is a repo-root convention, not a
// global per-tool file.
// ─────────────────────────────────────────────────────────────────

pub type Entry = (&'static str, &'static str, Kind, Agent); // (home_subpath, profile_subpath, kind, agent)

/// A concrete symlink to manage, resolved from an entry: (home, source).
pub type Link = (PathBuf, PathBuf);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Link,
    FanOut,
}

/// One coding agent mAId deploys to. Rows below are keyed by this, so
/// the variant — not a string — is what identifies an agent; the only
/// place a name is spelled is `Agent::name`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Agent {
    Claude,
    Kiro,
    Codex,
    Agy,
}

/// The link rows: agents with no plugin command of their own.
pub const REGISTRY: &[Entry] = &[
    (
        ".kiro/steering/skills",
        "share/maid/skills",
        Kind::Link,
        Agent::Kiro,
    ),
    (
        ".gemini/config/skills",
        "share/maid/skills",
        Kind::Link,
        Agent::Agy,
    ),
];

/// Agents that install mAId as a plugin through their own CLI, each with
/// where the links an older mAId install left live (reaped on install).
pub const PLUGIN_AGENTS: &[(Agent, &str, Kind)] = &[
    (Agent::Claude, ".claude/skills", Kind::Link),
    (Agent::Codex, ".codex/skills", Kind::FanOut),
];

/// The plugin and the marketplace it comes from, as both CLIs name them.
pub const PLUGIN_ID: &str = "maid@maid";
pub const MARKETPLACE: &str = "maid";

/// Where flake.nix puts the marketplace in the profile.
pub const PROFILE_MARKETPLACE_DIR: &str = "share/maid/marketplace";

/// The marketplace dir the agents are pointed at: a real dir beside the
/// profile whose entries link into it. Codex records a marketplace by its
/// resolved path, so registering the profile itself would pin one
/// generation's store path.
pub fn marketplace_root(profile: &Path) -> PathBuf {
    profile
        .parent()
        .unwrap_or(Path::new("/"))
        .join("marketplace")
}

impl Agent {
    /// Every agent. `every_agent_is_deployed_by_exactly_one_mechanism`
    /// holds this in step with REGISTRY.
    pub const ALL: &'static [Agent] = &[Agent::Claude, Agent::Kiro, Agent::Codex, Agent::Agy];

    /// The token `--agent` accepts. The one place a name is spelled.
    pub fn name(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Kiro => "kiro",
            Agent::Codex => "codex",
            Agent::Agy => "agy",
        }
    }

    /// Parse an `--agent` token; an unknown one lists the valid names,
    /// so a typo never silently installs nothing.
    pub fn parse(token: &str) -> Result<Agent> {
        if token == "antigravity" {
            return Ok(Agent::Agy);
        }
        Agent::ALL
            .iter()
            .copied()
            .find(|a| a.name() == token)
            .ok_or_else(|| {
                usage(format!(
                    "unknown coding agent {token:?} (known: {})",
                    Agent::ALL
                        .iter()
                        .map(|a| a.name())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            })
    }

    /// The CLI an agent is driven through.
    pub fn cli(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Kiro => "kiro-cli",
            Agent::Codex => "codex",
            Agent::Agy => "agy",
        }
    }

    /// Whether this agent gets mAId as a plugin rather than by link.
    pub fn is_plugin(self) -> bool {
        PLUGIN_AGENTS.iter().any(|(a, ..)| *a == self)
    }

    /// This agent's REGISTRY row.
    fn entry(self) -> Option<&'static Entry> {
        REGISTRY.iter().find(|(.., agent)| *agent == self)
    }

    /// The agent's skills root under `$HOME`. `None` when REGISTRY
    /// carries no row for it — an agent mAId knows but doesn't deploy
    /// to, which the registry is entitled to express.
    pub fn skills_root(self, home: &Path) -> Option<PathBuf> {
        self.entry().map(|(home_sub, ..)| home.join(home_sub))
    }

    /// Where this agent reads `<skill>`'s SKILL.md once installed — the
    /// post-install source, i.e. what the deployment exposes.
    pub fn installed_skill(self, home: &Path, skill: &str) -> Option<PathBuf> {
        self.skills_root(home)
            .map(|root| root.join(skill).join("SKILL.md"))
    }
}

/// Filter REGISTRY to the rows an `--agent` selection acts on: `None`
/// = every row (the default), `Some(a)` = just that agent's rows.
pub fn selected_entries(agent: Option<Agent>) -> Vec<Entry> {
    REGISTRY
        .iter()
        .filter(|(.., a)| agent.is_none_or(|sel| sel == *a))
        .copied()
        .collect()
}

/// Resolve an optional `--agent` token to the agent it selects, `None`
/// meaning all of them. The CLI boundary: clap hands over a string,
/// everything downstream works in `Agent`.
pub fn validate_agent(agent: Option<&str>) -> Result<Option<Agent>> {
    agent.map(Agent::parse).transpose()
}

/// Resolve an `--agent` value that may name several agents, as the
/// verification verbs accept (`--agent claude,kiro`). `None` means all of
/// them; the install verbs take a single agent and use `validate_agent`.
pub fn validate_agents(agents: Option<&str>) -> Result<Option<Vec<Agent>>> {
    let Some(list) = agents else {
        return Ok(None);
    };
    let parsed: Vec<Agent> = list
        .split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(Agent::parse)
        .collect::<Result<_>>()?;
    match parsed.is_empty() {
        true => Err(usage("--agent listed no agents")),
        false => Ok(Some(parsed)),
    }
}

/// Where skills are authored in the checkout.
pub const CONTENT_DIR: &str = "resources/content";

/// Where flake.nix puts that tree in the profile. Every REGISTRY row
/// names its `skills/` child.
pub const PROFILE_CONTENT_DIR: &str = "share/maid";

/// Where `<skill>`'s SKILL.md lives in the checkout — the pre-install
/// source. Agent-independent by design: before install there is only
/// one copy, which is why the explicit test kinds need no deploy.
pub fn checkout_skill(checkout: &Path, skill: &str) -> PathBuf {
    checkout
        .join(CONTENT_DIR)
        .join("skills")
        .join(skill)
        .join("SKILL.md")
}

// ─────────────────────────────────────────────────────────────────
// Roots.
// ─────────────────────────────────────────────────────────────────

pub fn repo_root() -> Result<PathBuf> {
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

/// The mAId profile: the nix profile `just install` builds into and every
/// agent's links point at. The Justfile owns where it is and passes it in.
pub fn profile_dir() -> Result<PathBuf> {
    let raw = std::env::var("MAID_PROFILE")
        .context("MAID_PROFILE not set — invoke via `just` (resources/Justfile sets it)")?;
    let profile = PathBuf::from(&raw);
    profile
        .is_absolute()
        .then_some(profile)
        .ok_or_else(|| anyhow!("MAID_PROFILE must be an absolute path (got {raw:?})"))
}

/// Whether a program is on PATH. Scans PATH directly: `command -v` is a
/// shell builtin, so spawning it always fails.
pub fn on_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
}

pub fn home_dir() -> Result<PathBuf> {
    let raw = std::env::var("HOME").context("HOME is not set")?;
    let home = PathBuf::from(&raw);
    (!raw.is_empty() && home.is_absolute())
        .then_some(home)
        .ok_or_else(|| anyhow!("HOME must be a non-empty absolute path (got {raw:?})"))
}

// ─────────────────────────────────────────────────────────────────
// Tests.
// ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── agent selector (--agent) ─────────────────────────────────

    #[test]
    fn selected_entries_default_is_all() {
        assert_eq!(selected_entries(None).len(), REGISTRY.len());
    }

    #[test]
    fn selected_entries_scopes_to_one_agent() {
        let kiro = selected_entries(Some(Agent::Kiro));
        assert_eq!(kiro.len(), 1);
        assert_eq!(kiro[0].3, Agent::Kiro);
        assert!(selected_entries(Some(Agent::Claude)).is_empty());
    }

    #[test]
    fn validate_agent_rejects_unknown() {
        assert!(validate_agent(Some("bogus")).is_err());
        assert!(validate_agent(Some("claude")).is_ok());
        assert!(validate_agent(None).is_ok());
    }

    // ── Agent · skill sources ────────────────────────────────────

    #[test]
    fn agent_parse_round_trips_every_name() {
        for agent in Agent::ALL {
            assert_eq!(Agent::parse(agent.name()).unwrap(), *agent);
        }
        assert!(Agent::parse("bogus").is_err());
    }

    /// The tokens the Justfile and project.md promise, spelled out — a
    /// name-derived test would round-trip a typo, but `install-skills
    /// kiro` would then reject the very token it documents.
    #[test]
    fn agent_names_are_the_documented_tokens() {
        assert_eq!(Agent::Claude.name(), "claude");
        assert_eq!(Agent::Kiro.name(), "kiro");
        assert_eq!(Agent::Codex.name(), "codex");
        assert_eq!(Agent::Agy.name(), "agy");
    }

    #[test]
    fn agent_parse_accepts_aliases() {
        assert_eq!(Agent::parse("antigravity").unwrap(), Agent::Agy);
        assert_eq!(Agent::parse("agy").unwrap(), Agent::Agy);
    }

    /// Every agent is deployed one way: by plugin or by link, never both,
    /// never neither. `ALL` is hand-written, so this checks it both ways.
    #[test]
    fn every_agent_is_deployed_by_exactly_one_mechanism() {
        for agent in Agent::ALL {
            let linked = REGISTRY.iter().any(|(.., a)| a == agent);
            assert!(linked != agent.is_plugin(), "{}", agent.name());
        }
        for agent in REGISTRY
            .iter()
            .map(|(.., a)| a)
            .chain(PLUGIN_AGENTS.iter().map(|(a, ..)| a))
        {
            assert!(Agent::ALL.contains(agent), "{} not in ALL", agent.name());
        }
        assert!(Agent::Claude.is_plugin() && Agent::Codex.is_plugin());
    }

    /// A selector that parses but matches no row installs nothing while
    /// reporting success, so parsing alone is not enough to assert.
    #[test]
    fn every_linked_agent_name_selects_exactly_its_own_rows() {
        for agent in Agent::ALL.iter().filter(|a| !a.is_plugin()) {
            let rows = selected_entries(Some(validate_agent(Some(agent.name())).unwrap().unwrap()));
            assert!(
                !rows.is_empty(),
                "--agent {} selected nothing",
                agent.name()
            );
            assert!(rows.iter().all(|(.., a)| a == agent));
        }
    }

    #[test]
    fn the_marketplace_root_sits_beside_the_profile() {
        assert_eq!(
            marketplace_root(Path::new("/s/maid/profile")),
            Path::new("/s/maid/marketplace")
        );
    }

    /// The linked roots and the old plugin-agent link paths, spelled out
    /// literally: a row edited to the wrong home path is otherwise
    /// invisible here, since every other assertion derives from the rows.
    #[test]
    fn skills_roots_match_each_agents_deployed_layout() {
        let home = Path::new("/home/u");
        for (agent, want) in [
            (Agent::Kiro, "/home/u/.kiro/steering/skills"),
            (Agent::Agy, "/home/u/.gemini/config/skills"),
        ] {
            assert_eq!(agent.skills_root(home).unwrap(), Path::new(want));
        }
        assert_eq!(Agent::Claude.skills_root(home), None);
        assert_eq!(
            PLUGIN_AGENTS
                .iter()
                .map(|(a, p, _)| (*a, *p))
                .collect::<Vec<_>>(),
            [
                (Agent::Claude, ".claude/skills"),
                (Agent::Codex, ".codex/skills")
            ]
        );
    }

    #[test]
    fn checkout_skill_is_agent_independent() {
        assert_eq!(
            checkout_skill(Path::new("/repo"), "notes"),
            Path::new("/repo/resources/content/skills/notes/SKILL.md")
        );
    }

    /// flake.nix copies the skills to this one profile path; a row naming
    /// another would link an agent at nothing.
    #[test]
    fn every_row_links_at_the_profiles_skills() {
        for (_, source, ..) in REGISTRY {
            assert_eq!(*source, format!("{PROFILE_CONTENT_DIR}/skills"));
        }
    }

    #[test]
    fn validate_agents_parses_a_list_and_rejects_junk() {
        assert_eq!(
            validate_agents(Some("claude,codex")).unwrap(),
            Some(vec![Agent::Claude, Agent::Codex])
        );
        assert_eq!(validate_agents(None).unwrap(), None);
        assert!(validate_agents(Some("claude,bogus")).is_err());
        assert!(validate_agents(Some(",")).is_err());
    }

    /// A bad invocation must be distinguishable from a failed run: clap's
    /// own errors exit 2, so an unknown agent or kind exiting 1 made the
    /// surface disagree with itself and hid "your command was wrong".
    #[test]
    fn an_unknown_agent_is_a_usage_error() {
        let e = Agent::parse("bogus").unwrap_err();
        assert!(e.downcast_ref::<UsageError>().is_some(), "{e}");
    }

    #[test]
    fn an_empty_agent_list_is_a_usage_error() {
        let e = validate_agents(Some(",")).unwrap_err();
        assert!(e.downcast_ref::<UsageError>().is_some(), "{e}");
    }
}
