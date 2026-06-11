//! Walk `mAId/resources/content/<kind>/` and return validated records.
//!
//! Today the only kind with content is `skills` (`<root>/skills/<name>/SKILL.md`).
//! `agents/` and `commands/` slots remain in the walker for future content;
//! the registry doesn't deploy them today (they were phantom slots dropped
//! in feat/resources-and-kaimux). If they reappear, the walker is ready.
//!
//! Returns records sorted deterministically by (kind, name).
//! Collects all schema errors rather than failing on the first.

use crate::schema::{self, SchemaError};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Skills,
    Agents,
    Commands,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Skills => "skills",
            Kind::Agents => "agents",
            Kind::Commands => "commands",
        }
    }

    fn order(self) -> u8 {
        match self {
            Kind::Agents => 0,
            Kind::Commands => 1,
            Kind::Skills => 2,
        }
    }
}

pub const ALL_KINDS: &[Kind] = &[Kind::Skills, Kind::Agents, Kind::Commands];

#[derive(Debug)]
pub struct SourceRecord {
    pub kind: Kind,
    pub name: String,
    #[allow(dead_code)] // Kept for parity with TS sources.ts; future
    // verbs (e.g. show, where) will read this.
    pub path: PathBuf,
}

/// Walk all kinds under `root_dir`. Returns sorted records on
/// success. If any schema validation fails, returns the joined
/// error messages — same behavior as the TS suite.
///
/// Also enforces that `<root_dir>/agents.md` exists and is
/// non-empty if present. AGENTS.md is plain markdown (no
/// frontmatter per the cross-tool standard), so we don't run the
/// SKILL.md-shape validator on it; we just refuse to deploy a
/// missing or empty preamble. If the file is absent, the registry
/// entries pointing at it surface as `SkippedMissingSource` at
/// deploy time anyway.
pub fn walk(root_dir: &Path) -> Result<Vec<SourceRecord>, String> {
    let mut records = Vec::new();
    let mut errors: Vec<SchemaError> = Vec::new();

    // AGENTS.md preamble — presence + non-empty check.
    let preamble = root_dir.join("agents.md");
    if preamble.exists() {
        match fs::read_to_string(&preamble) {
            Ok(content) if content.trim().is_empty() => {
                errors.push(SchemaError {
                    file_path: preamble.to_string_lossy().into(),
                    line: 1,
                    message: "AGENTS.md preamble is empty".into(),
                });
            }
            Ok(_) => {
                records.push(SourceRecord {
                    kind: Kind::Skills, // bucket it somewhere; not used downstream
                    name: "agents.md".into(),
                    path: preamble,
                });
            }
            Err(e) => {
                errors.push(SchemaError {
                    file_path: preamble.to_string_lossy().into(),
                    line: 1,
                    message: format!("cannot read AGENTS.md preamble: {e}"),
                });
            }
        }
    }

    for kind in ALL_KINDS {
        let kind_dir = root_dir.join(kind.as_str());
        if !kind_dir.exists() {
            continue;
        }

        match kind {
            Kind::Skills => walk_skills(&kind_dir, &mut records, &mut errors),
            Kind::Agents | Kind::Commands => walk_flat(*kind, &kind_dir, &mut records, &mut errors),
        }
    }

    if !errors.is_empty() {
        let msg: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        return Err(format!("Schema validation failed:\n{}", msg.join("\n")));
    }

    records.sort_by(|a, b| match a.kind.order().cmp(&b.kind.order()) {
        std::cmp::Ordering::Equal => a.name.cmp(&b.name),
        other => other,
    });

    Ok(records)
}

fn walk_skills(dir: &Path, out: &mut Vec<SourceRecord>, errs: &mut Vec<SchemaError>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let ft = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if !ft.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let skill_path = entry.path().join("SKILL.md");
        if !skill_path.exists() {
            continue;
        }
        match fs::read_to_string(&skill_path) {
            Ok(content) => {
                let path_str = skill_path.to_string_lossy();
                if let Err(e) = schema::validate(&path_str, &content) {
                    errs.push(e);
                } else {
                    out.push(SourceRecord {
                        kind: Kind::Skills,
                        name,
                        path: skill_path,
                    });
                }
            }
            Err(_) => continue,
        }
    }
}

