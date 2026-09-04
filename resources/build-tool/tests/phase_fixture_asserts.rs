//! Proves the agent-driven phase fixtures are worth paying for.
//!
//! An `.smoke` fixture's assert block is only evidence if it *fails* when
//! the agent does nothing. That is easy to get wrong and expensive to
//! discover — a vacuous assert makes a paid run report success while
//! testing nothing. So before any fixture is run against a real agent,
//! this suite runs its setup, does no agent work at all, and requires the
//! assert to fail.
//!
//! Free, deterministic, and it runs on every build.

use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

use build_tool::shared::repo_root;

fn root() -> PathBuf {
    repo_root().expect("repo root resolves under cargo test")
}

fn fixtures_dir() -> PathBuf {
    root().join("resources/tests/skills")
}

fn tools_dir() -> PathBuf {
    root().join("resources/content/skills/kdevkit/tools")
}

/// The `--- setup ---` and `--- assert ---` blocks of a fixture.
fn blocks(path: &Path) -> (String, String) {
    let body = std::fs::read_to_string(path).unwrap();
    let section = |name: &str| -> String {
        let marker = format!("--- {name} ---");
        let start = body
            .find(&marker)
            .unwrap_or_else(|| panic!("{} has no {marker}", path.display()))
            + marker.len();
        let rest = &body[start..];
        let end = rest.find("\n--- ").unwrap_or(rest.len());
        rest[..end].trim_start_matches('\n').to_string()
    };
    (section("setup"), section("assert"))
}

/// Run a script the way the fixture harness does, with the same
/// environment a fixture can rely on.
fn run(script: &str, dir: &Path) -> (bool, String) {
    let out = Command::new("bash")
        .arg("-c")
        .arg(format!("set -e\n{script}"))
        .current_dir(dir)
        .env("KDEVKIT_TOOLS", tools_dir())
        .output()
        .expect("bash runs");
    let mut combined = String::from_utf8_lossy(&out.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), combined)
}

/// Every fixture that exercises the phase machinery. Named explicitly
/// rather than globbed, so adding one is a deliberate act that shows up in
/// review.
const PHASE_FIXTURES: &[&str] = &[
    "kdevkit-phase-dev-to-review.smoke",
    "kdevkit-phase-gate-holds.smoke",
    "kdevkit-phase-return-to-plan.smoke",
    "kdevkit-phase-side-quest.smoke",
];

#[test]
fn every_phase_fixture_setup_succeeds_on_its_own() {
    // A setup that fails would make the whole fixture inconclusive, and the
    // failure would only show up mid-paid-run.
    for name in PHASE_FIXTURES {
        let tmp = TempDir::new().unwrap();
        let (setup, _) = blocks(&fixtures_dir().join(name));
        let (ok, out) = run(&setup, tmp.path());
        assert!(
            ok,
            "{name}: setup failed, so the fixture is unrunnable:\n{out}"
        );
    }
}

#[test]
fn every_phase_fixture_assert_fails_when_the_agent_does_nothing() {
    // The whole point. If this passes, the fixture is measuring the seed
    // rather than the agent, and a paid run would report a false clean.
    for name in PHASE_FIXTURES {
        let tmp = TempDir::new().unwrap();
        let path = fixtures_dir().join(name);
        let (setup, assert_block) = blocks(&path);

        let (ok, out) = run(&setup, tmp.path());
        assert!(ok, "{name}: setup failed:\n{out}");

        let (passed, out) = run(&assert_block, tmp.path());
        assert!(
            !passed,
            "{name}: the assert block PASSED with no agent work at all.\n\
             This fixture cannot detect a no-op agent and must not be paid \
             for until it can.\n{out}"
        );
    }
}

#[test]
fn no_phase_fixture_assert_can_be_satisfied_by_deleting_the_record() {
    // A negative check like "the stage is not planning" is satisfied by
    // having no stage at all. Each fixture must reject an emptied record,
    // not merely an unchanged one.
    for name in PHASE_FIXTURES {
        let tmp = TempDir::new().unwrap();
        let path = fixtures_dir().join(name);
        let (setup, assert_block) = blocks(&path);
        let (ok, out) = run(&setup, tmp.path());
        assert!(ok, "{name}: setup failed:\n{out}");

        // The laziest possible "work": strip the spec's plan items and the
        // handoff section entirely, and make no commits.
        let strip = r#"
for f in specs/feature/*.md; do
  [ -f "$f" ] || continue
  sed -i '/^- \[ \]/d; /^## Handoff/,/^## /{/^## Handoff/d}' "$f"
done
"#;
        let _ = run(strip, tmp.path());
        let (passed, out) = run(&assert_block, tmp.path());
        assert!(
            !passed,
            "{name}: the assert block passed after the record was gutted \
             rather than the work done:\n{out}"
        );
    }
}
