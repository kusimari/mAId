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
