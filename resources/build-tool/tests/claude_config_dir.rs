//! With `CLAUDE_CONFIG_DIR` set, install, uninstall and status read and
//! write Claude's user config in that dir, and nothing under `$HOME`. A stub
//! `claude` keeps its files where Claude Code 2.1.314 does.

use build_tool::shared::Agent;
use build_tool::stages::{cmd_install_memory, cmd_install_reads};
use maid_core::config::ClaudeConfig;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

/// The global file and settings sit in `$CLAUDE_CONFIG_DIR` when it is set,
/// else at `~/.claude.json` and in `~/.claude`.
const STUB: &str = r#"#!/bin/sh
g=${CLAUDE_CONFIG_DIR:+$CLAUDE_CONFIG_DIR/.claude.json}
g=${g:-$HOME/.claude.json}
echo "$*" >> "$HOME/../calls"
case "$1 $2" in
  "mcp add")
    name=$5; shift 6
    printf '{"mcpServers":{"%s":{"type":"stdio","command":"%s","args":["%s","%s"]}}}\n' "$name" "$1" "$2" "$3" > "$g" ;;
  "mcp remove") printf '{"mcpServers":{}}\n' > "$g" ;;
  *) echo '[]' ;;
esac
"#;

struct Env {
    _tmp: TempDir,
    root: PathBuf,
}

impl Env {
    /// HOME, Claude's config dir, a profile with mAId's plugin, a store
    /// and the stub first on PATH.
    fn new() -> Env {
        let tmp = TempDir::new().unwrap();
        let root = fs::canonicalize(tmp.path()).unwrap();
        let plugin = "profile/share/maid/marketplace/plugins/maid";
        for d in [
            "home",
            "claude-config",
            "parts",
            "bin",
            "config",
            "store",
            "profile/bin",
            plugin,
        ] {
            fs::create_dir_all(root.join(d)).unwrap();
        }
        let m = root.join("profile/share/maid/marketplace");
        for (at, text) in [
            (
                m.join(".claude-plugin/marketplace.json"),
                r#"{"name":"maid","owner":{"name":"mAId"},"plugins":[{"name":"maid","source":"./plugins/maid"}]}"#,
            ),
            (
                m.join("plugins/maid/.claude-plugin/plugin.json"),
                r#"{"name":"maid","version":"1.0.0"}"#,
            ),
            (
                m.join("plugins/maid/.codex-plugin/plugin.json"),
                r#"{"name":"maid","version":"1.0.0","skills":"./skills/"}"#,
            ),
        ] {
            fs::create_dir_all(at.parent().unwrap()).unwrap();
            fs::write(at, text).unwrap();
        }
        let skills = Path::new(env!("CARGO_MANIFEST_DIR")).join("../content/skills");
        std::os::unix::fs::symlink(
            fs::canonicalize(skills).unwrap(),
            root.join("profile/share/maid/skills"),
        )
        .unwrap();
        fs::write(root.join("profile/bin/pensieve"), "#!/bin/sh\n").unwrap();
        fs::write(root.join("bin/claude"), STUB).unwrap();
        fs::set_permissions(root.join("bin/claude"), fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(
            root.join("memory-store"),
            format!("{}\n", root.join("store").display()),
        )
        .unwrap();
        Env { _tmp: tmp, root }
    }

    fn at(&self, p: &str) -> PathBuf {
        self.root.join(p)
    }

    fn claude(&self) -> ClaudeConfig {
        ClaudeConfig::new(&self.at("home"), Some(&self.at("claude-config")))
    }

    fn json(&self, p: &str) -> serde_json::Value {
        serde_json::from_str(&fs::read_to_string(self.at(p)).unwrap()).unwrap()
    }

    fn run(&self, args: &[&str]) -> String {
        let out = Command::new(env!("CARGO_BIN_EXE_build-tool"))
            .args(args)
            .env_clear()
            .env("CARGO_MANIFEST_DIR", env!("CARGO_MANIFEST_DIR"))
            .env("HOME", self.at("home"))
            .env("CLAUDE_CONFIG_DIR", self.at("claude-config"))
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.at("bin").display()),
            )
            .env("MAID_PROFILE", self.at("profile"))
            .env("MY_HARNESS_PARTS", self.at("parts"))
            .env("MAID_CONFIG", self.at("config"))
            .env("MAID_MEMORY_STORE", self.at("memory-store"))
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&out.stdout).to_string()
            + &String::from_utf8_lossy(&out.stderr);
        assert!(out.status.success(), "{args:?}: {text}");
        text
    }

    /// Every `.claude*` entry under HOME.
    fn claude_under_home(&self) -> Vec<PathBuf> {
        fs::read_dir(self.at("home"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(".claude")
            })
            .collect()
    }
}

