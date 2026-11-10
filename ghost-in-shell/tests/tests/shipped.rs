//! The declarations this repo ships, read as the ghost reads them: every
//! skill, server and overlay under `resources/` and the ghost's own
//! plugin carry the bring-in contract, and the reflector only reads.

use ghost_in_shell_tests::{content, plugin};

fn files_under(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = vec![];
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        match p.is_dir() {
            true => out.extend(files_under(&p)),
            false => out.push(p),
        }
    }
    out
}

/// Every shipped skill is a turn bring-in with a job and a rules pointer
/// that names a few lines, not a whole skill (see project.md "My harness").
#[test]
fn shipped_skills_declare_the_contract() {
    use ghost::contract::{read, rules, Scope, Slot, Standard};
    // The person's skills, and the ghost's own beside the ghost.
    let dirs: Vec<std::path::PathBuf> = [content(), plugin()]
        .iter()
        .flat_map(|root| std::fs::read_dir(root.join("skills")).unwrap())
        .map(|e| e.unwrap().path())
        .collect();
    let mut names: Vec<_> = dirs
        .iter()
        .map(|d| d.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "browser",
            "kdevkit",
            "kreviewkit",
            "kyodakit",
            "notes",
            "reflect",
            "writing-style"
        ]
    );
    for name in &names {
        let dir = dirs.iter().find(|d| d.ends_with(name)).unwrap();
        let unit = dir.join("SKILL.md");
        let Ok(Slot::Declared(d)) = read(Standard::Skill, &unit) else {
            panic!("{name} is not declared");
        };
        assert_eq!(d.scope, Scope::Turn, "{name}");
        assert_eq!(d.contract, 1, "{name}");
        assert!(!d.job.is_empty(), "{name}");
        // Its repository is the plugin's it ships in.
        assert_eq!(d.repository, None, "{name}");
        // A skill reads a location only where the session names it, so no
        // run reaches the person's real config through a default. Where a
        // config key binds is the ghost's to resolve and name.
        for file in files_under(dir) {
            let text: String = std::fs::read_to_string(&file)
                .unwrap_or_default()
                .lines()
                .filter(|l| !l.trim_start().starts_with("my-harness.bind."))
                .collect::<Vec<_>>()
                .join("\n");
            for default in [
                "XDG_CONFIG_HOME",
                "~/.config",
                "$HOME/.config",
                "${HOME}/.config",
            ] {
                assert!(
                    !text.contains(default),
                    "{} names {default}",
                    file.display()
                );
            }
        }
        let lines = rules(&unit, d.rules.as_deref().expect(name)).unwrap();
        let n = lines.lines().count();
        assert!((2..=30).contains(&n), "{name}: rules are {n} lines");
        // No skill keeps a file tier: writing-style keeps its rules in the
        // persona's memory, and reads its older rules files until moved.
        assert!(d.state.is_none(), "{name} keeps no file tier");
        let keys: Vec<&str> = d.config.keys().map(String::as_str).collect();
        match name.as_str() {
            "writing-style" => {
                assert_eq!(keys, ["learned", "promoted"]);
                assert!(d.config.values().all(|k| k.optional));
                assert!(d
                    .config
                    .values()
                    .all(|k| k.bind.as_deref().is_some_and(|b| b.starts_with("path:"))));
                assert!(d.job.contains("persona's memory"), "{}", d.job);
            }
            _ => assert!(keys.is_empty(), "{name} binds no config"),
        }
        // Claude's grants on state dirs go to exactly the skills with
        // files to read there.
        let granted = plain::reads::STATEFUL_SKILLS.contains(&name.as_str());
        assert_eq!(granted, !keys.is_empty(), "{name}: STATEFUL_SKILLS");
    }
}

