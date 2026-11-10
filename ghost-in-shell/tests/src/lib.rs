//! What the tests that span crates share: where the repo and the ghost's
//! own things are, the fixture session, and the installed binary. The
//! tests themselves are in `tests/`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// The repo root: this crate sits at `ghost-in-shell/tests`.
pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the checkout is there")
}

/// The ghost's own Claude plugin, as the checkout holds it.
pub fn plugin() -> PathBuf {
    repo().join("ghost-in-shell/shell/claude/plugin")
}

/// The person's resources, as the checkout holds them.
pub fn content() -> PathBuf {
    repo().join("resources")
}

/// The binary the carriers call, built from this checkout. Cargo hands a
/// test the path of its own package's binaries only, so this builds
/// `shell-claude`'s once per test binary and reads the path cargo gives.
pub fn core() -> &'static str {
    static CORE: OnceLock<String> = OnceLock::new();
    CORE.get_or_init(|| {
        let out = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args([
                "build",
                "--quiet",
                // The lock is committed, as the flake's build takes it: a
                // test never rewrites it or reaches the network for it.
                "--locked",
                "--message-format",
                "json-render-diagnostics",
                "-p",
                "shell-claude",
                "--bin",
                "ghost-claude",
            ])
            .current_dir(repo())
            .output()
            .expect("cargo on PATH");
        assert!(
            out.status.success(),
            "cargo build -p shell-claude: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .filter_map(|v| v["executable"].as_str().map(PathBuf::from))
            .find(|p| p.file_name().is_some_and(|n| n == "ghost-claude"))
            .expect("cargo says where it put ghost-claude")
            .display()
            .to_string()
    })
}

/// The fixture Claude Code session (`tests/fixtures/claude/-work`).
pub const SESSION: &str = "0b7c6a8e-1111-4222-8333-944455556666";

/// The fixture projects dir, as the checkout holds it.
pub fn fixture_projects() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/claude")
}

/// A copy of the fixture projects dir at `<dir>/claude`.
pub fn claude_projects(dir: &Path) -> PathBuf {
    let to = dir.join("claude");
    copy(&fixture_projects(), &to);
    to
}

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let t = to.join(e.file_name());
        match e.file_type().unwrap().is_dir() {
            true => copy(&e.path(), &t),
            false => {
                std::fs::copy(e.path(), t).unwrap();
            }
        }
    }
}

/// A Claude Code config dir laid out as Claude Code 2.1.319 keeps it: the
/// plugin registry, `enabledPlugins` in the user's settings, MCP servers in
/// `.claude.json`, and the user's skills dir. A test installs units into it
/// as Claude Code would, and the ghost reads them back from there.
pub struct ClaudeHome {
    pub home: PathBuf,
    dir: PathBuf,
    global: PathBuf,
}

impl ClaudeHome {
    /// Claude Code's user config under `home` (`CLAUDE_CONFIG_DIR` unset).
    pub fn at(home: &Path) -> ClaudeHome {
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        ClaudeHome {
            home: home.to_path_buf(),
            dir: home.join(".claude"),
            global: home.join(".claude.json"),
        }
    }

    /// Claude Code's user config all in `dir`, as `CLAUDE_CONFIG_DIR` sets.
    pub fn in_dir(dir: &Path) -> ClaudeHome {
        std::fs::create_dir_all(dir).unwrap();
        ClaudeHome {
            home: dir.to_path_buf(),
            dir: dir.to_path_buf(),
            global: dir.join(".claude.json"),
        }
    }

    pub fn dir(&self) -> PathBuf {
        self.dir.clone()
    }

    fn edit(&self, file: &Path, change: impl FnOnce(&mut serde_json::Value)) {
        let mut v: serde_json::Value = std::fs::read_to_string(file)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_else(|| serde_json::json!({}));
        change(&mut v);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, v.to_string()).unwrap();
    }

    /// Install the plugin at `at` as `id`, enabled for the user.
    pub fn plugin(&self, id: &str, at: &Path) {
        self.edit(&self.dir().join("plugins/installed_plugins.json"), |v| {
            v["version"] = 2.into();
            v["plugins"][id] = serde_json::json!([{"scope": "user", "installPath": at,
                "installedAt": "2026-10-31T10:00:00.000Z"}]);
        });
        self.edit(&self.dir().join("settings.json"), |v| {
            v["enabledPlugins"][id] = true.into();
        });
    }

    /// The marketplace `name`, from the git repo `url`.
    pub fn marketplace(&self, name: &str, url: &str) {
        self.edit(&self.dir().join("plugins/known_marketplaces.json"), |v| {
            v[name] = serde_json::json!({"source": {"source": "git", "url": url}});
        });
    }

    /// Register the MCP server `name` for the user.
    pub fn mcp(&self, name: &str, command: &str, args: &[&str]) {
        self.edit(&self.global, |v| {
            v["mcpServers"][name] =
                serde_json::json!({"type": "stdio", "command": command, "args": args, "env": {}});
        });
    }

    /// A skill put straight into the user's skills dir.
    pub fn skill(&self, name: &str, text: &str) {
        let d = self.dir().join("skills").join(name);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("SKILL.md"), text).unwrap();
    }
}

/// The adapter's declaration, as the ghost's plugin manifest carries it.
pub fn adapter_declaration() -> serde_json::Value {
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(plugin().join(".claude-plugin/plugin.json")).unwrap(),
    )
    .unwrap();
    manifest["extensions"]["my-harness"].clone()
}

/// mAId's plugin laid out as plain stages it, at `at`: the Claude manifest
/// with its repository, the person's skills, and the server declarations
/// and overlays it ships.
pub fn maid_plugin(at: &Path) -> PathBuf {
    std::fs::create_dir_all(at.join(".claude-plugin")).unwrap();
    std::fs::write(
        at.join(".claude-plugin/plugin.json"),
        serde_json::json!({"name": "maid", "version": "1.0.0",
            "repository": "https://github.com/kusimari/mAId"})
        .to_string(),
    )
    .unwrap();
    copy(&content().join("skills"), &at.join("skills"));
    copy(&content().join("servers"), &at.join("servers"));
    copy(&content().join("overlays"), &at.join("overlays"));
    at.to_path_buf()
}

/// The ghost's plugin as the checkout holds it, copied to `at`: its
/// manifest with the adapter's declaration, hooks, MCP config, reflect and
/// the reflector.
pub fn ghost_plugin(at: &Path) -> PathBuf {
    copy(&plugin(), at);
    at.to_path_buf()
}

/// The ghost's package staged by its own step into `<d>/ghost`, the
/// executable this checkout builds in it: the plugin dir, for a
/// `--plugin-dir` session.
pub fn staged_ghost(d: &Path) -> PathBuf {
    let prefix = d.join("ghost");
    shell_claude::package::stage(&plugin(), Path::new(core()), &prefix, &prefix)
        .expect("the ghost's package stages");
    prefix.join(shell_claude::package::PLUGIN_DIR)
}
