//! The plugin deployment against the real claude and codex CLIs, in a temp
//! HOME. No model calls, but it needs both CLIs and takes seconds, so it
//! is ignored by `just test`:
//!
//!   cargo test -p build-tool --test plugins -- --ignored

use build_tool::deploy::{Deploy, Plugins, State};
use build_tool::shared::{on_path, Agent};
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

/// A profile generation laid out as flake.nix builds it, with one skill
/// whose text and plugin version are `tag`.
fn generation(dir: &Path, tag: &str) {
    let m = dir.join("share/maid/marketplace");
    let p = m.join("plugins/maid");
    let put = |at: &Path, text: &str| {
        fs::create_dir_all(at.parent().unwrap()).unwrap();
        fs::write(at, text).unwrap();
    };
    put(
        &m.join(".claude-plugin/marketplace.json"),
        r#"{"name":"maid","owner":{"name":"mAId"},"plugins":[{"name":"maid","source":"./plugins/maid"}]}"#,
    );
    put(
        &m.join(".agents/plugins/marketplace.json"),
        r#"{"name":"maid","plugins":[{"name":"maid","source":{"source":"local","path":"./plugins/maid"},"policy":{"installation":"AVAILABLE"}}]}"#,
    );
    let version = format!("1.0.0-{tag}");
    put(
        &p.join(".claude-plugin/plugin.json"),
        &format!(r#"{{"name":"maid","version":"{version}"}}"#),
    );
    put(
        &p.join(".codex-plugin/plugin.json"),
        &format!(r#"{{"name":"maid","version":"{version}","skills":"./skills/"}}"#),
    );
    put(
        &p.join("skills/alpha/SKILL.md"),
        &format!("---\nname: alpha\ndescription: Alpha.\n---\nSay {tag}.\n"),
    );
}

fn switch(profile: &Path, to: &Path) {
    let _ = fs::remove_file(profile);
    std::os::unix::fs::symlink(to, profile).unwrap();
}

fn run(home: &Path, cli: &str, args: &[&str]) -> String {
    let out = Command::new(cli)
        .args(args)
        .env("HOME", home)
        .output()
        .unwrap();
    assert!(out.status.success(), "{cli} {args:?}: {out:?}");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// The plugin states, claude then codex.
fn states(p: &Plugins) -> Vec<State> {
    p.status(None)
        .unwrap()
        .into_iter()
        .filter(|r| r.label.contains(" plugin "))
        .map(|r| r.state)
        .collect()
}

/// The marketplace states, claude then codex.
fn marketplaces(p: &Plugins) -> Vec<State> {
    p.status(None)
        .unwrap()
        .into_iter()
        .filter(|r| r.label.contains(" marketplace "))
        .map(|r| r.state)
        .collect()
}

#[test]
#[ignore = "drives the real claude and codex CLIs"]
fn plugins_install_update_disable_and_uninstall_with_the_real_clis() {
    // Either override would point a CLI past the temp HOME at real config.
    let overridden = ["CODEX_HOME", "CLAUDE_CONFIG_DIR"]
        .iter()
        .any(|v| std::env::var_os(v).is_some());
    if !(on_path("claude") && on_path("codex")) || overridden {
        eprintln!(
            "skipped: needs claude and codex on PATH, and CODEX_HOME and CLAUDE_CONFIG_DIR unset"
        );
        return;
    }
    let home = TempDir::new().unwrap();
    let h = home.path();
    let state = h.join(".local/state/maid");
    let (a, b) = (state.join("gen-a"), state.join("gen-b"));
    generation(&a, "aaaa");
    generation(&b, "bbbb");
    let profile = state.join("profile");
    switch(&profile, &a);
    let root = state.join("marketplace");
    let p = Plugins {
        home: h.to_path_buf(),
        profile: profile.clone(),
    };
    // A link an older mAId install left.
    fs::create_dir_all(h.join(".claude")).unwrap();
    std::os::unix::fs::symlink(profile.join("share/maid/skills"), h.join(".claude/skills"))
        .unwrap();

    // Install: both current, on the mAId marketplace dir, old link gone.
    p.install(None, false, false).unwrap();
    let current = |v: &str| State::Current(format!("1.0.0-{v}"));
    assert_eq!(states(&p), [current("aaaa"), current("aaaa")]);
    assert!(fs::symlink_metadata(h.join(".claude/skills")).is_err());
    assert_eq!(
        marketplaces(&p),
        [State::Ok(root.clone()), State::Ok(root.clone())]
    );
    for cli in ["claude", "codex"] {
        let listed = run(h, cli, &["plugin", "marketplace", "list", "--json"]);
        assert!(
            listed.contains(&root.display().to_string()),
            "{cli}: {listed}"
        );
        assert!(
            !listed.contains("gen-a"),
            "{cli} pinned a generation: {listed}"
        );
    }
    assert!(p.is_deployed(Agent::Claude) && p.is_deployed(Agent::Codex));

    // A new generation: both updated; codex's cache has the new text.
    switch(&profile, &b);
    p.install(None, false, false).unwrap();
    assert_eq!(states(&p), [current("bbbb"), current("bbbb")]);
    let cached = h.join(".codex/plugins/cache/maid/maid/1.0.0-bbbb/skills/alpha/SKILL.md");
    assert!(fs::read_to_string(cached).unwrap().contains("Say bbbb."));

    // codex's marketplace left on a dir that is gone (a crashed isolated
    // run): codex refuses every plugin command until install repairs it.
    let gone = state.join("gone/marketplace");
    fs::create_dir_all(&gone).unwrap();
    for entry in [".claude-plugin", ".agents", "plugins"] {
        std::os::unix::fs::symlink(root.join(entry), gone.join(entry)).unwrap();
    }
    run(h, "codex", &["plugin", "marketplace", "remove", "maid"]);
    run(
        h,
        "codex",
        &["plugin", "marketplace", "add", gone.to_str().unwrap()],
    );
    fs::remove_dir_all(state.join("gone")).unwrap();
    // Status still prints, with codex's lines unreadable.
    assert!(matches!(states(&p)[1], State::Unreadable(_)));
    p.install(None, false, false).unwrap();
    assert_eq!(states(&p), [current("bbbb"), current("bbbb")]);
    assert_eq!(
        marketplaces(&p),
        [State::Ok(root.clone()), State::Ok(root.clone())]
    );

    // Disabled by the user (codex has no disable verb; this is its switch),
    // then another install: both still disabled.
    run(h, "claude", &["plugin", "disable", "maid@maid"]);
    let config = h.join(".codex/config.toml");
    let text = fs::read_to_string(&config).unwrap();
    let off = text.replace(
        "[plugins.\"maid@maid\"]\nenabled = true",
        "[plugins.\"maid@maid\"]\nenabled = false",
    );
    assert_ne!(text, off, "codex config has no enabled maid@maid entry");
    fs::write(&config, off).unwrap();
    switch(&profile, &a);
    p.install(None, false, false).unwrap();
    let disabled = |found: &str| State::Disabled {
        found: format!("1.0.0-{found}"),
        want: "1.0.0-aaaa".into(),
    };
    // claude updates a disabled plugin and keeps it disabled; codex is left.
    assert_eq!(states(&p), [disabled("aaaa"), disabled("bbbb")]);

    // An isolated run's round trip: another profile's marketplace, then
    // back. Both repoint codex's marketplace; its plugin stays disabled.
    fs::create_dir_all(state.join("other")).unwrap();
    let other_profile = state.join("other/profile");
    switch(&other_profile, &b);
    let other = Plugins {
        home: h.to_path_buf(),
        profile: other_profile,
    };
    other.install(None, false, false).unwrap();
    assert!(matches!(states(&other)[1], State::Disabled { .. }));
    p.install(None, false, false).unwrap();
    assert_eq!(states(&p), [disabled("aaaa"), disabled("bbbb")]);
    assert_eq!(
        marketplaces(&p),
        [State::Ok(root.clone()), State::Ok(root.clone())]
    );
    assert!(!p.is_deployed(Agent::Claude));

    // Uninstall: nothing listed, no marketplace, no root.
    p.uninstall(None, false, false).unwrap();
    assert_eq!(states(&p), [State::Missing, State::Missing]);
    assert_eq!(marketplaces(&p), [State::Missing, State::Missing]);
    for cli in ["claude", "codex"] {
        let listed = run(h, cli, &["plugin", "marketplace", "list", "--json"]);
        assert!(
            !listed.contains(&root.display().to_string()),
            "{cli}: {listed}"
        );
    }
    assert!(!root.exists());
}
