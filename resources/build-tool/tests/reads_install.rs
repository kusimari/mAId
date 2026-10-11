//! Install's claude step for reading mAId's skill files, beside the memory
//! step, under a temp HOME. Its own test binary, because it sets PATH for
//! the whole process.

use build_tool::shared::Agent;
use build_tool::stages::{
    cmd_install_memory, cmd_install_reads, cmd_uninstall_memory, cmd_uninstall_reads,
};
use build_tool::whole;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

#[test]
fn install_adds_the_plugin_dir_keeps_the_rest_and_uninstall_puts_it_back() {
    let tmp = TempDir::new().unwrap();
    let root = fs::canonicalize(tmp.path()).unwrap();
    let (home, config, parts, bin) = (
        root.join("home"),
        root.join("config"),
        root.join("state"),
        root.join("bin"),
    );
    let profile = root.join("xdg/maid/profile");
    let plugin = root.join("xdg/maid/marketplace/plugins/maid");
    for d in [&home.join(".claude"), &config, &bin, &plugin] {
        fs::create_dir_all(d).unwrap();
    }
    fs::write(bin.join("claude"), "#!/bin/sh\n").unwrap();
    fs::set_permissions(bin.join("claude"), fs::Permissions::from_mode(0o755)).unwrap();
    std::env::set_var("PATH", &bin);

    let at = home.join(".claude/settings.json");
    let hand_rule = "Read(//elsewhere/marketplace/plugins/**)";
    let before = format!(
        "{{\n  \"theme\": \"dark\",\n  \"permissions\": {{\n    \"allow\": [\n      \"{hand_rule}\"\n    ]\n  }}\n}}\n"
    );
    fs::write(&at, &before).unwrap();

    cmd_install_memory(&home, &config, &parts, false, None).unwrap();
    cmd_install_reads(&home, &profile, &parts, false, None).unwrap();
    let mine = root.join("xdg/maid/marketplace/plugins/maid");
    let settings: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&at).unwrap()).unwrap();
    assert_eq!(
        settings["permissions"]["additionalDirectories"],
        serde_json::json!([mine.display().to_string()])
    );
    assert_eq!(
        settings["permissions"]["allow"],
        serde_json::json!([hand_rule])
    );

    // Both claude settings are in the part list; neither step erased the other.
    let list = fs::read_to_string(parts.join("maid-harness.json")).unwrap();
    assert!(list.contains("\"name\": \"autoMemoryDirectory\""), "{list}");
    assert!(
        list.contains("\"name\": \"permissions.additionalDirectories\"")
            && list.contains(&format!("\"value\": \"{}\"", mine.display())),
        "{list}"
    );

    let check = || {
        whole::check(
            &parts,
            &home,
            Some(bin.clone().into_os_string()),
            Some(Agent::Claude),
        )
        .unwrap()
    };
    let report = check().to_string();
    assert!(
        report.contains("present        setting permissions.additionalDirectories"),
        "{report}"
    );
    fs::write(&at, &before).unwrap();
    let report = check().to_string();
    assert!(
        report.contains("missing        setting permissions.additionalDirectories"),
        "{report}"
    );
    cmd_install_reads(&home, &profile, &parts, false, None).unwrap();

    cmd_uninstall_reads(&home, &parts, false, None).unwrap();
    cmd_uninstall_memory(&home, &parts, false, None).unwrap();
    assert_eq!(fs::read_to_string(&at).unwrap(), before);
    assert!(!parts.join("maid-harness.json").exists());
}
