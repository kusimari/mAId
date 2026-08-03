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

pub type Entry = (&'static str, &'static str, Kind, Agent); // (home_subpath, source_subpath, kind, agent)

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
}

pub const REGISTRY: &[Entry] = &[
    (
        ".claude/skills",
        "resources/content/skills",
        Kind::Link,
        Agent::Claude,
    ),
    (
        ".kiro/steering/skills",
        "resources/content/skills",
        Kind::Link,
        Agent::Kiro,
    ),
    (
        ".codex/skills",
        "resources/content/skills",
        Kind::FanOut,
        Agent::Codex,
    ),
];

impl Agent {
    /// Every agent, in registry order. `registry_rows_cover_every_agent`
    /// holds this in step with REGISTRY.
    pub const ALL: &'static [Agent] = &[Agent::Claude, Agent::Kiro, Agent::Codex];

    /// The token `--agent` accepts. The one place a name is spelled.
    pub fn name(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Kiro => "kiro",
            Agent::Codex => "codex",
        }
    }

    /// Parse an `--agent` token; an unknown one lists the valid names,
    /// so a typo never silently installs nothing.
    pub fn parse(token: &str) -> Result<Agent> {
        Agent::ALL
            .iter()
            .copied()
            .find(|a| a.name() == token)
            .ok_or_else(|| {
                anyhow!(
                    "unknown coding agent {token:?} (known: {})",
                    Agent::ALL
                        .iter()
                        .map(|a| a.name())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
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
        true => Err(anyhow!("--agent listed no agents")),
        false => Ok(Some(parsed)),
    }
}

/// The one source tree skills are authored in, per REGISTRY. Every row
/// shares it — `registry_rows_share_one_content_source` pins that — so
/// the first row answers. REGISTRY is a non-empty const.
fn content_source() -> &'static str {
    REGISTRY[0].1
}

/// Where `<skill>`'s SKILL.md lives in the checkout — the pre-install
/// source. Agent-independent by design: before install there is only
/// one copy, which is why the explicit test kinds need no deploy.
pub fn checkout_skill(checkout: &Path, skill: &str) -> PathBuf {
    checkout.join(content_source()).join(skill).join("SKILL.md")
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
        let codex = selected_entries(Some(Agent::Codex));
        assert_eq!(codex.len(), 1);
        assert_eq!(codex[0].3, Agent::Codex);
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
    }

    /// `ALL` is hand-written while REGISTRY is the manifest, so this
    /// asserts set equality both ways — an agent missing from either
    /// side fails. Length-only comparison would miss a substitution.
    #[test]
    fn registry_rows_cover_every_agent() {
        let tagged: Vec<Agent> = REGISTRY.iter().map(|(.., agent)| *agent).collect();
        for agent in Agent::ALL {
            assert!(
                tagged.contains(agent),
                "{} has no REGISTRY row",
                agent.name()
            );
        }
        for agent in &tagged {
            assert!(
                Agent::ALL.contains(agent),
                "REGISTRY tags {} which Agent::ALL omits",
                agent.name()
            );
        }
    }

    /// A selector that parses but matches no row installs nothing while
    /// reporting success, so parsing alone is not enough to assert.
    #[test]
    fn every_agent_name_selects_exactly_its_own_rows() {
        for agent in Agent::ALL {
            let rows = selected_entries(Some(validate_agent(Some(agent.name())).unwrap().unwrap()));
            assert!(
                !rows.is_empty(),
                "--agent {} selected nothing",
                agent.name()
            );
            assert!(rows.iter().all(|(.., a)| a == agent));
        }
    }

    /// The three deployed roots, spelled out literally: a REGISTRY row
    /// edited to the wrong home path is otherwise invisible here, since
    /// every other assertion derives from the same rows.
    #[test]
    fn installed_skill_matches_each_agents_deployed_layout() {
        let home = Path::new("/home/u");
        for (agent, want) in [
            (Agent::Claude, "/home/u/.claude/skills/notes/SKILL.md"),
            (Agent::Kiro, "/home/u/.kiro/steering/skills/notes/SKILL.md"),
            (Agent::Codex, "/home/u/.codex/skills/notes/SKILL.md"),
        ] {
            assert_eq!(
                agent.installed_skill(home, "notes").unwrap(),
                Path::new(want)
            );
        }
    }

    #[test]
    fn checkout_skill_is_agent_independent() {
        assert_eq!(
            checkout_skill(Path::new("/repo"), "notes"),
            Path::new("/repo/resources/content/skills/notes/SKILL.md")
        );
    }

    /// `checkout_skill` reads the source path off a single row, so a row
    /// pointing somewhere else would make the pre-install source depend
    /// on which row answered.
    #[test]
    fn registry_rows_share_one_content_source() {
        let sources: Vec<&str> = REGISTRY.iter().map(|(_, s, ..)| *s).collect();
        assert!(sources.windows(2).all(|w| w[0] == w[1]), "{sources:?}");
    }

    /// Compared against a *shared* root, so the two resolvers must differ
    /// by their own construction rather than by the caller's roots
    /// happening to be disjoint.
    #[test]
    fn checkout_and_installed_sources_never_collide() {
        let root = Path::new("/same");
        let from_checkout = checkout_skill(root, "notes");
        for agent in Agent::ALL {
            assert_ne!(agent.installed_skill(root, "notes").unwrap(), from_checkout);
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
}
