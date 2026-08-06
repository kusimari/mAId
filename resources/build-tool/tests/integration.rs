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
