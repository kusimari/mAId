//! Cross-stage tests — the ones that don't belong to any single stage's
//! unit suite because they exercise the crate from outside, against the
//! real repository rather than a synthetic tree.

use build_tool::shared::repo_root;
use build_tool::stages::check_content;

/// Every unit test builds a synthetic tree in a `TempDir`, so none of
/// them look at what actually deploys — which is how two skills reached
/// `main` with a `description:` that YAML read as a nested mapping (an
/// unquoted `": "` inside the value). The suite was green;
/// `install-skills` refused to run. This closes that gap: if content in
/// the repo can't be installed, `just test` says so.
#[test]
fn shipped_content_validates() {
    let content = repo_root()
        .expect("repo root resolves under cargo test")
        .join("resources/content");
    match check_content(&content) {
        Ok(n) => assert!(
            n > 0,
            "no skills found under {} — the walk is broken",
            content.display()
        ),
        Err(errs) => panic!(
            "shipped content is not installable:\n  {}",
            errs.join("\n  ")
        ),
    }
}

/// Every shipped `.smoke` fixture parses, and yields the kinds its
/// sections imply. The unit tests build synthetic fixtures, so without
/// this the parser can be green while the real suite is unrunnable —
/// the same gap `shipped_content_validates` closes for content.
#[test]
fn shipped_fixtures_parse() {
    use build_tool::harness::Fixture;

    let dir = repo_root()
        .expect("repo root resolves under cargo test")
        .join("resources/tests/skills");
    let mut seen = 0;
    for entry in std::fs::read_dir(&dir).expect("fixture dir exists") {
        let path = entry.expect("readable entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("smoke") {
            continue;
        }
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let body = std::fs::read_to_string(&path).expect("readable fixture");
        let fixture = Fixture::parse(&name, &body)
            .unwrap_or_else(|e| panic!("{name}.smoke does not parse: {e}"));
        assert!(!fixture.skill.is_empty());
        assert!(!fixture.agents.is_empty());
        assert!(
            !fixture.kinds().is_empty(),
            "{name} yields no kinds — it would run nothing"
        );
        seen += 1;
    }
    assert!(seen > 0, "no fixtures found under {}", dir.display());
}

/// The senior-developer judge slot, as shipped. kdevkit names
/// `kyodakit` as its default judge and `judgement:` as the override, the
/// default ships beside it, and the setting is documented where kdevkit's
/// other settings are.
#[test]
fn shipped_judgement_role_contract() {
    let skills = repo_root()
        .expect("repo root resolves under cargo test")
        .join("resources/content/skills");
    let read = |p: &std::path::Path| std::fs::read_to_string(p).expect("readable");

    let core = read(&skills.join("kdevkit/SKILL.md"));
    assert!(
        core.contains("The judge is `kyodakit` by\ndefault"),
        "kdevkit SKILL.md no longer names kyodakit as its default judge"
    );
    assert!(
        core.contains("overrides it with `judgement:`"),
        "kdevkit SKILL.md no longer offers the judgement: override"
    );
    assert!(
        skills.join("kyodakit/SKILL.md").is_file(),
        "the default judge, kyodakit, does not ship"
    );
    assert!(
        read(&skills.join("kdevkit/setup.md")).contains("judgement:"),
        "the judgement: setting is not documented in kdevkit setup.md"
    );
}

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
    use maid_core::contract::{read, rules, Scope, Slot, Standard};
    let skills = repo_root().unwrap().join("resources/content/skills");
    let mut names: Vec<_> = std::fs::read_dir(&skills)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
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
        let unit = skills.join(name).join("SKILL.md");
        let Ok(Slot::Declared(d)) = read(Standard::Skill, &unit) else {
            panic!("{name} is not declared");
        };
        assert_eq!(d.scope, Scope::Turn, "{name}");
        assert!(!d.job.is_empty(), "{name}");
        // A skill reads a location only where the session names it, so no
        // run reaches the person's real config through a default.
        for file in files_under(&skills.join(name)) {
            let text = std::fs::read_to_string(&file).unwrap_or_default();
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
                assert!(d.job.contains("persona's memory"), "{}", d.job);
            }
            _ => assert!(keys.is_empty(), "{name} binds no config"),
        }
        // Claude's grants on state dirs go to exactly the skills with
        // files to read there.
        let granted = build_tool::reads::STATEFUL_SKILLS.contains(&name.as_str());
        assert_eq!(granted, !keys.is_empty(), "{name}: STATEFUL_SKILLS");
    }
}

/// The browser server's manifest and the vendor plugin's overlay declare
/// the contract, and the overlay needs what plain status checks for.
#[test]
fn shipped_servers_and_overlays_declare_the_contract() {
    use build_tool::shared::BRING_INS;
    use maid_core::contract::{read, Scope, Slot, Standard};
    let content = repo_root().unwrap().join("resources/content");
    let declared = |standard, rel: &str| match read(standard, &content.join(rel)) {
        Ok(Slot::Declared(d)) => d,
        other => panic!("{rel}: {other:?}"),
    };
    let server = declared(Standard::Mcp, "servers/browser/server.json");
    assert_eq!(server.scope, Scope::Turn);
    for vendor in BRING_INS {
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
    let content = repo_root().unwrap().join("resources/content");
    let text = std::fs::read_to_string(content.join("agents/reflector.md")).unwrap();
    let agent = gray_matter::Matter::<gray_matter::engine::YAML>::new()
        .parse::<Agent>(&text)
        .unwrap()
        .data
        .unwrap();
    assert_eq!(agent.name, "reflector");
    assert!(!agent.description.is_empty());
    let tools: Vec<&str> = agent.tools.split(',').map(str::trim).collect();
    assert_eq!(tools, ["Read", "Grep", "Glob"]);
    let skill = std::fs::read_to_string(content.join("skills/reflect/SKILL.md")).unwrap();
    assert!(skill.contains("`maid:reflector`"));
    assert!(skill.contains("@maid-core@ adaptation --session"));
    assert!(skill.contains("${CLAUDE_SESSION_ID}"));
}
