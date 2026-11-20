//! Stage: plain's package, built from `resources/` into a prefix. The
//! `maid` plugin (its skills, the declarations it ships for the servers it
//! registers and the units it borrows, its claude and codex manifests), the
//! marketplace both agents install it from, the browser launcher, and each
//! runtime it is given. The plugin's version is the hash of its content, so
//! it changes exactly when the content does. The copy and the hash are
//! public, so another package stages itself the same way. See the
//! initiative spec `harness-core`, "Building the packages (D3)".

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};

/// The plugin's dir under a prefix.
pub const PLUGIN: &str = "share/maid/marketplace/plugins/maid";
/// The marketplace's dir under a prefix.
pub const MARKETPLACE: &str = "share/maid/marketplace";
/// Where a manifest's version goes until the content is hashed.
pub const VERSION: &str = "@version@";

const DESCRIPTION: &str = "mAId's skills";
/// Where a change to mAId's units is proposed; its skills, server
/// declarations and overlays inside the plugin inherit it.
pub const REPOSITORY: &str = "https://github.com/kusimari/mAId";

/// A runtime the package links into its `bin/`, by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Runtime {
    pub name: String,
    pub path: PathBuf,
}

impl std::str::FromStr for Runtime {
    type Err = String;
    /// `NAME=PATH`, the path absolute.
    fn from_str(s: &str) -> Result<Runtime, String> {
        let (name, path) = s
            .split_once('=')
            .filter(|(n, p)| !n.is_empty() && !n.contains('/') && Path::new(p).is_absolute())
            .ok_or_else(|| format!("{s:?} is not NAME=/absolute/path"))?;
        Ok(Runtime {
            name: name.into(),
            path: path.into(),
        })
    }
}

/// Stage plain's package from `resources` into `out`, which must not
/// exist. Returns the plugin's version.
pub fn plain(resources: &Path, out: &Path, runtimes: &[Runtime]) -> Result<String> {
    if out.exists() {
        bail!("{} is there already; stage into a new dir", out.display());
    }
    let plugin = out.join(PLUGIN);
    for dir in ["skills", "servers", "overlays"] {
        copy_tree(&resources.join(dir), &plugin.join(dir))?;
    }
    let claude = json!({
        "name": "maid",
        "version": VERSION,
        "description": DESCRIPTION,
        "author": {"name": "mAId"},
        "repository": REPOSITORY,
    });
    let codex = json!({
        "name": "maid",
        "version": VERSION,
        "description": DESCRIPTION,
        "skills": "./skills/",
    });
    write_json(&plugin.join(".claude-plugin/plugin.json"), &claude)?;
    write_json(&plugin.join(".codex-plugin/plugin.json"), &codex)?;
    let version = version(&plugin)?;
    fill_version(&plugin, &version)?;
    let market = out.join(MARKETPLACE);
    write_json(
        &market.join(".claude-plugin/marketplace.json"),
        &json!({
            "name": "maid",
            "owner": {"name": "mAId"},
            "description": DESCRIPTION,
            "plugins": [{"name": "maid", "source": "./plugins/maid", "description": DESCRIPTION}],
        }),
    )?;
    write_json(
        &market.join(".agents/plugins/marketplace.json"),
        &json!({
            "name": "maid",
            "plugins": [{
                "name": "maid",
                "source": {"source": "local", "path": "./plugins/maid"},
                "policy": {"installation": "AVAILABLE"},
            }],
        }),
    )?;
    // The paths kiro and agy are linked at, and where status reads the
    // declarations: the plugin's own, so there is one copy.
    for dir in ["skills", "servers", "overlays"] {
        symlink(
            format!("marketplace/plugins/maid/{dir}"),
            out.join("share/maid").join(dir),
        )?;
    }
    let bin = out.join("bin");
    fs::create_dir_all(&bin)?;
    let launcher = bin.join("maid-browser-mcp");
    fs::copy(resources.join("browser/launch"), &launcher)?;
    fs::set_permissions(&launcher, fs::Permissions::from_mode(0o755))?;
    link_runtimes(&bin, runtimes)?;
    Ok(version)
}

/// Link each runtime into `bin/` by its name.
pub fn link_runtimes(bin: &Path, runtimes: &[Runtime]) -> Result<()> {
    fs::create_dir_all(bin)?;
    for r in runtimes {
        symlink(&r.path, bin.join(&r.name))
            .with_context(|| format!("cannot link {} into {}", r.name, bin.display()))?;
    }
    Ok(())
}

