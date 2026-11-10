//! The ghost's package: its own plugin, staged from `plugin/` into a
//! prefix with the executable beside it. Every `@ghost-claude@` in the
//! plugin (the hooks, the MCP config, the reflect skill) names
//! `<prefix>/bin/ghost-claude`, so a generation's plugin always runs that
//! generation's binary; the version is the hash of the content, by plain's
//! rule: under nix, where the prefix is a store path, it moves with the
//! binary; in a profile dir, only with the plugin's content. See the
//! initiative spec `harness-core`, "Building the packages (D3)".

use anyhow::{bail, Context, Result};
use plain::stage::{self, write_json};
use serde_json::json;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// The marketplace the ghost's plugin is installed from.
pub const MARKETPLACE: &str = "ghost-in-shell";
/// The plugin's name, and its id in Claude Code.
pub const PLUGIN: &str = "ghost";
pub const PLUGIN_ID: &str = "ghost@ghost-in-shell";
/// Where the marketplace and the plugin are under a prefix.
pub const MARKETPLACE_DIR: &str = "share/ghost/marketplace";
pub const PLUGIN_DIR: &str = "share/ghost/marketplace/plugins/ghost";
/// The executable under a prefix.
pub const EXE: &str = "bin/ghost-claude";
/// What the plugin names the executable as, until it is staged.
pub const PLACEHOLDER: &str = "@ghost-claude@";

/// The files that name the executable.
const NAMING: &[&str] = &["hooks/hooks.json", ".mcp.json", "skills/reflect/SKILL.md"];

/// Stage the ghost's package from `plugin` (the authored dir) into `out`,
/// which must not exist, for installing at `prefix`: `exe` copied to
/// `bin/ghost-claude`, the plugin and its marketplace. Returns the version.
pub fn stage(plugin: &Path, exe: &Path, out: &Path, prefix: &Path) -> Result<String> {
    if out.exists() {
        bail!("{} is there already; stage into a new dir", out.display());
    }
    let to = out.join(PLUGIN_DIR);
    stage::copy_tree(plugin, &to)?;
    let named = prefix.join(EXE).display().to_string();
    // Claude Code runs a hook's command through a shell, and the path goes
    // into JSON text: a path that needs quoting in either is refused.
    if let Some(c) = named
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || "/._-+@:,=".contains(*c)))
    {
        bail!("{named} holds {c:?}; install the ghost under a path of plain characters");
    }
    for rel in NAMING {
        let at = to.join(rel);
        let text =
            fs::read_to_string(&at).with_context(|| format!("{} is missing", at.display()))?;
        fs::write(&at, text.replace(PLACEHOLDER, &named))?;
    }
    if let Some(left) = left_unfilled(&to)? {
        bail!("{} still names {PLACEHOLDER}", left.display());
    }
    let version = stage::version(&to)?;
    stage::fill_version(&to, &version)?;
    write_json(
        &out.join(MARKETPLACE_DIR)
            .join(".claude-plugin/marketplace.json"),
        &json!({
            "name": MARKETPLACE,
            "owner": {"name": "mAId"},
            "description": "The ghost in Claude Code",
            "plugins": [{"name": PLUGIN, "source": "./plugins/ghost"}],
        }),
    )?;
    let bin = out.join(EXE);
    fs::create_dir_all(bin.parent().unwrap())?;
    fs::copy(exe, &bin).with_context(|| format!("cannot copy {}", exe.display()))?;
    fs::set_permissions(&bin, fs::Permissions::from_mode(0o755))?;
    Ok(version)
}

/// A file under `dir` that still holds the placeholder.
fn left_unfilled(dir: &Path) -> Result<Option<PathBuf>> {
    for e in fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_dir() {
            if let Some(f) = left_unfilled(&p)? {
                return Ok(Some(f));
            }
        } else if fs::read_to_string(&p).is_ok_and(|t| t.contains(PLACEHOLDER)) {
            return Ok(Some(p));
        }
    }
    Ok(None)
}

/// Stage and install the ghost's package into `prefix`.
pub fn install(plugin: &Path, exe: &Path, prefix: &Path) -> Result<String> {
    let name = prefix.file_name().unwrap_or_default().to_string_lossy();
    let staged = prefix.with_file_name(format!(".{name}.staged"));
    if staged.exists() {
        fs::remove_dir_all(&staged)?;
    }
    let version = stage(plugin, exe, &staged, prefix)?;
    stage::install(&staged, prefix, MARKETPLACE_DIR)?;
    fs::remove_dir_all(&staged)?;
    Ok(version)
}

/// The version the plugin has in an installed prefix.
pub fn version_in(prefix: &Path) -> Option<String> {
    let at = prefix.join(PLUGIN_DIR).join(".claude-plugin/plugin.json");
    let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(at).ok()?).ok()?;
    v["version"].as_str().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn plugin() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("plugin")
    }

    #[test]
    fn every_placeholder_names_the_prefixs_binary_and_none_is_left() {
        let dir = TempDir::new().unwrap();
        let exe = dir.path().join("exe");
        fs::write(&exe, "binary").unwrap();
        let prefix = Path::new("/p/ghost");
        let out = dir.path().join("out");
        let v = stage(&plugin(), &exe, &out, prefix).unwrap();
        let p = out.join(PLUGIN_DIR);
        for rel in NAMING {
            let text = fs::read_to_string(p.join(rel)).unwrap();
            assert!(text.contains("/p/ghost/bin/ghost-claude"), "{rel}");
        }
        assert_eq!(left_unfilled(&out).unwrap(), None);
        let manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(p.join(".claude-plugin/plugin.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest["name"], "ghost");
        assert_eq!(manifest["version"], v);
        assert_eq!(
            manifest["extensions"]["my-harness"]["name"],
            "claude-adapter"
        );
        assert_eq!(fs::read_to_string(out.join(EXE)).unwrap(), "binary");
        assert!(out
            .join(MARKETPLACE_DIR)
            .join(".claude-plugin/marketplace.json")
            .is_file());
    }

    #[test]
    fn a_prefix_the_hooks_could_not_name_plainly_is_refused() {
        let dir = TempDir::new().unwrap();
        let exe = dir.path().join("exe");
        fs::write(&exe, "binary").unwrap();
        for bad in ["/p/my ghost", "/p/$(x)", "/p/a\"b"] {
            let out = dir.path().join("out");
            let e = stage(&plugin(), &exe, &out, Path::new(bad))
                .unwrap_err()
                .to_string();
            assert!(e.contains("plain characters"), "{bad}: {e}");
            let _ = fs::remove_dir_all(&out);
        }
    }

    #[test]
    fn the_version_moves_with_the_prefix_and_with_a_plugin_file() {
        let dir = TempDir::new().unwrap();
        let exe = dir.path().join("exe");
        fs::write(&exe, "binary").unwrap();
        let at = |n: &str, prefix: &str, src: &Path| {
            stage(src, &exe, &dir.path().join(n), Path::new(prefix)).unwrap()
        };
        let a = at("a", "/p/one", &plugin());
        assert_eq!(at("b", "/p/one", &plugin()), a);
        assert_ne!(
            at("c", "/p/two", &plugin()),
            a,
            "the hooks name another binary"
        );
        let src = dir.path().join("src");
        stage::copy_tree(&plugin(), &src).unwrap();
        fs::write(src.join("agents/reflector.md"), "changed").unwrap();
        assert_ne!(at("d", "/p/one", &src), a);
    }
}
