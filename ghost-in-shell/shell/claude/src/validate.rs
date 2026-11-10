//! Validate: every `my-harness` declaration the ghost will read reads, and
//! its `rules` pointer names a section that is there, so a renamed heading
//! never ships. It checks the person's resources and the ghost's own
//! plugin; plain checks only what each standard requires. See the
//! initiative spec `harness-core`, "The bring-in contract".

use ghost::contract::{self, Slot, Standard};
use std::fs;
use std::path::{Path, PathBuf};

/// Every declaration under each root (a `resources/` tree, or a plugin):
/// each skill's `SKILL.md`, each `servers/<name>/server.json`, each
/// `overlays/<name>/plugin.json`, and the root's own
/// `.claude-plugin/plugin.json`. The count checked, or every error.
pub fn validate(roots: &[PathBuf]) -> Result<usize, Vec<String>> {
    let units: Vec<(Standard, PathBuf)> = roots.iter().flat_map(|r| units(r)).collect();
    let errs: Vec<String> = units
        .iter()
        .filter_map(|(standard, p)| {
            declaration(*standard, p)
                .err()
                .map(|e| format!("{}: {e}", p.display()))
        })
        .collect();
    match errs.is_empty() {
        true => Ok(units.len()),
        false => Err(errs),
    }
}

/// The units under one root that may carry a declaration, in a fixed order.
fn units(root: &Path) -> Vec<(Standard, PathBuf)> {
    let each = |dir: &str, file: &str, standard: Standard| {
        let mut found: Vec<PathBuf> = fs::read_dir(root.join(dir))
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|e| e.path().join(file))
            .filter(|p| p.is_file())
            .collect();
        found.sort();
        found.into_iter().map(move |p| (standard, p))
    };
    each("skills", "SKILL.md", Standard::Skill)
        .chain(each("servers", "server.json", Standard::Mcp))
        .chain(each("overlays", "plugin.json", Standard::Plugin))
        .chain(
            Some(root.join(".claude-plugin/plugin.json"))
                .filter(|p| p.is_file())
                .map(|p| (Standard::Plugin, p)),
        )
        .collect()
}

/// A unit's declaration, if it has one, reads, and its rules pointer
/// resolves.
fn declaration(standard: Standard, path: &Path) -> Result<(), String> {
    let path = std::path::absolute(path).map_err(|e| e.to_string())?;
    match contract::read(standard, &path)? {
        Slot::Declared(d) => match &d.rules {
            Some(pointer) => contract::rules(&path, pointer).map(drop),
            None => Ok(()),
        },
        Slot::Undeclared { .. } => Ok(()),
    }
}

/// `ghost-claude validate`: print the count, or fail with every error.
pub fn cmd(roots: &[PathBuf]) -> anyhow::Result<()> {
    let n = validate(roots)
        .map_err(|errs| anyhow::anyhow!("declarations do not read:\n{}", errs.join("\n")))?;
    eprintln!("validated {n} declaration(s)");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(p: &Path, s: &str) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, s).unwrap();
    }

    fn check(dir: &TempDir) -> Result<usize, Vec<String>> {
        validate(&[dir.path().to_path_buf()])
    }

    #[test]
    fn a_shipped_server_or_overlay_must_declare_readably() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("servers/s/server.json"),
            r#"{"name":"s","_meta":{"my-harness":{"contract":1,"scope":"turn","job":"j"}}}"#,
        );
        write(
            &dir.path().join("overlays/o/plugin.json"),
            r#"{"name":"o"}"#,
        );
        assert_eq!(check(&dir).unwrap(), 2);
        write(
            &dir.path().join("servers/s/server.json"),
            r#"{"name":"s","_meta":{"my-harness":{"contract":1,"scope":"turn","job":"j","rules":"README.md#x"}}}"#,
        );
        let errs = check(&dir).unwrap_err();
        assert!(errs[0].contains("cannot read"), "{errs:?}");
        write(&dir.path().join("servers/s/README.md"), "## X\n\nfirst\n");
        assert_eq!(check(&dir).unwrap(), 2);
        write(&dir.path().join("overlays/o/plugin.json"), "not json");
        let errs = check(&dir).unwrap_err();
        assert!(
            errs[0].contains("overlays/o/plugin.json") && errs[0].contains("not JSON"),
            "{errs:?}"
        );
    }

    #[test]
    fn a_declaration_must_read_and_its_rules_resolve() {
        let dir = TempDir::new().unwrap();
        let skill = dir.path().join("skills/foo/SKILL.md");
        let decl = |rules: &str| {
            format!("---\nname: foo\ndescription: bar\nmetadata:\n  my-harness.contract: \"1\"\n  my-harness.scope: turn\n  my-harness.job: j\n  my-harness.rules: {rules}\n---\n## Rules\n\nfirst\n")
        };
        write(&skill, &decl("SKILL.md#rules"));
        assert_eq!(check(&dir).unwrap(), 1);
        write(&skill, &decl("SKILL.md#renamed"));
        let e = check(&dir).unwrap_err();
        assert!(e[0].contains("no heading"), "{e:?}");
        write(&skill, "---\nname: foo\ndescription: bar\nmetadata:\n  my-harness.contract: \"1\"\n  my-harness.scope: always\n  my-harness.job: j\n---\n");
        let e = check(&dir).unwrap_err();
        assert!(e[0].contains("declaration"), "{e:?}");
    }

    #[test]
    fn a_plugins_own_manifest_is_checked() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join(".claude-plugin/plugin.json"),
            r#"{"name":"p","extensions":{"my-harness":{"contract":2,"scope":"session","job":"j"}}}"#,
        );
        let e = check(&dir).unwrap_err();
        assert!(e[0].contains(".claude-plugin/plugin.json"), "{e:?}");
    }
}
