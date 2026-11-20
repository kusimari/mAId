//! Install's claude step for reading mAId's skill files, beside the memory
//! step, under a temp HOME. Its own test binary, because it sets PATH for
//! the whole process.

use plain::claude_config::ConfigDir;
use plain::memory::{cmd_install_memory, cmd_uninstall_memory};
use plain::reads::{cmd_install_reads, cmd_uninstall_reads};
use plain::shared::Agent;
use plain::status;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

#[test]
fn install_adds_the_plugin_dir_keeps_the_rest_and_uninstall_puts_it_back() {
    let tmp = TempDir::new().unwrap();
    // Everything sits behind a symlink, as a HOME linked elsewhere does.
    let real = fs::canonicalize(tmp.path()).unwrap().join("real");
    fs::create_dir(&real).unwrap();
    let root = real.with_file_name("link");
    std::os::unix::fs::symlink(&real, &root).unwrap();
    let (home, config, parts, bin) = (
        root.join("home"),
        root.join("config"),
        root.join("state"),
        root.join("bin"),
    );
    let profile = root.join("xdg/maid/profile");
    let plugin = profile.join("share/maid/marketplace/plugins/maid");
    for d in [&home.join(".claude"), &config, &bin, &plugin] {
        fs::create_dir_all(d).unwrap();
    }
    fs::write(bin.join("claude"), "#!/bin/sh\n").unwrap();
    fs::set_permissions(bin.join("claude"), fs::Permissions::from_mode(0o755)).unwrap();
    std::env::set_var("PATH", &bin);
    let cc = ConfigDir::new(&home, None);

    let at = home.join(".claude/settings.json");
    let hand_rule = "Read(//elsewhere/marketplace/plugins/**)";
    let before = format!(
        "{{\n  \"theme\": \"dark\",\n  \"permissions\": {{\n    \"allow\": [\n      \"{hand_rule}\"\n    ]\n  }}\n}}\n"
    );
    fs::write(&at, &before).unwrap();

    cmd_install_memory(
        &cc,
        &config.join("memory-store"),
        &profile,
        &parts,
        false,
        None,
    )
    .unwrap();
    cmd_install_reads(&cc, &profile, &config.join("maid"), &parts, false, None).unwrap();
    let mine = profile.join("share/maid/marketplace/plugins/maid");
    let settings: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&at).unwrap()).unwrap();
    assert_eq!(
        settings["permissions"]["additionalDirectories"],
        serde_json::json!([mine.display().to_string()])
    );
    // The config grant: an Edit rule on writing-style's state dir, as given
    // and as it resolves, beside the user's own rule.
    let edit = format!("Edit(/{}/maid/writing-style/**)", config.display());
    let resolved = format!("Edit(/{}/config/maid/writing-style/**)", real.display());
    assert_eq!(
        settings["permissions"]["allow"],
        serde_json::json!([hand_rule, edit, resolved])
    );

    // Both claude settings are in the install manifest; neither step erased the other.
    let list = fs::read_to_string(parts.join("maid.json")).unwrap();
    assert!(
        list.contains("\"name\": \"env.CLAUDE_CODE_DISABLE_AUTO_MEMORY\""),
        "{list}"
    );
    assert!(
        list.contains("\"name\": \"permissions.additionalDirectories\"")
            && list.contains(&format!("\"value\": \"{}\"", mine.display())),
        "{list}"
    );
    assert!(
        list.contains("\"name\": \"permissions.allow\"")
            && list.contains(&format!("\"value\": \"{edit}\""))
            && list.contains(&format!("\"value\": \"{resolved}\"")),
        "{list}"
    );

    let check = || {
        status::check(
            std::slice::from_ref(&parts),
            &home,
            &cc,
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
    assert!(
        report.contains("present        setting permissions.allow"),
        "{report}"
    );
    fs::write(&at, &before).unwrap();
    let report = check().to_string();
    assert!(
        report.contains("missing        setting permissions.additionalDirectories"),
        "{report}"
    );
    cmd_install_reads(&cc, &profile, &config.join("maid"), &parts, false, None).unwrap();

    cmd_uninstall_reads(&cc, &parts, false, None).unwrap();
    cmd_uninstall_memory(&cc, &profile, &parts, false, None).unwrap();
    assert_eq!(fs::read_to_string(&at).unwrap(), before);
    assert!(!parts.join("maid.json").exists());

    // The same element, set by hand and recorded in another package's manifest: not
    // adopted, so only one list claims it, and uninstall leaves it.
    let dir = mine.display().to_string();
    let theirs = serde_json::json!({"permissions": {"additionalDirectories": [dir]}}).to_string();
    fs::write(&at, &theirs).unwrap();
    fs::write(
        parts.join("other.json"),
        serde_json::json!({"format": 1, "package": "other", "agents": {"claude": [
            {"kind": "setting", "name": "permissions.additionalDirectories", "value": dir}]}})
        .to_string(),
    )
    .unwrap();
    cmd_install_reads(&cc, &profile, &config.join("maid"), &parts, false, None).unwrap();
    let list = fs::read_to_string(parts.join("maid.json")).unwrap();
    assert!(list.contains("\"skipped\": \"another part's\""), "{list}");
    assert!(!list.contains("adopted"), "{list}");
    cmd_uninstall_reads(&cc, &parts, false, None).unwrap();
    // mAId's config rule came and went, so the file was rewritten: compare
    // values.
    let after: serde_json::Value = serde_json::from_str(&fs::read_to_string(&at).unwrap()).unwrap();
    assert_eq!(after.to_string(), theirs);
}
