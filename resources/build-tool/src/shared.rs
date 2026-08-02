//! Vocabulary every stage of the pipeline speaks: which coding agents
//! exist, where each expects its skills, and where the roots are.
//!
//! Depends on nothing else in the crate — the bottom of the dependency
//! order (`stages` → `harness` → `shared`).

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};

// ─────────────────────────────────────────────────────────────────
// Registry — the deployment manifest.
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

pub type Entry = (&'static str, &'static str, Kind, &'static str); // (home_subpath, source_subpath, kind, agent)

/// A concrete symlink to manage, resolved from an entry: (home, source).
pub type Link = (PathBuf, PathBuf);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Link,
    FanOut,
}

/// The coding agents mAId deploys to. An install/uninstall/status can
/// be scoped to one (`--agent`) or, by default, cover them all. This
/// list is also the recognized universe used to validate `--agent`.
pub const AGENTS: &[&str] = &["claude", "kiro", "codex"];

pub const REGISTRY: &[Entry] = &[
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
pub fn selected_entries(agent: Option<&str>) -> Vec<Entry> {
    REGISTRY
        .iter()
        .filter(|(.., a)| agent.is_none_or(|sel| sel == *a))
        .copied()
        .collect()
}

/// Validate an `--agent` value against the known set, returning it
/// back for chaining. An unknown agent is a hard error listing the
/// valid names, so a typo never silently installs nothing.
pub fn validate_agent(agent: Option<&str>) -> Result<Option<&str>> {
    match agent {
        Some(a) if !AGENTS.contains(&a) => Err(anyhow!(
            "unknown coding agent {a:?} (known: {})",
            AGENTS.join(", ")
        )),
        other => Ok(other),
    }
}

// ─────────────────────────────────────────────────────────────────
// Agent — one coding agent, and where it reads skills from.
//
// Every stage needs some version of "where does agent X keep skills":
// install writes the tree, smoke reads a skill out of the installed
// tree, and check reads the same skill out of the checkout instead.
// That knowledge is derived from REGISTRY here rather than restated —
// the bash runner kept its own copy of the three home paths, and it
// drifted from the registry, which is the bug this type exists to make
// impossible.
// ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Agent {
    Claude,
    Kiro,
    Codex,
}

impl Agent {
    /// Every agent, in registry order.
    pub const ALL: &'static [Agent] = &[Agent::Claude, Agent::Kiro, Agent::Codex];

    /// The registry-facing name — the same token `--agent` accepts.
    pub fn name(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Kiro => "kiro",
            Agent::Codex => "codex",
        }
    }

    /// Parse an `--agent` token. `None` means "every agent", which is
    /// the default for every verb that takes the selector.
    pub fn parse(token: &str) -> Result<Agent> {
        Agent::ALL
            .iter()
            .copied()
            .find(|a| a.name() == token)
            .ok_or_else(|| {
                anyhow!(
                    "unknown coding agent {token:?} (known: {})",
                    AGENTS.join(", ")
                )
            })
    }

    /// The agent's skills root under `$HOME`, from its REGISTRY row.
    ///
    /// Panics only if REGISTRY loses a row for a variant — an
    /// unrepresentable state the `agents_have_registry_rows` test pins.
    pub fn skills_root(self, home: &Path) -> PathBuf {
        let (home_sub, ..) = REGISTRY
            .iter()
            .find(|(.., agent)| *agent == self.name())
            .expect("every Agent variant has a REGISTRY row");
        home.join(home_sub)
    }

    /// Where this agent reads `<skill>`'s SKILL.md once installed.
    /// The post-install (smoke) source: what the deployment exposes.
    pub fn installed_skill(self, home: &Path, skill: &str) -> PathBuf {
        self.skills_root(home).join(skill).join("SKILL.md")
    }
}

/// Where `<skill>`'s SKILL.md lives in the checkout — the pre-install
/// (check) source. Agent-independent by design: before install there is
/// only one copy, which is why the three explicit kinds need no deploy.
pub fn checkout_skill(checkout: &Path, skill: &str) -> PathBuf {
    checkout
        .join("resources/content/skills")
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

    // ── Agent · skill sources ────────────────────────────────────

    #[test]
    fn agent_parse_round_trips_every_name() {
        for agent in Agent::ALL {
            assert_eq!(Agent::parse(agent.name()).unwrap(), *agent);
        }
        assert!(Agent::parse("bogus").is_err());
    }

    /// `skills_root` reads REGISTRY, so a variant without a row would
    /// panic at runtime. Pin that here rather than in a caller.
    #[test]
    fn agents_have_registry_rows() {
        let home = Path::new("/home/u");
        for agent in Agent::ALL {
            assert!(agent.skills_root(home).starts_with(home));
        }
        assert_eq!(Agent::ALL.len(), AGENTS.len());
    }

    /// The three deployed roots, spelled out. This is the assertion the
    /// bash runner could not make: it hand-copied these paths, they
    /// drifted from REGISTRY, and nothing caught it.
    #[test]
    fn installed_skill_matches_each_agents_deployed_layout() {
        let home = Path::new("/home/u");
        assert_eq!(
            Agent::Claude.installed_skill(home, "notes"),
            Path::new("/home/u/.claude/skills/notes/SKILL.md")
        );
        assert_eq!(
            Agent::Kiro.installed_skill(home, "notes"),
            Path::new("/home/u/.kiro/steering/skills/notes/SKILL.md")
        );
        assert_eq!(
            Agent::Codex.installed_skill(home, "notes"),
            Path::new("/home/u/.codex/skills/notes/SKILL.md")
        );
    }

    /// Pre-install there is one copy of a skill, not three — which is
    /// why the explicit kinds need no deploy.
    #[test]
    fn checkout_skill_is_agent_independent() {
        let checkout = Path::new("/repo");
        assert_eq!(
            checkout_skill(checkout, "notes"),
            Path::new("/repo/resources/content/skills/notes/SKILL.md")
        );
    }

    /// The two sources must never collide: check reads the checkout,
    /// smoke reads $HOME, and conflating them is the inconsistency the
    /// bash runner shipped (announce read from one, prompts from the
    /// other).
    #[test]
    fn checkout_and_installed_sources_are_distinct() {
        let home = Path::new("/home/u");
        let checkout = Path::new("/repo");
        for agent in Agent::ALL {
            assert_ne!(
                agent.installed_skill(home, "notes"),
                checkout_skill(checkout, "notes")
            );
        }
    }
}