/// Install a staged tree at `prefix`: copy it beside the prefix, then put
/// it in the prefix's place, so a prefix is never half one install and
/// half another. The prefix is replaced whole, so one that is there and is
/// not an empty dir or an earlier install of this package (it holds
/// `marker`) is refused: `--prefix ~/.local` must not delete `~/.local`.
/// The `.<name>.next` and `.<name>.old` beside it are this step's own.
pub fn install(staged: &Path, prefix: &Path, marker: &str) -> Result<()> {
    if marker.is_empty() {
        bail!(
            "no marker names an earlier install of the package at {}",
            prefix.display()
        );
    }
    if fs::symlink_metadata(prefix).is_ok() {
        let empty = fs::read_dir(prefix).is_ok_and(|mut d| d.next().is_none());
        if !empty && !prefix.join(marker).exists() {
            bail!(
                "{} is there and is not an earlier install of this package (no {marker}); install into a new or empty dir",
                prefix.display()
            );
        }
    }
    let parent = prefix
        .parent()
        .ok_or_else(|| anyhow!("{} has no parent", prefix.display()))?;
    fs::create_dir_all(parent)?;
    let name = prefix.file_name().unwrap_or_default().to_string_lossy();
    let next = parent.join(format!(".{name}.next"));
    let old = parent.join(format!(".{name}.old"));
    for p in [&next, &old] {
        remove(p)?;
    }
    copy_tree(staged, &next)?;
    if fs::symlink_metadata(prefix).is_ok() {
        fs::rename(prefix, &old)?;
    }
    fs::rename(&next, prefix)?;
    remove(&old)
}

fn remove(p: &Path) -> Result<()> {
    match fs::symlink_metadata(p) {
        Ok(m) if m.is_dir() => fs::remove_dir_all(p)?,
        Ok(_) => fs::remove_file(p)?,
        Err(_) => {}
    }
    Ok(())
}

/// The version for a plugin's content: `1.0.0-` and 12 hex of a sha256
/// over every file's path, whether it is executable, and its bytes, in
/// path order, a manifest's version still `@version@`. A link is hashed by
/// its target.
pub fn version(plugin: &Path) -> Result<String> {
    let mut files = vec![];
    walk(plugin, Path::new(""), &mut files)?;
    files.sort();
    let mut h = Sha256::new();
    for rel in &files {
        let at = plugin.join(rel);
        h.update(rel.to_string_lossy().as_bytes());
        h.update([0]);
        match fs::read_link(&at) {
            Ok(target) => h.update(target.to_string_lossy().as_bytes()),
            Err(_) => {
                let exec = fs::metadata(&at)?.permissions().mode() & 0o111 != 0;
                h.update([u8::from(exec)]);
                h.update(fs::read(&at)?);
            }
        }
        h.update([0]);
    }
    let hex: String = h
        .finalize()
        .iter()
        .take(6)
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok(format!("1.0.0-{hex}"))
}

fn walk(root: &Path, rel: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for e in fs::read_dir(root.join(rel))? {
        let e = e?;
        let r = rel.join(e.file_name());
        match e.file_type()?.is_dir() {
            true => walk(root, &r, out)?,
            false => out.push(r),
        }
    }
    Ok(())
}

/// Write `version` into every manifest under the plugin that holds
/// `@version@`.
pub fn fill_version(plugin: &Path, version: &str) -> Result<()> {
    for m in [".claude-plugin/plugin.json", ".codex-plugin/plugin.json"] {
        let at = plugin.join(m);
        if let Ok(text) = fs::read_to_string(&at) {
            fs::write(&at, text.replace(VERSION, version))?;
        }
    }
    Ok(())
}

/// Copy a tree, keeping links as links and each file's mode; nothing
/// there, nothing copied.
pub fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    let Ok(meta) = fs::symlink_metadata(from) else {
        return Ok(());
    };
    if meta.file_type().is_symlink() {
        if let Some(p) = to.parent() {
            fs::create_dir_all(p)?;
        }
        symlink(fs::read_link(from)?, to)?;
    } else if meta.is_dir() {
        fs::create_dir_all(to)?;
        for e in fs::read_dir(from)? {
            let e = e?;
            copy_tree(&e.path(), &to.join(e.file_name()))?;
        }
    } else {
        if let Some(p) = to.parent() {
            fs::create_dir_all(p)?;
        }
        fs::copy(from, to).with_context(|| format!("cannot copy {}", from.display()))?;
        // A store path's files are read-only; a copy is the installer's.
        let mode = meta.permissions().mode() | 0o200;
        fs::set_permissions(to, fs::Permissions::from_mode(mode))?;
    }
    Ok(())
}

