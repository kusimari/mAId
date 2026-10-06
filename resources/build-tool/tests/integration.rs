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

/// The developer-judgement slot, as shipped. kdevkit must reach the judge
/// by role and never by name, so another skill can fill it; exactly one
/// shipped skill fills it, so role resolution has one answer; and the
/// `judgement:` setting that overrides it is documented where kdevkit's
/// other settings are.
#[test]
fn shipped_judgement_role_contract() {
    let skills = repo_root()
        .expect("repo root resolves under cargo test")
        .join("resources/content/skills");
    let read = |p: &std::path::Path| std::fs::read_to_string(p).expect("readable");

    let mut kdevkit = Vec::new();
    let mut stack = vec![skills.join("kdevkit")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("kdevkit dir") {
            let path = entry.expect("entry").path();
            match path.is_dir() {
                true => stack.push(path),
                false if path.extension().and_then(|e| e.to_str()) == Some("md") => {
                    kdevkit.push((path.clone(), read(&path)))
                }
                false => {}
            }
        }
    }
    for (path, text) in &kdevkit {
        assert!(
            !text.to_lowercase().contains("kyodakit"),
            "{} names a specific judge; kdevkit must name the role only",
            path.display()
        );
    }
    let core = read(&skills.join("kdevkit/SKILL.md"));
    assert!(
        core.contains("developer-judgement"),
        "kdevkit SKILL.md no longer reaches the developer-judgement role"
    );
    assert!(
        read(&skills.join("kdevkit/setup.md")).contains("judgement:"),
        "the judgement: setting is not documented in kdevkit setup.md"
    );

    let fillers: Vec<String> = std::fs::read_dir(&skills)
        .expect("skills dir")
        .filter_map(|e| {
            let dir = e.ok()?.path();
            let text = std::fs::read_to_string(dir.join("SKILL.md")).ok()?;
            let fills = text.contains("Fills the developer-judgement role")
                || text.contains("fills the **developer-judgement** role");
            fills.then(|| dir.file_name().unwrap().to_string_lossy().to_string())
        })
        .collect();
    assert_eq!(
        fillers.len(),
        1,
        "expected exactly one shipped skill filling the role, found {fillers:?}"
    );
}