fn put(at: &Path, text: &str) {
    fs::write(at, text).unwrap();
}

/// R3-1 as the report found it: a `personal-memory` that is not mAId's,
/// in the config dir, must stop the server step; settings there are read.
#[test]
fn the_install_plan_reads_claudes_config_dir_not_home() {
    let e = Env::new();
    put(
        &e.at("claude-config/.claude.json"),
        r#"{"mcpServers":{"personal-memory":{"command":"/elsewhere/pensieve"}}}"#,
    );
    let mine = e.at("profile/share/maid/marketplace/plugins/maid");
    put(
        &e.at("claude-config/settings.json"),
        &format!(
            r#"{{"permissions":{{"additionalDirectories":["{}"]}}}}"#,
            mine.display()
        ),
    );
    let plan = e.run(&["install", "--agent", "claude", "--dry-run"]);
    assert!(
        plan.contains("skip          claude mcp personal-memory (not mAId's)"),
        "{plan}"
    );
    assert!(
        !plan.contains("permissions.additionalDirectories +"),
        "the grant already in the config dir is found: {plan}"
    );
    assert!(
        !plan.contains(&e.at("home").display().to_string()),
        "{plan}"
    );
    assert_eq!(e.claude_under_home(), Vec::<PathBuf>::new());
}

/// Install's claude steps write only in the config dir; status and
/// uninstall, through the binary, read and take them back there. Install's
/// own run through the binary is the plan test above: its plugin step needs
/// a claude that installs plugins, which the stub does not.
#[test]
fn install_status_and_uninstall_use_the_config_dir_only() {
    let e = Env::new();
    // The steps find claude on the process PATH; this binary's own.
    std::env::set_var("PATH", e.at("bin"));
    let (c, profile, parts) = (e.claude(), e.at("profile"), e.at("parts"));
    put(&e.at("claude-config/settings.json"), r#"{"theme":"dark"}"#);
    let store_file = e.at("memory-store");
    cmd_install_memory(
        &c,
        &store_file,
        &profile,
        &parts,
        false,
        Some(Agent::Claude),
    )
    .unwrap();
    cmd_install_reads(
        &c,
        &profile,
        &e.at("config"),
        &parts,
        false,
        Some(Agent::Claude),
    )
    .unwrap();

    let settings = e.json("claude-config/settings.json");
    let mine = e.at("profile/share/maid/marketplace/plugins/maid");
    assert_eq!(settings["theme"], "dark");
    assert_eq!(
        settings["permissions"]["additionalDirectories"],
        serde_json::json!([mine.display().to_string()])
    );
    assert_eq!(settings["env"]["CLAUDE_CODE_DISABLE_AUTO_MEMORY"], "1");
    let global = e.json("claude-config/.claude.json");
    assert_eq!(
        global["mcpServers"]["personal-memory"]["command"],
        e.at("profile/bin/pensieve").display().to_string()
    );
    assert_eq!(e.claude_under_home(), Vec::<PathBuf>::new());

    // Another part's skill and agent, in the config dir as Claude reads them.
    put(
        &e.at("parts/other.json"),
        r#"{"format":1,"part":"other","harnesses":{"claude":[
            {"kind":"skill","name":"theirs"},{"kind":"agent","name":"helper"}]}}"#,
    );
    fs::create_dir_all(e.at("claude-config/skills/theirs")).unwrap();
    put(
        &e.at("claude-config/skills/theirs/SKILL.md"),
        "---\nname: theirs\n---\n",
    );
    fs::create_dir_all(e.at("claude-config/agents")).unwrap();
    put(
        &e.at("claude-config/agents/helper.md"),
        "---\nname: helper\n---\n",
    );
    let status = e.run(&["whole", "--agent", "claude"]);
    for line in [
        "present        mcp personal-memory",
        "present        setting permissions.additionalDirectories",
        "present        setting env.CLAUDE_CODE_DISABLE_AUTO_MEMORY",
        "present        skill theirs",
        "present        agent helper",
    ] {
        assert!(status.contains(line), "{line}: {status}");
    }
    let memory = e.run(&["memory", "status"]);
    assert!(memory.contains("personal-memory runs"), "{memory}");
    assert!(memory.contains("(mAId's)"), "{memory}");

    fs::remove_file(e.at("parts/other.json")).unwrap();
    e.run(&["uninstall", "--agent", "claude"]);
    assert_eq!(
        e.json("claude-config/settings.json"),
        serde_json::json!({"theme": "dark"})
    );
    assert_eq!(
        e.json("claude-config/.claude.json"),
        serde_json::json!({"mcpServers": {}})
    );
    assert_eq!(e.claude_under_home(), Vec::<PathBuf>::new());
}
