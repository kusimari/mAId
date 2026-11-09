//! The plugin deployment against the real claude and codex CLIs, in a temp
//! HOME. No model calls, but it needs both CLIs and takes seconds (the
//! bring-in test also clones the vendor repo), so it is ignored by
//! `just test`:
//!
//!   cargo test -p build-tool --test plugins -- --ignored

use build_tool::deploy::{BringIns, Deploy, Plugins, State};
use build_tool::part::{self, Item, What};
use build_tool::shared::{on_path, Agent, BRING_INS};
use build_tool::stages;
use shell_claude::config_dir::ConfigDir;
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
    if !real_clis() {
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
    // codex takes the links dir; claude the profile's own marketplace.
    let root = state.join("marketplace");
    let claude_root = profile.join("share/maid/marketplace");
    let p = Plugins {
        home: h.to_path_buf(),
        claude: ConfigDir::new(h, None),
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
        [State::Ok(claude_root.clone()), State::Ok(root.clone())]
    );
    for (cli, root) in [("claude", &claude_root), ("codex", &root)] {
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
    let mut placed = p.placed(None).unwrap();
    placed.sort_by_key(|(a, i)| (a.cli(), i.clone()));
    let both = |a| {
        [
            (a, Item::placed(What::Plugin, "maid@maid")),
            (a, Item::placed(What::Marketplace, "maid")),
        ]
    };
    assert_eq!(placed, [both(Agent::Claude), both(Agent::Codex)].concat());

    // claude's marketplace where an older mAId pointed it, the links dir:
    // install moves it to the profile's own.
    run(
        h,
        "claude",
        &["plugin", "marketplace", "add", root.to_str().unwrap()],
    );
    assert!(!p.is_deployed(Agent::Claude));
    p.install(None, false, false).unwrap();
    assert_eq!(
        marketplaces(&p),
        [State::Ok(claude_root.clone()), State::Ok(root.clone())]
    );

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
        [State::Ok(claude_root.clone()), State::Ok(root.clone())]
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
        claude: ConfigDir::new(h, None),
        profile: other_profile,
    };
    other.install(None, false, false).unwrap();
    assert!(matches!(states(&other)[1], State::Disabled { .. }));
    p.install(None, false, false).unwrap();
    assert_eq!(states(&p), [disabled("aaaa"), disabled("bbbb")]);
    assert_eq!(
        marketplaces(&p),
        [State::Ok(claude_root.clone()), State::Ok(root.clone())]
    );
    assert!(!p.is_deployed(Agent::Claude));

    // Uninstall: nothing listed, no marketplace, no root.
    p.uninstall(None, false, false).unwrap();
    assert_eq!(states(&p), [State::Missing, State::Missing]);
    assert_eq!(marketplaces(&p), [State::Missing, State::Missing]);
    for (cli, root) in [("claude", &claude_root), ("codex", &root)] {
        let listed = run(h, cli, &["plugin", "marketplace", "list", "--json"]);
        assert!(
            !listed.contains(&root.display().to_string()),
            "{cli}: {listed}"
        );
    }
    assert!(!root.exists());
    assert_eq!(p.placed(None).unwrap(), vec![]);
}

/// Whether this run can drive the real CLIs without reaching real config.
fn real_clis() -> bool {
    // Either override would point a CLI past the temp HOME at real config.
    let overridden = ["CODEX_HOME", "CLAUDE_CONFIG_DIR"]
        .iter()
        .any(|v| std::env::var_os(v).is_some());
    let ok = on_path("claude") && on_path("codex") && !overridden;
    if !ok {
        eprintln!(
            "skipped: needs claude and codex on PATH, and CODEX_HOME and CLAUDE_CONFIG_DIR unset"
        );
    }
    ok
}

