//! Install's claude memory step and the part list it leaves, under a temp
//! HOME. Its own test binary, because it sets PATH for the whole process.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};
use tempfile::TempDir;

struct Env {
    tmp: TempDir,
}

impl Env {
    fn new() -> Env {
        let tmp = TempDir::new().unwrap();
        for d in ["home", "config", "state", "stores/personal", "stores/work"] {
            fs::create_dir_all(tmp.path().join(d)).unwrap();
        }
        Env { tmp }
    }
    fn at(&self, p: &str) -> PathBuf {
        fs::canonicalize(self.tmp.path()).unwrap().join(p)
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_build-tool"))
            .args(args)
            .env("HOME", self.at("home"))
            .env("MY_HARNESS_MEMORY", self.at("config"))
            .env("MY_HARNESS_PARTS", self.at("state"))
            .env_remove("CODEX_HOME")
            .output()
            .unwrap()
    }
    fn ok(&self, args: &[&str]) -> String {
        let out = self.run(args);
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(out.status.success(), "{args:?} failed: {text}");
        text
    }
}

#[test]
fn install_points_claude_at_the_default_store_and_records_it() {
    use build_tool::stages::{cmd_install_memory, cmd_uninstall_memory};
    let e = Env::new();
    let bin = e.at("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("claude"), "#!/bin/sh\n").unwrap();
    fs::set_permissions(bin.join("claude"), fs::Permissions::from_mode(0o755)).unwrap();
    std::env::set_var("PATH", &bin);

    let (home, config, parts) = (e.at("home"), e.at("config"), e.at("state"));
    e.ok(&[
        "memory",
        "add-store",
        "personal",
        e.at("stores/personal").to_str().unwrap(),
        "--default",
    ]);
    fs::create_dir_all(home.join(".claude")).unwrap();
    fs::write(
        home.join(".claude/settings.json"),
        "{\"theme\": \"dark\"}\n",
    )
    .unwrap();

    cmd_install_memory(&home, &config, &parts, false, None).unwrap();
    let settings = fs::read_to_string(home.join(".claude/settings.json")).unwrap();
    let want = e.at("stores/personal/memory");
    assert!(
        settings.contains(&format!("\"autoMemoryDirectory\": \"{}\"", want.display())),
        "{settings}"
    );
    assert!(settings.contains("\"theme\": \"dark\""));
    let list = fs::read_to_string(parts.join("maid-harness.json")).unwrap();
    assert!(
        list.contains("\"kind\": \"setting\"")
            && list.contains("\"name\": \"autoMemoryDirectory\""),
        "{list}"
    );
    assert!(!list.contains("skipped"), "{list}");

    // Whole-status reads the recorded setting off claude's settings file.
    let whole = build_tool::whole::check(
        &parts,
        &home,
        Some(bin.clone().into_os_string()),
        Some(build_tool::shared::Agent::Claude),
    )
    .unwrap();
    assert!(!whole.failed(), "{whole}");
    assert!(
        whole.to_string().contains("setting autoMemoryDirectory"),
        "{whole}"
    );

    cmd_uninstall_memory(&home, &parts, false, None).unwrap();
    let settings = fs::read_to_string(home.join(".claude/settings.json")).unwrap();
    assert!(
        !settings.contains("autoMemoryDirectory") && settings.contains("theme"),
        "{settings}"
    );
    assert!(!parts.join("maid-harness.json").exists());

    // Round trip through the real part list: the recorded value lets a
    // later install repoint it, and take it away once there is no default.
    cmd_install_memory(&home, &config, &parts, false, None).unwrap();
    e.ok(&[
        "memory",
        "add-store",
        "personal",
        e.at("stores/work").to_str().unwrap(),
    ]);
    cmd_install_memory(&home, &config, &parts, false, None).unwrap();
    let settings = fs::read_to_string(home.join(".claude/settings.json")).unwrap();
    assert!(
        settings.contains(&e.at("stores/work/memory").display().to_string()),
        "{settings}"
    );
    e.ok(&["memory", "remove-store", "personal"]);
    cmd_install_memory(&home, &config, &parts, false, None).unwrap();
    let settings = fs::read_to_string(home.join(".claude/settings.json")).unwrap();
    assert!(!settings.contains("autoMemoryDirectory"), "{settings}");
    let list = fs::read_to_string(parts.join("maid-harness.json")).unwrap();
    assert!(
        list.contains("\"skipped\": \"mAId names no default memory store\""),
        "{list}"
    );

    // Another part's value: left, and the skip is recorded without a path.
    fs::write(
        home.join(".claude/settings.json"),
        "{\"autoMemoryDirectory\": \"/x/memory\"}",
    )
    .unwrap();
    cmd_install_memory(&home, &config, &parts, false, None).unwrap();
    assert!(fs::read_to_string(home.join(".claude/settings.json"))
        .unwrap()
        .contains("/x/memory"));
    let list = fs::read_to_string(parts.join("maid-harness.json")).unwrap();
    assert!(list.contains("\"skipped\": \"not mAId's\""), "{list}");

    // The exact value install would write, set by hand: adopted through the
    // real part list, kept adopted on reinstall, removed on uninstall.
    e.ok(&[
        "memory",
        "add-store",
        "personal",
        e.at("stores/personal").to_str().unwrap(),
        "--default",
    ]);
    fs::write(
        home.join(".claude/settings.json"),
        format!("{{\"autoMemoryDirectory\": \"{}\"}}", want.display()),
    )
    .unwrap();
    for _ in 0..2 {
        cmd_install_memory(&home, &config, &parts, false, None).unwrap();
        let list = fs::read_to_string(parts.join("maid-harness.json")).unwrap();
        assert!(list.contains("\"adopted\": true"), "{list}");
    }
    cmd_uninstall_memory(&home, &parts, false, None).unwrap();
    let settings = fs::read_to_string(home.join(".claude/settings.json")).unwrap();
    assert!(!settings.contains("autoMemoryDirectory"), "{settings}");

    // The same value recorded in another part's list: not adopted, so only
    // one list claims it, and uninstall leaves it.
    let theirs = format!("{{\"autoMemoryDirectory\": \"{}\"}}", want.display());
    fs::write(home.join(".claude/settings.json"), &theirs).unwrap();
    fs::write(
        parts.join("other.json"),
        format!(
            r#"{{"format": 1, "part": "other", "harnesses": {{"claude": [{{"kind": "setting", "name": "autoMemoryDirectory", "value": "{}"}}]}}}}"#,
            want.display()
        ),
    )
    .unwrap();
    cmd_install_memory(&home, &config, &parts, false, None).unwrap();
    let list = fs::read_to_string(parts.join("maid-harness.json")).unwrap();
    assert!(list.contains("\"skipped\": \"another part's\""), "{list}");
    assert!(!list.contains("adopted"), "{list}");
    cmd_uninstall_memory(&home, &parts, false, None).unwrap();
    assert_eq!(
        fs::read_to_string(home.join(".claude/settings.json")).unwrap(),
        theirs
    );
}