fn walk_flat(kind: Kind, dir: &Path, out: &mut Vec<SourceRecord>, errs: &mut Vec<SchemaError>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let ft = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if !ft.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || !name.ends_with(".md") {
            continue;
        }
        let path = entry.path();
        let bare = name.trim_end_matches(".md").to_string();
        match fs::read_to_string(&path) {
            Ok(content) => {
                let path_str = path.to_string_lossy();
                if let Err(e) = schema::validate(&path_str, &content) {
                    errs.push(e);
                } else {
                    out.push(SourceRecord {
                        kind,
                        name: bare,
                        path,
                    });
                }
            }
            Err(_) => continue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write(p: &Path, s: &str) {
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, s).unwrap();
    }

    #[test]
    fn empty_root_returns_no_records() {
        let root = TempDir::new().unwrap();
        let r = walk(root.path()).unwrap();
        assert!(r.is_empty());
    }

    #[test]
    fn agents_md_present_is_recorded() {
        let root = TempDir::new().unwrap();
        write(&root.path().join("agents.md"), "# preamble\n\nbody.\n");
        let r = walk(root.path()).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].name, "agents.md");
    }

    #[test]
    fn agents_md_empty_is_rejected() {
        let root = TempDir::new().unwrap();
        write(&root.path().join("agents.md"), "");
        let e = walk(root.path()).unwrap_err();
        assert!(e.contains("AGENTS.md preamble is empty"));
    }

    #[test]
    fn agents_md_whitespace_only_is_rejected() {
        let root = TempDir::new().unwrap();
        write(&root.path().join("agents.md"), "   \n\n  \n");
        let e = walk(root.path()).unwrap_err();
        assert!(e.contains("AGENTS.md preamble is empty"));
    }

    #[test]
    fn skill_with_frontmatter_is_parsed() {
        let root = TempDir::new().unwrap();
        write(
            &root.path().join("skills/foo/SKILL.md"),
            "---\nname: foo\ndescription: bar\n---\nbody.\n",
        );
        let r = walk(root.path()).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].name, "foo");
        assert_eq!(r[0].kind, Kind::Skills);
    }

    #[test]
    fn only_skill_md_is_parsed_in_skill_dirs() {
        // Sibling files in a skill dir (e.g. setup.md, interviews.md
        // for kdevkit) must NOT be parsed; only SKILL.md.
        let root = TempDir::new().unwrap();
        let skill_dir = root.path().join("skills/multi");
        write(
            &skill_dir.join("SKILL.md"),
            "---\nname: multi\ndescription: a multi-file skill\n---\nbody.\n",
        );
        write(
            &skill_dir.join("setup.md"),
            "# Plain markdown, no frontmatter.\n",
        );
        write(&skill_dir.join("interviews.md"), "# Same here.\n");

        let r = walk(root.path()).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].name, "multi");
    }

    #[test]
    fn agent_md_is_parsed() {
        let root = TempDir::new().unwrap();
        write(
            &root.path().join("agents/foo.md"),
            "---\nname: foo\ndescription: an agent\n---\n",
        );
        let r = walk(root.path()).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].name, "foo");
        assert_eq!(r[0].kind, Kind::Agents);
    }

    #[test]
    fn malformed_frontmatter_collects_error() {
        let root = TempDir::new().unwrap();
        write(
            &root.path().join("skills/bad/SKILL.md"),
            "---\nname: bad\n---\n",
        );
        let e = walk(root.path()).unwrap_err();
        assert!(e.contains("description"));
    }

    #[test]
    fn dotfile_dirs_skipped() {
        let root = TempDir::new().unwrap();
        write(
            &root.path().join("skills/.hidden/SKILL.md"),
            "---\nname: hidden\ndescription: should be skipped\n---\n",
        );
        let r = walk(root.path()).unwrap();
        assert!(r.is_empty());
    }

    #[test]
    fn sort_order_is_kind_then_name() {
        let root = TempDir::new().unwrap();
        write(
            &root.path().join("agents/zeta.md"),
            "---\nname: zeta\ndescription: a\n---\n",
        );
        write(
            &root.path().join("agents/alpha.md"),
            "---\nname: alpha\ndescription: a\n---\n",
        );
        write(
            &root.path().join("commands/foo.md"),
            "---\nname: foo\ndescription: a\n---\n",
        );
        let r = walk(root.path()).unwrap();
        assert_eq!(r.len(), 3);
        // Agents (alpha, zeta) before commands (foo).
        assert_eq!(r[0].name, "alpha");
        assert_eq!(r[1].name, "zeta");
        assert_eq!(r[2].name, "foo");
    }
}