#[test]
#[ignore = "drives the real claude and codex CLIs, and clones the vendor repo"]
fn bring_ins_install_and_uninstall_only_what_maid_added() {
    if !real_clis() {
        return;
    }
    let home = TempDir::new().unwrap();
    let h = home.path();
    let parts = h.join(".local/state/my-harness/parts");
    let content = h.join("content");
    fs::create_dir_all(content.join("skills/alpha")).unwrap();
    fs::write(
        content.join("skills/alpha/SKILL.md"),
        "---\nname: alpha\ndescription: Alpha.\n---\n",
    )
    .unwrap();
    // A fresh one per step, as each `just` run reads ownership anew.
    let target =
        || BringIns::new(h.to_path_buf(), ConfigDir::new(h, None), BRING_INS, &parts).unwrap();
    let install = || stages::cmd_install(&target(), &content, &parts, false, false, None).unwrap();
    let uninstall = || stages::cmd_uninstall(&target(), &parts, false, false, None).unwrap();
    let b = &BRING_INS[0];
    let (market, plugin) = (b.marketplace, b.plugin_id(b.plugins[0]));
    let other = format!("example-skills@{market}");
    let entries = |a: Agent| part::placed(&parts, a).unwrap();
    let listed = |cli: &str, id: &str| run(h, cli, &["plugin", "list", "--json"]).contains(id);
    let has_market =
        |cli: &str| run(h, cli, &["plugin", "marketplace", "list", "--json"]).contains(market);
    let mine = vec![
        Item::placed(What::Marketplace, market),
        Item::placed(What::Plugin, plugin.as_str()),
    ];

    // Nothing there: both placed, codex pinned to the commit.
    assert_eq!(install(), 0);
    for cli in ["claude", "codex"] {
        assert!(listed(cli, &plugin) && has_market(cli), "{cli}");
    }
    let config = fs::read_to_string(h.join(".codex/config.toml")).unwrap();
    assert!(config.contains(&format!("ref = \"{}\"", b.rev)), "{config}");
    let mut sorted = mine.clone();
    sorted.sort();
    assert_eq!(entries(Agent::Claude), sorted);
    assert_eq!(entries(Agent::Codex), sorted);

    // Again: nothing to do.
    assert!(target()
        .install(None, false, false)
        .unwrap()
        .iter()
        .all(|r| !r.acted));

    // The user installs another plugin from mAId's marketplace: uninstall
    // removes mAId's plugin and keeps the marketplace, which stays listed.
    run(
        h,
        "claude",
        &["plugin", "install", &other, "--scope", "user"],
    );
    run(h, "codex", &["plugin", "add", &other]);
    assert_eq!(uninstall(), 0);
    for cli in ["claude", "codex"] {
        assert!(
            !listed(cli, &plugin) && listed(cli, &other) && has_market(cli),
            "{cli}"
        );
    }
    assert_eq!(
        entries(Agent::Codex),
        [Item::placed(What::Marketplace, market)]
    );
    run(
        h,
        "claude",
        &["plugin", "uninstall", &other, "--scope", "user"],
    );
    run(h, "codex", &["plugin", "remove", &other]);
    assert_eq!(uninstall(), 0);
    for cli in ["claude", "codex"] {
        assert!(!has_market(cli), "{cli}");
    }
    assert!(!part::path(&parts).exists());

    // The user's own marketplace: kept and not claimed; mAId's plugin from
    // it is mAId's, and goes on uninstall.
    run(h, "claude", &["plugin", "marketplace", "add", b.source]);
    run(h, "codex", &["plugin", "marketplace", "add", b.source]);
    assert_eq!(install(), 0);
    let theirs = Item::skipped(What::Marketplace, market, "not mAId's");
    for a in [Agent::Claude, Agent::Codex] {
        assert!(target()
            .placed(Some(a))
            .unwrap()
            .contains(&(a, theirs.clone())));
        assert_eq!(entries(a), [Item::placed(What::Plugin, plugin.as_str())]);
    }
    assert_eq!(uninstall(), 0);
    for cli in ["claude", "codex"] {
        assert!(!listed(cli, &plugin) && has_market(cli), "{cli}");
    }
}

const UNREACHABLE: &[build_tool::shared::BringIn] = &[build_tool::shared::BringIn {
    marketplace: "unreachable",
    source: "https://127.0.0.1:9/none.git",
    rev: "0123456789012345678901234567890123456789",
    plugins: &["docs"],
    needs: &[],
}];

#[test]
#[ignore = "drives the real claude and codex CLIs"]
fn a_bring_in_that_cannot_be_reached_fails_the_run_and_is_recorded() {
    if !real_clis() {
        return;
    }
    let home = TempDir::new().unwrap();
    let h = home.path();
    let parts = h.join("parts");
    let content = h.join("content");
    fs::create_dir_all(content.join("skills/alpha")).unwrap();
    fs::write(
        content.join("skills/alpha/SKILL.md"),
        "---\nname: alpha\ndescription: Alpha.\n---\n",
    )
    .unwrap();
    let target = BringIns::new(
        h.to_path_buf(),
        ConfigDir::new(h, None),
        UNREACHABLE,
        &parts,
    )
    .unwrap();
    assert_eq!(
        stages::cmd_install(&target, &content, &parts, false, false, None).unwrap(),
        1
    );
    for a in [Agent::Claude, Agent::Codex] {
        let placed = target.placed(Some(a)).unwrap();
        for item in [
            Item::skipped(What::Marketplace, "unreachable", "failed"),
            Item::skipped(What::Plugin, "docs@unreachable", "failed"),
        ] {
            assert!(placed.contains(&(a, item)), "{placed:?}");
        }
    }
}
