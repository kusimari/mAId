//! Validate: each resource is what its own standard requires, before
//! anything is staged or placed: every skill's frontmatter, no skill named
//! twice, no repo path in a shipped file, and each shipped manifest a JSON
//! object with a name. Nothing of the ghost: its declarations are checked
//! by the ghost's own validate. See project.md "My harness" (the install record).

use anyhow::{anyhow, Result};
use std::fs;
use std::path::{Path, PathBuf};
// ─────────────────────────────────────────────────────────────────
// resources/ → a valid skill.
//
// Only skills are deployed. Each resources/skills/<name>/
// SKILL.md needs YAML frontmatter with a non-empty name + description.
// ─────────────────────────────────────────────────────────────────

/// Validate every root skills are authored under (in the checkout,
/// `shared::skill_roots`; after the build, the one profile dir),
/// returning the count of validated files or the joined error list.
/// Caller decides whether to print or abort.
pub fn check_content(roots: &[PathBuf]) -> Result<usize, Vec<String>> {
    let skills: Vec<PathBuf> = roots
        .iter()
        .flat_map(|root| fs::read_dir(root.join("skills")).into_iter().flatten())
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|dir| dir.join("SKILL.md").exists())
        .collect();
    let errs: Vec<String> = twice(&skills)
        .into_iter()
        .chain(skills.iter().flat_map(|dir| {
            check_one_skill(&dir.join("SKILL.md"))
                .err()
                .into_iter()
                .chain(repo_pointers(dir))
        }))
        .chain(
            roots
                .iter()
                .flat_map(|root| manifests(root))
                .filter_map(|p| manifest(&p).err().map(|e| format!("{}: {e}", p.display()))),
        )
        .collect();
    match errs.is_empty() {
        true => Ok(skills.len()),
        false => Err(errs),
    }
}

/// A name two roots both have a skill under: the build copies both to one
/// place, so one would end up inside the other.
fn twice(skills: &[PathBuf]) -> Vec<String> {
    let mut by_name: std::collections::BTreeMap<String, Vec<&PathBuf>> = Default::default();
    for dir in skills {
        let name = dir.file_name().unwrap_or_default().to_string_lossy();
        by_name.entry(name.into_owned()).or_default().push(dir);
    }
    by_name
        .into_iter()
        .filter(|(_, dirs)| dirs.len() > 1)
        .map(|(name, dirs)| {
            let at: Vec<String> = dirs.iter().map(|d| d.display().to_string()).collect();
            format!("skill {name} is in more than one root: {}", at.join(", "))
        })
        .collect()
}

/// The manifests shipped beside the skills: each MCP server's
/// `servers/<name>/server.json`, and each overlay's
/// `overlays/<name>/plugin.json` (a borrowed plugin's manifest).
pub fn manifests(content_dir: &Path) -> Vec<PathBuf> {
    [("servers", "server.json"), ("overlays", "plugin.json")]
        .into_iter()
        .flat_map(|(dir, file)| {
            let mut found: Vec<_> = fs::read_dir(content_dir.join(dir))
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .map(|e| e.path().join(file))
                .filter(|p| p.exists())
                .collect();
            found.sort();
            found
        })
        .collect()
}

/// What both standards require of a manifest: a JSON object with a
/// non-empty `name`.
fn manifest(path: &Path) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|e| format!("cannot read: {e}"))?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    match v["name"].as_str() {
        Some(n) if !n.trim().is_empty() => Ok(()),
        _ => Err("has no name".into()),
    }
}

/// Text that sends an installed session to the mAId repo, which it never
/// has (see project.md "Skill state"). Matched lowercased.
const REPO_POINTERS: [&str; 5] = [
    "resources/",
    "ghost-in-shell/",
    "maid checkout",
    "just install",
    "just plain::",
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
    match fm.name.trim().is_empty() || fm.description.trim().is_empty() {
        true => Err(at("name and description must be non-empty".into())),
        false => Ok(()),
    }
}

