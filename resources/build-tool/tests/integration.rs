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

/// Every shipped skill is a turn bring-in with a job and a rules pointer
/// that names a few lines, not a whole skill (see project.md "My harness").
#[test]
fn shipped_skills_declare_the_contract() {
    use maid_core::contract::{read, rules, Promote, Scope, Slot, Standard};
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
        let lines = rules(&unit, d.rules.as_deref().expect(name)).unwrap();
        let n = lines.lines().count();
        assert!((2..=30).contains(&n), "{name}: rules are {n} lines");
        let state = d.state.as_ref();
        match name.as_str() {
            "writing-style" => {
                let s = state.expect("writing-style keeps its rules");
                assert_eq!(s.local.as_deref(), Some("learned"));
                assert_eq!(s.permanent.as_deref(), Some("promoted"));
                assert_eq!(s.promote, Promote::Propose);
                assert!(s.keys().all(|k| d.config[k].optional));
            }
            _ => assert!(state.is_none(), "{name} keeps no state"),
        }
        // Claude's grants on state dirs go to exactly the skills with state.
        let granted = build_tool::reads::STATEFUL_SKILLS.contains(&name.as_str());
        assert_eq!(granted, state.is_some(), "{name}: STATEFUL_SKILLS");
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