/// The browser server's manifest and the vendor plugin's overlay declare
/// the contract, and the overlay needs what plain status checks for.
#[test]
fn shipped_servers_and_overlays_declare_the_contract() {
    use ghost::contract::{read, Scope, Slot, Standard};
    use plain::external::VENDORS;
    let content = content();
    let declared = |standard, rel: &str| match read(standard, &content.join(rel)) {
        Ok(Slot::Declared(d)) => d,
        other => panic!("{rel}: {other:?}"),
    };
    let server = declared(Standard::Mcp, "servers/chrome-devtools/server.json");
    assert_eq!(server.scope, Scope::Turn);
    let memory = declared(Standard::Mcp, "servers/personal-memory/server.json");
    assert_eq!(memory.persona.as_deref(), Some("personal"));
    let binds: Vec<(&str, Option<&str>)> = memory
        .config
        .iter()
        .map(|(k, v)| (k.as_str(), v.bind.as_deref()))
        .collect();
    assert_eq!(
        binds,
        [
            ("index", Some("value:memory/MEMORY.md")),
            ("serve", Some("command")),
            ("store", Some("arg:--store"))
        ]
    );
    for vendor in VENDORS {
        for plugin in vendor.plugins {
            let d = declared(Standard::Plugin, &format!("overlays/{plugin}/plugin.json"));
            assert_eq!(d.scope, Scope::Turn);
            assert_eq!(
                d.requires, vendor.needs,
                "{plugin}: requires drifted from needs"
            );
        }
    }
}

/// The reflector reads and never writes: it reads untrusted session
/// content, so it holds no write authority (see project.md "My harness",
/// Adaptation). The reflect skill names the core the flake fills in, and
/// the reflector it dispatches.
#[test]
fn the_reflector_reads_only_and_the_reflect_skill_names_it_and_the_core() {
    #[derive(serde::Deserialize)]
    struct Agent {
        name: String,
        description: String,
        tools: String,
    }
    let plugin = plugin();
    let text = std::fs::read_to_string(plugin.join("agents/reflector.md")).unwrap();
    let agent = gray_matter::Matter::<gray_matter::engine::YAML>::new()
        .parse::<Agent>(&text)
        .unwrap()
        .data
        .unwrap();
    assert_eq!(agent.name, "reflector");
    assert!(!agent.description.is_empty());
    let tools: Vec<&str> = agent.tools.split(',').map(str::trim).collect();
    assert_eq!(tools, ["Read", "Grep", "Glob"]);
    let skill = std::fs::read_to_string(plugin.join("skills/reflect/SKILL.md")).unwrap();
    assert!(skill.contains("`ghost:reflector`"));
    assert!(skill.contains("@ghost-claude@ adaptation --session"));
    assert!(skill.contains("${CLAUDE_SESSION_ID}"));
}

#[test]
fn the_shipped_memory_declaration_names_its_package_store_and_index() {
    use ghost::contract::{read, Scope, Slot, Standard};
    let path = content().join("servers/personal-memory/server.json");
    let Ok(Slot::Declared(d)) = read(Standard::Mcp, &path) else {
        panic!("{path:?} holds no declaration");
    };
    assert_eq!(d.scope, Scope::Session);
    assert_eq!(d.package.as_deref(), Some("pensieve"));
    let keys: Vec<_> = d.config.keys().map(String::as_str).collect();
    assert_eq!(keys, ["index", "serve", "store"]);
    assert!(d.config.values().all(|k| !k.optional && !k.secret));
    let state = d.state.unwrap();
    assert_eq!(
        (state.local, state.permanent.as_deref()),
        (None, Some("store"))
    );
}

#[test]
fn the_shipped_adapter_declaration_is_in_the_ghosts_plugin_manifest() {
    use ghost::contract::{read, Scope, Slot, Standard};
    let manifest = plugin().join(".claude-plugin/plugin.json");
    let Ok(Slot::Declared(d)) = read(Standard::Plugin, &manifest) else {
        panic!("no declaration");
    };
    assert_eq!(d.scope, Scope::Session);
    assert_eq!(d.name.as_deref(), Some("claude-adapter"));
    assert!(d.job.contains("Claude Code"), "{}", d.job);
}

/// Every declaration this repo ships reads, as `ghost-claude validate`
/// checks it before an install.
#[test]
fn every_shipped_declaration_validates() {
    let n = shell_claude::validate::validate(&[content(), plugin()]).unwrap();
    assert!(n > 8, "{n}");
}