/// Validate content, failing with every error found.
pub fn cmd_validate(roots: &[PathBuf]) -> Result<u8> {
    let count = check_content(roots)
        .map_err(|errs| anyhow!("Content validation failed:\n{}", errs.join("\n")))?;
    eprintln!("validated {count} content file(s)");
    Ok(0)
}

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

    // ── content checks ──────────────────────────────────────────
    #[test]
    fn check_content_empty_root_ok() {
        let dir = TempDir::new().unwrap();
        assert_eq!(check_content(&[dir.path().to_path_buf()]).unwrap(), 0);
    }

    #[test]
    fn check_content_skill_with_frontmatter_ok() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("skills/foo/SKILL.md"),
            "---\nname: foo\ndescription: bar\n---\nbody.\n",
        );
        assert_eq!(check_content(&[dir.path().to_path_buf()]).unwrap(), 1);
    }

    #[test]
    fn one_skill_name_under_two_roots_is_refused() {
        let (a, b) = (TempDir::new().unwrap(), TempDir::new().unwrap());
        for dir in [&a, &b] {
            write(
                &dir.path().join("skills/notes/SKILL.md"),
                "---\nname: notes\ndescription: capture notes\n---\nbody.\n",
            );
        }
        let roots = [a.path().to_path_buf(), b.path().to_path_buf()];
        let errs = check_content(&roots).unwrap_err();
        assert_eq!(errs.len(), 1, "{errs:?}");
        assert!(
            errs[0].starts_with("skill notes is in more than one root: "),
            "{}",
            errs[0]
        );
        assert_eq!(check_content(&roots[..1]).unwrap(), 1);
    }

    #[test]
    fn check_content_skill_missing_description_rejected() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("skills/foo/SKILL.md"),
            "---\nname: foo\n---\n",
        );
        let errs = check_content(&[dir.path().to_path_buf()]).unwrap_err();
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
        let errs = check_content(&[dir.path().to_path_buf()]).unwrap_err();
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
        assert_eq!(check_content(&[dir.path().to_path_buf()]).unwrap(), 1);
    }

    #[test]
    fn check_content_flags_repo_pointers_in_skill_md_and_modules() {
        let dir = TempDir::new().unwrap();
        let skill = dir.path().join("skills/foo");
        write(
            &skill.join("SKILL.md"),
            "---\nname: foo\ndescription: bar\n---\nok\nIn a mAId Checkout, edit it.\n",
        );
        write(&skill.join("phases/a.md"), "x\nsee resources/skills/foo\n");
        write(&skill.join("b.md"), "then run `just install`\n");
        write(&skill.join("c.md"), "or `just plain::browser-mcp-allow`\n");
        write(&skill.join("notes.txt"), "just install\n");
        let errs = check_content(&[dir.path().to_path_buf()]).unwrap_err();
        let at = |f: &str, n: usize| format!("{}:{n}:", skill.join(f).display());
        assert_eq!(errs.len(), 4, "{errs:?}");
        for (e, (f, n, pat)) in errs.iter().zip([
            ("SKILL.md", 6, "maid checkout"),
            ("b.md", 1, "just install"),
            ("c.md", 1, "just plain::"),
            ("phases/a.md", 2, "resources/"),
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
        assert_eq!(check_content(&[dir.path().to_path_buf()]).unwrap(), 1);
    }

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

    /// Plain checks a skill's standard only: a `my-harness` declaration the
    /// ghost would refuse (an unknown contract, a rules pointer to nothing)
    /// is not plain's to judge.
    #[test]
    fn a_declaration_the_ghost_refuses_is_not_plains_business() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("skills/foo/SKILL.md"),
            "---\nname: foo\ndescription: bar\nmetadata:\n  my-harness.contract: \"9\"\n  my-harness.scope: always\n  my-harness.rules: SKILL.md#gone\n---\n",
        );
        write(
            &dir.path().join("servers/s/server.json"),
            r#"{"name":"s","_meta":{"my-harness":{"contract":9}}}"#,
        );
        assert_eq!(check_content(&[dir.path().to_path_buf()]).unwrap(), 1);
    }
}