/// Write pretty JSON, making its dir.
pub fn write_json(at: &Path, v: &Value) -> Result<()> {
    if let Some(p) = at.parent() {
        fs::create_dir_all(p)?;
    }
    fs::write(at, serde_json::to_string_pretty(v)? + "\n")?;
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

    fn resources(dir: &Path) -> PathBuf {
        let r = dir.join("resources");
        write(
            &r.join("skills/notes/SKILL.md"),
            "---\nname: notes\ndescription: d\n---\n",
        );
        write(&r.join("servers/s/server.json"), r#"{"name":"s"}"#);
        write(&r.join("overlays/o/plugin.json"), r#"{"name":"o"}"#);
        write(&r.join("browser/launch"), "#!/bin/sh\n");
        r
    }

    fn manifest(out: &Path, m: &str) -> Value {
        serde_json::from_str(&fs::read_to_string(out.join(PLUGIN).join(m)).unwrap()).unwrap()
    }

    #[test]
    fn the_package_has_the_plugin_its_marketplace_and_the_launcher() {
        let dir = TempDir::new().unwrap();
        let out = dir.path().join("out");
        let runtime = Runtime {
            name: "pensieve".into(),
            path: "/nix/store/x/bin/pensieve".into(),
        };
        let v = plain(&resources(dir.path()), &out, &[runtime]).unwrap();
        assert!(v.starts_with("1.0.0-") && v.len() == 18, "{v}");
        let claude = manifest(&out, ".claude-plugin/plugin.json");
        assert_eq!(claude["version"], v);
        assert_eq!(claude["repository"], REPOSITORY);
        for key in ["extensions", "hooks", "mcpServers"] {
            assert!(claude.get(key).is_none(), "{key}: {claude}");
        }
        assert_eq!(manifest(&out, ".codex-plugin/plugin.json")["version"], v);
        assert!(out.join(PLUGIN).join("skills/notes/SKILL.md").is_file());
        assert!(out.join("share/maid/skills/notes/SKILL.md").is_file());
        assert!(out.join("share/maid/servers/s/server.json").is_file());
        assert!(out
            .join(MARKETPLACE)
            .join(".claude-plugin/marketplace.json")
            .is_file());
        assert!(out
            .join(MARKETPLACE)
            .join(".agents/plugins/marketplace.json")
            .is_file());
        let mode = fs::metadata(out.join("bin/maid-browser-mcp"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111);
        assert_eq!(
            fs::read_link(out.join("bin/pensieve")).unwrap(),
            Path::new("/nix/store/x/bin/pensieve")
        );
    }

    #[test]
    fn the_version_changes_exactly_when_a_plugin_file_does() {
        let dir = TempDir::new().unwrap();
        let r = resources(dir.path());
        let stage = |n: &str| plain(&r, &dir.path().join(n), &[]).unwrap();
        let first = stage("a");
        assert_eq!(stage("b"), first, "the same content, the same version");
        write(&r.join("browser/launch"), "#!/bin/sh\necho other\n");
        assert_eq!(stage("c"), first, "the launcher is not in the plugin");
        let script = r.join("skills/notes/run.sh");
        write(&script, "#!/bin/sh\n");
        let with_script = stage("d");
        assert_ne!(with_script, first);
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        assert_ne!(
            stage("e"),
            with_script,
            "a file made executable is a change"
        );
    }

    #[test]
    fn staging_over_a_tree_is_refused() {
        let dir = TempDir::new().unwrap();
        let out = dir.path().join("out");
        fs::create_dir_all(&out).unwrap();
        assert!(plain(&resources(dir.path()), &out, &[]).is_err());
    }

    #[test]
    fn install_puts_the_staged_tree_in_the_prefix_whole() {
        let dir = TempDir::new().unwrap();
        let staged = dir.path().join("staged");
        write(&staged.join("bin/new"), "new");
        let prefix = dir.path().join("profile");
        write(&prefix.join("bin/old"), "old");
        install(&staged, &prefix, "bin/old").unwrap();
        assert!(prefix.join("bin/new").is_file());
        assert!(!prefix.join("bin/old").exists());
        let left: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(left.len(), 2, "{left:?}");
    }

    #[test]
    fn install_refuses_a_prefix_that_is_not_an_earlier_install() {
        let dir = TempDir::new().unwrap();
        let staged = dir.path().join("staged");
        write(&staged.join("share/maid/marketplace/x"), "new");
        let prefix = dir.path().join("local");
        write(&prefix.join("lib/mine"), "the person's");
        let e = install(&staged, &prefix, MARKETPLACE)
            .unwrap_err()
            .to_string();
        assert!(e.contains("not an earlier install"), "{e}");
        assert!(prefix.join("lib/mine").is_file(), "nothing removed");
        fs::create_dir_all(dir.path().join("empty")).unwrap();
        install(&staged, &dir.path().join("empty"), MARKETPLACE).unwrap();
        install(&staged, &dir.path().join("empty"), MARKETPLACE).unwrap();
    }

    #[test]
    fn a_runtime_is_name_equals_an_absolute_path() {
        assert!("pensieve=/a/b".parse::<Runtime>().is_ok());
        for bad in ["pensieve", "=/a", "p=rel", "a/b=/c"] {
            assert!(bad.parse::<Runtime>().is_err(), "{bad}");
        }
    }
}
