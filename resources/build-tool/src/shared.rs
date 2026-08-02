//! Vocabulary every stage of the pipeline speaks: which coding agents
//! exist, where each expects its skills, and where the roots are.
//!
//! Depends on nothing else in the crate — the bottom of the dependency
//! order (`stages` → `harness` → `shared`).

use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;

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
}
